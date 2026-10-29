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
//!   replicas each pod has its own view (route `/v1/kv` to one pod, or use `store`).
//! - `store`: every key is a small object under `<prefix>/kv/` in the artifact store, so every
//!   replica sees the same keys and they survive restarts. One store round trip per operation.

use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::body::Body;
use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use bytes::Bytes;
use futures_util::{StreamExt, TryStreamExt};
use object_store::path::Path as StorePath;
use object_store::{Error as StoreError, ObjectStore, ObjectStoreExt, PutPayload};
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
/// Parallel store reads when listing with metadata.
const LIST_FANOUT: usize = 16;

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
    Store,
}

impl Backend {
    pub fn as_str(self) -> &'static str {
        match self {
            Backend::Memory => "memory",
            Backend::Store => "store",
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
    Store(StoreError),
}

impl PartialEq for KvError {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (KvError::Disabled, KvError::Disabled) | (KvError::TooLarge, KvError::TooLarge)
        )
    }
}

impl From<StoreError> for KvError {
    fn from(e: StoreError) -> Self {
        KvError::Store(e)
    }
}

pub struct KvStore {
    budget: u64,
    max_value: u64,
    inner: Mutex<Inner>,
    shared: Option<Shared>,
    /// `store` backend: counts from the last listing (the store is not asked on every scrape).
    last_seen: Mutex<KvStats>,
}

struct Shared {
    store: Arc<dyn ObjectStore>,
    /// `<prefix>/kv`, without leading or trailing slash; empty prefix gives `kv`.
    root: String,
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
}

fn compose(level: Level, scope: &str, key: &str) -> String {
    format!("{}{SEP}{scope}{SEP}{key}", level.as_str())
}

/// What a `store`-backed value looks like on disk: one JSON header line, then the raw bytes.
#[derive(Serialize, Deserialize)]
struct Envelope {
    ct: String,
    secret: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    expires_at: Option<u64>,
}

fn now_unix() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Parsed path of a store object under the kv root: `level/<scope segments>/key`.
#[derive(Clone)]
pub struct KeyRef {
    pub level: Level,
    pub scope: String,
    pub key: String,
}

impl KvStore {
    /// In-process LRU. `budget == 0` disables the store (its routes answer 404).
    pub fn new(budget: u64, max_value: u64) -> Self {
        Self {
            budget,
            max_value,
            inner: Mutex::new(Inner::default()),
            shared: None,
            last_seen: Mutex::new(KvStats::default()),
        }
    }

    /// Keys kept as objects under `<prefix>/kv/` in the artifact store, shared by every replica.
    /// `budget` only keeps the enabled/disabled semantics (`0` disables).
    pub fn with_store(budget: u64, max_value: u64, store: Arc<dyn ObjectStore>, prefix: &StorePath) -> Self {
        let p = prefix.as_ref().trim_matches('/');
        let root = if p.is_empty() {
            "kv".to_owned()
        } else {
            format!("{p}/kv")
        };
        Self {
            budget,
            max_value,
            inner: Mutex::new(Inner::default()),
            shared: Some(Shared { store, root }),
            last_seen: Mutex::new(KvStats::default()),
        }
    }

