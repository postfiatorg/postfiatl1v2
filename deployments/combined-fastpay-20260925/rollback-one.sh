#!/usr/bin/env bash
# EMERGENCY RECOVERY ONLY: return ONE validator from combined-fastpay-20260925 to
# fastpay-committee-20260925-r4 (executable 44b6794f...). Adapted from
# ../combined-devnet-20260923/rollback-one.sh. See DEPLOY-SHEET.md "Rollback".
#
# Before running, copy that validator's two r4 units from the local r4 stage
# into the host's r4 release directory (hashes are checked against the signed
# r4 manifest below):
#   scp .../fastpay-committee-20260925-r4/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
#       root@HOST:/etc/postfiat/releases/fastpay-committee-20260925-r4/
# Then: ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < rollback-one.sh
# HEIGHT/TIP/ROOT: the fleet's current converged identity (the pre-upgrade
# baseline if no block was committed since).
#
# Data: safe-rollout makes no per-host data anchor. The data directory is kept
# in place (nothing moved or deleted); r4 must verify it at the given identity
# or the script stops before r4 is installed. If it stops there, the fallback is
# the signed validator-1 backup in the rollout evidence (a manual import that
# preserves the node's own signer state), not this script.
set -euo pipefail
validator="${1:?validator ID required}"
height="${2:?height required}"
tip="${3:?tip required}"
root="${4:?state root required}"
case "$validator" in validator-[0-5]) ;; *) exit 64 ;; esac
old=/opt/postfiat/releases/fastpay-committee-20260925-r4/postfiat-node
config=/etc/postfiat/releases/fastpay-committee-20260925-r4
live="/var/lib/postfiat/$validator"
transport="postfiat-$validator.service"
rpc="postfiat-$validator-rpc.service"
declare -A want_transport=(
  [validator-0]=89a640de78a8c5fcf116a5981061ebf4e64dddd4ecdb3b38deb5c6e386b2cce6
  [validator-1]=eeb369fa926407cd0e1e7b9250f929b18083f9835d3460a2d2393755998cd57c
  [validator-2]=436ed7a2872a23b0d9592ea45368b87a7df192e1129a2d875723e3df58f1e837
  [validator-3]=b16a7d194e5bee18ac524217a3d66b23f111fea30a7b79d2c8efa31377d034f4
  [validator-4]=2d95ea7bbd8ca6d1f09109f9417e20787b3dfbb6d30e1946cbb68d13733832de
  [validator-5]=d6b70018a81f578c1c20f5f8959b8f931707a99c45cf4b7124cd7c4dedef6dcb
)
declare -A want_rpc=(
  [validator-0]=13d845ac6eb93569a7a8086231d0da3e78129c7131cd508bed8e9fa249538e05
  [validator-1]=c2c1ecfbc71d69c682696f24e746e1abe0609fb221a37629c7d9143f3f0c0a69
  [validator-2]=cfb304c5032fa0e52f8d06d8bc48489e76aa3aa154ef3eb22fba5d266c9db17d
  [validator-3]=15ddbd7956c41d3779cec2075225556bbc9905f19b09620921ed06519e086e58
  [validator-4]=b42cc6ad9a03b17f8480199f6a7ad2c42b27b38327c160ffa6308a74559fc6e3
  [validator-5]=c6f3dafc667406817a62d711145f88d972d1dae2968edc76af003130bd593900
)
test "$(sha256sum "$old" | cut -d' ' -f1)" = 44b6794f2f8eab577713dffb6e59880f8ee8c811863e99ed96ed1b0f4ec66bab
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
