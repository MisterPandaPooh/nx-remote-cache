//! A small key-value store on the side of the cache, for CI glue that has nothing to do with
//! Nx: feature flags, "last green sha", a JSON blob shared between jobs. Values keep the content
//! type they were stored with, JSON is validated, and every key may carry a TTL.
//!
//! Keys live in a scope chosen per request: `global` (default), `pipeline` (one namespace per
//! repository) or `run` (one namespace per CI run). The pipeline and run identifiers come from
//! the token context the client already sends (`secret;run=123;repo=org/app`) or from the
//! `pipeline=` / `run=` query parameters. A key stored with `secret=true` is masked in the
//! dashboard until someone clicks reveal.
//!
//! Two backends:
//! - `memory`: a byte-bounded LRU in this process. Fast, gone on restart, and with several
//!   replicas each pod has its own view.
//! - `redis`: every key is a Redis string `<prefix>|<level>|<scope>|<key>` with Redis' own
//!   expiry, so every replica shares the same keys. One round trip per operation.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use bytes::Bytes;
use serde::{Deserialize, Serialize};

use crate::app::AppState;
use crate::auth::{Access, Context};
use crate::config::parse_duration;
use crate::routes::{authenticate, text};

const MAX_KEY_LEN: usize = 200;
const MAX_SCOPE_LEN: usize = 200;
const DEFAULT_LIST_LIMIT: usize = 100;
const MAX_LIST_LIMIT: usize = 1000;
/// Separator inside the in-memory map key; never valid in a key or scope, so composed keys
/// cannot collide.
const SEP: char = '\u{1f}';
/// Separator inside Redis keys: readable in `redis-cli`, and `|` is not allowed in keys or scopes.
const REDIS_SEP: char = '|';
const SCAN_PAGE: usize = 500;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Level {
    Global,
    Pipeline,
    Run,
}

impl Level {
    pub fn parse(s: &str) -> Option<Level> {
        match s.trim().to_ascii_lowercase().as_str() {
            "" | "global" => Some(Level::Global),
            "pipeline" | "repo" | "repository" => Some(Level::Pipeline),
            "run" => Some(Level::Run),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Level::Global => "global",
            Level::Pipeline => "pipeline",
            Level::Run => "run",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Backend {
    Memory,
    Redis,
}

impl Backend {
    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Memory => "memory",
            Backend::Redis => "redis",
        }
    }
}

#[derive(Clone)]
pub struct Entry {
    pub value: Bytes,
    pub content_type: String,
    pub expires: Option<Instant>,
    pub secret: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct KvStats {
    pub items: u64,
    pub bytes: u64,
}

#[derive(Debug)]
pub enum KvError {
    Disabled,
    TooLarge,
    Redis(redis::RedisError),
}

impl PartialEq for KvError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (KvError::Disabled, KvError::Disabled) | (KvError::TooLarge, KvError::TooLarge)
        )
    }
}

impl From<redis::RedisError> for KvError {
    fn from(e: redis::RedisError) -> Self {
        KvError::Redis(e)
    }
}

pub struct KvStore {
    budget: u64,
    max_value: u64,
    inner: Mutex<Inner>,
    redis: Option<RedisBackend>,
    /// `redis` backend: counts from the last listing (Redis is not asked on every scrape).
    last_seen: Mutex<KvStats>,
}

struct RedisBackend {
    client: redis::Client,
    /// Built on first use; reconnects by itself afterwards.
    conn: tokio::sync::OnceCell<redis::aio::ConnectionManager>,
    prefix: String,
}

struct Stored {
    entry: Entry,
    level: Level,
    scope: String,
    key: String,
    generation: u64,
}

#[derive(Default)]
struct Inner {
    map: HashMap<String, Stored>,   // composed key -> value and the generation of its last use
    order: VecDeque<(String, u64)>, // (composed key, generation) oldest first; stale pairs are skipped
    bytes: u64,
    generation: u64,
}

impl Inner {
    /// Mark `full` as just used: a fresh generation at the back of the queue.
    fn touch(&mut self, full: &str) -> u64 {
        self.generation += 1;
        self.order.push_back((full.to_owned(), self.generation));
        self.generation
    }

    /// Drop stale queue pairs once the queue dwarfs the map (heavy re-reads under budget would
    /// otherwise grow it without bound). Call after the map holds the touched key.
    fn compact(&mut self) {
        if self.order.len() > self.map.len() * 4 + 64 {
            let Inner { map, order, .. } = self;
            order.retain(|(k, g)| map.get(k).is_some_and(|s| s.generation == *g));
        }
    }
}

fn compose(level: Level, scope: &str, key: &str) -> String {
    format!("{}{SEP}{scope}{SEP}{key}", level.as_str())
}

/// What a Redis value looks like: one JSON header line, then the raw bytes. Expiry is Redis' own.
#[derive(Serialize, Deserialize)]
struct Envelope {
    ct: String,
    secret: bool,
}

