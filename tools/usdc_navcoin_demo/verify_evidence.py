"""Independently verify both live ledgers and the public Lane B bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
from typing import Any

from tools.xrpl_navcoin_demo.pftl_adapter import (
    ASSET_ID,
    COORDINATOR,
    PFTL_BIN,
    PFTL_ROOT,
    USER,
    HardenedPftl,
)


RPC = "http://127.0.0.1:39545"
CAST = "/home/postfiat/.foundry/bin/cast"
EXPECTED_BINARY_SHA256 = (
    "006167226531582cf81666dded004f26707beedc2ce3fa850caf5b0b82fd22e7"
)


def cast_json(arguments: list[str]) -> Any:
    completed = subprocess.run(
        [CAST, *arguments, "--json"],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return json.loads(completed.stdout)


def call(contract: str, signature: str, *arguments: str) -> Any:
    return cast_json(["call", "--rpc-url", RPC, contract, signature, *arguments])


def rpc(method: str, *arguments: str) -> Any:
    return cast_json(["rpc", "--rpc-url", RPC, method, *arguments])


def code(address: str) -> bytes:
    completed = subprocess.run(
        [CAST, "code", "--rpc-url", RPC, address],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return bytes.fromhex(completed.stdout.strip().removeprefix("0x"))


def balance(token: str, account: str) -> int:
    return int(call(token, "balanceOf(address)(uint256)", account)[0])


def encoded_address(value: str) -> str:
    return value.lower().removeprefix("0x").rjust(64, "0")


def encoded_uint(value: int) -> str:
    return hex(value)[2:].rjust(64, "0")


def artifact_bytecode(path: Path) -> str:
    artifact = json.loads(path.read_text())
    return artifact["bytecode"]["object"].removeprefix("0x")


def certified_pftl_receipt(
    pftl: HardenedPftl,
    *,
    evidence: Path,
    report: dict[str, Any],
    label: str,
    scenario_name: str,
    transaction_key: str,
    operation_kind: str,
    amount_atoms: int,
    deltas: dict[str, int],
) -> dict[str, Any]:
    scenario = report["scenarios"][scenario_name]
    tx_id = scenario["transactions"][transaction_key]
    certified = json.loads(
        (evidence / "pftl" / f"{label}.certified.json").read_text()
    )
    operation = certified["operation"]
    if (
        certified["tx_id"] != tx_id
        or certified["receipt_code"] != "accepted"
        or operation["operation"] != operation_kind
    ):
        raise AssertionError(f"{label}: certified operation binding mismatch")
    if operation_kind == "escrow_create":
        if (
            int(operation["amount"]) != amount_atoms
            or operation["asset_id"] != ASSET_ID
        ):
            raise AssertionError(f"{label}: create principal mismatch")
    elif operation["escrow_id"] != scenario["pftl_escrow"]["escrow_id"]:
        raise AssertionError(f"{label}: terminal escrow binding mismatch")
    if sum(deltas.values()) != 0:
        raise AssertionError(f"{label}: principal deltas do not conserve atoms")

    history = pftl._run(
        [
            "account-tx",
            "--data-dir",
            str(pftl.node0),
            "--address",
            certified["source"],
            "--limit",
            "100",
        ]
    )
    matching_rows = [row for row in history["rows"] if row["tx_id"] == tx_id]
    if len(matching_rows) != 1:
        raise AssertionError(f"{label}: account-history binding is not unique")
    history_row = matching_rows[0]
    expected_to = (
        operation["owner"]
        if operation_kind == "escrow_cancel"
        else operation["recipient"]
    )
    if (
        history_row["transaction_kind"] != operation_kind
        or history_row["from"] != operation["owner"]
        or history_row["to"] != expected_to
        or history_row["escrow_id"] != scenario["pftl_escrow"]["escrow_id"]
        or not history_row["accepted"]
        or history_row["receipt_code"] != "accepted"
    ):
        raise AssertionError(f"{label}: exact account-history row mismatch")
    if operation_kind == "escrow_create" and (
        int(history_row["amount"]) != amount_atoms
        or history_row["asset_id"] != ASSET_ID
    ):
        raise AssertionError(f"{label}: account-history principal mismatch")

    proof = pftl._run(
        [
            "rpc",
            "--method",
            "tx",
            "--data-dir",
            str(pftl.node0),
            "--tx-id",
            tx_id,
        ]
    )
    result = proof["result"]
    receipt = result["receipt"]
    certificate = result["block"]["header"]["certificate"]
    expected_validators = {f"validator-{index}" for index in range(6)}
    if (
        not proof.get("ok")
        or not result["confirmed"]
        or result["tx_id"] != tx_id
        or not receipt["accepted"]
        or receipt["code"] != "accepted"
        or receipt["tx_id"] != tx_id
        or set(certificate["validators"]) != expected_validators
        or not all(vote["accept"] for vote in certificate["votes"])
    ):
        raise AssertionError(f"{label}: exact ACCEPTED finality proof mismatch")
    return {
        "label": label,
        "scenario": scenario_name,
        "operation": operation_kind,
        "amount_atoms": amount_atoms,
        "tx_id": tx_id,
        "block_height": result["block"]["header"]["height"],
        "receipt_accepted": True,
        "receipt_code": receipt["code"],
        "fee_charged": receipt["fee_charged"],
        "fee_burned": receipt["fee_burned"],
        "account_history_bound": True,
        "certificate_validators": sorted(certificate["validators"]),
        "certificate_accept_votes": len(certificate["votes"]),
        "exact_principal_deltas": deltas,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-root", required=True, type=Path)
    args = parser.parse_args()
    runtime = args.runtime_root.resolve()
    evidence = runtime / "public/evidence"
    report = json.loads((evidence / "live-demo-report.json").read_text())
    if report["result"] != "PASS":
        raise AssertionError("live report is not PASS")

    binary_sha = hashlib.sha256(PFTL_BIN.read_bytes()).hexdigest()
    if binary_sha != EXPECTED_BINARY_SHA256:
        raise AssertionError("hardened PFTL binary mismatch")

    evm = report["networks"]["evm"]
    if int(cast_json(["chain-id", "--rpc-url", RPC])) != 31337:
        raise AssertionError("Anvil chain-id mismatch")
    token = evm["mock_usdc"]
    htlc = evm["htlc"]
    runtime_hashes = {
        "mock_usdc": hashlib.sha256(code(token)).hexdigest(),
        "htlc": hashlib.sha256(code(htlc)).hexdigest(),
    }
    if runtime_hashes != evm["deployments"]["runtime_code_sha256"]:
        raise AssertionError("live EVM runtime code hash mismatch")

    # Bind each deployment to the compiled source creation bytecode and exact
    # constructor arguments, instead of trusting addresses in the report.
    repo = Path(__file__).resolve().parents[2]
    mock_creation = artifact_bytecode(
        repo / "out/MockUSDC.sol/MockUSDC.json"
    )
    htlc_creation = artifact_bytecode(
        repo / "out/USDCNavHTLC.sol/USDCNavHTLC.json"
    )
    mock_tx_hash = evm["deployments"]["mock_usdc"]["transaction_hash"]
    htlc_tx_hash = evm["deployments"]["htlc"]["transaction_hash"]
    mock_tx = rpc("eth_getTransactionByHash", mock_tx_hash)
    htlc_tx = rpc("eth_getTransactionByHash", htlc_tx_hash)
    expected_mock_input = (
        "0x"
        + mock_creation
        + encoded_address(evm["user"])
        + encoded_address(evm["coordinator"])
        + encoded_uint(1_000_000)
    )
    expected_htlc_input = "0x" + htlc_creation + encoded_address(token)
    if mock_tx["input"].lower() != expected_mock_input.lower():
        raise AssertionError("MockUSDC deployment is not compiled source+arguments")
    if htlc_tx["input"].lower() != expected_htlc_input.lower():
        raise AssertionError("HTLC deployment is not compiled source+arguments")

    tx_hashes = {mock_tx_hash, htlc_tx_hash, *report["approvals"]}
    for scenario in report["scenarios"].values():
        for key, value in scenario["transactions"].items():
            if key.startswith("evm_"):
                tx_hashes.add(value)
    receipts = {
        tx_hash: cast_json(["receipt", "--rpc-url", RPC, tx_hash])
        for tx_hash in tx_hashes
    }
    if any(item["status"] != "0x1" for item in receipts.values()):
        raise AssertionError("an EVM evidence transaction is not successful")

    observed_swaps: dict[str, dict[str, Any]] = {}
    for scenario_name, expected_state in (
        ("usdc_to_nav_happy", 2),
        ("nav_to_usdc_happy", 2),
        ("refund", 3),
    ):
        scenario = report["scenarios"][scenario_name]
        row = call(
            htlc,
            "swaps(bytes32)(address,address,uint128,uint64,bytes32,uint8)",
            scenario["swap_id"],
        )
        if int(row[5]) != expected_state:
            raise AssertionError(f"{scenario_name} EVM terminal state mismatch")
        if row[4].lower() != ("0x" + scenario["payment_hash"]).lower():
            raise AssertionError(f"{scenario_name} EVM hashlock mismatch")
        observed_swaps[scenario["swap_id"]] = {
            "refund_address": row[0],
            "recipient": row[1],
            "amount": int(row[2]),
            "refund_time": int(row[3]),
            "hashlock": row[4],
            "state": int(row[5]),
        }

    public_preimages: dict[str, str] = {}
    for scenario_name in ("usdc_to_nav_happy", "nav_to_usdc_happy"):
        scenario = report["scenarios"][scenario_name]
        redeem_hash = scenario["transactions"]["evm_redeem"]
        logs = [
            item
            for item in receipts[redeem_hash]["logs"]
            if item["address"].lower() == htlc.lower() and len(item["topics"]) == 2
        ]
        if len(logs) != 1:
            raise AssertionError("expected one HTLC Redeemed log")
        preimage = logs[0]["data"].removeprefix("0x")
        if hashlib.sha256(bytes.fromhex(preimage)).hexdigest() != scenario["payment_hash"]:
            raise AssertionError("public event preimage does not match SHA-256 hashlock")
        if preimage != scenario["public_preimage_hex"]:
            raise AssertionError("public event preimage/report mismatch")
        public_preimages[scenario_name] = preimage

    balances = {
        "user": balance(token, evm["user"]),
        "coordinator": balance(token, evm["coordinator"]),
        "htlc": balance(token, htlc),
    }
    total_supply = int(call(token, "totalSupply()(uint256)")[0])
    if (
        balances != {"user": 980_000, "coordinator": 1_020_000, "htlc": 0}
        or sum(balances.values()) != total_supply
        or total_supply != 2_000_000
    ):
        raise AssertionError("mock-USDC exact conservation mismatch")

    pftl = HardenedPftl(evidence / "verification")
    convergence = pftl.converged_status()
    ledgers = [
        json.loads((PFTL_ROOT / f"nodes/validator-{index}/ledger.json").read_text())
        for index in range(6)
    ]
    nav_records = [
        next(item for item in ledger["nav_assets"] if item["asset_id"] == ASSET_ID)
        for ledger in ledgers
    ]
    reserve_records = [
        next(
            item
            for item in ledger["nav_reserve_packets"]
            if item["asset_id"] == ASSET_ID and item["state"] == "finalized"
        )
        for ledger in ledgers
    ]
    if len({json.dumps(item, sort_keys=True) for item in nav_records}) != 1:
        raise AssertionError("six PFTL validators disagree on NAV state")
    if len({json.dumps(item, sort_keys=True) for item in reserve_records}) != 1:
        raise AssertionError("six PFTL validators disagree on reserve state")
    nav = nav_records[0]
    reserve = reserve_records[0]
    if (
        nav["finalized_epoch"] != 1
        or nav["nav_per_unit"] != 1_035_074_022
        or reserve["verified_net_assets"] != 3_105_222_068_834
        or reserve["reserve_accounts"] != ["a651-phase-b-20260721"]
        or not any(item["pass"] for item in reserve["attestations"])
    ):
        raise AssertionError("proven-NAV checkpoint mismatch")

    expected_pftl_states = {
        report["scenarios"]["usdc_to_nav_happy"]["pftl_escrow"]["escrow_id"]: "finished",
        report["scenarios"]["nav_to_usdc_happy"]["pftl_escrow"]["escrow_id"]: "finished",
        report["scenarios"]["refund"]["pftl_escrow"]["escrow_id"]: "canceled",
    }
    observed_pftl_states = {}
    for escrow_id, expected in expected_pftl_states.items():
        state = pftl.escrow_info(escrow_id)["escrow"]["state"]
        if state != expected:
            raise AssertionError(f"PFTL escrow {escrow_id} is not {expected}")
        observed_pftl_states[escrow_id] = state
    open_escrows = {
        item["escrow_id"]: item
        for owner in (USER, COORDINATOR)
        for item in pftl.escrows(owner, role="owner")
        if item["state"] == "open" and item["asset_id"] == ASSET_ID
    }
    user_nav_atoms = pftl.balance(USER)
    coordinator_nav_atoms = pftl.balance(COORDINATOR)
    locked_nav_atoms = sum(int(item["amount"]) for item in open_escrows.values())
    nav_total = user_nav_atoms + coordinator_nav_atoms + locked_nav_atoms
    if nav_total != 3_000_000_000:
        raise AssertionError("NAVcoin atom conservation mismatch")

    receipt_specs = (
        (
            "refund-02-nav-create",
            "refund",
            "pftl_create",
            "escrow_create",
            25_000,
            {"user_atoms": 0, "coordinator_atoms": -25_000, "escrow_atoms": 25_000},
        ),
        (
            "onramp-02-nav-create",
            "usdc_to_nav_happy",
            "pftl_create",
            "escrow_create",
            100_000,
            {"user_atoms": 0, "coordinator_atoms": -100_000, "escrow_atoms": 100_000},
        ),
        (
            "onramp-03-nav-finish",
            "usdc_to_nav_happy",
            "pftl_finish",
            "escrow_finish",
            100_000,
            {"user_atoms": 100_000, "coordinator_atoms": 0, "escrow_atoms": -100_000},
        ),
        (
            "offramp-01-nav-create",
            "nav_to_usdc_happy",
            "pftl_create",
            "escrow_create",
            80_000,
            {"user_atoms": -80_000, "coordinator_atoms": 0, "escrow_atoms": 80_000},
        ),
        (
            "offramp-04-nav-finish",
            "nav_to_usdc_happy",
            "pftl_finish",
            "escrow_finish",
            80_000,
            {"user_atoms": 0, "coordinator_atoms": 80_000, "escrow_atoms": -80_000},
        ),
        (
            "refund-03-nav-cancel",
            "refund",
            "pftl_cancel",
            "escrow_cancel",
            25_000,
            {"user_atoms": 0, "coordinator_atoms": 25_000, "escrow_atoms": -25_000},
        ),
    )
    pftl_receipts = [
        certified_pftl_receipt(
            pftl,
            evidence=evidence,
            report=report,
            label=label,
            scenario_name=scenario_name,
            transaction_key=transaction_key,
            operation_kind=operation_kind,
            amount_atoms=amount_atoms,
            deltas=deltas,
        )
        for (
            label,
            scenario_name,
            transaction_key,
            operation_kind,
            amount_atoms,
            deltas,
        ) in receipt_specs
    ]
    pftl_receipts.sort(key=lambda item: item["block_height"])
    lane_nav_deltas = {
        key: sum(item["exact_principal_deltas"][key] for item in pftl_receipts)
        for key in ("user_atoms", "coordinator_atoms", "escrow_atoms")
    }
    if lane_nav_deltas != {
        "user_atoms": 20_000,
        "coordinator_atoms": -20_000,
        "escrow_atoms": 0,
    }:
        raise AssertionError("NAVcoin exact lane deltas mismatch")

    adversarial = report["adversarial"]
    for category in adversarial.values():
        if not category["both_ledgers_mutation_free"]:
            raise AssertionError("adversarial probe was not mutation-free")
    if not (
        adversarial["wrong_preimage"]["evm"]["rejected"]
        and adversarial["early_cancel"]["evm"]["rejected"]
        and adversarial["late_at_or_after_cancel"]["evm_at_cancel"]["rejected"]
        and adversarial["late_at_or_after_cancel"]["evm_after_refund"]["rejected"]
        and all(item["rejected"] for item in adversarial["duplicates"]["evm_redeems"])
        and adversarial["duplicates"]["evm_refund"]["rejected"]
        and "WRONG_PREIMAGE" in adversarial["wrong_preimage"]["evm"]["error"]
        and "TOO_EARLY" in adversarial["early_cancel"]["evm"]["error"]
        and "EXPIRED" in adversarial["late_at_or_after_cancel"]["evm_at_cancel"]["error"]
        and "NOT_OPEN"
        in adversarial["late_at_or_after_cancel"]["evm_after_refund"]["error"]
        and all(
            "NOT_OPEN" in item["error"]
            for item in adversarial["duplicates"]["evm_redeems"]
        )
        and "NOT_OPEN" in adversarial["duplicates"]["evm_refund"]["error"]
    ):
        raise AssertionError("adversarial rejection evidence mismatch")

    source_hashes = {
        str(path.relative_to(repo)): hashlib.sha256(path.read_bytes()).hexdigest()
        for path in (
            repo / "tools/usdc_navcoin_demo/contracts/MockUSDC.sol",
            repo / "tools/usdc_navcoin_demo/contracts/USDCNavHTLC.sol",
        )
    }
    result = {
        "schema": "postfiat.anvil_usdc_navcoin.independent_verification.v1",
        "result": "PASS",
        "claim": report["claim"],
        "evm": {
            "network": "Anvil",
            "rpc": RPC,
            "chain_id": 31337,
            "mock_usdc": token,
            "htlc": htlc,
            "source_bound_deployments": True,
            "source_sha256": source_hashes,
            "runtime_code_sha256": runtime_hashes,
            "successful_transactions": len(receipts),
            "swap_states": observed_swaps,
            "public_preimages_authenticated": len(public_preimages),
            "refund_preimage_revealed": False,
            "balances": balances,
            "total_supply_atoms": total_supply,
            "exact_lane_deltas": {
                "user_atoms": -20_000,
                "coordinator_atoms": 20_000,
                "htlc_atoms": 0,
            },
            "exact_conservation": True,
        },
        "pftl": {
            "convergence": convergence,
            "binary_sha256": binary_sha,
            "asset_id": ASSET_ID,
            "nav_per_unit_usd_e8": nav["nav_per_unit"],
            "verified_net_assets_usd_e8": reserve["verified_net_assets"],
            "reserve_account": reserve["reserve_accounts"][0],
            "escrow_states": observed_pftl_states,
            "accepted_receipt_count": len(pftl_receipts),
            "accepted_receipts": pftl_receipts,
            "exact_lane_deltas": lane_nav_deltas,
            "final_principal": {
                "user_atoms": user_nav_atoms,
                "coordinator_atoms": coordinator_nav_atoms,
                "open_escrow_atoms": locked_nav_atoms,
            },
            "conserved_navcoin_atoms": nav_total,
        },
        "adversarial": {
            "wrong_preimage_mutation_free": True,
            "early_cancel_mutation_free": True,
            "late_at_and_after_cancel_mutation_free": True,
            "duplicates_mutation_free": True,
        },
        "public_preimages_authenticated": len(public_preimages),
        "refund_preimage_revealed": False,
        "reference_lane": report["reference_lane"],
    }
    output = evidence / "independent-verification.json"
    output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    manifest_path = evidence / "sha256-manifest.json"
    artifacts = []
    for path in sorted(evidence.rglob("*")):
        if not path.is_file() or path == manifest_path:
            continue
        payload = path.read_bytes()
        artifacts.append(
            {
                "path": str(path.relative_to(evidence)),
                "bytes": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
            }
        )
    manifest_path.write_text(
        json.dumps(
            {
                "schema": "postfiat.anvil_usdc_navcoin.evidence_manifest.v1",
                "artifact_count": len(artifacts),
                "artifacts": artifacts,
            },
            indent=2,
            sort_keys=True,
        )
        + "\n"
    )
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
