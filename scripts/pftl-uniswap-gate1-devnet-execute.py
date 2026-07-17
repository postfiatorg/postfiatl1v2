#!/usr/bin/env python3
"""Produce Gate 1 local-devnet evidence for the PFTL-Uniswap bridge packet path."""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[1]
NODE_BIN = ROOT / "target" / "debug" / "postfiat-node"
SOURCE_EVIDENCE = ROOT / "docs" / "evidence" / "pftl-uniswap-gate1-2026-07-01"
DEFAULT_OUT_DIR = ROOT / "docs" / "evidence" / "pftl-uniswap-gate1-devnet-2026-07-01"

ROUTE_ID = "pftl-a666-ethereum-wA666-usdc-v1"
EXPORT_PACKET_HASH = "aa" * 48
ETHEREUM_SENDER = "0x6666666666666666666666666666666666666666"
PFTL_RECIPIENT = "pf124071fd53a12ca4556b7aa1f5ec98b585e73468"
RETURN_NONCE = "ee" * 32
EXPECTED_RECEIPT_COUNT = 5
EXPECTED_STATUS = "verified"


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out-dir", default=str(DEFAULT_OUT_DIR), help="evidence output directory")
    parser.add_argument("--replace", action="store_true", help="replace an existing evidence directory")
    parser.add_argument("--validators", type=int, default=4, help="local devnet validator count")
    parser.add_argument("--keep-devnet", action="store_true", help="leave the temporary local devnet data dir on disk")
    return parser.parse_args()


def run(
    cmd: list[str],
    *,
    cwd: Path = ROOT,
    stdout_file: Path | None = None,
    env: dict[str, str] | None = None,
) -> str:
    result = subprocess.run(cmd, cwd=cwd, text=True, capture_output=True, check=False, env=env)
    if stdout_file is not None:
        stdout_file.parent.mkdir(parents=True, exist_ok=True)
        stdout_file.write_text(result.stdout, encoding="utf-8")
        if result.stderr:
            stdout_file.with_suffix(stdout_file.suffix + ".stderr").write_text(result.stderr, encoding="utf-8")
    if result.returncode != 0:
        raise RuntimeError(
            f"command failed ({result.returncode}): {' '.join(cmd)}\n{result.stdout}\n{result.stderr}"
        )
    return result.stdout


def write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=False) + "\n", encoding="utf-8")


def read_json(path: Path) -> Any:
    return json.loads(path.read_text(encoding="utf-8"))


def node_json(args: list[str], output_file: Path) -> dict[str, Any]:
    stdout = run([str(NODE_BIN), *args], stdout_file=output_file)
    return json.loads(stdout)


def copy_inputs(out_dir: Path) -> dict[str, Path]:
    inputs_dir = out_dir / "inputs"
    inputs_dir.mkdir(parents=True, exist_ok=True)
    copied = {}
    for name in ["route-config.json", "primary-subscription.json", "export-debit.json"]:
        source = SOURCE_EVIDENCE / "inputs" / name
        destination = inputs_dir / name
        shutil.copyfile(source, destination)
        copied[name] = destination
    return copied


def collect_validator_reports(
    *,
    out_dir: Path,
    base_dir: Path,
    validators: int,
    label: str,
) -> dict[str, Any]:
    reports_dir = out_dir / "reports"
    states = []
    for index in range(validators):
        node_dir = base_dir / f"node{index}"
        status = node_json(["status", "--data-dir", str(node_dir)], reports_dir / f"{label}-validator-{index}-status.json")
        blocks = node_json(
            ["verify-blocks", "--data-dir", str(node_dir)],
            reports_dir / f"{label}-validator-{index}-verify-blocks.json",
        )
        states.append(
            {
                "validator": index,
                "state_root": status.get("state_root"),
                "block_height": status.get("block_height"),
                "block_tip_hash": status.get("block_tip_hash"),
                "node_status": status.get("status"),
                "mempool_pending": status.get("mempool_pending"),
                "verified_block_count": blocks.get("block_count"),
                "verified_block_tip": blocks.get("tip_hash"),
                "block_tip_verified": blocks.get("tip_hash") == status.get("block_tip_hash"),
            }
        )
    roots = {state["state_root"] for state in states}
    heights = {state["block_height"] for state in states}
    tips = {state["block_tip_hash"] for state in states}
    verified = all(state["block_tip_verified"] for state in states)
    consensus_ok = len(roots) == 1 and len(heights) == 1 and len(tips) == 1 and verified
    report = {
        "schema": "postfiat-pftl-uniswap-gate1-devnet-validator-report-v1",
        "label": label,
        "validator_count": validators,
        "consensus_ok": consensus_ok,
        "states": states,
    }
    write_json(reports_dir / f"{label}-validator-summary.json", report)
    if not consensus_ok:
        raise RuntimeError(f"validator consensus failed for {label}: {report}")
    return report


