#!/usr/bin/env python3
"""Offline regression for the repository's node command wrappers."""

from pathlib import Path
import os
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parent.parent
WRAPPERS = {
    "node-init": ["init", "--data-dir", "fixture-data", "--chain-id", "fixture-chain", "--node-id", "fixture-node"],
    "node-run": ["run", "--unsafe-devnet-json-storage", "--data-dir", "fixture-data"],
    "node-status": ["status", "--data-dir", "fixture-data"],
    "node-account": ["account", "--data-dir", "fixture-data", "--address", "fixture-address"],
    "node-faucet": ["faucet", "--data-dir", "fixture-data"],
    "node-transfer": ["transfer", "--data-dir", "fixture-data", "--to", "fixture-to", "--amount", "7"],
}


def main() -> None:
    errors: list[str] = []
    with tempfile.TemporaryDirectory() as temp:
        temp_path = Path(temp)
        cargo = temp_path / "cargo"
        log = temp_path / "cargo.log"
        cargo.write_text('#!/usr/bin/env bash\nprintf "%s\\0" "$@" >"$CARGO_LOG"\n')
        cargo.chmod(0o755)
        env = os.environ | {
            "PATH": f"{temp}{os.pathsep}{os.environ['PATH']}",
            "CARGO_LOG": str(log),
            "DATA_DIR": "fixture-data",
            "CHAIN_ID": "fixture-chain",
            "NODE_ID": "fixture-node",
            "ADDRESS": "fixture-address",
            "TO": "fixture-to",
            "AMOUNT": "7",
        }
        prefix = ["run", "-p", "postfiat-node", "--bin", "postfiat-node", "--"]
        for wrapper, expected in WRAPPERS.items():
            subprocess.run([str(ROOT / "scripts" / wrapper)], cwd=ROOT, env=env, check=True)
            actual = log.read_bytes().split(b"\0")[:-1]
            wanted = [part.encode() for part in prefix + expected]
            if actual != wanted:
                errors.append(f"{wrapper}: {actual!r} != {wanted!r}")

    harness = (ROOT / "scripts/testnet-local-harness").read_text()
    expected_run = '"$POSTFIAT_NODE_BIN" run \\\n    --unsafe-devnet-json-storage \\\n    --data-dir "$node_dir"'
    if expected_run not in harness:
        errors.append("local harness run must acknowledge JSON storage")
    assert not errors, "\n".join(errors)


if __name__ == "__main__":
    main()
