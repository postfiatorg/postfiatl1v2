"""Check the running relay configuration or poll the new service before opening."""

import argparse
import json
from pathlib import Path
import subprocess
import time
import requests

WITHDRAW_REV = "56dbebb4672172c9f336dc95b831deb8281e3fd7"


def relay_flag(entries, enabled=False):
    flags = [e for e in entries if e.startswith(b"L1_SOLANA_WITHDRAWALS_ENABLED=")]
    expected = b"true" if enabled else b"false"
    if flags != [b"L1_SOLANA_WITHDRAWALS_ENABLED=" + expected]:
        raise ValueError(
            "The running relay must explicitly "
            + ("enable" if enabled else "disable")
            + " Solana withdrawals."
        )
    if enabled:
        urls = [e for e in entries if e.startswith(b"L1_SOLANA_WITHDRAW_SERVICE_URL=")]
        if urls != [b"L1_SOLANA_WITHDRAW_SERVICE_URL=http://127.0.0.1:18089"]:
            raise ValueError("Unexpected payout service URL.")


def relay_entry(cwd, args, release):
    if (
        len(args) != 2
        or (cwd / args[1]).resolve() != release.resolve() / "server/l1/relay.js"
    ):
        raise ValueError("Relay is not executing the reviewed release entry point.")


def relay(unit, user=False, *, release=None, enabled=False, node=None, probe_module=None):
    def pid():
        process = subprocess.check_output(
            [
                "systemctl",
                *(["--user"] if user else []),
                "show",
                "--property=MainPID",
                "--value",
                unit,
            ],
            text=True,
        ).strip()
        if not process.isdecimal() or int(process) <= 1:
            raise ValueError("No running relay MainPID.")
        return Path("/proc") / process

    proc = pid()
    if not any(
        arg.endswith(b"server/l1/relay.js")
        for arg in (proc / "cmdline").read_bytes().split(b"\0")
    ):
        raise ValueError("MainPID does not run the configured relay entry point.")
    if release is not None:
        args = (proc / "cmdline").read_text().rstrip("\0").split("\0")
        if node is not None:
            if not (proc / "exe").samefile(node) or args[1:3] != ["--require", str(probe_module)]:
                raise ValueError("Relay runtime or disabled-probe module differs.")
            args = args[:1] + args[3:]
        relay_entry((proc / "cwd").resolve(), args, release)
        revision = subprocess.check_output(
            ["git", "-C", release, "rev-parse", "HEAD"], text=True
        )
        if revision.strip() != WITHDRAW_REV:
            raise ValueError("Different relay release revision.")
        subprocess.run(
            ["git", "-C", release, "diff", "--quiet", WITHDRAW_REV, "--"], check=True
        )
    relay_flag((proc / "environ").read_bytes().split(b"\0"), enabled=enabled)
    if pid() != proc:
        raise ValueError("Relay restarted during inspection.")


def wait_closed(api, seconds):
    end = time.monotonic() + seconds
    while (remaining := end - time.monotonic()) > 0:
        try:
            response = requests.get(api + "/readiness", timeout=min(10, remaining))
            response.raise_for_status()
            data = response.json()
            if data.get("burns_open") is True:
                raise RuntimeError("Burns are already open.")
            if (
                data.get("payout_ready") is True
                and data.get("burns_open") is False
                and data.get("ready") is False
            ):
                return data
        except (requests.RequestException, ValueError):
            pass
        time.sleep(min(1, max(0, end - time.monotonic())))
    raise TimeoutError("Closed payout readiness deadline expired.")


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    sub = p.add_subparsers(dest="command", required=True)
    for action in ("relay-disabled", "relay-enabled"):
        relay_parser = sub.add_parser(action)
        relay_parser.add_argument("--unit", required=True)
        relay_parser.add_argument("--user", action="store_true")
        relay_parser.add_argument("--release", required=True, type=Path)
    wait = sub.add_parser("wait-closed")
    wait.add_argument("--api", required=True)
    wait.add_argument("--seconds", type=int, default=600)
    a = p.parse_args()
    if a.command.startswith("relay-"):
        relay(a.unit, a.user, release=a.release, enabled=a.command == "relay-enabled")
    else:
        print(json.dumps(wait_closed(a.api, a.seconds)))
