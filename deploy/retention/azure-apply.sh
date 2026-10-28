#!/usr/bin/env sh
# Enable last-access tracking (needed for daysAfterLastAccessTimeGreaterThan) and apply the policy.
# Usage: ./azure-apply.sh <resource-group> <storage-account>
set -eu
RG="$1"; ACCOUNT="$2"
az storage account blob-service-properties update \
  --resource-group "$RG" --account-name "$ACCOUNT" \
  --enable-last-access-tracking true
az storage account management-policy create \
  --resource-group "$RG" --account-name "$ACCOUNT" \
  --policy @azure-lifecycle.json
