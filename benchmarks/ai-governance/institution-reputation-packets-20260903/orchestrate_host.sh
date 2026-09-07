#!/bin/bash
# Push the frozen package to one Vast host, launch the pinned SGLang profile, run the
# fixed-batch pass twice, and pull the raw outputs back. Idempotent per host.
#
# usage: orchestrate_host.sh <ssh-host> <ssh-port> <run-prefix: primary|replay>
set -euo pipefail
HOST="$1"; PORT="$2"; PREFIX="$3"
PKG_DIR="$(cd "$(dirname "$0")" && pwd)"
CORPUS_DIR="$(cd "$PKG_DIR/../validator-identity-packets-20260901" && pwd)"
SSH=(ssh -i "$HOME/.ssh/id_ed25519" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -o ServerAliveInterval=30 -p "$PORT" "root@$HOST")
REMOTE=/root/institution-reputation-packets

"${SSH[@]}" "mkdir -p $REMOTE/inputs $REMOTE/outputs"
scp -i "$HOME/.ssh/id_ed25519" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -P "$PORT" \
  "$PKG_DIR/manifest.json" "$PKG_DIR/run_host.py" "$PKG_DIR/bootstrap_host.sh" "root@$HOST:$REMOTE/"
scp -i "$HOME/.ssh/id_ed25519" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -P "$PORT" \
  "$PKG_DIR"/inputs/*.json "$PKG_DIR/inputs/prompt.txt" "root@$HOST:$REMOTE/inputs/"

# Verify the package landed byte-exact before anything runs.
"${SSH[@]}" "cd $REMOTE && python3 - <<'PY'
import hashlib, json
m = json.load(open('manifest.json'))
for name, key in (('inputs/requests.json','requests_sha256'),('inputs/prompt.txt','prompt_sha256'),('inputs/batch_schedule.json','batch_schedule_sha256')):
    assert hashlib.sha256(open(name,'rb').read()).hexdigest() == m[key], name
print('package hashes ok')
PY"

"${SSH[@]}" "cd $REMOTE && chmod +x bootstrap_host.sh && ./bootstrap_host.sh"
"${SSH[@]}" "cd $REMOTE && cat host_identity.txt && python3 run_host.py ${PREFIX}-run1 && python3 run_host.py ${PREFIX}-run2"

scp -i "$HOME/.ssh/id_ed25519" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -P "$PORT" \
  "root@$HOST:$REMOTE/outputs/${PREFIX}-run1.json" "root@$HOST:$REMOTE/outputs/${PREFIX}-run2.json" \
  "$PKG_DIR/outputs/"
scp -i "$HOME/.ssh/id_ed25519" -o StrictHostKeyChecking=no -o UserKnownHostsFile=/dev/null -P "$PORT" \
  "root@$HOST:$REMOTE/host_identity.txt" "$PKG_DIR/outputs/host_identity_${PREFIX}.txt"
"${SSH[@]}" "cd $REMOTE && tail -3 sglang.log" || true
echo "DONE ${PREFIX} $(sha256sum "$PKG_DIR/outputs/${PREFIX}-run1.json" | cut -c1-16) $(sha256sum "$PKG_DIR/outputs/${PREFIX}-run2.json" | cut -c1-16)"
