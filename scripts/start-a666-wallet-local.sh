#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
state_dir=${A666_WALLET_STATE_DIR:-/home/postfiat/.local/state/postfiat-a666-wallet}
token_file=$state_dir/proxy-tokens.json
job_root=$state_dir/bridge-jobs
a666_job_root=$state_dir/a666-export-jobs
a666_return_job_root=$state_dir/a666-return-jobs
routes=$repo/deployments/wallet-bridge-mainnet-20260730/routes.json
a666_export_config=$repo/deployments/a666-export-relay-mainnet-20260731/service-config.json
a666_return_config=$repo/deployments/a666-export-relay-mainnet-20260731/return-service-config.json

umask 077
install -d -m 700 "$state_dir" "$job_root" "$a666_job_root" "$a666_return_job_root"
if test ! -s "$token_file"; then
  token=$(openssl rand -hex 32)
  temporary=$token_file.$$.tmp
  jq -n --arg token "$token" '{"local-demo":$token}' > "$temporary"
  chmod 600 "$temporary"
  mv "$temporary" "$token_file"
fi
chmod 600 "$token_file" "$routes" \
  "$repo/deployments/wallet-bridge-mainnet-20260730/driver-config.json" \
  "$a666_export_config" \
  "$repo/deployments/a666-export-relay-mainnet-20260731/driver-config.json" \
  "$a666_return_config" \
  "$repo/deployments/a666-export-relay-mainnet-20260731/return-driver-config.json"

exec env \
  LISTEN_HOST=127.0.0.1 \
  LISTEN_PORT=8080 \
  ALLOWED_ORIGINS=http://127.0.0.1:8080,http://localhost:8080,https://127.0.0.1:5173,https://localhost:5173 \
  RPC_HOST=127.0.0.1 \
  RPC_PORT=38650 \
  RPC_FLEET=validator-0=127.0.0.1:38650,validator-1=127.0.0.1:38651,validator-2=127.0.0.1:38652,validator-3=127.0.0.1:38653,validator-4=127.0.0.1:38654,validator-5=127.0.0.1:38655 \
  PFUSDC_ASSET_ID=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b \
  WALLET_STATIC_DIR="$repo/wallet-web/dist" \
  WALLET_PROXY_API_TOKENS_FILE="$token_file" \
  WALLET_PROXY_LOCAL_SESSION_PRINCIPAL=local-demo \
  TRUSTLESS_BRIDGE_ROUTES_JSON_FILE="$routes" \
  TRUSTLESS_BRIDGE_JOB_ROOT="$job_root" \
  TRUSTLESS_BRIDGE_READINESS_REFRESH_MS=15000 \
  TRUSTLESS_BRIDGE_READINESS_MAX_AGE_MS=45000 \
  TRUSTLESS_BRIDGE_RETRY_BASE_MS=5000 \
  TRUSTLESS_BRIDGE_RETRY_MAX_MS=300000 \
  A666_EXPORT_RELAY_CONFIG_FILE="$a666_export_config" \
  A666_EXPORT_RELAY_JOB_ROOT="$a666_job_root" \
  A666_EXPORT_RELAY_RETRY_BASE_MS=5000 \
  A666_EXPORT_RELAY_RETRY_MAX_MS=300000 \
  A666_RETURN_RELAY_CONFIG_FILE="$a666_return_config" \
  A666_RETURN_RELAY_JOB_ROOT="$a666_return_job_root" \
  A666_RETURN_RELAY_RETRY_BASE_MS=5000 \
  A666_RETURN_RELAY_RETRY_MAX_MS=300000 \
  node "$repo/wallet-proxy/server.js"
