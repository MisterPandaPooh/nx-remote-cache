{{/* vim: set filetype=mustache: */}}

{{/* Image reference, honouring global.imageRegistry and image.digest. */}}
{{- define "nx-remote-cache.image" -}}
{{- include "common.images.image" (dict "imageRoot" .Values.image "global" .Values.global) -}}
{{- end -}}

{{- define "nx-remote-cache.imagePullSecrets" -}}
{{- include "common.images.renderPullSecrets" (dict "images" (list .Values.image) "context" $) -}}
{{- end -}}

{{- define "nx-remote-cache.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "common.names.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{/* ---------- tokens ---------- */}}

{{- define "nx-remote-cache.secretName" -}}
{{- include "common.secrets.name" (dict "existingSecret" .Values.auth.existingSecret "defaultNameSuffix" "tokens" "context" $) -}}
{{- end -}}

{{- define "nx-remote-cache.tokenKey" -}}
{{- ternary .Values.auth.existingSecretTokenKey "NX_CACHE_TOKEN" (not (empty .Values.auth.existingSecret)) -}}
{{- end -}}

{{- define "nx-remote-cache.readTokenKey" -}}
{{- ternary .Values.auth.existingSecretReadTokenKey "NX_CACHE_READ_TOKEN" (not (empty .Values.auth.existingSecret)) -}}
{{- end -}}

{{/* ---------- store ---------- */}}

{{/* The store URL the server gets: typed settings, or store.url verbatim. */}}
{{- define "nx-remote-cache.storeUrl" -}}
{{- $s := .Values.store -}}
{{- $prefix := trimAll "/" (default "" $s.prefix) -}}
{{- if $s.url -}}
{{- $s.url -}}
{{- else if eq $s.type "azure" -}}
{{- printf "az://%s/%s" $s.azure.container $prefix -}}
{{- else if eq $s.type "s3" -}}
{{- printf "s3://%s/%s" $s.s3.bucket $prefix -}}
{{- else if eq $s.type "gcs" -}}
{{- printf "gs://%s/%s" $s.gcs.bucket $prefix -}}
{{- else if eq $s.type "filesystem" -}}
{{- printf "file://%s" .Values.persistence.mountPath -}}
{{- else -}}
memory:///
{{- end -}}
{{- end -}}

{{/* Secret holding store credentials: the user's, or the one this chart writes from inline values. */}}
{{- define "nx-remote-cache.storeSecretName" -}}
{{- $s := .Values.store -}}
{{- $existing := "" -}}
{{- if eq $s.type "azure" }}{{ $existing = $s.azure.existingSecret }}{{ end -}}
{{- if eq $s.type "s3" }}{{ $existing = $s.s3.existingSecret }}{{ end -}}
{{- if eq $s.type "gcs" }}{{ $existing = $s.gcs.existingSecret }}{{ end -}}
{{- include "common.secrets.name" (dict "existingSecret" $existing "defaultNameSuffix" "store" "context" $) -}}
{{- end -}}

{{/* Inline credentials to write into the chart-managed store secret, as key: value (empty when none). */}}
{{- define "nx-remote-cache.storeSecretData" -}}
{{- $s := .Values.store -}}
{{- $data := dict -}}
{{- if and (eq $s.type "azure") (not $s.azure.existingSecret) -}}
{{- if and (eq $s.azure.auth "accountKey") $s.azure.accountKey }}{{ $_ := set $data "AZURE_STORAGE_ACCOUNT_KEY" $s.azure.accountKey }}{{ end -}}
{{- if and (eq $s.azure.auth "sasToken") $s.azure.sasToken }}{{ $_ := set $data "AZURE_STORAGE_SAS_TOKEN" $s.azure.sasToken }}{{ end -}}
{{- if and (eq $s.azure.auth "servicePrincipal") $s.azure.servicePrincipal.clientSecret }}{{ $_ := set $data "AZURE_CLIENT_SECRET" $s.azure.servicePrincipal.clientSecret }}{{ end -}}
{{- end -}}
{{- if and (eq $s.type "s3") (not $s.s3.existingSecret) (eq $s.s3.auth "accessKey") -}}
{{- if $s.s3.accessKeyId }}{{ $_ := set $data "AWS_ACCESS_KEY_ID" $s.s3.accessKeyId }}{{ end -}}
{{- if $s.s3.secretAccessKey }}{{ $_ := set $data "AWS_SECRET_ACCESS_KEY" $s.s3.secretAccessKey }}{{ end -}}
{{- if $s.s3.sessionToken }}{{ $_ := set $data "AWS_SESSION_TOKEN" $s.s3.sessionToken }}{{ end -}}
{{- end -}}
{{- if and (eq $s.type "gcs") (not $s.gcs.existingSecret) (eq $s.gcs.auth "serviceAccountKey") $s.gcs.serviceAccountKey -}}
{{- $_ := set $data "GOOGLE_SERVICE_ACCOUNT_KEY" $s.gcs.serviceAccountKey -}}
{{- end -}}
{{- toYaml $data -}}
{{- end -}}

{{/* One env entry read from the store secret. */}}
{{- define "nx-remote-cache.storeSecretEnv" -}}
- name: {{ .key }}
  valueFrom:
    secretKeyRef:
      name: {{ include "nx-remote-cache.storeSecretName" .context }}
      key: {{ .key }}
      {{- if .optional }}
      optional: true
      {{- end }}
{{- end -}}

{{/* Environment for the selected store: URL, client settings, provider credentials. */}}
{{- define "nx-remote-cache.storeEnv" -}}
{{- $s := .Values.store }}
- name: NX_CACHE_STORE_URL
  value: {{ include "nx-remote-cache.storeUrl" . | quote }}
- name: NX_CACHE_STORE_TIMEOUT_SECS
  value: {{ $s.timeoutSeconds | quote }}
- name: NX_CACHE_STORE_CONNECT_TIMEOUT_SECS
  value: {{ $s.connectTimeoutSeconds | quote }}
- name: NX_CACHE_STORE_MAX_RETRIES
  value: {{ $s.maxRetries | quote }}
{{- if eq $s.type "azure" }}
{{- $a := $s.azure }}
{{- if eq $a.auth "emulator" }}
- name: AZURE_STORAGE_USE_EMULATOR
  value: "true"
- name: AZURE_STORAGE_ENDPOINT
  value: {{ $a.emulator.endpoint | quote }}
- name: AZURE_STORAGE_ACCOUNT_NAME
  value: {{ $a.accountName | default "devstoreaccount1" | quote }}
{{- else }}
- name: AZURE_STORAGE_ACCOUNT_NAME
  value: {{ $a.accountName | quote }}
{{- end }}
{{- if and (eq $a.auth "managedIdentity") $a.managedIdentity.clientId }}
- name: AZURE_CLIENT_ID
  value: {{ $a.managedIdentity.clientId | quote }}
{{- end }}
{{- if eq $a.auth "accountKey" }}
{{- include "nx-remote-cache.storeSecretEnv" (dict "key" "AZURE_STORAGE_ACCOUNT_KEY" "context" $) | nindent 0 }}
{{- end }}
{{- if eq $a.auth "sasToken" }}
{{- include "nx-remote-cache.storeSecretEnv" (dict "key" "AZURE_STORAGE_SAS_TOKEN" "context" $) | nindent 0 }}
{{- end }}
{{- if eq $a.auth "servicePrincipal" }}
- name: AZURE_CLIENT_ID
  value: {{ $a.servicePrincipal.clientId | quote }}
- name: AZURE_TENANT_ID
  value: {{ $a.servicePrincipal.tenantId | quote }}
{{- include "nx-remote-cache.storeSecretEnv" (dict "key" "AZURE_CLIENT_SECRET" "context" $) | nindent 0 }}
{{- end }}
{{- end }}
{{- if eq $s.type "s3" }}
{{- $b := $s.s3 }}
- name: AWS_REGION
  value: {{ $b.region | quote }}
{{- if $b.endpoint }}
- name: AWS_ENDPOINT
  value: {{ $b.endpoint | quote }}
{{- end }}
{{- if $b.allowHttp }}
- name: AWS_ALLOW_HTTP
  value: "true"
{{- end }}
{{- if eq $b.auth "accessKey" }}
{{- include "nx-remote-cache.storeSecretEnv" (dict "key" "AWS_ACCESS_KEY_ID" "context" $) | nindent 0 }}
{{- include "nx-remote-cache.storeSecretEnv" (dict "key" "AWS_SECRET_ACCESS_KEY" "context" $) | nindent 0 }}
{{- include "nx-remote-cache.storeSecretEnv" (dict "key" "AWS_SESSION_TOKEN" "optional" true "context" $) | nindent 0 }}
{{- end }}
{{- end }}
{{- if and (eq $s.type "gcs") (eq $s.gcs.auth "serviceAccountKey") }}
{{- include "nx-remote-cache.storeSecretEnv" (dict "key" "GOOGLE_SERVICE_ACCOUNT_KEY" "context" $) | nindent 0 }}
{{- end }}
{{- end -}}

{{/* ServiceAccount annotations derived from the store identity, merged with the user's. */}}
{{- define "nx-remote-cache.serviceAccountAnnotations" -}}
{{- $s := .Values.store -}}
{{- $derived := dict -}}
{{- if and (eq $s.type "azure") (eq $s.azure.auth "workloadIdentity") $s.azure.workloadIdentity.clientId -}}
{{- $_ := set $derived "azure.workload.identity/client-id" $s.azure.workloadIdentity.clientId -}}
{{- end -}}
{{- if and (eq $s.type "s3") (eq $s.s3.auth "irsa") $s.s3.irsa.roleArn -}}
{{- $_ := set $derived "eks.amazonaws.com/role-arn" $s.s3.irsa.roleArn -}}
{{- end -}}
{{- if and (eq $s.type "gcs") (eq $s.gcs.auth "workloadIdentity") $s.gcs.workloadIdentity.gcpServiceAccount -}}
{{- $_ := set $derived "iam.gke.io/gcp-service-account" $s.gcs.workloadIdentity.gcpServiceAccount -}}
{{- end -}}
{{- include "common.tplvalues.merge" (dict "values" (list .Values.serviceAccount.annotations $derived .Values.commonAnnotations) "context" .) -}}
{{- end -}}

{{/* Pod labels: the user's, commonLabels, and the AKS Workload Identity opt-in when relevant. */}}
{{- define "nx-remote-cache.podLabels" -}}
{{- $derived := dict -}}
{{- if and (eq .Values.store.type "azure") (eq .Values.store.azure.auth "workloadIdentity") -}}
{{- $_ := set $derived "azure.workload.identity/use" "true" -}}
{{- end -}}
{{- include "common.tplvalues.merge" (dict "values" (list .Values.podLabels $derived .Values.commonLabels) "context" .) -}}
{{- end -}}

{{/* ---------- server ---------- */}}

{{/* The key-value backend actually used: the bundled Redis wins over the value. */}}
{{- define "nx-remote-cache.kvBackend" -}}
{{- ternary "redis" .Values.server.kv.backend .Values.redis.enabled -}}
{{- end -}}

{{/* Name of the bundled Redis release objects, as bitnami/redis computes it. */}}
{{- define "nx-remote-cache.redis.fullname" -}}
{{- if .Values.redis.fullnameOverride -}}
{{- .Values.redis.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else if contains "redis" .Release.Name -}}
{{- .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-redis" .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{/* Secret and key holding the bundled Redis password. */}}
{{- define "nx-remote-cache.redis.secretName" -}}
{{- .Values.redis.auth.existingSecret | default (include "nx-remote-cache.redis.fullname" .) -}}
{{- end -}}
{{- define "nx-remote-cache.redis.passwordKey" -}}
{{- .Values.redis.auth.existingSecretPasswordKey | default "redis-password" -}}
{{- end -}}

{{- define "nx-remote-cache.serverEnv" -}}
{{- $v := .Values.server }}
- name: NX_CACHE_MAX_ARTIFACT_BYTES
  value: {{ $v.uploads.maxArtifactSize | quote }}
- name: NX_CACHE_INLINE_PUT_THRESHOLD
  value: {{ $v.uploads.inlineThreshold | quote }}
- name: NX_CACHE_STREAM_PUT_THRESHOLD
  value: {{ $v.uploads.streamThreshold | quote }}
- name: NX_CACHE_MULTIPART_CHUNK_BYTES
  value: {{ $v.uploads.multipartChunkSize | quote }}
- name: NX_CACHE_MULTIPART_CONCURRENCY
  value: {{ $v.uploads.multipartConcurrency | quote }}
- name: NX_CACHE_MAX_CONCURRENT_PUTS
  value: {{ $v.uploads.maxConcurrentPuts | quote }}
- name: NX_CACHE_MEMORY_CACHE_BYTES
  value: {{ ternary ($v.memoryTier.size | toString) "0" $v.memoryTier.enabled | quote }}
- name: NX_CACHE_MEMORY_CACHE_MAX_ITEM
  value: {{ $v.memoryTier.maxItemSize | quote }}
- name: NX_CACHE_KV_ENABLED
  value: {{ $v.kv.enabled | quote }}
- name: NX_CACHE_KV_BACKEND
  value: {{ include "nx-remote-cache.kvBackend" . | quote }}
{{- if eq (include "nx-remote-cache.kvBackend" .) "redis" }}
- name: NX_CACHE_KV_REDIS_PREFIX
  value: {{ $v.kv.redis.prefix | quote }}
{{- if and .Values.redis.enabled (not $v.kv.redis.url) (not $v.kv.redis.existingSecret) }}
- name: NX_CACHE_KV_REDIS_URL
  value: {{ printf "redis://%s-master:6379/0" (include "nx-remote-cache.redis.fullname" .) | quote }}
{{- if .Values.redis.auth.enabled }}
- name: NX_CACHE_KV_REDIS_PASSWORD
  valueFrom:
    secretKeyRef:
      name: {{ include "nx-remote-cache.redis.secretName" . }}
      key: {{ include "nx-remote-cache.redis.passwordKey" . }}
{{- end }}
{{- else }}
- name: NX_CACHE_KV_REDIS_URL
  valueFrom:
    secretKeyRef:
      name: {{ $v.kv.redis.existingSecret | default (printf "%s-kv" (include "common.names.fullname" .)) }}
      key: {{ ternary $v.kv.redis.existingSecretKey "NX_CACHE_KV_REDIS_URL" (not (empty $v.kv.redis.existingSecret)) }}
{{- end }}
{{- end }}
- name: NX_CACHE_STATS_BACKEND
  value: {{ ternary "redis" "local" (and $v.stats.shared (eq (include "nx-remote-cache.kvBackend" .) "redis")) | quote }}
- name: NX_CACHE_STATS_REDIS_PREFIX
  value: {{ $v.stats.redisPrefix | quote }}
- name: NX_CACHE_KV_BYTES
  value: {{ $v.kv.size | quote }}
- name: NX_CACHE_KV_MAX_VALUE
  value: {{ $v.kv.maxValueSize | quote }}
{{- if $v.retention.ttl }}
- name: NX_CACHE_TTL
  value: {{ $v.retention.ttl | quote }}
{{- end }}
{{- if $v.retention.maxTotalSize }}
- name: NX_CACHE_MAX_TOTAL_BYTES
  value: {{ $v.retention.maxTotalSize | quote }}
{{- end }}
- name: NX_CACHE_SWEEP_INTERVAL
  value: {{ $v.retention.sweepInterval | quote }}
- name: NX_CACHE_UI_ENABLED
  value: {{ $v.dashboard.enabled | quote }}
{{- if $v.dashboard.existingSecret }}
- name: NX_CACHE_UI_TOKEN
  valueFrom:
    secretKeyRef:
      name: {{ $v.dashboard.existingSecret }}
      key: {{ $v.dashboard.existingSecretKey }}
{{- else if $v.dashboard.token }}
- name: NX_CACHE_UI_TOKEN
  valueFrom:
    secretKeyRef:
      name: {{ include "nx-remote-cache.secretName" . }}
      key: NX_CACHE_UI_TOKEN
{{- end }}
{{- if $v.dashboard.trustedHeader }}
- name: NX_CACHE_UI_TRUSTED_HEADER
  value: {{ $v.dashboard.trustedHeader | quote }}
{{- end }}
- name: NX_CACHE_GITHUB_ENABLED
  value: {{ $v.github.enabled | quote }}
{{- if $v.github.url }}
- name: NX_CACHE_GITHUB_URL
  value: {{ $v.github.url | quote }}
{{- end }}
- name: NX_CACHE_LOG_LEVEL
  value: {{ $v.logging.level | quote }}
- name: NX_CACHE_LOG_FORMAT
  value: {{ $v.logging.format | quote }}
- name: NX_CACHE_ACCESS_LOG
  value: {{ $v.logging.accessLog | quote }}
{{- if $v.logging.filter }}
- name: RUST_LOG
  value: {{ $v.logging.filter | quote }}
{{- end }}
- name: NX_CACHE_SHUTDOWN_GRACE_SECS
  value: {{ $v.shutdownGraceSeconds | quote }}
{{- range $name, $value := $v.extraEnv }}
- name: {{ $name }}
  value: {{ $value | quote }}
{{- end }}
{{- end -}}

{{/* ---------- validation ---------- */}}

{{- define "nx-remote-cache.validateValues" -}}
{{- $s := .Values.store -}}
{{- $m := list -}}
{{- if eq $s.type "azure" -}}
{{- if and (ne $s.azure.auth "emulator") (not $s.azure.accountName) }}{{ $m = append $m "store.azure.accountName is required" }}{{ end -}}
{{- if not $s.azure.container }}{{ $m = append $m "store.azure.container is required" }}{{ end -}}
{{- if and (eq $s.azure.auth "accountKey") (not $s.azure.accountKey) (not $s.azure.existingSecret) }}{{ $m = append $m "store.azure.auth=accountKey needs store.azure.accountKey or store.azure.existingSecret (key AZURE_STORAGE_ACCOUNT_KEY)" }}{{ end -}}
{{- if and (eq $s.azure.auth "sasToken") (not $s.azure.sasToken) (not $s.azure.existingSecret) }}{{ $m = append $m "store.azure.auth=sasToken needs store.azure.sasToken or store.azure.existingSecret (key AZURE_STORAGE_SAS_TOKEN)" }}{{ end -}}
{{- if and (eq $s.azure.auth "servicePrincipal") (or (not $s.azure.servicePrincipal.clientId) (not $s.azure.servicePrincipal.tenantId)) }}{{ $m = append $m "store.azure.auth=servicePrincipal needs servicePrincipal.clientId and tenantId (and clientSecret or existingSecret)" }}{{ end -}}
{{- else if eq $s.type "s3" -}}
{{- if not $s.s3.bucket }}{{ $m = append $m "store.s3.bucket is required" }}{{ end -}}
{{- if not $s.s3.region }}{{ $m = append $m "store.s3.region is required (any value for MinIO / R2)" }}{{ end -}}
{{- if and (eq $s.s3.auth "accessKey") (not $s.s3.existingSecret) (or (not $s.s3.accessKeyId) (not $s.s3.secretAccessKey)) }}{{ $m = append $m "store.s3.auth=accessKey needs accessKeyId and secretAccessKey, or existingSecret (keys AWS_ACCESS_KEY_ID, AWS_SECRET_ACCESS_KEY)" }}{{ end -}}
{{- else if eq $s.type "gcs" -}}
{{- if not $s.gcs.bucket }}{{ $m = append $m "store.gcs.bucket is required" }}{{ end -}}
{{- if and (eq $s.gcs.auth "serviceAccountKey") (not $s.gcs.serviceAccountKey) (not $s.gcs.existingSecret) }}{{ $m = append $m "store.gcs.auth=serviceAccountKey needs serviceAccountKey or existingSecret (key GOOGLE_SERVICE_ACCOUNT_KEY)" }}{{ end -}}
{{- else if eq $s.type "filesystem" -}}
{{- if not .Values.persistence.enabled }}{{ $m = append $m "store.type=filesystem needs persistence.enabled: true (artifacts would vanish with the pod)" }}{{ end -}}
{{- end -}}
{{- if and (eq $s.type "memory") (gt (int .Values.replicaCount) 1) (not .Values.autoscaling.enabled) }}{{ $m = append $m "store.type=memory with several replicas gives each pod its own cache; set replicaCount: 1" }}{{ end -}}
{{- if and (eq $s.type "memory") .Values.autoscaling.enabled }}{{ $m = append $m "store.type=memory cannot autoscale: each pod would have its own cache" }}{{ end -}}
{{- if and (eq $s.type "filesystem") (or (gt (int .Values.replicaCount) 1) .Values.autoscaling.enabled) (not .Values.persistence.existingClaim) (not (has "ReadWriteMany" .Values.persistence.accessModes)) }}{{ $m = append $m "store.type=filesystem with several replicas needs a ReadWriteMany volume (persistence.accessModes) or persistence.existingClaim; otherwise set replicaCount: 1 and autoscaling.enabled: false" }}{{ end -}}
{{- if and .Values.server.dashboard.token (not .Values.server.dashboard.existingSecret) (or (not .Values.auth.enabled) .Values.auth.existingSecret) }}{{ $m = append $m "server.dashboard.token lives in the chart's token secret, which is not rendered with auth.enabled=false or auth.existingSecret; use server.dashboard.existingSecret" }}{{ end -}}
{{- if and (eq .Values.server.kv.backend "redis") (not .Values.redis.enabled) (not .Values.server.kv.redis.url) (not .Values.server.kv.redis.existingSecret) }}{{ $m = append $m "server.kv.backend=redis needs server.kv.redis.url, server.kv.redis.existingSecret, or redis.enabled=true to bundle one" }}{{ end -}}
{{- if $m -}}
{{- printf "\nVALUES VALIDATION:\n- %s\n" (join "\n- " $m) | fail -}}
{{- end -}}
{{- end -}}