impl KvStore {
    /// In-process LRU. `budget == 0` disables the store (its routes answer 404).
    pub fn new(budget: u64, max_value: u64) -> Self {
        Self {
            budget,
            max_value,
            inner: Mutex::new(Inner::default()),
            redis: None,
            last_seen: Mutex::new(KvStats::default()),
        }
    }

    /// Keys in Redis, shared by every replica. `budget` only keeps the enabled/disabled
    /// semantics (`0` disables); Redis' own `maxmemory` bounds the data.
    pub fn with_redis(
        budget: u64,
        max_value: u64,
        url: &str,
        password: Option<&str>,
        prefix: &str,
    ) -> Result<Self, redis::RedisError> {
        let mut info: redis::ConnectionInfo = redis::IntoConnectionInfo::into_connection_info(url)?;
        if let Some(p) = password {
            let settings = info.redis_settings().clone().set_password(p);
            info = info.set_redis_settings(settings);
        }
        let client = redis::Client::open(info)?;
        Ok(Self {
            budget,
            max_value,
            inner: Mutex::new(Inner::default()),
            redis: Some(RedisBackend {
                client,
                conn: tokio::sync::OnceCell::new(),
                prefix: prefix.trim_matches(REDIS_SEP).to_owned(),
            }),
            last_seen: Mutex::new(KvStats::default()),
        })
    }

    pub fn backend(&self) -> Backend {
        if self.redis.is_some() {
            Backend::Redis
        } else {
            Backend::Memory
        }
    }

    pub fn enabled(&self) -> bool {
        self.budget > 0
    }

    pub fn budget(&self) -> u64 {
        self.budget
    }

    pub fn max_value(&self) -> u64 {
        self.max_value
    }