    pub fn backend(&self) -> Backend {
        if self.shared.is_some() {
            Backend::Store
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
        if len > self.max_value || (self.shared.is_none() && len > self.budget) {
            return Err(KvError::TooLarge);
        }
        if let Some(shared) = &self.shared {
            let header = Envelope {
                ct: content_type,
                secret,
                expires_at: ttl.map(|t| now_unix() + t.as_secs().max(1)),
            };
            let mut body = serde_json::to_vec(&header).expect("envelope serialises");
            body.push(b'\n');
            body.extend_from_slice(&value);
            shared
                .store
                .put(&shared.path(level, scope, key), PutPayload::from(body))
                .await?;
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
        Ok(())
    }

    pub async fn get(&self, level: Level, scope: &str, key: &str) -> Result<Option<Entry>, KvError> {
        if let Some(shared) = &self.shared {
            let path = shared.path(level, scope, key);
            return match shared.store.get(&path).await {
                Ok(result) => {
                    let bytes = result.bytes().await?;
                    match decode(&bytes) {
                        Some(entry) => Ok(Some(entry)),
                        None => {
                            // Expired (or unreadable): drop it so the next reader does not pay again.
                            let _ = shared.store.delete(&path).await;
                            Ok(None)
                        }
                    }
                }
                Err(StoreError::NotFound { .. }) => Ok(None),
                Err(e) => Err(e.into()),
            };
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
        Ok(Some(stored.entry.clone()))
    }

    pub async fn delete(&self, level: Level, scope: &str, key: &str) -> Result<bool, KvError> {
        if let Some(shared) = &self.shared {
            let path = shared.path(level, scope, key);
            // Object stores answer 204 for a missing key, so look first to report 404 honestly.
            return match shared.store.head(&path).await {
                Ok(_) => {
                    shared.store.delete(&path).await?;
                    Ok(true)
                }
                Err(StoreError::NotFound { .. }) => Ok(false),
                Err(e) => Err(e.into()),
            };
        }
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        Ok(remove(&mut inner, &compose(level, scope, key)))
    }

    /// Live keys of one scope starting with `prefix`, sorted, at most `limit` of them.
    pub async fn list(&self, level: Level, scope: &str, prefix: &str, limit: usize) -> Result<Vec<KeyInfo>, KvError> {
        if let Some(shared) = &self.shared {
            let base = shared.scope_path(level, scope);
            let mut keys = self
                .list_store(shared, Some(&base), limit, |r| {
                    r.level == level && r.scope == scope && r.key.starts_with(prefix)
                })
                .await?;
            keys.truncate(limit);
            return Ok(keys);
        }
        Ok(self.collect(limit, |s| {
            s.level == level && s.scope == scope && s.key.starts_with(prefix)
        }))
    }

    /// Every live key of every scope, for the dashboard.
    pub async fn list_all(&self, limit: usize) -> Result<Vec<KeyInfo>, KvError> {
        if let Some(shared) = &self.shared {
            let base = StorePath::from(shared.root.as_str());
            let keys = self.list_store(shared, Some(&base), limit, |_| true).await?;
            let stats = KvStats {
                items: keys.len() as u64,
                bytes: keys.iter().map(|k| k.size).sum(),
            };
            *self.last_seen.lock().unwrap_or_else(|p| p.into_inner()) = stats;
            return Ok(keys);
        }
        Ok(self.collect(limit, |_| true))
    }

    async fn list_store(
        &self,
        shared: &Shared,
        base: Option<&StorePath>,
        limit: usize,
        keep: impl Fn(&KeyRef) -> bool,
    ) -> Result<Vec<KeyInfo>, KvError> {
        let mut refs: Vec<(KeyRef, StorePath)> = Vec::new();
        let mut stream = shared.store.list(base);
        while let Some(meta) = stream.try_next().await? {
            if let Some(r) = shared.parse(&meta.location) {
                if keep(&r) {
                    refs.push((r, meta.location));
                }
            }
        }
        refs.sort_by(|a, b| {
            (a.0.level.as_str(), &a.0.scope, &a.0.key).cmp(&(b.0.level.as_str(), &b.0.scope, &b.0.key))
        });
        refs.truncate(limit);
        let now = now_unix();
        let infos: Vec<Option<KeyInfo>> = futures_util::stream::iter(refs)
            .map(|(r, path)| {
                let store = shared.store.clone();
                async move {
                    let bytes = store.get(&path).await.ok()?.bytes().await.ok()?;
                    let (header, value_len) = split(&bytes)?;
                    if header.expires_at.is_some_and(|x| x <= now) {
                        let _ = store.delete(&path).await;
                        return None;
                    }
                    Some(KeyInfo {
                        level: r.level,
                        scope: r.scope,
                        key: r.key,
                        size: value_len as u64,
                        content_type: header.ct,
                        secret: header.secret,
                        expires_in_secs: header.expires_at.map(|x| x.saturating_sub(now)),
                    })
                }
            })
            .buffer_unordered(LIST_FANOUT)
            .collect()
            .await;
        let mut keys: Vec<KeyInfo> = infos.into_iter().flatten().collect();
        keys.sort_by(|a, b| (a.level.as_str(), &a.scope, &a.key).cmp(&(b.level.as_str(), &b.scope, &b.key)));
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
        if self.shared.is_some() {
            return *self.last_seen.lock().unwrap_or_else(|p| p.into_inner());
        }
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        KvStats {
            items: inner.map.len() as u64,
            bytes: inner.bytes,
        }
    }
}

impl Shared {
    /// `<root>/<level>/<scope segments>/<key>`, empty segments skipped (global has no scope).
    fn path(&self, level: Level, scope: &str, key: &str) -> StorePath {
        let mut s = format!("{}/{}", self.root, level.as_str());
        for seg in scope.split('/').filter(|x| !x.is_empty()) {
            s.push('/');
            s.push_str(seg);
        }
        s.push('/');
        s.push_str(key);
        StorePath::from(s)
    }

    fn scope_path(&self, level: Level, scope: &str) -> StorePath {
        let mut s = format!("{}/{}", self.root, level.as_str());
        for seg in scope.split('/').filter(|x| !x.is_empty()) {
            s.push('/');
            s.push_str(seg);
        }
        StorePath::from(s)
    }

    fn parse(&self, location: &StorePath) -> Option<KeyRef> {
        let rel = location.as_ref().strip_prefix(self.root.as_str())?.strip_prefix('/')?;
        let parts: Vec<&str> = rel.split('/').collect();
        if parts.len() < 2 {
            return None;
        }
        let level = Level::parse(parts[0])?;
        let key = parts[parts.len() - 1].to_owned();
        let scope = parts[1..parts.len() - 1].join("/");
        if !valid_key(&key) {
            return None;
        }
        Some(KeyRef { level, scope, key })
    }
}

/// Header and value length of a stored envelope; `None` when the object is not one of ours.
fn split(bytes: &[u8]) -> Option<(Envelope, usize)> {
    let nl = bytes.iter().position(|b| *b == b'\n')?;
    let header: Envelope = serde_json::from_slice(&bytes[..nl]).ok()?;
    Some((header, bytes.len() - nl - 1))
}

/// Entry from a stored envelope, `None` when expired or unreadable.
fn decode(bytes: &Bytes) -> Option<Entry> {
    let (header, _) = split(bytes)?;
    let now = now_unix();
    if header.expires_at.is_some_and(|x| x <= now) {
        return None;
    }
    let nl = bytes.iter().position(|b| *b == b'\n')?;
    Some(Entry {
        value: bytes.slice(nl + 1..),
        content_type: header.ct,
        expires: header
            .expires_at
            .map(|x| Instant::now() + Duration::from_secs(x.saturating_sub(now))),
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

fn store_failed(e: &StoreError) -> Response {
    tracing::warn!(error = %e, "key-value store operation failed");
    text(StatusCode::BAD_GATEWAY, "nx-remote-cache: key-value store unavailable")
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
        Err(KvError::Store(e)) => store_failed(&e),
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
        Ok(Some(entry)) => value_response(entry),
        Ok(None) => text(StatusCode::NOT_FOUND, "nx-remote-cache: no such key"),
        Err(KvError::Store(e)) => store_failed(&e),
        Err(_) => disabled(),
    }
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
        Err(KvError::Store(e)) => store_failed(&e),
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
        Err(KvError::Store(e)) => store_failed(&e),
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
            Err(KvError::Store(e)) => return store_failed(&e),
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
pub async fn dashboard_value(State(state): State<AppState>, Query(q): Query<ValueQuery>) -> Response {
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
    match state.kv.get(level, &scope, &q.key).await {
        Ok(Some(entry)) => value_response(entry),
        Ok(None) => text(StatusCode::NOT_FOUND, "nx-remote-cache: no such key"),
        Err(KvError::Store(e)) => store_failed(&e),
        Err(_) => disabled(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use object_store::memory::InMemory;

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

    #[tokio::test]
    async fn store_backend_is_shared_between_pods() {
        // Two servers (two pods) over one object store see the same keys.
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let prefix = StorePath::from("cache");
        let pod_a = KvStore::with_store(1 << 20, 1 << 16, store.clone(), &prefix);
        let pod_b = KvStore::with_store(1 << 20, 1 << 16, store.clone(), &prefix);
        assert_eq!(pod_a.backend(), Backend::Store);
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
        let seen = pod_b.get(Level::Run, "42", "token").await.unwrap().unwrap();
        assert_eq!(seen.value, Bytes::from_static(b"s3cr3t"));
        assert!(seen.secret);
        assert!(seen.expires.is_some());
        assert!(pod_b.get(Level::Run, "43", "token").await.unwrap().is_none());
        let all = pod_b.list_all(10).await.unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].level, Level::Pipeline);
        assert_eq!(all[0].scope, "org/app");
        assert_eq!(all[1].key, "token");
        assert_eq!(all[1].size, 6);
        assert_eq!(pod_b.stats().items, 2);
        let scoped = pod_b.list(Level::Pipeline, "org/app", "bu", 10).await.unwrap();
        assert_eq!(scoped.len(), 1);
        assert!(pod_b.delete(Level::Run, "42", "token").await.unwrap());
        assert!(!pod_a.delete(Level::Run, "42", "token").await.unwrap());
        assert!(pod_a.get(Level::Run, "42", "token").await.unwrap().is_none());
        // Keys live under <prefix>/kv/, apart from the artifacts.
        let objects: Vec<String> = store
            .list(None)
            .map_ok(|m| m.location.to_string())
            .try_collect()
            .await
            .unwrap();
        assert_eq!(objects, vec!["cache/kv/pipeline/org/app/build".to_string()]);
    }

    #[tokio::test]
    async fn store_backend_expiry() {
        let store: Arc<dyn ObjectStore> = Arc::new(InMemory::new());
        let kv = KvStore::with_store(1 << 20, 1 << 16, store, &StorePath::from(""));
        // A TTL is rounded up to one second; write an already-expired envelope by hand instead.
        kv.set(
            G,
            "",
            "fresh",
            Bytes::from_static(b"1"),
            TEXT.into(),
            Some(Duration::from_secs(60)),
            false,
        )
        .await
        .unwrap();
        let shared = kv.shared.as_ref().unwrap();
        let stale = format!(
            "{{\"ct\":\"text/plain\",\"secret\":false,\"expires_at\":{}}}\nold",
            now_unix() - 1
        );
        shared
            .store
            .put(&shared.path(G, "", "stale"), PutPayload::from(stale.into_bytes()))
            .await
            .unwrap();
        assert!(kv.get(G, "", "stale").await.unwrap().is_none());
        let all = kv.list_all(10).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].key, "fresh");
        assert!(all[0].expires_in_secs.is_some_and(|s| (55..=60).contains(&s)));
    }

    #[test]
    fn keys_and_scopes() {
        assert!(valid_key("last-green:main@v1.2"));
        assert!(!valid_key("a/b"));
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
