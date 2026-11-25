"""Plan a transaction-driven release and guard each host restart."""

import argparse
import fcntl
import math
import os
from pathlib import Path
import re
import subprocess
import time

from live import CHAIN, PFTL_GENESIS, HOSTS, read, rpc, save, six, verify_node_ids
from exit_precheck import parent_of
from exit_setup import require
from exit_release_kit import predecessor_kit


def snapshot():
    rows = six(lambda i: rpc(i, "status"))
    verify_node_ids(rows)
    require(
        len(rows) == 6
        and all(
            parent_of(row) == parent_of(rows[0])
            and row["chain_id"] == CHAIN.decode()
            and row["genesis_hash"] == PFTL_GENESIS.hex()
            for row in rows
        ),
        "Six matching finalized statuses are required.",
    )
    return dict(
        time=time.time(),
        validators=[
            dict(parent_of(row), node_id=row["node_id"], revision=row["build_git_revision"]) for row in rows
        ],
    )


def margin(rate, seconds):
    # Budget twice the measured time, a fixed minute and a final spare block.
    return 1 + math.ceil(max(rate, 1 / 60) * (2 * seconds + 60))


def require_margin(h, tip, rate, seconds):
    require(
        h - tip > margin(rate, seconds),
        "Insufficient remaining rollout margin. Stop the roll.",
    )


def rate_since(a, b):
    delta = b["validators"][0]["block_height"] - a["validators"][0]["block_height"]
    dt = b["time"] - a["time"]
    require(dt > 0 and delta >= 0, "Nonmonotonic fleet sample.")
    return delta / dt


def restart_times():
    durations = []
    for i in range(6):
        log = (predecessor_kit() / f"evidence/deploy-v{i}/deploy.log").read_text()

        def seconds(label):
            (stamp,) = re.findall(
                r"^\[(\d\d:\d\d:\d\d)Z\] validator-\d+ @ \S+: " + label + "$", log, re.M
            )
            h, m, s = map(int, stamp.split(":"))
            return h * 3600 + m * 60 + s

        elapsed = (seconds("deployed") - seconds("start")) % 86400
        require(elapsed > 0, "Missing predecessor deployment timing.")
        durations.append(elapsed)
    return durations


def plan(build, replay):
    require(
        all(math.isfinite(s) and s > 0 for s in (build, replay)),
        "Positive rehearsal times required.",
    )
    samples = [snapshot()]
    for _ in range(6):
        time.sleep(10)
        samples.append(snapshot())
    rate = max(rate_since(a, b) for a, b in zip(samples, samples[1:]))
    times = restart_times()
    # Each remaining host gets the slowest measured host plus a watchdog/convergence minute.
    host_seconds = max(times) + 60
    tip = samples[-1]["validators"][0]["block_height"]
    return dict(
        height=max(1755, tip + margin(rate, build + replay + 6 * host_seconds) + 1),
        build_seconds=build,
        replay_seconds=replay,
        restart_seconds=times,
        host_seconds=host_seconds,
        peak_blocks_per_second=rate,
        samples=samples,
    )


def guard(kit, n):
    timing = read(os.environ["EXIT_ROLL_PLAN"])
    now = snapshot()
    history = kit / "evidence/roll-latest.json"
    prev = read(history) if history.exists() else timing["samples"][-1]
    rate = max(
        timing["peak_blocks_per_second"], prev.get("peak", 0), rate_since(prev, now)
    )
    now["peak"] = rate
    save(history, now)
    require(
        int(os.environ["EXIT_ACTIVATION_HEIGHT"]) == timing["height"],
        "Scheduled height differs from timing plan.",
    )
    require_margin(
        timing["height"],
        now["validators"][0]["block_height"],
        rate,
        (6 - n) * timing["host_seconds"],
    )
    return now


def run_one(kit, n):
    log = kit / f"evidence/roll-v{n}.log"
    res = subprocess.run(
        ["bash", kit / "roll-rest.sh", str(n)],
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
    )
    log.write_text(res.stdout)
    require(
        "watchdog restore returned nonzero" not in res.stdout,
        "Host watchdog restoration failed.",
    )
    if res.returncode and (kit / f"evidence/deploy-v{n}/watchdog-before.txt").is_file():
        with (kit / f"evidence/watchdog-abort-v{n}.log").open("w") as restore_log:
            subprocess.run(
                ["bash", kit / "evidence/restore-watchdog.sh", str(n), HOSTS[n]],
                stdout=restore_log,
                stderr=subprocess.STDOUT,
                check=True,
            )
    require(
        res.returncode == 0 and res.stdout.rstrip().endswith("ALL-DONE"),
        f"Host deployment failed. Inspect {log} and restore its watchdogs.",
    )


