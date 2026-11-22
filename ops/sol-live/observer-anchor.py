"""Run on the funded observer host to archive a certified bank in a later epoch."""

import fcntl
import json
from pathlib import Path
import subprocess
import sys

base = Path("/work/g")
target = base / "sol-live" / "archive"
target.mkdir(parents=True, exist_ok=True)
first, observed = map(int, sys.argv[1:3])
with (target.parent / "anchor.lock").open("a") as lock:
    fcntl.flock(lock, fcntl.LOCK_EX)
    ready = [
        p
        for p in target.glob("*/certificate")
        if p.parent.name.isdecimal() and first <= int(p.parent.name) <= observed
    ]
    if ready:
        print(max(int(p.parent.name) for p in ready))
        sys.exit()

    def pairs():
        probe = subprocess.run(
            ["python3", base / "select-main.py"],
            capture_output=True,
            text=True,
            cwd=base,
        )
        return (
            [r for r in json.loads(probe.stdout) if r["target"] >= first]
            if probe.returncode == 0
            else []
        )

    rows = pairs()
    if not rows:
        subprocess.run(
            [
                "python3",
                base / "time-window.py",
                "mainnet-beta",
                "main-ledger",
                "18899",
            ],
            cwd=base,
            check=True,
            timeout=180,
        )
        rows = pairs()
    if not rows:
        raise RuntimeError("observer did not yield a finalized target and carrier")
    row = rows[0]
    root = base / "main-ledger" / "public-capture"
    subprocess.run(
        [
            base / "pfusdc-sol-capture",
            "public",
            root / row["t"],
            root / row["k"],
            target,
            *sys.argv[3:5],
        ],
        check=True,
    )
    print(row["target"])
