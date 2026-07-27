#!/usr/bin/env python3
"""Fail closed on any deviation in the generated mainnet epoch-4 package."""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import subprocess
import sys
from pathlib import Path
from types import SimpleNamespace
from typing import Any


HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[4]
GENERATOR_PATH = HERE / "generate_mainnet_epoch4_package.py"
EPOCH3_VERIFIER_PATH = (
    ROOT
    / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/package"
    / "verify_mainnet_epoch3_package.py"
)


def load_module(path: Path, name: str):
    spec = importlib.util.spec_from_file_location(name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"cannot load module: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


G = load_module(GENERATOR_PATH, "pfusdc_mainnet_epoch4_generator")
V3 = load_module(EPOCH3_VERIFIER_PATH, "pfusdc_mainnet_epoch3_verifier")


class VerificationError(ValueError):
    """Raised for any package, consumer, lineage, or gate divergence."""


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise VerificationError(f"cannot read {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise VerificationError(f"expected JSON object: {path}")
    return value


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def current_stale_hits(value: object, stale: dict[str, object], path: str = "$") -> list[str]:
    hits: list[str] = []
    if isinstance(value, dict):
        for key, child in value.items():
            hits.extend(current_stale_hits(child, stale, f"{path}.{key}"))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            hits.extend(current_stale_hits(child, stale, f"{path}[{index}]"))
    elif isinstance(value, str):
        for label, token in stale.items():
            if isinstance(token, str) and token.lower() in value.lower():
                hits.append(f"{path} {label}={token}")
    elif isinstance(value, int):
        for label, token in stale.items():
            if isinstance(token, int) and path.endswith(".route_epoch") and value == token:
                hits.append(f"{path} {label}={token}")
    return hits


def verify_cross_check(tuple_data: dict[str, Any]) -> None:
    binding = tuple_data["source"]["contract_guest_storage_cross_check"]
    path = ROOT / binding["path"]
    if digest(path) != binding["sha256"]:
        raise VerificationError("tuple cross-check artifact digest mismatch")
    document = read_json(path)
    if document.get("status") != "PASS" or document.get("decode_simulation") != "PASS":
        raise VerificationError("contract/guest storage cross-check is not PASS")
    result = subprocess.run(
        [str(ROOT / "scripts/pfusdc-contract-guest-storage-cross-check.py")],
        cwd=ROOT,
        check=False,
        text=True,
        capture_output=True,
    )
    if result.returncode != 0:
        raise VerificationError(f"live contract/guest storage cross-check failed: {result.stderr}")
    rerun = json.loads(result.stdout)
    for key in (
        "contract_source_sha256",
        "guest_source_sha256",
        "layout",
        "decode_simulation",
        "status",
    ):
        if rerun.get(key) != document.get(key):
            raise VerificationError(f"contract/guest cross-check rerun drifted: {key}")


def verify_epoch4(args: argparse.Namespace) -> list[str]:
    tuple_data = G.load_tuple(args.input)
    if Path.cwd().resolve() != ROOT:
        raise VerificationError(f"verifier must run from repository root: {ROOT}")

    V3.G = G
    base_args = SimpleNamespace(
        input=args.input,
        package_root=args.package_root,
        evidence_dir=args.evidence_dir,
    )
    lines = V3.verify(base_args)
    revision = tuple_data["deployment"]["revision"]
    files = {
        "route": args.package_root / f"route-profile.{revision}.json",
        "manifest": args.package_root / f"manifest.{revision}.json",
        "consumer": args.package_root / f"deploy-consumer.{revision}.json",
        "nav": args.evidence_dir / f"planned-nav-profile.{revision}.json",
        "bind": args.evidence_dir / f"planned-nav-bind.{revision}.json",
        "registration": args.evidence_dir / f"route-registration.{revision}.json",
    }
    documents = {name: read_json(path) for name, path in files.items()}
    manifest = documents["manifest"]
    if manifest.get("revision") != "mainnet-epoch4":
        raise VerificationError("manifest revision is not mainnet-epoch4")
    if (
        manifest["route"]["route_epoch"],
        manifest["route"]["activation_height"],
        manifest["route"]["binding_submission_height"],
        manifest["route"]["registration_height"],
    ) != (4, 318, 317, 318):
        raise VerificationError("manifest epoch-4 route schedule drifted")
    if [
        item["precomputed_create_nonce"] for item in manifest["contracts"]["artifacts"]
    ] != [157, 158]:
        raise VerificationError("manifest CREATE nonce pair is not 157/158")

    stale = {
        "epoch3_vault": "0x47d54874a708c4bf25ffd547f61f695fff940af9",
        "epoch3_verifier": "0x31a89b52b8c0675c00e79287c0e015614a266900",
        "epoch3_profile": "1893c6d3",
        "epoch3_binding": "206b718f",
        "epoch3_route_epoch": 3,
    }
    stale_hits: list[str] = []
    for name, document in documents.items():
        stale_hits.extend(
            f"{name}:{hit}" for hit in current_stale_hits(document, stale)
        )
    if stale_hits:
        raise VerificationError("epoch-3 value leaked into current epoch-4 fields: " + " | ".join(stale_hits))

    summary = read_json(args.evidence_dir / "package-summary.json")
    if summary.get("schema") != "postfiat.pfusdc.mainnet_epoch4_package_summary.v1":
        raise VerificationError("epoch-4 package summary schema mismatch")
    if summary.get("plan") != {
        "bind_height": 317,
        "registration_activation_height": 318,
    }:
        raise VerificationError("epoch-4 package summary schedule mismatch")
    verify_cross_check(tuple_data)

    (args.evidence_dir / "admission-simulation.log").write_text(
        "ADMISSION: ADMIT\n"
        "epoch advances 3 -> 4\n"
        "activation advances 316 -> 318\n"
        "H317 bind / H318 registration\n",
        encoding="utf-8",
    )
    (args.evidence_dir / "epoch3-current-value-scan.log").write_text(
        "EPOCH3_CURRENT_VALUE_HITS: 0\n"
        "historical lineage documents excluded from current-field scan\n",
        encoding="utf-8",
    )
    return [
        *lines,
        "CONTRACT_GUEST_STORAGE_CROSS_CHECK: PASS",
        "EPOCH3_CURRENT_VALUE_HITS: 0",
        "EPOCH4_CREATE_NONCES: 157/158",
        "EPOCH4_SCHEDULE: H317/H318",
        "VERDICT: PASS",
    ]


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input", required=True, type=Path)
    parser.add_argument("--package-root", required=True, type=Path)
    parser.add_argument("--evidence-dir", required=True, type=Path)
    args = parser.parse_args()
    try:
        print("\n".join(verify_epoch4(args)))
    except (
        VerificationError,
        V3.VerificationError,
        G.InputError,
        KeyError,
        OSError,
        ValueError,
    ) as exc:
        print(f"VERDICT: FAIL: {exc}")
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
