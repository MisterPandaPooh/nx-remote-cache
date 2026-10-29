use std::convert::Infallible;
use std::time::Instant;

use axum::body::Body;
use axum::extract::{Path, Query, Request, State};
use axum::http::{header, HeaderMap, HeaderName, HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::Json;
use bytes::Bytes;
use futures_util::{Stream, StreamExt, TryStreamExt};
use serde::{Deserialize, Serialize};
use tokio_stream::wrappers::BroadcastStream;

use crate::app::{AppState, VERSION};
use crate::auth::{Access, AuthError, Authenticated, Context, Tokens};
use crate::stats::{Event, Kind, LatencySummary, TimelinePoint};
use crate::store::{PutError, PutOutcome};

const OCTET_STREAM: HeaderValue = HeaderValue::from_static("application/octet-stream");

/// Nx shows these messages verbatim to the developer, so they say what to fix.
pub(crate) fn text(status: StatusCode, msg: impl Into<String>) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        msg.into(),
    )
        .into_response()
}

pub fn valid_hash(hash: &str) -> bool {
    (1..=128).contains(&hash.len())
        && hash
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

#[allow(clippy::result_large_err)]
pub(crate) fn authenticate(
    state: &AppState,
    headers: &HeaderMap,
    hash: &str,
    start: Instant,
) -> Result<Authenticated, Response> {
    let auth = headers.get(header::AUTHORIZATION).and_then(|v| v.to_str().ok());
    state.tokens.authenticate(auth).map_err(|e| {
        state.stats.record(Kind::Unauthorized, hash, 0, start.elapsed(), 401);
        match e {
            AuthError::Missing => text(
                StatusCode::UNAUTHORIZED,
                "nx-remote-cache: missing access token. Set NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN.",
            ),
            AuthError::Invalid => text(
                StatusCode::UNAUTHORIZED,
                "nx-remote-cache: invalid access token. Check NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN.",
            ),
        }
    })
}

pub async fn get_artifact(State(state): State<AppState>, Path(hash): Path<String>, headers: HeaderMap) -> Response {
    let start = Instant::now();
    if !valid_hash(&hash) {
        return text(StatusCode::BAD_REQUEST, "nx-remote-cache: invalid cache hash");
    }
    let auth = match authenticate(&state, &headers, &hash, start) {
        Ok(a) => a,
        Err(res) => return res,
    };
    let ctx = auth.context;
    if let Some(entry) = state.memcache.get(&hash) {
        let len = entry.data.len() as u64;
        state.stats.record_full(
            Kind::Hit,
            &hash,
            len,
            start.elapsed(),
            200,
            Some(ctx),
            Some((*entry.origin).clone()),
        );
        let mut res = Response::new(Body::from(entry.data));
        let h = res.headers_mut();
        h.insert(header::CONTENT_TYPE, OCTET_STREAM);
        h.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
        h.insert(
            header::CACHE_CONTROL,
            HeaderValue::from_static("private, max-age=31536000, immutable"),
        );
        h.insert(
            HeaderName::from_static("x-cache-tier"),
            HeaderValue::from_static("memory"),
        );
        return res;
    }
    match state.store.get(&hash).await {
        Ok(Some(result)) => {
            let len = result.meta.size;
            let etag = result.meta.e_tag.clone();
            let origin = crate::store::origin_from(&result.attributes);
            state.stats.record_full(
                Kind::Hit,
                &hash,
                len,
                start.elapsed(),
                200,
                Some(ctx),
                Some(origin.clone()),
            );
            let body = if len <= state.config.memory_cache_max_item && state.memcache.enabled() {
                // Small enough to keep: read it once, remember it, serve it.
                match result.bytes().await {
                    Ok(data) => {
                        state.memcache.insert(&hash, data.clone(), origin);
                        Body::from(data)
                    }
                    Err(e) => {
                        tracing::error!(hash, error = %e, "store read failed");
                        return text(
                            StatusCode::BAD_GATEWAY,
                            format!("nx-remote-cache: store read failed: {e}"),
                        );
                    }
                }
            } else {
                Body::from_stream(result.into_stream().map_err(std::io::Error::other))
            };
            let mut res = Response::new(body);
            let h = res.headers_mut();
            h.insert(header::CONTENT_TYPE, OCTET_STREAM);
            h.insert(header::CONTENT_LENGTH, HeaderValue::from(len));
            h.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=31536000, immutable"),
            );
            h.insert(
                HeaderName::from_static("x-cache-tier"),
                HeaderValue::from_static("store"),
            );
            if let Some(tag) = etag.and_then(|t| HeaderValue::from_str(&t).ok()) {
                h.insert(header::ETAG, tag);
            }
            res
        }
        Ok(None) => {
            state
                .stats
                .record_full(Kind::Miss, &hash, 0, start.elapsed(), 404, Some(ctx), None);
            StatusCode::NOT_FOUND.into_response()
        }
        Err(e) => {
            state.stats.record(Kind::Error, &hash, 0, start.elapsed(), 502);
            tracing::error!(hash, error = %e, "store read failed");
            text(
                StatusCode::BAD_GATEWAY,
                format!("nx-remote-cache: store read failed: {e}"),
            )
        }
    }
}