    /// Readiness: `PING` on the redis backend, nothing to check in memory.
    pub async fn probe(&self) -> Result<(), KvError> {
        if let Some(r) = &self.redis {
            let mut con = r.conn().await?;
            redis::cmd("PING").query_async::<String>(&mut con).await?;
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn set(
        &self,
        level: Level,
        scope: &str,
        key: &str,
        value: Bytes,
        content_type: String,
        ttl: Option<Duration>,
        secret: bool,
    ) -> Result<(), KvError> {
        if !self.enabled() {
            return Err(KvError::Disabled);
        }
        let len = value.len() as u64;
        if len > self.max_value || (self.redis.is_none() && len > self.budget) {
            return Err(KvError::TooLarge);
        }
        if let Some(r) = &self.redis {
            let mut body = serde_json::to_vec(&Envelope {
                ct: content_type,
                secret,
            })
            .expect("envelope serialises");
            body.push(b'\n');
            body.extend_from_slice(&value);
            let mut con = r.conn().await?;
            let mut cmd = redis::cmd("SET");
            cmd.arg(r.key(level, scope, key)).arg(body);
            if let Some(t) = ttl {
                cmd.arg("EX").arg(t.as_secs().max(1));
            }
            cmd.query_async::<()>(&mut con).await?;
            return Ok(());
        }
        let now = Instant::now();
        let full = compose(level, scope, key);
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        remove(&mut inner, &full);
        inner.bytes += len;
        let generation = inner.touch(&full);
        let entry = Entry {
            value,
            content_type,
            expires: ttl.map(|t| now + t),
            secret,
        };
        inner.map.insert(
            full,
            Stored {
                entry,
                level,
                scope: scope.to_owned(),
                key: key.to_owned(),
                generation,
            },
        );
        while inner.bytes > self.budget {
            evict_one(&mut inner);
        }
        inner.compact();
        Ok(())
    }

    pub async fn get(&self, level: Level, scope: &str, key: &str) -> Result<Option<Entry>, KvError> {
        if let Some(r) = &self.redis {
            let mut con = r.conn().await?;
            let k = r.key(level, scope, key);
            let (raw, ttl): (Option<Vec<u8>>, i64) = redis::pipe()
                .cmd("GET")
                .arg(&k)
                .cmd("TTL")
                .arg(&k)
                .query_async(&mut con)
                .await?;
            return Ok(raw.and_then(|bytes| decode(Bytes::from(bytes), ttl)));
        }
        let now = Instant::now();
        let full = compose(level, scope, key);
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let expired = inner
            .map
            .get(&full)
            .is_some_and(|s| s.entry.expires.is_some_and(|x| x <= now));
        if expired {
            remove(&mut inner, &full);
            return Ok(None);
        }
        if !inner.map.contains_key(&full) {
            return Ok(None);
        }
        let generation = inner.touch(&full);
        let stored = inner.map.get_mut(&full).expect("checked above");
        stored.generation = generation;
        let entry = stored.entry.clone();
        inner.compact();
        Ok(Some(entry))
    }

    pub async fn delete(&self, level: Level, scope: &str, key: &str) -> Result<bool, KvError> {
        if let Some(r) = &self.redis {
            let mut con = r.conn().await?;
            let n: i64 = redis::cmd("DEL")
                .arg(r.key(level, scope, key))
                .query_async(&mut con)
                .await?;
            return Ok(n > 0);
        }
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        Ok(remove(&mut inner, &compose(level, scope, key)))
    }

    /// Live keys of one scope starting with `prefix`, sorted, at most `limit` of them.
    pub async fn list(&self, level: Level, scope: &str, prefix: &str, limit: usize) -> Result<Vec<KeyInfo>, KvError> {
        if let Some(r) = &self.redis {
            let pattern = format!("{}{prefix}*", r.scope_prefix(level, scope));
            return self.list_redis(r, &pattern, limit).await;
        }
        Ok(self.collect(limit, |s| {
            s.level == level && s.scope == scope && s.key.starts_with(prefix)
        }))
    }

    /// Every live key of every scope, for the dashboard.
    pub async fn list_all(&self, limit: usize) -> Result<Vec<KeyInfo>, KvError> {
        if let Some(r) = &self.redis {
            let pattern = format!("{}{REDIS_SEP}*", r.prefix);
            let keys = self.list_redis(r, &pattern, limit).await?;
            let stats = KvStats {
                items: keys.len() as u64,
                bytes: keys.iter().map(|k| k.size).sum(),
            };
            *self.last_seen.lock().unwrap_or_else(|p| p.into_inner()) = stats;
            return Ok(keys);
        }
        Ok(self.collect(limit, |_| true))
    }

    /// `SCAN` the matching keys (sorted, at most `limit`), then one pipeline of `GET` + `TTL`.
    async fn list_redis(&self, r: &RedisBackend, pattern: &str, limit: usize) -> Result<Vec<KeyInfo>, KvError> {
        let mut con = r.conn().await?;
        let mut names: Vec<String> = Vec::new();
        let mut cursor: u64 = 0;
        loop {
            let (next, page): (u64, Vec<String>) = redis::cmd("SCAN")
                .arg(cursor)
                .arg("MATCH")
                .arg(pattern)
                .arg("COUNT")
                .arg(SCAN_PAGE)
                .query_async(&mut con)
                .await?;
            names.extend(page);
            cursor = next;
            if cursor == 0 || names.len() >= limit * 4 {
                break;
            }
        }
        let mut refs: Vec<(KeyRef, String)> = names.into_iter().filter_map(|n| r.parse(&n).map(|k| (k, n))).collect();
        refs.sort_by(|a, b| {
            (a.0.level.as_str(), &a.0.scope, &a.0.key).cmp(&(b.0.level.as_str(), &b.0.scope, &b.0.key))
        });
        refs.truncate(limit);
        if refs.is_empty() {
            return Ok(Vec::new());
        }
        let mut pipe = redis::pipe();
        for (_, name) in &refs {
            pipe.cmd("GET").arg(name).cmd("TTL").arg(name);
        }
        let replies: Vec<redis::Value> = pipe.query_async(&mut con).await?;
        let mut keys = Vec::with_capacity(refs.len());
        for (i, (kref, _)) in refs.into_iter().enumerate() {
            let raw: Option<Vec<u8>> = replies
                .get(2 * i)
                .and_then(|v| redis::from_redis_value(v.clone()).ok())
                .flatten();
            let ttl: i64 = replies
                .get(2 * i + 1)
                .and_then(|v| redis::from_redis_value(v.clone()).ok())
                .unwrap_or(-1);
            let Some(raw) = raw else { continue }; // expired between SCAN and GET
            let Some((header, len)) = split(&raw) else { continue };
            keys.push(KeyInfo {
                level: kref.level,
                scope: kref.scope,
                key: kref.key,
                size: len as u64,
                content_type: header.ct,
                secret: header.secret,
                expires_in_secs: (ttl >= 0).then_some(ttl as u64),
            });
        }
        Ok(keys)
    }

    fn collect(&self, limit: usize, keep: impl Fn(&Stored) -> bool) -> Vec<KeyInfo> {
        let now = Instant::now();
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let mut keys: Vec<KeyInfo> = inner
            .map
            .values()
            .filter(|s| s.entry.expires.is_none_or(|x| x > now) && keep(s))
            .map(|s| KeyInfo {
                level: s.level,
                scope: s.scope.clone(),
                key: s.key.clone(),
                size: s.entry.value.len() as u64,
                content_type: s.entry.content_type.clone(),
                secret: s.entry.secret,
                expires_in_secs: s.entry.expires.map(|x| x.saturating_duration_since(now).as_secs()),
            })
            .collect();
        keys.sort_by(|a, b| (a.level.as_str(), &a.scope, &a.key).cmp(&(b.level.as_str(), &b.scope, &b.key)));
        keys.truncate(limit);
        keys
    }

    pub fn stats(&self) -> KvStats {
        if self.redis.is_some() {
            return *self.last_seen.lock().unwrap_or_else(|p| p.into_inner());
        }
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        KvStats {
            items: inner.map.len() as u64,
            bytes: inner.bytes,
        }
    }
}

/// Parsed Redis key: `level|scope|key`.
#[derive(Clone)]
struct KeyRef {
    level: Level,
    scope: String,
    key: String,
}

impl RedisBackend {
    async fn conn(&self) -> Result<redis::aio::ConnectionManager, redis::RedisError> {
        let c = self
            .conn
            .get_or_try_init(|| redis::aio::ConnectionManager::new(self.client.clone()))
            .await?;
        Ok(c.clone())
    }

    fn key(&self, level: Level, scope: &str, key: &str) -> String {
        format!("{}{key}", self.scope_prefix(level, scope))
    }

    fn scope_prefix(&self, level: Level, scope: &str) -> String {
        format!(
            "{}{REDIS_SEP}{}{REDIS_SEP}{scope}{REDIS_SEP}",
            self.prefix,
            level.as_str()
        )
    }

    fn parse(&self, name: &str) -> Option<KeyRef> {
        let rest = name.strip_prefix(self.prefix.as_str())?.strip_prefix(REDIS_SEP)?;
        let mut parts = rest.splitn(3, REDIS_SEP);
        let level = Level::parse(parts.next()?)?;
        let scope = parts.next()?.to_owned();
        let key = parts.next()?.to_owned();
        if !valid_key(&key) || (level != Level::Global && !valid_scope(&scope)) {
            return None;
        }
        Some(KeyRef { level, scope, key })
    }
}

/// Header and value length of a stored envelope; `None` when the value is not one of ours.
fn split(bytes: &[u8]) -> Option<(Envelope, usize)> {
    let nl = bytes.iter().position(|b| *b == b'\n')?;
    let header: Envelope = serde_json::from_slice(&bytes[..nl]).ok()?;
    Some((header, bytes.len() - nl - 1))
}

/// Entry from a stored envelope and the Redis TTL (`-1` none, `-2` gone).
fn decode(bytes: Bytes, ttl: i64) -> Option<Entry> {
    let (header, _) = split(&bytes)?;
    let nl = bytes.iter().position(|b| *b == b'\n')?;
    Some(Entry {
        value: bytes.slice(nl + 1..),
        content_type: header.ct,
        expires: (ttl >= 0).then(|| Instant::now() + Duration::from_secs(ttl as u64)),
        secret: header.secret,
    })
}

fn remove(inner: &mut Inner, full: &str) -> bool {
    match inner.map.remove(full) {
        Some(s) => {
            inner.bytes -= s.entry.value.len() as u64;
            true
        }
        None => false,
    }
}

/// Drop the least recently used live key. Queue entries whose generation no longer matches
/// the map are leftovers of a later touch and are skipped.
fn evict_one(inner: &mut Inner) {
    while let Some((full, generation)) = inner.order.pop_front() {
        if inner.map.get(&full).is_some_and(|s| s.generation == generation) {
            remove(inner, &full);
            return;
        }
    }
}

pub fn valid_key(key: &str) -> bool {
    (1..=MAX_KEY_LEN).contains(&key.len())
        && key
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':' | b'@'))
}

