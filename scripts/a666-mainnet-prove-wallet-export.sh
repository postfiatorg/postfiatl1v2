#!/usr/bin/env bash
set -euo pipefail

repo=$(cd "$(dirname "$0")/.." && pwd)
packet_hash=
phase_dir=
workflow_id=
expected_recipient=
expected_amount_atoms=
resume=false
a100_host=${A666_A100_HOST:?A666_A100_HOST is required}
a100_port=${A666_A100_PORT:-30886}
validator2_host=${A666_VALIDATOR2_HOST:?A666_VALIDATOR2_HOST is required}
release_id=${A666_PFTL_RELEASE_ID:-resident-local-commit-777faa0}
ethereum_rpc=${A666_ETHEREUM_RPC:-https://ethereum-rpc.publicnode.com}

while (($#)); do
  case "$1" in
    --resume) resume=true; shift ;;
    --packet-hash) packet_hash=$2; shift 2 ;;
    --phase-dir) phase_dir=$2; shift 2 ;;
    --workflow-id) workflow_id=$2; shift 2 ;;
    --expected-recipient) expected_recipient=${2,,}; shift 2 ;;
    --expected-amount-atoms) expected_amount_atoms=$2; shift 2 ;;
    *) echo "unknown argument: $1" >&2; exit 2 ;;
  esac
done

for value in "$packet_hash" "$phase_dir" "$workflow_id" "$expected_recipient" "$expected_amount_atoms"; do
  test -n "$value"
done
[[ "$packet_hash" =~ ^[0-9a-f]{96}$ ]]
[[ "$workflow_id" =~ ^[a-z0-9][a-z0-9-]{0,63}$ ]]
[[ "$expected_recipient" =~ ^0x[0-9a-f]{40}$ ]]
[[ "$expected_amount_atoms" =~ ^[1-9][0-9]*$ ]]

cd "$repo"
if test -e "$phase_dir"; then
  if ! "$resume"; then
    echo "refusing to overwrite wallet export evidence: $phase_dir" >&2
    exit 1
  fi
  phase_dir=$(realpath "$phase_dir")
  if test -s "$phase_dir/completion.json"; then
    jq -e \
      --arg packet "$packet_hash" \
      --arg recipient "$expected_recipient" \
      --argjson amount "$expected_amount_atoms" \
      '.verdict=="PASS" and .packet_hash==$packet
       and (.recipient|ascii_downcase)==$recipient and .amount_atoms==$amount
       and (.mint_tx|ascii_downcase|test("^0x[0-9a-f]{64}$"))' \
      "$phase_dir/completion.json" >/dev/null
    cat "$phase_dir/completion.json"
    exit 0
  fi
else
  install -d -m 700 "$phase_dir"
fi
install -d -m 700 "$phase_dir/a666/ops" "$phase_dir/export-proof" "$phase_dir/ethereum"
phase_dir=$(realpath "$phase_dir")

remote_node=/opt/postfiat/releases/$release_id/postfiat-node
route_id=pftl-a666-ethereum-wA666-usdc-v1
a100_root=/workspace/a666-acceptance/live/$workflow_id
export_prover=/workspace/a666-acceptance/bin/pftl-uniswap-prover-cuda-optimized-20260729
export_elf=/workspace/a666-acceptance/witness/deployed-program-004e44.elf

packet_file="$phase_dir/a666/export-packet-before-proof.json"
packet_staging="$packet_file.staging"
packet_deadline=$((SECONDS + 120))
while ! test -s "$packet_file"; do
  if ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node navcoin-bridge-packet \
      --data-dir /var/lib/postfiat/validator-2 \
      --route-id $route_id \
      --packet-hash $packet_hash" \
      > "$packet_staging" 2>/dev/null \
    && jq -e \
      --arg packet "$packet_hash" \
      '.packet_hash==$packet and .packet.packet_hash==$packet and .packet.status=="SourceDebited"' \
      "$packet_staging" >/dev/null 2>&1; then
    mv "$packet_staging" "$packet_file"
    continue
  fi
  rm -f "$packet_staging"
  if ((SECONDS >= packet_deadline)); then
    echo "timed out waiting for finalized PFTL export packet $packet_hash on validator-2" >&2
    exit 1
  fi
  sleep 1
