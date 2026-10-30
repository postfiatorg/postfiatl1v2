#!/usr/bin/env python3
"""Read-only demo-day preflight for postfiat-wan-devnet-2 (vote-records-20261030).

Reuses observe-fleet.py's identity checks (status fields, running executable
hashes, deployment-manifest-verify, apt-daily-upgrade) and adds six-way ledger
agreement, unit uptime and restarts, the loopback Ethereum relays, checkpoint
signing key presence (validators 0-4), RPC readiness flags, disk free and the
logrotate rules. One SSH call per host, all hosts in parallel. Writes nothing
anywhere and never reads key file contents. Exit 0 READY, 1 ATTENTION, 2 FAIL.
"""
import argparse
from concurrent.futures import ThreadPoolExecutor
from datetime import datetime, timezone
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import time
import urllib.error
import urllib.request

PACKET = Path(__file__).resolve().parent
_spec = importlib.util.spec_from_file_location("observe_fleet", PACKET / "observe-fleet.py")
observe = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(observe)

SIGNERS = {f"validator-{n}" for n in range(5)}  # Ethereum checkpoint committee; validator-5 is outside it
DISK_ATTENTION_GB = 10
ARCHIVE_PROBE = ("0xA0b86991c6218b36c1d19D4a2e9Eb0cE3606eB48", 26094220)  # USDC code at a fixed block
READY_FLAGS = ("ready", "degraded", "event_log", "event_log_writable", "listener_bound",
               "local_state_loaded", "finality_key_available", "telemetry_failure_count")

# Runs on each host as `python3 - <validator> <release> <signer> <contract> <block>`. Read-only.
REMOTE = r'''
import hashlib, json, os, re, subprocess, sys, urllib.error, urllib.parse, urllib.request
v, release, signer, contract, block = sys.argv[1:6]
def sh(*cmd):
    return subprocess.run(cmd, capture_output=True, text=True, timeout=40)
out = {}
boot = float(open("/proc/uptime").read().split()[0])
units = {}
for u in (f"postfiat-{v}.service", f"postfiat-{v}-rpc.service", "postfiat-cobalt-shadow.service",
          "navcoin-ethereum-rpc-28703.service"):
    p = dict(l.split("=", 1) for l in sh("systemctl", "show", u, "--property=ActiveState,NRestarts,"
             "MainPID,ActiveEnterTimestamp,ActiveEnterTimestampMonotonic").stdout.splitlines() if "=" in l)
    mono = int(p.get("ActiveEnterTimestampMonotonic") or 0)
    e = {"active": p.get("ActiveState"), "restarts": int(p.get("NRestarts") or 0),
         "pid": int(p.get("MainPID") or 0), "since": p.get("ActiveEnterTimestamp"),
         "uptime_s": int(boot - mono / 1e6) if mono else None}
    if e["pid"] and u.startswith("postfiat-validator"):
        h = hashlib.sha256()
        with open(f"/proc/{e['pid']}/exe", "rb") as f:
            for chunk in iter(lambda: f.read(1 << 20), b""):
                h.update(chunk)
        e["exe_sha256"], e["exe"] = h.hexdigest(), os.readlink(f"/proc/{e['pid']}/exe")
    units[u] = e
out["units"] = units
binary, config = f"/opt/postfiat/releases/{release}/postfiat-node", f"/etc/postfiat/releases/{release}"
out["manifest_verify_rc"] = sh(binary, "deployment-manifest-verify",
    "--manifest-file", f"{config}/deployment-manifest.json",
    "--trusted-publisher-key-file", f"{config}/deployment.public.json", "--validator-id", v,
    "--validator-bindings-file", f"{config}/{v}.bindings.json", "--runtime-binary-file", binary,
    "--runtime-topology-file", f"{config}/topology.json",
    "--runtime-swap-circuit-metadata-file", f"{config}/swap.metadata.json",
    "--runtime-private-egress-circuit-metadata-file", f"{config}/private-egress.metadata.json").returncode
relays = {}
for line in sh("ss", "-ltnpH").stdout.splitlines():
    m = re.search(r"127\.0\.0\.1:(2870[123])\s.*pid=(\d+)", line)
    if not m:
        continue
    port, pid = m.groups()
    argv = open(f"/proc/{pid}/cmdline", "rb").read().decode(errors="replace").split("\0")
    script = next((a for a in argv if a.endswith(".py")), "")
    upstream = argv[argv.index("--upstream") + 1] if "--upstream" in argv else ""
    relays[port] = {"pid": int(pid),
                    "managed_by": open(f"/proc/{pid}/cgroup").read().strip().rsplit("/", 1)[-1],
                    "script": script, "script_exists": bool(script) and os.path.exists(script),
                    "upstream_host": urllib.parse.urlsplit(upstream).hostname}
out["relays"] = relays
body = json.dumps({"jsonrpc": "2.0", "id": 1, "method": "eth_getCode", "params": [contract, hex(int(block))]})
request = urllib.request.Request("http://127.0.0.1:28703", data=body.encode(),
                                 headers={"Content-Type": "application/json"})
try:
    with urllib.request.urlopen(request, timeout=20) as r:
        reply = json.loads(r.read())
        code = reply.get("result") or ""
        out["archive"] = {"http": r.status, "code_bytes": max(0, (len(code) - 2) // 2),
                          "error": (reply.get("error") or {}).get("message")}
except urllib.error.HTTPError as e:
    out["archive"] = {"http": e.code, "code_bytes": 0, "error": str(e.reason)[:80]}
except Exception as e:
    out["archive"] = {"http": None, "code_bytes": 0, "error": f"{type(e).__name__}: {str(e)[:80]}"}
if signer == "1":
    key, records = f"/var/lib/postfiat/{v}/validator_keys.json", f"/var/lib/postfiat/{v}/ethereum-checkpoint-signing"
    files = [e for e in os.scandir(records) if e.is_file()] if os.path.isdir(records) else []
    out["checkpoint"] = {"key_file": key, "key_present": os.path.isfile(key) and os.path.getsize(key) > 0,
                         "records_dir": records, "records": len(files),
                         "newest_unix": max((e.stat().st_mtime for e in files), default=None)}
try:
    out["readiness"] = json.load(open(f"/var/lib/postfiat/{v}/readiness/rpc.ready.json"))
except Exception as e:
    out["readiness"] = {"error": f"{type(e).__name__}"}
st = os.statvfs("/")
out["disk_free_bytes"] = st.f_bavail * st.f_frsize
out["logrotate"] = {n: os.path.isfile(f"/etc/logrotate.d/{n}") for n in ("postfiat-validator-events", "postfiat-rpc-events")}
out["apt_upgrade_active"] = sh("systemctl", "is-active", "apt-daily-upgrade.service").stdout.strip()
print(json.dumps(out))
'''


