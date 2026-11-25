set -eu
n=$1
echo "v$n before: $(df -h / | awk 'NR==2{print $5, $4" free"}')"
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
sed -i '/^[[:space:]]*delaycompress[[:space:]]*$/d' /etc/logrotate.d/postfiat-rpc-events /etc/logrotate.d/postfiat-validator-events
logrotate -d /etc/logrotate.d/postfiat-service-output /etc/logrotate.d/postfiat-rpc-events /etc/logrotate.d/postfiat-validator-events >/dev/null 2>/tmp/logrotate-dry.txt || { echo "dry run failed"; tail -5 /tmp/logrotate-dry.txt; exit 1; }
for f in /var/log/postfiat/validator-$n/*.ndjson.[0-9]; do [ -f "$f" ] && gzip -9 "$f"; done
logrotate /etc/logrotate.d/postfiat-service-output
mkdir -p /etc/systemd/journald.conf.d
printf '[Journal]\nSystemMaxUse=1G\n' > /etc/systemd/journald.conf.d/postfiat-size.conf
systemctl restart systemd-journald
journalctl --vacuum-size=1G >/dev/null 2>&1
echo "v$n after:  $(df -h / | awk 'NR==2{print $5, $4" free"}')"
ls -la --time-style=+%F /var/log/postfiat/validator-$n/ | awk 'NR>3{printf "  %6.0fM %s\n",$5/1048576,$7}'
systemctl is-active postfiat-validator-$n.service postfiat-validator-$n-rpc.service | tr '\n' ' '; echo
