# nx-remote-cache Helm chart

Installs [nx-remote-cache](https://github.com/MisterPandaPooh/nx-remote-cache), a free, self-hosted
remote cache server for Nx. Built on the [Bitnami `common`](https://github.com/bitnami/charts/tree/main/bitnami/common)
library, so values follow the Bitnami layout (`global.imageRegistry`, `commonLabels`,
`extraEnvVars`, `resourcesPreset`, `podAntiAffinityPreset`, ...). Published as an OCI chart on
every release; the chart version always equals the server version.

```bash
helm install nx-remote-cache oci://ghcr.io/misterpandapooh/charts/nx-remote-cache \
  --namespace nx-remote-cache --create-namespace \
  --set storeUrl=az://nx-cache/ \
  --set env.AZURE_STORAGE_ACCOUNT_NAME=mystorageaccount \
  --set 'podLabels.azure\.workload\.identity/use=true' \
  --set 'serviceAccount.annotations.azure\.workload\.identity/client-id=<client id>' \
  --set ingress.enabled=true --set ingress.hostname=nx-cache.example.com --set ingress.tls=true
```

Tokens are generated on first install and kept across upgrades; read them back with the command
printed after install, or bring your own secret with `auth.existingSecret`. Pin a version with
`--version 0.1.0`.

## Values

| Key | Default | Description |
|---|---|---|
| `storeUrl` | required | `az://`, `s3://`, `gs://`, `file:///data` (with `persistence.enabled`) or `memory:///`. |
| `auth.enabled` | `true` | `false` runs without tokens (private networks only). |
| `auth.token`, `auth.readToken` | generated | Read-write and read-only tokens. Empty values are generated once (48 chars) and reused on upgrade. |
| `auth.existingSecret` | `""` | Your own secret; `auth.existingSecretTokenKey` / `auth.existingSecretReadTokenKey` name its entries. |
| `env` | limits, logging | Plain environment variables: store credentials, limits, retention, logging ([reference](../../docs/configuration.md)). |
| `extraEnvVars`, `extraEnvVarsCM`, `extraEnvVarsSecret` | | Raw env entries, or a ConfigMap / Secret loaded with `envFrom` (cloud credentials). |
| `image.registry`, `image.repository`, `image.tag`, `image.digest` | ghcr.io, `misterpandapooh/nx-remote-cache`, chart appVersion | Docker Hub mirror: `registry: docker.io`. |
| `global.imageRegistry`, `global.imagePullSecrets`, `global.storageClass` | | Usual Bitnami globals. |
| `replicaCount`, `updateStrategy`, `podAntiAffinityPreset` | `2`, surge 1 / unavailable 0, `soft` | |
| `resourcesPreset`, `resources` | `small`, `{}` | Bitnami preset or explicit resources. Upload memory is bounded by `NX_CACHE_MAX_CONCURRENT_PUTS` × 32 MB plus the memory tier. |
| `persistence.*` | disabled | PVC mounted at `/data` for the local-disk store. |
| `service.*` | ClusterIP :80, Prometheus annotations | |
| `ingress.*` | disabled | `hostname`, `path`, `tls` (secret `<hostname>-tls`, cert-manager annotations honoured), `extraHosts`, `extraTls`; nginx annotations preset for large bodies. |
| `pdb.*`, `autoscaling.*` | `minAvailable: 1`, disabled | |
| `serviceAccount.*` | created, token not mounted | Annotate for AKS Workload Identity or EKS IRSA. |
| `metrics.serviceMonitor.*` | disabled | Prometheus Operator ServiceMonitor on `/metrics`. |
| `podSecurityContext`, `containerSecurityContext` | non-root 65534, read-only FS, no capabilities, seccomp | Adapted for OpenShift by `global.compatibility.openshift.adaptSecurityContext`. |
| `extraDeploy`, `extraVolumes`, `extraVolumeMounts`, `initContainers`, `sidecars` | `[]` | |

The server speaks plain HTTP (h1, h2c); TLS is the ingress's job. Several replicas share the
store but keep their own in-memory statistics and `/v1/kv` contents.
