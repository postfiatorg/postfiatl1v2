#!/usr/bin/env bash
# EMERGENCY DEPLOY-DAY RECOVERY ONLY. Not executed by preparation.
# Run on the target host via ssh ... bash -s -- validator-N HEIGHT TIP ROOT < this-file.
# Requires a separately verified, node-specific pre-upgrade data anchor, and
# preserved signer safety. See DEPLOY-SHEET.md. This is NOT a safe-rollout subcommand.
set -euo pipefail
validator="${1:?validator ID required}"
height="${2:?baseline height required}"
tip="${3:?baseline tip required}"
root="${4:?baseline root required}"
case "$validator" in validator-[0-5]) ;; *) exit 64 ;; esac
old=/opt/postfiat/releases/a666-source-route-20260907/postfiat-node
config=/etc/postfiat/releases/a666-source-route-20260907
live="/var/lib/postfiat/$validator"
anchor="/var/lib/postfiat/rollback/combined-devnet-20260923/$validator"
failed="/var/lib/postfiat/rollback/combined-devnet-20260923/$validator.failed"
transport="postfiat-$validator.service"
rpc="postfiat-$validator-rpc.service"
test -x "$old"
test "$(sha256sum "$old" | cut -d' ' -f1)" = 57b0f4d1d42d66878d7dbb8c33919c7fa0f87c6cc1a4b9cc1a85d75b634eec83
test -d "$live"
test -d "$anchor"
test -s "$anchor/validator_keys.json"
test -s "$anchor/.integrity.key"
test ! -e "$failed"
test -f "$config/$transport"
test -f "$config/$rpc"
systemctl stop "$rpc" "$transport"
! systemctl is-active --quiet "$rpc"
! systemctl is-active --quiet "$transport"
# Preserve failed data. Restore only at the original data path: authenticated
# transactional-generation pointers may bind that absolute location.
mv -T "$live" "$failed"
cp -a --reflink=auto "$anchor" "$live"
runuser -u postfiat -- "$old" verify-finalized-checkpoint --data-dir "$live"
runuser -u postfiat -- "$old" status --data-dir "$live" |
  python3 -c 'import json,sys; s=json.load(sys.stdin); v,h,t,r=sys.argv[1:]; assert s["node_id"]==v and s["block_height"]==int(h) and s["block_tip_hash"]==t and s["state_root"]==r and s["mempool_pending"]==0, "rollback identity differs"' "$validator" "$height" "$tip" "$root"
runuser -u postfiat -- "$old" validate-local-keys --data-dir "$live" --validators 6 --local-only
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
  --runtime-private-egress-circuit-metadata-file "$config/private-egress.metadata.json"
systemctl daemon-reload
systemctl start "$transport"
systemctl start "$rpc"
systemctl is-active "$transport" "$rpc"
# Run the workstation's six-node observation before any further action.
