#!/usr/bin/env python3
"""Z3 offline command sheet and one-confirmation-at-a-time composition CLI."""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "python"))
from postfiat_rpc.z3_composition import main

if __name__ == "__main__":
    raise SystemExit(main())
