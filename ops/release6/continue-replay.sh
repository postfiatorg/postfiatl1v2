#!/usr/bin/env bash
# Resume a verified height-1208 portable store across the archived release boundary.
set -euo pipefail
base=${1:?local evidence root}
n=${2:?validator number}
target=${3:?target height}
[[ $n =~ ^[0-5]$ ]]
r5=/home/postfiat/var/b24-release5-20261117/bin/postfiat-node
era="$base/era1209/postfiat-node"
echo "35513b7ce319162e070fab7ef6db8b6c75bec99201d04307bb8ffb0a9a89299a  $era" | sha256sum --check
store="$base/restore/validator-$n"
"$r5" status --data-dir "$store" | python3 -c 'import json,sys; assert json.load(sys.stdin)["block_height"] == 1208'
/usr/bin/time -p python3 ops/release6/catch-up.py "$era" "$store" "$base/era1209/replay-v$n" "$n" 1209
/usr/bin/time -p python3 ops/release6/catch-up.py "$r5" "$store" "$base/catchup-v$n-after" "$n" "$target"
