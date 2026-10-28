# Retention

Two layers, use one or both.

## 1. Store lifecycle policy (recommended)

Runs inside the cloud provider: no listing, no server CPU, scales to millions of blobs.

| Store | File | Apply | Notes |
|---|---|---|---|
| Azure Blob | `azure-lifecycle.json` | `./azure-apply.sh <rg> <account>` | True LRU: deletes blobs not **read** for 30 days (last-access tracking is enabled by the script). Lifecycle runs about once a day. |
| S3 | `s3-lifecycle.json` | `aws s3api put-bucket-lifecycle-configuration --bucket <b> --lifecycle-configuration file://s3-lifecycle.json` | Age since creation only (S3 has no last-access rule). Also aborts stale multipart uploads. |
| GCS | `gcs-lifecycle.json` | `gcloud storage buckets update gs://<b> --lifecycle-file=gcs-lifecycle.json` | Age since creation. |

Adjust the prefix to the path part of `NX_CACHE_STORE_URL` (for `az://nx-cache/` the container
is `nx-cache` and the prefix filter can be dropped).

Deleting is always safe: a missing artifact is a cache miss, Nx rebuilds the task and uploads it again.

## 2. Built-in sweeper (optional)

Set any of these on the server and a background pass runs every `NX_CACHE_SWEEP_INTERVAL` (default 1h):

| Variable | Effect |
|---|---|
| `NX_CACHE_TTL=30d` | delete artifacts whose store modification time is older than the TTL (creation time, since artifacts are immutable) |
| `NX_CACHE_MAX_TOTAL_BYTES=500GB` | when the prefix exceeds the cap, delete oldest-first down to 90 % of it |

Use it for a hard size budget (no provider offers one), for local disk or MinIO without ILM, or for
TTLs shorter than a day. It lists the whole prefix each pass (5 000 objects per request on Azure/S3),
so budget roughly one second per 50 000 artifacts. Several replicas may sweep at once; deletes are idempotent.
The last pass is visible in the dashboard, `/api/v1/stats` (`retention`) and `/metrics`
(`nx_cache_retention_*`, `nx_cache_store_bytes`).
