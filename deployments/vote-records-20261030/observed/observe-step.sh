#!/usr/bin/env bash
# Re-run only the fleet observation and the applied-identity check of one rollout step (read-only).
set -uo pipefail
V="$1"; APPLIED="$2"
D=~/.postfiat/deployments/vote-records-20261030; S=$D/stage; E=$D/evidence
PACKET=deployments/vote-records-20261030
NEW=87cfe797af8c8c1c98e08b4794473bd25acb58ce0b7f57f0ea1b1f0fe98023c7
cd ~/repos/postfiatl1v2-rel20261030
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
echo "observe $V PASS $(date -u +%FT%TZ)"
