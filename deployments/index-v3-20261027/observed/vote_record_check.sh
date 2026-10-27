#!/usr/bin/env bash
# Sheet section 8: one faucet grant, the six-way grant check, then read-only on the proposer the vote outcome record
# of that height. Usage: vote_record_check.sh LABEL
set -uo pipefail
LABEL="$1"
D=~/.postfiat/deployments/index-v3-20261027; E=$D/evidence
OUT=$HOME/repos/postfiatl1v2-rel20261027/deployments/index-v3-20261027/observed/vote-record
mkdir -p "$OUT"
HOSTS=(64.176.220.75 95.179.184.122 66.42.48.39 149.28.63.106 95.179.179.206 45.32.110.170)
(cd ~/repos/StakeHub && .venv/bin/pft --json faucet testing --asset PFT --amount 1000000) > "$E/grant-$LABEL.out" 2> "$E/grant-$LABEL.err" \
  || { echo "FAIL grant rc=$?"; tail -c 1500 "$E/grant-$LABEL.err"; exit 1; }
python3 "$E/grant-check.py" "$E/grant-$LABEL.out" "$E/grant-check-$LABEL.json" || { echo "FAIL grant check"; exit 1; }
read -r H PROPOSER < <(python3 -c 'import json,sys; g=json.load(open(sys.argv[1])); print(g["height"], g["proposer"])' "$E/grant-$LABEL.out")
N=${PROPOSER#validator-}; HOST=${HOSTS[$N]}
echo "grant height $H proposer $PROPOSER ($HOST)"
for i in $(seq 1 20); do
  ssh -o BatchMode=yes -o ConnectTimeout=15 "root@$HOST" "test -s /var/lib/postfiat/$PROPOSER/block-vote-records/height-$H.json" && break
  sleep 3
done
ssh -o BatchMode=yes -o ConnectTimeout=15 "root@$HOST" "ls -l /var/lib/postfiat/$PROPOSER/block-vote-records/ | tail -n 5; cat /var/lib/postfiat/$PROPOSER/block-vote-records/height-$H.json" \
  > "$OUT/listing-and-record-$LABEL.txt" 2>&1 || { echo "FAIL no record on the proposer"; cat "$OUT/listing-and-record-$LABEL.txt"; exit 1; }
python3 - "$OUT/listing-and-record-$LABEL.txt" "$H" "$PROPOSER" "$E/grant-check-$LABEL.json" "$OUT/height-$H.json" <<'PY'
import json, sys
text, h, proposer, check_file, out = open(sys.argv[1]).read(), int(sys.argv[2]), sys.argv[3], sys.argv[4], sys.argv[5]
rec = json.loads(text[text.index("{"):])
json.dump(rec, open(out, "w"), indent=1)
check = json.load(open(check_file))
entries = {e["validator"]: e for e in rec["entries"]}
six = [f"validator-{i}" for i in range(6)]
ok = (rec["schema"] == "postfiat.block_vote_record.v1" and rec["block_height"] == h and rec["proposer"] == proposer
      and rec["block_hash"] == check["block_hash"] and sorted(entries) == six
      and all(e["outcome"] in ("accepted", "Accepted") and e.get("vote_id") for e in entries.values()) and check["pass"])
print(("PASS" if ok else "FAIL"), "height", h, "proposer", proposer, "block", rec["block_hash"][:12], "quorum", rec["quorum"],
      [(v, e["outcome"], (e.get("vote_id") or "")[:12], e.get("answered_after_quorum")) for v, e in sorted(entries.items())],
      "applied on six", check["checks"]["identical_block_on_6"] and check["checks"]["tip_is_grant_block_on_6"])
sys.exit(0 if ok else 1)
PY
