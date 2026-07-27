#!/usr/bin/env bash
set -euo pipefail

node=/opt/postfiat/releases/pfusdc-eth-l1-f30d368/postfiat-node
data_dir=/var/lib/postfiat/validator-2
topology=/etc/postfiat/releases/pfusdc-eth-l1-f30d368/topology.json
validator_key=/var/lib/postfiat/validator-2/validator_keys.json
issuer_key=/var/lib/postfiat/validator-2/fast-ingress-live/pfusdc-issuer-key.json
holder_key=/var/lib/postfiat/validator-2/pfusdc-latency-20260727/holder-key.json
run_dir=/var/lib/postfiat/validator-2/pfusdc-latency-20260727
proof_dir="$run_dir/ingress-proof"
cast_bin="$run_dir/cast"
bundle="$run_dir/relay-bundle"
asset=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
policy=928eaf6cef31bd832f67a89e02b5c9195763c59505dadd46c7439679643b26a06e5a6269ae41de2bb2ef2960716a7c81
issuer=pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8
holder=pf20b271cb50b72a44c49a44bfacf2726a74dbe8d5
recipient=0xe568f9bbc54101dd0fad10b37116a1e40b8ae8cc
deposit_tx=0x316c5693ac4d20e11897d46ea851bd84d5ca6aa1394e320de6578ea74712a236
prior_checkpoint=63723c1725a7d150ac743797a1db16fc96394e9f2721bf0d8bacb7658ddfeaa073bcd9c56c69694a0b7ec36c956dc588

mkdir -p "$run_dir"
for path in "$node" "$topology" "$validator_key" "$issuer_key" "$holder_key" "$cast_bin" \
  "$proof_dir/proof-calldata.bin" "$proof_dir/public-values.bin"; do
  test -f "$path"
done

"$node" status --data-dir "$data_dir" > "$run_dir/pre-status.json"
test "$(jq -r .block_height "$run_dir/pre-status.json")" = 326
test "$(jq -r .mempool_pending "$run_dir/pre-status.json")" = 0

"$node" vault-bridge-deposit-relay-rpc-bundle \
  --cast-bin "$cast_bin" \
  --source-rpc-url https://ethereum-rpc.publicnode.com \
  --tx-hash "$deposit_tx" \
  --vault-address 0x8583409ddbac984ec195dfa06a21103d92403c1e \
  --token-address 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48 \
  --asset-id "$asset" \
  --policy-hash "$policy" \
  --proposer "$issuer" \
  --finalizer "$issuer" \
  --claimer "$issuer" \
  --expires-at-height 1024 \
  --bundle "$bundle" \
  --overwrite \
  --source-proof-kind sp1-ethereum-finality-v1 \
  --source-proof-file "$proof_dir/proof-calldata.bin" \
  --source-public-values-file "$proof_dir/public-values.bin" \
  > "$run_dir/relay-bundle.report.json"

"$node" pftl-certified-asset-ops-from-bundle \
  --bundle "$bundle" \
  --output "$run_dir/full.ops.json" \
  --proposer-key-file "$issuer_key" \
  --finalizer-key-file "$issuer_key" \
  --claimer-key-file "$issuer_key" \
  --overwrite

jq '{schema, operations: [.operations[] | select(.operation.operation == "vault_bridge_deposit_propose")]}' \
  "$run_dir/full.ops.json" > "$run_dir/propose.ops.json"
jq --arg holder "$holder" '{
  schema,
  operations: [
    .operations[]
    | select(.operation.operation == "vault_bridge_deposit_finalize" or .operation.operation == "vault_bridge_deposit_claim")
    | if .operation.operation == "vault_bridge_deposit_claim"
      then .operation.recipient = $holder
      else .
      end
  ]
}' \
  "$run_dir/full.ops.json" > "$run_dir/finalize-claim.ops.json"