pub async fn put_artifact(
    State(state): State<AppState>,
    Path(hash): Path<String>,
    headers: HeaderMap,
    body: Body,
) -> Response {
    let start = Instant::now();
    if !valid_hash(&hash) {
        return text(StatusCode::BAD_REQUEST, "nx-remote-cache: invalid cache hash");
    }
    // Early answers still read the body first: closing the socket with unread bytes makes the
    // kernel send a RST, and the Nx client then sees a network error instead of our status.
    let ctx: Context = match authenticate(&state, &headers, &hash, start) {
        Ok(Authenticated {
            access: Access::ReadWrite,
            context,
        }) => context,
        Ok(Authenticated {
            access: Access::ReadOnly,
            ..
        }) => {
            state.stats.record(Kind::Forbidden, &hash, 0, start.elapsed(), 403);
            drain(body, DRAIN_BUDGET).await;
            return text(
                StatusCode::FORBIDDEN,
                "nx-remote-cache: this token is read-only; artifacts are not uploaded.",
            );
        }
        Err(res) => {
            drain(body, DRAIN_BUDGET).await;
            return res;
        }
    };

    let max = state.config.max_artifact_bytes;
    let expected = match headers
        .get(header::CONTENT_LENGTH)
        .map(|v| v.to_str().unwrap_or("").parse::<u64>())
    {
        Some(Ok(n)) => Some(n),
        Some(Err(_)) => return text(StatusCode::BAD_REQUEST, "nx-remote-cache: invalid Content-Length"),
        None => None,
    };
    if expected.is_some_and(|n| n > max) {
        state
            .stats
            .record(Kind::Error, &hash, expected.unwrap_or(0), start.elapsed(), 413);
        drain(body, DRAIN_BUDGET).await;
        return text(
            StatusCode::PAYLOAD_TOO_LARGE,
            format!("nx-remote-cache: artifact exceeds the {max} byte limit (NX_CACHE_MAX_ARTIFACT_BYTES)"),
        );
    }

    let _permit = state.put_permits.acquire().await.expect("semaphore never closes");
    let counted = counted_stream(body, max);
    // Small uploads are copied into the memory tier on the way through (write-through).
    // Write-through is capped at 1 MiB: above that the extra copy costs more than the first
    // read it saves, and a read populates the tier anyway.
    let keep = expected.is_some_and(|n| n <= state.config.memory_cache_max_item.min(WRITE_THROUGH_MAX))
        && state.memcache.enabled();
    let tee: std::sync::Arc<std::sync::Mutex<Vec<u8>>> = Default::default();
    let outcome = if keep {
        let sink = tee.clone();
        sink.lock()
            .unwrap_or_else(|p| p.into_inner())
            .reserve(expected.unwrap_or(0) as usize);
        let teed =
            counted.inspect_ok(move |chunk| sink.lock().unwrap_or_else(|p| p.into_inner()).extend_from_slice(chunk));
        state.store.put(&hash, expected, Box::pin(teed), &ctx).await
    } else {
        state.store.put(&hash, expected, counted, &ctx).await
    };
    match outcome {
        Ok(PutOutcome::Created) => {
            if keep {
                let data = std::mem::take(&mut *tee.lock().unwrap_or_else(|p| p.into_inner()));
                state.memcache.insert(&hash, Bytes::from(data), ctx.clone());
            }
            state.stats.record_full(
                Kind::Put,
                &hash,
                expected.unwrap_or(0),
                start.elapsed(),
                200,
                Some(ctx.clone()),
                Some(ctx),
            );
            StatusCode::OK.into_response()
        }
        Ok(PutOutcome::AlreadyExists) => {
            state
                .stats
                .record_full(Kind::Conflict, &hash, 0, start.elapsed(), 409, Some(ctx), None);
            text(StatusCode::CONFLICT, "nx-remote-cache: artifact already exists")
        }
        Err(PutError::TooLarge) => {
            state.stats.record(Kind::Error, &hash, 0, start.elapsed(), 413);
            text(
                StatusCode::PAYLOAD_TOO_LARGE,
                format!("nx-remote-cache: artifact exceeds the {max} byte limit (NX_CACHE_MAX_ARTIFACT_BYTES)"),
            )
        }
        Err(PutError::BadBody(msg)) => {
            state.stats.record(Kind::Error, &hash, 0, start.elapsed(), 400);
            text(
                StatusCode::BAD_REQUEST,
                format!("nx-remote-cache: incomplete upload: {msg}"),
            )
        }
        Err(PutError::Store(e)) => {
            state.stats.record(Kind::Error, &hash, 0, start.elapsed(), 502);
            tracing::error!(hash, error = %e, "store write failed");
            text(
                StatusCode::BAD_GATEWAY,
                format!("nx-remote-cache: store write failed: {e}"),
            )
        }
    }
}

