#!/usr/bin/env bash
# Run the verification core of rollback-one.sh locally: the store-handle-20261021 executable
# checks the six disposable copies the new build ran six services on (startup and restart at 1021), before any unit would be installed.
set -euo pipefail
Q=$HOME/.cache/release-exchange-readiness-20261023; OLD=$Q/binaries/rollback
test "$(sha256sum "$OLD" | cut -d' ' -f1)" = eea96ba6761ed223047d7781663100d34efe5641d36031bd50874bd9c8b41f36
read -r height tip root < <(python3 -c 'import json; s=json.load(open("'$Q'/new-build-service-receipt.json"))["expected"]; print(s["block_height"], s["block_tip_hash"], s["state_root"])')
for n in ${NODES:-0 1 2 3 4 5}; do
  live=$Q/working/validator-$n
  test -s "$live/.integrity.key"
  "$OLD" verify-finalized-checkpoint --data-dir "$live" >/dev/null
  "$OLD" status --data-dir "$live" |
    python3 -c 'import json,sys; s=json.load(sys.stdin); v,h,t,r=sys.argv[1:]; assert s["node_id"]==v and s["block_height"]==int(h) and s["block_tip_hash"]==t and s["state_root"]==r and s["mempool_pending"]==0, "rollback identity differs"' "validator-$n" "$height" "$tip" "$root"
  echo "validator-$n: checkpoint and identity PASS at $height ${tip:0:8} ${root:0:8}"
  if test -s "$live/faucet_key.json"; then "$OLD" validate-local-keys --data-dir "$live" --validators 6 --local-only >/dev/null && echo "validator-$n: validate-local-keys PASS"
  else echo "validator-$n: validate-local-keys NOT RUN (the archived copy has no faucet key file; hosts do)"; fi
done
