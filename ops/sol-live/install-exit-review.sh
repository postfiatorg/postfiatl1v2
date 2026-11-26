#!/bin/bash
# Install the per-host Solana exit settlement reviewer on ONE validator host, as root on that host:
#   install-exit-review.sh VALIDATOR NODE_RPC_PORT SOLANA_RPC_URL ARCHIVE
#   e.g. install-exit-review.sh validator-3 27653 https://<this host's own Solana endpoint> /root/exit-review.tar
# Optionally add  --settler-key FILE  (the settlement loop's public key, printed by
# install-exit-settler.sh) to open the loop's restricted door: one `restrict,command=` line in the
# reviewer user's authorized_keys whose forced command (exit_review_door.py) accepts only "hand in
# a decision for this host's review" and "return this host's result"; no shell, file or forwarding.
# ARCHIVE is `git archive -o exit-review.tar <reviewed commit> ops/sol-live`, copied to the host
# (RUNBOOK-LIVE.md, "Independent settlement reviews").
# The reviewer runs as its own user, which alone can read its verdict key. The node user, which
# signs, reads only the public key, the config and the verdicts, so it can check a verdict but
# not make one. Every step's result is checked; any failure stops the install with its reason.
set -euo pipefail
umask 077
root=${INSTALL_ROOT:-}
fail() { echo "install-exit-review: $*" >&2; exit 1; }
if [[ ! ( $# == 4 || ( $# == 6 && $5 == --settler-key && -f $6 ) ) || ! $1 =~ ^validator-[0-5]$ || ! $2 =~ ^[0-9]+$ \
      || ! $3 =~ ^https://[^\"\\[:space:]]+$ || ! -f $4 ]]; then
    fail "usage: install-exit-review.sh validator-<0..5> NODE_RPC_PORT https://SOLANA_RPC_URL ARCHIVE [--settler-key FILE]"
fi
validator=$1 port=$2 url=$3 archive=$4 settler_key=${6:-}
settler_line=
if [[ -n $settler_key ]]; then
    # Exactly one ed25519 public key, nothing else: options in the file would loosen the door.
    settler_line=$(<"$settler_key")
    [[ $(wc -l <"$settler_key") -le 1 && $settler_line =~ ^ssh-ed25519\ [A-Za-z0-9+/]+=*(\ [A-Za-z0-9@._-]+)?$ ]] \
        || fail "$settler_key is not one ssh-ed25519 public key line"
    # The line's form is not enough: the key itself must decode, or no login with it can work.
    ssh-keygen -l -f "$settler_key" 2>/dev/null | grep -q '(ED25519)$' \
        || fail "$settler_key is not a valid ssh-ed25519 public key"
fi
user=pf-sol-exit-review
app=$root/opt/postfiat/sol-exit-review
etc=$root/etc/postfiat
reviews=$root/var/lib/postfiat-exit-review
logs=$root/var/log/pf-sol-exit-review
data=$root/var/lib/postfiat/$validator
units=$root/etc/systemd/system

python3 -c 'import sys; sys.exit(0 if sys.version_info >= (3, 10) else 1)' || fail "python3 3.10 or newer is required"
command -v setfacl >/dev/null || fail "setfacl is missing: install the acl package"
[[ -d $data/consensus-v2-safety ]] || fail "$data/consensus-v2-safety is missing: is $validator on this host?"
[[ -d $etc ]] || fail "$etc is missing"

if ! id -u "$user" >/dev/null 2>&1; then
    useradd --system --user-group --no-create-home --home-dir /nonexistent --shell /usr/sbin/nologin "$user" \
        || fail "cannot create the user $user"
fi
usermod -a -G "$user" postfiat || fail "cannot add postfiat to the group $user"

tmp=$(mktemp -d) || fail "cannot create a temporary directory"
trap 'rm -rf -- "$tmp"' EXIT
door_files=()
[[ -n $settler_key ]] && door_files=(ops/sol-live/exit_review_door.py ops/sol-live/pft-sol-exit-sign-door.path
                                    ops/sol-live/pft-sol-exit-sign-door.service)
tar -xf "$archive" -C "$tmp" --no-same-owner --no-same-permissions ops/sol-live/exit_review.py \
    ops/sol-live/pft-sol-exit-review.service ops/sol-live/pft-sol-exit-review.timer \
    ops/sol-live/exit-requirements.lock "${door_files[@]}" || fail "cannot read the reviewer from $archive"

install -d -m755 -o root -g root "$app" || fail "cannot create $app"
install -m644 -o root -g root "$tmp/ops/sol-live/exit_review.py" "$app/exit_review.py" || fail "cannot install the reviewer"
python3 -m venv "$app/venv" || fail "cannot create the reviewer's venv"
"$app/venv/bin/python" -m pip install --quiet --require-hashes --no-deps --only-binary=:all: \
    -r "$tmp/ops/sol-live/exit-requirements.lock" || fail "cannot install the hash-pinned requirements"
# The venv was created under umask 077; the reviewer's user must read and run it. Root keeps ownership.
chmod -R go+rX "$app/venv" || fail "cannot make the venv readable by the reviewer"

# The verdict key: created under umask 077, owned by the reviewer alone; its public half is public.
if [[ ! -f $etc/sol-exit-review.key ]]; then
    "$app/venv/bin/python" -I -c 'import sys
from solders.keypair import Keypair
key = Keypair()
open(sys.argv[1], "xb").write(bytes(key))
open(sys.argv[2], "w").write(str(key.pubkey()) + "\n")' "$etc/sol-exit-review.key" "$etc/sol-exit-review.pub" \
        || fail "cannot create the reviewer key"
fi
if [[ ! -f $etc/sol-exit-review.pub ]]; then
    "$app/venv/bin/python" -I -c 'import sys
from solders.keypair import Keypair
open(sys.argv[2], "w").write(str(Keypair.from_bytes(open(sys.argv[1], "rb").read()).pubkey()) + "\n")' \
        "$etc/sol-exit-review.key" "$etc/sol-exit-review.pub" || fail "cannot derive the reviewer's public key"
fi
chown "$user:$user" "$etc/sol-exit-review.key" || fail "cannot give the reviewer key to $user"
chmod 0600 "$etc/sol-exit-review.key" || fail "cannot make the reviewer key private"
chmod 0644 "$etc/sol-exit-review.pub" || fail "cannot publish the reviewer's public key"

"$app/venv/bin/python" -I -c 'import json, sys
json.dump(dict(validator=sys.argv[2], node=["127.0.0.1", int(sys.argv[3])], solana_rpc=sys.argv[4],
               review_dir="/var/lib/postfiat-exit-review", data_dir="/var/lib/postfiat/" + sys.argv[2]),
          open(sys.argv[1], "w"))' "$tmp/config.json" "$validator" "$port" "$url" || fail "cannot write the config"
install -m640 -o root -g "$user" "$tmp/config.json" "$etc/sol-exit-review.json" || fail "cannot install the config"
install -d -m750 -o "$user" -g "$user" "$reviews" "$logs" || fail "cannot create the verdict and log directories"

# The clearance review lists the node's signing history: the names only, nothing else of the node.
setfacl -m "u:$user:x" "$data" || fail "cannot let $user enter $data"
setfacl -m "u:$user:rx" "$data/consensus-v2-safety" || fail "cannot let $user list the signing history"

install -m644 "$tmp/ops/sol-live/pft-sol-exit-review.service" "$tmp/ops/sol-live/pft-sol-exit-review.timer" "$units/" \
    || fail "cannot install the units"
systemctl daemon-reload || fail "systemctl daemon-reload failed"
# The first run must approve or name a refusal; an RPC or access error fails the install.
systemctl start pft-sol-exit-review.service \
    || fail "the first review failed; see /var/log/pf-sol-exit-review/review.log"
systemctl enable --now pft-sol-exit-review.timer || fail "cannot enable the timer"

if [[ -n $settler_key ]]; then
    door="/opt/postfiat/sol-exit-review/venv/bin/python -I /opt/postfiat/sol-exit-review/exit_review_door.py"
    home=$root/var/lib/pf-sol-exit-review-ssh
    install -m644 -o root -g root "$tmp/ops/sol-live/exit_review_door.py" "$app/exit_review_door.py" \
        || fail "cannot install the door"
    # The door writes the inbox as the reviewer; the node user's signing step writes the outbox.
    install -d -m750 -o "$user" -g "$user" "$reviews/inbox" || fail "cannot create the door's inbox"
    # Set-group-id: a result the node user writes belongs to the reviewer's group, so the door,
    # which runs as the reviewer, can return it.
    install -d -m2750 -o postfiat -g "$user" "$reviews/outbox" || fail "cannot create the door's outbox"
    # Root owns the key file, so neither the reviewer nor the node user can widen it. sshd runs the
    # forced command through the user's shell, so the shell is /bin/sh; the command= line is all it runs.
    install -d -m755 -o root -g root "$home" "$home/.ssh" || fail "cannot create $home"
    printf 'restrict,command="%s" %s\n' "$door" "$settler_line" >"$tmp/authorized_keys" \
        || fail "cannot write the door's key line"
    install -m644 -o root -g root "$tmp/authorized_keys" "$home/.ssh/authorized_keys" \
        || fail "cannot install the door's key line"
    usermod -d /var/lib/pf-sol-exit-review-ssh -s /bin/sh "$user" || fail "cannot give $user its door"
    install -m644 "$tmp/ops/sol-live/pft-sol-exit-sign-door.path" "$tmp/ops/sol-live/pft-sol-exit-sign-door.service" \
        "$units/" || fail "cannot install the door's signing units"
    systemctl daemon-reload || fail "systemctl daemon-reload failed"
    systemctl enable --now pft-sol-exit-sign-door.path || fail "cannot enable the door's signing step"
    echo "Settlement loop's restricted door open for $validator (forced command: $door)"
fi
echo "Reviewer installed for $validator; verdicts in /var/lib/postfiat-exit-review, log /var/log/pf-sol-exit-review/review.log"