def roll(kit, n, dry):
    now = guard(kit, n)
    old = (predecessor_kit() / "evidence/source-revision").read_text().strip()[:8]
    rev = os.environ["REV"]
    require(
        [row["revision"] for row in now["validators"]] == [rev] * n + [old] * (6 - n),
        "Unexpected mixed-build state or host order.",
    )
    if n == 0:
        reports = [
            read(kit / f"evidence/tip-{variant}.json") for variant in ("old", "new")
        ]
        require(
            reports[0] == reports[1] and reports[0]["verified"],
            "Full replay comparison differs.",
        )
        result = reports[0]["block_log"]
        row = now["validators"][0]
        require(
            (result["block_count"], result["tip_hash"], result["state_root"])
            == (row["block_height"], row["block_tip_hash"], row["state_root"]),
            "Refresh the pre-roll snapshot.",
        )
        save(kit / "evidence/pre-roll.json", dict(fleet=now, replay=result))
    else:
        require(
            (kit / f"evidence/rolled-v{n - 1}.json").is_file(),
            "Previous host has not passed.",
        )
    if dry:
        return
    os.environ["EXIT_ROLL_GUARD"] = str(Path(__file__).resolve())
    run_one(kit, n)
    # Verify both watchdog states, including one that was deliberately inactive.
    from govern import remote

    units = ("postfiat-outbox-watchdog.service", "postfiat-outbox2-watchdog.service")
    expected = (
        (kit / f"evidence/deploy-v{n}/watchdog-before.txt").read_text().splitlines()
    )
    actual = []
    for unit in units:
        states = [
            remote(
                n, ["sh", "-c", f"systemctl is-{method} {unit} 2>&1 || true"]
            ).strip()
            for method in ("enabled", "active")
        ]
        actual.append(f"{unit} enabled={states[0]} active={states[1]}")
    require(actual == expected, "Host watchdog states differ from pre-roll evidence.")
    now = snapshot()
    require(
        [r["revision"] for r in now["validators"]] == [rev] * (n + 1) + [old] * (5 - n),
        "Host revision or six-way convergence failed.",
    )
    save(kit / f"evidence/rolled-v{n}.json", dict(fleet=now, watchdogs=actual))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="action", required=True)
    p = sub.add_parser("plan")
    p.add_argument("--build-seconds", type=float, required=True)
    p.add_argument("--replay-seconds", type=float, required=True)
    p.add_argument("--output", type=Path, required=True)
    for cmd in ("guard", "roll"):
        p = sub.add_parser(cmd)
        p.add_argument("host", type=int, choices=range(6))
        if cmd == "roll":
            p.add_argument("--dry-run", action="store_true")
    a = parser.parse_args()
    if a.action == "plan":
        save(a.output, plan(a.build_seconds, a.replay_seconds))
    elif a.action == "roll":
        kit = Path(os.environ["KIT"])
        os.environ["EXIT_ACTIVATION_HEIGHT"] = os.environ["H"]
        with (kit / "evidence/roll.lock").open("a") as lock:
            fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
            roll(kit, a.host, a.dry_run)
    else:
        # Mid-deploy check. Once the new units are installed, the old RPC process on the
        # host being rolled refuses status until it restarts, so read the other five.
        timing = read(os.environ["EXIT_ROLL_PLAN"])
        rows = [rpc(i, "status") for i in range(6) if i != a.host]
        require(
            all(parent_of(r) == parent_of(rows[0]) for r in rows)
            and [r["node_id"] for r in rows]
            == [f"validator-{i}" for i in range(6) if i != a.host],
            "Five matching finalized statuses are required mid-deploy.",
        )
        require(int(os.environ["EXIT_ACTIVATION_HEIGHT"]) == timing["height"], "Scheduled height differs from timing plan.")
        require_margin(
            timing["height"],
            rows[0]["block_height"],
            max(timing["peak_blocks_per_second"], 1 / 60),
            (6 - a.host) * timing["host_seconds"],
        )


if __name__ == "__main__":
    main()
