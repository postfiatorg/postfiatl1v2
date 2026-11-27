#!/bin/bash
# Run with sudo. Sends the Solana withdrawal service's proofs to the Colab proving session.
#   sudo ~/colab-sol-withdraw.sh          switch to Colab
#   sudo ~/colab-sol-withdraw.sh revert   go back to the Vast prover
set -euo pipefail
[ "$(id -u)" = 0 ] || { echo "run with sudo"; exit 1; }
T=/var/lib/pf-sol-exit/known_hosts; B=$T.vast-20261126
NEW=/home/postfiat/var/colab-prover/solexit/known_hosts.colab
MARK='# colab-prover: the Solana withdrawal prover is reached through a local port'
if [ "${1:-}" = revert ]; then
  [ -f "$B" ] && cp -p "$B" "$T"
  sed -i "/$MARK/d;/^127\.0\.0\.1 ssh8\.vast\.ai\$/d" /etc/hosts
  echo "reverted: Solana withdrawals prove on Vast again"; exit 0
fi
test "$(wc -l < "$NEW")" = 1 && grep -q '^\[ssh8\.vast\.ai\]:20628 ssh-ed25519 ' "$NEW"
[ -f "$B" ] || cp -p "$T" "$B"
cp "$NEW" "$T.new"; chown --reference="$B" "$T.new"; chmod --reference="$B" "$T.new"; mv "$T.new" "$T"
grep -q '^127\.0\.0\.1 ssh8\.vast\.ai$' /etc/hosts || printf '%s\n127.0.0.1 ssh8.vast.ai\n' "$MARK" >> /etc/hosts
sleep 2
curl -s -m 90 http://127.0.0.1:18089/solana/withdrawals/readiness | head -c 200; echo
echo "switched: Solana withdrawals prove on Colab while a window is open"
