#!/usr/bin/env python3
"""Validate checkpoint vote fanout output and emit assembler input."""

from __future__ import annotations

import argparse
import json
from pathlib import Path, PurePosixPath
import sys
from typing import Any


EXPECTED_VALIDATORS = 6
EXPECTED_SCHEMA = "postfiat-a666-parallel-checkpoint-votes-v1"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--fanout-file", type=Path, required=True)
    return parser.parse_args()


def validated_vote_files(document: Any) -> list[str]:
    if not isinstance(document, dict):
        raise ValueError("fanout document must be an object")
    if document.get("schema") != EXPECTED_SCHEMA:
        raise ValueError("unexpected checkpoint vote fanout schema")
    if document.get("validator_count") != EXPECTED_VALIDATORS:
        raise ValueError("checkpoint vote fanout must contain exactly six validators")

    vote_files = document.get("remote_vote_files")
    if not isinstance(vote_files, list) or len(vote_files) != EXPECTED_VALIDATORS:
        raise ValueError("remote_vote_files must contain exactly six paths")
    if any(not isinstance(path, str) or not path for path in vote_files):
        raise ValueError("every remote vote file must be a non-empty string")
    if len(set(vote_files)) != EXPECTED_VALIDATORS:
        raise ValueError("remote vote file paths must be unique")
    if any(not PurePosixPath(path).is_absolute() for path in vote_files):
        raise ValueError("every remote vote file must be an absolute path")

    expected_csv = ",".join(vote_files)
    if document.get("remote_vote_files_csv") != expected_csv:
        raise ValueError("remote_vote_files_csv does not match remote_vote_files")
    return vote_files


def main() -> None:
    args = parse_args()
    try:
        document = json.loads(args.fanout_file.read_text())
        print(",".join(validated_vote_files(document)))
    except (OSError, json.JSONDecodeError, ValueError) as error:
        print(f"a666-checkpoint-vote-files: {error}", file=sys.stderr)
        raise SystemExit(1) from error


if __name__ == "__main__":
    main()
