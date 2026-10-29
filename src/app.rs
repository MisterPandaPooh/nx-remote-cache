use std::sync::Arc;
use std::time::Instant;

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::auth::Tokens;
use crate::config::Config;
use crate::kv;
use crate::routes;
use crate::stats::Stats;
use crate::store::CacheStore;
use crate::ui;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Clone)]
pub struct AppState(pub Arc<Inner>);

pub struct Inner {
    pub config: Config,
    pub store: CacheStore,
    pub tokens: Tokens,
    pub stats: Stats,
    /// Bounds uploads in flight, and with them the server's memory.
    pub put_permits: tokio::sync::Semaphore,
    pub memcache: crate::memcache::MemCache,
    pub kv: crate::kv::KvStore,
}

impl AppState {
    pub fn new(config: Config, store: CacheStore) -> Self {
        let tokens = Tokens::new(
            &config.rw_tokens,
            &config.ro_tokens,
            config.allow_anonymous || !config.login_enabled,
        );
        let put_permits = tokio::sync::Semaphore::new(config.max_concurrent_puts);
        let memcache = crate::memcache::MemCache::new(config.memory_cache_bytes, config.memory_cache_max_item);
        let kv_budget = if config.kv_enabled { config.kv_bytes } else { 0 };
        let kv = match config.kv_backend {
            crate::config::KvBackend::Memory => crate::kv::KvStore::new(kv_budget, config.kv_max_value),
            crate::config::KvBackend::Redis => {
                let url = config
                    .kv_redis_url
                    .as_deref()
                    .expect("checked when the config was read");
                crate::kv::KvStore::with_redis(
                    kv_budget,
                    config.kv_max_value,
                    url,
                    config.kv_redis_password.as_deref(),
                    &config.kv_redis_prefix,
                )
                .expect("NX_CACHE_KV_REDIS_URL is not a valid redis:// or rediss:// URL")
            }
        };
        Self(Arc::new(Inner {
            config,
            store,
            tokens,
            stats: Stats::new(),
            put_permits,
            memcache,
            kv,
        }))
    }

    /// Run one retention pass now (TTL and size cap from the config).
    pub async fn sweep_once(&self) {
        let cfg = &self.config;
        if cfg.ttl.is_none() && cfg.max_total_bytes.is_none() {
            return;
        }
        match self.store.sweep(cfg.ttl, cfg.max_total_bytes).await {
            Ok(report) => {
                tracing::info!(
                    scanned = report.scanned,
                    deleted = report.deleted,
                    bytes_freed = report.bytes_freed,
                    total_bytes = report.total_bytes,
                    duration_ms = report.duration_ms,
                    "retention sweep done"
                );
                self.stats.record_sweep(report);
            }
            Err(e) => tracing::error!(error = %e, "retention sweep failed"),
        }
    }

    /// Background loop: a first pass shortly after boot, then every `sweep_interval`.
    pub fn spawn_sweeper(&self) {
        let cfg = &self.config;
        if cfg.ttl.is_none() && cfg.max_total_bytes.is_none() {
            return;
        }
        let state = self.clone();
        let interval = cfg.sweep_interval;
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(30)).await;
            loop {
                state.sweep_once().await;
                tokio::time::sleep(interval).await;
            }
        });
    }
}

impl std::ops::Deref for AppState {
    type Target = Inner;
    fn deref(&self) -> &Inner {
        &self.0
    }
}

pub fn router(state: AppState) -> Router {
    let api = Router::new()
        .route("/api/v1/stats", get(routes::stats))
        .route("/api/v1/events", get(routes::events))
        .route("/api/v1/events/stream", get(routes::events_stream))
        .route("/api/v1/kv", get(kv::dashboard_list))
        .route("/api/v1/kv/value", get(kv::dashboard_value))
        .route_layer(middleware::from_fn_with_state(state.clone(), routes::ui_guard));

    let mut app = Router::new()
        .route("/v1/cache/{hash}", get(routes::get_artifact).put(routes::put_artifact))
        .route("/v1/kv", get(kv::list_kv))
        .route("/v1/kv/{key}", get(kv::get_kv).put(kv::put_kv).delete(kv::delete_kv))
        .route("/healthz", get(routes::healthz))
        .route("/readyz", get(routes::readyz))
        .route("/metrics", get(routes::metrics))
        .merge(api);

    // The dashboard shell and its hashed assets are public; everything it shows comes from
    // /api/v1/*, which is behind the guard. The shell asks for a token when it gets a 401.
    if state.config.ui_enabled {
        app = app.fallback(get(ui::serve));
    }

    app.layer(DefaultBodyLimit::disable())
        .layer(middleware::from_fn_with_state(state.clone(), access_log))
        .with_state(state)
}

/// One structured log line per request: `info` with `NX_CACHE_ACCESS_LOG=true`, otherwise
/// `debug` (cache traffic can reach tens of thousands of requests per second; the event feed
/// and metrics are the production view). Failures are always `warn`.
async fn access_log(State(state): State<AppState>, req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let length = req
        .headers()
        .get(axum::http::header::CONTENT_LENGTH)
        .and_then(|v| v.to_str().ok()?.parse::<u64>().ok());
    let start = Instant::now();
    let mut res = next.run(req).await;
    res.headers_mut().insert(
        axum::http::header::X_CONTENT_TYPE_OPTIONS,
        axum::http::HeaderValue::from_static("nosniff"),
    );
    let status = res.status().as_u16();
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    if status >= 500 {
        tracing::warn!(target: "access", %method, %path, status, length, ms = format_args!("{ms:.2}"), "request failed");
    } else if state.config.access_log {
        tracing::info!(target: "access", %method, %path, status, length, ms = format_args!("{ms:.2}"), "request");
    } else {
        tracing::debug!(target: "access", %method, %path, status, length, ms = format_args!("{ms:.2}"), "request");
    }
    res
}

#[allow(dead_code)]
fn _state_assertions(_: State<AppState>) {}
