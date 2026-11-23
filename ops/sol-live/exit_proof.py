"""Use the existing frozen withdrawal proof host and its shared GPU reservation."""

from datetime import datetime, timezone
from functools import lru_cache
import hashlib
import json
from pathlib import Path
import subprocess
from uuid import uuid4

HOST = "root@ssh8.vast.ai"
PORT = "20628"
REMOTE = "/workspace/b23-withdraw-20261116"
LOCAL = Path.home() / "repos/b23-coord/evidence/withdraw"
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


def prove(kind, witness, directory):
    if kind not in ("checkpoint", "withdrawal"):
        raise ValueError("unknown proof kind")
    verify_local_host()
    data = json.dumps(witness, separators=(",", ":")).encode()
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
        name = "sol-exit-" + uuid4().hex
        job = f"{REMOTE}/jobs/{name}"
        run(SSH + [f"mkdir -p {job}"], 15)
        run(SCP + [source, f"{HOST}:{job}/witness.json"], 120)
        # A failed lock acquisition never starts CUDA. The worker retains the burn for retry.
        run(
            SSH
            + [
                f"cd {REMOTE} && flock -E 75 -n /workspace/b23-gpu/prover.lock env SP1_PROVER=cuda "
                f"RAYON_NUM_THREADS=4 timeout 1500 ./proof-host {kind} egress.elf {job}/witness.json {job}/proof prove >{job}/prove.log 2>&1"
            ],
            1550,
        )
        for filename in (
            "public-values.bin",
            "proof-report.json",
            "proof-calldata.bin",
        ):
            path = work / filename
            tmp = path.with_suffix(".partial")
            run(SCP + [f"{HOST}:{job}/proof/{filename}", tmp], 90)
            tmp.replace(path)
    if (
        work / "public-values.bin"
    ).read_bytes() != native or proof.stat().st_size != 356:
        raise ValueError("proof host returned mismatched proof artifacts")
    return native, proof.read_bytes()
