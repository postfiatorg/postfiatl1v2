#!/usr/bin/env python3
"""Fail-closed remote ingress proof leaf; no application imports."""
from __future__ import annotations
import argparse
import hashlib
import json
import shlex
import subprocess
import sys
from pathlib import Path


# Profile-pinned epoch-5 vault-claim verifier and committed ingress ELF lineage.
EXPECTED_PROGRAM_VKEY = "0x00a9f8f037da18dd1aa5a7b0f478df0c7c9fae411ee62b339baf48dc2505076e"
EXPECTED_ELF_SHA256 = "0e59a0cf7723b9028aaa4c57f9e9c0da72119a552d62a5577223ba7b2df222d3"


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for block in iter(lambda: fh.read(1024 * 1024), b""):
            h.update(block)
    return h.hexdigest()


def run(argv: list[str]) -> None:
    subprocess.run(argv, check=True)


def _validate_report_pins(report: dict) -> None:
    if report.get("program_vkey") != EXPECTED_PROGRAM_VKEY:
        raise RuntimeError("proof report program_vkey mismatch")
    if report.get("elf_sha256") != EXPECTED_ELF_SHA256:
        raise RuntimeError("proof report elf_sha256 mismatch")


def verify_existing(out: Path) -> dict:
    report_path = out / "proof-report.json"
    if not report_path.exists():
        raise RuntimeError("existing proof output lacks proof-report.json")
    report = json.loads(report_path.read_text())
    _validate_report_pins(report)
    for name in ("proof-calldata.bin", "public-values.bin"):
        path = out / name
        expected = report.get(name.replace(".bin", "_sha256")) or report.get(name + "_sha256")
        if not path.exists() or not expected or digest(path) != expected:
            raise RuntimeError(f"existing artifact failed hash verification: {name}")
    return report


def prove(args: argparse.Namespace) -> dict:
    out = Path(args.artifact_dir)
    reports = sorted(out.glob("**/evm-deposit.json"))
    if not reports:
        raise ValueError("artifact-dir has no evm-deposit.json")
    try:
        deposit_report = json.loads(reports[0].read_text())
    except (OSError, json.JSONDecodeError) as exc:
        raise ValueError("EVM deposit report malformed") from exc
    deposit_tx = deposit_report.get("deposit_tx")
    if not isinstance(deposit_tx, str) or not deposit_tx:
        raise ValueError("EVM deposit report missing deposit_tx")
    args.deposit_tx = deposit_tx
    out.mkdir(parents=True, exist_ok=True)
    out.mkdir(parents=True, exist_ok=True)
    if (out / "proof-report.json").exists():
        return verify_existing(out)
    remote = f"{args.prover_host}:{args.remote_workdir}"
    ssh = ["ssh"]
    witness = Path(args.witness) if args.witness else None
    remote_workdir = args.remote_workdir.rstrip("/")
    remote_witness = f"{remote_workdir}/witness.json"
    remote_binary = f"{remote_workdir}/tools/eth-l1-mainnet-fast-lane-p0/target/release/eth-l1-mainnet-fast-lane-p0"
    scp = ["scp"]
    if args.ssh_key:
        ssh += ["-i", args.ssh_key]
        scp += ["-i", args.ssh_key]
    if witness is not None:
        if not witness.exists():
            raise ValueError(f"witness missing: {witness}")
        run(scp + [str(witness), f"{args.prover_host}:{remote_witness}"])
    else:
        capture = (
            "SP1_PROVER=cuda " + shlex.quote(remote_binary)
            + " capture --deployment " + shlex.quote(f"{remote_workdir}/deployment.json")
            + " --output " + shlex.quote(remote_witness)
            + " --execution-rpc " + shlex.quote(args.execution_rpc)
            + " --beacon-rpc " + shlex.quote(args.beacon_rpc)
        )
        run(ssh + [args.prover_host, capture])
    command = (
        "SP1_PROVER=cuda " + shlex.quote(remote_binary)
        + " prove --witness " + shlex.quote(remote_witness)
        + " --output-dir " + shlex.quote(remote_workdir)
        + " --require-prover cuda"
    )
    run(ssh + [args.prover_host, command])
    run(scp + [f"{remote}/proof-calldata.bin", str(out / "proof-calldata.bin")])
    run(scp + [f"{remote}/public-values.bin", str(out / "public-values.bin")])
    run(scp + [f"{remote}/proof-report.json", str(out / "remote-proof-report.json")])
    report = json.loads((out / "remote-proof-report.json").read_text())
    _validate_report_pins(report)
    report["proof-calldata_sha256"] = digest(out / "proof-calldata.bin")
    report["public-values_sha256"] = digest(out / "public-values.bin")
    report["deposit_tx"] = args.deposit_tx
    (out / "proof-report.json").write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser()
    p.add_argument("--deposit-tx")
    p.add_argument("--source-rpc-url", action="append", required=True)
    p.add_argument("--prover-host", required=True)
    p.add_argument("--remote-workdir", required=True)
    p.add_argument("--artifact-dir", required=True)
    p.add_argument("--witness")
    p.add_argument("--execution-rpc", required=True)
    p.add_argument("--beacon-rpc", required=True)
    p.add_argument("--ssh-key")
    args = p.parse_args(argv)
    try:
        report = prove(args)
    except (OSError, subprocess.CalledProcessError, ValueError, RuntimeError, json.JSONDecodeError) as exc:
        print(f"STOP-no-retry: {exc}", file=sys.stderr)
        return 2
    print(json.dumps(report, sort_keys=True))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