done
export_height=$(jq -er '.packet.source_height' "$phase_dir/a666/export-packet-before-proof.json")
jq -e \
  --arg packet "$packet_hash" \
  --arg recipient "$expected_recipient" \
  --argjson amount "$expected_amount_atoms" \
  '.packet_hash==$packet
   and .packet.packet_hash==$packet
   and (.packet.ethereum_recipient|ascii_downcase)==$recipient
   and .packet.amount_atoms==$amount
   and .packet.status=="SourceDebited"' \
  "$phase_dir/a666/export-packet-before-proof.json" >/dev/null

ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
  "install -d -m 700 '$a100_root/export' '$a100_root/checkpoints'"

if test -s "$phase_dir/ethereum/mint-state.json" \
  && jq -e \
    --arg recipient "$expected_recipient" \
    --argjson amount "$expected_amount_atoms" \
    '.phase=="minted-to-recipient"
     and (.post_state.recipient_balance_atoms-.pre_state.recipient_balance_atoms)==$amount
     and (.post_state.token_total_supply-.pre_state.token_total_supply)==$amount' \
    "$phase_dir/ethereum/mint-state.json" >/dev/null; then
  verifier_height_before=$(jq -er '.pre_state.latest_finalized_height' "$phase_dir/ethereum/mint-state.json")
  verifier_height=$verifier_height_before
  bash scripts/a666-mainnet-record-destination-consume.sh \
    --resume-auto \
    --phase-dir "$phase_dir" \
    --workflow-id "$workflow_id" \
    --expected-pftl-height "$((export_height + 1))"
  jq -n \
    --arg packet "$packet_hash" \
    --arg recipient "$expected_recipient" \
    --argjson amount "$expected_amount_atoms" \
    --argjson export_height "$export_height" \
    --argjson verifier_height_before "$verifier_height_before" \
    --argjson receipt_prior_height "$verifier_height" \
    --arg mint_tx "$(jq -er '.transactions[] | select(.label=="consume finalized A666 mint packet") | .tx' "$phase_dir/ethereum/mint-state.json")" \
    '{schema:"postfiat.a666.wallet_export_completion.v1",verdict:"PASS",packet_hash:$packet,
      recipient:$recipient,amount_atoms:$amount,export_height:$export_height,
      verifier_height_before:$verifier_height_before,receipt_prior_height:$receipt_prior_height,
      mint_tx:$mint_tx}' \
    | tee "$phase_dir/completion.json"
  exit 0
fi

verifier_height=$(ssh -o BatchMode=yes "root@$validator2_host" \
  "/var/lib/postfiat/validator-2/pfusdc-latency-20260727-run2/cast call 0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A 'latestFinalizedHeight()(uint64)' --rpc-url '$ethereum_rpc'")
[[ "$verifier_height" =~ ^[0-9]+$ ]]
test "$export_height" -gt "$verifier_height"
verifier_height_before=$verifier_height
install -d -m 700 "$phase_dir/export-proof/checkpoints"

