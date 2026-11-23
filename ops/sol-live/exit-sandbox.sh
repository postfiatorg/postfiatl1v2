#!/bin/bash
set -euo pipefail
umask 077
app=$(cd "$(dirname "$0")/../.." && pwd -P)
state=${EXIT_JOB_STATE:-"$HOME/.local/state/pf-sol-exit/jobs"}
fee="$HOME/.config/postfiat/sol-bridge/exit-fee-payer.json"
identity=${EXIT_PROVER_IDENTITY:?set the path of the identity authorized on the prover}
trust=${EXIT_PROVER_KNOWN_HOSTS:?set the path of the prover-only known_hosts file}
python=/app/venv/bin/python
work=/app/ops/sol-live
case "${1:-}" in
    self-test|proof) command=("$python" -B "$work/exit_sandbox_check.py" "$1");;
    serve) command=("$python" -Bu "$work/exit_service.py" --state /state --port 18089);;
    rebuild) command=("$python" -Bu "$work/exit_service.py" --state /state --rebuild);;
    *) echo 'Usage: exit-sandbox.sh self-test|proof|serve|rebuild' >&2; exit 2;;
esac
# This directory must contain only the worker journal and its proof artifacts.
[[ -d "$state" && ! -L "$state" && "$(stat -c '%u:%a' "$state")" == "$(id -u):700" ]]
[[ -f "$fee" && ! -L "$fee" && "$(stat -c '%u:%a' "$fee")" == "$(id -u):600" ]]
if [[ ! -f "$identity" || -L "$identity" ]]; then
    echo 'Expected a regular prover identity file' >&2
    exit 1
fi
if [[ ! -f "$trust" || -L "$trust" ]]; then
    echo 'Expected a regular prover-only known_hosts file' >&2
    exit 1
fi
# Only a trust file dedicated to this endpoint may enter the sandbox. Permit
# hashed OpenSSH host fields only if ssh-keygen matches every one to the prover.
valid=$(awk 'NF && $1 !~ /^#/ { if ($1 != "[ssh8.vast.ai]:20628" && $1 !~ /^\|1\|/) exit 1; n++ } END {print n+0}' "$trust")
matched=$(ssh-keygen -l -F '[ssh8.vast.ai]:20628' -f "$trust" | awk 'NF && $1 !~ /^#/ {n++} END {print n+0}')
if (( valid == 0 || valid != matched )); then
    echo 'Prover trust file must contain only the verified prover host' >&2
    exit 1
fi
mounts=()
while IFS= read -r file; do
    mounts+=(--ro-bind "$file" "$file")
done < "$app/runtime-files"
exec /usr/bin/bwrap --unshare-all --share-net --unshare-user --disable-userns --cap-drop ALL \
    --new-session --die-with-parent --clearenv \
    --setenv HOME /home/worker --setenv PATH /usr/bin --setenv LANG C.UTF-8 \
    --setenv EXIT_OPERATOR_HOME "$HOME" --setenv EXIT_FEE_KEY /credentials/fee.json \
    --setenv EXIT_PROVER_SSH sandbox --setenv EXIT_PROOF_HOME /app/proof \
    --setenv EXIT_ANCHOR_WITNESS /app/anchor-witness.json \
    --setenv REQUESTS_CA_BUNDLE /etc/ssl/certs/ca-certificates.crt \
    --symlink usr/lib /lib --symlink usr/lib64 /lib64 "${mounts[@]}" \
    --symlink python3.12 /usr/bin/python3 \
    --ro-bind /etc/resolv.conf /etc/resolv.conf \
    --ro-bind /etc/ssl/certs/ca-certificates.crt /etc/ssl/certs/ca-certificates.crt \
    --ro-bind "$app/passwd" /etc/passwd --ro-bind "$app/nsswitch.conf" /etc/nsswitch.conf \
    --ro-bind "$app" /app --bind "$state" /state \
    --ro-bind "$fee" /credentials/fee.json --ro-bind "$identity" /credentials/prover \
    --ro-bind "$trust" /credentials/known_hosts \
    --dir /home/worker --tmpfs /tmp --proc /proc --dev /dev --remount-ro / \
    --chdir /app -- "${command[@]}"
