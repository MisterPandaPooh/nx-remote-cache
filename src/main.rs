use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::EnvFilter;

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
    init_tracing(config.log_json);

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
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    tracing::info!("shutdown complete");
    Ok(())
}

fn init_tracing(json: bool) {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let builder = tracing_subscriber::fmt().with_env_filter(filter).with_target(false);
    if json {
        builder.json().flatten_event(true).init();
    } else {
        builder.init();
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
