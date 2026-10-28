//! Load generator for nx-remote-cache (or any server implementing the Nx cache API).
//!
//! nx-cache-bench --url http://localhost:3000 --token rw-token --requests 500 --concurrency 32 --size 262144

use std::sync::Arc;
use std::time::{Duration, Instant};

use anyhow::{bail, Result};
use tokio::sync::Semaphore;

struct Args {
    url: String,
    token: Option<String>,
    requests: usize,
    concurrency: usize,
    size: usize,
}

fn parse_args() -> Result<Args> {
    let mut a = Args {
        url: "http://localhost:3000".into(),
        token: None,
        requests: 200,
        concurrency: 16,
        size: 256 * 1024,
    };
    let mut it = std::env::args().skip(1);
    while let Some(flag) = it.next() {
        let mut val = || it.next().ok_or_else(|| anyhow::anyhow!("missing value for {flag}"));
        match flag.as_str() {
            "--url" => a.url = val()?,
            "--token" => a.token = Some(val()?),
            "--requests" => a.requests = val()?.parse()?,
            "--concurrency" => a.concurrency = val()?.parse()?,
            "--size" => a.size = val()?.parse()?,
            "-h" | "--help" => {
                println!(
                    "usage: nx-cache-bench [--url URL] [--token T] [--requests N] [--concurrency C] [--size BYTES]"
                );
                std::process::exit(0);
            }
            other => bail!("unknown flag {other}"),
        }
    }
    Ok(a)
}

#[derive(Default)]
struct Phase {
    name: &'static str,
    latencies: Vec<Duration>,
    bytes: u64,
    wall: Duration,
    failures: usize,
}

impl Phase {
    fn report(&mut self) {
        self.latencies.sort();
        let n = self.latencies.len().max(1);
        let p = |q: f64| {
            self.latencies
                .get(((q * n as f64) as usize).min(n - 1))
                .copied()
                .unwrap_or_default()
                .as_secs_f64()
                * 1000.0
        };
        let secs = self.wall.as_secs_f64().max(1e-9);
        println!(
            "{:<10} {:>6} req  {:>8.0} req/s  {:>8.1} MB/s  p50 {:>7.2} ms  p90 {:>7.2} ms  p99 {:>7.2} ms  failures {}",
            self.name,
            self.latencies.len(),
            self.latencies.len() as f64 / secs,
            self.bytes as f64 / secs / 1e6,
            p(0.50),
            p(0.90),
            p(0.99),
            self.failures
        );
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    nx_remote_cache::install_crypto_provider();
    let args = Arc::new(parse_args()?);
    let mut headers = reqwest::header::HeaderMap::new();
    if let Some(t) = &args.token {
        headers.insert(reqwest::header::AUTHORIZATION, format!("Bearer {t}").parse()?);
    }
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .pool_max_idle_per_host(args.concurrency)
        .build()?;
    let run_id = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)?
        .as_nanos();
    let payload = bytes::Bytes::from((0..args.size).map(|i| (i % 253) as u8).collect::<Vec<u8>>());
    println!(
        "target {}  requests/phase {}  concurrency {}  artifact {} bytes",
        args.url, args.requests, args.concurrency, args.size
    );

    let hash = |i: usize| format!("bench-{run_id}-{i}");
    let mut put = run(&args, &client, "PUT", |c, i| {
        let body = payload.clone();
        let url = format!("{}/v1/cache/{}", args.url, hash(i));
        let size = args.size as u64;
        async move {
            Ok::<_, reqwest::Error>((
                c.put(url)
                    .header("content-length", size)
                    .body(body)
                    .send()
                    .await?
                    .status()
                    .as_u16(),
                size,
            ))
        }
    })
    .await;
    put.report();

    let mut hit = run(&args, &client, "GET hit", |c, i| {
        let url = format!("{}/v1/cache/{}", args.url, hash(i));
        async move {
            let res = c.get(url).send().await?;
            let status = res.status().as_u16();
            let bytes = res.bytes().await?.len() as u64;
            Ok::<_, reqwest::Error>((status, bytes))
        }
    })
    .await;
    hit.report();

    let mut miss = run(&args, &client, "GET miss", |c, i| {
        let url = format!("{}/v1/cache/miss-{run_id}-{i}", args.url);
        async move { Ok::<_, reqwest::Error>((c.get(url).send().await?.status().as_u16(), 0)) }
    })
    .await;
    miss.report();
    Ok(())
}

async fn run<F, Fut>(args: &Arc<Args>, client: &reqwest::Client, name: &'static str, make: F) -> Phase
where
    F: Fn(reqwest::Client, usize) -> Fut,
    Fut: std::future::Future<Output = Result<(u16, u64), reqwest::Error>> + Send + 'static,
{
    let expected = if name == "GET miss" { 404 } else { 200 };
    run_expecting(args, client, name, expected, make).await
}

async fn run_expecting<F, Fut>(
    args: &Arc<Args>,
    client: &reqwest::Client,
    name: &'static str,
    expected: u16,
    make: F,
) -> Phase
where
    F: Fn(reqwest::Client, usize) -> Fut,
    Fut: std::future::Future<Output = Result<(u16, u64), reqwest::Error>> + Send + 'static,
{
    let sem = Arc::new(Semaphore::new(args.concurrency));
    let start = Instant::now();
    let mut tasks = Vec::with_capacity(args.requests);
    for i in 0..args.requests {
        let permit = sem.clone().acquire_owned().await.expect("semaphore");
        let fut = make(client.clone(), i);
        tasks.push(tokio::spawn(async move {
            let t = Instant::now();
            let r = fut.await;
            drop(permit);
            (t.elapsed(), r)
        }));
    }
    let mut phase = Phase {
        name,
        ..Default::default()
    };
    let expected_ok = |s: u16| s == expected;
    for t in tasks {
        if let Ok((lat, r)) = t.await {
            match r {
                Ok((status, bytes)) if expected_ok(status) => {
                    phase.latencies.push(lat);
                    phase.bytes += bytes;
                }
                _ => phase.failures += 1,
            }
        }
    }
    phase.wall = start.elapsed();
    phase
}
