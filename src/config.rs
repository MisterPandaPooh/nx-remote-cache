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
    pub max_artifact_bytes: u64,
    /// Bodies up to this size are buffered and written with a single atomic create.
    pub inline_put_threshold: u64,
    pub multipart_chunk_bytes: usize,
    pub multipart_concurrency: usize,
    pub store_timeout: Duration,
    pub store_connect_timeout: Duration,
    pub store_max_retries: usize,
    /// Global cap on uploads in flight; bounds memory to roughly
    /// `max_concurrent_puts * max(inline_put_threshold, multipart_chunk_bytes * multipart_concurrency)`.
    pub max_concurrent_puts: usize,
    /// Delete artifacts older than this (by store modification time). `None` keeps them forever.
    pub ttl: Option<Duration>,
    /// When the prefix holds more than this many bytes, delete the oldest artifacts down to 90 %.
    pub max_total_bytes: Option<u64>,
    pub sweep_interval: Duration,
    pub ui_enabled: bool,
    pub ui_token: Option<String>,
    /// Name of a header set by an authenticating reverse proxy (oauth2-proxy, Pomerium...).
    /// A dashboard request carrying it passes the guard; the service must then only be reachable through that proxy.
    pub ui_trusted_header: Option<String>,
    pub log_json: bool,
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
        let allow_anonymous = env_bool("NX_CACHE_ALLOW_ANONYMOUS", false)?;
        if rw_tokens.is_empty() && ro_tokens.is_empty() && !allow_anonymous {
            bail!("no token configured: set NX_CACHE_TOKEN (and optionally NX_CACHE_READ_TOKEN), or NX_CACHE_ALLOW_ANONYMOUS=true to run open");
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
            max_artifact_bytes,
            inline_put_threshold,
            multipart_chunk_bytes,
            multipart_concurrency: multipart_concurrency.max(1),
            store_timeout: Duration::from_secs(env_parse("NX_CACHE_STORE_TIMEOUT_SECS", 300)?),
            store_connect_timeout: Duration::from_secs(env_parse("NX_CACHE_STORE_CONNECT_TIMEOUT_SECS", 5)?),
            store_max_retries: env_parse("NX_CACHE_STORE_MAX_RETRIES", 3)?,
            max_concurrent_puts: env_parse::<usize>("NX_CACHE_MAX_CONCURRENT_PUTS", 64)?.max(1),
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
            log_json: std::env::var("NX_CACHE_LOG_FORMAT")
                .map(|v| v != "pretty")
                .unwrap_or(true),
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
            max_artifact_bytes: 64 << 20,
            inline_put_threshold: 1 << 20,
            multipart_chunk_bytes: 5 << 20,
            multipart_concurrency: 2,
            store_timeout: Duration::from_secs(30),
            store_connect_timeout: Duration::from_secs(5),
            store_max_retries: 1,
            max_concurrent_puts: 8,
            ttl: None,
            max_total_bytes: None,
            sweep_interval: Duration::from_secs(3600),
            ui_enabled: true,
            ui_token: None,
            ui_trusted_header: None,
            log_json: false,
            pod_name: "test".into(),
            shutdown_grace: Duration::from_secs(1),
        }
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
