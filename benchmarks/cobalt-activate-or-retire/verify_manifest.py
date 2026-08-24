#!/usr/bin/env python3
"""Verify the frozen Cobalt activate-or-retire decision manifest."""

from __future__ import annotations

import hashlib
import importlib.util
import json
from pathlib import Path
from typing import Any

HERE = Path(__file__).resolve().parent
ROOT = HERE.parent.parent
MANIFEST = HERE / "scenario-manifest.json"
EXPECTED_CASES = 18
EXPECTED_COMPATIBLE = 13
EXPECTED_INCOMPATIBLE = 5


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def file_sha256(path: Path) -> str:
    return sha256(path.read_bytes())


def canonical_bytes(value: Any) -> bytes:
    return json.dumps(value, ensure_ascii=False, separators=(",", ":")).encode()


def load_generator() -> Any:
    path = HERE / "generate_inputs.py"
    spec = importlib.util.spec_from_file_location("cobalt_decisive_generator", path)
    if spec is None or spec.loader is None:
        raise RuntimeError("cannot load decisive input generator")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main() -> int:
    raw = MANIFEST.read_bytes()
    manifest = json.loads(raw)
    assert manifest["schema"] == "postfiat-cobalt-decisive-manifest-v1"

    declared_manifest = manifest["manifest_sha256"]
    canonical = dict(manifest)
    canonical["manifest_sha256"] = ""
    assert sha256(canonical_bytes(canonical)) == declared_manifest

    generator = load_generator()
    generated_input = {
        "schema": generator.SCHEMA,
        "source_pins": manifest["source_pins"],
        "cases": generator.build_cases(),
    }
    generated_bytes = (
        json.dumps(generated_input, indent=2, sort_keys=True) + "\n"
    ).encode()
    assert sha256(generated_bytes) == manifest["input_sha256"]

    assert file_sha256(ROOT / "crates/cobalt_decision_oracle/src/lib.rs") == manifest[
        "oracle"
    ]["source_sha256"]
    assert file_sha256(HERE / "oracle-contract.md") == manifest["oracle"][
        "contract_sha256"
    ]
    adapters = {
        "cobalt": ROOT
        / "crates/node/src/bin/postfiat_cobalt_decisive_benchmark.rs",
        "rippled": HERE / "rippled/DecisiveGovernanceBenchmark_test.cpp",
    }
    assert {
        label: file_sha256(path) for label, path in sorted(adapters.items())
    } == manifest["adapter_sha256"]

    cases = manifest["cases"]
    assert len(cases) == EXPECTED_CASES
    classifications = [case["expected"]["classification"] for case in cases]
    assert classifications.count("compatible") == EXPECTED_COMPATIBLE
    assert classifications.count("incompatible") == EXPECTED_INCOMPATIBLE
    assert b"characterize" not in raw
    for case in cases:
        correct = sorted(case["correct_nodes"])
        assert sorted(case["expected"]["cobalt_nodes"]) == correct
        assert sorted(case["expected"]["rippled_nodes"]) == correct
    assert [
        case["id"]
        for case in cases
        if case["expected"]["material_safety_delta"]
    ] == ["six-divergent-local-quorums"]

    print(
        json.dumps(
            {
                "status": "manifest-ok",
                "cases": len(cases),
                "compatible": EXPECTED_COMPATIBLE,
                "incompatible": EXPECTED_INCOMPATIBLE,
                "canonical_manifest_id": declared_manifest,
                "raw_manifest_sha256": sha256(raw),
            },
            sort_keys=True,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
