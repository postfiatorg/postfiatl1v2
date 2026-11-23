#!/bin/bash
set -euo pipefail
umask 0022
if [[ ${1:-} == --probe ]]; then
  mkdir "$2/bin"
  exit
fi
if [[ ${1:-} == --help ]]; then
  echo 'Usage: install-exit-runtime.sh APP PROOF_DIR ANCHOR_WITNESS < reviewed-code.tar'
  exit
fi
[[ $# == 3 ]]
app=$1
install -d -m755 "$app" "$app/proof/bin" "$app/proof/historical"
tar -xf - -C "$app" --no-same-owner --no-same-permissions
install -m755 "$2/bin/proof-host" "$app/proof/bin/proof-host"
install -m644 "$2/historical/egress.elf" "$app/proof/historical/egress.elf"
install -m644 "$3" "$app/anchor-witness.json"
python3 -m venv "$app/venv"
"$app/venv/bin/pip" install solders==0.28.0 base58==2.1.1 requests==2.31.0
