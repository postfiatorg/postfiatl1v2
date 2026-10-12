#!/usr/bin/env python3
"""DEPLOY-SHEET section 7, one host: FastSwap store conversion (2026-10-12).

Usage: fastswap-convert-host.py N   (validator-N). Halts on the first failed check.
Exit 0 = host converted, healthy, six-way agreement after.
"""
import json, socket, subprocess, sys, time
from datetime import datetime, timezone
from pathlib import Path

HOSTS = {0: "64.176.220.75", 1: "95.179.184.122", 2: "66.42.48.39",
         3: "149.28.63.106", 4: "95.179.179.206", 5: "45.32.110.170"}
D = Path.home() / ".postfiat/deployments/signer-rotation-20261007"
E, S = D / "evidence", D / "stage"
REPO = Path.home() / "repos/postfiatl1v2-rel20261007"
PACKET = REPO / "deployments/signer-rotation-20261007"
APPLIED = "validator-1,validator-0,validator-2,validator-3,validator-4,validator-5"
BIN = "/opt/postfiat/releases/signer-rotation-20261007/postfiat-node"
EXPECT_RECORDS, EXPECT_BYTES = 26, 177899
FASTSWAP_REQUIRED = {0, 1, 2, 3, 4}   # validator-5's local key is not its epoch-1 committee key

n = int(sys.argv[1]); v = f"validator-{n}"; ip = HOSTS[n]; port = 27650 + n
DATA = f"/var/lib/postfiat/{v}"; STORE = f"{DATA}/fastswap-v1"
BACKUP = f"/var/lib/postfiat/fastswap-v1-backup-{v}"
UNITS = (f"postfiat-{v}.service", f"postfiat-{v}-rpc.service")
record = {"validator": v, "host": ip}
LOG = E / f"fastswap-convert-{v}.log"


def now():
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def log(msg):
    line = f"[{now()}] {v}: {msg}"
    print(line, flush=True)
    with LOG.open("a") as f:
        f.write(line + "\n")


def save():
    (E / f"fastswap-convert-{v}.record.json").write_text(json.dumps(record, indent=2) + "\n")


class Halt(Exception):
    pass


def ssh(cmd, check=True):
    r = subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=15", f"root@{ip}", cmd],
                       capture_output=True, text=True)
    if check and r.returncode:
        raise Halt(f"ssh rc={r.returncode}: {cmd!r}: {r.stderr.strip()[:400]}")
    return r


def rpc(p, method, params=None):
    req = {"version": "postfiat-local-rpc-v1", "id": f"fastswap-convert-{method}",
           "method": method, "params": params or {}}
    with socket.create_connection(("127.0.0.1", p), timeout=30) as sock:
        stream = sock.makefile("rwb")
        stream.write((json.dumps(req) + "\n").encode())
        stream.flush()
        return json.loads(stream.readline(16 * 1024 * 1024))


def observe(tag):
    out = E / f"observe-{tag}-{v}.json"
    r = subprocess.run(["python3", str(PACKET / "observe-fleet.py"), "--stage", str(S), "--applied", APPLIED,
                        "--output", str(out)], capture_output=True, text=True, cwd=REPO)
    log(f"observe {tag}: rc={r.returncode} {r.stdout.strip()} {r.stderr.strip()[-400:]}")
    if r.returncode:
        raise Halt(f"observe {tag} failed")
    return json.loads(out.read_text())["ledger_identity"]


def store_listing():
    r = ssh(f"cd {STORE} && stat -c '%s %U:%G %a %n' * .integrity.key 2>/dev/null; "
            f"du -sb --apparent-size . | cut -f1; ls vote-artifacts | wc -l; sha256sum fastswap-v1.wal")
    lines = r.stdout.strip().splitlines()
    return {"files": lines[:-3], "dir_bytes": int(lines[-3]), "vote_artifacts": int(lines[-2]),
            "wal_sha256": lines[-1].split()[0]}


def migrate(name, *flags):
    r = ssh(f"cd / && runuser -u postfiat -- {BIN} fastswap-store-migrate --data-dir {DATA} {' '.join(flags)}",
            check=False)
    (E / f"{name}-{v}.json").write_text(r.stdout if r.stdout.strip() else json.dumps(
        {"exit_code": r.returncode, "stderr": r.stderr.strip()}) + "\n")
    log(f"{name}: rc={r.returncode} stderr={r.stderr.strip()[:300]!r}")
    return r.returncode, (json.loads(r.stdout) if r.returncode == 0 else None)


def units_state():
    return ssh(f"systemctl is-active {' '.join(UNITS)}", check=False).stdout.split()


def fleet_status():
    rows = {}
    for i in range(6):
        try:
            s = rpc(27650 + i, "status")
            rows[i] = (s["result"]["block_height"], s["result"]["block_tip_hash"], s["result"]["state_root"]) \
                if s.get("ok") else None
        except (OSError, ValueError, KeyError):
            rows[i] = None
    return rows


