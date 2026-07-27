#!/usr/bin/env bash
set -euo pipefail

: "${PFTL_FLEET_FILE:?set PFTL_FLEET_FILE to the validator inventory}"
: "${PFTL_SSH_KEY:?set PFTL_SSH_KEY to the fleet SSH identity}"
fleet=$PFTL_FLEET_FILE
ssh_key=$PFTL_SSH_KEY
v1=$(awk '$1=="validator-1"{print $2}' "$fleet")
v2=$(awk '$1=="validator-2"{print $2}' "$fleet")
v3=$(awk '$1=="validator-3"{print $2}' "$fleet")
node=/opt/postfiat/releases/pfusdc-eth-l1-f30d368/postfiat-node
topology=/etc/postfiat/releases/pfusdc-eth-l1-f30d368/topology.json
run=/var/lib/postfiat/validator-2/pfusdc-latency-20260727-run2
local_evidence=$(cd "$(dirname "$0")" && pwd)
issuer_key=/var/lib/postfiat/validator-2/fast-ingress-live/pfusdc-issuer-key.json
holder_key=/var/lib/postfiat/validator-2/closed-roundtrip/keys/holder.json
asset=02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b
policy=928eaf6cef31bd832f67a89e02b5c9195763c59505dadd46c7439679643b26a06e5a6269ae41de2bb2ef2960716a7c81
issuer=pf23d8831301aa1cce6fdd7bf4a2db2aead1619ba8
holder=pfab9b9228942e5c529633a13aa271d5297bec6353
recipient=0xe568f9bbc54101dd0fad10b37116a1e40b8ae8cc
deposit_tx=0xd2e7254f1b2fd9c73b536b814ae50f7a91149086981096348c93731d82666729
bucket=1e2e40b622f0a3e100380fb89956e1abbbbb690ae66b393f6a537413ac3c8251fe49a737588d3cbb2c3a64d0a5240eac
prior_checkpoint=32f0d07455d9d9e8e452f95bb5817260f855d3c0ea4a7e346355bbe9e1ba76ea4c5fbb3b77625a56cee4ede6dd54d004

ssh_v2() {
  ssh -i "$ssh_key" "root@$v2" "$@"
}

ssh_v2 "set -euo pipefail
  test -x '$run/cast'
  test -s '$run/ingress-proof/proof-calldata.bin'
  test -s '$run/ingress-proof/public-values.bin'
  '$node' status --data-dir /var/lib/postfiat/validator-2 --expect-height 330 >/dev/null
  '$node' vault-bridge-deposit-relay-rpc-bundle \
    --cast-bin '$run/cast' \
    --source-rpc-url https://ethereum-rpc.publicnode.com \
    --tx-hash '$deposit_tx' \
    --vault-address 0x8583409ddbac984ec195dfa06a21103d92403c1e \
    --token-address 0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48 \
    --asset-id '$asset' \
    --policy-hash '$policy' \
    --proposer '$issuer' \
    --finalizer '$issuer' \
    --claimer '$issuer' \
    --expires-at-height 1024 \
    --bundle '$run/relay-bundle' \
    --overwrite \
    --source-proof-kind sp1-ethereum-finality-v1 \
    --source-proof-file '$run/ingress-proof/proof-calldata.bin' \
    --source-public-values-file '$run/ingress-proof/public-values.bin' \
    > '$run/relay-bundle.report.json'
  '$node' pftl-certified-asset-ops-from-bundle \
    --bundle '$run/relay-bundle' \
    --output '$run/full.ops.json' \
    --proposer-key-file '$issuer_key' \
    --finalizer-key-file '$issuer_key' \
    --claimer-key-file '$issuer_key' \
    --overwrite
  jq '{schema,operations:[.operations[]|select(.label==\"propose\")]}' \
    '$run/full.ops.json' > '$run/h331-propose.ops.json'
  jq --arg holder '$holder' \
    '{schema,operations:[.operations[]|select(.label==\"finalize\" or .label==\"claim\")|if .label==\"claim\" then .operation.recipient=\$holder else . end]}' \
    '$run/full.ops.json' > '$run/h332-finalize-claim.ops.json'
  test \"\$(jq '.operations|length' '$run/h331-propose.ops.json')\" = 1
  test \"\$(jq '.operations|length' '$run/h332-finalize-claim.ops.json')\" = 2"

scp -q -3 -i "$ssh_key" \
  "root@$v2:$run/h331-propose.ops.json" \
  "root@$v1:/var/lib/postfiat/validator-1/pfusdc-latency-h331-propose.ops.json"
