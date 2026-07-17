#!/usr/bin/env python3
"""Run the PostFiat PFTL transfer CLI from a source checkout."""

from __future__ import annotations

import runpy
import sys
from pathlib import Path


REPO_ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(REPO_ROOT / "python"))

runpy.run_module("postfiat_rpc.pftl_transfer", run_name="__main__")
