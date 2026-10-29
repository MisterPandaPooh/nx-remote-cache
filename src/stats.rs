//! In-process counters, latency histograms, a 24 h timeline and a live event feed.
//! Everything is lock-free or behind a very short mutex: recording an event must cost
//! well under a microsecond so it never shows up in request latency.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering::Relaxed};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use chrono::{DateTime, SecondsFormat, Utc};
use serde::Serialize;
use tokio::sync::broadcast;

pub const TIMELINE_BUCKET_SECS: i64 = 300;
pub const TIMELINE_BUCKETS: usize = 288; // 24 h
pub const EVENT_RING: usize = 1000;

/// Histogram upper bounds in milliseconds (last bucket is +Inf).
pub const LATENCY_BOUNDS_MS: [f64; 16] = [
    0.25, 0.5, 1.0, 2.0, 5.0, 10.0, 25.0, 50.0, 100.0, 250.0, 500.0, 1000.0, 2500.0, 5000.0, 10000.0, 30000.0,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Hit,
    Miss,
    Put,
    Conflict,
    Forbidden,
    Unauthorized,
    Error,
}

impl Kind {
    pub fn as_str(self) -> &'static str {
        match self {
            Kind::Hit => "hit",
            Kind::Miss => "miss",
            Kind::Put => "put",
            Kind::Conflict => "conflict",
            Kind::Forbidden => "forbidden",
            Kind::Unauthorized => "unauthorized",
            Kind::Error => "error",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Event {
    pub ts: String,
    pub kind: Kind,
    pub hash: String,
    pub bytes: u64,
    pub ms: f64,
    pub status: u16,
    /// Context the requester sent with its token (e.g. the CI run reading the cache).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ctx: Option<crate::auth::Context>,
    /// Context recorded when the artifact was uploaded (the run that produced it).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub origin: Option<crate::auth::Context>,
}

#[derive(Default)]
pub struct Histogram {
    buckets: [AtomicU64; LATENCY_BOUNDS_MS.len() + 1],
    count: AtomicU64,
    sum_micros: AtomicU64,
}

impl Histogram {
    pub fn record(&self, d: Duration) {
        let ms = d.as_secs_f64() * 1000.0;
        let idx = LATENCY_BOUNDS_MS
            .iter()
            .position(|b| ms <= *b)
            .unwrap_or(LATENCY_BOUNDS_MS.len());
        self.buckets[idx].fetch_add(1, Relaxed);
        self.count.fetch_add(1, Relaxed);
        self.sum_micros.fetch_add(d.as_micros() as u64, Relaxed);
    }

    pub fn snapshot(&self) -> HistogramSnapshot {
        let buckets: Vec<u64> = self.buckets.iter().map(|b| b.load(Relaxed)).collect();
        let count = self.count.load(Relaxed);
        let sum_micros = self.sum_micros.load(Relaxed);
        HistogramSnapshot {
            buckets,
            count,
            sum_micros,
        }
    }
}

#[derive(Clone, Debug)]
pub struct HistogramSnapshot {
    pub buckets: Vec<u64>,
    pub count: u64,
    pub sum_micros: u64,
}

impl HistogramSnapshot {
    /// Estimate a percentile (0..1) by linear interpolation inside the matching bucket.
    pub fn percentile(&self, p: f64) -> f64 {
        if self.count == 0 {
            return 0.0;
        }
        let target = (p * self.count as f64).ceil().max(1.0) as u64;
        let mut seen = 0u64;
        let mut lower = 0.0;
        for (i, n) in self.buckets.iter().enumerate() {
            let upper = LATENCY_BOUNDS_MS
                .get(i)
                .copied()
                .unwrap_or(LATENCY_BOUNDS_MS[LATENCY_BOUNDS_MS.len() - 1] * 2.0);
            if seen + n >= target {
                if *n == 0 {
                    return upper;
                }
                let frac = (target - seen) as f64 / *n as f64;
                return lower + (upper - lower) * frac;
            }
            seen += n;
            lower = upper;
        }
        lower
    }

    pub fn summary(&self) -> LatencySummary {
        LatencySummary {
            p50: round(self.percentile(0.50)),
            p90: round(self.percentile(0.90)),
            p99: round(self.percentile(0.99)),
            count: self.count,
        }
    }
}

fn round(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[derive(Clone, Debug, Serialize)]
pub struct LatencySummary {
    pub p50: f64,
    pub p90: f64,
    pub p99: f64,
    pub count: u64,
}

#[derive(Default)]
pub struct Counters {
    pub hits: AtomicU64,
    pub misses: AtomicU64,
    pub puts: AtomicU64,
    pub conflicts: AtomicU64,
    pub unauthorized: AtomicU64,
    pub forbidden: AtomicU64,
    pub errors: AtomicU64,
    pub bytes_served: AtomicU64,
    pub bytes_stored: AtomicU64,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CountersSnapshot {
    pub hits: u64,
    pub misses: u64,
    pub puts: u64,
    pub conflicts: u64,
    pub unauthorized: u64,
    pub forbidden: u64,
    pub errors: u64,
    pub bytes_served: u64,
    pub bytes_stored: u64,
}

#[derive(Clone, Copy, Debug, Default)]
struct Bucket {
    start: i64,
    hits: u64,
    misses: u64,
    puts: u64,
    bytes_served: u64,
    bytes_stored: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct TimelinePoint {
    pub ts: String,
    pub hits: u64,
    pub misses: u64,
    pub puts: u64,
    pub bytes_served: u64,
    pub bytes_stored: u64,
}

pub struct Stats {
    pub started_at: DateTime<Utc>,
    last_sweep: Mutex<Option<crate::store::SweepReport>>,
    pub swept_objects: AtomicU64,
    pub swept_bytes: AtomicU64,
    started: Instant,
    pub counters: Counters,
    pub get_hit: Histogram,
    pub get_miss: Histogram,
    pub put: Histogram,
    timeline: Mutex<Vec<Bucket>>,
    events: Mutex<VecDeque<Arc<Event>>>,
    live: broadcast::Sender<Arc<Event>>,
}

impl Default for Stats {
    fn default() -> Self {
        Self::new()
    }
}

impl Stats {
    pub fn new() -> Self {
        let (live, _) = broadcast::channel(512);
        Self {
            started_at: Utc::now(),
            last_sweep: Mutex::new(None),
            swept_objects: AtomicU64::new(0),
            swept_bytes: AtomicU64::new(0),
            started: Instant::now(),
            counters: Counters::default(),
            get_hit: Histogram::default(),
            get_miss: Histogram::default(),
            put: Histogram::default(),
            timeline: Mutex::new(vec![Bucket::default(); TIMELINE_BUCKETS]),
            events: Mutex::new(VecDeque::with_capacity(EVENT_RING)),
            live,
        }
    }

    pub fn uptime(&self) -> Duration {
        self.started.elapsed()
    }

    pub fn record_sweep(&self, report: crate::store::SweepReport) {
        self.swept_objects.fetch_add(report.deleted, Relaxed);
        self.swept_bytes.fetch_add(report.bytes_freed, Relaxed);
        *self.last_sweep.lock().unwrap_or_else(|p| p.into_inner()) = Some(report);
    }

    pub fn last_sweep(&self) -> Option<crate::store::SweepReport> {
        self.last_sweep.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<Event>> {
        self.live.subscribe()
    }

    pub fn record(&self, kind: Kind, hash: &str, bytes: u64, elapsed: Duration, status: u16) {
        self.record_full(kind, hash, bytes, elapsed, status, None, None);
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_full(
        &self,
        kind: Kind,
        hash: &str,
        bytes: u64,
        elapsed: Duration,
        status: u16,
        ctx: Option<crate::auth::Context>,
        origin: Option<crate::auth::Context>,
    ) {
        let c = &self.counters;
        match kind {
            Kind::Hit => {
                c.hits.fetch_add(1, Relaxed);
                c.bytes_served.fetch_add(bytes, Relaxed);
                self.get_hit.record(elapsed);
            }
            Kind::Miss => {
                c.misses.fetch_add(1, Relaxed);
                self.get_miss.record(elapsed);
            }
            Kind::Put => {
                c.puts.fetch_add(1, Relaxed);
                c.bytes_stored.fetch_add(bytes, Relaxed);
                self.put.record(elapsed);
            }
            Kind::Conflict => {
                c.conflicts.fetch_add(1, Relaxed);
                self.put.record(elapsed);
            }
            Kind::Forbidden => {
                c.forbidden.fetch_add(1, Relaxed);
            }
            Kind::Unauthorized => {
                c.unauthorized.fetch_add(1, Relaxed);
            }
            Kind::Error => {
                c.errors.fetch_add(1, Relaxed);
            }
        }

        let now = Utc::now();
        self.bump_timeline(now.timestamp(), kind, bytes);

        let event = Arc::new(Event {
            ts: now.to_rfc3339_opts(SecondsFormat::Millis, true),
            kind,
            hash: hash.to_owned(),
            bytes,
            ms: round(elapsed.as_secs_f64() * 1000.0),
            status,
            ctx: ctx.filter(|c| !c.is_empty()),
            origin: origin.filter(|c| !c.is_empty()),
        });
        {
            let mut ring = self.events.lock().unwrap_or_else(|p| p.into_inner());
            if ring.len() == EVENT_RING {
                ring.pop_front();
            }
            ring.push_back(event.clone());
        }
        let _ = self.live.send(event);
    }

    fn bump_timeline(&self, now_secs: i64, kind: Kind, bytes: u64) {
        let start = now_secs - now_secs.rem_euclid(TIMELINE_BUCKET_SECS);
        let slot = ((start / TIMELINE_BUCKET_SECS).rem_euclid(TIMELINE_BUCKETS as i64)) as usize;
        let mut tl = self.timeline.lock().unwrap_or_else(|p| p.into_inner());
        let b = &mut tl[slot];
        if b.start != start {
            *b = Bucket {
                start,
                ..Default::default()
            };
        }
        match kind {
            Kind::Hit => {
                b.hits += 1;
                b.bytes_served += bytes;
            }
            Kind::Miss => b.misses += 1,
            Kind::Put => {
                b.puts += 1;
                b.bytes_stored += bytes;
            }
            _ => {}
        }
    }

    pub fn counters(&self) -> CountersSnapshot {
        let c = &self.counters;
        CountersSnapshot {
            hits: c.hits.load(Relaxed),
            misses: c.misses.load(Relaxed),
            puts: c.puts.load(Relaxed),
            conflicts: c.conflicts.load(Relaxed),
            unauthorized: c.unauthorized.load(Relaxed),
            forbidden: c.forbidden.load(Relaxed),
            errors: c.errors.load(Relaxed),
            bytes_served: c.bytes_served.load(Relaxed),
            bytes_stored: c.bytes_stored.load(Relaxed),
        }
    }

    /// The last 24 h as zero-filled 5-minute points, oldest first.
    pub fn timeline(&self) -> Vec<TimelinePoint> {
        let now = Utc::now().timestamp();
        let newest = now - now.rem_euclid(TIMELINE_BUCKET_SECS);
        let tl = self.timeline.lock().unwrap_or_else(|p| p.into_inner());
        (0..TIMELINE_BUCKETS)
            .map(|i| {
                let start = newest - TIMELINE_BUCKET_SECS * (TIMELINE_BUCKETS as i64 - 1 - i as i64);
                let slot = ((start / TIMELINE_BUCKET_SECS).rem_euclid(TIMELINE_BUCKETS as i64)) as usize;
                let b = tl[slot];
                let b = if b.start == start {
                    b
                } else {
                    Bucket {
                        start,
                        ..Default::default()
                    }
                };
                TimelinePoint {
                    ts: DateTime::<Utc>::from_timestamp(start, 0)
                        .unwrap_or_default()
                        .to_rfc3339_opts(SecondsFormat::Secs, true),
                    hits: b.hits,
                    misses: b.misses,
                    puts: b.puts,
                    bytes_served: b.bytes_served,
                    bytes_stored: b.bytes_stored,
                }
            })
            .collect()
    }

    /// Newest first.
    pub fn recent_events(&self, limit: usize) -> Vec<Arc<Event>> {
        let ring = self.events.lock().unwrap_or_else(|p| p.into_inner());
        ring.iter().rev().take(limit).cloned().collect()
    }

    /// Prometheus text exposition.
    pub fn prometheus(
        &self,
        version: &str,
        store_kind: &str,
        mem: &crate::memcache::MemCacheStats,
        kv: crate::kv::KvStats,
    ) -> String {
        use std::fmt::Write;
        let c = self.counters();
        let mut out = String::with_capacity(4096);
        let _ = writeln!(out, "# HELP nx_cache_build_info Build information.\n# TYPE nx_cache_build_info gauge\nnx_cache_build_info{{version=\"{version}\",store=\"{store_kind}\"}} 1");
        let _ = writeln!(out, "# HELP nx_cache_uptime_seconds Seconds since the server started.\n# TYPE nx_cache_uptime_seconds gauge\nnx_cache_uptime_seconds {}", self.uptime().as_secs());
        let _ = writeln!(
            out,
            "# HELP nx_cache_requests_total Cache requests by outcome.\n# TYPE nx_cache_requests_total counter"
        );
        for (k, v) in [
            ("hit", c.hits),
            ("miss", c.misses),
            ("put", c.puts),
            ("conflict", c.conflicts),
            ("unauthorized", c.unauthorized),
            ("forbidden", c.forbidden),
            ("error", c.errors),
        ] {
            let _ = writeln!(out, "nx_cache_requests_total{{kind=\"{k}\"}} {v}");
        }
        let _ = writeln!(out, "# HELP nx_cache_bytes_served_total Artifact bytes sent to clients.\n# TYPE nx_cache_bytes_served_total counter\nnx_cache_bytes_served_total {}", c.bytes_served);
        let _ = writeln!(out, "# HELP nx_cache_bytes_stored_total Artifact bytes written to the store.\n# TYPE nx_cache_bytes_stored_total counter\nnx_cache_bytes_stored_total {}", c.bytes_stored);
        let _ = writeln!(out, "# HELP nx_cache_memory_tier_hits_total Hits served from the in-memory tier.\n# TYPE nx_cache_memory_tier_hits_total counter\nnx_cache_memory_tier_hits_total {}", mem.hits);
        let _ = writeln!(out, "# HELP nx_cache_memory_tier_bytes Bytes currently held in the in-memory tier.\n# TYPE nx_cache_memory_tier_bytes gauge\nnx_cache_memory_tier_bytes {}", mem.bytes);
        let _ = writeln!(out, "# HELP nx_cache_memory_tier_items Artifacts currently held in the in-memory tier.\n# TYPE nx_cache_memory_tier_items gauge\nnx_cache_memory_tier_items {}", mem.items);
        let _ = writeln!(out, "# HELP nx_cache_kv_items Keys currently held in the key-value store.\n# TYPE nx_cache_kv_items gauge\nnx_cache_kv_items {}", kv.items);
        let _ = writeln!(out, "# HELP nx_cache_kv_bytes Bytes currently held in the key-value store.\n# TYPE nx_cache_kv_bytes gauge\nnx_cache_kv_bytes {}", kv.bytes);
        let _ = writeln!(out, "# HELP nx_cache_retention_deleted_total Artifacts removed by the retention sweeper.\n# TYPE nx_cache_retention_deleted_total counter\nnx_cache_retention_deleted_total {}", self.swept_objects.load(Relaxed));
        let _ = writeln!(out, "# HELP nx_cache_retention_freed_bytes_total Bytes removed by the retention sweeper.\n# TYPE nx_cache_retention_freed_bytes_total counter\nnx_cache_retention_freed_bytes_total {}", self.swept_bytes.load(Relaxed));
        if let Some(sw) = self.last_sweep() {
            let _ = writeln!(out, "# HELP nx_cache_store_bytes Bytes held under the cache prefix at the last sweep.\n# TYPE nx_cache_store_bytes gauge\nnx_cache_store_bytes {}", sw.total_bytes);
            let _ = writeln!(out, "# HELP nx_cache_store_objects Artifacts under the cache prefix at the last sweep.\n# TYPE nx_cache_store_objects gauge\nnx_cache_store_objects {}", sw.scanned - sw.deleted);
        }
        let _ = writeln!(out, "# HELP nx_cache_request_duration_seconds Server-side request latency.\n# TYPE nx_cache_request_duration_seconds histogram");
        for (route, h) in [
            ("get_hit", &self.get_hit),
            ("get_miss", &self.get_miss),
            ("put", &self.put),
        ] {
            let s = h.snapshot();
            let mut cumulative = 0u64;
            for (i, n) in s.buckets.iter().enumerate() {
                cumulative += n;
                let le = match LATENCY_BOUNDS_MS.get(i) {
                    Some(ms) => format!("{}", ms / 1000.0),
                    None => "+Inf".to_string(),
                };
                let _ = writeln!(
                    out,
                    "nx_cache_request_duration_seconds_bucket{{route=\"{route}\",le=\"{le}\"}} {cumulative}"
                );
            }
            let _ = writeln!(
                out,
                "nx_cache_request_duration_seconds_sum{{route=\"{route}\"}} {}",
                s.sum_micros as f64 / 1_000_000.0
            );
            let _ = writeln!(
                out,
                "nx_cache_request_duration_seconds_count{{route=\"{route}\"}} {}",
                s.count
            );
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles_interpolate() {
        let h = Histogram::default();
        for _ in 0..90 {
            h.record(Duration::from_micros(800)); // bucket (0.5, 1]
        }
        for _ in 0..10 {
            h.record(Duration::from_millis(40)); // bucket (25, 50]
        }
        let s = h.snapshot().summary();
        assert!(s.p50 > 0.5 && s.p50 <= 1.0, "p50={}", s.p50);
        assert!(s.p99 > 25.0 && s.p99 <= 50.0, "p99={}", s.p99);
        assert_eq!(s.count, 100);
        assert_eq!(
            HistogramSnapshot {
                buckets: vec![0; 17],
                count: 0,
                sum_micros: 0
            }
            .percentile(0.5),
            0.0
        );
    }

    #[test]
    fn counters_timeline_and_events() {
        let s = Stats::new();
        s.record(Kind::Hit, "a", 100, Duration::from_millis(2), 200);
        s.record(Kind::Miss, "b", 0, Duration::from_micros(300), 404);
        s.record(Kind::Put, "c", 50, Duration::from_millis(20), 200);
        s.record(Kind::Conflict, "c", 0, Duration::from_millis(1), 409);
        let c = s.counters();
        assert_eq!((c.hits, c.misses, c.puts, c.conflicts), (1, 1, 1, 1));
        assert_eq!((c.bytes_served, c.bytes_stored), (100, 50));
        let tl = s.timeline();
        assert_eq!(tl.len(), TIMELINE_BUCKETS);
        let last = tl.last().unwrap();
        assert_eq!((last.hits, last.misses, last.puts), (1, 1, 1));
        assert_eq!(tl[0].hits, 0);
        let ev = s.recent_events(10);
        assert_eq!(ev.len(), 4);
        assert_eq!(ev[0].kind, Kind::Conflict);
        assert_eq!(ev[3].kind, Kind::Hit);
        let prom = s.prometheus(
            "t",
            "memory",
            &crate::memcache::MemCache::new(0, 0).stats(),
            Default::default(),
        );
        assert!(prom.contains("nx_cache_requests_total{kind=\"hit\"} 1"));
        assert!(prom.contains("nx_cache_request_duration_seconds_bucket{route=\"put\",le=\"+Inf\"} 2"));
    }

    #[test]
    fn event_ring_is_bounded() {
        let s = Stats::new();
        for i in 0..(EVENT_RING + 5) {
            s.record(Kind::Miss, &i.to_string(), 0, Duration::ZERO, 404);
        }
        assert_eq!(s.recent_events(usize::MAX).len(), EVENT_RING);
    }
}
