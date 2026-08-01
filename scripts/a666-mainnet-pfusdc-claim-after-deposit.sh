#!/usr/bin/env bash
set -euo pipefail

# Prove one finalized Ethereum pfUSDC deposit and claim it on PFTL.
# This deliberately stops before A666 reserve/subscription/export operations.

repo=$(cd "$(dirname "$0")/.." && pwd)
run_dir=
workflow_id=
expected_pftl_height=
expected_holder_atoms=
resume_after_proof=false
a100_host=${A666_A100_HOST:?A666_A100_HOST is required}
a100_port=${A666_A100_PORT:-30886}
validator2_host=${A666_VALIDATOR2_HOST:?A666_VALIDATOR2_HOST is required}
release_id=${A666_PFTL_RELEASE_ID:-resident-local-commit-777faa0}

while (($#)); do
  case "$1" in
    --run-dir) run_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --expected-pftl-height) expected_pftl_height=$2; shift 2 ;;
    --expected-holder-atoms) expected_holder_atoms=$2; shift 2 ;;
    --resume-after-proof) resume_after_proof=true; shift ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in \
  "$run_dir" \
  "$workflow_id" \
  "$expected_pftl_height" \
  "$expected_holder_atoms"
do
  test -n "$value"
done
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,63}$ ]]
[[ "$expected_pftl_height" =~ ^[0-9]+$ ]]
[[ "$expected_holder_atoms" =~ ^[1-9][0-9]*$ ]]

cd "$repo"
run_dir=$(realpath "$run_dir")
deposit_file="$run_dir/deposit/deposit-result.json"
test -s "$deposit_file"
jq -e '.schema=="postfiat.a666.pfusdc_buyer_deposit.v1" and .verdict=="PASS"' \
  "$deposit_file" >/dev/null

deposit_tx=$(jq -er '.deposit.tx_hash' "$deposit_file")
deposit_id=$(jq -er '.event.deposit_id | ltrimstr("0x")' "$deposit_file")
deposit_atoms=$(jq -er '.amount_atoms' "$deposit_file")
[[ "$deposit_tx" =~ ^0x[0-9a-f]{64}$ ]]
[[ "$deposit_id" =~ ^[0-9a-f]{64}$ ]]
[[ "$deposit_atoms" =~ ^[1-9][0-9]*$ ]]

remote_node=/opt/postfiat/releases/$release_id/postfiat-node
remote_topology=/etc/postfiat/releases/$release_id/topology.json
remote_run="/var/lib/postfiat/validator-2/$workflow_id"
a100_root="/workspace/a666-acceptance/live/$workflow_id"
capture_bin=/workspace/a666-acceptance/bin/eth-l1-mainnet-fast-lane-p0-cuda
prove_bin=/workspace/a666-acceptance/bin/eth-l1-mainnet-fast-lane-p0-cuda-optimized
local_ingress="$run_dir/ingress"
local_pftl="$run_dir/pftl-claim"
local_proof="$local_ingress/proof-cuda"
install -d -m 700 "$local_ingress" "$local_pftl"

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node status --data-dir /var/lib/postfiat/validator-2 \
    --expect-height $expected_pftl_height" \
  > "$local_pftl/status-before.json"

jq '{
  vault,
  deposit_tx:.deposit.tx_hash,
  amount_atoms,
  recipient:.pftl_recipient,
  route_binding:(.route_binding|ltrimstr("0x")),
  nonce:(.nonce|ltrimstr("0x")),
  creation_bytecode_hash:"0xc02403a4d05a2b4400d21b360e5787ad560c1fccd293c1ad937840f986fdcd38"
}' "$deposit_file" > "$local_ingress/capture-deployment.json"

if ! "$resume_after_proof"; then
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "test ! -e '$a100_root'; install -d -m 700 '$a100_root/ingress'"
  scp -q -P "$a100_port" "$local_ingress/capture-deployment.json" \
    "root@$a100_host:$a100_root/ingress/deployment.json"
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "'$capture_bin' capture \
      --deployment '$a100_root/ingress/deployment.json' \
      --output '$a100_root/ingress/witness.json' \
      --wait-seconds 1800"
  scp -q -P "$a100_port" \
    "root@$a100_host:$a100_root/ingress/witness.json" \
    "$local_ingress/witness.json"
  scp -q -P "$a100_port" \
    "root@$a100_host:$a100_root/ingress/witness.public-values.json" \
    "$local_ingress/witness.public-values.json"
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "SP1_PROVER=cuda '$prove_bin' prove \
      --witness '$a100_root/ingress/witness.json' \
      --output-dir '$a100_root/ingress-proof' \
      --require-prover cuda \
      --skip-redundant-execute"
  install -d -m 700 "$local_proof"
  rsync -a -e "ssh -p $a100_port" \
    "root@$a100_host:$a100_root/ingress-proof/" \
    "$local_proof/"
fi

test -s "$local_ingress/witness.public-values.json"
test -s "$local_proof/proof-calldata.bin"
test -s "$local_proof/public-values.bin"
test -s "$local_proof/proof-report.json"
jq -e \
  --arg deposit_id "$deposit_id" \
  --argjson amount "$deposit_atoms" \
  '.deposit_id==$deposit_id and .amount_atoms==$amount' \
  "$local_ingress/witness.public-values.json" >/dev/null
jq -e '
  .program_vkey=="0x00a9f8f037da18dd1aa5a7b0f478df0c7c9fae411ee62b339baf48dc2505076e"
  and .prover_backend=="cuda"
  and .host_execute_skipped==true
  and .execute_ms==0
  and .proof_bytes==356
' "$local_proof/proof-report.json" >/dev/null

ssh -o BatchMode=yes "root@$validator2_host" \
  "if test -e '$remote_run'; then
     test -d '$remote_run'
   else
     install -d -o root -g root -m 700 '$remote_run'
   fi
   install -d -o root -g root -m 700 '$remote_run/ingress-proof'"
rsync -a "$local_proof/" "root@$validator2_host:$remote_run/ingress-proof/"

DEPOSIT_TX="$deposit_tx" \
DEPOSIT_ATOMS="$deposit_atoms" \
EXPECTED_HOLDER_ATOMS="$expected_holder_atoms" \
PFTL_NODE_BIN="$remote_node" \
PFTL_TOPOLOGY="$remote_topology" \
PFTL_POLICY_HASH=5025bdfe92669e3d8f81ce7e739fd132063261b92ef7e7ee7db19b2762e88b736bd40cd4826375e041584533f4137158 \
PFTL_VAULT_ADDRESS=0xaaa78fda7062efce769e95cd72fc55e507bc8183 \
PFTL_RUN_DIR="$remote_run" \
PFTL_PROOF_DIR="$remote_run/ingress-proof" \
PFTL_LOCAL_EVIDENCE="$local_pftl" \
bash "$repo/scripts/a666-mainnet-pfusdc-relay.sh"

jq -n \
  --arg deposit_tx "$deposit_tx" \
  --arg deposit_id "$deposit_id" \
  --argjson amount_atoms "$deposit_atoms" \
  --argjson expected_holder_atoms "$expected_holder_atoms" \
  --slurpfile relay "$local_pftl/summary.json" \
  '{
    schema:"postfiat.a666.pfusdc_claim_after_deposit.v1",
    verdict:"PASS",
    deposit_tx:$deposit_tx,
    deposit_id:$deposit_id,
    amount_atoms:$amount_atoms,
    expected_holder_atoms:$expected_holder_atoms,
    relay:$relay[0]
  }' > "$run_dir/pfusdc-claim-summary.json"
cat "$run_dir/pfusdc-claim-summary.json"
