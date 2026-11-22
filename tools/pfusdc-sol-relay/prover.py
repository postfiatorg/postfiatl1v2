"""Persist locally verified Groth16 calldata. The remote machine receives only public inputs."""

import hashlib
import json
import os
import shlex
import subprocess
import tempfile
import uuid
from concurrent.futures import ThreadPoolExecutor
from contextlib import nullcontext
from pathlib import Path

from storage import Invalid, locked, save

GPU_LOCK = "/workspace/b23-gpu/prover.lock"


def remote_prove(cfg, elf, witness, kind, out):
    target = cfg.get("gpu_ssh", "root@ssh8.vast.ai")
    port = str(cfg.get("gpu_port", 20628))
    work = "/workspace/sol-relay/" + uuid.uuid4().hex
    ssh = [
        "ssh",
        "-oBatchMode=yes",
        "-oConnectTimeout=8",
        "-oControlMaster=auto",
        "-oControlPersist=15",
        "-oControlPath=" + str(out / "ssh"),
        "-p",
        port,
    ]

    def command(*args):
        subprocess.run([*ssh, target, shlex.join(args)], check=True)

    def upload(pair):
        src, name = pair
        subprocess.run(
            ["rsync", "-a", "-e", shlex.join(ssh), str(src), f"{target}:{work}/{name}"],
            check=True,
        )

    command("mkdir", "-p", work)
    try:
        with ThreadPoolExecutor(max_workers=3) as pool:
            list(
                pool.map(
                    upload,
                    [
                        (cfg["host"], "host"),
                        (elf, "guest"),
                        (witness, "witness"),
                        (Path(__file__).with_name("proof_worker.py"), "worker.py"),
                    ],
                )
            )
        args = ["python3", work + "/worker.py", work, kind]
        backend = "cuda"
        if cfg.get("prover", "auto") == "auto":
            # Exit 75 means the lock was busy. Never disrupt another prover.
            try:
                command(
                    "flock",
                    "-o",
                    "-w",
                    "1",
                    "-E",
                    "75",
                    GPU_LOCK,
                    "env",
                    "SP1_PROVER=cuda",
                    "RAYON_NUM_THREADS=4",
                    *args,
                )
            except subprocess.CalledProcessError as exc:
                if exc.returncode != 75:
                    raise
                backend = "remote_cpu"
                command(
                    "env",
                    "SP1_PROVER=cpu",
                    "RAYON_NUM_THREADS=12",
                    "TOKIO_WORKER_THREADS=12",
                    "GOMAXPROCS=12",
                    *args,
                )
        else:
            command(
                "flock",
                "-o",
                GPU_LOCK,
                "env",
                "SP1_PROVER=cuda",
                "RAYON_NUM_THREADS=4",
                *args,
            )
        subprocess.run(
            [
                "rsync",
                "-a",
                "-e",
                shlex.join(ssh),
                *[
                    f"{target}:{work}/{name}"
                    for name in ("proof.bin", "public.bin", "vkey")
                ],
                str(out) + "/",
            ],
            check=True,
        )
        return backend
    finally:
        # This directory was allocated exclusively for this proof.
        subprocess.run([*ssh, target, shlex.join(["rm", "-rf", work])], check=False)


def guest_prefix(cfg, kind):
    return kind if kind == "deposit" or kind + "_elf" in cfg else "recorder"


def native_journal(cfg, kind, job):
    prefix = guest_prefix(cfg, kind)
    host_kind = {"handoff": "epoch-handoff", "keys": "key-table"}.get(prefix, kind)
    try:
        subprocess.run(
            [
                cfg["host"],
                "inspect",
                host_kind,
                job / "witness.bin",
                job / "native.bin",
            ],
            check=True,
        )
    except subprocess.CalledProcessError as error:
        raise Invalid("native verifier refused captured witness") from error
    return (job / "native.bin").read_bytes()


def prove(cfg, kind, job, native=None):
    """Prove the job's witness. `native`, when given, finishes the native check that runs
    alongside the proof request; the warm journal is compared only after it returns."""
    prefix = guest_prefix(cfg, kind)
    host_kind = {"handoff": "epoch-handoff", "keys": "key-table"}.get(prefix, kind)
    elf = cfg[prefix + "_elf"]
    with tempfile.TemporaryDirectory(prefix="sol-proof-") as tmp:
        out = Path(tmp)
        backend = cfg.get("prover", "auto")
        if backend == "warm":
            script = Path(__file__).resolve().parents[1] / "pfusdc-sol-ingress-host/scripts/prove.py"
            response = out / "response.json"
            # A second route's handoff waits here so it cannot occupy the
            # warm service's other request slot while the handoff GPU is busy.
            gate = cfg.get("handoff_prover_lock") if kind == "handoff" else None
            with locked(Path(gate)) if gate else nullcontext():
                subprocess.run(
                    ["python3", script, cfg["prover_socket"],
                     {"deposit":"parallel","keys":"key_table","handoff":"handoff"}[kind],
                     "groth16", job/"witness.bin", response],
                    check=True,
                )
            report = json.loads(response.read_bytes())
            if report["witness_sha256"] != hashlib.sha256((job/"witness.bin").read_bytes()).hexdigest():
                raise Invalid("warm prover returned a different witness")
            public = bytes(report.pop("public_values"))
            if native is not None:
                native()
            if public != (job/"native.bin").read_bytes():
                raise Invalid("warm prover journal differs from native verification")
            (out/"proof.bin").write_bytes(bytes.fromhex(report.pop("proof_hex")))
            (out/"public.bin").write_bytes(public)
            (out/"vkey").write_text(report["program_vkey"])
            save(job/"prover-timing.json",report)
        elif backend not in ("cuda", "cpu", "auto"):
            raise Invalid("unsupported prover")
        elif backend != "cpu":
            try:
                backend = remote_prove(cfg, elf, job / "witness.bin", host_kind, out)
            except (OSError, subprocess.CalledProcessError):
                if backend != "auto":
                    raise
                backend = "cpu"
        if backend == "cpu":
            subprocess.run(
                [
                    cfg["host"],
                    "prove",
                    elf,
                    job / "witness.bin",
                    "groth16",
                    out,
                    host_kind,
                ],
                check=True,
                env={**os.environ, "SP1_PROVER": "cpu"},
            )
        pin = cfg[prefix + "_vkey"]
        if (out / "vkey").read_text().strip() != pin:
            raise Invalid("guest verifying key differs from configured pin")
        verify(cfg, kind, out)
        for name in ("public.bin", "proof.bin"):
            save(job / name, (out / name).read_bytes())
        return backend


def verify(cfg, kind, directory):
    pin = cfg[guest_prefix(cfg, kind) + "_vkey"]
    subprocess.run(
        [
            cfg["host"],
            "verify-groth16",
            pin,
            directory / "proof.bin",
            directory / "public.bin",
        ],
        check=True,
    )
