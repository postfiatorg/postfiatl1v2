#!/bin/bash
# B23 release 1: deploy ONE validator host (manual canary-first rollout).
# Usage: ops/b23/deploy_host.sh N HOST
# Requires the signed stage in $D/stage. Never touches key material. Writes a
# per-host log to the evidence dir. Stops at the first failed check.
set -euo pipefail
N=$1; H=$2
R=${REL:-b23-release1-20261112}; OLD=${OLD:-r2-1-activation-bound-20261112}
D=${STAGE_ROOT:-/home/postfiat/var/b23-release1}; S=$D/stage/rootfs
E=${EVID:-/home/postfiat/repos/b23-coord/evidence/ops/release1}/deploy-v$N; mkdir -p "$E"
SHA=${SHA:-c822bac5fd087eed645660d80ef50969e706f788dca9405f42597a3c6085c8a0}
SSH="ssh -o BatchMode=yes -o ConnectTimeout=10 root@$H"
log() { echo "[$(date -u +%H:%M:%SZ)] $*" | tee -a "$E/deploy.log"; }

log "validator-$N @ $H: start"
# a. outbox watchdogs: record, then disable
$SSH "for u in postfiat-outbox-watchdog.service postfiat-outbox2-watchdog.service; do echo \"\$u enabled=\$(systemctl is-enabled \$u 2>&1) active=\$(systemctl is-active \$u 2>&1)\"; done" | tee "$E/watchdog-before.txt"
for u in postfiat-outbox-watchdog.service postfiat-outbox2-watchdog.service; do
  if grep -q "^$u enabled=enabled\|^$u .*active=active" "$E/watchdog-before.txt"; then
    $SSH "systemctl disable --now $u" 2>&1 | tee -a "$E/deploy.log"; log "disabled $u"
  fi
done
$SSH "for u in postfiat-outbox-watchdog.service postfiat-outbox2-watchdog.service; do echo \"\$u enabled=\$(systemctl is-enabled \$u 2>&1) active=\$(systemctl is-active \$u 2>&1)\"; done" | tee "$E/watchdog-after.txt"

# b. binary to an incoming name, then install and check sha
scp -q -o BatchMode=yes "$S/opt/postfiat/releases/$R/postfiat-node" "root@$H:/opt/postfiat/releases/.incoming-$R-postfiat-node"
$SSH "install -d -m0755 /opt/postfiat/releases/$R && install -m0755 /opt/postfiat/releases/.incoming-$R-postfiat-node /opt/postfiat/releases/$R/postfiat-node && rm -f /opt/postfiat/releases/.incoming-$R-postfiat-node && sha256sum /opt/postfiat/releases/$R/postfiat-node" | tee "$E/binary-sha.txt"
grep -q "^$SHA " "$E/binary-sha.txt" || { log "BINARY SHA MISMATCH"; exit 1; }

# c. release dir (common files + this validator's files only)
tar -C "$S/etc/postfiat/releases/$R" -cf - deployment-manifest.json deployment.public.json topology.json swap.metadata.json private-egress.metadata.json validator-$N.bindings.json validator-$N.rpc.env validator-$N.transport.env \
  | $SSH "install -d -m0755 /etc/postfiat/releases/$R && tar -C /etc/postfiat/releases/$R --no-same-owner -xf - && chown -R root:root /etc/postfiat/releases/$R && chmod 0644 /etc/postfiat/releases/$R/*"

# d. save the live units for rollback
$SSH "install -d -m0755 /etc/postfiat/releases/$OLD/rollback-units && cp -a /etc/systemd/system/postfiat-validator-$N.service /etc/systemd/system/postfiat-validator-$N-rpc.service /etc/postfiat/releases/$OLD/rollback-units/ && sha256sum /etc/postfiat/releases/$OLD/rollback-units/*" | tee "$E/rollback-units.sha"

# e. install the new units
scp -q -o BatchMode=yes "$S/etc/systemd/system/postfiat-validator-$N.service" "$S/etc/systemd/system/postfiat-validator-$N-rpc.service" "root@$H:/etc/systemd/system/"

# f. verify the signed manifest on the host (plain = ExecStartPre form; and with this validator's bindings)
$SSH "/opt/postfiat/releases/$R/postfiat-node deployment-manifest-verify --manifest-file /etc/postfiat/releases/$R/deployment-manifest.json --trusted-publisher-key-file /etc/postfiat/releases/$R/deployment.public.json > /dev/null && echo plain-verify-ok && /opt/postfiat/releases/$R/postfiat-node deployment-manifest-verify --manifest-file /etc/postfiat/releases/$R/deployment-manifest.json --trusted-publisher-key-file /etc/postfiat/releases/$R/deployment.public.json --validator-id validator-$N --validator-bindings-file /etc/postfiat/releases/$R/validator-$N.bindings.json > /dev/null && echo bound-verify-ok" | tee "$E/manifest-verify.txt"
grep -q bound-verify-ok "$E/manifest-verify.txt" || { log "MANIFEST VERIFY FAILED (units installed but NOT restarted; run rollback_host.sh)"; exit 1; }

# g. restart validator, then rpc
log "restarting postfiat-validator-$N"
$SSH "systemctl daemon-reload && systemctl restart postfiat-validator-$N.service"
sleep 8
log "restarting postfiat-validator-$N-rpc"
$SSH "systemctl restart postfiat-validator-$N-rpc.service"
sleep 8

# h. check the processes themselves
$SSH "systemctl is-active postfiat-validator-$N.service postfiat-validator-$N-rpc.service; for p in \$(pgrep -f 'postfiat-node (transport-validator-serve|rpc-serve)'); do echo \"pid \$p \$(readlink /proc/\$p/exe) \$(sha256sum /proc/\$p/exe | cut -c1-64) rss_kb=\$(awk '/VmRSS/{print \$2}' /proc/\$p/status) \$(tr '\\0' ' ' < /proc/\$p/cmdline | cut -c1-60)\"; done; free -m | awk '/Mem/{print \"mem_avail_mb=\"\$7}'; ls /var/lib/postfiat/validator-$N/readiness/" | tee "$E/processes.txt"
n_new=$(grep -c "$SHA" "$E/processes.txt" || true)
log "processes on new sha: $n_new (expect 2)"
[ "$n_new" = "2" ] || { log "PROCESS CHECK FAILED"; exit 1; }
# pending outbox jobs (U4)
$SSH "cd /var/lib/postfiat/validator-$N/certified-send-outbox 2>/dev/null && grep -L '\"completed\": *true' */job.json 2>/dev/null | wc -l" | sed 's/^/incomplete_outbox_jobs=/' | tee -a "$E/deploy.log"
log "validator-$N @ $H: deployed"
