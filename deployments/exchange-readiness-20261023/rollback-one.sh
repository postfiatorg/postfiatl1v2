#!/usr/bin/env bash
# EMERGENCY RECOVERY ONLY: return ONE validator from exchange-readiness-20261023 to
# store-handle-20261021 (executable eea96ba6...). Adapted from
# store-handle-20261021/rollback-one.sh (branch release/store-handle-20261021). See DEPLOY-SHEET.md "Rollback".
#
# Before running, copy that validator's two store-handle-20261021 units from
# the local 2026-10-21 stage into the host's 2026-10-21 release directory (hashes
# are checked below and against the signed 2026-10-21 manifest):
#   scp ~/.postfiat/deployments/store-handle-20261021/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
#       root@HOST:/etc/postfiat/releases/store-handle-20261021/
# Then: ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < rollback-one.sh
# HEIGHT/TIP/ROOT: the fleet's current converged identity (the pre-upgrade
# baseline if no block was committed since).
#
# Data: safe-rollout makes no per-host data anchor. The data directory is kept
# in place (nothing moved or deleted); the 2026-10-21 executable must verify it
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
old=/opt/postfiat/releases/store-handle-20261021/postfiat-node
config=/etc/postfiat/releases/store-handle-20261021
live="/var/lib/postfiat/$validator"
transport="postfiat-$validator.service"
rpc="postfiat-$validator-rpc.service"
declare -A want_transport=(
  [validator-0]=3c2db55dd5d8921572e0bc50a683845179ad603260078cac0aa9d3be118f49df
  [validator-1]=76801ee1b68491e52fd76f032557276dea5150abd54f3eac2244f3e4abbc0a40
  [validator-2]=cec48e4f6e557282950f94357ccbbb0717a0ef3dd9797a38661dda69ed2b654c
  [validator-3]=631703138e3a9899a1df5c1b4e297e0d09c669805106b6c3570c1fd0a24a026c
  [validator-4]=fcddf35317d4527fa55fd4d388b6e51f580969f42ff5fbbfefabad68e4c0da66
  [validator-5]=8bba869cad3de8eba160b55b26f63467ae68578e79f1355415cd05969371bdaf
)
declare -A want_rpc=(
  [validator-0]=8ec12580d2d402196138b46673e069ec1e3dca7835c2434d75a36bd788a85461
  [validator-1]=378fe754fa5c64c02dc7d387f9ae7d94ae6f39704d49c0e3c998c273484b469f
  [validator-2]=6416f14b3c9456cecb5fe71a02c2641a92b22b8fdea685d6889231c18e54e97c
  [validator-3]=f29799038addf8b88d36a084ee3d24352c0894c6bc458837c347e71695de73dd
  [validator-4]=0681bac7262be0e04f69a3987fe6886b1e3237885e783a0086b2e28e0ba3335a
  [validator-5]=98a5e8337fa695ff4364af3e9cb67a7b91534ae6409b3d3dd6bf71ebeb75fbb7
)
test "$(sha256sum "$old" | cut -d' ' -f1)" = eea96ba6761ed223047d7781663100d34efe5641d36031bd50874bd9c8b41f36
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
