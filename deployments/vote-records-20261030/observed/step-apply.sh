#!/usr/bin/env bash
# One rollout step: apply-next, one 1-PFT faucet grant, the six-way grant check, the fleet observation.
set -uo pipefail
N="$1"; V="$2"; APPLIED="$3"
D=~/.postfiat/deployments/vote-records-20261030; S=$D/stage; E=$D/evidence
PACKET=deployments/vote-records-20261030
NEW=87cfe797af8c8c1c98e08b4794473bd25acb58ce0b7f57f0ea1b1f0fe98023c7
cd ~/repos/postfiatl1v2-rel20261030
test ! -e "$E/apply-$N.json" || { echo "apply-$N.json exists"; exit 2; }
date -u +%FT%TZ > "$E/apply-$N.start"
echo "apply-$N $V start $(cat "$E/apply-$N.start")"
scripts/postfiat-safe-rollout apply-next --state-file "$E/rollout-state.json" > "$E/apply-$N.json" 2> "$E/apply-$N.stderr" \
  || { echo "FAIL apply-next rc=$?"; tail -c 2000 "$E/apply-$N.stderr"; exit 1; }
python3 - "$E/apply-$N.json" "$V" <<'PY' || exit 1
import json, sys
s = json.load(open(sys.argv[1])); v = sys.argv[2]
r = s["apply_reports"][v]; c = r["fleet_convergence"]
ok = s["applied"][-1] == v and c["verified"] and r["mempool_pending"] == 0
print(("PASS" if ok else "FAIL"), "apply", v, "applied", s["applied"], "height", r["height"], r["tip"][:12], r["state_root"][:12],
      "convergence", c["height"], c["tip"][:12], c["state_root"][:12])
sys.exit(0 if ok else 1)
PY
echo "apply-$N done $(date -u +%FT%TZ)"
(cd ~/repos/StakeHub && .venv/bin/pft --json faucet testing --asset PFT --amount 1000000) > "$E/grant-after-$V.out" 2> "$E/grant-after-$V.err" \
  || { echo "FAIL grant rc=$?"; tail -c 2000 "$E/grant-after-$V.err"; exit 1; }
python3 -c 'import json,sys; g=json.load(open(sys.argv[1])); print("grant", g["ok"], g["height"], g["proposer"], g["tx_id"][:16], g["verified_6_of_6"], g["sender_after"], g["recipient_after"])' "$E/grant-after-$V.out"
echo "grant done $(date -u +%FT%TZ)"
python3 $PACKET/observed/grant-check.py "$E/grant-after-$V.out" "$E/grant-check-after-$V.json" || { echo "FAIL grant check"; exit 1; }
echo "apply+grant $N $V PASS $(date -u +%FT%TZ)"
