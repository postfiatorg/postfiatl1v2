#!/usr/bin/env python3
"""Verify the frozen Cobalt E1 oracle-comparison evidence packet."""

from __future__ import annotations

import gzip
import hashlib
import json
from pathlib import Path
from typing import BinaryIO

PACKET = Path(__file__).resolve().parent
CASE_COUNT = 10_240
CORPUS_SHA256 = "42ed266ba207136eec560f8be14c904c2e63ffe305e188860e4ff04731cd5fd2"
INITIAL_CLASSIFICATIONS_SHA256 = (
    "90eee779ec246901c16d66b6391079b90568430198c9a68dcd447eefd2d5b368"
)
INITIAL_DISAGREEMENTS_SHA256 = (
    "51e76c1fe477a1525b488644dff3eba34c077515dfa794455bc89328b0d1dbbf"
)
RECONCILED_CLASSIFICATIONS_SHA256 = (
    "66ed6e8b2f7fd33927448b5b2e866ae4275263840128cbf1842d7460f3ca19cd"
)
PACKET_FILES = {
    "README.md",
    "clean-rerun/disagreements.json",
    "clean-rerun/review.md",
    "clean-rerun/summary.json",
    "corpus-manifest.json",
    "initial/classifications.jsonl.gz",
    "initial/disagreements.json.gz",
    "initial/mismatch-review.md",
    "initial/summary.json",
    "reconciled/classifications.jsonl.gz",
    "reconciled/disagreements.json",
    "reconciled/review.md",
    "reconciled/summary.json",
    "verify_packet.py",
}


def digest_stream(stream: BinaryIO) -> str:
    value = hashlib.sha256()
    while chunk := stream.read(1024 * 1024):
        value.update(chunk)
    return value.hexdigest()


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return digest_stream(stream)


def gzip_digest(path: Path) -> str:
    with gzip.open(path, "rb") as stream:
        return digest_stream(stream)


def object_file(path: Path) -> dict:
    assert path.is_file() and not path.is_symlink(), path
    value = json.loads(path.read_text(encoding="utf-8"))
    assert isinstance(value, dict), path
    return value


def json_file(path: Path) -> object:
    assert path.is_file() and not path.is_symlink(), path
    return json.loads(path.read_text(encoding="utf-8"))


def classification_ids(path: Path) -> set[str]:
    observed: set[str] = set()
    with gzip.open(path, "rt", encoding="utf-8") as stream:
        for line_number, line in enumerate(stream, start=1):
            row = json.loads(line)
            assert isinstance(row, dict), (path, line_number)
            case_id = row.get("case_id")
            assert isinstance(case_id, str), (path, line_number)
            assert case_id not in observed, case_id
            observed.add(case_id)
    return observed


checksum_lines = (PACKET / "SHA256SUMS.txt").read_text(encoding="ascii").splitlines()
assert checksum_lines
listed: set[str] = set()
for line in checksum_lines:
    expected, separator, name = line.partition("  ")
    path = PACKET / name
    assert separator == "  ", line
    assert name not in listed, name
    assert name in PACKET_FILES, name
    assert path.is_file() and not path.is_symlink(), path
    assert digest(path) == expected, name
    listed.add(name)
assert listed == PACKET_FILES

manifest = object_file(PACKET / "corpus-manifest.json")
initial = object_file(PACKET / "initial/summary.json")
reconciled = object_file(PACKET / "reconciled/summary.json")
clean = object_file(PACKET / "clean-rerun/summary.json")

assert manifest["schema"] == "postfiat-cobalt-adversarial-e1-corpus-v1"
assert manifest["generator_version"] == "cobalt-adversarial-graph-generator-v1"
assert manifest["oracle_version"] == "cobalt-independent-essential-subset-oracle-v2"
assert manifest["case_count"] == CASE_COUNT
assert manifest["validator_count_min"] == 6
assert manifest["validator_count_max"] == 20
assert manifest["corpus_sha256"] == CORPUS_SHA256
assert len(manifest["boundary_case_counts"]) == 14
assert all(value > 0 for value in manifest["boundary_case_counts"].values())

common = {
    "schema": "postfiat-cobalt-adversarial-e1-comparison-v1",
    "corpus_sha256": CORPUS_SHA256,
    "case_count": CASE_COUNT,
    "valid_case_count": 8_534,
    "invalid_boundary_case_count": 1_706,
    "compatible_case_count": 3_889,
    "incompatible_case_count": 4_645,
    "second_oracle": "postfiat-cobalt-adversarial-oracle (independent v2)",
    "first_oracle": "postfiat-cobalt-decision-oracle (activation v1)",
    "production_routes": [
        "analyze_trust_graph",
        "has_strong_support",
        "verify_nonuniform_governance_certificate",
    ],
}
for summary in (initial, reconciled, clean):
    for key, value in common.items():
        assert summary[key] == value, key

assert initial["disagreement_count"] == 8_534
assert initial["classification_sha256"] == INITIAL_CLASSIFICATIONS_SHA256
assert initial["summary_only"] is False
assert initial["pass"] is False

assert reconciled["disagreement_count"] == 0
assert reconciled["classification_sha256"] == RECONCILED_CLASSIFICATIONS_SHA256
assert reconciled["summary_only"] is False
assert reconciled["pass"] is True

assert clean["disagreement_count"] == 0
assert clean["classification_sha256"] == RECONCILED_CLASSIFICATIONS_SHA256
assert clean["summary_only"] is True
assert clean["pass"] is True

assert gzip_digest(PACKET / "initial/classifications.jsonl.gz") == INITIAL_CLASSIFICATIONS_SHA256
assert gzip_digest(PACKET / "initial/disagreements.json.gz") == INITIAL_DISAGREEMENTS_SHA256
assert (
    gzip_digest(PACKET / "reconciled/classifications.jsonl.gz")
    == RECONCILED_CLASSIFICATIONS_SHA256
)

initial_ids = classification_ids(PACKET / "initial/classifications.jsonl.gz")
reconciled_ids = classification_ids(PACKET / "reconciled/classifications.jsonl.gz")
assert len(initial_ids) == CASE_COUNT
assert initial_ids == reconciled_ids

with gzip.open(PACKET / "initial/disagreements.json.gz", "rt", encoding="utf-8") as stream:
    initial_disagreements = json.load(stream)
assert isinstance(initial_disagreements, list)
assert len(initial_disagreements) == initial["disagreement_count"]
assert json_file(PACKET / "reconciled/disagreements.json") == []
assert json_file(PACKET / "clean-rerun/disagreements.json") == []

scan = b"\n".join(
    (PACKET / name).read_bytes().lower()
    for name in PACKET_FILES
    if not name.endswith(".gz") and name != "verify_packet.py"
)
for forbidden in (
    b'"private_key"',
    b'"secret_key"',
    b'"api_key"',
    b'"seed_hex"',
    b'"signature_hex"',
):
    assert forbidden not in scan, forbidden

print("e1-packet-ok")
print(f"sha256sums_sha256={digest(PACKET / 'SHA256SUMS.txt')}")
