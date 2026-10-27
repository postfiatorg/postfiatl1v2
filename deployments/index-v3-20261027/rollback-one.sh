#!/usr/bin/env bash
# EMERGENCY RECOVERY ONLY: return ONE validator from index-v3-20261027 to
# exchange-readiness-20261023 (executable 4085ac70...). Adapted from
# exchange-readiness-20261023/rollback-one.sh (branch release/exchange-readiness-20261023). See DEPLOY-SHEET.md "Rollback".
#
# Before running, copy that validator's two exchange-readiness-20261023 units from
# the local 2026-10-23 stage into the host's 2026-10-23 release directory (hashes
# are checked below and against the signed 2026-10-23 manifest):
#   scp ~/.postfiat/deployments/exchange-readiness-20261023/stage/rootfs/etc/systemd/system/postfiat-validator-N{,-rpc}.service \
#       root@HOST:/etc/postfiat/releases/exchange-readiness-20261023/
# Then: ssh root@HOST bash -s -- validator-N HEIGHT TIP ROOT < rollback-one.sh
# HEIGHT/TIP/ROOT: the fleet's current converged identity (the pre-upgrade
# baseline if no block was committed since).
#
# Data: safe-rollout makes no per-host data anchor. The data directory is kept
# in place (nothing moved or deleted); the 2026-10-23 executable must verify it
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
old=/opt/postfiat/releases/exchange-readiness-20261023/postfiat-node
config=/etc/postfiat/releases/exchange-readiness-20261023
live="/var/lib/postfiat/$validator"
transport="postfiat-$validator.service"
rpc="postfiat-$validator-rpc.service"
declare -A want_transport=(
  [validator-0]=1849a4bba37a3539100b09b9c09f3e8e89ee109e8b9cb42009b71de4400ef95d
  [validator-1]=6683d0262afb0a34948c3b60c996b0742db4a1e3aa28e0135a567e5d06f97f18
  [validator-2]=ef0e524813e1b234a6e65030e3d13b236d244a857a360b5a3d036ea451a33506
  [validator-3]=53f19555e5c06ab319938fbd9027fa3314757d48c8b16afe1c8c116a5726e89f
  [validator-4]=0d920505bead0e335276d7c7e6c767bd07223266ee930a42a19098193cd619a4
  [validator-5]=7ed8a7e67b198fbafcd0f4ac5d1ddfe6098a1756cd9bbd389cd9386f912f76fa
)
declare -A want_rpc=(
  [validator-0]=5be4c86bd1abfdaabdd23eddb5080a8342c606b4f7a32d518619d58e0ccf169c
  [validator-1]=7aa324e91b4e951e0a054af0a09e75de5494b53bb325e963e1ad51f2f5f0d1fb
  [validator-2]=339d41c9c36235e557dc4494aa8b8ea35c87ec11b92878bfc074d8c8d40b05ca
  [validator-3]=70ca6463adb81bb4c181a729ef38c6510d2b9a2369bba8326113b5529043c636
  [validator-4]=bb4dab5074197c0bcf113fcf3fe07ce96ccf8a6f85ae6132381b17434580a812
  [validator-5]=f01544d37703302a1874dc5248ad2dee8e03a5f5ef631f179b6afbbb9542f0d1
)
test "$(sha256sum "$old" | cut -d' ' -f1)" = 4085ac70c7bb3c21b46cc67be8cb1bbb3556bfea9f49fbf6b128cdac2adcbef9
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
