#!/usr/bin/env bash
# EMERGENCY RECOVERY ONLY: return ONE validator from store-handle-20261021 to
# fastlane-cap-20261014 (executable b1b3dbb1...). Adapted from
# fastlane-cap-20261014/rollback-one.sh (branch release/fastlane-cap-20261014). See DEPLOY-SHEET.md "Rollback".
#
# Before running, copy that validator's two fastlane-cap-20261014 units from
# the local 2026-10-15 stage into the host's 2026-10-15 release directory (hashes
# are checked below and against the signed 2026-10-15 manifest):
#   scp ~/.postfiat/deployments/fastlane-cap-20261014/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
#       root@HOST:/etc/postfiat/releases/fastlane-cap-20261014/
# Then: ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < rollback-one.sh
# HEIGHT/TIP/ROOT: the fleet's current converged identity (the pre-upgrade
# baseline if no block was committed since).
#
# Data: safe-rollout makes no per-host data anchor. The data directory is kept
# in place (nothing moved or deleted); the 2026-10-15 executable must verify it
# at the given identity or the script stops before that release is installed.
# If it stops there, the fallback is the signed validator-1 backup taken on
# the release day (sheet §4) in the rollout evidence (a manual import that preserves the node's own
# signer state), not this script.
set -euo pipefail
validator="${1:?validator ID required}"
height="${2:?height required}"
tip="${3:?tip required}"
root="${4:?state root required}"
case "$validator" in validator-[0-5]) ;; *) exit 64 ;; esac
old=/opt/postfiat/releases/fastlane-cap-20261014/postfiat-node
config=/etc/postfiat/releases/fastlane-cap-20261014
live="/var/lib/postfiat/$validator"
transport="postfiat-$validator.service"
rpc="postfiat-$validator-rpc.service"
declare -A want_transport=(
  [validator-0]=db95bde87ac38b4830c8a09379ce4e5b54fcaff3636fa361e5b799bd5cfbd2d6
  [validator-1]=9d7a028b48c709c7b9d44301884190e32e6ddeee78b13c81f161a92252711a32
  [validator-2]=a0f9781e7c00983b2f70672b847acde3e2b44a194e4ab96e29c1c43959a3d573
  [validator-3]=9b559acdfe14f2632ae8083585063fff80c190d199810e9a4394f1545eb92dce
  [validator-4]=e61f720c52d629866be5781bad18303fc15c3c97e7bb1242a802924d47476d12
  [validator-5]=60fd02391c10179477ab4413b31d59a09e8331fee19269200c9d7b2e54a2bd7c
)
declare -A want_rpc=(
  [validator-0]=7b6918e006d69ec4422b188b2e820f247c97bacd29fa02723a807a019659fb6e
  [validator-1]=1e4a2ff118634298b3bfa1ba83678318c559308161e3bb66c2227792a4a391a7
  [validator-2]=ef90047390552d2c3651faaff78eed92416af8f5909cf2cd493fad2d8eb4b73f
  [validator-3]=b94b96bd386a6aeaa50a76293cf4b534a4b99a56d185afb182cea49be7a314d9
  [validator-4]=cbcf96ae2f9416bfb58cb823774d938927eb21d1e6cb1795fa3bf60b245afe52
  [validator-5]=d70dc485de802b6c4bac62ec2a2713dc01923c54dba767e9fe148efb6262446b
)
test "$(sha256sum "$old" | cut -d' ' -f1)" = b1b3dbb150d3596577df3bb3fe145103d90988cf71558c4d34a035fe4bb44405
test "$(sha256sum "$config/$transport" | cut -d' ' -f1)" = "${want_transport[$validator]}"
test "$(sha256sum "$config/$rpc" | cut -d' ' -f1)" = "${want_rpc[$validator]}"
test -d "$live"
test -s "$live/.integrity.key"
systemctl stop "$rpc" "$transport"
! systemctl is-active --quiet "$rpc"
! systemctl is-active --quiet "$transport"
runuser -u postfiat -- "$old" verify-finalized-checkpoint --data-dir "$live" >/dev/null
runuser -u postfiat -- "$old" status --data-dir "$live" |
  python3 -c 'import json,sys; s=json.load(sys.stdin); v,h,t,r=sys.argv[1:]; assert s["node_id"]==v and s["block_height"]==int(h) and s["block_tip_hash"]==t and s["state_root"]==r and s["mempool_pending"]==0, "rollback identity differs"' "$validator" "$height" "$tip" "$root"
runuser -u postfiat -- "$old" validate-local-keys --data-dir "$live" --validators 6 --local-only >/dev/null
install -m 0644 "$config/$transport" "/etc/systemd/system/$transport"
install -m 0644 "$config/$rpc" "/etc/systemd/system/$rpc"
"$old" deployment-manifest-verify \
  --manifest-file "$config/deployment-manifest.json" \
  --trusted-publisher-key-file "$config/deployment.public.json" \
  --validator-id "$validator" \
  --validator-bindings-file "$config/$validator.bindings.json" \
  --runtime-binary-file "$old" \
  --runtime-topology-file "$config/topology.json" \
  --runtime-swap-circuit-metadata-file "$config/swap.metadata.json" \
  --runtime-private-egress-circuit-metadata-file "$config/private-egress.metadata.json" >/dev/null
systemctl daemon-reload
systemctl start "$transport"
systemctl start "$rpc"
systemctl is-active "$transport" "$rpc"
# Then run observe-fleet.py from the workstation before any further action.
