#!/usr/bin/env python3
"""Hold the GPU lease while running the prover and its SDK child processes."""
import argparse
import fcntl
import os
from pathlib import Path
import signal
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--lock", type=Path, required=True)
    start_mode = parser.add_mutually_exclusive_group(required=True)
    start_mode.add_argument("--measure-cold", action="store_true", help="include first artifact load in the first request")
    start_mode.add_argument("--prime", type=Path, help="valid deposit for startup Groth16 warmup")
    parser.add_argument("host", type=Path)
    parser.add_argument("elf_dir", type=Path)
    parser.add_argument("socket", type=Path)
    parser.add_argument("gpus", type=int)
    args = parser.parse_args()
    stopping = False

    def stop(_signum, _frame):
        nonlocal stopping
        stopping = True

    for sig in (signal.SIGINT, signal.SIGTERM, signal.SIGHUP):
        signal.signal(sig, stop)

    command = [args.host.resolve(), "serve", args.elf_dir, args.socket, str(args.gpus)]
    command.append(args.prime or "--measure-cold")
    with args.lock.open("a") as lock:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
        if args.socket.exists():
            parser.error(f"socket path already exists: {args.socket}")
        proc = subprocess.Popen(command, start_new_session=True, close_fds=True)
        try:
            while proc.poll() is None and not stopping:
                time.sleep(0.2)
        finally:
            # The SDK starts a child per GPU. Signal only this job's process group.
            for sig in (signal.SIGTERM, signal.SIGKILL):
                try:
                    os.killpg(proc.pid, sig)
                except ProcessLookupError:
                    break
                if sig == signal.SIGTERM:
                    try:
                        proc.wait(timeout=3)
                    except subprocess.TimeoutExpired:
                        pass
            proc.wait()
            args.socket.unlink(missing_ok=True)
        return 0 if stopping else proc.returncode


if __name__ == "__main__":
    raise SystemExit(main())
