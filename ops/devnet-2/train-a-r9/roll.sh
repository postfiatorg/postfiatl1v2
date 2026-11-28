#!/bin/bash
set -euo pipefail

REL=train-a-r9
OLD=train-a-r8
STAGE_ROOT=/home/postfiat/var/$REL
S=$STAGE_ROOT/stage/rootfs
EVID=$STAGE_ROOT/evidence
SHA=$(cut -d" " -f1 "$EVID/binary.sha256")
HOSTS=(64.176.220.75 95.179.184.122 66.42.48.39 149.28.63.106 95.179.179.206 45.32.110.170)

check_six_way() {
  local out
  out=$(for p in 39650 39651 39652 39653 39654 39655; do
    echo '{"version":"postfiat-local-rpc-v1","id":"s","method":"status","params":{}}' | timeout 15 nc -q1 127.0.0.1 $p | python3 -c "import sys,json;r=json.load(sys.stdin)['result'];print(r['block_height'],r['state_root'],r['block_tip_hash'])" 2>/dev/null || true
  done)
  local n uniq
  n=$(echo "$out" | grep -c '^[0-9]' || true)
  uniq=$(echo "$out" | grep '^[0-9]' | sort -u | wc -l || true)
  if [ "$n" = "6" ] && [ "$uniq" = "1" ]; then
    echo "SIX_WAY_OK: $(echo "$out" | head -1 | cut -c1-48)"
    return 0
  fi
  echo "SIX_WAY_WAIT (n=$n uniq=$uniq)"
  return 1
}