while ((export_height - verifier_height - 1 > 64)); do
  target_height=$((verifier_height + 65))
  checkpoint_dir="$phase_dir/export-proof/checkpoints/$verifier_height-$target_height"
  remote_checkpoint="$a100_root/checkpoints/$verifier_height-$target_height"
  install -d -m 700 "$checkpoint_dir/proof-cuda"

  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node blocks --data-dir /var/lib/postfiat/validator-2 --from-height $verifier_height --limit 1" \
    > "$checkpoint_dir/prior-block.json"
  prior_checkpoint=$(jq -er \
    --argjson height "$verifier_height" \
    '.[0] | select(.header.height==$height) | .header.block_hash' \
    "$checkpoint_dir/prior-block.json")
  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node blocks --data-dir /var/lib/postfiat/validator-2 --from-height $target_height --limit 1" \
    > "$checkpoint_dir/target-block.json"
  target_checkpoint=$(jq -er \
    --argjson height "$target_height" \
    '.[0] | select(.header.height==$height) | .header.block_hash' \
    "$checkpoint_dir/target-block.json")
  [[ "$prior_checkpoint" =~ ^[0-9a-f]{96}$ ]]
  [[ "$target_checkpoint" =~ ^[0-9a-f]{96}$ ]]

  ssh -o BatchMode=yes "root@$validator2_host" \
    "$remote_node pfusdc-checkpoint-witness \
      --data-dir /var/lib/postfiat/validator-2 \
      --prior-checkpoint $prior_checkpoint \
      --target-block $target_checkpoint" \
    > "$checkpoint_dir/source-witness.json"
  jq '.schema="postfiat-pftl-uniswap-checkpoint-proof-witness-v1"' \
    "$checkpoint_dir/source-witness.json" > "$checkpoint_dir/witness.json"
  jq -e \
    --arg prior "$prior_checkpoint" \
    --arg target "$target_checkpoint" \
    --argjson height "$target_height" \
    '.schema=="postfiat-pftl-uniswap-checkpoint-proof-witness-v1"
     and .prior_checkpoint_block_id==$prior
     and .block.header.height==$height
     and .block.header.block_hash==$target
     and (.finality_ancestry|length)<=64' \
    "$checkpoint_dir/witness.json" >/dev/null

  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "install -d -m 700 '$remote_checkpoint'"
  scp -q -P "$a100_port" "$checkpoint_dir/witness.json" \
    "root@$a100_host:$remote_checkpoint/witness.json"
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "SP1_PROVER=cuda '$export_prover' checkpoint \
      --witness '$remote_checkpoint/witness.json' \
      --output-dir '$remote_checkpoint/proof' \
      --elf '$export_elf' \
      --prove \
      --require-prover cuda"
  rsync -a -e "ssh -p $a100_port" \
    "root@$a100_host:$remote_checkpoint/proof/" \
    "$checkpoint_dir/proof-cuda/"
  jq -e '
    .program_vkey=="0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9"
    and .proof_mode=="groth16"
    and .prover_backend=="cuda"
    and .proof_bytes==356
    and .public_values_bytes==256
  ' "$checkpoint_dir/proof-cuda/proof-report.json" >/dev/null
  python3 scripts/a666-mainnet-advance-pftl-checkpoint.py \
    --execute \
    --proof-dir "$checkpoint_dir/proof-cuda" \
    --prior-block-id "$prior_checkpoint" \
    --target-block-id "$target_checkpoint" \
    --prior-height "$verifier_height" \
    --target-height "$target_height" \
    --state-file "$checkpoint_dir/ethereum-state.json" \
    > "$checkpoint_dir/execute.stdout.json"
  jq -e \
    --argjson height "$target_height" \
    '.phase=="checkpoint-advanced" and .target_height==$height' \
    "$checkpoint_dir/ethereum-state.json" >/dev/null
  verifier_height=$target_height
done

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node blocks --data-dir /var/lib/postfiat/validator-2 --from-height $verifier_height --limit 1" \
  > "$phase_dir/export-proof/prior-checkpoint-block.json"
prior_checkpoint=$(jq -er \
  --argjson height "$verifier_height" \
  '.[0] | select(.header.height==$height) | .header.block_hash' \
  "$phase_dir/export-proof/prior-checkpoint-block.json")
[[ "$prior_checkpoint" =~ ^[0-9a-f]{96}$ ]]

ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node pftl-uniswap-receipt-witness \
    --data-dir /var/lib/postfiat/validator-2 \
    --packet-hash $packet_hash \
    --prior-checkpoint $prior_checkpoint" \
  > "$phase_dir/export-proof/receipt-witness.json"
jq -e \
  --arg packet "$packet_hash" \
  --arg prior "$prior_checkpoint" \
  --arg recipient "$expected_recipient" \
  --argjson height "$export_height" \
  --argjson amount "$expected_amount_atoms" \
  '.schema=="postfiat-pftl-uniswap-receipt-proof-witness-v1"
   and .prior_checkpoint_block_id==$prior
   and .receipt.packet_hash==$packet
   and .receipt.block_height==$height
   and .receipt.amount_atoms==$amount
   and .mint_packet.source_packet_hash==$packet
   and .mint_packet.mint_amount_atoms==$amount
   and (.mint_packet.ethereum_recipient|ascii_downcase)==$recipient
   and .block.header.height==$height' \
  "$phase_dir/export-proof/receipt-witness.json" >/dev/null