/// How much of a rejected upload we are willing to read so the client gets our answer.
const DRAIN_BUDGET: u64 = 64 << 20;
/// Largest upload copied into the memory tier on the way through.
const WRITE_THROUGH_MAX: u64 = 1 << 20;

async fn drain(body: Body, budget: u64) {
    let mut stream = body.into_data_stream();
    let mut seen = 0u64;
    while let Some(Ok(chunk)) = stream.next().await {
        seen += chunk.len() as u64;
        if seen > budget {
            break;
        }
    }
}

/// Pass body chunks through, failing fast once `max` bytes have been seen.
fn counted_stream(body: Body, max: u64) -> impl Stream<Item = Result<Bytes, PutError>> + Unpin {
    let inner = body.into_data_stream();
    Box::pin(futures_util::stream::try_unfold(
        (inner, 0u64),
        move |(mut inner, seen)| async move {
            match inner.next().await {
                None => Ok(None),
                Some(Err(e)) => Err(PutError::BadBody(e.to_string())),
                Some(Ok(chunk)) => {
                    let seen = seen + chunk.len() as u64;
                    if seen > max {
                        return Err(PutError::TooLarge);
                    }
                    Ok(Some((chunk, (inner, seen))))
                }
            }
        },
    ))
}

pub async fn healthz() -> &'static str {
    "ok"
}

/// Readiness: the store answers and we are not shutting down. The key-value store is reported
/// but never fails the probe: the cache API works without it, and taking every pod out of the
/// Service because Redis restarted would stop the builds it serves.
pub async fn readyz(State(state): State<AppState>) -> Response {
    if state.draining.load(std::sync::atomic::Ordering::Relaxed) {
        return text(StatusCode::SERVICE_UNAVAILABLE, "draining");
    }
    if let Err(e) = state.store.probe().await {
        tracing::warn!(error = %e, "readiness probe failed");
        return text(StatusCode::SERVICE_UNAVAILABLE, "store unavailable");
    }
    if let Err(e) = state.kv.probe().await {
        tracing::warn!(error = ?e, "key-value store (redis) unavailable; cache API still ready");
        return "ok (key-value store unavailable)".into_response();
    }
    "ok".into_response()
}

pub async fn metrics(State(state): State<AppState>) -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4; charset=utf-8")],
        state
            .stats
            .prometheus(VERSION, state.store.kind, &state.memcache.stats(), state.kv.stats()),
    )
        .into_response()
}

#[derive(Serialize)]
pub struct StatsResponse {
    version: &'static str,
    uptime_seconds: u64,
    started_at: String,
    store: StoreInfo,
    pod: String,
    counters: crate::stats::CountersSnapshot,
    latency_ms: Latencies,
    retention: Retention,
    memory_cache: crate::memcache::MemCacheStats,
    kv: crate::kv::KvInfo,
    ci: CiInfo,
    timeline: Vec<TimelinePoint>,
}

/// Which CI system the dashboard links to. Only GitHub (and GitHub Enterprise) so far.
#[derive(Serialize)]
struct CiInfo {
    provider: Option<&'static str>,
    url: String,
}

#[derive(Serialize)]
struct Retention {
    ttl_seconds: Option<u64>,
    max_total_bytes: Option<u64>,
    max_artifact_bytes: u64,
    sweep_interval_seconds: u64,
    deleted_total: u64,
    bytes_freed_total: u64,
    last_sweep: Option<crate::store::SweepReport>,
}

#[derive(Serialize)]
struct StoreInfo {
    kind: &'static str,
    location: String,
}

#[derive(Serialize)]
struct Latencies {
    get_hit: LatencySummary,
    get_miss: LatencySummary,
    put: LatencySummary,
}

