set -u
df -h / | tail -1
df -B1 / | tail -1
echo '--releases'
du -sh /opt/postfiat/releases 2>&1
du -sh /opt/postfiat/releases/* 2>/dev/null | sort -rh | head -8
echo '--snapshots'
du -sh /var/lib/postfiat/pre-rollout-snapshots 2>&1
du -sh /var/lib/postfiat/pre-rollout-snapshots/* /var/lib/postfiat/pre-rollout-snapshots/.[!.]* 2>/dev/null | sort -rh | head -8
echo '--varlib'
du -sh /var/lib/postfiat/* 2>/dev/null | sort -rh | head -6
