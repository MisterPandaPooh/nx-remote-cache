use std::future::IntoFuture;

use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

use nx_remote_cache::config::LogFormat;
use nx_remote_cache::store::CacheStore;
use nx_remote_cache::{install_crypto_provider, router, AppState, Config};

// musl's malloc serialises on a global lock and is slow under 32+ concurrent uploads;
// mimalloc keeps the static (scratch) image fast.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    install_crypto_provider();
    let config = Config::from_env()?;
    init_tracing(config.log_format, &config.log_level);

    let store = CacheStore::open(&config).await.context("opening the artifact store")?;
    if let Err(e) = store.probe().await {
        tracing::warn!(error = %e, "store is not reachable yet; /readyz will report it");
    }
    let state = AppState::new(config.clone(), store);
    let app = router(state.clone());
    state.spawn_sweeper();

    let listener = TcpListener::bind(config.bind)
        .await
        .with_context(|| format!("binding {}", config.bind))?;
    tracing::info!(
        version = nx_remote_cache::app::VERSION,
        bind = %config.bind,
        store = state.store.kind,
        location = %state.store.location,
        anonymous = config.allow_anonymous,
        ui = config.ui_enabled,
        ttl_secs = config.ttl.map(|d| d.as_secs()),
        max_total_bytes = config.max_total_bytes,
        max_artifact_bytes = config.max_artifact_bytes,
        "nx-remote-cache listening"
    );
    // On SIGTERM: readiness flips to 503, the listener closes, in-flight requests get
    // `shutdown_grace` to finish, then the process exits whatever is still open.
    let grace = config.shutdown_grace;
    let (signalled_tx, signalled_rx) = tokio::sync::oneshot::channel::<()>();
    let draining = state.clone();
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            shutdown_signal().await;
            draining.draining.store(true, std::sync::atomic::Ordering::Relaxed);
            let _ = signalled_tx.send(());
        })
        .into_future();
    tokio::pin!(server);
    tokio::select! {
        r = &mut server => r?,
        _ = async {
            let _ = signalled_rx.await;
            tokio::time::sleep(grace).await;
        } => tracing::warn!(grace_secs = grace.as_secs(), "shutdown grace elapsed, exiting with requests in flight"),
    }
    tracing::info!("shutdown complete");
    Ok(())
}

/// `NX_CACHE_LOG_LEVEL` sets the base level; `RUST_LOG` (full tracing directives such as
/// `info,access=debug,object_store=trace`) wins when it is set.
fn init_tracing(format: LogFormat, level: &str) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));
    let builder = tracing_subscriber::fmt().with_env_filter(filter).with_target(true);
    match format {
        LogFormat::Json => builder.json().flatten_event(true).init(),
        LogFormat::Pretty => builder.init(),
        LogFormat::Compact => builder.compact().init(),
    }
}

async fn shutdown_signal() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
            Ok(mut sig) => {
                sig.recv().await;
            }
            Err(_) => std::future::pending::<()>().await,
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
    tracing::info!("shutdown signal received, draining connections");
}
