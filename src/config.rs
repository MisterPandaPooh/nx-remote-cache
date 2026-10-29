use std::net::{IpAddr, SocketAddr};
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use url::Url;

/// Runtime configuration, read once from the environment at startup.
#[derive(Clone, Debug)]
pub struct Config {
    pub bind: SocketAddr,
    pub store_url: Url,
    /// Tokens allowed to read and write. Empty with `allow_anonymous` means open access.
    pub rw_tokens: Vec<String>,
    /// Tokens allowed to read only; a PUT with one of these gets 403.
    pub ro_tokens: Vec<String>,
    pub allow_anonymous: bool,
    /// `NX_CACHE_ENABLE_LOGIN=false`: no token anywhere, cache API and dashboard open.
    pub login_enabled: bool,
    pub max_artifact_bytes: u64,
    /// Bodies up to this size are buffered and written with a single atomic create.
    pub inline_put_threshold: u64,
    pub multipart_chunk_bytes: usize,
    pub multipart_concurrency: usize,
    pub store_timeout: Duration,
    pub store_connect_timeout: Duration,
    pub store_max_retries: usize,
    /// Memory budget for the hot-artifact LRU (0 disables it).
    pub memory_cache_bytes: u64,
    /// Artifacts above this size are never kept in memory.
    pub memory_cache_max_item: u64,
    /// Global cap on uploads in flight; bounds memory to roughly
    /// `max_concurrent_puts * max(inline_put_threshold, multipart_chunk_bytes * multipart_concurrency)`.
    pub max_concurrent_puts: usize,
    /// Delete artifacts older than this (by store modification time). `None` keeps them forever.
    /// Budget of the side key-value store (`/v1/kv`); 0 disables it.
    /// `NX_CACHE_KV_ENABLED`; with `false` the store is off whatever the budget says.
    pub kv_enabled: bool,
    /// `NX_CACHE_KV_BACKEND`: `memory` (one view per pod) or `redis` (shared by every replica).
    pub kv_backend: KvBackend,
    /// `NX_CACHE_KV_REDIS_URL`, required with the redis backend; never logged (it may hold a password).
    pub kv_redis_url: Option<String>,
    /// `NX_CACHE_KV_REDIS_PREFIX`: first part of every Redis key.
    pub kv_redis_prefix: String,
    pub kv_bytes: u64,
    pub kv_max_value: u64,
    /// CI integration: links in the dashboard point at `github_url` (GitHub.com or an
    /// Enterprise instance). GitHub is the only provider so far.
    pub github_enabled: bool,
    pub github_url: String,
    pub ttl: Option<Duration>,
    /// When the prefix holds more than this many bytes, delete the oldest artifacts down to 90 %.
    pub max_total_bytes: Option<u64>,
    pub sweep_interval: Duration,
    pub ui_enabled: bool,
    pub ui_token: Option<String>,
    /// Name of a header set by an authenticating reverse proxy (oauth2-proxy, Pomerium...).
    /// A dashboard request carrying it passes the guard; the service must then only be reachable through that proxy.
    pub ui_trusted_header: Option<String>,
    pub log_format: LogFormat,
    /// Base level (`error`..`trace`); `RUST_LOG` overrides it with full filter directives.
    pub log_level: String,
    /// Log every request at `info` (otherwise requests are `debug`, failures `warn`).
    pub access_log: bool,
    pub pod_name: String,
    pub shutdown_grace: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let port: u16 = env_parse("PORT", 3000)?;
        let bind_ip: IpAddr = env_parse("NX_CACHE_BIND", IpAddr::from([0, 0, 0, 0]))?;
        let store_url = std::env::var("NX_CACHE_STORE_URL")
            .map_err(|_| anyhow!("NX_CACHE_STORE_URL is required, e.g. az://nx-cache/, s3://bucket/prefix, gs://bucket, file:///data/nx-cache"))?;
        let store_url =
            Url::parse(&store_url).with_context(|| format!("NX_CACHE_STORE_URL `{store_url}` is not a valid URL"))?;

