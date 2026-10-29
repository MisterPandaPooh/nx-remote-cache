//! One dashboard for every replica: counters, latency histograms, the 24 h timeline, the event
//! feed and the retention sweeper's lock live in Redis (`NX_CACHE_STATS_BACKEND=redis`).
//!
//! The request path never talks to Redis: every pod keeps its own atomics (`stats.rs`) and a
//! background task pushes the deltas every `NX_CACHE_STATS_FLUSH_MS` in one pipeline, whatever
//! the traffic. Events go to a capped stream in batches, and each pod relays that stream to its
//! SSE clients, so the live view shows the whole cluster. When Redis is unreachable the dashboard
//! falls back to this pod's own numbers and the cache keeps serving.
//!
//! Keys under `<prefix>:`: `counters` and `hist:<route>` (hashes), `tl:<rfc3339>` (one hash per
//! 5-minute bucket, 25 h TTL), `events` (stream, `MAXLEN ~ 1000`), `pod:<name>` (heartbeat,
//! 15 s TTL), `sweep:lock` and `sweep:last`.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, Context as _, Result};
use redis::aio::ConnectionManager;
use redis::Value;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, OnceCell};

use crate::stats::{CountersSnapshot, Event, HistogramSnapshot, Stats, TimelinePoint, EVENT_RING, LATENCY_BOUNDS_MS};
use crate::store::SweepReport;

const ROUTES: [&str; 3] = ["get_hit", "get_miss", "put"];
const POD_TTL_SECS: u64 = 15;
const TIMELINE_TTL_SECS: u64 = 25 * 3600;
/// Events are pushed in batches this often; a batch is one pipeline whatever its size.
pub const EVENT_BATCH: Duration = Duration::from_millis(100);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PodInfo {
    pub name: String,
    pub started_at: String,
}

/// What the dashboard shows when the stats are shared: the sum over every pod alive or not.
pub struct ClusterSnapshot {
    pub counters: CountersSnapshot,
    pub swept_objects: u64,
    pub swept_bytes: u64,
    pub get_hit: HistogramSnapshot,
    pub get_miss: HistogramSnapshot,
    pub put: HistogramSnapshot,
    pub timeline: Vec<TimelinePoint>,
    pub last_sweep: Option<SweepReport>,
    pub pods: Vec<PodInfo>,
}

/// What the flusher has already pushed, so the next pass only sends the difference.
#[derive(Default)]
pub struct Flushed {
    counters: CountersSnapshot,
    swept_objects: u64,
    swept_bytes: u64,
    hist: [Option<HistogramSnapshot>; 3],
    timeline: HashMap<String, TimelinePoint>,
}

pub struct SharedStats {
    client: redis::Client,
    conn: OnceCell<ConnectionManager>,
    prefix: String,
    pod: String,
    pub flush_interval: Duration,
    /// Events of the whole cluster, read back from the stream.
    live: broadcast::Sender<Arc<Event>>,
}

impl SharedStats {
    pub fn new(
        url: &str,
        password: Option<&str>,
        prefix: &str,
        pod: &str,
        flush_interval: Duration,
    ) -> Result<Self, redis::RedisError> {
        let mut info: redis::ConnectionInfo = redis::IntoConnectionInfo::into_connection_info(url)?;
        if let Some(p) = password {
            let settings = info.redis_settings().clone().set_password(p);
            info = info.set_redis_settings(settings);
        }
        let (live, _) = broadcast::channel(512);
        Ok(Self {
            client: redis::Client::open(info)?,
            conn: OnceCell::new(),
            prefix: prefix.trim_matches(':').to_owned(),
            pod: pod.to_owned(),
            flush_interval,
            live,
        })
    }

    async fn conn(&self) -> Result<ConnectionManager> {
        let c = self
            .conn
            .get_or_try_init(|| ConnectionManager::new(self.client.clone()))
            .await?;
        Ok(c.clone())
    }

