"""Use the existing frozen withdrawal proof host and its shared GPU reservation."""

from datetime import datetime, timezone
from functools import lru_cache
import base64
import os
import hashlib
import json
from pathlib import Path
import subprocess


HOST = "root@ssh8.vast.ai"
PORT = "20628"
LOCAL = Path(
    os.getenv("EXIT_PROOF_HOME", Path.home() / "repos/b23-coord/evidence/withdraw")
)
def prover_ssh():
    mode = os.getenv("EXIT_PROVER_SSH", "operator")
    directory, identity_name, trust_name = {
        "system": (Path("/var/lib/pf-sol-exit"), "prover", "known_hosts"),
        "sandbox": (Path("/credentials"), "prover", "known_hosts"),
        "operator": (Path.home()/".config/postfiat/sol-bridge", "exit-prover-ssh", "exit-prover-known_hosts"),
    }[mode]
    identity, trust = directory / identity_name, directory / trust_name
    ops = Path(__file__).resolve().parent
    if mode != "sandbox":  # The sandbox launcher verifies both before the read-only mounts.
        run(["bash", ops / "check-prover-identity.sh", identity,
             (ops / "exit-prover.fingerprint").read_text().strip()], 10)
        run(["bash", ops / "check-prover-host.sh", trust], 10)
    return ["ssh", "-F", "/dev/null", "-oIdentitiesOnly=yes", "-oIdentityAgent=none",
            "-i", str(identity), "-oStrictHostKeyChecking=yes", "-oGlobalKnownHostsFile=/dev/null",
            f"-oUserKnownHostsFile={trust}", "-oUpdateHostKeys=no", "-oBatchMode=yes",
            "-oConnectTimeout=8", "-oServerAliveInterval=20", "-p", PORT, HOST]


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
    run(prover_ssh() + ["pf-sol-exit-check"], 15)


def remote_proof(kind, data):
    if kind not in ("checkpoint", "withdrawal"):
        raise ValueError("Unknown proof kind.")
    command = "pf-sol-exit-" + kind
    result = subprocess.run(
        prover_ssh() + [command], input=data, capture_output=True, timeout=1550
    )
    if result.returncode:
        raise RuntimeError("Withdrawal prover unavailable.")
    lines = result.stdout.splitlines()
    if len(lines) != 3:
        raise ValueError("Incomplete proof response.")
    return [base64.b64decode(line, validate=True) for line in lines]


def check_witness(kind, witness, directory):
    """Run the frozen verifier locally. Returns the work folder, the bytes sent and the public values."""
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
    return work, data, (work / "native-public-values.bin").read_bytes()


def public_values(kind, witness, directory):
    """The proof's public values are known in a second, long before the proof."""
    return check_witness(kind, witness, directory)[2]


def prove(kind, witness, directory):
    work, data, native = check_witness(kind, witness, directory)
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