        let rw_tokens = env_list("NX_CACHE_TOKEN");
        let ro_tokens = env_list("NX_CACHE_READ_TOKEN");
        let login_enabled = env_bool("NX_CACHE_ENABLE_LOGIN", true)?;
        let allow_anonymous = env_bool("NX_CACHE_ALLOW_ANONYMOUS", false)? || !login_enabled;
        if KvBackend::from_env()? == KvBackend::Redis
            && std::env::var("NX_CACHE_KV_REDIS_URL")
                .map(|s| s.trim().is_empty())
                .unwrap_or(true)
        {
            bail!("NX_CACHE_KV_BACKEND=redis needs NX_CACHE_KV_REDIS_URL (redis://[:password@]host:6379/0 or rediss://...)");
        }
        if rw_tokens.is_empty() && ro_tokens.is_empty() && !allow_anonymous {
            bail!("no token configured: set NX_CACHE_TOKEN (and optionally NX_CACHE_READ_TOKEN), or NX_CACHE_ENABLE_LOGIN=false to run open");
        }

        let max_artifact_bytes = env_size("NX_CACHE_MAX_ARTIFACT_BYTES", 4 << 30)?;
        let inline_put_threshold = env_size("NX_CACHE_INLINE_PUT_THRESHOLD", 8 << 20)?;
        let multipart_chunk_bytes = env_size("NX_CACHE_MULTIPART_CHUNK_BYTES", 8 << 20)? as usize;
        if multipart_chunk_bytes < 5 << 20 {
            bail!("NX_CACHE_MULTIPART_CHUNK_BYTES must be at least 5MB (cloud stores reject smaller parts)");
        }
        let multipart_concurrency: usize = env_parse("NX_CACHE_MULTIPART_CONCURRENCY", 4)?;

