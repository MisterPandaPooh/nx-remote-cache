//! Walks the vendored Nx OpenAPI spec (`spec/nx-remote-cache-openapi.json`) and checks every
//! declared operation, status code and content type against the live router.

use std::collections::{BTreeMap, BTreeSet};
use std::net::SocketAddr;

use nx_remote_cache::store::CacheStore;
use nx_remote_cache::{router, AppState, Config};
use serde_json::Value;
use tokio::net::TcpListener;

const SPEC: &str = include_str!("../spec/nx-remote-cache-openapi.json");

/// Statuses the spec declares but this server never emits, with the reason.
const NEVER_EMITTED: &[(&str, u16, &str)] = &[("get", 403, "every valid token may read")];
/// Statuses this server emits beyond the spec (outside the Nx client's happy path).
const EXTRA: &[(&str, u16)] = &[("put", 400), ("put", 413), ("put", 502), ("get", 400), ("get", 502)];

struct Observed {
    status: u16,
    content_type: Option<String>,
}

async fn start() -> (String, reqwest::Client) {
    nx_remote_cache::install_crypto_provider();
    let cfg = Config::for_tests("memory:///");
    let store = CacheStore::open(&cfg).await.unwrap();
    let app = router(AppState::new(cfg, store));
    let listener = TcpListener::bind(SocketAddr::from(([127, 0, 0, 1], 0))).await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}"), reqwest::Client::new())
}

fn observed(res: &reqwest::Response) -> Observed {
    Observed {
        status: res.status().as_u16(),
        content_type: res
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_owned()),
    }
}

#[tokio::test]
async fn every_spec_operation_and_status_is_implemented() {
    let spec: Value = serde_json::from_str(SPEC).unwrap();
    assert_eq!(spec["openapi"], "3.0.0");
    let paths = spec["paths"].as_object().unwrap();
    assert_eq!(
        paths.keys().collect::<Vec<_>>(),
        vec!["/v1/cache/{hash}"],
        "the spec has exactly one path"
    );
    assert_eq!(spec["components"]["securitySchemes"]["bearerToken"]["scheme"], "bearer");

    let (base, client) = start().await;
    let url = |hash: &str| format!("{base}/v1/cache/{hash}");
    let body = vec![7u8; 1234];

    // Exercise the server once per situation the spec describes.
    let mut seen: BTreeMap<(&str, u16), Observed> = BTreeMap::new();
    let mut note = |op: &'static str, res: reqwest::Response| {
        let o = observed(&res);
        seen.entry((op, o.status)).or_insert(o);
    };

    // PUT 200: Content-Length is the required header parameter.
    let res = client
        .put(url("spec1"))
        .bearer_auth("rw-token")
        .header("content-type", "application/octet-stream")
        .header("content-length", body.len())
        .body(body.clone())
        .send()
        .await
        .unwrap();
    note("put", res);
    // PUT 409
    note(
        "put",
        client
            .put(url("spec1"))
            .bearer_auth("rw-token")
            .body(body.clone())
            .send()
            .await
            .unwrap(),
    );
    // PUT 401 (missing and invalid)
    note("put", client.put(url("spec2")).body(body.clone()).send().await.unwrap());
    note(
        "put",
        client
            .put(url("spec2"))
            .bearer_auth("wrong")
            .body(body.clone())
            .send()
            .await
            .unwrap(),
    );
    // PUT 403
    note(
        "put",
        client
            .put(url("spec3"))
            .bearer_auth("ro-token")
            .body(body.clone())
            .send()
            .await
            .unwrap(),
    );
    // PUT extras
    note(
        "put",
        client
            .put(url("bad.hash"))
            .bearer_auth("rw-token")
            .body(body.clone())
            .send()
            .await
            .unwrap(),
    );
    note(
        "put",
        client
            .put(url("spec4"))
            .bearer_auth("rw-token")
            .body(vec![0u8; 65 << 20])
            .send()
            .await
            .unwrap(),
    );
    // GET 200 / 404 / 401 (the spec's GET lists no 401; it is the bearer scheme's natural answer)
    note(
        "get",
        client.get(url("spec1")).bearer_auth("ro-token").send().await.unwrap(),
    );
    note(
        "get",
        client.get(url("missing")).bearer_auth("rw-token").send().await.unwrap(),
    );
    note("get", client.get(url("spec1")).send().await.unwrap());
    note(
        "get",
        client
            .get(url("bad.hash"))
            .bearer_auth("rw-token")
            .send()
            .await
            .unwrap(),
    );

    // 1. Everything the spec declares is produced (or explicitly documented as never emitted).
    let mut missing = Vec::new();
    for (op, operation) in paths["/v1/cache/{hash}"].as_object().unwrap() {
        assert_eq!(
            operation["security"][0]["bearerToken"],
            Value::Array(vec![]),
            "{op} requires bearer auth"
        );
        for (code, response) in operation["responses"].as_object().unwrap() {
            let status: u16 = code.parse().unwrap();
            let op: &str = op.as_str();
            if NEVER_EMITTED.iter().any(|(o, s, _)| *o == op && *s == status) {
                continue;
            }
            match seen.get(&(op, status)) {
                None => missing.push(format!("{op} {status}")),
                Some(o) => {
                    if let Some(content) = response.get("content").and_then(|c| c.as_object()) {
                        let declared = content.keys().next().unwrap();
                        let actual = o.content_type.as_deref().unwrap_or("");
                        assert!(
                            actual.starts_with(declared),
                            "{op} {status}: spec says {declared}, server sent {actual}"
                        );
                    }
                }
            }
        }
    }
    assert!(missing.is_empty(), "declared by the spec but not produced: {missing:?}");

    // 2. Everything the server produces is either in the spec, a documented extra, or 401 on GET.
    let declared: BTreeSet<(&str, u16)> = paths["/v1/cache/{hash}"]
        .as_object()
        .unwrap()
        .iter()
        .flat_map(|(op, operation)| {
            let op: &str = match op.as_str() {
                "put" => "put",
                "get" => "get",
                other => panic!("unexpected operation {other}"),
            };
            operation
                .get("responses")
                .and_then(|r| r.as_object())
                .into_iter()
                .flatten()
                .map(move |(code, _)| (op, code.parse::<u16>().unwrap()))
                .collect::<Vec<_>>()
        })
        .collect();
    for (op, status) in seen.keys() {
        let ok =
            declared.contains(&(op, *status)) || EXTRA.contains(&(op, *status)) || (*op == "get" && *status == 401);
        assert!(ok, "server produced {op} {status}, which the spec does not declare");
    }

    // 3. The required PUT parameters are what the spec says.
    let params = paths["/v1/cache/{hash}"]["put"]["parameters"].as_array().unwrap();
    let names: BTreeSet<&str> = params.iter().map(|p| p["name"].as_str().unwrap()).collect();
    assert_eq!(names, ["Content-Length", "hash"].into_iter().collect());
    assert!(params.iter().all(|p| p["required"] == true));
    assert_eq!(
        paths["/v1/cache/{hash}"]["put"]["requestBody"]["content"]
            .as_object()
            .unwrap()
            .keys()
            .next()
            .unwrap(),
        "application/octet-stream"
    );
}
