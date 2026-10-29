//! A small in-memory key-value store on the side of the cache, for CI glue that has nothing to
//! do with Nx: feature flags, "last green sha", a JSON blob shared between jobs. Values keep
//! the content type they were stored with, JSON is validated, and every key may carry a TTL.
//!
//! Keys live in a scope chosen per request: `global` (default), `pipeline` (one namespace per
//! repository) or `run` (one namespace per CI run). The pipeline and run identifiers come from
//! the token context the client already sends (`secret;run=123;repo=org/app`) or from the
//! `pipeline=` / `run=` query parameters. A key stored with `secret=true` is masked in the
//! dashboard until someone clicks reveal.
//!
//! The store is a byte-bounded LRU: when it is full, the least recently read or written key
//! goes first. It lives in one process: with several replicas, route `/v1/kv` to one of them
//! or accept that each pod has its own view.

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
/// Separator inside the map key; never valid in a key or scope, so composed keys cannot collide.
const SEP: char = '\u{1f}';

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

pub struct KvStore {
    budget: u64,
    max_value: u64,
    inner: Mutex<Inner>,
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

#[derive(Debug, PartialEq, Eq)]
pub enum KvError {
    Disabled,
    TooLarge,
}

impl KvStore {
    /// `budget == 0` disables the store (its routes answer 404).
    pub fn new(budget: u64, max_value: u64) -> Self {
        Self {
            budget,
            max_value,
            inner: Mutex::new(Inner::default()),
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
    pub fn set(
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
        if len > self.max_value || len > self.budget {
            return Err(KvError::TooLarge);
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

    pub fn get(&self, level: Level, scope: &str, key: &str) -> Option<Entry> {
        let now = Instant::now();
        let full = compose(level, scope, key);
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let expired = inner
            .map
            .get(&full)
            .is_some_and(|s| s.entry.expires.is_some_and(|x| x <= now));
        if expired {
            remove(&mut inner, &full);
            return None;
        }
        if !inner.map.contains_key(&full) {
            return None;
        }
        let generation = inner.touch(&full);
        let stored = inner.map.get_mut(&full)?;
        stored.generation = generation;
        Some(stored.entry.clone())
    }

    pub fn delete(&self, level: Level, scope: &str, key: &str) -> bool {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        remove(&mut inner, &compose(level, scope, key))
    }

    /// Live keys of one scope starting with `prefix`, sorted, at most `limit` of them.
    pub fn list(&self, level: Level, scope: &str, prefix: &str, limit: usize) -> Vec<KeyInfo> {
        self.collect(limit, |s| {
            s.level == level && s.scope == scope && s.key.starts_with(prefix)
        })
    }

    /// Every live key of every scope, for the dashboard.
    pub fn list_all(&self, limit: usize) -> Vec<KeyInfo> {
        self.collect(limit, |_| true)
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
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        KvStats {
            items: inner.map.len() as u64,
            bytes: inner.bytes,
        }
    }
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
    match state.kv.set(level, &scope, &key, value, content_type, ttl, secret) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(KvError::TooLarge) => too_large(),
        Err(KvError::Disabled) => disabled(),
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
    match state.kv.get(level, &scope, &key) {
        Some(entry) => value_response(entry),
        None => text(StatusCode::NOT_FOUND, "nx-remote-cache: no such key"),
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
    if state.kv.delete(level, &scope, &key) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        text(StatusCode::NOT_FOUND, "nx-remote-cache: no such key")
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
    let keys = state.kv.list(level, &scope, q.prefix.as_deref().unwrap_or(""), limit);
    Json(ListResponse { level, scope, keys }).into_response()
}

/// What the dashboard shows about the store.
#[derive(Serialize)]
pub struct KvInfo {
    pub enabled: bool,
    pub items: u64,
    pub bytes: u64,
    pub budget_bytes: u64,
    pub max_value_bytes: u64,
}

pub fn info(store: &KvStore) -> KvInfo {
    let s = store.stats();
    KvInfo {
        enabled: store.enabled(),
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
        state.kv.list_all(MAX_LIST_LIMIT)
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
    match state.kv.get(level, &scope, &q.key) {
        Some(entry) => value_response(entry),
        None => text(StatusCode::NOT_FOUND, "nx-remote-cache: no such key"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const G: Level = Level::Global;

    #[test]
    fn set_get_delete_and_budget() {
        let kv = KvStore::new(100, 60);
        assert_eq!(
            kv.set(G, "", "a", Bytes::from_static(&[0; 70]), "x".into(), None, false),
            Err(KvError::TooLarge)
        );
        kv.set(
            G,
            "",
            "a",
            Bytes::from_static(&[0; 50]),
            "text/plain".into(),
            None,
            false,
        )
        .unwrap();
        kv.set(
            G,
            "",
            "b",
            Bytes::from_static(&[0; 40]),
            "text/plain".into(),
            None,
            false,
        )
        .unwrap();
        assert_eq!(kv.stats(), KvStats { items: 2, bytes: 90 });
        // `a` is touched, so `b` is the one evicted when `c` pushes us over budget.
        kv.get(G, "", "a");
        kv.set(
            G,
            "",
            "c",
            Bytes::from_static(&[0; 30]),
            "text/plain".into(),
            None,
            false,
        )
        .unwrap();
        assert!(kv.get(G, "", "b").is_none());
        assert!(kv.get(G, "", "a").is_some());
        assert!(kv.delete(G, "", "a"));
        assert!(!kv.delete(G, "", "a"));
        assert_eq!(kv.stats(), KvStats { items: 1, bytes: 30 });
    }

    #[test]
    fn scopes_are_isolated() {
        let kv = KvStore::new(1000, 100);
        kv.set(G, "", "k", Bytes::from_static(b"g"), "text/plain".into(), None, false)
            .unwrap();
        kv.set(
            Level::Pipeline,
            "org/app",
            "k",
            Bytes::from_static(b"p"),
            "text/plain".into(),
            None,
            true,
        )
        .unwrap();
        kv.set(
            Level::Run,
            "42",
            "k",
            Bytes::from_static(b"r"),
            "text/plain".into(),
            None,
            false,
        )
        .unwrap();
        assert_eq!(kv.get(G, "", "k").unwrap().value, Bytes::from_static(b"g"));
        assert_eq!(
            kv.get(Level::Pipeline, "org/app", "k").unwrap().value,
            Bytes::from_static(b"p")
        );
        assert!(kv.get(Level::Pipeline, "org/other", "k").is_none());
        assert_eq!(kv.get(Level::Run, "42", "k").unwrap().value, Bytes::from_static(b"r"));
        assert_eq!(kv.list(Level::Pipeline, "org/app", "", 10).len(), 1);
        assert!(kv.list(Level::Pipeline, "org/app", "", 10)[0].secret);
        let all = kv.list_all(10);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].level, G); // sorted by level, then scope, then key
    }

    #[test]
    fn ttl_expires() {
        let kv = KvStore::new(1000, 100);
        kv.set(
            G,
            "",
            "t",
            Bytes::from_static(b"1"),
            "text/plain".into(),
            Some(Duration::from_millis(1)),
            false,
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(kv.get(G, "", "t").is_none());
        assert_eq!(kv.stats().items, 0);
        assert!(kv.list(G, "", "", 10).is_empty());
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
        assert_eq!(Level::parse("Pipeline"), Some(Level::Pipeline));
        assert_eq!(Level::parse(""), Some(Level::Global));
        assert_eq!(Level::parse("team"), None);
    }
}
