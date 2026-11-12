#!/usr/bin/env python3
"""Six-node check: RPC status (build, height, root, mempool) plus the sha256 of the
actual executables of both node processes on each host. Read-only.
Usage: PYTHONDONTWRITEBYTECODE=1 python3 ops/b23/fleet_check.py OUT.json
Exit 0 only if all six report one (height, root)."""
from __future__ import annotations

import json
import subprocess
import sys

sys.path.insert(0, "/home/postfiat/repos/postfiatl1v2-navcoin-proof-b2.2/.scratch-deposit")
import pftl  # noqa: E402

SHAS = {"092314967d021c2f": "ca287e65(rel3)", "b4a6d3fc3d6d0b45": "7b4d850d(rel2)", "c822bac5fd087eed": "bbefee80(rel1)", "35513b7ce319162e": "5d13c775(r2-1)"}


def exes(host: str) -> list[str]:
    cmd = ("for p in $(pgrep -f 'postfiat-node (transport-validator-serve|rpc-serve)'); do "
           "echo \"$(sha256sum /proc/$p/exe | cut -c1-16) $(awk '/VmRSS/{print $2}' /proc/$p/status) "
           "$(tr '\\0' ' ' < /proc/$p/cmdline | awk '{print $2}')\"; done")
    out = subprocess.run(["ssh", "-o", "BatchMode=yes", "-o", "ConnectTimeout=8", f"root@{host}", cmd],
                         text=True, capture_output=True, timeout=40).stdout.split("\n")
    rows = []
    for line in filter(None, out):
        sha, rss, role = line.split()[:3]
        rows.append(f"{role.split('-')[0]}:{SHAS.get(sha, sha)}:{int(rss)//1024}MB")
    return sorted(rows)


def main() -> int:
    report = {}
    for name, host, port in pftl.PEERS:
        try:
            st = pftl.rpc("status", {}, "fc", (name, host, port), 40)
            report[name] = {"build": st.get("build_git_revision"), "height": st.get("block_height"),
                            "root": st.get("state_root"), "mempool": st.get("mempool_pending"),
                            "processes": exes(host)}
        except Exception as exc:  # unreachable node is data
            report[name] = {"error": str(exc)[:200]}
        r = report[name]
        print(name, r.get("build"), r.get("height"), str(r.get("root"))[:16], "mempool", r.get("mempool"),
              r.get("processes"), r.get("error", ""))
    points = {(r.get("height"), r.get("root")) for r in report.values()}
    json.dump(report, open(sys.argv[1], "w"), indent=1)
    print("CONVERGED" if len(points) == 1 else f"DIVERGED {points}")
    return 0 if len(points) == 1 else 2


if __name__ == "__main__":
    sys.exit(main())
