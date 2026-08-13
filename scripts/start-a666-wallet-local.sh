#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
state_dir=${A666_WALLET_STATE_DIR:-/home/postfiat/.local/state/postfiat-a666-wallet}
token_file=$state_dir/proxy-tokens.json
job_root=$state_dir/bridge-jobs
a666_job_root=$state_dir/a666-export-jobs
a666_return_job_root=$state_dir/a666-return-jobs
pfusdc_withdrawal_job_root=$state_dir/pfusdc-withdrawal-jobs
pfusdc_prover_source=/home/postfiat/repos/postfiatl1v2-public-main-verification-20260717/tools/pfusdc-tier4-prover/target/release/pfusdc-tier4-prover
pfusdc_prover_bin=$state_dir/bin/pfusdc-tier4-prover
pnok_fix_job_root=$state_dir/pnok-fix-jobs
routes=$repo/deployments/wallet-bridge-mainnet-20260730/routes.json
a666_export_config=$repo/deployments/a666-export-relay-mainnet-20260731/service-config.json
a666_return_config=$repo/deployments/a666-export-relay-mainnet-20260731/return-service-config.json
pnok_fix_config=$repo/deployments/pnok-private-fix-20260801/wallet-service-config.json

umask 077
install -d -m 700 "$state_dir" "$state_dir/bin" "$job_root" "$a666_job_root" "$a666_return_job_root" "$pfusdc_withdrawal_job_root" "$pnok_fix_job_root"
test -x "$pfusdc_prover_source"
if ! test -x "$pfusdc_prover_bin" || ! cmp -s "$pfusdc_prover_source" "$pfusdc_prover_bin"; then
  install -m 700 "$pfusdc_prover_source" "$pfusdc_prover_bin.tmp"
  mv "$pfusdc_prover_bin.tmp" "$pfusdc_prover_bin"
fi
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
  "$repo/deployments/a666-export-relay-mainnet-20260731/return-driver-config.json" \
  "$pnok_fix_config"

exec env \
  LISTEN_HOST=127.0.0.1 \
  LISTEN_PORT=8080 \
  ALLOWED_ORIGINS=http://127.0.0.1:8080,http://localhost:8080,https://127.0.0.1:5173,https://localhost:5173 \
  RPC_HOST=127.0.0.1 \
  RPC_PORT=39650 \
  RPC_FLEET=validator-0=127.0.0.1:39650,validator-1=127.0.0.1:39651,validator-2=127.0.0.1:39652,validator-3=127.0.0.1:39653,validator-4=127.0.0.1:39654,validator-5=127.0.0.1:39655 \
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
  NAVCOIN_EXPORT_RELAY_CONFIG_FILE="$a666_export_config" \
  NAVCOIN_EXPORT_RELAY_JOB_ROOT="$a666_job_root" \
  NAVCOIN_EXPORT_RELAY_RETRY_BASE_MS=5000 \
  NAVCOIN_EXPORT_RELAY_RETRY_MAX_MS=300000 \
  NAVCOIN_RETURN_RELAY_CONFIG_FILE="$a666_return_config" \
  NAVCOIN_RETURN_RELAY_JOB_ROOT="$a666_return_job_root" \
  NAVCOIN_RETURN_RELAY_RETRY_BASE_MS=5000 \
  NAVCOIN_RETURN_RELAY_RETRY_MAX_MS=300000 \
  PFUSDC_WITHDRAWAL_ENABLED=true \
  PFUSDC_WITHDRAWAL_JOB_ROOT="$pfusdc_withdrawal_job_root" \
  PFUSDC_WITHDRAWAL_SCRIPT="$repo/scripts/a666-mainnet-pfusdc-proof-egress.sh" \
  PFUSDC_WITHDRAWAL_CONFIG_FILE=/home/postfiat/.config/stakehub/a666-roundtrip.json \
  PFUSDC_WITHDRAWAL_LOCAL_PROVER_BIN="$pfusdc_prover_bin" \
  PFUSDC_WITHDRAWAL_EGRESS_ELF="$repo/programs/pfusdc-egress/target/elf-compilation/riscv64im-succinct-zkvm-elf/release/pfusdc-egress-program" \
  PFUSDC_WITHDRAWAL_QUALIFICATION_REPORT=/home/postfiat/.local/state/pfusdc-withdrawal-qualification/proof-report.json \
  A666_PFTL_RPC_PORTS=39650,39651,39652,39653,39654,39655 \
  SHARD_SIZE=1048576 TRACE_CHUNK_SLOTS=2 GAS_TRACE_CHUNK_SLOTS=2 \
  SP1_WORKER_NUM_SPLICING_WORKERS=1 SP1_WORKER_SPLICING_BUFFER_SIZE=1 \
  SP1_WORKER_NUMBER_OF_SEND_SPLICE_WORKERS_PER_SPLICE=1 SP1_WORKER_SEND_SPLICE_INPUT_BUFFER_SIZE_PER_SPLICE=1 SP1_WORKER_GLOBAL_MEMORY_BUFFER_SIZE=1 \
  SP1_WORKER_NUM_CORE_WORKERS=1 SP1_WORKER_CORE_BUFFER_SIZE=1 SP1_WORKER_NUM_SETUP_WORKERS=1 SP1_WORKER_SETUP_BUFFER_SIZE=1 SP1_WORKER_NORMALIZE_PROGRAM_CACHE_SIZE=1 \
  SP1_WORKER_NUM_PREPARE_REDUCE_WORKERS=1 SP1_WORKER_PREPARE_REDUCE_BUFFER_SIZE=1 SP1_WORKER_NUM_RECURSION_EXECUTOR_WORKERS=1 SP1_WORKER_RECURSION_EXECUTOR_BUFFER_SIZE=1 \
  SP1_WORKER_NUM_RECURSION_PROVER_WORKERS=1 SP1_WORKER_RECURSION_PROVER_BUFFER_SIZE=1 SP1_WORKER_NUM_DEFERRED_WORKERS=1 SP1_WORKER_DEFERRED_BUFFER_SIZE=1 \
  SP1_WORKER_NUMBER_OF_GAS_EXECUTORS=1 SP1_WORKER_GAS_EXECUTOR_BUFFER_SIZE=1 RAYON_NUM_THREADS=20 \
  PNOK_FIX_WALLET_CONFIG_FILE="$pnok_fix_config" \
  PNOK_FIX_WALLET_JOB_ROOT="$pnok_fix_job_root" \
  PFTL_PRIVATE_SWAP_URL=http://127.0.0.1:39798 \
  PFTL_PRIVATE_SWAP_CONTROLLED_WALLET_ID=pfab9b9228942e5c529633a13aa271d5297bec6353 \
  PFTL_PRIVATE_SWAP_ROUTE_ID=pftl-a666-ethereum-wA666-usdc-v1 \
  node "$repo/wallet-proxy/server.js"
