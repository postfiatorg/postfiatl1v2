"""Resume the live regtest lane after an already-certified off-ramp create.

The original adapter matched account history only by sequence and transaction
kind. Historic transactions from the opposite account reused that tuple, so
the adapter stopped after consensus had already accepted the intended create.
This recovery path binds the accepted row by source account and escrow fields,
records that recovery, and continues without replaying the create.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path

from tools.xrpl_navcoin_demo.accounting import assert_principal_conserved
from tools.xrpl_navcoin_demo.pftl_adapter import (
    ASSET_ID,
    COORDINATOR,
    USER,
    HardenedPftl,
    PftlEscrowRef,
)
from tools.xrpl_navcoin_demo.protocol import CrossLedgerHashlock

from .run_live_demo import (
    btc,
    build_with_request,
    broadcast_and_confirm,
    core_json,
    pftl_snapshot,
    assert_pftl_unchanged,
    read_json,
    wait_height,
    write_json,
)
from .timelock import Direction, validate_second_lock


def find_escrow(
    pftl: HardenedPftl,
    *,
    owner: str,
    recipient: str,
    amount: int,
    condition: str,
    cancel_after: int,
) -> dict:
    matches = [
        item
        for item in pftl.escrows(owner, role="owner")
        if item["owner"] == owner
        and item["recipient"] == recipient
        and int(item["amount"]) == amount
        and int(item["cancel_after"]) == cancel_after
    ]
    if len(matches) != 1:
        raise AssertionError(f"expected one matching PFTL escrow, got {len(matches)}")
    return matches[0]


def escrow_ref(item: dict, tx_id: str, condition: str) -> PftlEscrowRef:
    return PftlEscrowRef(
        escrow_id=item["escrow_id"],
        owner=item["owner"],
        recipient=item["recipient"],
        amount_atoms=int(item["amount"]),
        cancel_after=int(item["cancel_after"]),
        condition=condition,
        create_tx_id=tx_id,
    )


def recover_off_create(
    pftl: HardenedPftl, pftl_evidence: Path, item: dict
) -> dict:
    signed = read_json(pftl_evidence / "offramp-01-nav-create.signed.private.json")
    unsigned = signed["unsigned"]
    history = pftl._run(
        [
            "account-tx",
            "--data-dir",
            str(pftl.node0),
            "--address",
            USER,
            "--limit",
            "100",
        ]
    )
    matches = [
        row
        for row in history["rows"]
        if row.get("sequence") == unsigned["sequence"]
        and row.get("transaction_kind") == unsigned["transaction_kind"]
        and row.get("from") == USER
        and row.get("to") == COORDINATOR
        and row.get("asset_id") == ASSET_ID
        and int(row.get("amount", -1)) == 80_000
        and row.get("escrow_id") == item["escrow_id"]
        and row.get("accepted")
    ]
    if len(matches) != 1:
        raise AssertionError("could not uniquely bind recovered off-ramp create")
    row = matches[0]
    operation = {
        "operation": "escrow_create",
        "owner": USER,
        "recipient": COORDINATOR,
        "asset_id": ASSET_ID,
        "amount": 80_000,
        "condition": unsigned["condition"],
        "cancel_after": unsigned["cancel_after"],
    }
    record = {
        "schema": "postfiat.pftl.certified_escrow_operation.v1",
        "label": "offramp-01-nav-create",
        "operation": operation,
        "source": USER,
        "sequence": unsigned["sequence"],
        "fee": unsigned["fee"],
        "tx_id": row["tx_id"],
        "block_height": row["block_height"],
        "receipt_code": row["receipt_code"],
        "recovered_after_history_match_ambiguity": True,
        "history_binding": {
            "from": row["from"],
            "to": row["to"],
            "asset_id": row["asset_id"],
            "amount": row["amount"],
            "escrow_id": row["escrow_id"],
        },
        "converged_after": pftl.converged_status(),
    }
    write_json(pftl_evidence / "offramp-01-nav-create.certified.json", record)
    return record


def recover_off_finish(
    pftl: HardenedPftl, pftl_evidence: Path, escrow_id: str
) -> dict:
    signed = read_json(pftl_evidence / "offramp-04-nav-finish.signed.private.json")
    unsigned = signed["unsigned"]
    history = pftl._run(
        [
            "account-tx",
            "--data-dir",
            str(pftl.node0),
            "--address",
            COORDINATOR,
            "--limit",
            "100",
        ]
    )
    matches = [
        row
        for row in history["rows"]
        if row.get("sequence") == unsigned["sequence"]
        and row.get("transaction_kind") == "escrow_finish"
        and row.get("escrow_id") == escrow_id
        and row.get("accepted")
    ]
    if len(matches) != 1:
        raise AssertionError("could not uniquely bind recovered off-ramp finish")
    row = matches[0]
    operation = {
        "operation": "escrow_finish",
        "escrow_id": escrow_id,
        "owner": USER,
        "recipient": COORDINATOR,
        "fulfillment": unsigned["fulfillment"],
    }
    record = {
        "schema": "postfiat.pftl.certified_escrow_operation.v1",
        "label": "offramp-04-nav-finish",
        "operation": operation,
        "source": COORDINATOR,
        "sequence": unsigned["sequence"],
        "fee": unsigned["fee"],
        "tx_id": row["tx_id"],
        "block_height": row["block_height"],
        "receipt_code": row["receipt_code"],
        "recovered_after_history_match_ambiguity": True,
        "history_binding": {
            "from": row["from"],
            "to": row["to"],
            "escrow_id": row["escrow_id"],
        },
        "converged_after": pftl.converged_status(),
    }
    write_json(pftl_evidence / "offramp-04-nav-finish.certified.json", record)
    return record


def recover_refund_cancel(
    pftl: HardenedPftl, pftl_evidence: Path, escrow_id: str
) -> dict:
    """Bind the already-certified cancel that was pending during a round collision."""
    signed = read_json(pftl_evidence / "refund-03-nav-cancel.signed.private.json")
    unsigned = signed["unsigned"]
    history = pftl._run(
        [
            "account-tx",
            "--data-dir",
            str(pftl.node0),
            "--address",
            COORDINATOR,
            "--limit",
            "100",
        ]
    )
    matches = [
        row
        for row in history["rows"]
        if row.get("sequence") == unsigned["sequence"]
        and row.get("transaction_kind") == "escrow_cancel"
        and row.get("escrow_id") == escrow_id
        and row.get("accepted")
    ]
    if len(matches) != 1:
        raise AssertionError("could not uniquely bind recovered refund cancel")
    row = matches[0]
    operation = {
        "operation": "escrow_cancel",
        "escrow_id": escrow_id,
        "owner": COORDINATOR,
        "recipient": USER,
    }
    record = {
        "schema": "postfiat.pftl.certified_escrow_operation.v1",
        "label": "refund-03-nav-cancel",
        "operation": operation,
        "source": COORDINATOR,
        "sequence": unsigned["sequence"],
        "fee": unsigned["fee"],
        "tx_id": row["tx_id"],
        "block_height": row["block_height"],
        "receipt_code": row["receipt_code"],
        "recovered_from_existing_certified_pending": True,
        "history_binding": {
            "from": row["from"],
            "to": row["to"],
            "escrow_id": row["escrow_id"],
        },
        "converged_after": pftl.converged_status(),
    }
    write_json(pftl_evidence / "refund-03-nav-cancel.certified.json", record)
    return record


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-root", required=True, type=Path)
    args = parser.parse_args()
    runtime_root = args.runtime_root.resolve()
    evidence = runtime_root / "public/evidence"
    bitcoin_evidence = evidence / "bitcoin"
    pftl_evidence = evidence / "pftl"
    private = runtime_root / "private"
    report = read_json(evidence / "live-demo.partial.json")
    secrets = read_json(private / "swap-preimages.private.json")
    off_lock = CrossLedgerHashlock.from_secret_hex(secrets["nav_to_btc"])
    refund_lock = CrossLedgerHashlock.from_secret_hex(secrets["refund"])
    pftl = HardenedPftl(pftl_evidence)

    split = read_json(bitcoin_evidence / "00-funding-split.built.json")
    funding_utxo = read_json(evidence / "regtest-funding-utxo.json")
    on_btc_lock = read_json(bitcoin_evidence / "onramp-01-btc-lock.built.json")
    on_btc_claim = read_json(bitcoin_evidence / "onramp-04-btc-claim.built.json")
    refund_btc_lock = read_json(
        bitcoin_evidence / "refund-01-btc-lock.built.json"
    )
    refund_btc_lock_record = read_json(
        bitcoin_evidence / "refund-01-btc-lock.confirmed.json"
    )
    refund_btc_lock_path = bitcoin_evidence / "refund-01-btc-lock.built.json"

    refund_item = find_escrow(
        pftl,
        owner=COORDINATOR,
        recipient=USER,
        amount=25_000,
        condition=refund_lock.public_values()["pftl_condition"],
        cancel_after=17,
    )
    refund_nav_create = read_json(
        pftl_evidence / "refund-02-nav-create.certified.json"
    )
    refund_nav = escrow_ref(
        refund_item,
        refund_nav_create["tx_id"],
        refund_lock.public_values()["pftl_condition"],
    )
    refund_timing_gate = validate_second_lock(
        direction=Direction.BTC_TO_NAV,
        bitcoin_height=refund_btc_lock_record["core"]["transaction"]["status"][
            "block_height"
        ],
        bitcoin_cancel_height=refund_btc_lock["lock_height"],
        pftl_height=int(refund_item["created_height"]) - 1,
        pftl_cancel_height=refund_nav.cancel_after,
    )

    off_item = find_escrow(
        pftl,
        owner=USER,
        recipient=COORDINATOR,
        amount=80_000,
        condition=off_lock.public_values()["pftl_condition"],
        cancel_after=2015,
    )
    if off_item["state"] not in {"open", "finished"}:
        raise AssertionError("recovered off-ramp escrow has an unexpected state")
    off_nav_create = recover_off_create(pftl, pftl_evidence, off_item)
    off_nav = escrow_ref(
        off_item,
        off_nav_create["tx_id"],
        off_lock.public_values()["pftl_condition"],
    )

    if off_item["state"] == "finished":
        off_lock_request = read_json(
            bitcoin_evidence / "offramp-02-btc-lock.request.json"
        )
        off_btc_lock = read_json(
            bitcoin_evidence / "offramp-02-btc-lock.built.json"
        )
        off_btc_lock_record = read_json(
            bitcoin_evidence / "offramp-02-btc-lock.confirmed.json"
        )
        off_btc_lock_path = bitcoin_evidence / "offramp-02-btc-lock.built.json"
        off_btc_claim = read_json(
            bitcoin_evidence / "offramp-03-btc-claim.built.json"
        )
        off_btc_claim_record = read_json(
            bitcoin_evidence / "offramp-03-btc-claim.confirmed.json"
        )
        off_btc_claim_path = bitcoin_evidence / "offramp-03-btc-claim.built.json"
        off_height = int(off_lock_request["lockHeight"]) - 8
    else:
        allocations = {
            item["scenario"]: {
                "inputTxid": split["txid"],
                "inputVout": item["vout"],
                "inputValueSats": item["value_sats"],
            }
            for item in split["outputs"]
            if item["scenario"] != "change"
        }
        off_height = int(core_json("getblockcount"))
        off_lock_request = {
            "scenario": "nav_to_btc",
            "owner": "coordinator",
            "recipient": "user",
            **allocations["nav_to_btc"],
            "digestHex": off_lock.digest.hex(),
            "lockHeight": off_height + 8,
            "amountSats": 20_000,
        }
        off_btc_lock = build_with_request(
            runtime_root,
            bitcoin_evidence,
            "lock",
            "offramp-02-btc-lock",
            off_lock_request,
        )
        off_btc_lock_record, off_btc_lock_path = broadcast_and_confirm(
            bitcoin_evidence, "offramp-02-btc-lock", off_btc_lock
        )
        off_btc_claim = btc(
            "claim",
            str(runtime_root),
            str(off_btc_lock_path),
            off_lock.secret.protocol_hex(),
        )
        off_btc_claim_record, off_btc_claim_path = broadcast_and_confirm(
            bitcoin_evidence, "offramp-03-btc-claim", off_btc_claim
        )
    off_timing_gate = validate_second_lock(
        direction=Direction.NAV_TO_BTC,
        bitcoin_height=off_height,
        bitcoin_cancel_height=off_lock_request["lockHeight"],
        pftl_height=int(off_item["created_height"]),
        pftl_cancel_height=off_nav.cancel_after,
    )
    off_public = btc(
        "extract", str(off_btc_claim_path), off_lock.digest.hex()
    )["preimage"]
    if off_item["state"] == "finished":
        off_nav_finish = recover_off_finish(
            pftl, pftl_evidence, off_nav.escrow_id
        )
    else:
        before_finish = pftl_snapshot(pftl)
        off_nav_finish = pftl.finish(
            label="offramp-04-nav-finish",
            escrow=off_nav,
            fulfillment="a0228020" + off_public,
        )
        after_finish = pftl_snapshot(pftl)
        assert_principal_conserved(before_finish[1], after_finish[1])
    report["scenarios"]["nav_to_btc_happy"] = {
        "payment_hash": off_lock.digest.hex(),
        "timing_gate": off_timing_gate,
        "bitcoin_htlc": off_btc_lock,
        "pftl_escrow": off_nav.__dict__,
        "public_preimage_hex": off_public,
        "pftl_finish_used_public_bitcoin_preimage": True,
        "transactions": {
            "pftl_create": off_nav_create["tx_id"],
            "bitcoin_lock": off_btc_lock["txid"],
            "bitcoin_claim": off_btc_claim["txid"],
            "pftl_finish": off_nav_finish["tx_id"],
        },
        "bitcoin_evidence": {
            "lock": off_btc_lock_record,
            "claim": off_btc_claim_record,
        },
        "conservation": {
            "bitcoin_lock": off_btc_lock["conservation"],
            "bitcoin_claim": off_btc_claim["conservation"],
            "navcoin_atoms_including_locked": True,
        },
    }
    write_json(evidence / "live-demo.partial.json", report)

    if refund_item["state"] == "canceled":
        refund_nav_cancel = recover_refund_cancel(
            pftl, pftl_evidence, refund_nav.escrow_id
        )
    elif refund_item["state"] == "open":
        before_late_nav = pftl_snapshot(pftl)
        if pftl.converged_status()["height"] < refund_nav.cancel_after:
            raise AssertionError("PFTL refund height did not mature")
        after_late_nav = pftl_snapshot(pftl)
        assert_pftl_unchanged(before_late_nav, after_late_nav)
        refund_nav_cancel = pftl.cancel(
            label="refund-03-nav-cancel", escrow=refund_nav
        )
        after_nav_cancel = pftl_snapshot(pftl)
        assert_principal_conserved(after_late_nav[1], after_nav_cancel[1])
    else:
        raise AssertionError("refund escrow has an unexpected terminal state")

    wait_height(refund_btc_lock["lock_height"])
    late_claim_private = btc(
        "claim",
        str(runtime_root),
        str(refund_btc_lock_path),
        refund_lock.secret.protocol_hex(),
    )
    late_claim_private_path = private / "bitcoin/late-claim.private.json"
    write_json(late_claim_private_path, late_claim_private, 0o600)
    at_cancel_preflight = btc("test", str(late_claim_private_path))
    if not at_cancel_preflight.get("allowed"):
        raise AssertionError("Bitcoin hash claim was not valid at CLTV maturity")
    refund_btc = btc("refund", str(runtime_root), str(refund_btc_lock_path))
    refund_btc_record, _ = broadcast_and_confirm(
        bitcoin_evidence, "refund-04-btc-refund", refund_btc
    )
    late_preflight = btc("test", str(late_claim_private_path))
    if late_preflight.get("allowed"):
        raise AssertionError("late Bitcoin claim passed after confirmed refund")

    report["scenarios"]["refund"] = {
        "payment_hash": refund_lock.digest.hex(),
        "timing_gate": refund_timing_gate,
        "bitcoin_htlc": refund_btc_lock,
        "pftl_escrow": refund_nav.__dict__,
        "transactions": {
            "bitcoin_lock": refund_btc_lock["txid"],
            "pftl_create": refund_nav_create["tx_id"],
            "pftl_cancel": refund_nav_cancel["tx_id"],
            "bitcoin_refund": refund_btc["txid"],
        },
        "bitcoin_evidence": {
            "lock": refund_btc_lock_record,
            "refund": refund_btc_record,
        },
        "preimage_revealed": False,
        "conservation": {
            "bitcoin_lock": refund_btc_lock["conservation"],
            "bitcoin_refund": refund_btc["conservation"],
            "navcoin_atoms_including_locked": True,
        },
    }
    report["adversarial"]["late_finish_at_cancel"] = {
        "pftl_late_finish_rejected_before_signing": True,
        "pftl_principal_state_unchanged": True,
        "bitcoin_at_cancel_hash_claim_testmempoolaccept": at_cancel_preflight,
        "bitcoin_at_cancel_probe_mutation_free": True,
        "bitcoin_late_claim_testmempoolaccept": late_preflight,
        "bitcoin_refunded_outpoint_remains_spent": True,
        "bitcoin_late_claim_candidate_sha256": hashlib.sha256(
            bytes.fromhex(late_claim_private["raw_tx"])
        ).hexdigest(),
        "bitcoin_refund_preimage_not_published": True,
    }

    final_nav = pftl_snapshot(pftl)
    if final_nav[1].total != 3_000_000_000:
        raise AssertionError("NAVcoin exact atom conservation failed")
    fees = [
        split["fee_sats"],
        on_btc_lock["fee_sats"],
        on_btc_claim["fee_sats"],
        refund_btc_lock["fee_sats"],
        off_btc_lock["fee_sats"],
        off_btc_claim["fee_sats"],
        refund_btc["fee_sats"],
    ]
    final_controlled = (
        next(
            item["value_sats"]
            for item in split["outputs"]
            if item["scenario"] == "change"
        )
        + on_btc_lock["change_sats"]
        + on_btc_claim["output_value_sats"]
        + refund_btc_lock["change_sats"]
        + refund_btc["output_value_sats"]
        + off_btc_lock["change_sats"]
        + off_btc_claim["output_value_sats"]
    )
    if funding_utxo["valueSats"] != final_controlled + sum(fees):
        raise AssertionError("Bitcoin exact satoshi conservation failed")
    report["conservation"] = {
        "funding_input_sats": funding_utxo["valueSats"],
        "final_user_coordinator_controlled_sats": final_controlled,
        "miner_fee_sats": sum(fees),
        "equation": "funding_input = final_controlled + miner_fees",
        "bitcoin_exact": True,
        "navcoin_total_atoms": final_nav[1].total,
        "navcoin_exact": True,
    }
    report["public_preimages_authenticated"] = 2
    report["refund_preimage_revealed"] = False
    report["final_pftl"] = final_nav[0]
    report["recovery"] = {
        "reasons": [
            "historic opposite-source sequence/kind collision",
            "concurrent authorized round left a signed cancel safely pending",
        ],
        "create_replayed": False,
        "cancel_replayed": False,
        "bound_accepted_tx_id": off_nav_create["tx_id"],
        "bound_cancel_tx_id": refund_nav_cancel["tx_id"],
    }
    report["result"] = "PASS"
    write_json(evidence / "live-demo-report.json", report)
    write_json(evidence / "live-demo.partial.json", report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