pub async fn stats(State(state): State<AppState>) -> Json<StatsResponse> {
    let s = &state.stats;
    Json(StatsResponse {
        version: VERSION,
        uptime_seconds: s.uptime().as_secs(),
        started_at: s.started_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
        store: StoreInfo {
            kind: state.store.kind,
            location: state.store.location.clone(),
        },
        pod: state.config.pod_name.clone(),
        counters: s.counters(),
        latency_ms: Latencies {
            get_hit: s.get_hit.snapshot().summary(),
            get_miss: s.get_miss.snapshot().summary(),
            put: s.put.snapshot().summary(),
        },
        retention: Retention {
            ttl_seconds: state.config.ttl.map(|d| d.as_secs()),
            max_total_bytes: state.config.max_total_bytes,
            max_artifact_bytes: state.config.max_artifact_bytes,
            sweep_interval_seconds: state.config.sweep_interval.as_secs(),
            deleted_total: s.swept_objects.load(std::sync::atomic::Ordering::Relaxed),
            bytes_freed_total: s.swept_bytes.load(std::sync::atomic::Ordering::Relaxed),
            last_sweep: s.last_sweep(),
        },
        memory_cache: state.memcache.stats(),
        kv: crate::kv::info(&state.kv),
        ci: CiInfo {
            provider: state.config.github_enabled.then_some("github"),
            url: state.config.github_url.clone(),
        },
        timeline: s.timeline(),
    })
}

#[derive(Deserialize)]
pub struct EventsQuery {
    limit: Option<usize>,
}

#[derive(Serialize)]
pub struct EventsResponse {
    events: Vec<std::sync::Arc<Event>>,
}

pub async fn events(State(state): State<AppState>, Query(q): Query<EventsQuery>) -> Json<EventsResponse> {
    let limit = q.limit.unwrap_or(100).min(crate::stats::EVENT_RING);
    Json(EventsResponse {
        events: state.stats.recent_events(limit),
    })
}

pub async fn events_stream(State(state): State<AppState>) -> Sse<impl Stream<Item = Result<SseEvent, Infallible>>> {
    let rx = state.stats.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|item| async move {
        // A lagging subscriber just skips what it missed; the UI re-syncs from /events.
        let ev = item.ok()?;
        let data = serde_json::to_string(&*ev).ok()?;
        Some(Ok(SseEvent::default().data(data)))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Deserialize)]
pub struct TokenQuery {
    token: Option<String>,
}

/// Only route where the token may travel in the query string: `EventSource` cannot set headers.
/// Everywhere else a `?token=` would end up in proxy and browser logs for nothing.
const QUERY_TOKEN_PATH: &str = "/api/v1/events/stream";

/// The `Authorization` header value, or one built from `?token=` when allowed on this path.
pub(crate) fn presented_bearer(headers: &HeaderMap, query_token: Option<&str>) -> Option<String> {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .or_else(|| query_token.map(|t| format!("Bearer {t}")))
}

/// Gate for the dashboard and its API. Accepts `NX_CACHE_UI_TOKEN` when set, or any cache
/// token (read-write or read-only). Open when login is disabled, or when the cache runs
/// anonymous and no dashboard token is set.
pub async fn ui_guard(
    State(state): State<AppState>,
    Query(q): Query<TokenQuery>,
    req: Request,
    next: Next,
) -> Response {
    let ui_token = state.config.ui_token.as_deref();
    if !state.config.login_enabled || (ui_token.is_none() && state.config.allow_anonymous) {
        return next.run(req).await;
    }
    if let Some(name) = state.config.ui_trusted_header.as_deref() {
        if req.headers().get(name).is_some_and(|v| !v.is_empty()) {
            return next.run(req).await;
        }
    }
    let query_token = q.token.as_deref().filter(|_| req.uri().path() == QUERY_TOKEN_PATH);
    let presented = presented_bearer(req.headers(), query_token);
    let raw = presented
        .as_deref()
        .and_then(|h| h.split_once(char::is_whitespace))
        .map(|(_, t)| t.trim().to_owned());
    let ok = ui_token
        .zip(raw.as_deref())
        .is_some_and(|(e, p)| Tokens::matches_single(e, p))
        || (!state.config.allow_anonymous && state.tokens.authenticate(presented.as_deref()).is_ok());
    if ok {
        next.run(req).await
    } else {
        text(
            StatusCode::UNAUTHORIZED,
            "dashboard token required (NX_CACHE_UI_TOKEN or a cache token)",
        )
    }
}

#[cfg(test)]
mod tests {
    use super::valid_hash;

    #[test]
    fn hashes() {
        assert!(valid_hash("1234567890abcdef"));
        assert!(valid_hash("a-b_C"));
        assert!(!valid_hash(""));
        assert!(!valid_hash("../etc"));
        assert!(!valid_hash("a b"));
        assert!(!valid_hash(&"x".repeat(129)));
    }
}
