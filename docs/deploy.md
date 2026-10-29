# Deploying and choosing a store

The server is one container with one required setting, `NX_CACHE_STORE_URL`. This page walks through a Kubernetes deployment on Azure Blob, then lists every store and how retention works. Every variable is in [`configuration.md`](configuration.md).

## Helm

The chart in [`charts/nx-remote-cache/`](../charts/nx-remote-cache/README.md) is built on the Bitnami
`common` library and published as an OCI artifact on every release, one chart version per server
version. Tokens are generated on first install and kept across upgrades (or bring your own secret
with `auth.existingSecret`).

```bash
helm install nx-remote-cache oci://ghcr.io/misterpandapooh/charts/nx-remote-cache --version 0.1.0 \
  --namespace nx-remote-cache --create-namespace \
  --set storeUrl=az://nx-cache/ --set env.AZURE_STORAGE_ACCOUNT_NAME=mystorageaccount \
  --set 'podLabels.azure\.workload\.identity/use=true' \
  --set 'serviceAccount.annotations.azure\.workload\.identity/client-id=<client id>' \
  --set ingress.enabled=true --set ingress.hostname=nx-cache.example.com --set ingress.tls=true
```

Images come from `ghcr.io/misterpandapooh/nx-remote-cache` by default; `--set image.registry=docker.io`
switches to the Docker Hub mirror. The plain manifests below do the same without Helm.

## Deploy on Kubernetes with Azure Blob

1. Create a storage account and a container (`az storage container create -n nx-cache ...`).
2. Give the pod an identity. With AKS Workload Identity the pod receives `AZURE_CLIENT_ID`,
   `AZURE_TENANT_ID` and `AZURE_FEDERATED_TOKEN_FILE` automatically; grant that identity
   *Storage Blob Data Contributor* on the container. No key leaves Azure. (Managed identity, SAS
   tokens and account keys work too, see [Stores](#stores).)
3. Apply the manifests, after editing the account name and the secret:

```bash
kubectl create namespace nx-remote-cache
kubectl -n nx-remote-cache create secret generic nx-remote-cache \
  --from-literal=NX_CACHE_TOKEN="$(openssl rand -hex 32)" \
  --from-literal=NX_CACHE_READ_TOKEN="$(openssl rand -hex 32)"
kubectl apply -k deploy/k8s
```

`deploy/k8s/` holds a Deployment (2 replicas, probes, non-root, read-only root FS), Service,
PodDisruptionBudget and an Ingress example with the body-size and timeout settings big artifacts
need. Terminate TLS at the ingress: the Nx client only trusts the OS certificate store.

For the dashboard behind your SSO (Keycloak, SAML, Kong) see [`deploy/sso/`](../deploy/sso/README.md).
To try the Azure path locally, `docker compose -f deploy/docker-compose.yml up` starts the server
against [Azurite](https://github.com/Azure/Azurite).

## Stores

`NX_CACHE_STORE_URL` picks the backend; credentials come from the provider's usual environment
variables (read by [`object_store`](https://docs.rs/object_store)).

| Store | URL | Credentials |
|---|---|---|
| Azure Blob | `az://<container>/<prefix>` | `AZURE_STORAGE_ACCOUNT_NAME` + one of: Workload Identity (`AZURE_CLIENT_ID`, `AZURE_TENANT_ID`, `AZURE_FEDERATED_TOKEN_FILE`), managed identity (nothing), `AZURE_STORAGE_SAS_TOKEN`, `AZURE_STORAGE_ACCOUNT_KEY`, `AZURE_STORAGE_CLIENT_SECRET`. Azurite: `AZURE_STORAGE_USE_EMULATOR=true` (+ `AZURITE_BLOB_STORAGE_URL`). |
| S3 / MinIO / R2 | `s3://<bucket>/<prefix>` | `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_REGION`, optional `AWS_ENDPOINT` (+ `AWS_ALLOW_HTTP=true`), IRSA / instance profile |
| Google Cloud Storage | `gs://<bucket>/<prefix>` | `GOOGLE_SERVICE_ACCOUNT` / `GOOGLE_APPLICATION_CREDENTIALS`, or workload identity |
| Local disk | `file:///data/nx-cache` | none; mount a volume, `fsGroup: 65534` |
| Memory | `memory:///` | none; tests and demos |

Artifacts are stored as `<prefix>/<hash>`, immutable. On object stores the uploading CI run is
recorded as metadata (`nx_run`, `nx_repo`, ...), see [CI runs](ci.md#link-artifacts-to-ci-runs).

## Retention

Recommended: a lifecycle policy on the bucket, it costs nothing and scales to millions of blobs.
[`deploy/retention/`](../deploy/retention/README.md) has ready-made policies for Azure (true LRU with
last-access tracking), S3 and GCS. The built-in sweeper (`NX_CACHE_TTL`, `NX_CACHE_MAX_TOTAL_BYTES`)
adds what providers do not offer: a hard size budget, retention on local disk, TTLs under a day.
Deleting is always safe: a missing artifact is a cache miss and Nx rebuilds the task.