        Ok(Self {
            bind: SocketAddr::new(bind_ip, port),
            store_url,
            rw_tokens,
            ro_tokens,
            allow_anonymous,
            login_enabled,
            max_artifact_bytes,
            inline_put_threshold,
            multipart_chunk_bytes,
            multipart_concurrency: multipart_concurrency.max(1),
            store_timeout: Duration::from_secs(env_parse("NX_CACHE_STORE_TIMEOUT_SECS", 300)?),
            store_connect_timeout: Duration::from_secs(env_parse("NX_CACHE_STORE_CONNECT_TIMEOUT_SECS", 5)?),
            store_max_retries: env_parse("NX_CACHE_STORE_MAX_RETRIES", 3)?,
            memory_cache_bytes: env_size("NX_CACHE_MEMORY_CACHE_BYTES", 256 << 20)?,
            memory_cache_max_item: env_size("NX_CACHE_MEMORY_CACHE_MAX_ITEM", 4 << 20)?,
            max_concurrent_puts: env_parse::<usize>("NX_CACHE_MAX_CONCURRENT_PUTS", 64)?.max(1),
            kv_enabled: env_bool("NX_CACHE_KV_ENABLED", true)?,
            kv_backend: KvBackend::from_env()?,
            kv_redis_url: std::env::var("NX_CACHE_KV_REDIS_URL")
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty()),
            kv_redis_prefix: std::env::var("NX_CACHE_KV_REDIS_PREFIX")
                .ok()
                .map(|s| s.trim().to_owned())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| "nxrc:kv".to_owned()),
            kv_bytes: env_size("NX_CACHE_KV_BYTES", 64 << 20)?,
            kv_max_value: env_size("NX_CACHE_KV_MAX_VALUE", 1 << 20)?,
            github_enabled: env_bool("NX_CACHE_GITHUB_ENABLED", true)?,
            github_url: std::env::var("NX_CACHE_GITHUB_URL")
                .ok()
                .map(|u| u.trim().trim_end_matches('/').to_owned())
                .filter(|u| !u.is_empty())
                .unwrap_or_else(|| "https://github.com".to_owned()),
            ttl: env_duration("NX_CACHE_TTL")?,
            max_total_bytes: match std::env::var("NX_CACHE_MAX_TOTAL_BYTES") {
                Ok(v) if !v.trim().is_empty() => Some(env_size("NX_CACHE_MAX_TOTAL_BYTES", 0)?),
                _ => None,
            },
            sweep_interval: env_duration("NX_CACHE_SWEEP_INTERVAL")?.unwrap_or(Duration::from_secs(3600)),
            ui_enabled: env_bool("NX_CACHE_UI_ENABLED", true)?,
            ui_token: std::env::var("NX_CACHE_UI_TOKEN").ok().filter(|s| !s.is_empty()),
            ui_trusted_header: std::env::var("NX_CACHE_UI_TRUSTED_HEADER")
                .ok()
                .filter(|s| !s.is_empty()),
            log_format: LogFormat::from_env()?,
            log_level: log_level_from_env()?,
            access_log: env_bool("NX_CACHE_ACCESS_LOG", false)?,
            pod_name: std::env::var("HOSTNAME").unwrap_or_else(|_| "local".into()),
            shutdown_grace: Duration::from_secs(env_parse("NX_CACHE_SHUTDOWN_GRACE_SECS", 20)?),
        })
    }

    /// A configuration for tests and embedding: open access, memory-friendly defaults.
    pub fn for_tests(store_url: &str) -> Self {
        Self {
            bind: SocketAddr::from(([127, 0, 0, 1], 0)),
            store_url: Url::parse(store_url).expect("valid store url"),
            rw_tokens: vec!["rw-token".into()],
            ro_tokens: vec!["ro-token".into()],
            allow_anonymous: false,
            login_enabled: true,
            max_artifact_bytes: 64 << 20,
            inline_put_threshold: 1 << 20,
            multipart_chunk_bytes: 5 << 20,
            multipart_concurrency: 2,
            store_timeout: Duration::from_secs(30),
            store_connect_timeout: Duration::from_secs(5),
            store_max_retries: 1,
            memory_cache_bytes: 8 << 20,
            memory_cache_max_item: 1 << 20,
            max_concurrent_puts: 8,
            kv_enabled: true,
            kv_backend: KvBackend::Memory,
            kv_redis_url: None,
            kv_redis_prefix: "nxrc:kv".to_owned(),
            kv_bytes: 1 << 20,
            kv_max_value: 64 << 10,
            github_enabled: true,
            github_url: "https://github.com".to_owned(),
            ttl: None,
            max_total_bytes: None,
            sweep_interval: Duration::from_secs(3600),
            ui_enabled: true,
            ui_token: None,
            ui_trusted_header: None,
            log_format: LogFormat::Pretty,
            log_level: "info".into(),
            access_log: false,
            pod_name: "test".into(),
            shutdown_grace: Duration::from_secs(1),
        }
    }
}

/// Where the key-value store keeps its keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KvBackend {
    Memory,
    Redis,
}

impl KvBackend {
    pub fn from_env() -> Result<Self> {
        match std::env::var("NX_CACHE_KV_BACKEND")
            .unwrap_or_default()
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "" | "memory" => Ok(KvBackend::Memory),
            "redis" => Ok(KvBackend::Redis),
            other => bail!("NX_CACHE_KV_BACKEND must be memory or redis, got `{other}`"),
        }
    }
}

/// `NX_CACHE_LOG_FORMAT`: one JSON object per line (default), human `pretty`, or `compact`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LogFormat {
    Json,
    Pretty,
    Compact,
}

impl LogFormat {
    fn from_env() -> Result<Self> {
        match std::env::var("NX_CACHE_LOG_FORMAT").as_deref().map(str::trim) {
            Err(_) | Ok("") | Ok("json") => Ok(Self::Json),
            Ok("pretty") | Ok("text") => Ok(Self::Pretty),
            Ok("compact") => Ok(Self::Compact),
            Ok(other) => bail!("NX_CACHE_LOG_FORMAT=`{other}`: use json, pretty or compact"),
        }
    }
}

