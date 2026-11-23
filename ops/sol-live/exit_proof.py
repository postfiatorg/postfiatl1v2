"""Use the existing frozen withdrawal proof host and its shared GPU reservation."""

from datetime import datetime, timezone
from functools import lru_cache
import base64
import os
import shlex
import hashlib
import json
from pathlib import Path
import subprocess


HOST = "root@ssh8.vast.ai"
PORT = "20628"
REMOTE = "/workspace/b23-withdraw-20261116"
LOCAL = Path(
    os.getenv("EXIT_PROOF_HOME", Path.home() / "repos/b23-coord/evidence/withdraw")
)
SSH = [
    "ssh",
    "-oBatchMode=yes",
    "-oConnectTimeout=8",
    "-oServerAliveInterval=20",
    "-p",
    PORT,
    HOST,
]
SCP = ["scp", "-q", "-oBatchMode=yes", "-oConnectTimeout=8", "-P", PORT]


def run(args, timeout):
    result = subprocess.run(
        args, timeout=timeout, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE
    )
    if result.returncode == 75:
        raise RuntimeError(
            "GPU prover is busy. Retry this burn when the current job finishes."
        )
    if result.returncode:
        raise RuntimeError(
            f"{Path(args[0]).name} failed with exit status {result.returncode}"
        )


@lru_cache(maxsize=1)
def verify_local_host():
    with (LOCAL / "historical/egress.elf").open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    if (
        digest != "4d5f84493c9b02b0d2a082c446229e30ce6645210a00c271dfb125b2761c67e0"
        or not (LOCAL / "bin/proof-host").is_file()
    ):
        raise ValueError("Frozen withdrawal proof host is unavailable.")


def available():
    verify_local_host()
    run(SSH + [f"test -x {REMOTE}/proof-host && test -s {REMOTE}/egress.elf"], 15)


def remote_proof(kind, data):
    # One fixed scratch path, exclusively under the existing GPU lock. An interrupted
    # session leaves at most one directory, removed on the next lock acquisition.
    script = f"""
set -eu
umask 077
cd {REMOTE}
job=jobs/sol-exit-worker
mkdir -p jobs
rm -rf -- "$job"
mkdir "$job"
trap 'rm -rf -- "$job"' EXIT
cat > "$job/witness.json"
SP1_PROVER=cuda RAYON_NUM_THREADS=4 timeout 1500 ./proof-host {kind} egress.elf "$job/witness.json" "$job/proof" prove > "$job/prove.log" 2>&1
for file in public-values.bin proof-report.json proof-calldata.bin; do
    test "$(wc -c < "$job/proof/$file")" -le 8192
    base64 -w0 "$job/proof/$file"
    echo
done
"""
    command = "flock -E 75 -n /workspace/b23-gpu/prover.lock bash -c " + shlex.quote(
        script
    )
    result = subprocess.run(
        SSH + [command], input=data, capture_output=True, timeout=1550
    )
    if result.returncode:
        raise RuntimeError("Withdrawal prover unavailable.")
    lines = result.stdout.splitlines()
    if len(lines) != 3:
        raise ValueError("Incomplete proof response.")
    return [base64.b64decode(line, validate=True) for line in lines]


def prove(kind, witness, directory):
    if kind not in ("checkpoint", "withdrawal"):
        raise ValueError("unknown proof kind")
    verify_local_host()
    data = json.dumps(witness, separators=(",", ":")).encode()
    if len(data) > 24 * 1024 * 1024:
        raise ValueError("Witness exceeds the worker size limit.")
    work = directory / hashlib.sha256(data).hexdigest()
    work.mkdir(parents=True, exist_ok=True)
    source = work / "witness.json"
    source.write_bytes(data)
    run(
        [
            LOCAL / "bin/proof-host",
            kind,
            LOCAL / "historical/egress.elf",
            source,
            work,
            "native",
        ],
        120,
    )
    native = (work / "native-public-values.bin").read_bytes()
    proof = work / "proof-calldata.bin"
    if (
        not proof.exists()
        or proof.stat().st_size != 356
        or (work / "public-values.bin").read_bytes() != native
    ):
        hhmm = datetime.now(timezone.utc).strftime("%H%M")
        if "1803" <= hhmm < "1827":
            raise RuntimeError(
                "Proof host reserved for its scheduled job. Retry after 18:27 UTC."
            )
        values, report, calldata = remote_proof(kind, data)
        if (
            values != native
            or len(calldata) != 356
            or not json.loads(report)["verified"]
        ):
            raise ValueError("Unverified proof response.")
        for name, contents in (
            ("public-values.bin", values),
            ("proof-report.json", report),
            ("proof-calldata.bin", calldata),
        ):
            path = work / name
            temp = path.with_suffix(".partial")
            temp.write_bytes(contents)
            temp.replace(path)
    if (
        work / "public-values.bin"
    ).read_bytes() != native or proof.stat().st_size != 356:
        raise ValueError("proof host returned mismatched proof artifacts")
    return native, proof.read_bytes()
