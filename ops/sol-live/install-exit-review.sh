#!/bin/bash
# Install the per-host Solana exit settlement reviewer on ONE validator host, as root on that host.
# Usage: install-exit-review.sh VALIDATOR NODE_RPC_PORT SOLANA_RPC_URL < reviewed-checkout.tar
#   e.g. install-exit-review.sh validator-3 27653 https://<this host's own Solana endpoint>
# The review key is generated here and never leaves this host.
set -euo pipefail
umask 0022
[[ $# == 3 && $1 =~ ^validator-[0-5]$ && $2 =~ ^[0-9]+$ && $3 =~ ^https:// ]]
app=/opt/postfiat/sol-exit-review
install -d -m755 "$app" /var/lib/postfiat-exit-review
tmp=$(mktemp -d); trap 'rm -rf "$tmp"' EXIT
tar -xf - -C "$tmp" --no-same-owner --no-same-permissions ops/sol-live/exit_review.py ops/sol-live/pft-sol-exit-review.service ops/sol-live/pft-sol-exit-review.timer ops/sol-live/exit-requirements.lock
install -m644 "$tmp/ops/sol-live/exit_review.py" "$app/exit_review.py"
python3 -m venv "$app/venv"
"$app/venv/bin/pip" install --require-hashes -r "$tmp/ops/sol-live/exit-requirements.lock"
if [[ ! -f /etc/postfiat/sol-exit-review.key ]]; then
  head -c 32 /dev/urandom > /etc/postfiat/sol-exit-review.key
fi
chown root:postfiat /etc/postfiat/sol-exit-review.key && chmod 0640 /etc/postfiat/sol-exit-review.key
printf '{"validator": "%s", "node": ["127.0.0.1", %s], "solana_rpc": "%s", "review_dir": "/var/lib/postfiat-exit-review"}\n' \
  "$1" "$2" "$3" > /etc/postfiat/sol-exit-review.json
chmod 0644 /etc/postfiat/sol-exit-review.json
install -m644 "$tmp/ops/sol-live/pft-sol-exit-review.service" "$tmp/ops/sol-live/pft-sol-exit-review.timer" /etc/systemd/system/
systemctl daemon-reload
systemctl start pft-sol-exit-review.service || true
systemctl enable --now pft-sol-exit-review.timer
echo "Reviewer installed for $1; verdicts in /var/lib/postfiat-exit-review, log /var/log/pf-sol-exit-review.log"
