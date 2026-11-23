#!/usr/bin/env bash
# Run on the prepared development machine, using a verified height-1753 archive.
# Usage: test-exit-e2e.sh COPIED_STORE AGAVE_VALIDATOR MAINNET_IMAGES NEW_OUTPUT_DIR
set -euo pipefail
umask 077
if [ "$#" -ne 4 ]; then
  echo "Usage: $0 COPIED_STORE AGAVE_VALIDATOR MAINNET_IMAGES NEW_OUTPUT_DIR" >&2
  exit 2
fi
root=$(git -C "$(dirname "$0")" rev-parse --show-toplevel)
archive=$(realpath "$1")
agave=$(realpath "$2")
images=$(realpath "$3")
out=$(realpath -m "$4")
mkdir "$out"
mkdir "$out/source" "$out/evidence"
cp -a --reflink=auto "$archive" "$out/store"
git -C "$root" archive HEAD | tar -x -C "$out/source"
cd "$out/source"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-/home/postfiat/var/sol-exit/target}
python3 - <<'PY'
from pathlib import Path
p = Path("crates/types/src/solana_exit.rs")
marker = "None, // Release sets the agreed future height here."
s = p.read_text()
assert s.count(marker) == 1, "expected the unscheduled development branch"
p.write_text(s.replace(marker, "Some(1754), // isolated rehearsal"))
PY
cargo test --release --locked -p postfiat-node -p postfiat-pfusdc-proofs --lib --no-run --message-format=json > "$out/evidence/build.jsonl"
for lib in node pfusdc_proofs; do
  file=$(jq -r --arg name "postfiat_$lib" 'select(.executable and .target.name==$name)|.executable' "$out/evidence/build.jsonl")
  cp "$file" "$out/evidence/$lib"
done
run_case() {
  docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges -u "$(id -u):$(id -g)" \
    -v "$out/store:/var/lib/postfiat/validator-3" -v "$out/evidence:/evidence" \
    -e SOL_EXIT_EVIDENCE=/evidence ubuntu:24.04 "/evidence/$1" "$2" --ignored --nocapture | tee "$out/evidence/$2.log"
  rg -q '1 passed' "$out/evidence/$2.log"
}
run_case node copied_solana_exit_governance_and_burn
run_case pfusdc_proofs certify_rehearsal_burn_for_local_solana
run_case node export_local_solana_exit_chain
cargo build --release --locked -p postfiat-node --bin postfiat-node
cp "$CARGO_TARGET_DIR/release/postfiat-node" "$out/evidence/postfiat-node"
python3 ops/sol-live/rehearse_exit.py --node "$out/evidence/postfiat-node" --validator "$agave" --images "$images" --evidence "$out/evidence"
jq -e '.job.stage=="paid" and .before==0 and .after==20000 and .custody_after==980000' "$out/evidence/local-payout.json"