fn log_level_from_env() -> Result<String> {
    let level = std::env::var("NX_CACHE_LOG_LEVEL").unwrap_or_else(|_| "info".into());
    let level = level.trim().to_ascii_lowercase();
    match level.as_str() {
        "error" | "warn" | "info" | "debug" | "trace" | "off" => Ok(level),
        other => bail!("NX_CACHE_LOG_LEVEL=`{other}`: use error, warn, info, debug, trace or off"),
    }
}

fn env_parse<T: std::str::FromStr>(key: &str, default: T) -> Result<T>
where
    T::Err: std::fmt::Display,
{
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => v.trim().parse::<T>().map_err(|e| anyhow!("{key}=`{v}`: {e}")),
        _ => Ok(default),
    }
}

fn env_bool(key: &str, default: bool) -> Result<bool> {
    match std::env::var(key) {
        Ok(v) => match v.trim().to_ascii_lowercase().as_str() {
            "1" | "true" | "yes" | "on" => Ok(true),
            "0" | "false" | "no" | "off" | "" => Ok(false),
            other => bail!("{key}=`{other}` is not a boolean"),
        },
        Err(_) => Ok(default),
    }
}

fn env_list(key: &str) -> Vec<String> {
    std::env::var(key)
        .map(|v| {
            v.split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect()
        })
        .unwrap_or_default()
}

fn env_size(key: &str, default: u64) -> Result<u64> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => {
            parse_size(&v).ok_or_else(|| anyhow!("{key}=`{v}` is not a size (use e.g. 512MB, 2GB, 1073741824)"))
        }
        _ => Ok(default),
    }
}

fn env_duration(key: &str) -> Result<Option<Duration>> {
    match std::env::var(key) {
        Ok(v) if !v.trim().is_empty() => parse_duration(&v)
            .map(Some)
            .ok_or_else(|| anyhow!("{key}=`{v}` is not a duration (use e.g. 30d, 12h, 90m, 3600s)")),
        _ => Ok(None),
    }
}

/// Parse `30d`, `12h`, `90m`, `45s` or a bare number of seconds.
pub fn parse_duration(raw: &str) -> Option<Duration> {
    let s = raw.trim().to_ascii_lowercase();
    let digits_end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if digits_end == 0 {
        return None;
    }
    let (num, unit) = s.split_at(digits_end);
    let num: u64 = num.parse().ok()?;
    let secs = match unit.trim() {
        "" | "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86_400,
        "w" => 7 * 86_400,
        _ => return None,
    };
    Some(Duration::from_secs(num.checked_mul(secs)?))
}

/// Parse `123`, `10KB`, `512MB`, `2GB`, `1GiB` (case-insensitive, binary units).
pub fn parse_size(raw: &str) -> Option<u64> {
    let s = raw.trim().to_ascii_uppercase();
    let digits_end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if digits_end == 0 {
        return None;
    }
    let (num, unit) = s.split_at(digits_end);
    let num: u64 = num.parse().ok()?;
    let mult: u64 = match unit.trim() {
        "" | "B" => 1,
        "K" | "KB" | "KIB" => 1 << 10,
        "M" | "MB" | "MIB" => 1 << 20,
        "G" | "GB" | "GIB" => 1 << 30,
        "T" | "TB" | "TIB" => 1 << 40,
        _ => return None,
    };
    num.checked_mul(mult)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes() {
        assert_eq!(parse_size("1024"), Some(1024));
        assert_eq!(parse_size("10kb"), Some(10 << 10));
        assert_eq!(parse_size("512MB"), Some(512 << 20));
        assert_eq!(parse_size("2 GiB"), Some(2 << 30));
        assert_eq!(parse_size("abc"), None);
        assert_eq!(parse_size("12xb"), None);
    }

    #[test]
    fn durations() {
        assert_eq!(parse_duration("30d"), Some(Duration::from_secs(30 * 86_400)));
        assert_eq!(parse_duration("12h"), Some(Duration::from_secs(12 * 3600)));
        assert_eq!(parse_duration("90m"), Some(Duration::from_secs(5400)));
        assert_eq!(parse_duration("45"), Some(Duration::from_secs(45)));
        assert_eq!(parse_duration("1y"), None);
    }
}