deploy_one() {
  local N=$1
  local H=${HOSTS[$N]}
  local E=$EVID/deploy-v$N
  mkdir -p "$E"
  local SSH="ssh -o BatchMode=yes -o ConnectTimeout=15 root@$H"

  echo "=== [$(date -u +%H:%M:%SZ)] validator-$N @ $H ==="
  # 1. Record and disable outbox watchdogs
  $SSH 'for u in postfiat-outbox-watchdog.service postfiat-outbox2-watchdog.service; do echo "$u enabled=$(systemctl is-enabled $u 2>&1 || true) active=$(systemctl is-active $u 2>&1 || true)"; done' > "$E/watchdog-before.txt"
  for u in postfiat-outbox-watchdog.service postfiat-outbox2-watchdog.service; do
    if grep -q "^$u enabled=enabled\|^$u .*active=active" "$E/watchdog-before.txt"; then
      $SSH "systemctl disable --now $u" >/dev/null 2>&1 || true
    fi
  done

  # 2. Ship binary & verify SHA
  scp -q -o BatchMode=yes "$S/opt/postfiat/releases/$REL/postfiat-node" "root@$H:/opt/postfiat/releases/.incoming-$REL-postfiat-node"
  $SSH "install -d -m0755 /opt/postfiat/releases/$REL && install -m0755 /opt/postfiat/releases/.incoming-$REL-postfiat-node /opt/postfiat/releases/$REL/postfiat-node && rm -f /opt/postfiat/releases/.incoming-$REL-postfiat-node && sha256sum /opt/postfiat/releases/$REL/postfiat-node" > "$E/binary-sha.txt"
  grep -q "^$SHA " "$E/binary-sha.txt" || { echo "BINARY SHA MISMATCH on v$N"; exit 1; }

  # 3. Ship release metadata/env
  tar -C "$S/etc/postfiat/releases/$REL" -cf - deployment-manifest.json topology.json swap.metadata.json private-egress.metadata.json validator-$N.bindings.json validator-$N.rpc.env validator-$N.transport.env \
    | $SSH "install -d -m0755 /etc/postfiat/releases/$REL && ln -f /etc/postfiat/releases/$OLD/deployment.public.json /etc/postfiat/releases/$REL/deployment.public.json && tar -C /etc/postfiat/releases/$REL --no-same-owner -xf - && chown -R root:root /etc/postfiat/releases/$REL && chmod 0644 /etc/postfiat/releases/$REL/*"

  # 4. Backup old units & install new units (+ validator-0 timeout drop-in)
  $SSH "install -d -m0755 /etc/postfiat/releases/$OLD/rollback-units && cp -a /etc/systemd/system/postfiat-validator-$N.service /etc/systemd/system/postfiat-validator-$N-rpc.service /etc/postfiat/releases/$OLD/rollback-units/"
  scp -q -o BatchMode=yes "$S/etc/systemd/system/postfiat-validator-$N.service" "$S/etc/systemd/system/postfiat-validator-$N-rpc.service" "root@$H:/etc/systemd/system/"
  if [ "$N" = "0" ]; then
    $SSH "cp -a /etc/systemd/system/postfiat-validator-0-rpc.service.d/20-request-timeout.conf /etc/postfiat/releases/$OLD/rollback-units/ && sed -i 's/$OLD/$REL/g' /etc/systemd/system/postfiat-validator-0-rpc.service.d/20-request-timeout.conf && (grep -q -- '--allow-private-action-admission' /etc/systemd/system/postfiat-validator-0-rpc.service.d/20-request-timeout.conf || sed -i 's/--allow-mempool-submit-finality/--allow-mempool-submit-finality --allow-private-action-admission/g' /etc/systemd/system/postfiat-validator-0-rpc.service.d/20-request-timeout.conf)"
  fi

  # 5. Verify signed manifest against installed units BEFORE restarting
  $SSH "/opt/postfiat/releases/$REL/postfiat-node deployment-manifest-verify --manifest-file /etc/postfiat/releases/$REL/deployment-manifest.json --trusted-publisher-key-file /etc/postfiat/releases/$REL/deployment.public.json > /dev/null && /opt/postfiat/releases/$REL/postfiat-node deployment-manifest-verify --manifest-file /etc/postfiat/releases/$REL/deployment-manifest.json --trusted-publisher-key-file /etc/postfiat/releases/$REL/deployment.public.json --validator-id validator-$N --validator-bindings-file /etc/postfiat/releases/$REL/validator-$N.bindings.json > /dev/null && echo MANIFEST_OK" > "$E/manifest-verify.txt"
  grep -q MANIFEST_OK "$E/manifest-verify.txt" || { echo "MANIFEST VERIFY FAILED on v$N"; exit 1; }

  # 6. Restart validator then RPC
  $SSH "systemctl daemon-reload && systemctl restart postfiat-validator-$N.service"
  sleep 8
  $SSH "systemctl restart postfiat-validator-$N-rpc.service"
  sleep 8

  # 7. Verify both processes run the new SHA and RPC has --allow-private-action-admission
  $SSH "systemctl is-active postfiat-validator-$N.service postfiat-validator-$N-rpc.service && for p in \$(pgrep -f 'postfiat-node (transport-validator-serve|rpc-serve)'); do echo \"pid=\$p sha=\$(sha256sum /proc/\$p/exe | cut -c1-64) cmd=\$(tr '\\0' ' ' < /proc/\$p/cmdline)\"; done" > "$E/processes.txt"
  local n_new has_adm
  n_new=$(grep -c "sha=$SHA" "$E/processes.txt" || true)
  has_adm=$(grep -c -- "--allow-private-action-admission" "$E/processes.txt" || true)
  if [ "$n_new" != "2" ] || [ "$has_adm" -lt "1" ]; then
    echo "PROCESS CHECK FAILED on v$N (n_new=$n_new has_adm=$has_adm)"
    cat "$E/processes.txt"
    exit 1
  fi

  # 8. Restore watchdogs
  for u in postfiat-outbox-watchdog.service postfiat-outbox2-watchdog.service; do
    if grep -q "^$u enabled=enabled" "$E/watchdog-before.txt"; then
      $SSH "systemctl enable $u" >/dev/null 2>&1 || true
    fi
    if grep -q "^$u .*active=active$" "$E/watchdog-before.txt"; then
      $SSH "systemctl start $u" >/dev/null 2>&1 || true
    fi
  done

  # 9. Require 6-way consensus agreement
  local ok=0
  for t in 1 2 3 4 5 6 7 8; do
    if check_six_way; then ok=1; break; fi
    sleep 5
  done
  if [ "$ok" != "1" ]; then
    echo "NO SIX-WAY AGREEMENT AFTER v$N"
    exit 1
  fi
  echo "validator-$N DEPLOYED AND VERIFIED"
}

for n in "$@"; do
  deploy_one "$n"
done
echo "ALL_REQUESTED_VALIDATORS_ROLLED"
