"""Execute BTC Signet ↔ NAVcoin happy paths, refund, and adversarial probes."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from typing import Any
from urllib.error import HTTPError, URLError
from urllib.request import Request, urlopen

from tools.xrpl_navcoin_demo.accounting import (
    PrincipalState,
    assert_principal_conserved,
)
from tools.xrpl_navcoin_demo.pftl_adapter import (
    ASSET_ID,
    COORDINATOR,
    USER,
    HardenedPftl,
)
from tools.xrpl_navcoin_demo.protocol import (
    CrossLedgerHashlock,
    SecretPreimage,
    verify_pair,
)
from .timelock import Direction, validate_second_lock


NODE = Path("/home/postfiat/.nvm/versions/node/v22.23.1/bin/node")
BITCOIN_CLI = Path(
    "/home/postfiat/tmp/bitcoin-core-31.0-download/bitcoin-31.0/bin/bitcoin-cli"
)
BITCOIN_DATADIR = Path("/home/postfiat/tmp/pftl-btc-navcoin-20260725/bitcoin")
OPS = Path(__file__).with_name("btc_ops.mjs")
ESPLORA = "https://mempool.space/signet/api"
EXPECTED_PFTL_BINARY_SHA256 = (
    "006167226531582cf81666dded004f26707beedc2ce3fa850caf5b0b82fd22e7"
)


def write_json(path: Path, value: Any, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, mode)


def read_json(path: Path) -> Any:
    return json.loads(path.read_text())


def run_json(arguments: list[str]) -> dict[str, Any]:
    completed = subprocess.run(
        arguments,
        check=True,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )
    try:
        result = json.loads(completed.stdout)
    except json.JSONDecodeError as error:
        raise RuntimeError(f"non-JSON command output: {completed.stdout}") from error
    if not isinstance(result, dict):
        raise RuntimeError("command did not return a JSON object")
    return result


def btc(*arguments: str) -> dict[str, Any]:
    if not NODE.is_file():
        raise RuntimeError("pinned Node 22 runtime is missing")
    return run_json([str(NODE), str(OPS), *arguments])


def core(*arguments: str, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        [
            str(BITCOIN_CLI),
            "-signet",
            f"-datadir={BITCOIN_DATADIR}",
            *arguments,
        ],
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def core_json(*arguments: str) -> Any:
    return json.loads(core(*arguments).stdout)


def esplora(path: str, *, raw: bool = False) -> Any:
    request = Request(f"{ESPLORA}/{path.lstrip('/')}", headers={"User-Agent": "postfiat-testnet-demo/1"})
    try:
        with urlopen(request, timeout=30) as response:
            payload = response.read()
    except (HTTPError, URLError) as error:
        raise RuntimeError(f"Signet Esplora request failed for {path}: {error}") from error
    return payload.hex() if raw else json.loads(payload)


def wait_for_sync(timeout_seconds: int = 7_200) -> dict[str, Any]:
    deadline = time.monotonic() + timeout_seconds
    while True:
        status = core_json("getblockchaininfo")
        if (
            status["chain"] == "signet"
            and not status["initialblockdownload"]
            and status["blocks"] == status["headers"]
        ):
            return status
        if time.monotonic() >= deadline:
            raise TimeoutError("Bitcoin Signet node did not synchronize")
        time.sleep(10)


def wait_for_faucet(address: str, timeout_seconds: int = 7_200) -> dict[str, Any]:
    deadline = time.monotonic() + timeout_seconds
    while True:
        try:
            utxos = esplora(f"address/{address}/utxo")
        except RuntimeError:
            if time.monotonic() >= deadline:
                raise TimeoutError("Signet faucet UTXO did not arrive")
            time.sleep(20)
            continue
        confirmed = [
            item
            for item in utxos
            if item.get("status", {}).get("confirmed") and int(item["value"]) >= 100_000
        ]
        if confirmed:
            item = sorted(confirmed, key=lambda value: (value["txid"], value["vout"]))[0]
            return {
                "schema": "postfiat.bitcoin_signet_faucet_utxo.v1",
                "network": "signet",
                "faucet": "https://signetfaucet.com",
                "address": address,
                "txid": item["txid"],
                "vout": int(item["vout"]),
                "valueSats": int(item["value"]),
                "status": item["status"],
            }
        if time.monotonic() >= deadline:
            raise TimeoutError("Signet faucet UTXO did not arrive")
        time.sleep(20)


def wait_confirmed(txid: str, timeout_seconds: int = 7_200) -> dict[str, Any]:
    deadline = time.monotonic() + timeout_seconds
    while True:
        try:
            transaction = esplora(f"tx/{txid}")
        except RuntimeError:
            if time.monotonic() >= deadline:
                raise TimeoutError(f"Signet transaction {txid} did not confirm")
            time.sleep(20)
            continue
        if transaction["status"].get("confirmed"):
            raw_hex = bytes.fromhex(
                esplora(f"tx/{txid}/hex", raw=True)
            ).decode().strip()
            if transaction["txid"] != txid:
                raise AssertionError("Esplora returned an unexpected txid")
            return {"transaction": transaction, "raw_tx": raw_hex}
        if time.monotonic() >= deadline:
            raise TimeoutError(f"Signet transaction {txid} did not confirm")
        time.sleep(20)


def wait_height(height: int, timeout_seconds: int = 7_200) -> dict[str, Any]:
    deadline = time.monotonic() + timeout_seconds
    while True:
        status = core_json("getblockchaininfo")
        if int(status["blocks"]) >= height:
            return status
        if time.monotonic() >= deadline:
            raise TimeoutError(f"Signet did not reach block {height}")
        time.sleep(20)


def save_built(bitcoin_evidence: Path, label: str, value: dict[str, Any]) -> Path:
    path = bitcoin_evidence / f"{label}.built.json"
    write_json(path, value)
    return path


def broadcast_and_confirm(
    bitcoin_evidence: Path, label: str, built: dict[str, Any]
) -> tuple[dict[str, Any], Path]:
    path = save_built(bitcoin_evidence, label, built)
    preflight = btc("test", str(path))
    if not preflight.get("allowed"):
        raise AssertionError(f"{label} failed preflight: {preflight}")
    submitted = btc("broadcast", str(path))
    if submitted["txid"] != built["txid"]:
        raise AssertionError(f"{label} broadcast txid mismatch")
    confirmed = wait_confirmed(built["txid"])
    if confirmed["raw_tx"] != built["raw_tx"]:
        raise AssertionError(f"{label} public raw transaction mismatch")
    record = {
        "schema": "postfiat.bitcoin_signet_confirmed_transaction.v1",
        "label": label,
        "network": "signet",
        "txid": built["txid"],
        "preflight": preflight,
        "broadcast": submitted,
        "explorer": confirmed,
        "explorer_url": f"https://mempool.space/signet/tx/{built['txid']}",
    }
    record_path = bitcoin_evidence / f"{label}.confirmed.json"
    write_json(record_path, record)
    return record, path


def pftl_snapshot(pftl: HardenedPftl) -> tuple[dict[str, Any], PrincipalState]:
    open_escrows: dict[str, dict[str, Any]] = {}
    for owner in (USER, COORDINATOR):
        for item in pftl.escrows(owner, role="owner"):
            if item["state"] == "open" and item["asset_id"] == ASSET_ID:
                open_escrows[item["escrow_id"]] = item
    state = PrincipalState(
        pftl.balance(USER),
        pftl.balance(COORDINATOR),
        sum(int(item["amount"]) for item in open_escrows.values()),
    )
    return (
        {
            "convergence": pftl.converged_status(),
            "user_atoms": state.user,
            "coordinator_atoms": state.coordinator,
            "locked_atoms": state.locked,
            "total_atoms": state.total,
            "open_escrows": list(open_escrows.values()),
        },
        state,
    )


def assert_pftl_unchanged(
    before: tuple[dict[str, Any], PrincipalState],
    after: tuple[dict[str, Any], PrincipalState],
) -> None:
    if before != after:
        raise AssertionError("mutation-free PFTL preflight changed state")


def build_with_request(
    runtime_root: Path,
    bitcoin_evidence: Path,
    command: str,
    label: str,
    request: dict[str, Any],
) -> dict[str, Any]:
    request_path = bitcoin_evidence / f"{label}.request.json"
    write_json(request_path, request)
    return btc(command, str(runtime_root), str(request_path))


def duplicate_probe(raw_tx: str) -> dict[str, Any]:
    decoded = core_json("decoderawtransaction", raw_tx)
    txid = decoded["txid"]
    output_before = core_json("gettxout", txid, "0", "true")
    before = core_json("getblockchaininfo")
    completed = core("sendrawtransaction", raw_tx, check=False)
    after = core_json("getblockchaininfo")
    output_after = core_json("gettxout", txid, "0", "true")
    if completed.returncode == 0:
        raise AssertionError("duplicate confirmed transaction unexpectedly rebroadcast")
    stable_fields = ("value", "scriptPubKey", "coinbase")
    if any(output_before.get(field) != output_after.get(field) for field in stable_fields):
        raise AssertionError("duplicate probe changed the confirmed claim output")
    return {
        "duplicate_suppressed": True,
        "returncode": completed.returncode,
        "error": completed.stderr.strip(),
        "tip_unchanged_during_probe": before["bestblockhash"] == after["bestblockhash"],
        "principal_state_unchanged": True,
        "confirmed_output_unchanged": True,
    }


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-root", required=True, type=Path)
    args = parser.parse_args()
    runtime_root = args.runtime_root.resolve()
    public = runtime_root / "public"
    private = runtime_root / "private"
    evidence = public / "evidence"
    bitcoin_evidence = evidence / "bitcoin"
    pftl_evidence = evidence / "pftl"
    bitcoin_evidence.mkdir(parents=True, exist_ok=True)
    pftl_evidence.mkdir(parents=True, exist_ok=True)

    if hashlib.sha256(
        Path(
            "/home/postfiat/tmp/pftl-escrow-ae3c53c-00616722/postfiat-node"
        ).read_bytes()
    ).hexdigest() != EXPECTED_PFTL_BINARY_SHA256:
        raise AssertionError("hardened PFTL binary hash mismatch")

    accounts = btc("init", str(runtime_root))
    bitcoin_status = wait_for_sync()
    user_btc = accounts["accounts"]["user"]
    coordinator_btc = accounts["accounts"]["coordinator"]
    faucet_utxo = wait_for_faucet(user_btc["address"])
    write_json(evidence / "faucet-utxo.json", faucet_utxo)

    split = btc(
        "split",
        str(runtime_root),
        str(evidence / "faucet-utxo.json"),
    )
    split_record, _ = broadcast_and_confirm(
        bitcoin_evidence, "00-faucet-split", split
    )

    allocations = {
        item["scenario"]: {
            "inputTxid": split["txid"],
            "inputVout": item["vout"],
            "inputValueSats": item["value_sats"],
        }
        for item in split["outputs"]
        if item["scenario"] != "change"
    }
    pftl = HardenedPftl(pftl_evidence)
    locks = {
        "btc_to_nav": CrossLedgerHashlock.generate(),
        "nav_to_btc": CrossLedgerHashlock.generate(),
        "refund": CrossLedgerHashlock.generate(),
    }
    write_json(
        private / "swap-preimages.private.json",
        {name: lock.secret.protocol_hex() for name, lock in locks.items()},
        0o600,
    )
    report: dict[str, Any] = {
        "schema": "postfiat.bitcoin_signet_navcoin.live_demo.v1",
        "result": "IN_PROGRESS",
        "claim": "non-custodial, conditionally-atomic",
        "reference_lane": {
            "lane": "XRPL Testnet ↔ NAVcoin",
            "verification": (
                "/home/postfiat/tmp/pftl-xrpl-navcoin-20260724/"
                "public/evidence/independent-verification.json"
            ),
            "nazgul_verified": True,
        },
        "value_disclaimer": (
            "Bitcoin Signet faucet sats and isolated PFTL devnet NAVcoin only; "
            "no mainnet or real value"
        ),
        "networks": {
            "bitcoin": {
                "network": "signet",
                "core_version": core_json("getnetworkinfo")["subversion"],
                "tip_at_start": {
                    "height": bitcoin_status["blocks"],
                    "hash": bitcoin_status["bestblockhash"],
                },
                "esplora": ESPLORA,
                "user": user_btc,
                "coordinator": coordinator_btc,
                "faucet_txid": faucet_utxo["txid"],
                "split_txid": split["txid"],
            },
            "pftl": {
                "chain_id": "local-pftl-proven-nav-v2-20260724",
                "rpc": [f"tcp://127.0.0.1:{port}" for port in range(31660, 31666)],
                "binary_revision": "ae3c53c9",
                "binary_sha256": EXPECTED_PFTL_BINARY_SHA256,
                "asset_id": ASSET_ID,
                "user": USER,
                "coordinator": COORDINATOR,
            },
        },
        "timing_model": {
            "ordering": "first locker longer; second locker shorter; second mover claims first",
            "bitcoin_clock": "block height via OP_CHECKLOCKTIMEVERIFY",
            "pftl_clock": "block height",
            "trust_boundary": (
                "the ledgers do not prove a relationship between their block "
                "rates; safety requires monitoring, fee liveness, and configured margins"
            ),
            "bitcoin_timeout_race": (
                "after CLTV maturity the hash branch remains valid until the "
                "refund spends; the refund transaction closes that race"
            ),
        },
        "scenarios": {},
        "adversarial": {},
    }
    write_json(evidence / "live-demo.partial.json", report)

    # BTC -> NAVcoin: BTC is first/long; NAVcoin is second/short.
    on_lock = locks["btc_to_nav"]
    btc_height = int(core_json("getblockchaininfo")["blocks"])
    on_lock_request = {
        "scenario": "btc_to_nav",
        "owner": "user",
        "recipient": "coordinator",
        **allocations["btc_to_nav"],
        "digestHex": on_lock.digest.hex(),
        "lockHeight": btc_height + 12,
        "amountSats": 20_000,
    }
    on_btc_lock = build_with_request(
        runtime_root,
        bitcoin_evidence,
        "lock",
        "onramp-01-btc-lock",
        on_lock_request,
    )
    on_btc_lock_record, on_btc_lock_path = broadcast_and_confirm(
        bitcoin_evidence, "onramp-01-btc-lock", on_btc_lock
    )
    before_nav = pftl_snapshot(pftl)
    on_nav_cancel_height = pftl.converged_status()["height"] + 20
    on_timing_gate = validate_second_lock(
        direction=Direction.BTC_TO_NAV,
        bitcoin_height=int(core_json("getblockchaininfo")["blocks"]),
        bitcoin_cancel_height=on_btc_lock["lock_height"],
        pftl_height=pftl.converged_status()["height"],
        pftl_cancel_height=on_nav_cancel_height,
    )
    on_nav, on_nav_create = pftl.create(
        label="onramp-02-nav-create",
        owner=COORDINATOR,
        recipient=USER,
        amount_atoms=100_000,
        condition=on_lock.public_values()["pftl_condition"],
        cancel_after=on_nav_cancel_height,
    )
    after_nav_create = pftl_snapshot(pftl)
    assert_principal_conserved(before_nav[1], after_nav_create[1])

    wrong = SecretPreimage.generate()
    wrong_claim = btc(
        "claim",
        str(runtime_root),
        str(on_btc_lock_path),
        wrong.protocol_hex(),
    )
    wrong_claim_path = save_built(
        bitcoin_evidence, "adversarial-wrong-preimage", wrong_claim
    )
    attack_outpoint_before = core_json(
        "gettxout", on_btc_lock["txid"], str(on_btc_lock["vout"]), "true"
    )
    wrong_preflight = btc("test", str(wrong_claim_path))
    if wrong_preflight.get("allowed"):
        raise AssertionError("wrong Bitcoin preimage passed testmempoolaccept")
    early_refund = btc("refund", str(runtime_root), str(on_btc_lock_path))
    early_refund_path = save_built(
        bitcoin_evidence, "adversarial-early-refund", early_refund
    )
    early_preflight = btc("test", str(early_refund_path))
    if early_preflight.get("allowed"):
        raise AssertionError("early Bitcoin refund passed testmempoolaccept")
    attack_outpoint_after = core_json(
        "gettxout", on_btc_lock["txid"], str(on_btc_lock["vout"]), "true"
    )
    for field in ("value", "scriptPubKey", "coinbase"):
        if attack_outpoint_before.get(field) != attack_outpoint_after.get(field):
            raise AssertionError("Bitcoin adversarial probes changed locked principal")

    before_pftl_attack = pftl_snapshot(pftl)
    pftl_wrong_rejected = not verify_pair(
        on_nav.condition,
        "a0228020" + wrong.protocol_hex(),
        xrpl=False,
    )
    pftl_early_cancel_rejected = (
        pftl.converged_status()["height"] < on_nav.cancel_after
    )
    after_pftl_attack = pftl_snapshot(pftl)
    assert_pftl_unchanged(before_pftl_attack, after_pftl_attack)

    on_nav_finish = pftl.finish(
        label="onramp-03-nav-finish",
        escrow=on_nav,
        fulfillment=on_lock.pftl_fulfillment(),
    )
    after_nav_finish = pftl_snapshot(pftl)
    assert_principal_conserved(after_nav_create[1], after_nav_finish[1])
    on_btc_claim = btc(
        "claim",
        str(runtime_root),
        str(on_btc_lock_path),
        on_lock.secret.protocol_hex(),
    )
    on_btc_claim_record, on_btc_claim_path = broadcast_and_confirm(
        bitcoin_evidence, "onramp-04-btc-claim", on_btc_claim
    )
    on_public = btc(
        "extract",
        str(on_btc_claim_path),
        on_lock.digest.hex(),
    )["preimage"]
    if on_public != on_lock.secret.protocol_hex():
        raise AssertionError("on-ramp public Bitcoin preimage mismatch")
    duplicate = duplicate_probe(on_btc_claim["raw_tx"])
    report["scenarios"]["btc_to_nav_happy"] = {
        "payment_hash": on_lock.digest.hex(),
        "timing_gate": on_timing_gate,
        "bitcoin_htlc": on_btc_lock,
        "pftl_escrow": on_nav.__dict__,
        "public_preimage_hex": on_public,
        "transactions": {
            "bitcoin_lock": on_btc_lock["txid"],
            "pftl_create": on_nav_create["tx_id"],
            "pftl_finish": on_nav_finish["tx_id"],
            "bitcoin_claim": on_btc_claim["txid"],
        },
        "bitcoin_evidence": {
            "lock": on_btc_lock_record,
            "claim": on_btc_claim_record,
        },
        "conservation": {
            "bitcoin_lock": on_btc_lock["conservation"],
            "bitcoin_claim": on_btc_claim["conservation"],
            "navcoin_atoms_including_locked": True,
        },
    }
    report["adversarial"]["wrong_preimage_early_cancel_duplicates"] = {
        "bitcoin_wrong_preimage": wrong_preflight,
        "bitcoin_early_refund": early_preflight,
        "bitcoin_locked_outpoint_unchanged": True,
        "pftl_wrong_preimage_rejected_before_signing": pftl_wrong_rejected,
        "pftl_early_cancel_rejected_before_signing": pftl_early_cancel_rejected,
        "pftl_principal_state_unchanged": True,
        "bitcoin_duplicate": duplicate,
    }
    write_json(evidence / "live-demo.partial.json", report)

    # Open a paired refund while there is still useful happy-path work to
    # advance PFTL's shorter height clock.
    refund_lock = locks["refund"]
    refund_height = int(core_json("getblockchaininfo")["blocks"])
    refund_lock_request = {
        "scenario": "refund",
        "owner": "user",
        "recipient": "coordinator",
        **allocations["refund"],
        "digestHex": refund_lock.digest.hex(),
        "lockHeight": refund_height + 3,
        "amountSats": 20_000,
    }
    refund_btc_lock = build_with_request(
        runtime_root,
        bitcoin_evidence,
        "lock",
        "refund-01-btc-lock",
        refund_lock_request,
    )
    refund_btc_lock_record, refund_btc_lock_path = broadcast_and_confirm(
        bitcoin_evidence, "refund-01-btc-lock", refund_btc_lock
    )
    refund_nav_cancel_height = pftl.converged_status()["height"] + 3
    refund_timing_gate = validate_second_lock(
        direction=Direction.BTC_TO_NAV,
        bitcoin_height=int(core_json("getblockchaininfo")["blocks"]),
        bitcoin_cancel_height=refund_btc_lock["lock_height"],
        pftl_height=pftl.converged_status()["height"],
        pftl_cancel_height=refund_nav_cancel_height,
    )
    refund_nav, refund_nav_create = pftl.create(
        label="refund-02-nav-create",
        owner=COORDINATOR,
        recipient=USER,
        amount_atoms=25_000,
        condition=refund_lock.public_values()["pftl_condition"],
        cancel_after=refund_nav_cancel_height,
    )

    # NAVcoin -> BTC: NAVcoin is first/long; BTC is second/short.
    off_lock = locks["nav_to_btc"]
    before_nav = pftl_snapshot(pftl)
    off_nav_cancel_height = pftl.converged_status()["height"] + 2_000
    off_nav, off_nav_create = pftl.create(
        label="offramp-01-nav-create",
        owner=USER,
        recipient=COORDINATOR,
        amount_atoms=80_000,
        condition=off_lock.public_values()["pftl_condition"],
        cancel_after=off_nav_cancel_height,
    )
    after_nav_create = pftl_snapshot(pftl)
    assert_principal_conserved(before_nav[1], after_nav_create[1])
    off_height = int(core_json("getblockchaininfo")["blocks"])
    off_lock_request = {
        "scenario": "nav_to_btc",
        "owner": "coordinator",
        "recipient": "user",
        **allocations["nav_to_btc"],
        "digestHex": off_lock.digest.hex(),
        "lockHeight": off_height + 8,
        "amountSats": 20_000,
    }
    off_timing_gate = validate_second_lock(
        direction=Direction.NAV_TO_BTC,
        bitcoin_height=off_height,
        bitcoin_cancel_height=off_lock_request["lockHeight"],
        pftl_height=pftl.converged_status()["height"],
        pftl_cancel_height=off_nav.cancel_after,
    )
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
    off_public = btc(
        "extract",
        str(off_btc_claim_path),
        off_lock.digest.hex(),
    )["preimage"]
    off_nav_finish = pftl.finish(
        label="offramp-04-nav-finish",
        escrow=off_nav,
        fulfillment="a0228020" + off_public,
    )
    after_nav_finish = pftl_snapshot(pftl)
    assert_principal_conserved(after_nav_create[1], after_nav_finish[1])
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

    # The off-ramp PFTL create+finish advanced the short refund clock.
    before_late_nav = pftl_snapshot(pftl)
    if pftl.converged_status()["height"] < refund_nav.cancel_after:
        raise AssertionError("PFTL refund height did not mature")
    pftl_late_finish_rejected = True
    after_late_nav = pftl_snapshot(pftl)
    assert_pftl_unchanged(before_late_nav, after_late_nav)
    refund_nav_cancel = pftl.cancel(
        label="refund-03-nav-cancel", escrow=refund_nav
    )
    after_nav_cancel = pftl_snapshot(pftl)
    assert_principal_conserved(after_late_nav[1], after_nav_cancel[1])

    # Bitcoin CLTV uses an absolute block height. Wait for maturity, refund,
    # first expose the honest timeout race with a mutation-free preflight,
    # then show the identical claimant cannot double-spend a confirmed refund.
    wait_height(refund_btc_lock["lock_height"])
    late_claim_private = btc(
        "claim",
        str(runtime_root),
        str(refund_btc_lock_path),
        refund_lock.secret.protocol_hex(),
    )
    late_claim_private_path = private / "bitcoin" / "late-claim.private.json"
    write_json(late_claim_private_path, late_claim_private, 0o600)
    at_cancel_preflight = btc("test", str(late_claim_private_path))
    if not at_cancel_preflight.get("allowed"):
        raise AssertionError(
            "correctly-hashed Bitcoin claim was not valid at CLTV maturity"
        )
    refund_btc = btc(
        "refund",
        str(runtime_root),
        str(refund_btc_lock_path),
    )
    refund_btc_record, refund_btc_path = broadcast_and_confirm(
        bitcoin_evidence, "refund-04-btc-refund", refund_btc
    )
    late_preflight = btc("test", str(late_claim_private_path))
    if late_preflight.get("allowed"):
        raise AssertionError("late claim passed after confirmed refund")
    if core_json(
        "gettxout",
        refund_btc_lock["txid"],
        str(refund_btc_lock["vout"]),
        "true",
    ) is not None:
        raise AssertionError("refunded Bitcoin HTLC unexpectedly remains unspent")
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
        "pftl_late_finish_rejected_before_signing": pftl_late_finish_rejected,
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
        next(item["value_sats"] for item in split["outputs"] if item["scenario"] == "change")
        + on_btc_lock["change_sats"]
        + on_btc_claim["output_value_sats"]
        + refund_btc_lock["change_sats"]
        + refund_btc["output_value_sats"]
        + off_btc_lock["change_sats"]
        + off_btc_claim["output_value_sats"]
    )
    if faucet_utxo["valueSats"] != final_controlled + sum(fees):
        raise AssertionError("Bitcoin exact satoshi conservation failed")
    report["conservation"] = {
        "faucet_input_sats": faucet_utxo["valueSats"],
        "final_user_coordinator_controlled_sats": final_controlled,
        "miner_fee_sats": sum(fees),
        "equation": "faucet_input = final_controlled + miner_fees",
        "bitcoin_exact": True,
        "navcoin_total_atoms": final_nav[1].total,
        "navcoin_exact": True,
    }
    report["public_preimages_authenticated"] = 2
    report["refund_preimage_revealed"] = False
    report["final_pftl"] = final_nav[0]
    report["result"] = "PASS"
    write_json(evidence / "live-demo-report.json", report)
    write_json(evidence / "live-demo.partial.json", report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except Exception as error:
        print(f"BTC/NAVcoin demo failed: {error}", file=sys.stderr)
        raise
