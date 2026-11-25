#!/bin/bash
# Install the settlement loop as a service on the RELAY machine, as root there:
#   install-exit-settler.sh ARCHIVE KNOWN_HOSTS
# ARCHIVE is `git archive -o exit-settler.tar <reviewed commit> ops/sol-live solana/deploy`.
# KNOWN_HOSTS holds the six validator hosts' SSH host keys you trust (for example the relay
# login's own ~/.ssh/known_hosts).
# The loop runs as its own user with its own SSH key, generated here, mode 0600, never printed and
# never on a command line. It reaches each host only through that host's restricted door
# (install-exit-review.sh --settler-key), so it holds no validator key and logs in as root nowhere.
# The unit is enabled only when all six doors answer the read-only request; until then the tmux
# loop stays in use (RUNBOOK-LIVE.md, "Settlement loop as a service").
set -euo pipefail
umask 077
root=${INSTALL_ROOT:-}
fail() { echo "install-exit-settler: $*" >&2; exit 1; }
[[ $# == 2 && -f $1 && -f $2 ]] || fail "usage: install-exit-settler.sh ARCHIVE KNOWN_HOSTS"
archive=$1 known=$2
user=pft-sol-exit-settler
app=$root/opt/postfiat/sol-exit-settler
home=$root/var/lib/pft-sol-exit-settler
env=$root/etc/postfiat/sol-exit-settler.env
units=$root/etc/systemd/system
key=$home/.ssh/pft-sol-exit-settler
door="/opt/postfiat/sol-exit-review/venv/bin/python -I /opt/postfiat/sol-exit-review/exit_review_door.py"

python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)' || fail "python3 3.10 or newer is required"
command -v ssh-keygen >/dev/null || fail "ssh-keygen is missing"
if ! id -u "$user" >/dev/null 2>&1; then
    useradd --system --user-group --home-dir /var/lib/pft-sol-exit-settler --shell /usr/sbin/nologin "$user" \
        || fail "cannot create the user $user"
fi

install -d -m755 -o root -g root "$app" || fail "cannot create $app"
tar -xf "$archive" -C "$app" --no-same-owner --no-same-permissions ops/sol-live solana/deploy \
    || fail "cannot read the settlement loop from $archive"
chmod -R go+rX "$app" || fail "cannot make $app readable"
python3 -m venv "$app/venv" || fail "cannot create the venv"
"$app/venv/bin/python" -m pip install --quiet --require-hashes --no-deps --only-binary=:all: \
    -r "$app/ops/sol-live/exit-requirements.lock" || fail "cannot install the hash-pinned requirements"
chmod -R go+rX "$app/venv" || fail "cannot make the venv readable"

install -d -m750 -o "$user" -g "$user" "$home" || fail "cannot create $home"
install -d -m700 -o "$user" -g "$user" "$home/.ssh" || fail "cannot create $home/.ssh"
install -m644 -o "$user" -g "$user" "$known" "$home/.ssh/known_hosts" || fail "cannot install the host keys"
if [[ ! -f $key ]]; then
    ssh-keygen -q -t ed25519 -N "" -C "$user" -f "$key" || fail "cannot create the loop's SSH key"
fi
chown "$user:$user" "$key" "$key.pub" || fail "cannot give the key to $user"
chmod 0600 "$key" || fail "cannot make the key private"
if [[ ! -f $env ]]; then
    # Settings, not on any command line: SOL_MAINNET_RPC=<the relay's Solana RPC URL>.
    printf '# SOL_MAINNET_RPC=https://...\n' >"$env" || fail "cannot create $env"
fi
chown "root:$user" "$env" && chmod 0640 "$env" || fail "cannot protect $env"
install -m644 "$app/ops/sol-live/pft-sol-exit-settler.service" "$app/ops/sol-live/pft-sol-exit-settler.timer" \
    "$units/" || fail "cannot install the units"
systemctl daemon-reload || fail "systemctl daemon-reload failed"

pub=$(<"$key.pub")
echo "The loop's public key: $pub"
echo "On each validator host, add it with install-exit-review.sh ... --settler-key FILE (FILE holding the key above)."
echo "That writes this line into /var/lib/pf-sol-exit-review-ssh/.ssh/authorized_keys:"
echo "  restrict,command=\"$door\" $pub"

# The read-only request on all six doors: a result for no decision. Each host is named.
missing=$(cd "$app/ops/sol-live" && runuser -u "$user" -- env PFT_SETTLER_SSH_KEY="${key#"$root"}" \
    "$app/venv/bin/python" -c '
import govern
lost = []
for i in range(6):
    try:
        govern.door(i, dict(request="result", decision_sha256="0" * 64))
    except Exception as error:
        lost.append(f"validator-{i} ({type(error).__name__}: {error})")
print("; ".join(lost))') || fail "cannot run the read-only door check"
if [[ -n $missing ]]; then
    echo "Not enabled: these hosts did not answer the read-only request: $missing" >&2
    echo "The tmux loop stays in use until all six answer." >&2
    exit 3
fi
# The doors carry the signatures. Drafting, assembling and the proposer's submission still need the
# root login (RUNBOOK-LIVE.md, "What stays open"); the unit is enabled once they have a door too.
echo "All six doors answer. Not enabled yet: drafting, assembling and submitting still need a root login," >&2
echo "which this loop must not have. The tmux loop stays in use." >&2
exit 4
