"""Independent live verifier for the packaged XRP/NAVcoin demonstration."""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from .pftl_adapter import ASSET_ID, COORDINATOR, PFTL_BIN, PFTL_ROOT, USER, HardenedPftl
from .protocol import extract_xrpl_finish_preimage
from .xrpl_adapter import XrplTestnet


EXPECTED_BINARY_SHA256 = (
    "006167226531582cf81666dded004f26707beedc2ce3fa850caf5b0b82fd22e7"
)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-root", required=True, type=Path)
    args = parser.parse_args()
    report = json.loads(
        (args.runtime_root / "public/evidence/live-demo-report.json").read_text()
    )
    if report["result"] != "PASS":
        raise AssertionError("demo report is not PASS")

    binary_sha = hashlib.sha256(PFTL_BIN.read_bytes()).hexdigest()
    if binary_sha != EXPECTED_BINARY_SHA256:
        raise AssertionError("hardened PFTL binary hash mismatch")

    pftl = HardenedPftl(args.runtime_root / "public/evidence/verification")
    convergence = pftl.converged_status()
    ledgers = [
        json.loads(
            (PFTL_ROOT / f"nodes/validator-{index}/ledger.json").read_text()
        )
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
    canonical_nav = json.dumps(nav_records[0], sort_keys=True)
    canonical_reserve = json.dumps(reserve_records[0], sort_keys=True)
    if any(json.dumps(item, sort_keys=True) != canonical_nav for item in nav_records):
        raise AssertionError("validators disagree on NAV state")
    if any(
        json.dumps(item, sort_keys=True) != canonical_reserve
        for item in reserve_records
    ):
        raise AssertionError("validators disagree on reserve packet")
    nav = nav_records[0]
    reserve = reserve_records[0]
    if (
        nav["finalized_epoch"] != 1
        or nav["nav_per_unit"] != 1_035_074_022
        or reserve["verified_net_assets"] != 3_105_222_068_834
        or reserve["reserve_accounts"] != ["a651-phase-b-20260721"]
        or not any(item["pass"] for item in reserve["attestations"])
    ):
        raise AssertionError("proven NAV checkpoint mismatch")

    expected_pftl_states = {
        report["scenarios"]["xrp_to_nav_happy"]["pftl_escrow"]["escrow_id"]: "finished",
        report["scenarios"]["nav_to_xrp_happy"]["pftl_escrow"]["escrow_id"]: "finished",
        report["scenarios"]["refund"]["pftl_escrow"]["escrow_id"]: "canceled",
    }
    observed_pftl_states = {}
    for escrow_id, expected in expected_pftl_states.items():
        escrow = pftl.escrow_info(escrow_id)["escrow"]
        if escrow["state"] != expected:
            raise AssertionError(f"PFTL escrow {escrow_id} is not {expected}")
        observed_pftl_states[escrow_id] = escrow["state"]

    # PFTL NAVcoin principal is exactly its 3B atom issuance, including the
    # unrelated pre-existing open escrow inherited from the proven-NAV chain.
    pftl_open = {
        item["escrow_id"]: item
        for owner in (USER, COORDINATOR)
        for item in pftl.escrows(owner, role="owner")
        if item["state"] == "open" and item["asset_id"] == ASSET_ID
    }
    nav_total = (
        pftl.balance(USER)
        + pftl.balance(COORDINATOR)
        + sum(int(item["amount"]) for item in pftl_open.values())
    )
    if nav_total != 3_000_000_000:
        raise AssertionError("NAVcoin atom conservation failed")

    xrpl = XrplTestnet(args.runtime_root / "public/evidence/verification")
    xrpl_hashes = []
    for scenario in report["scenarios"].values():
        xrpl_hashes.extend(
            value
            for key, value in scenario["transactions"].items()
            if key.startswith("xrpl_")
        )
    validated = {tx_hash: xrpl.tx(tx_hash) for tx_hash in xrpl_hashes}
    total_fees = 0
    for tx_hash, transaction in validated.items():
        if transaction["meta"]["TransactionResult"] != "tesSUCCESS":
            raise AssertionError(f"XRPL transaction {tx_hash} was not tesSUCCESS")
        total_fees += int(transaction["tx_json"]["Fee"])

    for scenario_name in ("xrp_to_nav_happy", "nav_to_xrp_happy"):
        scenario = report["scenarios"][scenario_name]
        finish = validated[scenario["transactions"]["xrpl_finish"]]["tx_json"]
        secret = extract_xrpl_finish_preimage(
            finish, expected_condition=scenario["hashlock"]["xrpl_condition"]
        )
        if secret.protocol_hex() != scenario["xrpl_public_preimage_hex"]:
            raise AssertionError("authenticated public XRPL preimage mismatch")

    user = report["networks"]["xrpl"]["user"]
    coordinator = report["networks"]["xrpl"]["coordinator"]
    accounts = [xrpl.account(user), xrpl.account(coordinator)]
    open_xrpl = xrpl.escrows(user) + xrpl.escrows(coordinator)
    if open_xrpl:
        raise AssertionError("demo XRPL accounts still own open escrows")
    final_xrp = sum(account["balance_drops"] for account in accounts)
    if 200_000_000 - final_xrp != total_fees:
        raise AssertionError("XRP conservation does not equal validated fees")

    adversarial = report["adversarial"]
    if not (
        adversarial["wrong_preimage_and_early_cancel"][
            "principal_state_unchanged"
        ]
        and adversarial["late_finish"]["principal_state_unchanged"]
        and adversarial["duplicate"]["duplicate_suppressed"]
        and adversarial["duplicate"]["side_effect_callable_count"] == 1
    ):
        raise AssertionError("adversarial evidence failed")

    result = {
        "schema": "postfiat.xrpl_navcoin.independent_verification.v1",
        "result": "PASS",
        "claim": report["claim"],
        "pftl": {
            "convergence": convergence,
            "binary_sha256": binary_sha,
            "asset_id": ASSET_ID,
            "nav_per_unit_usd_e8": nav["nav_per_unit"],
            "verified_net_assets_usd_e8": reserve["verified_net_assets"],
            "reserve_account": reserve["reserve_accounts"][0],
            "escrow_states": observed_pftl_states,
            "conserved_navcoin_atoms": nav_total,
        },
        "xrpl": {
            "network": "Testnet",
            "validated_transactions": len(validated),
            "validated_fee_total_drops": total_fees,
            "final_account_total_drops": final_xrp,
            "open_escrows": 0,
        },
        "public_preimages_authenticated": 2,
        "refund_preimage_revealed": False,
    }
    output = args.runtime_root / "public/evidence/independent-verification.json"
    output.write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    evidence_root = output.parent
    manifest_path = evidence_root / "sha256-manifest.json"
    artifacts = []
    for path in sorted(evidence_root.rglob("*")):
        if not path.is_file() or path == manifest_path:
            continue
        payload = path.read_bytes()
        artifacts.append(
            {
                "path": str(path.relative_to(evidence_root)),
                "bytes": len(payload),
                "sha256": hashlib.sha256(payload).hexdigest(),
            }
        )
    manifest = {
        "schema": "postfiat.xrpl_navcoin.evidence_manifest.v1",
        "artifact_count": len(artifacts),
        "artifacts": artifacts,
    }
    manifest_path.write_text(json.dumps(manifest, indent=2, sort_keys=True) + "\n")
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
