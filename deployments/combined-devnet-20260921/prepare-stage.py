#!/usr/bin/env python3
"""Regenerate the canonical release stage locally; never contacts validators."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

PACKET = Path(__file__).resolve().parent
RELEASE = "combined-devnet-20260921"
BINARY_SHA256 = "051ad12ce22c33f2370388259d754a9c1016a8edc1b52db7d2852fe78f6fbc7d"

def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--trusted-publisher-public-file", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    stage = args.output_dir.resolve()
    config = PACKET / "rootfs/etc/postfiat/releases" / RELEASE
    unsigned = json.loads((PACKET / "manifest-input.unsigned.json").read_text())
    if sha(binary) != BINARY_SHA256:
        raise SystemExit("qualified binary SHA-256 mismatch")
    if sha(args.trusted_publisher_public_file) != unsigned["trusted_publisher_file_sha256"]:
        raise SystemExit("existing deployment trust file SHA-256 mismatch")
    stage.parent.mkdir(parents=True, exist_ok=True)
    command = [
        str(binary), "deployment-validator-units-stage", "--release-id", RELEASE,
        "--topology-file", str(config / "topology.json"), "--binary-file", str(binary),
        "--swap-circuit-metadata-file", str(config / "swap.metadata.json"),
        "--private-egress-circuit-metadata-file", str(config / "private-egress.metadata.json"),
        "--output-dir", str(stage),
    ]
    subprocess.run(command, check=True, stdout=subprocess.DEVNULL)
    for original in sorted((PACKET / "rootfs").rglob("*")):
        if original.is_file():
            generated = stage / "rootfs" / original.relative_to(PACKET / "rootfs")
            if original.read_bytes() != generated.read_bytes():
                raise SystemExit(f"generator differs from reviewed input: {original}")
    shutil.copy2(args.trusted_publisher_public_file,
                 stage / "rootfs/etc/postfiat/releases" / RELEASE / "deployment.public.json")
    shutil.copytree(PACKET / "operator", stage / "operator")
    print(json.dumps({"result": "PASS", "stage_report": str(stage / "stage-report.json"),
                      "signed": False, "binary_sha256": sha(binary)}, indent=2))

if __name__ == "__main__":
    main()
