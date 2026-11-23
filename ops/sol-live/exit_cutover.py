"""Check the running relay configuration or poll the new service before opening."""
import argparse
import json
from pathlib import Path
import subprocess
import time
import requests


def relay_flag(entries):
    flags = [e for e in entries if e.startswith(b"L1_SOLANA_WITHDRAWALS_ENABLED=")]
    if flags != [b"L1_SOLANA_WITHDRAWALS_ENABLED=false"]:
        raise ValueError("The running relay must explicitly disable Solana withdrawals.")


def relay(unit):
    def pid():
        process = subprocess.check_output(["systemctl", "show", "--property=MainPID", "--value", unit],text=True).strip()
        if not process.isdecimal() or int(process) <= 1:
            raise ValueError("No running relay MainPID.")
        return Path("/proc")/process
    proc = pid()
    if not any(arg.endswith(b"server/l1/relay.js") for arg in (proc/"cmdline").read_bytes().split(b"\0")):
        raise ValueError("MainPID does not run the configured relay entry point.")
    relay_flag((proc/"environ").read_bytes().split(b"\0"))
    if pid() != proc:
        raise ValueError("Relay restarted during inspection.")


def wait_closed(api, seconds):
    end = time.monotonic() + seconds
    while (remaining := end - time.monotonic()) > 0:
        try:
            response = requests.get(api+"/readiness",timeout=min(10,remaining))
            response.raise_for_status()
            data = response.json()
            if data.get("burns_open") is True:
                raise RuntimeError("Burns are already open.")
            if data.get("payout_ready") is True and data.get("burns_open") is False and data.get("ready") is False:
                return data
        except (requests.RequestException, ValueError):
            pass
        time.sleep(min(1,max(0,end-time.monotonic())))
    raise TimeoutError("Closed payout readiness deadline expired.")


if __name__ == "__main__":
    p=argparse.ArgumentParser(description=__doc__)
    sub=p.add_subparsers(dest="command",required=True)
    sub.add_parser("relay-disabled").add_argument("--unit",required=True)
    wait=sub.add_parser("wait-closed")
    wait.add_argument("--api",required=True)
    wait.add_argument("--seconds",type=int,default=600)
    a=p.parse_args()
    if a.command=="relay-disabled":
        relay(a.unit)
    else:
        print(json.dumps(wait_closed(a.api,a.seconds)))
