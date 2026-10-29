use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use bytes::{Bytes, BytesMut};
use futures_util::{Stream, StreamExt, TryStreamExt};
use object_store::aws::AmazonS3Builder;
use object_store::azure::MicrosoftAzureBuilder;
use object_store::gcp::GoogleCloudStorageBuilder;
use object_store::local::LocalFileSystem;
use object_store::memory::InMemory;
use object_store::path::Path as StorePath;
use object_store::{
    Attribute, Attributes, BackoffConfig, ClientOptions, Error as StoreError, GetResult, ObjectStore, ObjectStoreExt,
    ObjectStoreScheme, PutMode, PutMultipartOptions, PutOptions, PutPayload, RetryConfig, WriteMultipart,
};

use crate::auth::Context as RequestContext;

/// Disk writes up to this size are gathered and written in one blocking call; larger bodies
/// stream into a writer thread in blocks of `FS_WRITE_BLOCK`, so they are copied once.
const FS_GATHER_MAX: u64 = 256 << 10;

/// Body chunks are batched until this many bytes are pending before crossing to the writer
/// thread: one wake-up per megabyte instead of one per socket read. Large chunks are passed
/// through untouched (no copy); small ones are gathered so a chunked body does not turn into
/// thousands of tiny writes.
const FS_WRITE_BLOCK: usize = 1 << 20;
const FS_GATHER_CHUNK: usize = 64 << 10;

/// Prefix for the user-defined metadata keys written next to an artifact.
const META_PREFIX: &str = "nx_";

/// Turn a request context into store metadata (`x-ms-meta-nx_run: 123` on Azure).
pub fn attributes_from(ctx: &RequestContext) -> Attributes {
    ctx.iter()
        .map(|(k, v): (&String, &String)| {
            (
                Attribute::Metadata(format!("{META_PREFIX}{k}").into()),
                object_store::AttributeValue::from(v.clone()),
            )
        })
        .collect()
}

/// Read back the context an artifact was uploaded with.
pub fn origin_from(attrs: &Attributes) -> RequestContext {
    attrs
        .iter()
        .filter_map(|(k, v)| match k {
            Attribute::Metadata(name) => name
                .strip_prefix(META_PREFIX)
                .map(|n| (n.to_owned(), v.as_ref().to_owned())),
            _ => None,
        })
        .collect()
}
use object_store::signer::{SignedUrlOptions, Signer};
use url::Url;

use crate::config::Config;

/// Outcome of a write: either the artifact is new, or it was already there.
#[derive(Debug, PartialEq, Eq)]
pub enum PutOutcome {
    Created,
    AlreadyExists,
}

#[derive(Debug)]
pub enum PutError {
    /// The client sent fewer or more bytes than announced, or more than allowed.
    BadBody(String),
    TooLarge,
    Store(StoreError),
}

impl From<StoreError> for PutError {
    fn from(e: StoreError) -> Self {
        PutError::Store(e)
    }
}

/// A content-addressed artifact store over any `object_store` backend.
#[derive(Clone)]
pub struct CacheStore {
    inner: Arc<dyn ObjectStore>,
    prefix: StorePath,
    /// Set for `file://` stores: reads and writes go straight through tokio::fs (streamed,
    /// no buffering), object_store is kept for listing, deleting and probing.
    fs_root: Option<std::path::PathBuf>,
    /// Object stores that can presign a request (S3, GCS, Azure): bodies of known length up to
    /// `stream_threshold` are streamed straight to the store through a presigned `PUT`.
    signer: Option<Arc<dyn Signer>>,
    http: reqwest::Client,
    pub kind: &'static str,
    pub location: String,
    inline_threshold: u64,
    stream_threshold: u64,
    chunk_bytes: usize,
    concurrency: usize,
}

impl std::fmt::Debug for CacheStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CacheStore")
            .field("kind", &self.kind)
            .field("location", &self.location)
            .finish()
    }
}

