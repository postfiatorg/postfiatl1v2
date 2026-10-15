#!/usr/bin/env bash
# EMERGENCY RECOVERY ONLY: return ONE validator from fastlane-cap-20261014 to
# signer-rotation-20261007 (executable decaa411...). Adapted from
# signer-rotation-20261007/rollback-one.sh (branch release/signer-rotation-20261007). See DEPLOY-SHEET.md "Rollback".
#
# Before running, copy that validator's two signer-rotation-20261007 units from
# the local 2026-09-28 stage into the host's 2026-10-07 release directory (hashes
# are checked below and against the signed 2026-10-07 manifest):
#   scp ~/.postfiat/deployments/signer-rotation-20261007/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
#       root@HOST:/etc/postfiat/releases/signer-rotation-20261007/
# Then: ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < rollback-one.sh
# HEIGHT/TIP/ROOT: the fleet's current converged identity (the pre-upgrade
# baseline if no block was committed since).
#
# Data: safe-rollout makes no per-host data anchor. The data directory is kept
# in place (nothing moved or deleted); the 2026-10-07 executable must verify it
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
old=/opt/postfiat/releases/signer-rotation-20261007/postfiat-node
config=/etc/postfiat/releases/signer-rotation-20261007
live="/var/lib/postfiat/$validator"
transport="postfiat-$validator.service"
rpc="postfiat-$validator-rpc.service"
declare -A want_transport=(
  [validator-0]=b19584e79b07dae6bd2a88d8c3384ae5203b4ff883b059d56d590a4d2bdb0362
  [validator-1]=b75da7d0b2cc8d1a291b77ca5d088c27566dd2208e381d2ad5d5fc7d032e9bbb
  [validator-2]=8bf202e796753a5cd2b5e261d59ee87eb7ef0dc9561e87a6964cdfda707c7d18
  [validator-3]=77d88a0f599c7097851394e08b4146804305bb10bdc9af6cc5ef607fc9e2696c
  [validator-4]=090d3cf3578f2ca3cb4ce59ecccef7cb5e4da6cb17d28b39c5ca72352c34eb03
  [validator-5]=5691e3b4f41ab50340fcad45989d9326359c78f73e81a655b27863bac919b6c3
)
declare -A want_rpc=(
  [validator-0]=682cae64a0c85559002794bb11394e88b3a098c885b7a490bfe4ed218e5fc73f
  [validator-1]=0a7e5ff10bcadc8b504a263d6af57a074fedb3c8c9ca59ea54ebe40dcc97da1f
  [validator-2]=df3703f0258c2ecc5d6da641f64952b45e674e08dc754786bce323f52a4d9db3
  [validator-3]=3632f4ea4bc42072aefecbd44395ff47d1cfefd9ea9de636cc4dda37f75a4f4a
  [validator-4]=b71c1045a99b9f3200894cdb022f9df75c34bb30bd177fb1e5dd30d2099f5c61
  [validator-5]=4b8a89d3a5e695897b96f18d684b288b3922d8bfc9334d99f3ad3e3999a9a88f
)
test "$(sha256sum "$old" | cut -d' ' -f1)" = decaa411376a125fb377a29037b6dd470afb2208a268d30a3ce8130f571b370f
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
