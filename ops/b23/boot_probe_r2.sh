#!/bin/bash
# Runs INSIDE a network-less container. Boots rpc-serve and transport-validator-serve
# against a copied validator data dir at its original path, waits for readiness,
# runs read-only queries over loopback, then stops both. Output: /out/*.
D=/var/lib/postfiat/validator-3; B=/opt/rel/postfiat-node; O=/out
mkdir -p $D/readiness $O
export POSTFIAT_PREWARM_SHIELDED_VERIFIER=1 POSTFIAT_PREWARM_ASSET_ORCHARD_SWAP_VERIFIER=1 POSTFIAT_PREWARM_ASSET_ORCHARD_PRIVATE_EGRESS_VERIFIER=1
rm -f $D/readiness/*.ready.json
POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE=$D/readiness/transport-validator.ready.json \
POSTFIAT_TRANSPORT_BLOCK_VOTE_READY_FILE=$D/readiness/transport-block-vote.ready.json \
$B transport-validator-serve --unsafe-devnet-file-signer --unsafe-devnet-json-storage --data-dir $D \
  --topology /etc/rel/topology.json --key-file $D/validator_keys.json --vote-dir $D/transport_votes \
  --bind-host 127.0.0.1 --max-connections 100 --timeout-ms 900000 --event-log $O/transport-events.ndjson \
  > $O/transport.stdout 2> $O/transport.stderr &
TP=$!
$B rpc-serve --unsafe-devnet-json-storage --data-dir $D --spool-dir $D/runtime/rpc-spool \
  --ready-file $D/readiness/rpc.ready.json --bind-host 127.0.0.1 --port 27653 --max-requests 1000 --allow-mempool-submit-finality --finality-topology /etc/rel/topology.json --finality-key-file $D/validator_keys.json --finality-proposal-key-file $D/validator_keys.json --finality-artifact-root $D/finality-artifacts --finality-timeout-ms 5000 \
  --timeout-ms 30000 --child-timeout-ms 30000 --event-log $O/rpc-events.ndjson > $O/rpc.stdout 2> $O/rpc.stderr &
RP=$!
for i in $(seq 1 240); do [ -f $D/readiness/rpc.ready.json ] && [ -f $D/readiness/transport-validator.ready.json ] && break; sleep 1; done
echo "rpc_ready=$([ -f $D/readiness/rpc.ready.json ] && echo yes || echo no) transport_ready=$([ -f $D/readiness/transport-validator.ready.json ] && echo yes || echo no) waited=${i}s transport_alive=$(kill -0 $TP 2>/dev/null && echo yes || echo no) rpc_alive=$(kill -0 $RP 2>/dev/null && echo yes || echo no)" > $O/boot.txt
q() { exec 3<>/dev/tcp/127.0.0.1/27653; printf '%s\n' "{\"version\":\"postfiat-local-rpc-v1\",\"id\":\"$1\",\"method\":\"$2\",\"params\":$3}" >&3; timeout 60 head -n 1 <&3 > $O/q-$1.json; exec 3<&-; }
q server_info server_info '{}'
q status status '{}'
q block1209 blocks '{"from_height":1209,"limit":1}'
q block1214 blocks '{"from_height":1214,"limit":1}'
q block1008 blocks '{"from_height":1008,"limit":1}'
q rcpt1209 tx '{"tx_id":"8d5f3cb096606375037064e3ece44fcabd6487ea50e7255f73dcfbf104c20a6b9cf87b7598e8b6be64a473d643b03c45"}'
q archive1209 batch_archive '{"batch_id":"8e8f2dde6ace8ea3cda95f15a254fe0a3d72c863431e9dcc83dc01ca910b7d7ce49d9a5d52b9a62b5ab3b722681ed3ea","limit":1}'
q supply navcoin_bridge_supply_status '{"route_id":"pftl-pfdep-schwab-primary-pfusdc-v1"}'
q vroute vault_bridge_route '{"asset_id":"02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b"}'
q bob account_lines '{"account":"pf5c266a1bdb18320e140961411a8c4d8383eaafb2","limit":20}'
q mgmt account '{"address":"pfee68d0a5a89e8f554bcd942d2f38659f45bfab1b"}'
q navb23 nav_reserve_proof_status '{"asset_id":"ca7845134c8e95819082fcd2f1612808e20a37dfabab98dab120f99fd2c54c18ffb50d98c0664480420c881773244627"}'
q rcptprobe pftl_uniswap_receipt '{"tx_id":"'$(printf "ab%.0s" $(seq 1 48))'"}'
q offersubmit mempool_submit_signed_offer_transaction_finality '{"signed_offer_transaction_json":"{}"}'
sleep 3
echo "transport_alive_end=$(kill -0 $TP 2>/dev/null && echo yes || echo no) rpc_alive_end=$(kill -0 $RP 2>/dev/null && echo yes || echo no)" >> $O/boot.txt
kill $RP $TP 2>/dev/null; wait 2>/dev/null