def write_readme(out_dir: Path, summary: dict[str, Any]) -> None:
    initial_ok = str(summary["initial_validator_consensus_ok"]).lower()
    final_ok = str(summary["final_validator_consensus_ok"]).lower()
    supply_invariant = str(summary["supply_invariant"]).lower()
    text = f"""# PFTL-Uniswap Gate 1 Local Devnet Evidence

Generated: 2026-07-01.

Scope: clean local-devnet evidence for the Gate 1 PFTL bridge packet prototype.
The runner creates a fresh `{summary["validator_count"]}`-validator local devnet,
runs the checked Gate 1 bridge transition set against validator `node0`, captures
validator convergence before and after the bridge-side operations, and verifies
receipt replay from the copied devnet-side bridge ledger and receipt files.

This is still controlled local evidence. It is not Gate 5 verifier evidence and
does not enable public routing.

## Command

```bash
python3 scripts/pftl-uniswap-gate1-devnet-execute.py --replace
```

## Result

- Chain id: `{summary["chain_id"]}`
- Validator count: `{summary["validator_count"]}`
- Initial validator consensus: `{initial_ok}`
- Final validator consensus: `{final_ok}`
- Receipt replay status: `{summary["receipt_replay_status"]}`
- Receipt count: `{summary["receipt_count"]}`
- Receipt root: `{summary["receipt_root"]}`
- Final ledger hash: `{summary["final_ledger_hash"]}`
- Supply invariant: `{supply_invariant}`

## Reports

- `reports/00-devnet-up.txt`
- `reports/01-initial-validator-summary.json`
- `reports/02-route-init.json`
- `reports/03-primary-subscription.json`
- `reports/04-export-debit.json`
- `reports/05-destination-consume.json`
- `reports/06-return-burn-request.json`
- `reports/07-record-return-burn.json`
- `reports/08-import-return.json`
- `reports/09-routes.json`
- `reports/10-packet.json`
- `reports/11-claims.json`
- `reports/12-supply-status.json`
- `reports/13-receipt-replay.json`
- `reports/14-final-validator-summary.json`
- `node0-bridge-state/pftl_uniswap_bridge_ledgers.json`
- `node0-bridge-state/pftl_uniswap_bridge_receipts.json`
"""
    (out_dir / "README.md").write_text(text, encoding="utf-8")


