#!/usr/bin/env bash
# Stage exchange-readiness-20261023 and sign its deployment manifest, as store-handle-20261021 tools/stage.sh and
# deployments/release-repair-20260928/canary-backup/prepare_rollout.sh (stage and sign only).
# Local only: no preflight, no backup, no host contact. Keys are passed by path only.
set -euo pipefail

D=$HOME/.postfiat/deployments/exchange-readiness-20261023
S=$D/stage; E=$D/evidence; R=exchange-readiness-20261023
C=$S/rootfs/etc/postfiat/releases/$R; U=$S/rootfs/etc/systemd/system
B=$S/rootfs/opt/postfiat/releases/$R/postfiat-node
Q=$HOME/.cache/release-exchange-readiness-20261023
BIN=$Q/binaries/candidate-1
L=$HOME/.postfiat/deployments/combined-fastpay-20260925/r4-layout-validator-0
PUBK=$HOME/.postfiat/deployments/fastpay-committee-20260925-r4/keys
SRC=6604c93e92c0bb5eac37837bb1669b987426469a
step() { echo "$(date -u +%FT%TZ) $*"; }
test ! -e "$D"
mkdir -p "$E"; chmod 700 "$D" "$E"
step "binary $(sha256sum "$BIN" | cut -d' ' -f1)"
step stage
"$BIN" deployment-validator-units-stage --release-id $R \
  --topology-file $L/topology.json --binary-file "$BIN" \
  --swap-circuit-metadata-file $L/swap.metadata.json \
  --private-egress-circuit-metadata-file $L/private-egress.metadata.json \
  --output-dir $S > $E/stage-command.stdout 2> $E/stage-command.stderr
install -m 0644 $PUBK/deployment.public.json $C/deployment.public.json
cmp "$BIN" "$B"
FROM=$(date -u +%s); UNTIL=$((FROM + 31536300))
echo "from=$FROM until=$UNTIL" > $E/manifest-validity.txt
step sign
"$B" deployment-manifest-create --deployment-id $R \
  --valid-from-unix $FROM --valid-until-unix $UNTIL \
  --chain-id postfiat-wan-devnet-2 --genesis-hash ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9 \
  --git-revision $SRC --binary-file "$B" \
  --build-profile release --build-features privacy,rpc,transport \
  --protocol-version 1 --rpc-schema postfiat-local-rpc-v1 \
  --service-unit-file $U/postfiat-validator-0-rpc.service \
  --environment-file $C/validator-0.rpc.env \
  --validator-bindings-file $S/validator-bindings.signing.json \
  --topology-file $C/topology.json --swap-circuit-metadata-file $C/swap.metadata.json \
  --private-egress-circuit-metadata-file $C/private-egress.metadata.json \
  --publisher-key-file $PUBK/deployment-publisher.key.json \
  --manifest-file $C/deployment-manifest.json > $E/manifest-create.stdout 2> $E/manifest-create.stderr
step "manifest $(sha256sum $C/deployment-manifest.json | cut -d' ' -f1)"
mkdir -p $E/verified-records
for n in 0 1 2 3 4 5; do
  POSTFIAT_DEPLOYMENT_VERIFIED_RECORD=$E/verified-records/validator-$n.json \
  "$B" deployment-manifest-verify --manifest-file $C/deployment-manifest.json \
    --trusted-publisher-key-file $C/deployment.public.json --validator-id validator-$n \
    --validator-bindings-file $C/validator-$n.bindings.json --runtime-binary-file "$B" \
    --runtime-topology-file $C/topology.json --runtime-swap-circuit-metadata-file $C/swap.metadata.json \
    --runtime-private-egress-circuit-metadata-file $C/private-egress.metadata.json \
    > $E/verify-validator-$n.json 2> $E/verify-validator-$n.stderr
  step "verify validator-$n ok, record $(test -s $E/verified-records/validator-$n.json && echo written)"
done
step done