def start_units():
    t = int(ssh(f"date +%s; systemctl start {UNITS[0]} {UNITS[1]}").stdout.split()[0])
    record["start_time"] = datetime.fromtimestamp(t, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    log(f"started {UNITS} at {record['start_time']}")
    return t


def wait_rejoin(t0, limit):
    while True:
        elapsed = int(time.time()) - t0
        states = units_state()
        rows = fleet_status()
        others = {rows[i] for i in rows if i != n}
        ready = None
        r = ssh(f"stat -c %Y {DATA}/readiness/rpc.ready.json && cat {DATA}/readiness/rpc.ready.json", check=False)
        if r.returncode == 0:
            mtime, body = r.stdout.split("\n", 1)
            ready = json.loads(body).get("ready") is True and int(mtime) >= t0
        if states == ["active", "active"] and len(others) == 1 and rows[n] in others and ready:
            log(f"rejoined after {elapsed}s: height={rows[n][0]} tip={rows[n][1][:12]} root={rows[n][2][:12]} rpc ready")
            return elapsed, rows[n]
        if elapsed > limit:
            log(f"not rejoined after {elapsed}s: units={states} host={rows[n]} others={others} ready={ready}")
            return None, None
        time.sleep(5)


def main():
    LOG.touch()
    log("begin")
    ident = observe("before")
    record["before_identity"] = ident
    record["disk_avail_bytes"] = int(ssh("df -B1 --output=avail /var/lib/postfiat | tail -1").stdout)
    if record["disk_avail_bytes"] < 2 * 10**9:
        raise Halt("free disk below 2 GB")
    record["before_store"] = store_listing()
    log(f"before: identity h={ident['height']} disk_avail={record['disk_avail_bytes']} store={record['before_store']}")
    if ssh(f"test ! -e {BACKUP}", check=False).returncode:
        raise Halt(f"{BACKUP} already exists")
    ssh(f"install -d -o postfiat -g postfiat -m 700 {BACKUP} && stat -c '%U:%G %a %n' {BACKUP}")
    log(f"created empty {BACKUP} (postfiat:postfiat 700)")
    save()

    t = int(ssh(f"date +%s; systemctl stop {UNITS[1]} {UNITS[0]}").stdout.split()[0])
    record["stop_time"] = datetime.fromtimestamp(t, timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    states = units_state()
    log(f"stopped at {record['stop_time']}: {states}")
    if states != ["inactive", "inactive"]:
        raise Halt(f"units not inactive after stop: {states}")
    save()

    rc, dry = migrate("store-migrate-dry-run", "--dry-run")
    record["dry_run"] = dry
    if rc or dry["outcome"] != "would-convert" or dry["wal_records"] != dry["legacy_wal_records"] \
            or dry["wal_records"] != EXPECT_RECORDS or dry["wal_bytes"] != EXPECT_BYTES:
        log("dry run not as expected; nothing changed; restarting units")
        start_units(); wait_rejoin(int(time.time()), 300); save()
        raise Halt(f"dry run unexpected: rc={rc} {dry}")

    rc, conv = migrate("store-migrate", "--backup-dir", BACKUP)
    record["convert"] = conv
    if rc or conv["outcome"] != "converted" or conv["converted_wal_records"] != EXPECT_RECORDS \
            or "normal open verified" not in conv["verification"] or conv["backup_files_verified"] < 3:
        after = store_listing()
        record["after_failed_store"] = after; save()
        if after["wal_sha256"] == record["before_store"]["wal_sha256"]:
            log("conversion failed; original WAL in place; restarting units")
            start_units(); wait_rejoin(int(time.time()), 300); save()
        else:
            log("conversion failed AND WAL differs from before; units left stopped")
        raise Halt(f"conversion failed: rc={rc} {conv}")

    rc, again = migrate("store-migrate-recheck", "--dry-run")
    record["recheck"] = again
    record["after_store"] = store_listing()
    record["backup_listing"] = ssh(f"stat -c '%U:%G %a' {BACKUP}; ls -A {BACKUP} | wc -l; "
                                   f"sha256sum {BACKUP}/fastswap-v1.wal | cut -d' ' -f1").stdout.split("\n")[:3]
    log(f"recheck rc={rc} outcome={again and again['outcome']}; after store={record['after_store']}; "
        f"backup={record['backup_listing']}")
    save()
    if rc or again["outcome"] != "nothing-to-convert" or \
            record["backup_listing"][2] != record["before_store"]["wal_sha256"]:
        raise Halt("recheck or backup digest unexpected; units left stopped for manual decision")

    t0 = start_units()
    elapsed, row = wait_rejoin(t0, 300)
    if elapsed is None:
        log("ROLLBACK: restoring backup")
        ts = int(time.time())
        ssh(f"systemctl stop {UNITS[1]} {UNITS[0]} && mv {STORE} {STORE}.converted-{ts} && "
            f"cp -a {BACKUP} {STORE} && systemctl start {UNITS[0]} {UNITS[1]}")
        record["rollback"] = {"at": now(), "aside": f"{STORE}.converted-{ts}"}; save()
        raise Halt("validator did not rejoin within five minutes; rolled back")
    record["rejoin_seconds"] = elapsed
    record["rejoin_identity"] = row
    save()

    for method in ("fastswap_capabilities", "fastswap_checkpoint_status"):
        reply = rpc(port, method)
        (E / f"{method.replace('_', '-')}-{v}.json").write_text(json.dumps(reply, indent=2) + "\n")
        err = None if reply.get("ok") else reply.get("error")
        record[method] = "ok" if reply.get("ok") else err
        log(f"{method}: {'ok' if reply.get('ok') else err}")
    save()
    if n in FASTSWAP_REQUIRED and (record["fastswap_capabilities"] != "ok"
                                   or record["fastswap_checkpoint_status"] != "ok"):
        raise Halt("FastSwap RPC did not answer")

    ident_after = observe("after")
    record["after_identity"] = ident_after
    save()
    log(f"done: six-way agreement h={ident_after['height']} tip={ident_after['tip'][:12]} "
        f"root={ident_after['state_root'][:12]}")


if __name__ == "__main__":
    try:
        main()
    except Exception as error:  # Halt or any unexpected failure
        record["halt"] = str(error); save()
        log(f"HALT: {error}")
        sys.exit(2)