/// Pipeline and run identifiers: repository slugs (`org/app`) and run ids, nothing exotic.
pub fn valid_scope(scope: &str) -> bool {
    (1..=MAX_SCOPE_LEN).contains(&scope.len())
        && scope
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b':' | b'@' | b'/'))
        && !scope.starts_with('/')
        && !scope.ends_with('/')
        && !scope.contains("//")
}

#[derive(Serialize, Clone)]
pub struct KeyInfo {
    pub level: Level,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub scope: String,
    pub key: String,
    pub size: u64,
    pub content_type: String,
    pub secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_secs: Option<u64>,
}

/// Query parameters shared by every `/v1/kv` route: which scope, plus the write-only options.
#[derive(Deserialize, Default)]
pub struct KvQuery {
    /// `global` (default), `pipeline` or `run`.
    pub level: Option<String>,
    /// Pipeline identifier; defaults to `repo` from the token context.
    pub pipeline: Option<String>,
    /// Run identifier; defaults to `run` from the token context.
    pub run: Option<String>,
    /// `30s`, `10m`, `2h`, `7d` or a number of seconds; absent means no expiry (PUT only).
    pub ttl: Option<String>,
    /// `true` masks the value in the dashboard (PUT only).
    pub secret: Option<String>,
    pub prefix: Option<String>,
    pub limit: Option<usize>,
}

