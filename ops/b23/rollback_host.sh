#!/bin/bash
# B23 release 1: roll ONE validator host back to r2-1-activation-bound-20261112.
# Usage: ops/b23/rollback_host.sh N HOST
set -euo pipefail
N=$1; H=$2; OLD=${OLD:-r2-1-activation-bound-20261112}
E=${EVID:-/home/postfiat/repos/b23-coord/evidence/ops/release1}/rollback-v$N; mkdir -p "$E"
SSH="ssh -o BatchMode=yes -o ConnectTimeout=10 root@$H"
$SSH "test -f /etc/postfiat/releases/$OLD/rollback-units/postfiat-validator-$N.service && cp -a /etc/postfiat/releases/$OLD/rollback-units/postfiat-validator-$N.service /etc/postfiat/releases/$OLD/rollback-units/postfiat-validator-$N-rpc.service /etc/systemd/system/ && systemctl daemon-reload && systemctl restart postfiat-validator-$N.service && sleep 8 && systemctl restart postfiat-validator-$N-rpc.service && sleep 8 && systemctl is-active postfiat-validator-$N.service postfiat-validator-$N-rpc.service; for p in \$(pgrep -f 'postfiat-node (transport-validator-serve|rpc-serve)'); do echo \"pid \$p \$(readlink /proc/\$p/exe) \$(sha256sum /proc/\$p/exe | cut -c1-16)\"; done" | tee "$E/rollback.txt"
echo "rolled back validator-$N; expect exe sha 35513b7ce319162e. Watchdogs were left as recorded in deploy-v$N/watchdog-before.txt (restore with systemctl enable --now <unit> if required)."
