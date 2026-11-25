#!/bin/bash
# Second release: settles every finalized Solana payout of any covered batch on the L1.
# Each validator's own reviewer approves the payout; this loop only drafts and collects (5 of 6).
set -o pipefail
cd "${SETTLER_DIR:-/home/postfiat/var/sol-exit/sol-exit-r2}" || exit 1
export PYTHONPATH=$PWD/ops/sol-live EXIT_WORK=$HOME/var/sol-exit
REVISION=${SETTLER_REVISION:-04600868}
RELEASE_HEIGHT=${SETTLER_RELEASE_HEIGHT:-1890}
# One settlement waits on the hosts' reviewers; a hung one must not stop the loop.
ATTEMPT_SECONDS=${SETTLER_ATTEMPT_SECONDS:-900}
errors=$(mktemp) || exit 1
trap 'rm -f -- "$errors"' EXIT

settle_pass() {
  local burns burn
  # Reads fall back across validators; a failed read is reported, not taken for "nothing to settle".
  if ! burns=$(timeout "$ATTEMPT_SECONDS" python3 - 2>"$errors" <<'P'
from exit_payout import *
from exit_payout import chain_rules, covered_buckets, rpc_any, signature
c=cluster()
s=rpc_any("vault_bridge_status",asset_id=ASSET)
ok=covered_buckets(s["buckets"],chain_rules()["covered"])
for r in s["redemptions"]:
    if r["bucket_id"] in ok and not r.get("settlement_receipt_hash") and r.get("state")=="pending" and signature(c,r["burn_tx_id"],None): print(r["burn_tx_id"])
P
  ); then
    echo "$(date -u +%T) discovery failed: $(tail -n 1 "$errors")"
    return 1
  fi
  for burn in $burns; do
    echo "$(date -u +%T) settling $burn"
    # The one reviewed settlement path; it checks the payout against the agreed record itself.
    timeout "$ATTEMPT_SECONDS" python3 ops/sol-live/exit_govern.py paid --burn "$burn" --revision "$REVISION" \
      --release-height "$RELEASE_HEIGHT" --output "$EXIT_WORK/settled-$burn.json" 2>&1 | tail -3
    echo "$(date -u +%T) settle $burn exit ${PIPESTATUS[0]}"
  done
}

while :; do
  settle_pass
  [[ -n ${SETTLER_ONCE:-} ]] && exit 0
  sleep 90
done
