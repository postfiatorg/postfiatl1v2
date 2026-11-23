#!/bin/bash
# Stage a fresh reviewed runtime. Never install a unit, move a signer or start a worker.
set -euo pipefail
umask 077
app=${1:?usage: install-exit-user.sh APP WITNESS}
witness=${2:?missing verified anchor witness}
test "$#" -eq 2
root=$(git rev-parse --show-toplevel)
cd "$root"
test -f "$witness"
mkdir -p "$(dirname "$app")"
mkdir -m700 "$app"
ops=ops/sol-live
files=("$ops"/{exit_service.py,exit_payout.py,exit_setup.py,exit_precheck.py,exit_keys.py,exit_proof.py,exit_segments.py,profile.py,live.py,exit_sandbox_check.py,exit-sandbox.sh,fixtures})
files+=(solana/deploy/{bridge,keys,route_binding}.py solana/pfusdc-egress-verifier/bootstrap.py crates/types/src/solana_exit_profile.json)
git archive HEAD "${files[@]}" | bash "$ops/install-exit-runtime.sh" "$app" \
    "$HOME/repos/b23-coord/evidence/withdraw" "$witness"
echo "1fa81b51967ccf72e91ce2edde135968abd519395047fd1f304b9d5aec25a600  $app/proof/bin/proof-host" | sha256sum -c -
echo "4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0  $app/proof/historical/egress.elf" | sha256sum -c -
/usr/bin/python3 "$ops/prepare-sandbox-runtime.py" "$app"
git rev-parse HEAD > "$app/revision"
