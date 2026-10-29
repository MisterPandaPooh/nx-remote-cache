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
    if cfg.pod_name == "test" {
        cfg.pod_name = "test-pod".into();
    }
    let store = CacheStore::open(&cfg).await.unwrap();
    let state = AppState::new(cfg, store);
    state.spawn_shared_stats();
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

    // Above the gather limit the body is coalesced into 1 MiB blocks: a size that is not a
    // multiple of the block must still round-trip byte for byte, and a second PUT is a 409.
    let big = payload((3 << 20) + 12_345);
    assert_eq!(s.put("fs-big", "rw-token", big.clone()).await.status(), 200);
    assert_eq!(s.get("fs-big", "rw-token").await.bytes().await.unwrap().to_vec(), big);
    assert_eq!(s.put("fs-big", "rw-token", big.clone()).await.status(), 409);
    assert!(std::fs::read_dir(dir.path()).unwrap().all(|e| !e
        .unwrap()
        .file_name()
        .to_string_lossy()
        .ends_with(".tmp")));

    // Chunked (no Content-Length) large upload takes the same streaming path.
    let chunks: Vec<Result<bytes::Bytes, std::io::Error>> = big
        .chunks(100_000)
        .map(|c| Ok(bytes::Bytes::copy_from_slice(c)))
        .collect();
    let res = s
        .client
        .put(s.url("/v1/cache/fs-chunked"))
        .bearer_auth("rw-token")
        .body(reqwest::Body::wrap_stream(futures_util::stream::iter(chunks)))
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(
        s.get("fs-chunked", "rw-token").await.bytes().await.unwrap().to_vec(),
        big
    );
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
            .bearer_auth("rw-token")
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
            .bearer_auth("rw-token")
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

    // The UI shell is public; only its API is guarded.
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
    let res = s
        .client
        .get(s.url("/api/v1/events/stream"))
        .bearer_auth("rw-token")
        .send()
        .await
        .unwrap();
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
    // `?token=` only works on the SSE stream (EventSource cannot set headers); anywhere else
    // it would only end up in proxy logs.
    assert_eq!(
        s.client
            .get(s.url("/api/v1/stats?token=ui-secret"))
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(
        s.client
            .get(s.url("/api/v1/events/stream?token=ui-secret"))
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
    assert_eq!(s.client.get(s.url("/")).send().await.unwrap().status(), 200);
    // Cache tokens also open the dashboard API; random ones do not.
    assert_eq!(
        s.client
            .get(s.url("/api/v1/stats"))
            .bearer_auth("ro-token")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        s.client
            .get(s.url("/api/v1/stats"))
            .bearer_auth("nope")
            .send()
            .await
            .unwrap()
            .status(),
        401
    );
    assert_eq!(s.get("x", "rw-token").await.status(), 404);
    assert_eq!(s.client.get(s.url("/metrics")).send().await.unwrap().status(), 200);
}

#[tokio::test]
async fn anonymous_cache_means_open_dashboard() {
    let mut cfg = Config::for_tests("memory:///");
    cfg.rw_tokens.clear();
    cfg.ro_tokens.clear();
    cfg.allow_anonymous = true;
    let s = start(cfg).await;
    assert_eq!(s.client.get(s.url("/api/v1/stats")).send().await.unwrap().status(), 200);
}

#[tokio::test]
async fn login_disabled_works_without_any_token() {
    // NX_CACHE_ENABLE_LOGIN=false: no token for the cache, no login screen, even with a UI token set.
    let mut cfg = Config::for_tests("memory:///");
    cfg.login_enabled = false;
    cfg.ui_token = Some("ignored".into());
    let s = start(cfg).await;
    let put = s
        .client
        .put(s.url("/v1/cache/open1"))
        .body(b"z".to_vec())
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);
    assert_eq!(
        s.client.get(s.url("/v1/cache/open1")).send().await.unwrap().status(),
        200
    );
    assert_eq!(s.client.get(s.url("/api/v1/stats")).send().await.unwrap().status(), 200);
    assert_eq!(s.client.get(s.url("/")).send().await.unwrap().status(), 200);
}

