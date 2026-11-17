#!/usr/bin/env python3
"""Run ops/b23/submit_asset_op.py signed by a StakeHub pft wallet.

The wallet is unlocked from an owner-only passphrase FILE (never printed) with
StakeHub's own pft_wallet.wallets.signing_files, which writes the ML-DSA key to
a temporary directory for this one signing leg and deletes it afterwards.

Usage (StakeHub venv python):
  /home/postfiat/repos/StakeHub/.venv/bin/python ops/b23/with_pft_key.py \
      WALLET_LABEL PASSPHRASE_FILE TX_LABEL OPERATION_JSON_FILE
"""
from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, "/home/postfiat/repos/StakeHub")
from pft_wallet.wallets import signing_files  # noqa: E402

HERE = Path(__file__).resolve().parent


def main() -> int:
    wallet, pass_file, label, op_file = sys.argv[1:5]
    path = Path(pass_file)
    if path.stat().st_mode & 0o077:
        raise SystemExit("passphrase file must be owner-only")
    passphrase = path.read_text(encoding="utf-8").rstrip("\r\n")
    with signing_files(wallet, passphrase) as (public, key_file, _backup):
        env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
        done = subprocess.run(["python3", str(HERE / "submit_asset_op.py"), label,
                               public.ce22_address, str(key_file), op_file], env=env, check=False)
    return done.returncode


if __name__ == "__main__":
    sys.exit(main())
