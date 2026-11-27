#!/usr/bin/env python3
"""Fetch the SP1 Groth16 circuit archive with parallel range requests and install it.

One connection to the bucket gives about 5 MB/s from a Colab VM and 16 give about 100 MB/s
(measured 2026-11-26). The archive's sha256 is pinned here and each installed file is checked
against circuits.sha256, so the proofs never download anything themselves."""
import hashlib, os, subprocess, sys, urllib.request
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path
URL = "https://sp1-circuits.s3-us-east-2.amazonaws.com/v6.1.0-groth16.tar.gz"
SHA = "18beebb6cd0cc9b4d4a240ee4f49511da6c2a7e51724bad4232de538a9147810"
SIZE = 6211807514
DEST = Path("/root/.sp1/circuits/groth16/v6.1.0")
PART = 128 * 1024 * 1024
def main():
    sums = Path(sys.argv[1]).read_text()
    if (DEST / ".complete").exists() and subprocess.run(["sha256sum", "-c", "--quiet", sys.argv[1]], cwd=DEST).returncode == 0:
        print("circuits already installed"); return
    tar = Path("/content/circuits.tar.gz")
    with tar.open("wb") as f: f.truncate(SIZE)
    def part(start):
        end = min(start + PART, SIZE) - 1
        for attempt in range(5):
            try:
                req = urllib.request.Request(URL, headers={"Range": f"bytes={start}-{end}"})
                data = urllib.request.urlopen(req, timeout=120).read()
                if len(data) == end - start + 1:
                    fd = os.open(tar, os.O_WRONLY); os.pwrite(fd, data, start); os.close(fd); return
            except Exception as error: last = error
        raise RuntimeError(f"range {start} failed: {last}")
    with ThreadPoolExecutor(16) as pool: list(pool.map(part, range(0, SIZE, PART)))
    digest = hashlib.sha256()
    with tar.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 24), b""): digest.update(chunk)
    if digest.hexdigest() != SHA: raise SystemExit("circuit archive hash differs from the pinned one")
    DEST.mkdir(parents=True, exist_ok=True)
    subprocess.run(["tar", "-xzf", str(tar), "-C", str(DEST), "--warning=no-unknown-keyword"], check=True)
    subprocess.run(["sha256sum", "-c", "--quiet", sys.argv[1]], cwd=DEST, check=True)
    (DEST / ".complete").touch(); tar.unlink()
    print("circuits installed")
main()
