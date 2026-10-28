use std::sync::Arc;
use std::time::Duration;

use anyhow::{anyhow, bail, Context, Result};
use bytes::{Bytes, BytesMut};
use futures_util::{Stream, StreamExt};
use object_store::aws::AmazonS3Builder;
use object_store::azure::MicrosoftAzureBuilder;
use object_store::gcp::GoogleCloudStorageBuilder;
use object_store::local::LocalFileSystem;
use object_store::memory::InMemory;
use object_store::path::Path as StorePath;
use object_store::{
    BackoffConfig, ClientOptions, Error as StoreError, GetResult, ObjectStore, ObjectStoreExt, ObjectStoreScheme,
    PutMode, PutOptions, PutPayload, RetryConfig, WriteMultipart,
};
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
    pub kind: &'static str,
    pub location: String,
    inline_threshold: u64,
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

        let (inner, kind, prefix): (Arc<dyn ObjectStore>, &'static str, StorePath) = match scheme {
            ObjectStoreScheme::Local => {
                let dir = url
                    .to_file_path()
                    .map_err(|_| anyhow!("file:// store URL must be an absolute path"))?;
                tokio::fs::create_dir_all(&dir)
                    .await
                    .with_context(|| format!("creating {}", dir.display()))?;
                let fs = LocalFileSystem::new_with_prefix(&dir)?.with_automatic_cleanup(true);
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
                (Arc::new(store), "azure", prefix)
            }
            ObjectStoreScheme::AmazonS3 => {
                let store = AmazonS3Builder::from_env()
                    .with_url(url.as_str())
                    .with_client_options(client)
                    .with_retry(retry)
                    .build()
                    .context("building the S3 client")?;
                (Arc::new(store), "s3", prefix)
            }
            ObjectStoreScheme::GoogleCloudStorage => {
                let store = GoogleCloudStorageBuilder::from_env()
                    .with_url(url.as_str())
                    .with_client_options(client)
                    .with_retry(retry)
                    .build()
                    .context("building the GCS client")?;
                (Arc::new(store), "gcs", prefix)
            }
            other => bail!("store scheme {other:?} is not supported"),
        };

        Ok(Self {
            inner,
            prefix,
            kind,
            location: redact(url),
            inline_threshold: cfg.inline_put_threshold,
            chunk_bytes: cfg.multipart_chunk_bytes,
            concurrency: cfg.multipart_concurrency,
        })
    }

    fn key(&self, hash: &str) -> StorePath {
        self.prefix.clone().join(hash)
    }

    pub async fn exists(&self, hash: &str) -> Result<bool, StoreError> {
        match self.inner.head(&self.key(hash)).await {
            Ok(_) => Ok(true),
            Err(StoreError::NotFound { .. }) => Ok(false),
            Err(e) => Err(e),
        }
    }

    pub async fn get(&self, hash: &str) -> Result<Option<GetResult>, StoreError> {
        match self.inner.get(&self.key(hash)).await {
            Ok(r) => Ok(Some(r)),
            Err(StoreError::NotFound { .. }) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Write an artifact. Small bodies use one atomic create; large ones stream through a
    /// multipart upload. `expected_len` is the announced Content-Length, when known.
    pub async fn put<S>(&self, hash: &str, expected_len: Option<u64>, mut body: S) -> Result<PutOutcome, PutError>
    where
        S: Stream<Item = Result<Bytes, PutError>> + Unpin,
    {
        let key = self.key(hash);
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
            return self.put_inline(&key, buf.freeze()).await;
        }

        // Multipart: cheap existence check first so a duplicate is not uploaded to the store.
        // The body is still read to the end so the client reliably receives the 409.
        if self.inner.head(&key).await.is_ok() {
            while let Some(chunk) = body.next().await {
                chunk?;
            }
            return Ok(PutOutcome::AlreadyExists);
        }
        let upload = self.inner.put_multipart(&key).await?;
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

    async fn put_inline(&self, key: &StorePath, data: Bytes) -> Result<PutOutcome, PutError> {
        let opts = PutOptions {
            mode: PutMode::Create,
            ..Default::default()
        };
        match self.inner.put_opts(key, PutPayload::from(data), opts).await {
            Ok(_) => Ok(PutOutcome::Created),
            Err(StoreError::AlreadyExists { .. }) => Ok(PutOutcome::AlreadyExists),
            Err(e) => Err(e.into()),
        }
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
            s.put("h1", Some(3), body(vec![b"abc"])).await.unwrap(),
            PutOutcome::Created
        );
        assert_eq!(
            s.put("h1", Some(3), body(vec![b"abc"])).await.unwrap(),
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
        let err = s.put("h2", Some(5), body(vec![b"abc"])).await.unwrap_err();
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
            s.put("big", Some(data.len() as u64), body(chunks.clone()))
                .await
                .unwrap(),
            PutOutcome::Created
        );
        assert_eq!(
            s.put("big", None, body(chunks)).await.unwrap(),
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
        s.put("fs1", Some(3), body(vec![b"xyz"])).await.unwrap();
        assert_eq!(&s.get("fs1").await.unwrap().unwrap().bytes().await.unwrap()[..], b"xyz");
        s.probe().await.unwrap();
    }

    #[test]
    fn redacts_credentials() {
        let u = Url::parse("s3://user:pw@bucket/prefix?x=1").unwrap();
        assert_eq!(redact(&u), "s3://bucket/prefix");
    }
}
