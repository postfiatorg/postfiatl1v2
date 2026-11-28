#!/bin/bash
# Stage train-a-r9 from the binary in bin/. Same recipe as the earlier trains:
# units and env files are the previous release's with the release name changed.
set -euo pipefail
source /home/postfiat/var/train-a-r9/env
test -x "$B" && test -s "$EVID/source-revision"
sha256sum "$B" | cut -d" " -f1 > "$EVID/binary.sha256.tmp"; echo "$(cat $EVID/binary.sha256.tmp)  postfiat-node" > "$EVID/binary.sha256"; rm "$EVID/binary.sha256.tmp"
SHA=$(cut -d" " -f1 "$EVID/binary.sha256")
rev=$(cat "$EVID/source-revision")
genesis=$(jq -re .genesis_hash "$OC/deployment-manifest.json")
rm -rf "$S"
"$B" deployment-validator-units-stage --release-id "$REL" --topology-file "$OC/topology.json" --binary-file "$B" --swap-circuit-metadata-file "$OC/swap.metadata.json" --private-egress-circuit-metadata-file "$OC/private-egress.metadata.json" --output-dir "$S" > "$STAGE_ROOT/stage.json"
for n in {0..5}; do
  for name in "postfiat-validator-$n.service" "postfiat-validator-$n-rpc.service" "validator-$n.rpc.env" "validator-$n.transport.env"; do
    case "$name" in *.service) olddir=$OU; newdir=$U;; *) olddir=$OC; newdir=$C;; esac
    sed "s/$OLD/$REL/g" "$olddir/$name" > "$newdir/$name"
    sed "s/$REL/$OLD/g" "$newdir/$name" | cmp - "$olddir/$name"
  done
done
echo "all 24 unit and environment round-trip comparisons passed"
ln -f "$OC/deployment.public.json" "$C/deployment.public.json"
now=$(date +%s)
"$B" deployment-manifest-create --deployment-id "$REL" --valid-from-unix "$now" --valid-until-unix "$((now+31536000))" --chain-id postfiat-wan-devnet-2 --genesis-hash "$genesis" --git-revision "$rev" --binary-file "$B" --build-profile release --build-features privacy,rpc,transport --protocol-version 1 --rpc-schema postfiat-local-rpc-v1 --service-unit-file "$U/postfiat-validator-0-rpc.service" --environment-file "$C/validator-0.rpc.env" --validator-bindings-file "$S/validator-bindings.signing.json" --topology-file "$C/topology.json" --swap-circuit-metadata-file "$C/swap.metadata.json" --private-egress-circuit-metadata-file "$C/private-egress.metadata.json" --publisher-key-file /home/postfiat/.postfiat/trusted-deployment-publisher.private.json --manifest-file "$C/deployment-manifest.json" > /dev/null
for n in {0..5}; do
  "$B" deployment-manifest-verify --manifest-file "$C/deployment-manifest.json" --trusted-publisher-key-file "$OC/deployment.public.json" --validator-id "validator-$n" --validator-bindings-file "$S/validator-bindings.signing.json" --runtime-binary-file "$S/rootfs/opt/postfiat/releases/$REL/postfiat-node" --runtime-topology-file "$C/topology.json" --runtime-swap-circuit-metadata-file "$C/swap.metadata.json" --runtime-private-egress-circuit-metadata-file "$C/private-egress.metadata.json" >/dev/null
  echo "validator-$n publisher, validity, runtime and service bindings verified"
done
sha256sum "$C/deployment-manifest.json"
jq 'del(.public_key_hex,.signature_hex)' "$C/deployment-manifest.json" > "$EVID/manifest-fields.json"
test "$SHA" = "$(jq -re .binary_sha256 "$EVID/manifest-fields.json")"
echo STAGE_OK
