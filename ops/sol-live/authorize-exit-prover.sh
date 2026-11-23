#!/bin/bash
# Run as the orchestrator with its existing administration login on the prover.
set -euo pipefail
ops=$(cd "$(dirname "$0")" && pwd -P)
key="$HOME/.config/postfiat/sol-bridge/exit-prover-ssh"
trust="$HOME/.config/postfiat/sol-bridge/exit-prover-known_hosts"
expected=$(cat "$ops/exit-prover.fingerprint")
bash "$ops/check-prover-host.sh" "$trust"
bash "$ops/check-prover-identity.sh" "$key" "$expected"
[[ -f "$key.pub" && ! -L "$key.pub" && $(ssh-keygen -lf "$key.pub" | awk '{print $2}') == "$expected" ]]
admin=(ssh -o ForwardAgent=no -o StrictHostKeyChecking=yes -o GlobalKnownHostsFile=/dev/null
       -o "UserKnownHostsFile=$trust" -o UpdateHostKeys=no -p 20628 root@ssh8.vast.ai)
"${admin[@]}" '
    set -eu
    tmp=$(mktemp /usr/local/bin/.pf-sol-exit.XXXXXXXX)
    cat > "$tmp"
    chmod 0755 "$tmp"
    mv -T "$tmp" /usr/local/bin/pf-sol-exit-prove
' < "$ops/exit-prover-command.sh"
"${admin[@]}" '
    set -eu
    umask 077
    mkdir -p ~/.ssh
    touch ~/.ssh/authorized_keys
    exec 9>>~/.ssh/authorized_keys
    flock -x 9
    IFS= read -r pub
    blob=$(printf "%s\n" "$pub" | cut -d" " -f2)
    line="restrict,command=\"/usr/local/bin/pf-sol-exit-prove\" $pub"
    if grep -Fq " $blob" ~/.ssh/authorized_keys; then
        test "$(grep -F " $blob" ~/.ssh/authorized_keys)" = "$line"
    else
        printf "\n%s\n" "$line" >&9
    fi
' < "$key.pub"
echo "Authorized $expected for the fixed proof commands on ssh8.vast.ai:20628"
