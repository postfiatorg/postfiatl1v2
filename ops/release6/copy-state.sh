#!/usr/bin/env bash
# Pull canonical store files over the existing fleet SSH identities. Output is local.
set -euo pipefail
umask 077
out=${1:?usage: copy-state.sh LOCAL_OUTPUT VALIDATOR_NUMBER}
n=${2:?missing validator number}
[[ $n =~ ^[0-5]$ ]]
mapfile -t tunnels < <(pgrep -af "^ssh .*127[.]0[.]0[.]1:3965$n:127[.]0[.]0[.]1:2765$n root@")
(( ${#tunnels[@]} == 1 ))
host=${tunnels[0]##* }
[[ $host =~ ^root@[0-9.]+$ ]]
identity=$(awk '{for(i=1;i<NF;i++)if($i=="-i")print $(i+1)}' <<< "${tunnels[0]}")
[[ -f $identity && $identity =~ ^/[a-zA-Z0-9_./-]+$ ]]
dir=$out/validator-$n
mkdir -p "$dir"
# An allowlist excludes signers, node integrity keys and operational scratch files.
args=()
for file in genesis node_state governance ledger blocks receipts chain_tip batch_archive ordered_batches mempool shielded bridge faucet_account validator_registry validator_registry_genesis transactional_generation owned_locks fastpay_speculative_effects_v1 private_pool_snapshot_points; do
  args+=("--include=/$file.json")
done
for journal in blocks receipts batch_archive ordered_batches; do
  args+=("--include=/$journal.append.jsonl" "--include=/$journal.append.jsonl.head")
done
for directory in transactional-generation transactional-generation-927; do
  args+=("--include=/$directory/")
  for file in postfiat-state-v1.redb canonical-history.jsonl storage-migration-manifest.json storage-migration-manifest.sha3-384; do
    args+=("--include=/$directory/$file")
  done
done
for directory in consensus-v2-qcs consensus-v2-safety; do
  args+=("--include=/$directory/" "--include=/$directory/epoch-*.json")
done
rsync -rzt --sparse --no-links --stats --include=/owned_locks.wal "${args[@]}" --exclude='*' \
  -e "ssh -i $identity -o BatchMode=yes -o StrictHostKeyChecking=yes" \
  "$host:/var/lib/postfiat/validator-$n/" "$dir/"
# Never mistake a migration-era generation for the one the source selected.
python3 - "$dir" "$n" <<'PY'
import json
from pathlib import Path
import sys

store = Path(sys.argv[1])
pointer, _ = json.JSONDecoder().raw_decode((store / "transactional_generation.json").read_text())
source = Path(pointer["database_directory"])
if (source.parent != Path("/var/lib/postfiat") / f"validator-{sys.argv[2]}"
        or pointer["database_file"] != "postfiat-state-v1.redb"
        or source.name not in {"transactional-generation", "transactional-generation-927"}
        or not (store / source.name / pointer["database_file"]).is_file()):
    raise SystemExit("the source selected a generation outside this release 5 capture")
print(f"captured selected generation {source.name}")
PY
