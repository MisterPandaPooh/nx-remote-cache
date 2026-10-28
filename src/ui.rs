//! Serves the embedded dashboard built from `ui/` (see `ui/README.md`).

use axum::body::Body;
use axum::http::{header, StatusCode, Uri};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

#[derive(RustEmbed)]
#[folder = "ui/dist/"]
struct Assets;

const NOT_BUILT: &str = r#"<!doctype html><meta charset="utf-8"><title>nx-remote-cache</title>
<body style="font:15px system-ui;background:#0f172a;color:#e2e8f0;padding:48px;max-width:640px">
<h1 style="font-weight:600">nx-remote-cache is running</h1>
<p>The dashboard was not bundled into this binary. Build it with <code>cd ui && pnpm install && pnpm build</code>
before <code>cargo build --release</code>, or use the Docker image.</p>
<p>API: <a style="color:#93c5fd" href="/api/v1/stats">/api/v1/stats</a> · <a style="color:#93c5fd" href="/metrics">/metrics</a> · <a style="color:#93c5fd" href="/healthz">/healthz</a></p>"#;

pub async fn serve(uri: Uri) -> Response {
    let path = uri.path().trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    match Assets::get(path) {
        Some(file) => asset_response(path, file.data.into_owned()),
        None if path.contains('.') => StatusCode::NOT_FOUND.into_response(),
        None => match Assets::get("index.html") {
            Some(index) => asset_response("index.html", index.data.into_owned()),
            None => ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], NOT_BUILT).into_response(),
        },
    }
}

fn asset_response(path: &str, data: Vec<u8>) -> Response {
    let mime = mime_guess::from_path(path).first_or_octet_stream();
    // Vite hashes everything under assets/, so those can be cached forever.
    let cache = if path.starts_with("assets/") {
        "public, max-age=31536000, immutable"
    } else {
        "no-cache"
    };
    (
        [
            (header::CONTENT_TYPE, mime.as_ref().to_string()),
            (header::CACHE_CONTROL, cache.to_string()),
        ],
        Body::from(data),
    )
        .into_response()
}