jq -n \
  --arg packet "$packet_hash" \
  --argjson amount "$expected_amount_atoms" \
  '{schema:"postfiat.a666.wallet_export_manifest.v1",packet_hash:$packet,mint_amount_atoms:$amount}' \
  > "$phase_dir/a666/ops/manifest.json"
ssh -o BatchMode=yes "root@$validator2_host" \
  "$remote_node navcoin-bridge-supply-status \
    --data-dir /var/lib/postfiat/validator-2 \
    --route-id $route_id" \
  > "$phase_dir/pftl-supply-status-after.json"

if ! test -s "$phase_dir/export-proof/proof-cuda/proof-report.json"; then
  scp -q -P "$a100_port" "$phase_dir/export-proof/receipt-witness.json" \
    "root@$a100_host:$a100_root/export/receipt-witness.json"
  ssh -o BatchMode=yes -p "$a100_port" "root@$a100_host" \
    "SP1_PROVER=cuda '$export_prover' receipt \
      --witness '$a100_root/export/receipt-witness.json' \
      --output-dir '$a100_root/export-proof' \
      --elf '$export_elf' \
      --prove \
      --require-prover cuda \
      --skip-redundant-execute"
  install -d -m 700 "$phase_dir/export-proof/proof-cuda"
  rsync -a -e "ssh -p $a100_port" \
    "root@$a100_host:$a100_root/export-proof/" \
    "$phase_dir/export-proof/proof-cuda/"
fi
jq -e '
  .program_vkey=="0x004e44aca326861252ee5ff7863b1174635b727759b75d46b28bb28d4a7b34f9"
  and .proof_mode=="groth16"
  and .prover_backend=="cuda"
  and .host_execute_skipped==true
  and .execute_ms==0
  and .proof_bytes==356
  and .public_values_bytes==1120
' "$phase_dir/export-proof/proof-cuda/proof-report.json" >/dev/null

python3 scripts/a666-mainnet-accept-and-mint.py \
  --execute \
  --receipt-witness "$phase_dir/export-proof/receipt-witness.json" \
  --proof-dir "$phase_dir/export-proof/proof-cuda" \
  --state-file "$phase_dir/ethereum/mint-state.json" \
  --expected-finalized-height "$export_height" \
  > "$phase_dir/ethereum/execute.stdout.json"
jq -e \
  --argjson amount "$expected_amount_atoms" \
  '.phase=="minted-to-recipient"
   and (.post_state.recipient_balance_atoms-.pre_state.recipient_balance_atoms)==$amount
   and (.post_state.token_total_supply-.pre_state.token_total_supply)==$amount' \
  "$phase_dir/ethereum/mint-state.json" >/dev/null

bash scripts/a666-mainnet-record-destination-consume.sh \
  --resume-auto \
  --phase-dir "$phase_dir" \
  --workflow-id "$workflow_id" \
  --expected-pftl-height "$((export_height + 1))"

jq -n \
  --arg packet "$packet_hash" \
  --arg recipient "$expected_recipient" \
  --argjson amount "$expected_amount_atoms" \
  --argjson export_height "$export_height" \
  --argjson verifier_height_before "$verifier_height_before" \
  --argjson receipt_prior_height "$verifier_height" \
  --arg mint_tx "$(jq -er '.transactions[] | select(.label=="consume finalized A666 mint packet") | .tx' "$phase_dir/ethereum/mint-state.json")" \
  '{schema:"postfiat.a666.wallet_export_completion.v1",verdict:"PASS",packet_hash:$packet,
    recipient:$recipient,amount_atoms:$amount,export_height:$export_height,
    verifier_height_before:$verifier_height_before,receipt_prior_height:$receipt_prior_height,
    mint_tx:$mint_tx}' \
  | tee "$phase_dir/completion.json"
