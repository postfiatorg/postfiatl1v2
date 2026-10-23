#!/usr/bin/env bash
# One rollout step: apply-next, one 1-PFT faucet grant, the six-way grant check, the fleet observation.
set -uo pipefail
N="$1"; V="$2"; APPLIED="$3"
D=~/.postfiat/deployments/exchange-readiness-20261023; S=$D/stage; E=$D/evidence
PACKET=deployments/exchange-readiness-20261023
NEW=4085ac70c7bb3c21b46cc67be8cb1bbb3556bfea9f49fbf6b128cdac2adcbef9
cd ~/repos/postfiatl1v2-rel20261023
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
python3 "$E/grant-check.py" "$E/grant-after-$V.out" "$E/grant-check-after-$V.json" || { echo "FAIL grant check"; exit 1; }
python3 $PACKET/observe-fleet.py --stage "$S" --applied "$APPLIED" --output "$PACKET/observed/after-$V.json" \
  || { echo "FAIL observe-fleet"; exit 1; }
python3 - "$PACKET/observed/after-$V.json" "$APPLIED" "$NEW" <<'PY' || { echo "FAIL applied identity"; exit 1; }
import json, sys
d = json.load(open(sys.argv[1])); applied = sys.argv[2].split(","); new = sys.argv[3]
bad = []
for r in d["validators"]:
    if r["mempool_pending"] != 0: bad.append((r["validator_id"], "mempool"))
    if r["status"].get("deployment_manifest_verified") is not True: bad.append((r["validator_id"], "deployment_manifest_verified"))
    if r["validator_id"] in applied:
        for u in (f"postfiat-{r['validator_id']}.service", f"postfiat-{r['validator_id']}-rpc.service"):
            if r["processes"][u]["sha256"] != new: bad.append((r["validator_id"], u))
print(("PASS" if not bad else "FAIL"), "applied identity", applied, "on", new[:8], "manifest verified on six", bad)
sys.exit(1 if bad else 0)
PY
echo "step $N $V PASS $(date -u +%FT%TZ)"
