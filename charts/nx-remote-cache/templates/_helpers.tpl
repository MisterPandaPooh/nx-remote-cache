{{/* vim: set filetype=mustache: */}}

{{/* Image reference, honouring global.imageRegistry and image.digest. */}}
{{- define "nx-remote-cache.image" -}}
{{- include "common.images.image" (dict "imageRoot" .Values.image "global" .Values.global) -}}
{{- end -}}

{{/* Image pull secrets, global ones first. */}}
{{- define "nx-remote-cache.imagePullSecrets" -}}
{{- include "common.images.renderPullSecrets" (dict "images" (list .Values.image) "context" $) -}}
{{- end -}}

{{/* Service account name. */}}
{{- define "nx-remote-cache.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "common.names.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{/* Secret holding the tokens: the user's, or the one this chart manages. */}}
{{- define "nx-remote-cache.secretName" -}}
{{- include "common.secrets.name" (dict "existingSecret" .Values.auth.existingSecret "defaultNameSuffix" "tokens" "context" $) -}}
{{- end -}}

{{- define "nx-remote-cache.tokenKey" -}}
{{- if .Values.auth.existingSecret -}}
{{- .Values.auth.existingSecretTokenKey -}}
{{- else -}}
NX_CACHE_TOKEN
{{- end -}}
{{- end -}}

{{- define "nx-remote-cache.readTokenKey" -}}
{{- if .Values.auth.existingSecret -}}
{{- .Values.auth.existingSecretReadTokenKey -}}
{{- else -}}
NX_CACHE_READ_TOKEN
{{- end -}}
{{- end -}}

{{/* Values that must be set. */}}
{{- define "nx-remote-cache.validateValues" -}}
{{- $messages := list -}}
{{- if not .Values.storeUrl -}}
{{- $messages = append $messages "storeUrl is required: az://<container>/<prefix>, s3://<bucket>/<prefix>, gs://<bucket>/<prefix>, file:///data or memory:///" -}}
{{- end -}}
{{- if and (hasPrefix "file://" .Values.storeUrl) (not .Values.persistence.enabled) -}}
{{- $messages = append $messages "storeUrl uses file:// but persistence.enabled is false: artifacts would vanish with the pod" -}}
{{- end -}}
{{- $messages := without $messages "" -}}
{{- if $messages -}}
{{- printf "\nVALUES VALIDATION:\n%s" (join "\n" $messages) | fail -}}
{{- end -}}
{{- end -}}
