use std::sync::Arc;
use std::time::Instant;

use axum::extract::{DefaultBodyLimit, Request, State};
use axum::middleware::{self, Next};
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::auth::Tokens;
use crate::config::Config;
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
}

impl AppState {
    pub fn new(config: Config, store: CacheStore) -> Self {
        let tokens = Tokens::new(&config.rw_tokens, &config.ro_tokens, config.allow_anonymous);
        let put_permits = tokio::sync::Semaphore::new(config.max_concurrent_puts);
        let memcache = crate::memcache::MemCache::new(config.memory_cache_bytes, config.memory_cache_max_item);
        Self(Arc::new(Inner {
            config,
            store,
            tokens,
            stats: Stats::new(),
            put_permits,
            memcache,
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
        .route_layer(middleware::from_fn_with_state(state.clone(), routes::ui_guard));

    let mut app = Router::new()
        .route("/v1/cache/{hash}", get(routes::get_artifact).put(routes::put_artifact))
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
        .layer(middleware::from_fn(access_log))
        .with_state(state)
}

/// One structured log line per request. Tokens are never logged.
async fn access_log(req: Request, next: Next) -> Response {
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let start = Instant::now();
    let res = next.run(req).await;
    let status = res.status().as_u16();
    let ms = start.elapsed().as_secs_f64() * 1000.0;
    if path.starts_with("/v1/cache/") {
        tracing::info!(target: "access", %method, %path, status, ms = format_args!("{ms:.2}"), "request");
    } else {
        tracing::debug!(target: "access", %method, %path, status, ms = format_args!("{ms:.2}"), "request");
    }
    res
}

#[allow(dead_code)]
fn _state_assertions(_: State<AppState>) {}
