"""Retired qualification evidence stays reproducible and fail-closed."""

import json
import runpy
import subprocess
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture
def checker():
    module = runpy.run_path(str(ROOT / "scripts/check-a666-public-source-qualification"))
    return module["main"].__globals__


def test_retained_packet_passes_with_explicit_historical_boundary(checker, capsys):
    checker["main"]()
    output = capsys.readouterr().out
    assert "sources=6 epochs=2 inputs=12 proofs=2" in output
    assert "evidence=git-archive:" + checker["ARCHIVE_COMMIT"] in output


def test_missing_archive_commit_fails_closed(checker, monkeypatch):
    monkeypatch.setitem(checker, "ARCHIVE_COMMIT", "0" * 40)
    with pytest.raises(SystemExit, match="pinned archive object unavailable"):
        checker["main"]()


def test_partial_working_tree_packet_does_not_fall_back(checker, monkeypatch, tmp_path):
    evidence = tmp_path / "evidence"
    evidence.mkdir()
    monkeypatch.setitem(checker, "EVIDENCE", evidence)
    monkeypatch.setitem(checker, "ROOT", tmp_path)
    with pytest.raises(SystemExit, match="cannot read"):
        checker["load"](evidence / "source-qualification-draft.json")


def test_corrupt_working_tree_record_does_not_fall_back(checker, monkeypatch, tmp_path):
    evidence = tmp_path / "evidence"
    evidence.mkdir()
    source = evidence / "source-qualification-draft.json"
    source.write_text("not JSON", encoding="utf-8")
    monkeypatch.setitem(checker, "EVIDENCE", evidence)
    monkeypatch.setitem(checker, "ROOT", tmp_path)
    with pytest.raises(SystemExit, match="cannot read"):
        checker["load"](source)


def test_archive_parent_traversal_is_rejected(checker):
    with pytest.raises(SystemExit, match="archive path must remain inside"):
        checker["read_bytes"](checker["EVIDENCE"] / ".." / "outside.json")


@pytest.mark.parametrize("mutation, expected", [
    ("input_hash", "input SHA-256 mismatch"),
    ("proof_hash", "proof.bin SHA-256 mismatch"),
    ("proof_size", "proof.bin size mismatch"),
    ("proof_verdict", "epoch 7 proof mismatch"),
    ("duplicate_artifact", "exact duplicate-free set"),
    ("artifact_traversal", "exact duplicate-free set"),
    ("public_values", "aggregate public values mismatch"),
    ("incomplete_acceptance", "aggregate acceptance is incomplete"),
])
def test_packet_tampering_still_fails(checker, monkeypatch, mutation, expected):
    original = checker["read_bytes"]

    def tampered(path):
        raw = original(path)
        if path == checker["SOURCE_RECORD"] and mutation == "input_hash":
            value = json.loads(raw)
            value["sources"][0]["epochs"][0]["input_sha256"] = "00" * 32
        elif path == checker["AGGREGATE_RECORD"] and mutation in {
            "public_values", "incomplete_acceptance",
        }:
            value = json.loads(raw)
            if mutation == "public_values":
                value["epochs"][0]["public_values_sha256"] = "00" * 32
            else:
                value["acceptance"]["independent_verification_passed"] = False
        elif path == checker["EVIDENCE"] / "epoch-7/verification.json":
            value = json.loads(raw)
            artifact = next(a for a in value["artifacts"] if a["path"] == "proof.bin")
            if mutation == "proof_hash":
                artifact["sha256"] = "00" * 32
            elif mutation == "proof_size":
                artifact["size_bytes"] += 1
            elif mutation == "proof_verdict":
                value["verification"]["valid"] = False
            elif mutation == "duplicate_artifact":
                value["artifacts"][0] = dict(value["artifacts"][1])
            elif mutation == "artifact_traversal":
                artifact["path"] = "../proof.bin"
        else:
            return raw
        return json.dumps(value).encode()

    monkeypatch.setitem(checker, "read_bytes", tampered)
    with pytest.raises(SystemExit, match=expected):
        checker["main"]()


def test_restored_readiness_document_preserves_open_gates():
    result = subprocess.run(
        [str(ROOT / "scripts/check-a666-public-adapter-readiness")],
        cwd=ROOT, capture_output=True, text=True, check=True,
    )
    assert "qualified=0/6 stakehub_deprecated=false" in result.stdout