    fn key(&self, rest: &str) -> String {
        format!("{}:{rest}", self.prefix)
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Event>> {
        self.live.subscribe()
    }

    /// Push what changed since the last successful flush, plus this pod's heartbeat.
    pub async fn flush(&self, stats: &Stats, last: &mut Flushed) -> Result<()> {
        let mut p = redis::pipe();
        let ck = self.key("counters");
        let cur = stats.counters();
        let swept_objects = stats.swept_objects.load(std::sync::atomic::Ordering::Relaxed);
        let swept_bytes = stats.swept_bytes.load(std::sync::atomic::Ordering::Relaxed);
        for (field, now, before) in [
            ("hits", cur.hits, last.counters.hits),
            ("misses", cur.misses, last.counters.misses),
            ("puts", cur.puts, last.counters.puts),
            ("conflicts", cur.conflicts, last.counters.conflicts),
            ("unauthorized", cur.unauthorized, last.counters.unauthorized),
            ("forbidden", cur.forbidden, last.counters.forbidden),
            ("errors", cur.errors, last.counters.errors),
            ("bytes_served", cur.bytes_served, last.counters.bytes_served),
            ("bytes_stored", cur.bytes_stored, last.counters.bytes_stored),
            ("swept_objects", swept_objects, last.swept_objects),
            ("swept_bytes", swept_bytes, last.swept_bytes),
        ] {
            if now > before {
                p.cmd("HINCRBY").arg(&ck).arg(field).arg(now - before).ignore();
            }
        }

        let hists = [
            stats.get_hit.snapshot(),
            stats.get_miss.snapshot(),
            stats.put.snapshot(),
        ];
        for (i, route) in ROUTES.iter().enumerate() {
            let hk = self.key(&format!("hist:{route}"));
            let now = &hists[i];
            let zero = HistogramSnapshot {
                buckets: vec![0; now.buckets.len()],
                count: 0,
                sum_micros: 0,
            };
            let before = last.hist[i].as_ref().unwrap_or(&zero);
            for (b, (n, l)) in now.buckets.iter().zip(before.buckets.iter()).enumerate() {
                if n > l {
                    p.cmd("HINCRBY").arg(&hk).arg(format!("b{b}")).arg(n - l).ignore();
                }
            }
            if now.count > before.count {
                p.cmd("HINCRBY")
                    .arg(&hk)
                    .arg("count")
                    .arg(now.count - before.count)
                    .ignore();
            }
            if now.sum_micros > before.sum_micros {
                p.cmd("HINCRBY")
                    .arg(&hk)
                    .arg("sum_micros")
                    .arg(now.sum_micros - before.sum_micros)
                    .ignore();
            }
        }

        // Only the newest buckets can still move; older ones were flushed when they were new.
        let timeline = stats.timeline();
        let recent: Vec<TimelinePoint> = timeline.iter().rev().take(3).cloned().collect();
        let mut seen = HashMap::with_capacity(recent.len());
        for pt in &recent {
            let tk = self.key(&format!("tl:{}", pt.ts));
            let before = last.timeline.get(&pt.ts);
            let mut touched = false;
            for (field, now, was) in [
                ("hits", pt.hits, before.map_or(0, |b| b.hits)),
                ("misses", pt.misses, before.map_or(0, |b| b.misses)),
                ("puts", pt.puts, before.map_or(0, |b| b.puts)),
                ("bytes_served", pt.bytes_served, before.map_or(0, |b| b.bytes_served)),
                ("bytes_stored", pt.bytes_stored, before.map_or(0, |b| b.bytes_stored)),
            ] {
                if now > was {
                    p.cmd("HINCRBY").arg(&tk).arg(field).arg(now - was).ignore();
                    touched = true;
                }
            }
            if touched {
                p.cmd("EXPIRE").arg(&tk).arg(TIMELINE_TTL_SECS).ignore();
            }
            seen.insert(pt.ts.clone(), pt.clone());
        }

        p.cmd("SET")
            .arg(self.key(&format!("pod:{}", self.pod)))
            .arg(stats.started_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
            .arg("EX")
            .arg(POD_TTL_SECS)
            .ignore();

        let mut conn = self.conn().await?;
        p.query_async::<()>(&mut conn).await?;
        last.counters = cur;
        last.swept_objects = swept_objects;
        last.swept_bytes = swept_bytes;
        last.hist = hists.map(Some);
        last.timeline = seen;
        Ok(())
    }

    pub async fn push_events(&self, events: &[Arc<Event>]) -> Result<()> {
        if events.is_empty() {
            return Ok(());
        }
        let key = self.key("events");
        let mut p = redis::pipe();
        for ev in events {
            p.cmd("XADD")
                .arg(&key)
                .arg("MAXLEN")
                .arg("~")
                .arg(EVENT_RING)
                .arg("*")
                .arg("json")
                .arg(serde_json::to_string(&**ev)?)
                .ignore();
        }
        let mut conn = self.conn().await?;
        p.query_async::<()>(&mut conn).await?;
        Ok(())
    }

    /// Newest first, like `Stats::recent_events`.
    pub async fn recent_events(&self, limit: usize) -> Result<Vec<Arc<Event>>> {
        let mut conn = self.conn().await?;
        let reply: Value = redis::cmd("XREVRANGE")
            .arg(self.key("events"))
            .arg("+")
            .arg("-")
            .arg("COUNT")
            .arg(limit)
            .query_async(&mut conn)
            .await?;
        Ok(parse_entries(reply)?.into_iter().map(|(_, e)| e).collect())
    }

    /// Relay the cluster's event stream to this pod's SSE subscribers. Blocking reads need a
    /// connection of their own; the loop reconnects with a short pause whenever Redis drops.
    pub async fn relay_events(&self) {
        let key = self.key("events");
        let mut last_id = "$".to_owned();
        loop {
            let mut conn = match self.client.get_multiplexed_async_connection().await {
                Ok(c) => c,
                Err(e) => {
                    tracing::warn!(error = %e, "shared stats: event relay cannot reach redis");
                    tokio::time::sleep(Duration::from_secs(2)).await;
                    continue;
                }
            };
            loop {
                let reply: Result<Value, _> = redis::cmd("XREAD")
                    .arg("COUNT")
                    .arg(256)
                    .arg("BLOCK")
                    .arg(5000)
                    .arg("STREAMS")
                    .arg(&key)
                    .arg(&last_id)
                    .query_async(&mut conn)
                    .await;
                let reply = match reply {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!(error = %e, "shared stats: event relay lost redis");
                        tokio::time::sleep(Duration::from_secs(1)).await;
                        break;
                    }
                };
                for (_, entries) in pairs(reply) {
                    match parse_entries(entries) {
                        Ok(list) => {
                            for (id, ev) in list {
                                last_id = id;
                                let _ = self.live.send(ev);
                            }
                        }
                        Err(e) => tracing::warn!(error = %e, "shared stats: bad event in stream"),
                    }
                }
            }
        }
    }

    /// `true` when this pod gets to run the retention pass of this interval.
    pub async fn try_sweep_lock(&self, ttl: Duration) -> Result<bool> {
        let mut conn = self.conn().await?;
        let got: Option<String> = redis::cmd("SET")
            .arg(self.key("sweep:lock"))
            .arg(&self.pod)
            .arg("NX")
            .arg("EX")
            .arg(ttl.as_secs().max(1))
            .query_async(&mut conn)
            .await?;
        Ok(got.is_some())
    }

    pub async fn record_sweep(&self, report: &SweepReport) -> Result<()> {
        let mut conn = self.conn().await?;
        redis::cmd("SET")
            .arg(self.key("sweep:last"))
            .arg(serde_json::to_string(report)?)
            .query_async::<()>(&mut conn)
            .await?;
        Ok(())
    }

    pub async fn snapshot(&self, timeline_ts: &[String]) -> Result<ClusterSnapshot> {
        let mut conn = self.conn().await?;
        let mut p = redis::pipe();
        p.cmd("HGETALL").arg(self.key("counters"));
        for route in ROUTES {
            p.cmd("HGETALL").arg(self.key(&format!("hist:{route}")));
        }
        p.cmd("GET").arg(self.key("sweep:last"));
        for ts in timeline_ts {
            p.cmd("HGETALL").arg(self.key(&format!("tl:{ts}")));
        }
        let replies: Vec<Value> = p.query_async(&mut conn).await?;
        let mut it = replies.into_iter();
        let next = |it: &mut std::vec::IntoIter<Value>| it.next().ok_or_else(|| anyhow!("short pipeline reply"));

        let c = hash_u64(next(&mut it)?)?;
        let counters = CountersSnapshot {
            hits: get(&c, "hits"),
            misses: get(&c, "misses"),
            puts: get(&c, "puts"),
            conflicts: get(&c, "conflicts"),
            unauthorized: get(&c, "unauthorized"),
            forbidden: get(&c, "forbidden"),
            errors: get(&c, "errors"),
            bytes_served: get(&c, "bytes_served"),
            bytes_stored: get(&c, "bytes_stored"),
        };
        let mut hists = Vec::with_capacity(3);
        for _ in ROUTES {
            let h = hash_u64(next(&mut it)?)?;
            hists.push(HistogramSnapshot {
                buckets: (0..=LATENCY_BOUNDS_MS.len())
                    .map(|b| get(&h, &format!("b{b}")))
                    .collect(),
                count: get(&h, "count"),
                sum_micros: get(&h, "sum_micros"),
            });
        }
        let last_sweep = match next(&mut it)? {
            Value::Nil => None,
            v => {
                let raw: String = redis::from_redis_value(v)?;
                serde_json::from_str(&raw).ok()
            }
        };
        let mut timeline = Vec::with_capacity(timeline_ts.len());
        for ts in timeline_ts {
            let h = hash_u64(next(&mut it)?)?;
            timeline.push(TimelinePoint {
                ts: ts.clone(),
                hits: get(&h, "hits"),
                misses: get(&h, "misses"),
                puts: get(&h, "puts"),
                bytes_served: get(&h, "bytes_served"),
                bytes_stored: get(&h, "bytes_stored"),
            });
        }
        let put = hists.pop().unwrap();
        let get_miss = hists.pop().unwrap();
        let get_hit = hists.pop().unwrap();

        Ok(ClusterSnapshot {
            swept_objects: get(&c, "swept_objects"),
            swept_bytes: get(&c, "swept_bytes"),
            counters,
            get_hit,
            get_miss,
            put,
            timeline,
            last_sweep,
            pods: self.pods(&mut conn).await?,
        })
    }

    /// Pods seen in the last 15 s, by name.
    async fn pods(&self, conn: &mut ConnectionManager) -> Result<Vec<PodInfo>> {
        let pattern = self.key("pod:*");
        let strip = self.key("pod:");
        let mut keys: Vec<String> = Vec::new();
        let mut cursor: u64 = 0;
        loop {
            let (next, batch): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(&pattern)
                .arg("COUNT")
                .arg(200)
                .query_async(conn)
                .await?;
            keys.extend(batch);
            cursor = next;
            if cursor == 0 {
                break;
            }
        }
        keys.sort();
        keys.dedup();
        if keys.is_empty() {
            return Ok(Vec::new());
        }
        let started: Vec<Option<String>> = redis::cmd("MGET").arg(&keys).query_async(conn).await?;
        Ok(keys
            .into_iter()
            .zip(started)
            .filter_map(|(k, s)| {
                Some(PodInfo {
                    name: k.strip_prefix(&strip)?.to_owned(),
                    started_at: s?,
                })
            })
            .collect())
    }
}

fn get(h: &HashMap<String, u64>, field: &str) -> u64 {
    h.get(field).copied().unwrap_or(0)
}

fn hash_u64(v: Value) -> Result<HashMap<String, u64>> {
    redis::from_redis_value(v).context("HGETALL reply")
}

/// RESP2 sends maps as flat arrays, RESP3 as maps: either way, pairs.
fn pairs(v: Value) -> Vec<(Value, Value)> {
    match v {
        Value::Map(m) => m,
        Value::Array(items) => {
            let mut it = items.into_iter();
            let mut out = Vec::new();
            while let (Some(a), Some(b)) = (it.next(), it.next()) {
                out.push((a, b));
            }
            out
        }
        _ => Vec::new(),
    }
}

/// Stream entries `[id, [field, value, ...]]` to `(id, event)`, keeping the stream's order.
fn parse_entries(v: Value) -> Result<Vec<(String, Arc<Event>)>> {
    let Value::Array(entries) = v else {
        return Ok(Vec::new());
    };
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let Value::Array(mut parts) = entry else { continue };
        if parts.len() != 2 {
            continue;
        }
        let fields = parts.pop().unwrap();
        let id: String = redis::from_redis_value(parts.pop().unwrap())?;
        for (k, val) in pairs(fields) {
            let k: String = redis::from_redis_value(k)?;
            if k == "json" {
                let raw: String = redis::from_redis_value(val)?;
                let ev: Event = serde_json::from_str(&raw).context("event json")?;
                out.push((id.clone(), Arc::new(ev)));
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sweep_lock_is_taken_once_per_interval() {
        // Needs NX_CACHE_TEST_REDIS_URL (skipped otherwise).
        let Ok(url) = std::env::var("NX_CACHE_TEST_REDIS_URL") else {
            return;
        };
        let prefix = format!("nxrc-unit-test-stats:{}", std::process::id());
        let a = SharedStats::new(&url, None, &prefix, "pod-a", Duration::from_secs(1)).unwrap();
        let b = SharedStats::new(&url, None, &prefix, "pod-b", Duration::from_secs(1)).unwrap();
        assert!(a.try_sweep_lock(Duration::from_secs(5)).await.unwrap());
        assert!(!b.try_sweep_lock(Duration::from_secs(5)).await.unwrap());
        assert!(!a.try_sweep_lock(Duration::from_secs(5)).await.unwrap());
        let mut conn = a.conn().await.unwrap();
        redis::cmd("DEL")
            .arg(a.key("sweep:lock"))
            .query_async::<()>(&mut conn)
            .await
            .unwrap();
        assert!(b.try_sweep_lock(Duration::from_secs(5)).await.unwrap());
        redis::cmd("DEL")
            .arg(a.key("sweep:lock"))
            .query_async::<()>(&mut conn)
            .await
            .unwrap();
    }

    #[test]
    fn stream_entries_parse_in_both_resp_shapes() {
        let ev = Event {
            ts: "2026-10-29T18:00:00.000Z".into(),
            kind: crate::stats::Kind::Hit,
            hash: "abc".into(),
            bytes: 10,
            ms: 1.5,
            status: 200,
            ctx: None,
            origin: None,
        };
        let json = serde_json::to_string(&ev).unwrap();
        let entry = |id: &str, fields: Value| Value::Array(vec![Value::BulkString(id.as_bytes().to_vec()), fields]);
        let flat = Value::Array(vec![
            Value::BulkString(b"json".to_vec()),
            Value::BulkString(json.clone().into_bytes()),
        ]);
        let map = Value::Map(vec![(
            Value::BulkString(b"json".to_vec()),
            Value::BulkString(json.into_bytes()),
        )]);
        let got = parse_entries(Value::Array(vec![entry("1-0", flat), entry("2-0", map)])).unwrap();
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].0, "1-0");
        assert_eq!(got[1].1.hash, "abc");
        assert_eq!(got[1].1.kind, crate::stats::Kind::Hit);
    }
}
