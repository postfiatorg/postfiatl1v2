#!/usr/bin/env bash
# Log retention for validator-N service output (run as root on the validator host):
#   ssh <host> 'sudo bash -s N' < ops/validator-host/logrotate/install.sh
# Exactly one logrotate rule may own each file: if the full configuration would list a
# postfiat log twice (for example systemd/postfiat-logrotate.example installed as
# /etc/logrotate.d/postfiat), nothing is changed and the script exits 1.
set -euo pipefail
umask 022 # logrotate ignores group- or world-writable rule files
n=${1:?usage: install.sh VALIDATOR_INDEX}
rules=(/etc/logrotate.d/postfiat-service-output /etc/logrotate.d/postfiat-rpc-events /etc/logrotate.d/postfiat-validator-events)
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

# Dry-run the whole configuration, as logrotate.timer runs it. Any error naming a postfiat
# rule or log fails: a duplicate entry makes logrotate skip the later rule file and exit 1.
full_config_dry_run() {
  local rc=0
  logrotate --debug --state "$work/state" /etc/logrotate.conf >/dev/null 2>"$work/dry.txt" || rc=$?
  if grep -E '^error: .*postfiat' "$work/dry.txt" >"$work/errors.txt"; then
    cat "$work/errors.txt" >&2
    return 1
  fi
  [ "$rc" -eq 0 ] || echo "note: logrotate rc=$rc from rules outside postfiat: $(grep -m1 '^error:' "$work/dry.txt")" >&2
}

echo "v$n before: $(df -h / | awk 'NR==2{print $5, $4" free"}')"
if ! full_config_dry_run; then
  echo "refusing: the installed logrotate configuration already has postfiat errors (above); fix those first" >&2
  exit 1
fi
for r in "${rules[@]}"; do
  if [ -e "$r" ]; then cp -p "$r" "$work/${r##*/}"; fi
done
cat > /etc/logrotate.d/postfiat-service-output <<'C'
# stdout and stderr of the validator and RPC services (systemd append: files).
/var/log/postfiat/validator-*/stdout.log
/var/log/postfiat/validator-*/stderr.log
/var/log/postfiat/validator-*/rpc-stdout.log
/var/log/postfiat/validator-*/rpc-stderr.log {
    size 100M
    rotate 3
    compress
    missingok
    notifempty
    copytruncate
}
C
# copytruncate needs no delay before compressing; keep rotated event logs compressed.
for r in /etc/logrotate.d/postfiat-rpc-events /etc/logrotate.d/postfiat-validator-events; do
  if [ -e "$r" ]; then sed -i '/^[[:space:]]*delaycompress[[:space:]]*$/d' "$r"; fi
done
if ! full_config_dry_run; then
  for r in "${rules[@]}"; do
    if [ -e "$work/${r##*/}" ]; then cp -p "$work/${r##*/}" "$r"; else rm -f "$r"; fi
  done
  echo "refusing: another rule already rotates these files (above); restored ${rules[*]}" >&2
  echo "keep either systemd/postfiat-logrotate.example or the validator-host rules, not both" >&2
  exit 1
fi
for f in "/var/log/postfiat/validator-$n"/*.ndjson.[0-9]; do
  if [ -f "$f" ]; then gzip -9 "$f"; fi
done
logrotate /etc/logrotate.d/postfiat-service-output
mkdir -p /etc/systemd/journald.conf.d
printf '[Journal]\nSystemMaxUse=1G\n' > /etc/systemd/journald.conf.d/postfiat-size.conf
systemctl restart systemd-journald
journalctl --vacuum-size=1G >/dev/null 2>&1
echo "v$n after:  $(df -h / | awk 'NR==2{print $5, $4" free"}')"
find "/var/log/postfiat/validator-$n" -maxdepth 1 -type f -printf '%s %f\n' | sort -k2 | awk '{printf "  %6.0fM %s\n",$1/1048576,$2}'
{ systemctl is-active "postfiat-validator-$n.service" "postfiat-validator-$n-rpc.service" || true; } | tr '\n' ' '; echo
