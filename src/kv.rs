//! A small in-memory key-value store on the side of the cache, for CI glue that has nothing to
//! do with Nx: feature flags, "last green sha", a JSON blob shared between jobs. Values keep
//! the content type they were stored with, JSON is validated, and every key may carry a TTL.
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
use crate::auth::Access;
use crate::config::parse_duration;
use crate::routes::{authenticate, text};

const MAX_KEY_LEN: usize = 200;
const DEFAULT_LIST_LIMIT: usize = 100;
const MAX_LIST_LIMIT: usize = 1000;

#[derive(Clone)]
pub struct Entry {
    pub value: Bytes,
    pub content_type: String,
    pub expires: Option<Instant>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct KvStats {
    pub items: u64,
    pub bytes: u64,
}

pub struct KvStore {
    budget: u64,
    max_value: u64,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    map: HashMap<String, (Entry, u64)>, // value, generation of its last use
    order: VecDeque<(String, u64)>,     // (key, generation) oldest first; stale pairs are skipped
    bytes: u64,
    generation: u64,
}

impl Inner {
    /// Mark `key` as just used: a fresh generation at the back of the queue.
    fn touch(&mut self, key: &str) -> u64 {
        self.generation += 1;
        self.order.push_back((key.to_owned(), self.generation));
        self.generation
    }
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

    pub fn max_value(&self) -> u64 {
        self.max_value
    }

    pub fn set(&self, key: &str, value: Bytes, content_type: String, ttl: Option<Duration>) -> Result<(), KvError> {
        if !self.enabled() {
            return Err(KvError::Disabled);
        }
        let len = value.len() as u64;
        if len > self.max_value || len > self.budget {
            return Err(KvError::TooLarge);
        }
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        remove(&mut inner, key);
        inner.bytes += len;
        let generation = inner.touch(key);
        let entry = Entry {
            value,
            content_type,
            expires: ttl.map(|t| now + t),
        };
        inner.map.insert(key.to_owned(), (entry, generation));
        while inner.bytes > self.budget {
            evict_one(&mut inner);
        }
        Ok(())
    }

    pub fn get(&self, key: &str) -> Option<Entry> {
        let now = Instant::now();
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let expired = inner
            .map
            .get(key)
            .is_some_and(|(e, _)| e.expires.is_some_and(|x| x <= now));
        if expired {
            remove(&mut inner, key);
            return None;
        }
        if !inner.map.contains_key(key) {
            return None;
        }
        let generation = inner.touch(key);
        let (entry, last) = inner.map.get_mut(key)?;
        *last = generation;
        Some(entry.clone())
    }

    pub fn delete(&self, key: &str) -> bool {
        let mut inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        remove(&mut inner, key)
    }