#[tokio::test]
async fn retention_sweep_is_reported_in_stats_and_metrics() {
    let mut cfg = Config::for_tests("memory:///");
    cfg.max_total_bytes = Some(2500);
    let s = start(cfg.clone()).await;
    for i in 0..5 {
        assert_eq!(s.put(&format!("r{i}"), "rw-token", payload(1000)).await.status(), 200);
        tokio::time::sleep(std::time::Duration::from_millis(3)).await;
    }
    // Drive a sweep through a second state sharing nothing but the config shape: build it directly.
    let store = CacheStore::open(&cfg).await.unwrap();
    let state = AppState::new(cfg, store);
    state.sweep_once().await; // empty memory store: nothing to do, must not panic
    let stats: serde_json::Value = serde_json::from_str(
        &s.client
            .get(s.url("/api/v1/stats"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stats["retention"]["max_total_bytes"], 2500);
    assert!(stats["retention"]["ttl_seconds"].is_null());
    let metrics = s
        .client
        .get(s.url("/metrics"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(metrics.contains("nx_cache_retention_deleted_total 0"));
}

#[tokio::test]
async fn trusted_proxy_header_opens_dashboard_only() {
    let mut cfg = Config::for_tests("memory:///");
    cfg.ui_trusted_header = Some("x-auth-request-email".into());
    let s = start(cfg).await;
    assert_eq!(s.client.get(s.url("/api/v1/stats")).send().await.unwrap().status(), 401);
    let res = s
        .client
        .get(s.url("/api/v1/stats"))
        .header("x-auth-request-email", "dev@example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 200);
    // The cache API still wants its own token.
    let res = s
        .client
        .get(s.url("/v1/cache/abc"))
        .header("x-auth-request-email", "dev@example.com")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 401);
}

#[tokio::test]
async fn memory_tier_serves_small_hot_artifacts() {
    let s = start(Config::for_tests("memory:///")).await;
    assert_eq!(s.put("hot", "rw-token", payload(2000)).await.status(), 200);
    // Write-through: the first read already comes from memory.
    let res = s.get("hot", "rw-token").await;
    assert_eq!(res.status(), 200);
    assert_eq!(
        res.headers().get("x-cache-tier").map(|v| v.to_str().unwrap()),
        Some("memory"),
        "{:?}",
        res.headers()
    );
    assert_eq!(res.bytes().await.unwrap().len(), 2000);

    // Something written past the memory limit is read from the store, then kept if small enough.
    assert_eq!(s.put("warm", "rw-token", payload(3000)).await.status(), 200);
    let stats: serde_json::Value = serde_json::from_str(
        &s.client
            .get(s.url("/api/v1/stats"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stats["memory_cache"]["enabled"], true);
    assert!(stats["memory_cache"]["items"].as_u64().unwrap() >= 2);
    let metrics = s
        .client
        .get(s.url("/metrics"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(metrics.contains("nx_cache_memory_tier_hits_total 1"));
}

#[tokio::test]
async fn token_context_links_artifacts_to_ci_runs() {
    let s = start(Config::for_tests("memory:///")).await;
    // The producer appends its CI run to the token; the secret part still has to match.
    assert_eq!(
        s.put("ctx1", "rw-token;run=1001;repo=org/app;actor=alice", payload(500))
            .await
            .status(),
        200
    );
    assert_eq!(s.put("ctx2", "nope;run=1001", payload(500)).await.status(), 401);
    // A different run reads it: the event carries both the reader and the producer.
    assert_eq!(s.get("ctx1", "ro-token;run=2002;repo=org/app").await.status(), 200);
    let events: serde_json::Value = serde_json::from_str(
        &s.client
            .get(s.url("/api/v1/events?limit=5"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    )
    .unwrap();
    let hit = &events["events"][0];
    assert_eq!(hit["kind"], "hit");
    assert_eq!(hit["ctx"]["run"], "2002");
    assert_eq!(hit["origin"]["run"], "1001");
    assert_eq!(hit["origin"]["actor"], "alice");
    let put = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "put")
        .unwrap();
    assert_eq!(put["ctx"]["run"], "1001");
    // No context: the fields are simply absent.
    s.get("none", "rw-token").await;
    let events: serde_json::Value = serde_json::from_str(
        &s.client
            .get(s.url("/api/v1/events?limit=1"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(events["events"][0].get("ctx").is_none());
}

#[tokio::test]
async fn kv_store_roundtrip_types_ttl_and_auth() {
    let s = start(Config::for_tests("memory:///")).await;
    let kv = |k: &str| s.url(&format!("/v1/kv/{k}"));

    // JSON keeps its content type and is validated.
    let res = s
        .client
        .put(kv("flags"))
        .bearer_auth("rw-token")
        .header("content-type", "application/json")
        .body(r#"{"canary":true,"ratio":0.25}"#)
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
    let res = s.client.get(kv("flags")).bearer_auth("ro-token").send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert_eq!(res.headers()["content-type"], "application/json");
    let body: serde_json::Value = serde_json::from_slice(&res.bytes().await.unwrap()).unwrap();
    assert_eq!(body["ratio"], 0.25);
    let res = s
        .client
        .put(kv("bad"))
        .bearer_auth("rw-token")
        .header("content-type", "application/json")
        .body("{not json")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    // Strings and numbers are stored verbatim with their content type.
    for (key, body) in [("last-green", "a1b2c3d"), ("build-number", "4242")] {
        let res = s
            .client
            .put(kv(key))
            .bearer_auth("rw-token")
            .header("content-type", "text/plain")
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(res.status(), 204);
        let res = s.client.get(kv(key)).bearer_auth("rw-token").send().await.unwrap();
        assert!(res.headers()["content-type"]
            .to_str()
            .unwrap()
            .starts_with("text/plain"));
        assert_eq!(res.text().await.unwrap(), body);
    }

    // Listing, prefix filter, read-only token cannot write or delete.
    let res = s
        .client
        .get(s.url("/v1/kv?prefix=la"))
        .bearer_auth("ro-token")
        .send()
        .await
        .unwrap();
    let list: serde_json::Value = serde_json::from_slice(&res.bytes().await.unwrap()).unwrap();
    assert_eq!(list["keys"].as_array().unwrap().len(), 1);
    assert_eq!(list["keys"][0]["key"], "last-green");
    assert_eq!(
        s.client
            .put(kv("x"))
            .bearer_auth("ro-token")
            .body("1")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        s.client
            .delete(kv("flags"))
            .bearer_auth("ro-token")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(s.client.get(kv("flags")).send().await.unwrap().status(), 401);

    // TTL: present, then gone; delete answers 204 then 404.
    let res = s
        .client
        .put(s.url("/v1/kv/short?ttl=1s"))
        .bearer_auth("rw-token")
        .body("soon")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
    let res = s.client.get(kv("short")).bearer_auth("rw-token").send().await.unwrap();
    assert_eq!(res.status(), 200);
    assert!(res.headers().contains_key("x-kv-expires-in"));
    tokio::time::sleep(std::time::Duration::from_millis(1100)).await;
    assert_eq!(
        s.client
            .get(kv("short"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        s.client
            .delete(kv("flags"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        s.client
            .delete(kv("flags"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );

    // Values above NX_CACHE_KV_MAX_VALUE (64 KiB in tests) are refused; bad keys and TTLs too.
    let big = vec![b'x'; (64 << 10) + 1];
    assert_eq!(
        s.client
            .put(kv("big"))
            .bearer_auth("rw-token")
            .body(big)
            .send()
            .await
            .unwrap()
            .status(),
        413
    );
    assert_eq!(
        s.client
            .put(kv("a%2Fb"))
            .bearer_auth("rw-token")
            .body("1")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    let res = s
        .client
        .put(s.url("/v1/kv/k?ttl=1y"))
        .bearer_auth("rw-token")
        .body("1")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 400);

    // Metrics expose the store size.
    let metrics = s
        .client
        .get(s.url("/metrics"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(metrics.contains("nx_cache_kv_items "));
}

#[tokio::test]
async fn kv_scopes_secrets_and_dashboard() {
    let s = start(Config::for_tests("memory:///")).await;
    let ctx = "rw-token;run=42;repo=org/app";
    let put = |path: &str, body: &'static str| {
        s.client
            .put(s.url(path))
            .bearer_auth(ctx)
            .header("content-type", "text/plain")
            .body(body)
    };
    // The same key in three scopes; pipeline and run come from the token context.
    assert_eq!(put("/v1/kv/k", "g").send().await.unwrap().status(), 204);
    assert_eq!(put("/v1/kv/k?level=pipeline", "p").send().await.unwrap().status(), 204);
    assert_eq!(
        put("/v1/kv/k?level=run&secret=true", "r")
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    let get = |path: &str, token: &'static str| s.client.get(s.url(path)).bearer_auth(token);
    assert_eq!(get("/v1/kv/k", ctx).send().await.unwrap().text().await.unwrap(), "g");
    assert_eq!(
        get("/v1/kv/k?level=pipeline", ctx)
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
        "p"
    );
    let run = get("/v1/kv/k?level=run", ctx).send().await.unwrap();
    assert_eq!(run.headers().get("x-kv-secret").unwrap(), "true");
    assert_eq!(run.text().await.unwrap(), "r");
    // A secret is only served to the read-write token; the read-only one (pull requests)
    // gets 403, and still reads plain values.
    assert_eq!(
        get("/v1/kv/k?level=run&run=42", "ro-token")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        get("/v1/kv/k?level=pipeline&pipeline=org/app", "ro-token")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    // Another run or repo does not see them; explicit identifiers override the context.
    assert_eq!(
        get("/v1/kv/k?level=run", "rw-token;run=43")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        get("/v1/kv/k?level=run&run=42", "rw-token")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        get("/v1/kv/k?level=pipeline", "rw-token")
            .send()
            .await
            .unwrap()
            .status(),
        400
    );
    assert_eq!(get("/v1/kv/k?level=team", ctx).send().await.unwrap().status(), 400);
    // Scoped listing.
    let list: serde_json::Value = serde_json::from_slice(
        &get("/v1/kv?level=pipeline", ctx)
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(list["scope"], "org/app");
    assert_eq!(list["keys"].as_array().unwrap().len(), 1);
    // Dashboard: every scope with metadata, secrets flagged, values on demand.
    let all: serde_json::Value = serde_json::from_slice(
        &get("/api/v1/kv", "rw-token")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(all["enabled"], true);
    assert_eq!(all["items"], 3);
    let keys = all["keys"].as_array().unwrap();
    assert_eq!(keys.len(), 3);
    assert!(keys
        .iter()
        .any(|k| k["level"] == "run" && k["scope"] == "42" && k["secret"] == true));
    let value = get("/api/v1/kv/value?level=run&scope=42&key=k", "rw-token")
        .send()
        .await
        .unwrap();
    assert_eq!(value.status(), 200);
    assert_eq!(value.text().await.unwrap(), "r");
    // The reveal button needs the read-write token for secrets; plain values stay readable.
    assert_eq!(
        get("/api/v1/kv/value?level=run&scope=42&key=k", "ro-token")
            .send()
            .await
            .unwrap()
            .status(),
        403
    );
    assert_eq!(
        get("/api/v1/kv/value?level=global&key=k", "ro-token")
            .send()
            .await
            .unwrap()
            .status(),
        200
    );
    assert_eq!(
        get("/api/v1/kv/value?level=global&key=nope", "rw-token")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(s.client.get(s.url("/api/v1/kv")).send().await.unwrap().status(), 401);
    // Stats expose the store and the CI integration.
    let stats: serde_json::Value = serde_json::from_slice(
        &get("/api/v1/stats", "rw-token")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stats["kv"]["enabled"], true);
    assert_eq!(stats["kv"]["items"], 3);
    assert_eq!(stats["ci"]["provider"], "github");
    assert_eq!(stats["ci"]["url"], "https://github.com");
}

#[tokio::test]
async fn kv_can_be_switched_off_and_github_links_too() {
    let mut cfg = Config::for_tests("memory:///");
    cfg.kv_enabled = false;
    cfg.github_enabled = false;
    cfg.github_url = "https://ghe.example.com".into();
    let s = start(cfg).await;
    assert_eq!(
        s.client
            .put(s.url("/v1/kv/k"))
            .bearer_auth("rw-token")
            .body("x")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
    let all: serde_json::Value = serde_json::from_slice(
        &s.client
            .get(s.url("/api/v1/kv"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(all["enabled"], false);
    let stats: serde_json::Value = serde_json::from_slice(
        &s.client
            .get(s.url("/api/v1/stats"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(stats["ci"]["provider"].is_null());
    assert_eq!(stats["ci"]["url"], "https://ghe.example.com");
}

#[tokio::test]
async fn stats_are_shared_between_two_servers_over_redis() {
    // Two pods, one Redis: an upload on A and a hit and a miss on B all show in A's dashboard,
    // B lists A's event, both pods are listed, and only one pod gets the sweep lock. Needs
    // NX_CACHE_TEST_REDIS_URL (skipped otherwise).
    let Ok(url) = std::env::var("NX_CACHE_TEST_REDIS_URL") else {
        eprintln!("NX_CACHE_TEST_REDIS_URL not set, skipping");
        return;
    };
    let mut cfg = Config::for_tests("memory:///");
    cfg.stats_backend = nx_remote_cache::config::StatsBackend::Redis;
    cfg.kv_redis_url = Some(url);
    cfg.stats_redis_prefix = format!("nxrc-http-test-stats:{}", std::process::id());
    cfg.stats_flush_interval = std::time::Duration::from_millis(50);
    cfg.pod_name = "pod-a".into();
    let a = start(cfg.clone()).await;
    cfg.pod_name = "pod-b".into();
    let b = start(cfg).await;
    // Both pods share one memory:/// URL but not one store, so A's artifact is a miss on B.
    assert_eq!(a.put("c0ffee", "rw-token", payload(100)).await.status(), 200);
    assert_eq!(a.get("c0ffee", "rw-token").await.status(), 200);
    assert_eq!(b.get("c0ffee", "rw-token").await.status(), 404);
    tokio::time::sleep(std::time::Duration::from_millis(400)).await;

    let stats: serde_json::Value = serde_json::from_slice(
        &b.client
            .get(b.url("/api/v1/stats"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(stats["stats_backend"], "redis");
    assert_eq!(stats["pod"], "pod-b");
    assert_eq!(
        (
            stats["counters"]["puts"].as_u64(),
            stats["counters"]["hits"].as_u64(),
            stats["counters"]["misses"].as_u64()
        ),
        (Some(1), Some(1), Some(1))
    );
    assert_eq!(stats["counters"]["bytes_stored"], 100);
    assert_eq!(stats["latency_ms"]["put"]["count"], 1);
    let pods: Vec<&str> = stats["pods"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["name"].as_str().unwrap())
        .collect();
    assert_eq!(pods, ["pod-a", "pod-b"]);
    let last = stats["timeline"].as_array().unwrap().last().unwrap();
    assert_eq!(
        (last["puts"].as_u64(), last["hits"].as_u64(), last["misses"].as_u64()),
        (Some(1), Some(1), Some(1))
    );

    let events: serde_json::Value = serde_json::from_slice(
        &b.client
            .get(b.url("/api/v1/events?limit=10"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    let kinds: Vec<&str> = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["kind"].as_str().unwrap())
        .collect();
    assert_eq!(kinds, ["miss", "hit", "put"]);
    assert_eq!(events["events"][2]["hash"], "c0ffee");
}

#[tokio::test]
async fn kv_redis_backend_is_shared_between_two_servers() {
    // Two pods over one Redis: a key written on one is read on the other. Needs
    // NX_CACHE_TEST_REDIS_URL (skipped otherwise).
    let Ok(url) = std::env::var("NX_CACHE_TEST_REDIS_URL") else {
        eprintln!("NX_CACHE_TEST_REDIS_URL not set, skipping");
        return;
    };
    let mut cfg = Config::for_tests("memory:///");
    cfg.kv_backend = nx_remote_cache::config::KvBackend::Redis;
    cfg.kv_redis_url = Some(url);
    cfg.kv_redis_prefix = format!("nxrc-http-test:{}", std::process::id());
    let a = start(cfg.clone()).await;
    let b = start(cfg).await;
    assert_eq!(a.client.get(a.url("/readyz")).send().await.unwrap().status(), 200);
    let ctx = "rw-token;run=42;repo=org/app";
    let res = a
        .client
        .put(a.url("/v1/kv/token?level=run&secret=true&ttl=1h"))
        .bearer_auth(ctx)
        .header("content-type", "text/plain")
        .body("s3cr3t")
        .send()
        .await
        .unwrap();
    assert_eq!(res.status(), 204);
    let seen = b
        .client
        .get(b.url("/v1/kv/token?level=run"))
        .bearer_auth(ctx)
        .send()
        .await
        .unwrap();
    assert_eq!(seen.status(), 200);
    assert_eq!(seen.headers().get("x-kv-secret").unwrap(), "true");
    assert!(seen.headers().get("x-kv-expires-in").is_some());
    assert_eq!(seen.text().await.unwrap(), "s3cr3t");
    let all: serde_json::Value = serde_json::from_slice(
        &b.client
            .get(b.url("/api/v1/kv"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(all["backend"], "redis");
    assert_eq!(all["keys"].as_array().unwrap().len(), 1);
    assert_eq!(all["keys"][0]["scope"], "42");
    assert_eq!(
        b.client
            .delete(b.url("/v1/kv/token?level=run"))
            .bearer_auth(ctx)
            .send()
            .await
            .unwrap()
            .status(),
        204
    );
    assert_eq!(
        a.client
            .get(a.url("/v1/kv/token?level=run"))
            .bearer_auth(ctx)
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}

#[tokio::test]
async fn streamed_upload_to_s3_keeps_origin_and_create_only() {
    // Needs an S3-compatible store: NX_CACHE_TEST_S3_URL=s3://bucket/ plus the AWS_* variables
    // (endpoint, keys, region, AWS_ALLOW_HTTP). Skipped otherwise.
    let Ok(url) = std::env::var("NX_CACHE_TEST_S3_URL") else {
        eprintln!("NX_CACHE_TEST_S3_URL not set, skipping");
        return;
    };
    let mut cfg = Config::for_tests(&format!(
        "{}t-{}/",
        url.trim_end_matches('/').to_owned() + "/",
        std::process::id()
    ));
    cfg.inline_put_threshold = 64 << 10; // 64 KiB buffered, above that a presigned streaming PUT
    cfg.stream_put_threshold = 64 << 20;
    cfg.memory_cache_bytes = 0;
    let s = start(cfg).await;
    let ctx = "rw-token;run=4242;repo=org/app";
    let body: Vec<u8> = (0..(3 << 20) + 777).map(|i| (i % 251) as u8).collect();
    let put = s
        .client
        .put(s.url("/v1/cache/9100000000000000000001"))
        .bearer_auth(ctx)
        .header("content-type", "application/octet-stream")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200, "{}", put.text().await.unwrap_or_default());
    let got = s
        .client
        .get(s.url("/v1/cache/9100000000000000000001"))
        .bearer_auth("ro-token")
        .send()
        .await
        .unwrap();
    assert_eq!(got.status(), 200);
    assert_eq!(got.headers().get("x-cache-tier").unwrap(), "store");
    assert_eq!(got.bytes().await.unwrap().as_ref(), body.as_slice());
    // The upload context travelled as signed metadata headers: the hit knows its origin run.
    let events: serde_json::Value = serde_json::from_slice(
        &s.client
            .get(s.url("/api/v1/events?limit=5"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
    )
    .unwrap();
    let hit = events["events"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["kind"] == "hit")
        .expect("a hit event");
    assert_eq!(hit["origin"]["run"], "4242");
    // Create-only: a second upload of the same hash is a 409 (S3 honours If-None-Match: *; a mock
    // that ignores it answers 200, which the Nx client treats the same way).
    let again = s
        .client
        .put(s.url("/v1/cache/9100000000000000000001"))
        .bearer_auth(ctx)
        .header("content-type", "application/octet-stream")
        .body(body.clone())
        .send()
        .await
        .unwrap();
    assert!(
        matches!(again.status().as_u16(), 200 | 409),
        "second upload: {}",
        again.status()
    );
    // A body shorter than announced is refused, nothing half-written is served.
    let short = s
        .client
        .put(s.url("/v1/cache/9100000000000000000002"))
        .bearer_auth(ctx)
        .header("content-type", "application/octet-stream")
        .header("content-length", "2000000")
        .body(reqwest::Body::wrap_stream(futures_util::stream::iter(vec![Ok::<
            _,
            std::io::Error,
        >(
            bytes::Bytes::from(vec![1u8; 1000]),
        )])))
        .send()
        .await;
    assert!(short.is_err() || short.unwrap().status().is_client_error());
    assert_eq!(
        s.client
            .get(s.url("/v1/cache/9100000000000000000002"))
            .bearer_auth("ro-token")
            .send()
            .await
            .unwrap()
            .status(),
        404
    );
}
