# nx-remote-cache Helm chart

Installs [nx-remote-cache](https://github.com/MisterPandaPooh/nx-remote-cache), a free, self-hosted
remote cache server for Nx. Built on the [Bitnami `common`](https://github.com/bitnami/charts/tree/main/bitnami/common)
library (so `global.imageRegistry`, `commonLabels`, `resourcesPreset`, `podAntiAffinityPreset`,
`extraDeploy` and friends work as in any Bitnami chart) with typed values for the product: pick a
store and its authentication, switch the memory tier, the key-value store, retention, dashboard and
logging. Published as an OCI chart on every release; the chart version equals the server version.

```bash
helm install nx-remote-cache oci://ghcr.io/misterpandapooh/charts/nx-remote-cache \
  --namespace nx-remote-cache --create-namespace \
  --set store.type=azure --set store.azure.accountName=mystorageaccount \
  --set store.azure.workloadIdentity.clientId=<managed identity client id> \
  --set ingress.enabled=true --set ingress.hostname=nx-cache.example.com --set ingress.tls=true
```

Tokens are generated on first install and kept across upgrades; the install notes print how to
read them back. Pin a version with `--version 0.1.0`.

## Stores

| `store.type` | Required | `auth` options | What the chart does |
|---|---|---|---|
| `azure` | `store.azure.accountName`, `container` (default `nx-cache`) | `workloadIdentity` (default; `workloadIdentity.clientId`), `managedIdentity` (`managedIdentity.clientId` for user-assigned), `accountKey`, `sasToken`, `servicePrincipal`, `emulator` (Azurite, `emulator.endpoint`) | Annotates the ServiceAccount with `azure.workload.identity/client-id` and labels the pods `azure.workload.identity/use`; keys and secrets go to a chart-managed secret, or use `existingSecret` (keys `AZURE_STORAGE_ACCOUNT_KEY`, `AZURE_STORAGE_SAS_TOKEN`, `AZURE_CLIENT_SECRET`). |
| `s3` | `store.s3.bucket`, `region`; `endpoint` + `allowHttp` for MinIO / R2 | `irsa` (default; `irsa.roleArn`), `instanceProfile`, `accessKey` (`accessKeyId`, `secretAccessKey`, `sessionToken`) | Annotates the ServiceAccount with `eks.amazonaws.com/role-arn`; keys to a secret or `existingSecret` (`AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_SESSION_TOKEN`). |
| `gcs` | `store.gcs.bucket` | `workloadIdentity` (default; `workloadIdentity.gcpServiceAccount`), `serviceAccountKey` (`serviceAccountKey` JSON, or `existingSecret` with `GOOGLE_SERVICE_ACCOUNT_KEY`) | Annotates the ServiceAccount with `iam.gke.io/gcp-service-account`. |
| `filesystem` | `persistence.enabled: true` | | A PVC (or `persistence.existingClaim`) mounted at `persistence.mountPath`. |
| `memory` | `replicaCount: 1` | | Demos and tests; artifacts vanish with the pod. |

`store.prefix` adds a folder inside the container or bucket; `store.url` sets the full store URL by
hand for anything unusual. Missing or inconsistent settings fail at `helm install` with a message
naming the value.

## Server

| Key | Default | Description |
|---|---|---|
| `auth.enabled` | `true` | `false` sets `NX_CACHE_ENABLE_LOGIN=false`: no token for the cache API, no login screen. Private networks only: any build that can reach it can poison it (CREEP, see SECURITY.md). |
| `auth.token`, `auth.readToken` | generated | Read-write and read-only (pull requests) tokens; empty values are generated once and reused on upgrade. |
| `auth.existingSecret` | `""` | Your own secret; `auth.existingSecretTokenKey` / `auth.existingSecretReadTokenKey` name its entries. |
| `server.uploads.*` | 2GB max, 8MB inline, 8MB × 4 multipart, 16 in flight | Upload limits; memory for uploads is bounded by `maxConcurrentPuts × max(inlineThreshold, multipartChunkSize × multipartConcurrency)`. |
| `server.memoryTier.*` | on, 256MB, 4MB per item | Hot artifacts served from RAM. |
| `server.kv.*` | on, `memory` backend, 64MB, 1MB per value | The `/v1/kv` key-value store. `backend: redis` with `redis.url` (or `redis.existingSecret` holding `NX_CACHE_KV_REDIS_URL`) shares keys between replicas; `memory` is per pod. |
| `redis.enabled` | `false` | Deploy a Redis with the [Bitnami redis subchart](https://github.com/bitnami/charts/tree/main/bitnami/redis) and point the key-value store at it (URL and password wired automatically; the password is generated and kept across upgrades). Every other `redis.*` value goes to the subchart: `architecture: standalone`, `auth.enabled`, `master.persistence`, ... |
| `server.github.*` | on, github.com | Run and repository context shown as GitHub links in the dashboard; `url` for GitHub Enterprise. GitHub is the only CI integration so far. |
| `server.retention.*` | off | `ttl` (`30d`), `maxTotalSize` (`500GB`), `sweepInterval` (`1h`). |
| `server.dashboard.*` | on | `token` or `existingSecret`/`existingSecretKey` for a dedicated dashboard token, `trustedHeader` for an SSO proxy. |
| `server.logging.*` | `info`, `json` | `level`, `format` (`json`, `pretty`, `compact`), `accessLog`, `filter` (`RUST_LOG`). |
| `server.shutdownGraceSeconds` | `20` | |
| `server.extraEnv`, `extraEnvVars`, `extraEnvVarsCM`, `extraEnvVarsSecret` | | Anything else, as `NAME: value`, raw env entries, or `envFrom` sources. |

## Kubernetes

| Key | Default | Description |
|---|---|---|
| `image.registry`, `image.repository`, `image.tag`, `image.digest` | ghcr.io, `misterpandapooh/nx-remote-cache`, chart appVersion | Docker Hub mirror: `image.registry=docker.io`. |
| `replicaCount`, `updateStrategy`, `podAntiAffinityPreset` | `2`, surge 1 / unavailable 0, `soft` | |
| `resourcesPreset`, `resources` | `small`, `{}` | Bitnami preset or explicit resources; 1Gi of memory is comfortable with the upload defaults. |
| `persistence.*` | disabled | PVC for the filesystem store. |
| `service.*` | ClusterIP :80, Prometheus annotations | |
| `ingress.*` | disabled | `hostname`, `path`, `tls` (secret `<hostname>-tls`, cert-manager annotations honoured), `extraHosts`, `extraTls`; nginx annotations preset for large bodies and long timeouts. |
| `pdb.*`, `autoscaling.*` | `minAvailable: 1`, disabled | |
| `serviceAccount.*` | created, token not mounted | Cloud identity annotations come from `store.*`; add your own in `annotations`. |
| `metrics.serviceMonitor.*` | disabled | Prometheus Operator ServiceMonitor on `/metrics`. |
| `podSecurityContext`, `containerSecurityContext` | non-root 65534, read-only FS, no capabilities, seccomp | Adapted for OpenShift by `global.compatibility.openshift.adaptSecurityContext`. |
| `extraDeploy`, `extraVolumes`, `extraVolumeMounts`, `initContainers`, `sidecars` | `[]` | |

The server speaks plain HTTP (h1, h2c); TLS is the ingress's job. Several replicas share the
store but keep their own in-memory statistics and `/v1/kv` contents.
