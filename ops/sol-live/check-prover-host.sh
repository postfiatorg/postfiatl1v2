#!/bin/bash
set -euo pipefail
trust=$1
[[ -f "$trust" && ! -L "$trust" ]]
# Only a trust file dedicated to this endpoint may enter the sandbox. Permit
# hashed OpenSSH host fields only if ssh-keygen matches every one to the prover.
valid=$(awk 'NF && $1 !~ /^#/ { if ($1 != "[ssh8.vast.ai]:20628" && $1 !~ /^\|1\|/) exit 1; n++ } END {print n+0}' "$trust")
matched=$(ssh-keygen -l -F '[ssh8.vast.ai]:20628' -f "$trust" | awk 'NF && $1 !~ /^#/ {n++} END {print n+0}')
if (( valid == 0 || valid != matched )); then
    echo 'Prover trust file must contain only the verified prover host' >&2
    exit 1
fi