/// Which scope a request addresses, from its query and the token context.
fn resolve_scope(q: &KvQuery, ctx: &Context) -> Result<(Level, String), String> {
    let bad = |msg: &str| Err(format!("nx-remote-cache: {msg}"));
    let Some(level) = Level::parse(q.level.as_deref().unwrap_or("")) else {
        return bad("invalid level (global, pipeline or run)");
    };
    let from_ctx = |keys: &[&str]| keys.iter().find_map(|k| ctx.get(*k).cloned());
    let scope = match level {
        Level::Global => String::new(),
        Level::Pipeline => match q.pipeline.clone().or_else(|| from_ctx(&["pipeline", "repo"])) {
            Some(s) => s,
            None => return bad("level=pipeline needs ?pipeline=<name> or repo=<org/app> in the token context"),
        },
        Level::Run => match q.run.clone().or_else(|| from_ctx(&["run"])) {
            Some(s) => s,
            None => return bad("level=run needs ?run=<id> or run=<id> in the token context"),
        },
    };
    if level != Level::Global && !valid_scope(&scope) {
        return bad("invalid pipeline or run identifier ([A-Za-z0-9_./:@-]{1,200})");
    }
    Ok((level, scope))
}

fn truthy(v: Option<&str>) -> bool {
    matches!(
        v.map(|s| s.trim().to_ascii_lowercase()).as_deref(),
        Some("1" | "true" | "yes" | "on")
    )
}

fn disabled() -> Response {
    text(
        StatusCode::NOT_FOUND,
        "nx-remote-cache: key-value store disabled (NX_CACHE_KV_ENABLED=false or NX_CACHE_KV_BYTES=0)",
    )
}

fn redis_failed(e: &redis::RedisError) -> Response {
    tracing::warn!(error = %e, "key-value store: redis operation failed");
    text(
        StatusCode::BAD_GATEWAY,
        "nx-remote-cache: key-value store (redis) unavailable",
    )
}

fn value_response(entry: Entry) -> Response {
    let mut res = Response::new(Body::from(entry.value));
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&entry.content_type).unwrap_or(HeaderValue::from_static("application/octet-stream")),
    );
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Some(x) = entry.expires {
        let left = x.saturating_duration_since(Instant::now()).as_secs();
        h.insert("x-kv-expires-in", HeaderValue::from(left));
    }
    if entry.secret {
        h.insert("x-kv-secret", HeaderValue::from_static("true"));
    }
    res
}

pub async fn put_kv(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<KvQuery>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let start = Instant::now();
    if !state.kv.enabled() {
        return disabled();
    }
    if !valid_key(&key) {
        return text(
            StatusCode::BAD_REQUEST,
            "nx-remote-cache: invalid key ([A-Za-z0-9_.:@-]{1,200})",
        );
    }
    let auth = match authenticate(&state, &headers, &key, start) {
        Ok(a) => a,
        Err(res) => return res,
    };
    if auth.access != Access::ReadWrite {
        return text(
            StatusCode::FORBIDDEN,
            "nx-remote-cache: read-only token cannot write keys",
        );
    }
    let (level, scope) = match resolve_scope(&q, &auth.context) {
        Ok(s) => s,
        Err(msg) => return text(StatusCode::BAD_REQUEST, msg),
    };
    let ttl = match q.ttl.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        Some(raw) => match parse_duration(raw) {
            Some(d) => Some(d),
            None => {
                return text(
                    StatusCode::BAD_REQUEST,
                    "nx-remote-cache: invalid ttl (use e.g. 30s, 10m, 2h, 7d)",
                )
            }
        },
        None => None,
    };
    let max = state.kv.max_value();
    let too_large = || {
        text(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("nx-remote-cache: value exceeds the {max} byte limit (NX_CACHE_KV_MAX_VALUE)"),
        )
    };
    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok()?.parse::<u64>().ok())
        .is_some_and(|n| n > max)
    {
        return too_large();
    }
    let value = match axum::body::to_bytes(body, max as usize).await {
        Ok(b) => b,
        Err(_) => return too_large(),
    };
    let content_type = headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|s| !s.is_empty() && s.is_ascii())
        .unwrap_or("application/octet-stream")
        .to_owned();
    if content_type.starts_with("application/json") && serde_json::from_slice::<serde_json::Value>(&value).is_err() {
        return text(StatusCode::BAD_REQUEST, "nx-remote-cache: body is not valid JSON");
    }
    let secret = truthy(q.secret.as_deref());
    match state
        .kv
        .set(level, &scope, &key, value, content_type, ttl, secret)
        .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(KvError::TooLarge) => too_large(),
        Err(KvError::Disabled) => disabled(),
        Err(KvError::Redis(e)) => redis_failed(&e),
    }
}