def expected_release():
    desired = json.loads((PACKET / "manifest-input.unsigned.json").read_text())
    after = json.loads((PACKET / "observed/after.json").read_text())
    manifests = {r["deployment_manifest_sha256"] for r in after["validators"]}
    observe.require(len(manifests) == 1, "observed/after.json manifest hashes differ")
    return {"release_id": desired["deployment_id"], "binary_sha256": desired["binary_sha256"],
            "build_git_revision": desired["git_revision"][:8], "manifest_sha256": manifests.pop()}


def status(port):
    try:
        return observe.rpc(port, "status")
    except (OSError, RuntimeError, ValueError):
        time.sleep(6)  # the RPC unit restarts within 5 s after its accept budget
        return observe.rpc(port, "status")


def host_facts(validator, host, release):
    signer = "1" if validator in SIGNERS else "0"
    done = subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", f"root@{host}",
                           "python3", "-", validator, release, signer, ARCHIVE_PROBE[0], str(ARCHIVE_PROBE[1])],
                          input=REMOTE, capture_output=True, text=True, timeout=55)
    observe.require(done.returncode == 0, f"ssh exit {done.returncode}: {done.stderr.strip()[-160:]}")
    return json.loads(done.stdout)


def age(seconds):
    if seconds is None:
        return "?"
    d, rem = divmod(int(seconds), 86400)
    return f"{d}d{rem // 3600}h" if d else f"{rem // 3600}h{rem % 3600 // 60:02d}m"