ssh -i "$ssh_key" "root@$v1" "set -euo pipefail
  '$node' pftl-submit-certified-asset-ops \
    --data-dir /var/lib/postfiat/validator-1 \
    --topology '$topology' \
    --key-file /var/lib/postfiat/validator-1/validator_keys.json \
    --ops-file /var/lib/postfiat/validator-1/pfusdc-latency-h331-propose.ops.json \
    --artifact-dir /var/lib/postfiat/validator-1/pfusdc-latency-h331-propose \
    --height 331 \
    --timeout-ms 180000 \
    --send-retries 3 \
    --retry-backoff-ms 250 \
    --quorum-early-full-propagation \
    --local-apply-before-certified-send" > "$local_evidence/h331.report.json"

ssh_v2 "set -euo pipefail
  '$node' status --data-dir /var/lib/postfiat/validator-2 --expect-height 331 >/dev/null
  '$node' pftl-submit-certified-asset-ops \
    --data-dir /var/lib/postfiat/validator-2 \
    --topology '$topology' \
    --key-file /var/lib/postfiat/validator-2/validator_keys.json \
    --ops-file '$run/h332-finalize-claim.ops.json' \
    --artifact-dir '$run/h332-finalize-claim' \
    --max-transactions 2 \
    --height 332 \
    --timeout-ms 180000 \
    --send-retries 3 \
    --retry-backoff-ms 250 \
    --quorum-early-full-propagation \
    --local-apply-before-certified-send \
    > '$run/h332-finalize-claim.report.json'
  test \"\$(jq -r .end_height '$run/h332-finalize-claim.report.json')\" = 332
  test \"\$(jq -r .end_mempool_pending '$run/h332-finalize-claim.report.json')\" = 0
  '$node' account-assets \
    --data-dir /var/lib/postfiat/validator-2 \
    --account '$holder' \
    --asset-id '$asset' > '$run/holder-after-claim.json'
  jq -e '.assets|any(.balance >= 1800000)' '$run/holder-after-claim.json' >/dev/null
  '$node' vault-bridge-burn-to-redeem-bundle \
    --data-dir /var/lib/postfiat/validator-2 \
    --owner '$holder' \
    --issuer '$issuer' \
    --asset-id '$asset' \
    --amount-atoms 1000000 \
    --bucket-id '$bucket' \
    --destination-ref 'evm-erc20:1:$recipient' \
    --bundle '$run/burn-bundle' \
    --overwrite > '$run/burn-bundle.report.json'
  '$node' pftl-certified-asset-ops-from-bundle \
    --bundle '$run/burn-bundle' \
    --output '$run/h333-burn.ops.json' \
    --owner-key-file '$holder_key' \
    --overwrite"

scp -q -3 -i "$ssh_key" \
  "root@$v2:$run/h333-burn.ops.json" \
  "root@$v3:/var/lib/postfiat/validator-3/pfusdc-latency-h333-burn.ops.json"
ssh -i "$ssh_key" "root@$v3" "set -euo pipefail
  '$node' status --data-dir /var/lib/postfiat/validator-3 --expect-height 332 >/dev/null
  '$node' pftl-submit-certified-asset-ops \
    --data-dir /var/lib/postfiat/validator-3 \
    --topology '$topology' \
    --key-file /var/lib/postfiat/validator-3/validator_keys.json \
    --ops-file /var/lib/postfiat/validator-3/pfusdc-latency-h333-burn.ops.json \
    --artifact-dir /var/lib/postfiat/validator-3/pfusdc-latency-h333-burn \
    --height 333 \
    --timeout-ms 180000 \
    --send-retries 3 \
    --retry-backoff-ms 250 \
    --quorum-early-full-propagation \
    --local-apply-before-certified-send" > "$local_evidence/h333.report.json"

ssh_v2 "set -euo pipefail
  '$node' status --data-dir /var/lib/postfiat/validator-2 --expect-height 333 > '$run/post-status.json'
  '$node' vault-bridge-status \
    --data-dir /var/lib/postfiat/validator-2 \
    --asset-id '$asset' > '$run/vault-bridge-status-h333.json'
  redemption_id=\$(jq -er \
    '[.redemptions[]|select(.owner==\"$holder\" and .destination_ref==\"evm-erc20:1:$recipient\" and .amount_atoms==1000000 and .created_at_height==333)]|if length==1 then .[0].redemption_id else error(\"expected one run2 redemption\") end' \
    '$run/vault-bridge-status-h333.json')
  '$node' pfusdc-egress-witness \
    --data-dir /var/lib/postfiat/validator-2 \
    --withdrawal-id \"\$redemption_id\" \
    --prior-checkpoint '$prior_checkpoint' > '$run/egress-witness.json'
  jq -n --arg redemption_id \"\$redemption_id\" \
    '{schema:\"postfiat.pfusdc.ethereum_mainnet_latency_pftl.v1\",verdict:\"PASS\",finalized_height:333,redemption_id:\$redemption_id,amount_atoms:1000000}' \
    > '$run/summary.json'
  cat '$run/summary.json'"
