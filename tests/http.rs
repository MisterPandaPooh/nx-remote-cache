//! End-to-end tests against the real router over TCP, using the in-memory store.

use std::net::SocketAddr;

use nx_remote_cache::store::CacheStore;
use nx_remote_cache::{router, AppState, Config};
use tokio::net::TcpListener;

struct Server {
    base: String,
    client: reqwest::Client,
}

async fn start(mut cfg: Config) -> Server {
    nx_remote_cache::install_crypto_provider();
    let _ = tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_test_writer()
        .try_init();
    cfg.pod_name = "test-pod".into();
    let store = CacheStore::open(&cfg).await.unwrap();
    let state = AppState::new(cfg, store);
    let app = router(state);
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Server {
        base: format!("http://{addr}"),
        client: reqwest::Client::new(),
    }
}

impl Server {
    fn url(&self, path: &str) -> String {
        format!("{}{}", self.base, path)
    }
    async fn put(&self, hash: &str, token: &str, body: Vec<u8>) -> reqwest::Response {
        let len = body.len();
        self.client
            .put(self.url(&format!("/v1/cache/{hash}")))
            .bearer_auth(token)
            .header("content-type", "application/octet-stream")
            .header("content-length", len)
            .body(body)
            .send()
            .await
            .unwrap()
    }
    async fn get(&self, hash: &str, token: &str) -> reqwest::Response {
        self.client
            .get(self.url(&format!("/v1/cache/{hash}")))
            .bearer_auth(token)
            .send()
            .await
            .unwrap()
    }
}

fn payload(n: usize) -> Vec<u8> {
    (0..n).map(|i| (i % 241) as u8).collect()
}

#[tokio::test]
async fn put_get_roundtrip_and_conflict() {
    let s = start(Config::for_tests("memory:///")).await;
    let data = payload(10_000);

    let res = s.put("abc123", "rw-token", data.clone()).await;
    assert_eq!(res.status(), 200);

    let res = s.get("abc123", "rw-token").await;
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-type"], "application/octet-stream");
    assert_eq!(res.headers()["content-length"], "10000");
    assert_eq!(res.bytes().await.unwrap().to_vec(), data);

    let res = s.put("abc123", "rw-token", data).await;
    assert_eq!(res.status(), 409);
}

#[tokio::test]
async fn miss_is_404() {
    let s = start(Config::for_tests("memory:///")).await;
    assert_eq!(s.get("nothere", "rw-token").await.status(), 404);
    assert_eq!(s.get("nothere", "ro-token").await.status(), 404);
}

#[tokio::test]
async fn auth_rules() {
    let s = start(Config::for_tests("memory:///")).await;
    let res = s.client.get(s.url("/v1/cache/h")).send().await.unwrap();
    assert_eq!(res.status(), 401);
    assert!(res
        .text()
        .await
        .unwrap()
        .contains("NX_SELF_HOSTED_REMOTE_CACHE_ACCESS_TOKEN"));
    assert_eq!(s.get("h", "wrong").await.status(), 401);

    assert_eq!(s.put("h", "ro-token", b"x".to_vec()).await.status(), 403);
    assert_eq!(s.put("h", "rw-token", b"x".to_vec()).await.status(), 200);
    assert_eq!(s.get("h", "ro-token").await.status(), 200);
}

#[tokio::test]
async fn anonymous_mode() {
    let mut cfg = Config::for_tests("memory:///");
    cfg.rw_tokens.clear();
    cfg.ro_tokens.clear();
    cfg.allow_anonymous = true;
    let s = start(cfg).await;
    let res = s
        .client
        .put(s.url("/v1/cache/anon"))
        .body(b"z".to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(
        s.client.get(s.url("/v1/cache/anon")).send().await.unwrap().status(),
        200
    );
}

#[tokio::test]
async fn invalid_hash_and_limits() {
    let mut cfg = Config::for_tests("memory:///");
    cfg.max_artifact_bytes = 1000;
    let s = start(cfg).await;
    assert_eq!(s.get("bad%20hash", "rw-token").await.status(), 400);
    assert_eq!(s.get("a.b", "rw-token").await.status(), 400);

    let res = s.put("toolarge", "rw-token", payload(1001)).await;
    assert_eq!(res.status(), 413);
    assert_eq!(s.get("toolarge", "rw-token").await.status(), 404);

    // Chunked upload over the limit is caught while streaming.
    let stream = futures_util::stream::iter((0..20).map(|_| Ok::<_, std::io::Error>(bytes::Bytes::from(payload(100)))));
    let res = s
        .client
        .put(s.url("/v1/cache/chunkedbig"))
        .bearer_auth("rw-token")
        .body(reqwest::Body::wrap_stream(stream))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 413);
}

#[tokio::test]
async fn large_streaming_upload_roundtrip() {
    let mut cfg = Config::for_tests("memory:///");
    cfg.inline_put_threshold = 1 << 20; // 12 MiB goes through the multipart path
    let s = start(cfg).await;
    let data = payload(12 << 20);
    assert_eq!(s.put("big", "rw-token", data.clone()).await.status(), 200);
    let res = s.get("big", "rw-token").await;
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-length"], (12 << 20).to_string());
    assert_eq!(res.bytes().await.unwrap().to_vec(), data);
    assert_eq!(s.put("big", "rw-token", data).await.status(), 409);
}