def evaluate(validator, st, facts, expect, now):
    rows = []
    add = lambda check, result, detail: rows.append((check, validator, result, detail))
    if isinstance(st, Exception) or isinstance(facts, Exception):
        err = st if isinstance(st, Exception) else facts
        add("reachable", "FAIL", f"{type(err).__name__}: {str(err)[:100]}")
        if isinstance(facts, Exception):
            return rows
    if not isinstance(st, Exception):
        wanted = {"node_id": validator, "deployment_validator_id": validator, **observe.CHAIN,
                  "build_git_revision": expect["build_git_revision"], "build_profile": "release",
                  "deployment_manifest_sha256": expect["manifest_sha256"]}
        bad = [k for k, v in wanted.items() if st.get(k) != v]
        if st["deployment_runtime_artifacts"]["binary_sha256"] != expect["binary_sha256"]:
            bad.append("runtime binary")
        binary = f"/opt/postfiat/releases/{expect['release_id']}/postfiat-node"
        for unit, e in facts["units"].items():
            if unit.startswith("postfiat-validator") and (e.get("exe_sha256"), e.get("exe")) != (expect["binary_sha256"], binary):
                bad.append(f"running {unit}")
        if facts["manifest_verify_rc"] != 0:
            bad.append("deployment-manifest-verify")
        add("release", "FAIL" if bad else "PASS",
            f"mismatch: {', '.join(bad)}" if bad else
            f"{expect['release_id']} {expect['build_git_revision']} {expect['binary_sha256'][:8]}, manifest verified")
        pending = st.get("mempool_pending")
        add("mempool", "PASS" if pending == 0 else "ATTENTION", f"{pending} pending")
    units = facts["units"]
    core = [units[f"postfiat-{validator}.service"], units[f"postfiat-{validator}-rpc.service"]]
    aux = {u: e for u, e in units.items() if not u.startswith("postfiat-validator")}
    down = [u for u, e in units.items() if e["active"] != "active"]
    result = "FAIL" if down else "ATTENTION" if (core[1]["uptime_s"] or 0) < 60 else "PASS"
    add("units", result, (f"inactive: {', '.join(down)}; " if down else "")
        + f"validator up {age(core[0]['uptime_s'])} restarts {core[0]['restarts']}; "
        f"rpc up {age(core[1]['uptime_s'])} restarts {core[1]['restarts']}; aux {len(aux) - len(set(down) & set(aux))}/{len(aux)} active")
    archive = facts["archive"]
    ok = archive["http"] == 200 and archive["code_bytes"] > 0
    add("relay 28703 archive", "PASS" if ok else "FAIL" if validator in SIGNERS else "ATTENTION",
        f"eth_getCode @{ARCHIVE_PROBE[1]}: HTTP {archive['http']}, {archive['code_bytes']} B"
        + (f", {archive['error']}" if archive["error"] else ""))
    parts, fragile = [], False
    for port in ("28701", "28702"):
        r = facts["relays"].get(port)
        if not r:
            parts.append(f"{port} not listening")
            continue
        unit = r["managed_by"].endswith(".service")
        fragile |= not unit or not r["script_exists"]
        how = r["managed_by"] if unit else f"hand-started ({r['managed_by']})"
        parts.append(f"{port} {r['upstream_host']} {how}" + ("" if r["script_exists"] else ", script deleted"))
    add("relays 28701/28702", "ATTENTION" if fragile else "PASS", "; ".join(parts))
    if validator in SIGNERS:
        cp = facts["checkpoint"]
        newest = (datetime.fromtimestamp(cp["newest_unix"], timezone.utc).strftime("%Y-%m-%d %H:%MZ")
                  if cp["newest_unix"] else "none")
        add("checkpoint signing", "PASS" if cp["key_present"] and cp["records"] else "FAIL",
            f"{cp['key_file']} {'present' if cp['key_present'] else 'MISSING'}; "
            f"{cp['records']} records, newest {newest}")
    ready = facts["readiness"]
    if "error" in ready:
        add("rpc readiness", "FAIL", f"rpc.ready.json unreadable ({ready['error']})")
    else:
        flags = {k: ready.get(k) for k in READY_FLAGS}
        good = (flags["ready"] is True and flags["degraded"] is False and flags["event_log_writable"] is True
                and flags["listener_bound"] is True and flags["local_state_loaded"] is True
                and flags["finality_key_available"] is True and bool(flags["event_log"]))
        add("rpc readiness", "PASS" if good and not flags["telemetry_failure_count"] else "ATTENTION" if good else "FAIL",
            f"ready={flags['ready']} degraded={flags['degraded']} event_log_writable={flags['event_log_writable']} "
            f"finality_key={flags['finality_key_available']} telemetry_failures={flags['telemetry_failure_count']}")
    free = facts["disk_free_bytes"] / 1e9
    add("disk /", "ATTENTION" if free < DISK_ATTENTION_GB else "PASS", f"{free:.1f} GB free (threshold {DISK_ATTENTION_GB} GB)")
    missing = [n for n, present in facts["logrotate"].items() if not present]
    add("logrotate", "ATTENTION" if missing else "PASS", f"missing: {', '.join(missing)}" if missing else "postfiat-validator-events, postfiat-rpc-events")
    apt = facts["apt_upgrade_active"]
    add("apt-daily-upgrade", "ATTENTION" if apt == "active" else "PASS", f"service {apt}")
    return rows


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--rpc-tunnel-base-port", type=int, default=27650)
    parser.add_argument("--wallet-url", help="optional: HTTP GET this URL and report the status code only")
    parser.add_argument("--json", action="store_true", help="print the full result as JSON")
    args = parser.parse_args()
    started, now = time.monotonic(), datetime.now(timezone.utc)
    expect = expected_release()
    entries = list(enumerate(observe.parse_inventory(PACKET / "inventory.txt")))

    def safe(fn, *a):
        try:
            return fn(*a)
        except Exception as error:  # reported as a FAIL row
            return error

    with ThreadPoolExecutor(max_workers=2 * len(entries)) as pool:
        statuses = [pool.submit(safe, status, args.rpc_tunnel_base_port + i) for i, _ in entries]
        facts = [pool.submit(safe, host_facts, e.validator_id, e.host, expect["release_id"]) for _, e in entries]
        statuses, facts = [f.result() for f in statuses], [f.result() for f in facts]
    rows = []
    good = [s for s in statuses if not isinstance(s, Exception)]
    identities = {(s["block_height"], s["block_tip_hash"], s["state_root"]) for s in good}
    if len(good) == len(entries) and len(identities) == 1:
        height, tip, root = next(iter(identities))
        rows.append(("ledger agreement", "fleet", "PASS", f"6/6 at height {height}, tip {tip[:8]}, root {root[:8]}"))
    else:
        seen = sorted({(h, t[:8], r[:8]) for h, t, r in identities})
        rows.append(("ledger agreement", "fleet", "FAIL", f"{len(good)}/{len(entries)} answered; identities {seen}"))
    for (_, e), st, fa in zip(entries, statuses, facts):
        rows.extend(evaluate(e.validator_id, st, fa, expect, now))
    if args.wallet_url:
        try:
            with urllib.request.urlopen(urllib.request.Request(args.wallet_url, method="GET"), timeout=15) as r:
                code = r.status
        except urllib.error.HTTPError as error:
            code = error.code
        except Exception as error:
            code = f"{type(error).__name__}"
        rows.append(("wallet url", "probe", "PASS" if isinstance(code, int) and code < 400 else "FAIL", f"HTTP {code}"))
    counts = {k: sum(r[2] == k for r in rows) for k in ("PASS", "ATTENTION", "FAIL")}
    verdict = "NOT READY" if counts["FAIL"] else "READY WITH ATTENTION" if counts["ATTENTION"] else "READY"
    elapsed = time.monotonic() - started
    line = (f"VERDICT: {verdict} - {counts['PASS']} PASS, {counts['ATTENTION']} ATTENTION, {counts['FAIL']} FAIL "
            f"({now.strftime('%Y-%m-%dT%H:%M:%SZ')}, {elapsed:.0f} s)")
    if args.json:
        print(json.dumps({"observed_at": now.isoformat(), "verdict": verdict, "counts": counts,
                          "expected": expect, "checks": [dict(zip(("check", "scope", "result", "detail"), r)) for r in rows],
                          "hosts": {e.validator_id: (str(f) if isinstance(f, Exception) else f) for (_, e), f in zip(entries, facts)}},
                         indent=2))
    else:
        widths = [max(len(str(r[i])) for r in rows + [("Check", "Scope", "Result")]) for i in range(3)]
        print(f"{'Check':<{widths[0]}}  {'Scope':<{widths[1]}}  {'Result':<{widths[2]}}  Detail")
        for check, scope, result, detail in rows:
            print(f"{check:<{widths[0]}}  {scope:<{widths[1]}}  {result:<{widths[2]}}  {detail}")
        print(line)
    sys.exit(2 if counts["FAIL"] else 1 if counts["ATTENTION"] else 0)


if __name__ == "__main__":
    main()