impl CacheStore {
    pub async fn open(cfg: &Config) -> Result<Self> {
        // The presigned-upload client speaks TLS to real stores; rustls needs its provider first.
        crate::install_crypto_provider();
        let url = &cfg.store_url;
        let (scheme, prefix) =
            ObjectStoreScheme::parse(url).map_err(|e| anyhow!("unsupported store URL `{url}`: {e}"))?;
        let prefix = StorePath::parse(prefix)?;
        let client = ClientOptions::new()
            .with_timeout(cfg.store_timeout)
            .with_connect_timeout(cfg.store_connect_timeout)
            .with_allow_http(true);
        let retry = RetryConfig {
            backoff: BackoffConfig::default(),
            max_retries: cfg.store_max_retries,
            retry_timeout: Duration::from_secs(60),
        };

        let mut fs_root = None;
        let mut signer: Option<Arc<dyn Signer>> = None;
        let (inner, kind, prefix): (Arc<dyn ObjectStore>, &'static str, StorePath) = match scheme {
            ObjectStoreScheme::Local => {
                let dir = url
                    .to_file_path()
                    .map_err(|_| anyhow!("file:// store URL must be an absolute path"))?;
                tokio::fs::create_dir_all(&dir)
                    .await
                    .with_context(|| format!("creating {}", dir.display()))?;
                let fs = LocalFileSystem::new_with_prefix(&dir)?.with_automatic_cleanup(true);
                fs_root = Some(dir.canonicalize().unwrap_or(dir));
                (Arc::new(fs), "filesystem", StorePath::default())
            }
            ObjectStoreScheme::Memory => (Arc::new(InMemory::new()), "memory", prefix),
            ObjectStoreScheme::MicrosoftAzure => {
                let store = MicrosoftAzureBuilder::from_env()
                    .with_url(url.as_str())
                    .with_client_options(client)
                    .with_retry(retry)
                    .build()
                    .context("building the Azure Blob client (check AZURE_STORAGE_ACCOUNT_NAME and credentials)")?;
                let store = Arc::new(store);
                signer = Some(store.clone());
                (store, "azure", prefix)
            }
            ObjectStoreScheme::AmazonS3 => {
                let store = AmazonS3Builder::from_env()
                    .with_url(url.as_str())
                    .with_client_options(client)
                    .with_retry(retry)
                    .build()
                    .context("building the S3 client")?;
                let store = Arc::new(store);
                signer = Some(store.clone());
                (store, "s3", prefix)
            }
            ObjectStoreScheme::GoogleCloudStorage => {
                let store = GoogleCloudStorageBuilder::from_env()
                    .with_url(url.as_str())
                    .with_client_options(client)
                    .with_retry(retry)
                    .build()
                    .context("building the GCS client")?;
                let store = Arc::new(store);
                signer = Some(store.clone());
                (store, "gcs", prefix)
            }
            other => bail!("store scheme {other:?} is not supported"),
        };

        let http = reqwest::Client::builder()
            .timeout(cfg.store_timeout)
            .connect_timeout(cfg.store_connect_timeout)
            .build()
            .context("building the HTTP client for presigned uploads")?;
        Ok(Self {
            inner,
            prefix,
            fs_root,
            signer,
            http,
            kind,
            location: redact(url),
            inline_threshold: cfg.inline_put_threshold,
            stream_threshold: cfg.stream_put_threshold,
            chunk_bytes: cfg.multipart_chunk_bytes,
            concurrency: cfg.multipart_concurrency,
        })
    }

    fn key(&self, hash: &str) -> StorePath {
        self.prefix.clone().join(hash)
    }