def main() -> int:
    args = parse_args()
    if args.validators < 1:
        raise RuntimeError("--validators must be positive")

    out_dir = Path(args.out_dir).resolve()
    if out_dir.exists():
        if not args.replace:
            raise RuntimeError(f"evidence directory already exists: {out_dir}; pass --replace")
        shutil.rmtree(out_dir)
    (out_dir / "reports").mkdir(parents=True)
    (out_dir / "node0-bridge-state").mkdir(parents=True)

    inputs = copy_inputs(out_dir)
    temp_root = Path(tempfile.mkdtemp(prefix="pftl-uniswap-gate1-devnet-"))
    base_dir = temp_root / "local"
    config_dir = out_dir / "configs"
    log_dir = temp_root / "logs"
    chain_id = "postfiat-pftl-uniswap-gate1-local"
    env = os.environ.copy()
    env.update(
        {
            "VALIDATORS": str(args.validators),
            "BASE_DIR": str(base_dir),
            "LOG_DIR": str(log_dir),
            "CONFIG_DIR": str(config_dir),
            "CHAIN_ID": chain_id,
            "BASE_PORT": "27650",
        }
    )

    try:
        run(["scripts/devnet-up"], stdout_file=out_dir / "reports" / "00-devnet-up.txt", env=env)
        initial = collect_validator_reports(
            out_dir=out_dir,
            base_dir=base_dir,
            validators=args.validators,
            label="01-initial",
        )
        node0 = base_dir / "node0"
        route_init = node_json(
            [
                "navcoin-bridge-route-init",
                "--data-dir",
                str(node0),
                "--config-file",
                str(inputs["route-config.json"]),
                "--ethereum-chain-id",
                "1",
                "--latest-finalized-nav-epoch",
                "7",
                "--return-finality-blocks",
                "64",
                "--replace",
            ],
            out_dir / "reports" / "02-route-init.json",
        )
        primary = node_json(
            [
                "navcoin-bridge-primary-subscribe",
                "--data-dir",
                str(node0),
                "--request-file",
                str(inputs["primary-subscription.json"]),
            ],
            out_dir / "reports" / "03-primary-subscription.json",
        )
        export = node_json(
            [
                "navcoin-bridge-export-debit",
                "--data-dir",
                str(node0),
                "--request-file",
                str(inputs["export-debit.json"]),
            ],
            out_dir / "reports" / "04-export-debit.json",
        )
        destination = node_json(
            [
                "navcoin-bridge-destination-consume",
                "--data-dir",
                str(node0),
                "--route-id",
                ROUTE_ID,
                "--packet-hash",
                EXPORT_PACKET_HASH,
            ],
            out_dir / "reports" / "05-destination-consume.json",
        )
        return_burn_file = out_dir / "inputs" / "return-burn.json"
        return_burn = node_json(
            [
                "navcoin-bridge-return-burn-request",
                "--data-dir",
                str(node0),
                "--route-id",
                ROUTE_ID,
                "--ethereum-sender",
                ETHEREUM_SENDER,
                "--pftl-recipient",
                PFTL_RECIPIENT,
                "--amount-atoms",
                "25",
                "--return-nonce",
                RETURN_NONCE,
                "--burn-height",
                "100",
                "--output-file",
                str(return_burn_file),
                "--overwrite",
            ],
            out_dir / "reports" / "06-return-burn-request.json",
        )
        record_return = node_json(
            [
                "navcoin-bridge-record-return-burn",
                "--data-dir",
                str(node0),
                "--route-id",
                ROUTE_ID,
                "--request-file",
                str(return_burn_file),
            ],
            out_dir / "reports" / "07-record-return-burn.json",
        )
        imported = node_json(
            [
                "navcoin-bridge-import-return",
                "--data-dir",
                str(node0),
                "--route-id",
                ROUTE_ID,
                "--burn-event-hash",
                return_burn["burn_event_hash"],
                "--pftl-recipient",
                PFTL_RECIPIENT,
            ],
            out_dir / "reports" / "08-import-return.json",
        )
        routes = node_json(
            ["navcoin-bridge-routes", "--data-dir", str(node0)],
            out_dir / "reports" / "09-routes.json",
        )
        packet = node_json(
            [
                "navcoin-bridge-packet",
                "--data-dir",
                str(node0),
                "--route-id",
                ROUTE_ID,
                "--packet-hash",
                EXPORT_PACKET_HASH,
            ],
            out_dir / "reports" / "10-packet.json",
        )
        claims = node_json(
            ["navcoin-bridge-claims", "--data-dir", str(node0), "--route-id", ROUTE_ID, "--include-terminal"],
            out_dir / "reports" / "11-claims.json",
        )
        supply = node_json(
            ["navcoin-bridge-supply-status", "--data-dir", str(node0), "--route-id", ROUTE_ID],
            out_dir / "reports" / "12-supply-status.json",
        )
        replay = node_json(
            ["navcoin-bridge-receipt-replay", "--data-dir", str(node0), "--route-id", ROUTE_ID],
            out_dir / "reports" / "13-receipt-replay.json",
        )
        final = collect_validator_reports(
            out_dir=out_dir,
            base_dir=base_dir,
            validators=args.validators,
            label="14-final",
        )

        bridge_state = out_dir / "node0-bridge-state"
        shutil.copyfile(node0 / "pftl_uniswap_bridge_ledgers.json", bridge_state / "pftl_uniswap_bridge_ledgers.json")
        shutil.copyfile(node0 / "pftl_uniswap_bridge_receipts.json", bridge_state / "pftl_uniswap_bridge_receipts.json")

        if replay.get("status") != EXPECTED_STATUS or replay.get("receipt_count") != EXPECTED_RECEIPT_COUNT:
            raise RuntimeError(f"receipt replay did not verify expected Gate 1 packet: {replay}")
        if supply.get("invariant_holds") is not True:
            raise RuntimeError(f"supply invariant failed: {supply}")

        summary = {
            "schema": "postfiat-pftl-uniswap-gate1-devnet-evidence-summary-v1",
            "chain_id": chain_id,
            "validator_count": args.validators,
            "node0_data_dir_class": "fresh_local_devnet_validator",
            "initial_validator_consensus_ok": initial["consensus_ok"],
            "final_validator_consensus_ok": final["consensus_ok"],
            "route_config_digest": route_init["route_config_digest"],
            "receipt_replay_status": replay["status"],
            "receipt_count": replay["receipt_count"],
            "receipt_root": replay["receipt_root"],
            "initial_ledger_hash": replay["initial_ledger_hash"],
            "final_ledger_hash": replay["final_ledger_hash"],
            "supply_invariant": supply["invariant_holds"],
            "transition_reports": {
                "route_init": route_init.get("schema"),
                "primary_subscription": primary.get("schema"),
                "export_debit": export.get("schema"),
                "destination_consume": destination.get("schema"),
                "return_burn_request": return_burn.get("schema"),
                "record_return_burn": record_return.get("schema"),
                "import_return": imported.get("schema"),
                "routes": routes.get("schema"),
                "packet": packet.get("schema"),
                "claims": claims.get("schema"),
                "supply_status": supply.get("schema"),
                "receipt_replay": replay.get("schema"),
            },
        }
        write_json(out_dir / "reports" / "15-summary.json", summary)
        write_readme(out_dir, summary)
        print(json.dumps(summary, indent=2))
        return 0
    finally:
        if args.keep_devnet:
            print(f"kept local devnet data at {temp_root}")
        else:
            shutil.rmtree(temp_root, ignore_errors=True)


if __name__ == "__main__":
    raise SystemExit(main())
