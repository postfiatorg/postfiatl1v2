#!/usr/bin/env python3
"""Fetch the frozen testnet rounds 12-19 for the sub-scorer shadow evaluation.

Drives the fork's own replay harness (``scripts/replay_model_candidate.py``
in the read-only ``dynamic-unl-scoring`` clone) ``fetch`` path for each
round, then additionally fetches ``outputs/validator_scores.json`` from the
same route family (the harness's fetch list predates that artifact). The
fetched data lands in ``rounds/`` (gitignored); a committed
``rounds-manifest.json`` records the per-file SHA-256 of everything fetched
so the exact inputs of the evaluation are pinned without redistributing the
round artifacts.

Usage (from this directory, venv with the fork's parser deps):

    .venv/bin/python fetch_rounds.py            # rounds 12-19
    .venv/bin/python fetch_rounds.py --rounds 12 13

Rounds 9-11 predate the frozen-input-package contract on testnet and are
not replayable; do not extend the range downward.
"""

from __future__ import annotations

import argparse
import hashlib
import importlib
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
FORK_ROOT = Path.home() / "repos" / "dynamic-unl-scoring"
NETWORK = "testnet"
DEFAULT_ROUNDS = tuple(range(12, 20))
EXTRA_OUTPUT_FILES = ("outputs/validator_scores.json",)
MANIFEST_PATH = HERE / "rounds-manifest.json"
ROUNDS_DIR = HERE / "rounds"


def _load_harness():
    """Import the fork's replay harness read-only, never modifying the clone."""
    for path in (FORK_ROOT / "scripts", FORK_ROOT):
        if str(path) not in sys.path:
            sys.path.insert(0, str(path))
    return importlib.import_module("replay_model_candidate")


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def fetch(rounds: list[int]) -> int:
    harness = _load_harness()
    manifest: dict = {
        "network": NETWORK,
        "service": harness.SERVICE_URLS[NETWORK],
        "harness": "dynamic-unl-scoring/scripts/replay_model_candidate.py fetch",
        "rounds": {},
        "failures": [],
    }
    exit_code = 0

    for round_number in rounds:
        print(f"fetching {NETWORK} round {round_number}...")
        status = harness.fetch_round(NETWORK, round_number, ROUNDS_DIR)
        round_dir = ROUNDS_DIR / f"{NETWORK}-r{round_number}"

        # The harness's fetch list stops at model_response/selected_unl;
        # validator_scores.json lives on the same outputs route.
        base = harness.SERVICE_URLS[NETWORK]
        for rel_path in EXTRA_OUTPUT_FILES:
            url = f"{base}/api/scoring/rounds/{round_number}/{rel_path}"
            payload = harness._fetch_json(url)
            if payload is None:
                print(f"  {rel_path}: MISSING")
                manifest["failures"].append(
                    {"round": round_number, "file": rel_path, "reason": "fetch failed"}
                )
                continue
            target = round_dir / rel_path
            target.parent.mkdir(parents=True, exist_ok=True)
            target.write_text(json.dumps(payload, indent=1))
            print(f"  {rel_path}: {target.stat().st_size} bytes")

        if status != 0:
            manifest["failures"].append(
                {"round": round_number, "file": None, "reason": "required files missing"}
            )
            exit_code = 1

        files = {}
        if round_dir.exists():
            for path in sorted(round_dir.rglob("*.json")):
                files[str(path.relative_to(round_dir))] = {
                    "sha256": _sha256(path),
                    "bytes": path.stat().st_size,
                }
        manifest["rounds"][str(round_number)] = files

    MANIFEST_PATH.write_text(json.dumps(manifest, indent=1) + "\n")
    print(f"wrote {MANIFEST_PATH.name} ({len(rounds)} rounds, "
          f"{len(manifest['failures'])} failures)")
    return exit_code


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--rounds", type=int, nargs="+", default=list(DEFAULT_ROUNDS),
        help="testnet round numbers to fetch (default: 12-19)",
    )
    args = parser.parse_args()
    return fetch(args.rounds)


if __name__ == "__main__":
    sys.exit(main())
