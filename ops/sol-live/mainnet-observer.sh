#!/usr/bin/env bash
# Install on the Vast observer under /work/g. /root/onstart.sh calls this at container boot.
set -euo pipefail
cd /work/g
if [[ ${1:-} == --install ]]; then
    hook=/work/g/mainnet-observer.sh
    grep -Fxq "$hook" /root/onstart.sh || echo "$hook" >> /root/onstart.sh
fi
if [[ ${1:-} != --loop ]]; then
    tmux has-session -t '=mainnet-observer' 2>/dev/null ||
        tmux new-session -d -s mainnet-observer 'exec /work/g/mainnet-observer.sh --loop'
    exit
fi
exec 9>sol-live/supervisor.lock
flock -n 9 || exit 1
# The gossip port must be reachable at the number it advertises: on Vast that is an identity
# port (requested as 70000). An unreachable gossip port got the node dropped from the cluster.
export MAPPED_PORTS=1 PUBLIC_TVU=37.63.76.95:37504 GOSSIP_PORT=37521 DYNAMIC_PORT_RANGE=8001-8050
export AGAVE_VALIDATOR=/work/g/validator-main-custody
while :; do
    # The node writes no snapshots, so a restart after a stop would begin from a stale pair.
    python3 /work/g/fresh-snapshot.py /work/g/main-ledger >>sol-live/supervisor.log 2>&1 ||
        date -u +'%FT%TZ snapshot refresh failed' >>sol-live/supervisor.log
    if compgen -G 'main-ledger/snapshot-*.tar.zst' >/dev/null &&
       compgen -G 'main-ledger/incremental-snapshot-*.tar.zst' >/dev/null; then
        # The capture hook writes only blocks that change this account (the vault custody).
        mkdir -p main-ledger/public-capture
        cp /work/g/watch-account main-ledger/public-capture/watch-account
        date -u +'%FT%TZ observer starting' >>sol-live/supervisor.log
        ./run-node.sh mainnet-beta /work/g/main-ledger /work/g/main-identity.json 18899 >>sol-live/supervisor.log 2>&1 || true
        date -u +'%FT%TZ observer exited' >>sol-live/supervisor.log
    fi
    sleep 5
done
