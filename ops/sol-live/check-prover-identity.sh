#!/bin/bash
# Derive the public fingerprint from the private file, ignoring any .pub sidecar.
set -euo pipefail
identity=$1
expected=$2
[[ -f "$identity" && ! -L "$identity" && $(stat -c '%u:%a' "$identity") == "$(id -u):600" ]]
fingerprint=$(ssh-keygen -y -P '' -f "$identity" | ssh-keygen -E sha256 -lf - | awk '{print $2}')
if [[ "$fingerprint" != "$expected" ]]; then
    echo 'Unapproved prover identity fingerprint' >&2
    exit 1
fi