    pub async fn exists(&self, hash: &str) -> Result<bool, StoreError> {
        if let Some(root) = &self.fs_root {
            return tokio::fs::try_exists(root.join(hash)).await.map_err(io_err);
        }
        match self.inner.head(&self.key(hash)).await {
            Ok(_) => Ok(true),
            Err(StoreError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn get(&self, hash: &str) -> Result<Option<GetResult>, StoreError> {
        if let Some(root) = &self.fs_root {
            return self.get_fs(root, hash).await;
        }
        match self.inner.get(&self.key(hash)).await {
            Ok(r) => Ok(Some(r)),
            Err(StoreError::NotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Write an artifact. Small bodies use one atomic create; large ones stream through a
    /// multipart upload. `expected_len` is the announced Content-Length, when known.
    pub async fn put<S>(
        &self,
        hash: &str,
        expected_len: Option<u64>,
        mut body: S,
        context: &RequestContext,
    ) -> Result<PutOutcome, PutError>
    where
        S: Stream<Item = Result<Bytes, PutError>> + Unpin + Send + 'static,
    {
        let key = self.key(hash);
        if let Some(root) = &self.fs_root {
            return self.put_fs(root, hash, expected_len, body).await;
        }
        // Local disks have no metadata; everything else records who produced the artifact.
        let attributes = if self.kind == "filesystem" {
            Attributes::default()
        } else {
            attributes_from(context)
        };
        if expected_len.is_some_and(|n| n <= self.inline_threshold) {
            let expected = expected_len.unwrap_or(0) as usize;
            let mut buf = BytesMut::with_capacity(expected);
            while let Some(chunk) = body.next().await {
                buf.extend_from_slice(&chunk?);
            }
            if buf.len() != expected {
                return Err(PutError::BadBody(format!(
                    "announced {expected} bytes, received {}",
                    buf.len()
                )));
            }
            return self.put_inline(&key, buf.freeze(), attributes).await;
        }

        // Known length, presignable store: stream the body straight through, nothing buffered.
        if let (Some(len), Some(signer)) = (expected_len, self.signer.as_ref()) {
            if len <= self.stream_threshold {
                return self.put_presigned(signer.as_ref(), &key, len, body, context).await;
            }
        }

        // Multipart: cheap existence check first so a duplicate is not uploaded to the store.
        // The body is still read to the end so the client reliably receives the 409.
        if self.inner.head(&key).await.is_ok() {
            while let Some(chunk) = body.next().await {
                chunk?;
            }
            return Ok(PutOutcome::AlreadyExists);
        }
        let upload = self
            .inner
            .put_multipart_opts(
                &key,
                PutMultipartOptions {
                    attributes,
                    ..Default::default()
                },
            )
            .await?;
        let mut writer = WriteMultipart::new_with_chunk_size(upload, self.chunk_bytes);
        let mut received: u64 = 0;
        while let Some(chunk) = body.next().await {
            let chunk = match chunk {
                Ok(c) => c,
                Err(e) => {
                    let _ = writer.abort().await;
                    return Err(e);
                }
            };
            received += chunk.len() as u64;
            writer.put(chunk);
            if let Err(e) = writer.wait_for_capacity(self.concurrency).await {
                let _ = writer.abort().await;
                return Err(e.into());
            }
        }
        if let Some(expected) = expected_len {
            if received != expected {
                let _ = writer.abort().await;
                return Err(PutError::BadBody(format!(
                    "announced {expected} bytes, received {received}"
                )));
            }
        }
        writer.finish().await?;
        Ok(PutOutcome::Created)
    }

    /// Disk read with one blocking-pool hop for files up to the inline threshold (the common
    /// case), a 256 KiB chunk stream above it.
    async fn get_fs(&self, root: &std::path::Path, hash: &str) -> Result<Option<GetResult>, StoreError> {
        let path = root.join(hash);
        let inline = self.inline_threshold;
        let read = tokio::task::spawn_blocking(
            move || -> std::io::Result<Option<(std::fs::Metadata, Option<Vec<u8>>)>> {
                let file = match std::fs::File::open(&path) {
                    Ok(f) => f,
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
                    Err(e) => return Err(e),
                };
                let meta = file.metadata()?;
                if meta.len() <= inline {
                    let mut buf = Vec::with_capacity(meta.len() as usize);
                    std::io::Read::read_to_end(&mut &file, &mut buf)?;
                    Ok(Some((meta, Some(buf))))
                } else {
                    Ok(Some((meta, None)))
                }
            },
        )
        .await
        .map_err(|e| io_err(std::io::Error::other(e)))?
        .map_err(io_err)?;
        let Some((meta, data)) = read else { return Ok(None) };
        let size = meta.len();
        let last_modified = meta
            .modified()
            .map(chrono::DateTime::<chrono::Utc>::from)
            .unwrap_or_default();
        let payload = match data {
            Some(buf) => object_store::GetResultPayload::Stream(
                futures_util::stream::once(async move { Ok(Bytes::from(buf)) }).boxed(),
            ),
            None => {
                let file = tokio::fs::File::open(root.join(hash)).await.map_err(io_err)?;
                object_store::GetResultPayload::Stream(
                    tokio_util::io::ReaderStream::with_capacity(file, 256 << 10)
                        .map_err(io_err)
                        .boxed(),
                )
            }
        };
        Ok(Some(GetResult {
            payload,
            meta: object_store::ObjectMeta {
                location: self.key(hash),
                last_modified,
                size,
                e_tag: None,
                version: None,
            },
            range: 0..size,
            attributes: Attributes::default(),
            extensions: Default::default(),
        }))
    }

    /// Disk write: the body streams through a channel into one blocking task that writes a
    /// temp file and renames it into place. No fsync: a crash loses at most the artifact being
    /// written, which Nx simply uploads again.
    async fn put_fs<S>(
        &self,
        root: &std::path::Path,
        hash: &str,
        expected_len: Option<u64>,
        mut body: S,
    ) -> Result<PutOutcome, PutError>
    where
        S: Stream<Item = Result<Bytes, PutError>> + Unpin,
    {
        let final_path = root.join(hash);
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        // Dot-prefixed so the retention sweep ignores it while it is being written.
        let tmp = root.join(format!(".{hash}.{nonce}.tmp"));

        // Small bodies: gather, then one blocking hop does exists + write + rename.
        if expected_len.is_some_and(|n| n <= FS_GATHER_MAX) {
            let expected = expected_len.unwrap_or(0) as usize;
            let mut buf = Vec::with_capacity(expected);
            while let Some(chunk) = body.next().await {
                buf.extend_from_slice(&chunk?);
            }
            if buf.len() != expected {
                return Err(PutError::BadBody(format!(
                    "announced {expected} bytes, received {}",
                    buf.len()
                )));
            }
            return tokio::task::spawn_blocking(move || write_whole(&final_path, &tmp, &buf))
                .await
                .map_err(|e| io_err(std::io::Error::other(e)))?
                .map_err(|e| PutError::Store(io_err(e)));
        }

        // Large or unknown-length bodies: stream chunks to a writer thread.
        if tokio::fs::try_exists(&final_path).await.unwrap_or(false) {
            while let Some(chunk) = body.next().await {
                chunk?;
            }
            return Ok(PutOutcome::AlreadyExists);
        }
        let (tx, rx) = std::sync::mpsc::sync_channel::<Vec<Bytes>>(4);
        let (fp, tp) = (final_path.clone(), tmp.clone());
        // Set when the body fails: the writer then discards the temp file instead of renaming
        // a truncated artifact into place.
        let aborted = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let aborted_w = aborted.clone();
        let writer = tokio::task::spawn_blocking(move || -> std::io::Result<PutOutcome> {
            use std::io::Write;
            let mut file = std::fs::File::create(&tp)?;
            for batch in rx {
                for piece in batch {
                    file.write_all(&piece)?;
                }
            }
            drop(file);
            if aborted_w.load(std::sync::atomic::Ordering::Acquire) {
                let _ = std::fs::remove_file(&tp);
                return Err(std::io::Error::other("upload aborted"));
            }
            if fp.exists() {
                let _ = std::fs::remove_file(&tp);
                return Ok(PutOutcome::AlreadyExists);
            }
            std::fs::rename(&tp, &fp)?;
            Ok(PutOutcome::Created)
        });
        let mut received: u64 = 0;
        let mut failure: Option<PutError> = None;
        let mut batch: Vec<Bytes> = Vec::new();
        let mut pending = 0usize;
        let mut small = BytesMut::new();
        while let Some(chunk) = body.next().await {
            match chunk {
                Ok(c) => {
                    received += c.len() as u64;
                    pending += c.len();
                    if c.len() >= FS_GATHER_CHUNK {
                        if !small.is_empty() {
                            batch.push(small.split().freeze());
                        }
                        batch.push(c);
                    } else {
                        small.extend_from_slice(&c);
                    }
                    if pending >= FS_WRITE_BLOCK {
                        if !small.is_empty() {
                            batch.push(small.split().freeze());
                        }
                        if tx.send(std::mem::take(&mut batch)).is_err() {
                            break;
                        }
                        pending = 0;
                    }
                }
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            }
        }
        if failure.is_none() {
            if !small.is_empty() {
                batch.push(small.freeze());
            }
            if !batch.is_empty() {
                let _ = tx.send(batch);
            }
        }
        if failure.is_none() {
            if let Some(expected) = expected_len {
                if received != expected {
                    failure = Some(PutError::BadBody(format!(
                        "announced {expected} bytes, received {received}"
                    )));
                }
            }
        }
        if failure.is_some() {
            aborted.store(true, std::sync::atomic::Ordering::Release);
        }
        drop(tx);
        let outcome = writer.await.map_err(|e| io_err(std::io::Error::other(e)));
        if let Some(e) = failure {
            // Only the temp file: `final_path` may be a valid artifact a concurrent PUT just
            // renamed into place.
            let _ = tokio::fs::remove_file(&tmp).await;
            return Err(e);
        }
        outcome?.map_err(|e| PutError::Store(io_err(e)))
    }

    /// One streaming `PUT` to the store through a presigned URL: the body leaves as it arrives,
    /// nothing is buffered. The upload context travels as provider metadata headers (signed on
    /// S3 and GCS; Azure's SAS signs no header), and the write is create-only where the provider
    /// supports it (`If-None-Match: *` on S3 and Azure, a generation match of 0 on GCS).
    async fn put_presigned<S>(
        &self,
        signer: &dyn Signer,
        key: &StorePath,
        len: u64,
        body: S,
        context: &RequestContext,
    ) -> Result<PutOutcome, PutError>
    where
        S: Stream<Item = Result<Bytes, PutError>> + Unpin + Send + 'static,
    {
        use axum::http::{header, HeaderMap, HeaderName, HeaderValue, Method};
        use std::sync::atomic::{AtomicU64, Ordering::Relaxed};

        let meta_prefix = match self.kind {
            "s3" => "x-amz-meta-",
            "gcs" => "x-goog-meta-",
            _ => "x-ms-meta-",
        };
        let mut signed = HeaderMap::new();
        let mut unsigned = HeaderMap::new();
        for (k, v) in context.iter() {
            let name = HeaderName::from_bytes(format!("{meta_prefix}{META_PREFIX}{k}").as_bytes());
            if let (Ok(name), Ok(value)) = (name, HeaderValue::from_str(v)) {
                if self.kind == "azure" {
                    unsigned.insert(name, value);
                } else {
                    signed.insert(name, value);
                }
            }
        }
        match self.kind {
            "s3" => {
                unsigned.insert(header::IF_NONE_MATCH, HeaderValue::from_static("*"));
            }
            "azure" => {
                unsigned.insert(
                    HeaderName::from_static("x-ms-blob-type"),
                    HeaderValue::from_static("BlockBlob"),
                );
                unsigned.insert(header::IF_NONE_MATCH, HeaderValue::from_static("*"));
            }
            _ => {
                signed.insert(
                    HeaderName::from_static("x-goog-if-generation-match"),
                    HeaderValue::from_static("0"),
                );
            }
        }
        let mut options = SignedUrlOptions::default();
        options.signed_headers = signed.clone();
        let url = signer
            .signed_url_opts(Method::PUT, key, Duration::from_secs(900), &options)
            .await?;

        let sent = Arc::new(AtomicU64::new(0));
        let counter = sent.clone();
        let stream = body.map(move |chunk| match chunk {
            Ok(c) => {
                counter.fetch_add(c.len() as u64, Relaxed);
                Ok::<Bytes, std::io::Error>(c)
            }
            Err(e) => Err(std::io::Error::other(format!("{e:?}"))),
        });
        let mut req = self
            .http
            .put(url)
            .header(header::CONTENT_LENGTH, len)
            .body(reqwest::Body::wrap_stream(stream));
        for (name, value) in signed.iter().chain(unsigned.iter()) {
            req = req.header(name, value);
        }
        let generic = |source: Box<dyn std::error::Error + Send + Sync>| {
            PutError::Store(StoreError::Generic {
                store: "presigned put",
                source,
            })
        };
        let res = match req.send().await {
            Ok(res) => res,
            Err(e) => {
                let received = sent.load(Relaxed);
                if received != len {
                    return Err(PutError::BadBody(format!("announced {len} bytes, received {received}")));
                }
                return Err(generic(Box::new(e)));
            }
        };
        let received = sent.load(Relaxed);
        if received != len {
            return Err(PutError::BadBody(format!("announced {len} bytes, received {received}")));
        }
        match res.status().as_u16() {
            200..=299 => Ok(PutOutcome::Created),
            409 | 412 => Ok(PutOutcome::AlreadyExists),
            code => {
                let text = res.text().await.unwrap_or_default();
                let text: String = text.chars().take(300).collect();
                Err(generic(format!("HTTP {code}: {text}").into()))
            }
        }
    }

    async fn put_inline(&self, key: &StorePath, data: Bytes, attributes: Attributes) -> Result<PutOutcome, PutError> {
        let opts = PutOptions {
            mode: PutMode::Create,
            attributes,
            ..Default::default()
        };
        match self.inner.put_opts(key, PutPayload::from(data), opts).await {
            Ok(_) => Ok(PutOutcome::Created),
            Err(StoreError::AlreadyExists { .. }) => Ok(PutOutcome::AlreadyExists),
            Err(e) => Err(e.into()),
        }
    }

    /// Retention pass: drop artifacts older than `ttl`, then the oldest ones until the prefix
    /// holds at most 90 % of `max_total_bytes`. Safe to run from several replicas at once.
    pub async fn sweep(&self, ttl: Option<Duration>, max_total_bytes: Option<u64>) -> Result<SweepReport, StoreError> {
        let started = std::time::Instant::now();
        let now = chrono::Utc::now();
        let mut entries: Vec<(StorePath, u64, chrono::DateTime<chrono::Utc>)> = Vec::new();
        let mut list = self.inner.list(Some(&self.prefix));
        while let Some(meta) = list.try_next().await? {
            if meta.location.filename().is_some_and(|f| f.starts_with('.')) {
                continue;
            }
            entries.push((meta.location, meta.size, meta.last_modified));
        }
        let scanned = entries.len() as u64;
        let mut total: u64 = entries.iter().map(|e| e.1).sum();
        let mut doomed: Vec<(StorePath, u64)> = Vec::new();

        if let Some(ttl) = ttl {
            let cutoff = now - chrono::Duration::from_std(ttl).unwrap_or(chrono::Duration::MAX);
            entries.retain(|(path, size, modified)| {
                if *modified < cutoff {
                    doomed.push((path.clone(), *size));
                    false
                } else {
                    true
                }
            });
        }
        if let Some(max) = max_total_bytes {
            let live: u64 = entries.iter().map(|e| e.1).sum();
            if live > max {
                let target = max / 10 * 9;
                entries.sort_by_key(|e| e.2);
                let mut remaining = live;
                for (path, size, _) in entries {
                    if remaining <= target {
                        break;
                    }
                    remaining -= size;
                    doomed.push((path, size));
                }
            }
        }

        let mut deleted = 0u64;
        let mut bytes_freed = 0u64;
        if !doomed.is_empty() {
            let sizes: std::collections::HashMap<StorePath, u64> = doomed.iter().cloned().collect();
            let paths = futures_util::stream::iter(doomed.into_iter().map(|(p, _)| Ok(p))).boxed();
            let mut results = self.inner.delete_stream(paths);
            while let Some(r) = results.next().await {
                match r {
                    Ok(path) => {
                        deleted += 1;
                        bytes_freed += sizes.get(&path).copied().unwrap_or(0);
                    }
                    Err(StoreError::NotFound { .. }) => {}
                    Err(e) => tracing::warn!(error = %e, "retention delete failed"),
                }
            }
            total = total.saturating_sub(bytes_freed);
        }
        Ok(SweepReport {
            at: now.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            scanned,
            deleted,
            bytes_freed,
            total_bytes: total,
            duration_ms: started.elapsed().as_millis() as u64,
        })
    }

    /// Readiness probe: the store answers (a missing probe object is a healthy answer).
    pub async fn probe(&self) -> Result<(), StoreError> {
        match self
            .inner
            .head(&self.prefix.clone().join(".nx-remote-cache-probe"))
            .await
        {
            Ok(_) | Err(StoreError::NotFound { .. }) => Ok(()),
            Err(e) => Err(e),
        }
    }
}

/// Synchronous exists + write + rename, run on the blocking pool.
fn write_whole(final_path: &std::path::Path, tmp: &std::path::Path, data: &[u8]) -> std::io::Result<PutOutcome> {
    if final_path.exists() {
        return Ok(PutOutcome::AlreadyExists);
    }
    std::fs::write(tmp, data)?;
    if final_path.exists() {
        let _ = std::fs::remove_file(tmp);
        return Ok(PutOutcome::AlreadyExists);
    }
    std::fs::rename(tmp, final_path)?;
    Ok(PutOutcome::Created)
}

fn io_err(e: std::io::Error) -> StoreError {
    StoreError::Generic {
        store: "filesystem",
        source: Box::new(e),
    }
}

#[derive(Clone, Debug, Default, serde::Serialize)]
pub struct SweepReport {
    pub at: String,
    pub scanned: u64,
    pub deleted: u64,
    pub bytes_freed: u64,
    pub total_bytes: u64,
    pub duration_ms: u64,
}

/// Strip credentials from a URL before showing it anywhere.
fn redact(url: &Url) -> String {
    let mut u = url.clone();
    let _ = u.set_password(None);
    let _ = u.set_username("");
    u.set_query(None);
    u.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::stream;

    fn body(chunks: Vec<&'static [u8]>) -> impl Stream<Item = Result<Bytes, PutError>> + Unpin {
        stream::iter(chunks.into_iter().map(|c| Ok(Bytes::from_static(c))))
    }

    async fn memory_store(inline: u64) -> CacheStore {
        let mut cfg = Config::for_tests("memory:///");
        cfg.inline_put_threshold = inline;
        CacheStore::open(&cfg).await.unwrap()
    }

    #[tokio::test]
    async fn inline_put_is_create_only() {
        let s = memory_store(1024).await;
        assert_eq!(
            s.put("h1", Some(3), body(vec![b"abc"]), &RequestContext::new())
                .await
                .unwrap(),
            PutOutcome::Created
        );
        assert_eq!(
            s.put("h1", Some(3), body(vec![b"abc"]), &RequestContext::new())
                .await
                .unwrap(),
            PutOutcome::AlreadyExists
        );
        assert!(s.exists("h1").await.unwrap());
        let got = s.get("h1").await.unwrap().unwrap().bytes().await.unwrap();
        assert_eq!(&got[..], b"abc");
        assert!(s.get("missing").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn inline_put_rejects_length_mismatch() {
        let s = memory_store(1024).await;
        let err = s
            .put("h2", Some(5), body(vec![b"abc"]), &RequestContext::new())
            .await
            .unwrap_err();
        assert!(matches!(err, PutError::BadBody(_)));
        assert!(!s.exists("h2").await.unwrap());
    }

    #[tokio::test]
    async fn multipart_put_roundtrip_and_conflict() {
        let s = memory_store(0).await; // force the multipart path
        let data: Vec<u8> = (0..(6 << 20)).map(|i| (i % 251) as u8).collect();
        let leaked: &'static [u8] = Box::leak(data.clone().into_boxed_slice());
        let chunks: Vec<&'static [u8]> = leaked.chunks(1 << 16).collect();
        assert_eq!(
            s.put(
                "big",
                Some(data.len() as u64),
                body(chunks.clone()),
                &RequestContext::new()
            )
            .await
            .unwrap(),
            PutOutcome::Created
        );
        assert_eq!(
            s.put("big", None, body(chunks), &RequestContext::new()).await.unwrap(),
            PutOutcome::AlreadyExists
        );
        let got = s.get("big").await.unwrap().unwrap().bytes().await.unwrap();
        assert_eq!(got.len(), data.len());
        assert_eq!(&got[..], &data[..]);
    }

    #[tokio::test]
    async fn filesystem_store_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let url = Url::from_directory_path(dir.path()).unwrap();
        let cfg = Config::for_tests(url.as_str());
        let s = CacheStore::open(&cfg).await.unwrap();
        assert_eq!(s.kind, "filesystem");
        s.put("fs1", Some(3), body(vec![b"xyz"]), &RequestContext::new())
            .await
            .unwrap();
        assert_eq!(&s.get("fs1").await.unwrap().unwrap().bytes().await.unwrap()[..], b"xyz");
        s.probe().await.unwrap();
    }

    #[tokio::test]
    async fn sweep_enforces_size_cap_and_ignores_probe_keys() {
        let s = memory_store(1024).await;
        for (i, size) in [(0, 100), (1, 100), (2, 100), (3, 100)] {
            let data: &'static [u8] = Box::leak(vec![i as u8; size].into_boxed_slice());
            s.put(
                &format!("h{i}"),
                Some(size as u64),
                body(vec![data]),
                &RequestContext::new(),
            )
            .await
            .unwrap();
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        let report = s.sweep(None, Some(250)).await.unwrap();
        assert_eq!(report.scanned, 4);
        assert_eq!(report.deleted, 2, "{report:?}");
        assert_eq!(report.total_bytes, 200);
        assert!(!s.exists("h0").await.unwrap());
        assert!(!s.exists("h1").await.unwrap());
        assert!(s.exists("h3").await.unwrap());

        let report = s.sweep(Some(Duration::ZERO), None).await.unwrap();
        assert_eq!(report.deleted, 2);
        assert_eq!(report.total_bytes, 0);
        let report = s.sweep(Some(Duration::from_secs(3600)), Some(1 << 30)).await.unwrap();
        assert_eq!((report.scanned, report.deleted), (0, 0));
    }

    #[tokio::test]
    async fn filesystem_fast_path_is_create_only_and_streams() {
        let dir = tempfile::tempdir().unwrap();
        let url = Url::from_directory_path(dir.path()).unwrap();
        let s = CacheStore::open(&Config::for_tests(url.as_str())).await.unwrap();
        let data: Vec<u8> = (0..(3 << 20)).map(|i| (i % 199) as u8).collect();
        let leaked: &'static [u8] = Box::leak(data.clone().into_boxed_slice());
        let chunks: Vec<&'static [u8]> = leaked.chunks(64 << 10).collect();
        let ctx = RequestContext::new();
        assert_eq!(
            s.put("big", Some(data.len() as u64), body(chunks.clone()), &ctx)
                .await
                .unwrap(),
            PutOutcome::Created
        );
        assert_eq!(
            s.put("big", None, body(chunks), &ctx).await.unwrap(),
            PutOutcome::AlreadyExists
        );
        let got = s.get("big").await.unwrap().unwrap();
        assert_eq!(got.meta.size, data.len() as u64);
        assert_eq!(&got.bytes().await.unwrap()[..], &data[..]);
        // A truncated upload leaves nothing behind.
        let err = s.put("short", Some(10), body(vec![b"abc"]), &ctx).await.unwrap_err();
        assert!(matches!(err, PutError::BadBody(_)));
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(leftovers.len(), 1, "{leftovers:?}");
        assert!(!s.exists("short").await.unwrap());
        assert!(s.get("missing").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn filesystem_streamed_failure_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let url = Url::from_directory_path(dir.path()).unwrap();
        let s = CacheStore::open(&Config::for_tests(url.as_str())).await.unwrap();
        let ctx = RequestContext::new();
        let chunk: &'static [u8] = Box::leak(vec![7u8; 256 << 10].into_boxed_slice());
        // Above FS_GATHER_MAX, so the body streams to the writer thread; then the body fails.
        let failing = stream::iter(vec![
            Ok(Bytes::from_static(chunk)),
            Ok(Bytes::from_static(chunk)),
            Err(PutError::BadBody("client went away".into())),
        ]);
        let err = s.put("cut", Some(4 << 20), failing, &ctx).await.unwrap_err();
        assert!(matches!(err, PutError::BadBody(_)));
        // A body shorter than announced is the same story.
        let short = stream::iter(vec![Ok(Bytes::from_static(chunk)), Ok(Bytes::from_static(chunk))]);
        let err = s.put("short", Some(4 << 20), short, &ctx).await.unwrap_err();
        assert!(matches!(err, PutError::BadBody(_)));
        // No truncated artifact was ever renamed into place, no temp file lingers.
        assert!(!s.exists("cut").await.unwrap());
        assert!(!s.exists("short").await.unwrap());
        let leftovers: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[tokio::test]
    async fn metadata_roundtrip_on_object_stores() {
        let s = memory_store(1024).await;
        let ctx: RequestContext = [("run", "42"), ("repo", "org/app")]
            .into_iter()
            .map(|(k, v)| (k.into(), v.into()))
            .collect();
        s.put("meta", Some(3), body(vec![b"abc"]), &ctx).await.unwrap();
        let got = s.get("meta").await.unwrap().unwrap();
        assert_eq!(origin_from(&got.attributes), ctx);
    }

    #[test]
    fn redacts_credentials() {
        let u = Url::parse("s3://user:pw@bucket/prefix?x=1").unwrap();
        assert_eq!(redact(&u), "s3://bucket/prefix");
    }
}
