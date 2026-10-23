# Read-only health snapshot. Runs on a host as: bash -s -- validator-N
v=$1; n=${v#validator-}
echo "## host $v $(date -u +%FT%TZ) uptime: $(uptime -p)"
echo "## loadavg $(cat /proc/loadavg) nproc $(nproc)"
echo "## memory"; free -b | sed -n 1,3p
echo "## pressure"; for r in cpu memory io; do echo "$r $(head -1 /proc/pressure/$r 2>/dev/null)"; done
echo "## disk"; df -B1 --output=size,used,avail,pcent / | tail -1
for u in postfiat-$v.service postfiat-$v-rpc.service; do
  echo "## unit $u"; systemctl show $u --property=ActiveState,SubState,NRestarts,ActiveEnterTimestamp,MainPID,MemoryCurrent,MemoryPeak,MemoryMax,MemoryHigh,MemorySwapMax,TasksCurrent,TasksMax,LimitNOFILE,CPUQuotaPerSecUSec,CPUUsageNSec | tr '\n' ' '; echo
  pid=$(systemctl show $u --property=MainPID --value); [ "$pid" -gt 0 ] 2>/dev/null && echo "fds $(ls /proc/$pid/fd | wc -l) threads $(ls /proc/$pid/task | wc -l) cpu% $(ps -o pcpu= -p $pid) rss_kb $(ps -o rss= -p $pid)"
done
vpid=$(systemctl show postfiat-$v.service --property=MainPID --value)
echo "## transport connections (validator pid $vpid)"; ss -tanp 2>/dev/null | grep "pid=$vpid," | awk '{print $1}' | sort | uniq -c | tr '\n' ' '; echo
echo "listening: $(ss -tlnp 2>/dev/null | grep "pid=$vpid," | awk '{print $4}' | tr '\n' ' ')"
echo "## all TCP states"; ss -tan | awk 'NR>1{print $1}' | sort | uniq -c | tr '\n' ' '; echo
echo "## journal last 2h (both units)"
for u in postfiat-$v.service postfiat-$v-rpc.service; do
  j=$(journalctl -u $u --since "2 hours ago" --no-pager 2>/dev/null)
  echo "$u lines=$(echo "$j" | grep -vc '^-- No entries' ) error=$(echo "$j" | grep -ciE 'error|panic') reset=$(echo "$j" | grep -ciE 'reset') timeout=$(echo "$j" | grep -ciE 'timed out|timeout') fail=$(echo "$j" | grep -ciE 'fail')"
  echo "$j" | grep -iE 'error|panic|reset|timed out|timeout|fail|oom|killed' | tail -5 | cut -c1-240
done
echo "## kernel last 2h"; journalctl -k --since "2 hours ago" --no-pager 2>/dev/null | grep -iE 'oom|killed|reset|drop|error|tcp|nf_conntrack' | tail -5 | cut -c1-240; echo "(end kernel)"
echo "## transport event log last 2h"; f=/var/log/postfiat/$v/transport-validator-events.ndjson
python3 - "$f" <<'PY'
import json, os, sys, time
f = sys.argv[1]; size = os.path.getsize(f); start = max(0, size - 40 * 1024 * 1024)
from collections import Counter
c = Counter(); errs = Counter()
with open(f, "rb") as h:
    h.seek(start); data = h.read()
lines = data.splitlines()[1:]
# events carry no timestamp; count the tail written in the last 2 h by size share is not exact, so report the last 400 events
for line in lines[-400:]:
    try: e = json.loads(line)
    except ValueError: continue
    c[(e.get("kind"), e.get("outcome"))] += 1
    if e.get("outcome") != "accepted":
        r = e.get("rejection") or {}
        errs[str(r.get("error") if isinstance(r, dict) else r)[:110]] += 1
print("last 400 events by kind/outcome:", dict(c)); print("non-accepted:", dict(errs))
print("log mtime", time.strftime("%H:%M:%S", time.gmtime(os.path.getmtime(f))), "size", size)
PY
echo "## relay/aux units"; systemctl list-units --all --no-legend --plain 2>/dev/null | grep -E "2870[0-9]|relay|cobalt-shadow|navcoin" | awk '{print $1, $3, $4}'
for u in $(systemctl list-units --all --no-legend --plain 2>/dev/null | grep -E "2870[0-9]" | awk '{print $1}'); do echo "$u NRestarts=$(systemctl show $u --property=NRestarts --value) since=$(systemctl show $u --property=ActiveEnterTimestamp --value)"; done
echo "## top cpu"; ps -eo pid,pcpu,pmem,etime,comm --sort=-pcpu | head -6