pub async fn get_kv(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<KvQuery>,
    headers: HeaderMap,
) -> Response {
    let start = Instant::now();
    if !state.kv.enabled() {
        return disabled();
    }
    if !valid_key(&key) {
        return text(StatusCode::BAD_REQUEST, "nx-remote-cache: invalid key");
    }
    let auth = match authenticate(&state, &headers, &key, start) {
        Ok(a) => a,
        Err(res) => return res,
    };
    let (level, scope) = match resolve_scope(&q, &auth.context) {
        Ok(s) => s,
        Err(msg) => return text(StatusCode::BAD_REQUEST, msg),
    };
    match state.kv.get(level, &scope, &key).await {
        Ok(Some(entry)) if entry.secret && auth.access != Access::ReadWrite => secret_forbidden(),
        Ok(Some(entry)) => value_response(entry),
        Ok(None) => text(StatusCode::NOT_FOUND, "nx-remote-cache: no such key"),
        Err(KvError::Redis(e)) => redis_failed(&e),
        Err(_) => disabled(),
    }
}

/// A value stored with `secret=true` is only served to the read-write token: the read-only
/// one is handed to pull requests, which must not read what a trusted job stored for the next.
fn secret_forbidden() -> Response {
    text(
        StatusCode::FORBIDDEN,
        "nx-remote-cache: this key is a secret; only the read-write token can read it",
    )
}

pub async fn delete_kv(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<KvQuery>,
    headers: HeaderMap,
) -> Response {
    let start = Instant::now();
    if !state.kv.enabled() {
        return disabled();
    }
    if !valid_key(&key) {
        return text(StatusCode::BAD_REQUEST, "nx-remote-cache: invalid key");
    }
    let auth = match authenticate(&state, &headers, &key, start) {
        Ok(a) => a,
        Err(res) => return res,
    };
    if auth.access != Access::ReadWrite {
        return text(
            StatusCode::FORBIDDEN,
            "nx-remote-cache: read-only token cannot delete keys",
        );
    }
    let (level, scope) = match resolve_scope(&q, &auth.context) {
        Ok(s) => s,
        Err(msg) => return text(StatusCode::BAD_REQUEST, msg),
    };
    match state.kv.delete(level, &scope, &key).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => text(StatusCode::NOT_FOUND, "nx-remote-cache: no such key"),
        Err(KvError::Redis(e)) => redis_failed(&e),
        Err(_) => disabled(),
    }
}

#[derive(Serialize)]
struct ListResponse {
    level: Level,
    #[serde(skip_serializing_if = "String::is_empty")]
    scope: String,
    keys: Vec<KeyInfo>,
}

pub async fn list_kv(State(state): State<AppState>, Query(q): Query<KvQuery>, headers: HeaderMap) -> Response {
    let start = Instant::now();
    if !state.kv.enabled() {
        return disabled();
    }
    let auth = match authenticate(&state, &headers, "kv", start) {
        Ok(a) => a,
        Err(res) => return res,
    };
    let (level, scope) = match resolve_scope(&q, &auth.context) {
        Ok(s) => s,
        Err(msg) => return text(StatusCode::BAD_REQUEST, msg),
    };
    let limit = q.limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT);
    match state
        .kv
        .list(level, &scope, q.prefix.as_deref().unwrap_or(""), limit)
        .await
    {
        Ok(keys) => Json(ListResponse { level, scope, keys }).into_response(),
        Err(KvError::Redis(e)) => redis_failed(&e),
        Err(_) => disabled(),
    }
}

/// What the dashboard shows about the store.
#[derive(Serialize)]
pub struct KvInfo {
    pub enabled: bool,
    pub backend: &'static str,
    pub items: u64,
    pub bytes: u64,
    pub budget_bytes: u64,
    pub max_value_bytes: u64,
}

pub fn info(store: &KvStore) -> KvInfo {
    let s = store.stats();
    KvInfo {
        enabled: store.enabled(),
        backend: store.backend().as_str(),
        items: s.items,
        bytes: s.bytes,
        budget_bytes: store.budget(),
        max_value_bytes: store.max_value(),
    }
}

#[derive(Serialize)]
struct DashboardList {
    #[serde(flatten)]
    info: KvInfo,
    keys: Vec<KeyInfo>,
}

/// `GET /api/v1/kv` (dashboard guard): every key of every scope, values not included.
pub async fn dashboard_list(State(state): State<AppState>) -> Response {
    let keys = if state.kv.enabled() {
        match state.kv.list_all(MAX_LIST_LIMIT).await {
            Ok(keys) => keys,
            Err(KvError::Redis(e)) => return redis_failed(&e),
            Err(_) => Vec::new(),
        }
    } else {
        Vec::new()
    };
    Json(DashboardList {
        info: info(&state.kv),
        keys,
    })
    .into_response()
}

#[derive(Deserialize)]
pub struct ValueQuery {
    level: Option<String>,
    scope: Option<String>,
    key: String,
}

