#!/usr/bin/env python3
"""Regression tests for checkpoint certificate vote-file extraction."""

from __future__ import annotations

import json
from pathlib import Path
import subprocess
import tempfile
import unittest


REPO = Path(__file__).resolve().parent.parent
PROGRAM = REPO / "scripts" / "a666-checkpoint-vote-files.py"


def valid_document() -> dict[str, object]:
    vote_files = [
        f"/var/lib/postfiat/validator-2/workflow/validator-{index}.vote.json"
        for index in range(6)
    ]
    return {
        "schema": "postfiat-a666-parallel-checkpoint-votes-v1",
        "validator_count": 6,
        "remote_vote_files": vote_files,
        "remote_vote_files_csv": ",".join(vote_files),
    }


class VoteFileExtractionTests(unittest.TestCase):
    def run_program(self, document: object) -> subprocess.CompletedProcess[str]:
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "fanout.json"
            path.write_text(json.dumps(document))
            return subprocess.run(
                ["python3", str(PROGRAM), "--fanout-file", str(path)],
                check=False,
                capture_output=True,
                text=True,
            )

    def test_emits_exact_six_path_csv(self) -> None:
        document = valid_document()
        result = self.run_program(document)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(
            result.stdout.strip(),
            document["remote_vote_files_csv"],
        )
        self.assertNotEqual(result.stdout.strip(), "true")

    def test_rejects_boolean_csv_from_jq_and_bug(self) -> None:
        document = valid_document()
        document["remote_vote_files_csv"] = True
        result = self.run_program(document)
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_fewer_than_six_votes(self) -> None:
        document = valid_document()
        document["validator_count"] = 5
        result = self.run_program(document)
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_duplicate_vote_paths(self) -> None:
        document = valid_document()
        vote_files = document["remote_vote_files"]
        assert isinstance(vote_files, list)
        vote_files[-1] = vote_files[0]
        document["remote_vote_files_csv"] = ",".join(vote_files)
        result = self.run_program(document)
        self.assertNotEqual(result.returncode, 0)

    def test_rejects_relative_vote_path(self) -> None:
        document = valid_document()
        vote_files = document["remote_vote_files"]
        assert isinstance(vote_files, list)
        vote_files[-1] = "validator-5.vote.json"
        document["remote_vote_files_csv"] = ",".join(vote_files)
        result = self.run_program(document)
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    unittest.main()