    /// Live keys starting with `prefix`, sorted, at most `limit` of them.
    pub fn list(&self, prefix: &str, limit: usize) -> Vec<KeyInfo> {
        let now = Instant::now();
        let inner = self.inner.lock().unwrap_or_else(|p| p.into_inner());
        let mut keys: Vec<KeyInfo> = inner
            .map
            .iter()
            .filter(|(k, (e, _))| k.starts_with(prefix) && e.expires.is_none_or(|x| x > now))
            .map(|(k, (e, _))| KeyInfo {
                key: k.clone(),
                size: e.value.len() as u64,
                content_type: e.content_type.clone(),
                expires_in_secs: e.expires.map(|x| x.saturating_duration_since(now).as_secs()),
            })
            .collect();
        keys.sort_by(|a, b| a.key.cmp(&b.key));
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

fn remove(inner: &mut Inner, key: &str) -> bool {
    match inner.map.remove(key) {
        Some((e, _)) => {
            inner.bytes -= e.value.len() as u64;
            true
        }
        None => false,
    }
}

/// Drop the least recently used live key. Queue entries whose generation no longer matches
/// the map are leftovers of a later touch and are skipped.
fn evict_one(inner: &mut Inner) {
    while let Some((key, generation)) = inner.order.pop_front() {
        if inner.map.get(&key).is_some_and(|(_, g)| *g == generation) {
            remove(inner, &key);
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

#[derive(Serialize)]
pub struct KeyInfo {
    pub key: String,
    pub size: u64,
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_in_secs: Option<u64>,
}

#[derive(Deserialize, Default)]
pub struct PutQuery {
    /// `30s`, `10m`, `2h`, `7d` or a number of seconds; absent means no expiry.
    pub ttl: Option<String>,
}

#[derive(Deserialize, Default)]
pub struct ListQuery {
    pub prefix: Option<String>,
    pub limit: Option<usize>,
}

#[derive(Serialize)]
struct ListResponse {
    keys: Vec<KeyInfo>,
}

fn disabled() -> Response {
    text(
        StatusCode::NOT_FOUND,
        "nx-remote-cache: key-value store disabled (NX_CACHE_KV_BYTES=0)",
    )
}

pub async fn put_kv(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<PutQuery>,
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
    match state.kv.set(&key, value, content_type, ttl) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(KvError::TooLarge) => too_large(),
        Err(KvError::Disabled) => disabled(),
    }
}

pub async fn get_kv(State(state): State<AppState>, Path(key): Path<String>, headers: HeaderMap) -> Response {
    let start = Instant::now();
    if !state.kv.enabled() {
        return disabled();
    }
    if !valid_key(&key) {
        return text(StatusCode::BAD_REQUEST, "nx-remote-cache: invalid key");
    }
    if let Err(res) = authenticate(&state, &headers, &key, start) {
        return res;
    }
    let Some(entry) = state.kv.get(&key) else {
        return text(StatusCode::NOT_FOUND, "nx-remote-cache: no such key");
    };
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
    res
}

pub async fn delete_kv(State(state): State<AppState>, Path(key): Path<String>, headers: HeaderMap) -> Response {
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
    if state.kv.delete(&key) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        text(StatusCode::NOT_FOUND, "nx-remote-cache: no such key")
    }
}

pub async fn list_kv(State(state): State<AppState>, Query(q): Query<ListQuery>, headers: HeaderMap) -> Response {
    let start = Instant::now();
    if !state.kv.enabled() {
        return disabled();
    }
    if let Err(res) = authenticate(&state, &headers, "kv", start) {
        return res;
    }
    let limit = q.limit.unwrap_or(DEFAULT_LIST_LIMIT).clamp(1, MAX_LIST_LIMIT);
    let keys = state.kv.list(q.prefix.as_deref().unwrap_or(""), limit);
    Json(ListResponse { keys }).into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_get_delete_and_budget() {
        let kv = KvStore::new(100, 60);
        assert_eq!(
            kv.set("a", Bytes::from_static(&[0; 70]), "x".into(), None),
            Err(KvError::TooLarge)
        );
        kv.set("a", Bytes::from_static(&[0; 50]), "text/plain".into(), None)
            .unwrap();
        kv.set("b", Bytes::from_static(&[0; 40]), "text/plain".into(), None)
            .unwrap();
        assert_eq!(kv.stats(), KvStats { items: 2, bytes: 90 });
        // `a` is touched, so `b` is the one evicted when `c` pushes us over budget.
        kv.get("a");
        kv.set("c", Bytes::from_static(&[0; 30]), "text/plain".into(), None)
            .unwrap();
        assert!(kv.get("b").is_none());
        assert!(kv.get("a").is_some());
        assert!(kv.delete("a"));
        assert!(!kv.delete("a"));
        assert_eq!(kv.stats(), KvStats { items: 1, bytes: 30 });
    }

    #[test]
    fn ttl_expires() {
        let kv = KvStore::new(1000, 100);
        kv.set(
            "t",
            Bytes::from_static(b"1"),
            "text/plain".into(),
            Some(Duration::from_millis(1)),
        )
        .unwrap();
        std::thread::sleep(Duration::from_millis(5));
        assert!(kv.get("t").is_none());
        assert_eq!(kv.stats().items, 0);
        assert!(kv.list("", 10).is_empty());
    }

    #[test]
    fn keys() {
        assert!(valid_key("last-green:main@v1.2"));
        assert!(!valid_key("a/b"));
        assert!(!valid_key(""));
        assert!(!valid_key(&"k".repeat(201)));
    }
}