/// `GET /api/v1/kv/value?level=&scope=&key=` (dashboard guard): one value, for the reveal button.
/// Secrets need the read-write cache token in `Authorization`; the dashboard token and the
/// read-only token see the key, never the value.
pub async fn dashboard_value(
    State(state): State<AppState>,
    Query(q): Query<ValueQuery>,
    headers: HeaderMap,
) -> Response {
    if !state.kv.enabled() {
        return disabled();
    }
    let Some(level) = Level::parse(q.level.as_deref().unwrap_or("")) else {
        return text(StatusCode::BAD_REQUEST, "nx-remote-cache: invalid level");
    };
    let scope = q.scope.unwrap_or_default();
    if !valid_key(&q.key) || (level != Level::Global && !valid_scope(&scope)) {
        return text(StatusCode::BAD_REQUEST, "nx-remote-cache: invalid key or scope");
    }
    let read_write = || {
        let presented = crate::routes::presented_bearer(&headers, None);
        state
            .tokens
            .authenticate(presented.as_deref())
            .is_ok_and(|a| a.access == Access::ReadWrite)
    };
    match state.kv.get(level, &scope, &q.key).await {
        Ok(Some(entry)) if entry.secret && !read_write() => secret_forbidden(),
        Ok(Some(entry)) => value_response(entry),
        Ok(None) => text(StatusCode::NOT_FOUND, "nx-remote-cache: no such key"),
        Err(KvError::Redis(e)) => redis_failed(&e),
        Err(_) => disabled(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: Level = Level::Global;
    const TEXT: &str = "text/plain";

    #[tokio::test]
    async fn set_get_delete_and_budget() {
        let kv = KvStore::new(100, 60);
        assert_eq!(
            kv.set(G, "", "a", Bytes::from_static(&[0; 70]), "x".into(), None, false)
                .await,
            Err(KvError::TooLarge)
        );
        kv.set(G, "", "a", Bytes::from_static(&[0; 50]), TEXT.into(), None, false)
            .await
            .unwrap();
        kv.set(G, "", "b", Bytes::from_static(&[0; 40]), TEXT.into(), None, false)
            .await
            .unwrap();
        assert_eq!(kv.stats(), KvStats { items: 2, bytes: 90 });
        // `a` is touched, so `b` is the one evicted when `c` pushes us over budget.
        kv.get(G, "", "a").await.unwrap();
        kv.set(G, "", "c", Bytes::from_static(&[0; 30]), TEXT.into(), None, false)
            .await
            .unwrap();
        assert!(kv.get(G, "", "b").await.unwrap().is_none());
        assert!(kv.get(G, "", "a").await.unwrap().is_some());
        assert!(kv.delete(G, "", "a").await.unwrap());
        assert!(!kv.delete(G, "", "a").await.unwrap());
        assert_eq!(kv.stats(), KvStats { items: 1, bytes: 30 });
    }

    #[tokio::test]
    async fn lru_queue_stays_bounded_under_reads() {
        let kv = KvStore::new(1000, 100);
        kv.set(G, "", "hot", Bytes::from_static(b"1"), TEXT.into(), None, false)
            .await
            .unwrap();
        for _ in 0..10_000 {
            kv.get(G, "", "hot").await.unwrap();
        }
        let inner = kv.inner.lock().unwrap();
        assert!(inner.order.len() <= inner.map.len() * 4 + 64, "{}", inner.order.len());
    }

    #[tokio::test]
    async fn scopes_are_isolated() {
        let kv = KvStore::new(1000, 100);
        kv.set(G, "", "k", Bytes::from_static(b"g"), TEXT.into(), None, false)
            .await
            .unwrap();
        kv.set(
            Level::Pipeline,
            "org/app",
            "k",
            Bytes::from_static(b"p"),
            TEXT.into(),
            None,
            true,
        )
        .await
        .unwrap();
        kv.set(
            Level::Run,
            "42",
            "k",
            Bytes::from_static(b"r"),
            TEXT.into(),
            None,
            false,
        )
        .await
        .unwrap();
        assert_eq!(
            kv.get(G, "", "k").await.unwrap().unwrap().value,
            Bytes::from_static(b"g")
        );
        assert_eq!(
            kv.get(Level::Pipeline, "org/app", "k").await.unwrap().unwrap().value,
            Bytes::from_static(b"p")
        );
        assert!(kv.get(Level::Pipeline, "org/other", "k").await.unwrap().is_none());
        assert_eq!(
            kv.get(Level::Run, "42", "k").await.unwrap().unwrap().value,
            Bytes::from_static(b"r")
        );
        let scoped = kv.list(Level::Pipeline, "org/app", "", 10).await.unwrap();
        assert_eq!(scoped.len(), 1);
        assert!(scoped[0].secret);
        let all = kv.list_all(10).await.unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].level, G); // sorted by level, then scope, then key
    }

    #[tokio::test]
    async fn ttl_expires() {
        let kv = KvStore::new(1000, 100);
        kv.set(
            G,
            "",
            "t",
            Bytes::from_static(b"1"),
            TEXT.into(),
            Some(Duration::from_millis(1)),
            false,
        )
        .await
        .unwrap();
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert!(kv.get(G, "", "t").await.unwrap().is_none());
        assert_eq!(kv.stats().items, 0);
        assert!(kv.list(G, "", "", 10).await.unwrap().is_empty());
    }

    /// Needs a Redis: `NX_CACHE_TEST_REDIS_URL=redis://127.0.0.1:6379/` (skipped otherwise).
    #[tokio::test]
    async fn redis_backend_is_shared_between_pods() {
        let Ok(url) = std::env::var("NX_CACHE_TEST_REDIS_URL") else {
            eprintln!("NX_CACHE_TEST_REDIS_URL not set, skipping");
            return;
        };
        let prefix = format!("nxrc-test:{}", std::process::id());
        let pod_a = KvStore::with_redis(1 << 20, 1 << 16, &url, None, &prefix).unwrap();
        let pod_b = KvStore::with_redis(1 << 20, 1 << 16, &url, None, &prefix).unwrap();
        assert_eq!(pod_a.backend(), Backend::Redis);
        pod_a.probe().await.unwrap();
        pod_a
            .set(
                Level::Run,
                "42",
                "token",
                Bytes::from_static(b"s3cr3t"),
                TEXT.into(),
                Some(Duration::from_secs(60)),
                true,
            )
            .await
            .unwrap();
        pod_a
            .set(
                Level::Pipeline,
                "org/app",
                "build",
                Bytes::from_static(b"7"),
                "application/json".into(),
                None,
                false,
            )
            .await
            .unwrap();
        pod_a
            .set(
                G,
                "",
                "bin",
                Bytes::from_static(&[0, 1, 2, 10, 255]),
                "application/octet-stream".into(),
                None,
                false,
            )
            .await
            .unwrap();
        let seen = pod_b.get(Level::Run, "42", "token").await.unwrap().unwrap();
        assert_eq!(seen.value, Bytes::from_static(b"s3cr3t"));
        assert!(seen.secret);
        assert!(seen.expires.is_some());
        assert_eq!(seen.content_type, TEXT);
        // Binary values with a newline inside survive the envelope.
        assert_eq!(
            pod_b.get(G, "", "bin").await.unwrap().unwrap().value,
            Bytes::from_static(&[0, 1, 2, 10, 255])
        );
        assert!(pod_b.get(Level::Run, "43", "token").await.unwrap().is_none());
        let all = pod_b.list_all(10).await.unwrap();
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].level, G);
        assert_eq!(all[1].scope, "org/app");
        assert_eq!(all[2].key, "token");
        assert_eq!(all[2].size, 6);
        assert!(all[2].expires_in_secs.is_some_and(|s| (55..=60).contains(&s)));
        assert_eq!(pod_b.stats().items, 3);
        let scoped = pod_b.list(Level::Pipeline, "org/app", "bu", 10).await.unwrap();
        assert_eq!(scoped.len(), 1);
        assert!(pod_b.delete(Level::Run, "42", "token").await.unwrap());
        assert!(!pod_a.delete(Level::Run, "42", "token").await.unwrap());
        assert!(pod_a.get(Level::Run, "42", "token").await.unwrap().is_none());
        for (l, s, k) in [(G, "", "bin"), (Level::Pipeline, "org/app", "build")] {
            pod_a.delete(l, s, k).await.unwrap();
        }
        assert!(pod_b.list_all(10).await.unwrap().is_empty());
    }

    #[test]
    fn redis_password_apart_from_the_url() {
        let kv = KvStore::with_redis(1, 1, "redis://redis-master:6379/0", Some("pw"), "nxrc:kv").unwrap();
        let info = kv.redis.as_ref().unwrap().client.get_connection_info();
        assert_eq!(info.redis_settings().password(), Some("pw"));
        assert_eq!(info.redis_settings().db(), 0);
        assert!(KvStore::with_redis(1, 1, "not a url", None, "x").is_err());
    }

    #[test]
    fn keys_and_scopes() {
        assert!(valid_key("last-green:main@v1.2"));
        assert!(!valid_key("a/b"));
        assert!(!valid_key("a|b"));
        assert!(!valid_key(""));
        assert!(!valid_key(&"k".repeat(201)));
        assert!(valid_scope("org/app"));
        assert!(valid_scope("9412876503"));
        assert!(!valid_scope("a b"));
        assert!(!valid_scope("/org"));
        assert!(!valid_scope("org//app"));
        assert_eq!(Level::parse("Pipeline"), Some(Level::Pipeline));
        assert_eq!(Level::parse(""), Some(Level::Global));
        assert_eq!(Level::parse("team"), None);
    }
}
