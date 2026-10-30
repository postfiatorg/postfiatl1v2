#!/usr/bin/env bash
# EMERGENCY RECOVERY ONLY: return ONE validator from vote-records-20261030 to
# index-v3-20261027 (executable b5cf3ce4...). Adapted from
# index-v3-20261027/rollback-one.sh (branch release/index-v3-20261027). See DEPLOY-SHEET.md "Rollback".
#
# Before running, copy that validator's two index-v3-20261027 units from
# the local 2026-10-27 stage into the host's 2026-10-27 release directory (hashes
# are checked below and against the signed 2026-10-27 manifest):
#   scp ~/.postfiat/deployments/index-v3-20261027/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
#       root@HOST:/etc/postfiat/releases/index-v3-20261027/
# Then: ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < rollback-one.sh
# HEIGHT/TIP/ROOT: the fleet's current converged identity (the pre-upgrade
# baseline if no block was committed since).
#
# Data: safe-rollout makes no per-host data anchor. The data directory is kept
# in place (nothing moved or deleted); the 2026-10-27 executable must verify it
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
old=/opt/postfiat/releases/index-v3-20261027/postfiat-node
config=/etc/postfiat/releases/index-v3-20261027
live="/var/lib/postfiat/$validator"
transport="postfiat-$validator.service"
rpc="postfiat-$validator-rpc.service"
declare -A want_transport=(
  [validator-0]=bc2741861d26e4c8e66dbbb81180dccfb95a257ea2526ada197efc04b16516ad
  [validator-1]=31930af54f8f40abc7c276bdc37aa5c2f6d5ac2390957119176d536f38050264
  [validator-2]=0ced50f7d22f63ce6ef6b715bad438034c17e8aef0197e8d4807e2c614c75b26
  [validator-3]=220beed6bd7363c80d02518c6a2cdb4ebc63e83517a1424251367f9c0ceea7ea
  [validator-4]=4474ee74f05713a9d03587122d43edfa86c8c317061bdcf7a97db51b782b3acf
  [validator-5]=90ec52b3ce4b6915e2ea9e16c359be64c1616b91f51009ae556a3c70f55dd7f2
)
declare -A want_rpc=(
  [validator-0]=51961e4b7d46c7b642d4156ae0468b7bd99a02e1df308c26c506843d9c6af847
  [validator-1]=362def676ac67c59a2276a06fd340a4a84547f405bfab16612ce3579a98c7e3d
  [validator-2]=24cac63a8ac081cc9412426b4c249f6556c17d2b5dfc4d7191fb62e846a52e64
  [validator-3]=7b8949b69263e7f2a1841eb194a718a1bcd90e8fefcef825ae09c721054de474
  [validator-4]=ae875011a370d102f15281b084158fc08953f16e9c576319248e7c79491cb912
  [validator-5]=759eac6e941413d2586059b00301211e3c8a6d9fb1f17c8977258646f4eba036
)
test "$(sha256sum "$old" | cut -d' ' -f1)" = b5cf3ce4bc046e6338beca6d5ead6fd0343426ddb01ac2f461c31e2775f9c8eb
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
