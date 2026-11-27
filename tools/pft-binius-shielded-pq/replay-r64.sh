#!/usr/bin/env bash
# Compare independent executable replays on isolated copies of R64 validator-3.
set -euo pipefail
if [[ $# != 4 ]]; then
  echo "usage: $0 ARCHIVED_VALIDATOR_3 PRE_H_NODE POST_H_NODE NEW_OUTPUT_DIRECTORY" >&2
  exit 2
fi
archive=$(realpath "$1")
old=$(realpath "$2")
new=$(realpath "$3")
out=$(realpath -m "$4")
umask 077
mkdir "$out"
for version in pre post; do
  bin=$old
  if [[ $version == post ]]; then bin=$new; fi
  # The retained transactional pointer references this absolute container path.
  copy="$out/$version"
  cp -a --reflink=auto "$archive" "$copy"
  node=(docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges
    -u "$(id -u):$(id -g)" -v "$copy:/var/lib/postfiat/validator-3"
    -v "$bin:/opt/node:ro" ubuntu:24.04 /opt/node)
  sha256sum "$bin"
  for height in 1 513 1025 1537; do
    "${node[@]}" blocks --data-dir /var/lib/postfiat/validator-3 --from-height "$height" --limit 512 |
      python3 -c 'import json, sys
for block in json.load(sys.stdin):
    header = block["header"]
    print(header["height"], header["block_hash"], header["state_root"])'
  done >"$out/$version.rows"
  (cd "$out" && echo "c28eb87280050e96e996ad5c9e8c2eae12f2fb9774438f3a7b5f1295ef155910  $version.rows" | sha256sum -c -)
  "${node[@]}" verify-blocks --data-dir /var/lib/postfiat/validator-3 >"$out/$version.json"
done
cmp "$out/pre.rows" "$out/post.rows"
python3 - "$out" <<'PY'
import json, pathlib, sys
out = pathlib.Path(sys.argv[1])
old, new = (json.loads((out / f"{v}.json").read_text()) for v in ("pre", "post"))
assert old == new and new["verified"] is True and new["block_count"] == 1734
print(f'R64 replayed {new["block_count"]} blocks with both binaries. Tip {new["tip_hash"]}, root {new["state_root"]}.')
PY
