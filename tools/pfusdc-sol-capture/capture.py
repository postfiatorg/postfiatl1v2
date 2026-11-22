#!/usr/bin/env python3
"""Run the local Alpenglow capture gate and retain the slot archive and witness."""

import argparse
import os
from pathlib import Path
import resource
import subprocess
import time


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path, help="new directory for this cluster run")
    parser.add_argument("--binary", type=Path, default=Path(__file__).resolve().parent
                        / "target/release/pfusdc-sol-capture")
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    directory = args.directory.resolve()
    directory.mkdir(parents=True, exist_ok=False)
    archive = directory / "archive"
    witness = directory / "witness.bin"
    env = {**os.environ, "FARF_DIR": str(directory / "ledgers")}
    started = time.monotonic()
    subprocess.run([binary, "local", archive], env=env, check=True)
    print(f"cluster_elapsed_s={time.monotonic() - started:.3f} "
          f"cluster_peak_rss_kib={resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss}",
          flush=True)
    slots = [p.name for p in archive.iterdir() if p.is_dir() and p.name.isdecimal()]
    if len(slots) != 1:
        raise RuntimeError(f"expected one captured slot, found {slots}")
    for stage, command in [("build", ["build", archive, slots[0], witness]),
                           ("verify", ["verify", witness])]:
        started = time.monotonic()
        subprocess.run([binary, *command], check=True)
        print(f"{stage}_elapsed_s={time.monotonic() - started:.3f}", flush=True)
    print(f"witness={witness}")


if __name__ == "__main__":
    main()