test "$(jq '.operations | length' "$run_dir/propose.ops.json")" = 1
test "$(jq '.operations | length' "$run_dir/finalize-claim.ops.json")" = 2

"$node" pftl-submit-certified-asset-ops \
  --data-dir "$data_dir" \
  --topology "$topology" \
  --key-file "$validator_key" \
  --ops-file "$run_dir/propose.ops.json" \
  --artifact-dir "$run_dir/h327-propose" \
  --height 327 \
  --timeout-ms 180000 \
  --send-retries 3 \
  --retry-backoff-ms 250 \
  --quorum-early-full-propagation \
  --local-apply-before-certified-send \
  > "$run_dir/h327-propose.report.json"

"$node" pftl-submit-certified-asset-ops \
  --data-dir "$data_dir" \
  --topology "$topology" \
  --key-file "$validator_key" \
  --ops-file "$run_dir/finalize-claim.ops.json" \
  --artifact-dir "$run_dir/h328-finalize-claim" \
  --height 328 \
  --timeout-ms 180000 \
  --send-retries 3 \
  --retry-backoff-ms 250 \
  --quorum-early-full-propagation \
  --local-apply-before-certified-send \
  > "$run_dir/h328-finalize-claim.report.json"

"$node" account-assets \
  --data-dir "$data_dir" \
  --account "$holder" \
  --asset-id "$asset" > "$run_dir/issuer-assets-after-claim.json"
jq -e '.assets | any(.balance >= 1000000)' \
  "$run_dir/issuer-assets-after-claim.json" >/dev/null

"$node" vault-bridge-burn-to-redeem-bundle \
  --data-dir "$data_dir" \
  --owner "$holder" \
  --issuer "$issuer" \
  --asset-id "$asset" \
  --amount-atoms 1000000 \
  --destination-ref "evm-erc20:1:$recipient" \
  --bundle "$run_dir/burn-bundle" \
  --overwrite > "$run_dir/burn-bundle.report.json"

"$node" pftl-certified-asset-ops-from-bundle \
  --bundle "$run_dir/burn-bundle" \
  --output "$run_dir/burn.ops.json" \
  --owner-key-file "$holder_key" \
  --overwrite

"$node" pftl-submit-certified-asset-ops \
  --data-dir "$data_dir" \
  --topology "$topology" \
  --key-file "$validator_key" \
  --ops-file "$run_dir/burn.ops.json" \
  --artifact-dir "$run_dir/h329-burn" \
  --height 329 \
  --timeout-ms 180000 \
  --send-retries 3 \
  --retry-backoff-ms 250 \
  --quorum-early-full-propagation \
  --local-apply-before-certified-send \
  > "$run_dir/h329-burn.report.json"

"$node" vault-bridge-status --data-dir "$data_dir" --asset-id "$asset" \
  > "$run_dir/vault-bridge-status-h329.json"
redemption_id=$(
  jq -er --arg owner "$holder" --arg destination "evm-erc20:1:$recipient" '
    [.redemptions[]
      | select(
          .owner == $owner
          and .destination_ref == $destination
          and .amount_atoms == 1000000
        )]
    | if length == 1 then .[0].redemption_id else error("expected one latency redemption") end
  ' "$run_dir/vault-bridge-status-h329.json"
)

"$node" pfusdc-egress-witness \
  --data-dir "$data_dir" \
  --withdrawal-id "$redemption_id" \
  --prior-checkpoint "$prior_checkpoint" > "$run_dir/egress-witness.json"
"$node" status --data-dir "$data_dir" > "$run_dir/post-status.json"

jq -n \
  --arg redemption_id "$redemption_id" \
  --argjson finalized_height "$(jq .block_height "$run_dir/post-status.json")" \
  '{
    schema: "postfiat.pfusdc.ethereum_mainnet_latency_pftl.v1",
    verdict: "PASS",
    finalized_height: $finalized_height,
    redemption_id: $redemption_id,
    amount_atoms: 1000000
  }'
