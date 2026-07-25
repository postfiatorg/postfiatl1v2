"""Independent verifier for the Bitcoin regtest/NAVcoin evidence bundle."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
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


NODE = Path("/home/postfiat/.nvm/versions/node/v22.23.1/bin/node")
BITCOIN_CLI = Path(
    "/home/postfiat/tmp/bitcoin-core-31.0-download/bitcoin-31.0/bin/bitcoin-cli"
)
BITCOIN_DATADIR = Path(
    "/home/postfiat/tmp/pftl-btc-navcoin-regtest-v2-20260725/bitcoin"
)
BITCOIN_VERIFIER = Path(__file__).with_name("verify_bitcoin_evidence.mjs")
BTC_OPS = Path(__file__).with_name("btc_ops.mjs")
EXPECTED_BINARY_SHA256 = (
    "006167226531582cf81666dded004f26707beedc2ce3fa850caf5b0b82fd22e7"
)


def read_json(path: Path) -> Any:
    return json.loads(path.read_text())


def write_json(path: Path, value: Any, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, mode)


def run_json(arguments: list[str]) -> dict[str, Any]:
    completed = subprocess.run(
        arguments,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    value = json.loads(completed.stdout)
    if not isinstance(value, dict):
        raise AssertionError("verifier command did not return an object")
    return value


def core_json(*arguments: str) -> Any:
    completed = subprocess.run(
        [
            str(BITCOIN_CLI),
            "-regtest",
            f"-datadir={BITCOIN_DATADIR}",
            *arguments,
        ],
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    return json.loads(completed.stdout)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-root", required=True, type=Path)
    args = parser.parse_args()
    runtime_root = args.runtime_root.resolve()
    evidence = runtime_root / "public/evidence"
    report_path = evidence / "live-demo-report.json"
    report = read_json(report_path)
    if (
        report["schema"] != "postfiat.bitcoin_regtest_navcoin.live_demo.v1"
        or report["result"] != "PASS"
        or report["claim"] != "non-custodial, conditionally-atomic"
    ):
        raise AssertionError("live demo report is not the expected PASS report")

    bitcoin = run_json([str(NODE), str(BITCOIN_VERIFIER), str(report_path)])
    chain = core_json("getblockchaininfo")
    if (
        chain["chain"] != "regtest"
        or chain["initialblockdownload"]
        or chain["blocks"] != chain["headers"]
    ):
        raise AssertionError("independent Bitcoin Core node is not synchronized")

    # Ask the locally validating regtest node for a merkle inclusion proof for
    # each evidence transaction in its independently validated block.
    block_proofs = {}
    for txid in bitcoin["confirmed_transactions"]:
        record_candidates = list(evidence.glob(f"bitcoin/*.confirmed.json"))
        record = next(
            (
                read_json(path)
                for path in record_candidates
                if read_json(path).get("txid") == txid
            ),
            None,
        )
        if record is None:
            raise AssertionError(f"missing confirmed record for {txid}")
        block_hash = record["core"]["transaction"]["status"]["block_hash"]
        header = core_json("getblockheader", block_hash)
        if header["hash"] != block_hash or header["confirmations"] < 1:
            raise AssertionError(f"regtest block {block_hash} is not active")
        proof = subprocess.run(
            [
                str(BITCOIN_CLI),
                "-regtest",
                f"-datadir={BITCOIN_DATADIR}",
                "gettxoutproof",
                json.dumps([txid]),
                block_hash,
            ],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        ).stdout.strip()
        verified = core_json("verifytxoutproof", proof)
        if verified != [txid]:
            raise AssertionError(f"regtest merkle proof failed for {txid}")
        block_proofs[txid] = {
            "block_hash": block_hash,
            "block_height": header["height"],
            "confirmations": header["confirmations"],
            "proof_sha256": hashlib.sha256(bytes.fromhex(proof)).hexdigest(),
        }

    binary_sha = hashlib.sha256(PFTL_BIN.read_bytes()).hexdigest()
    if binary_sha != EXPECTED_BINARY_SHA256:
        raise AssertionError("hardened PFTL binary hash mismatch")
    pftl = HardenedPftl(evidence / "verification/pftl")
    convergence = pftl.converged_status()
    ledgers = [
        read_json(PFTL_ROOT / f"nodes/validator-{index}/ledger.json")
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
    canonical_nav = json.dumps(nav_records[0], sort_keys=True, separators=(",", ":"))
    canonical_reserve = json.dumps(
        reserve_records[0], sort_keys=True, separators=(",", ":")
    )
    if any(
        json.dumps(record, sort_keys=True, separators=(",", ":")) != canonical_nav
        for record in nav_records
    ):
        raise AssertionError("PFTL validators disagree on NAV asset state")
    if any(
        json.dumps(record, sort_keys=True, separators=(",", ":"))
        != canonical_reserve
        for record in reserve_records
    ):
        raise AssertionError("PFTL validators disagree on reserve packet")
    nav = nav_records[0]
    reserve = reserve_records[0]
    if (
        nav["finalized_epoch"] != 1
        or nav["nav_per_unit"] != 1_035_074_022
        or reserve["verified_net_assets"] != 3_105_222_068_834
        or reserve["reserve_accounts"] != ["a651-phase-b-20260721"]
    ):
        raise AssertionError("proven-NAV epoch does not match the pinned checkpoint")

    expected_states = {
        report["scenarios"]["btc_to_nav_happy"]["pftl_escrow"]["escrow_id"]: "finished",
        report["scenarios"]["nav_to_btc_happy"]["pftl_escrow"]["escrow_id"]: "finished",
        report["scenarios"]["refund"]["pftl_escrow"]["escrow_id"]: "canceled",
    }
    observed_states = {}
    for escrow_id, expected in expected_states.items():
        escrow = pftl.escrow_info(escrow_id)["escrow"]
        if escrow["state"] != expected:
            raise AssertionError(f"PFTL escrow {escrow_id} is not {expected}")
        observed_states[escrow_id] = expected

    open_nav = {
        item["escrow_id"]: item
        for owner in (USER, COORDINATOR)
        for item in pftl.escrows(owner, role="owner")
        if item["state"] == "open" and item["asset_id"] == ASSET_ID
    }
    nav_total = (
        pftl.balance(USER)
        + pftl.balance(COORDINATOR)
        + sum(int(item["amount"]) for item in open_nav.values())
    )
    if nav_total != 3_000_000_000:
        raise AssertionError("NAVcoin atom conservation failed")

    adversarial = report["adversarial"]
    first = adversarial["wrong_preimage_early_cancel_duplicates"]
    late = adversarial["late_finish_at_cancel"]
    if (
        first["bitcoin_wrong_preimage"].get("allowed") is not False
        or first["bitcoin_early_refund"].get("allowed") is not False
        or not first["pftl_wrong_preimage_rejected_before_signing"]
        or not first["pftl_early_cancel_rejected_before_signing"]
        or not first["pftl_principal_state_unchanged"]
        or not first["bitcoin_locked_outpoint_unchanged"]
        or not first["bitcoin_duplicate"]["duplicate_suppressed"]
        or not first["bitcoin_duplicate"]["confirmed_output_unchanged"]
        or not late["pftl_late_finish_rejected_before_signing"]
        or late["bitcoin_at_cancel_hash_claim_testmempoolaccept"].get("allowed")
        is not True
        or not late["bitcoin_at_cancel_probe_mutation_free"]
        or late["bitcoin_late_claim_testmempoolaccept"].get("allowed") is not False
        or not late["bitcoin_refunded_outpoint_remains_spent"]
        or not late["bitcoin_refund_preimage_not_published"]
    ):
        raise AssertionError("adversarial evidence is incomplete")

    late_private = runtime_root / "private/bitcoin/late-claim.private.json"
    late_candidate = read_json(late_private)
    if (
        hashlib.sha256(bytes.fromhex(late_candidate["raw_tx"])).hexdigest()
        != late["bitcoin_late_claim_candidate_sha256"]
    ):
        raise AssertionError("private late-claim candidate hash mismatch")
    late_now = run_json([str(NODE), str(BTC_OPS), "test", str(late_private)])
    if late_now.get("allowed") is not False:
        raise AssertionError("late claim is no longer rejected")

    result = {
        "schema": "postfiat.bitcoin_regtest_navcoin.independent_verification.v1",
        "result": "PASS",
        "claim": report["claim"],
        "reference_lane": report["reference_lane"],
        "bitcoin": {
            **bitcoin,
            "node_chain": chain["chain"],
            "node_height": chain["blocks"],
            "node_best_block_hash": chain["bestblockhash"],
            "merkle_inclusion_proofs": block_proofs,
        },
        "pftl": {
            "asset_id": ASSET_ID,
            "binary_sha256": binary_sha,
            "convergence": convergence,
            "escrow_states": observed_states,
            "conserved_navcoin_atoms": nav_total,
            "nav_per_unit_usd_e8": nav["nav_per_unit"],
            "verified_net_assets_usd_e8": reserve["verified_net_assets"],
            "reserve_account": reserve["reserve_accounts"][0],
        },
        "adversarial": {
            "wrong_preimage_mutation_free": True,
            "early_cancel_mutation_free": True,
            "late_finish_after_refund_mutation_free": True,
            "bitcoin_timeout_race_observed_without_broadcast": True,
            "duplicates_suppressed": True,
        },
        "public_preimages_authenticated": 2,
        "refund_preimage_revealed": False,
    }
    output = evidence / "independent-verification.json"
    write_json(output, result)

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
    write_json(
        manifest_path,
        {
            "schema": "postfiat.bitcoin_regtest_navcoin.evidence_manifest.v1",
            "artifact_count": len(artifacts),
            "artifacts": artifacts,
        },
    )
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