#[tokio::test]
async fn chunked_upload_without_content_length() {
    let s = start(Config::for_tests("memory:///")).await;
    let data = payload(3 << 20);
    let chunks: Vec<Result<bytes::Bytes, std::io::Error>> = data
        .chunks(64 << 10)
        .map(|c| Ok(bytes::Bytes::copy_from_slice(c)))
        .collect();
    let res = s
        .client
        .put(s.url("/v1/cache/chunked"))
        .bearer_auth("rw-token")
        .body(reqwest::Body::wrap_stream(futures_util::stream::iter(chunks)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(s.get("chunked", "rw-token").await.bytes().await.unwrap().to_vec(), data);
}

#[tokio::test]
async fn filesystem_store_end_to_end() {
    let dir = tempfile::tempdir().unwrap();
    let url = url::Url::from_directory_path(dir.path()).unwrap();
    let s = start(Config::for_tests(url.as_str())).await;
    assert_eq!(s.put("fs", "rw-token", payload(5000)).await.status(), 200);
    assert!(dir.path().join("fs").is_file());
    assert_eq!(s.get("fs", "rw-token").await.bytes().await.unwrap().len(), 5000);
    assert_eq!(s.client.get(s.url("/readyz")).send().await.unwrap().status(), 200);
}

#[tokio::test]
async fn ops_and_dashboard_endpoints() {
    let s = start(Config::for_tests("memory:///")).await;
    s.put("h1", "rw-token", payload(10)).await;
    s.get("h1", "rw-token").await;
    s.get("h2", "rw-token").await;
    s.put("h1", "rw-token", payload(10)).await;

    assert_eq!(
        s.client
            .get(s.url("/healthz"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "ok"
    );
    assert_eq!(s.client.get(s.url("/readyz")).send().await.unwrap().status(), 200);

    let metrics = s
        .client
        .get(s.url("/metrics"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(metrics.contains("nx_cache_requests_total{kind=\"hit\"} 1"));
    assert!(metrics.contains("nx_cache_requests_total{kind=\"miss\"} 1"));
    assert!(metrics.contains("nx_cache_requests_total{kind=\"conflict\"} 1"));
    assert!(metrics.contains("nx_cache_bytes_stored_total 10"));

    let stats: serde_json::Value = serde_json::from_str(
        &s.client
            .get(s.url("/api/v1/stats"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stats["counters"]["hits"], 1);
    assert_eq!(stats["counters"]["puts"], 1);
    assert_eq!(stats["store"]["kind"], "memory");
    assert_eq!(stats["pod"], "test-pod");
    assert_eq!(stats["timeline"].as_array().unwrap().len(), 288);
    assert!(stats["latency_ms"]["get_hit"]["count"] == 1);

    let events: serde_json::Value = serde_json::from_str(
        &s.client
            .get(s.url("/api/v1/events?limit=2"))
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    )
    .unwrap();
    let events = events["events"].as_array().unwrap();
    assert_eq!(events.len(), 2);
    assert_eq!(events[0]["kind"], "conflict");
    assert_eq!(events[1]["kind"], "miss");

    // The UI fallback answers something for `/`.
    let home = s.client.get(s.url("/")).send().await.unwrap();
    assert_eq!(home.status(), 200);
    assert!(home.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/html"));
}

#[tokio::test]
async fn sse_stream_delivers_live_events() {
    use futures_util::StreamExt;
    let s = start(Config::for_tests("memory:///")).await;
    let res = s.client.get(s.url("/api/v1/events/stream")).send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert!(res.headers()["content-type"]
        .to_str()
        .unwrap()
        .starts_with("text/event-stream"));
    let mut body = res.bytes_stream();
    s.get("live1", "rw-token").await;
    let mut collected = String::new();
    while let Some(chunk) = tokio::time::timeout(std::time::Duration::from_secs(5), body.next())
        .await
        .expect("event in time")
    {
        collected.push_str(std::str::from_utf8(&chunk.unwrap()).unwrap());
        if collected.contains("\"hash\":\"live1\"") {
            break;
        }
    }
    assert!(collected.contains("\"kind\":\"miss\""));
}

#[tokio::test]
async fn ui_token_guards_dashboard_but_not_cache_api() {
    let mut cfg = Config::for_tests("memory:///");
    cfg.ui_token = Some("ui-secret".into());
    let s = start(cfg).await;
    assert_eq!(s.client.get(s.url("/api/v1/stats")).send().await.unwrap().status(), 401);
    assert_eq!(
        s.client
            .get(s.url("/api/v1/stats?token=ui-secret"))
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        s.client
            .get(s.url("/api/v1/stats"))
            .bearer_auth("ui-secret")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(s.client.get(s.url("/")).send().await.unwrap().status(), 401);
    assert_eq!(s.get("x", "rw-token").await.status(), 404);
    assert_eq!(s.client.get(s.url("/metrics")).send().await.unwrap().status(), 200);
}
