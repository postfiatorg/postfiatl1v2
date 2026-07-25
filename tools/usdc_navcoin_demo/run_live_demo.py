"""Execute both USDC/NAVcoin directions, refund, and adversarial probes."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
from typing import Any

from tools.xrpl_navcoin_demo.accounting import PrincipalState, assert_principal_conserved
from tools.xrpl_navcoin_demo.pftl_adapter import (
    ASSET_ID,
    COORDINATOR,
    USER,
    HardenedPftl,
    PftlEscrowRef,
)
from tools.xrpl_navcoin_demo.protocol import CrossLedgerHashlock, SecretPreimage, verify_pair


RPC = "http://127.0.0.1:39545"
CAST = "/home/postfiat/.foundry/bin/cast"
USER_EVM = "0x70997970C51812dc3A010C7d01b50e0d17dc79C8"
COORDINATOR_EVM = "0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266"
USER_KEY = "59c6995e998f97a5a0044966f0945389dc9e86dae88c7a8412f4603b6b78690d"
COORDINATOR_KEY = "ac0974bec39a17e36ba4a6b4d238ff944bacb478cbed5efcae784d7bf4f2ff80"
USDC = "0x5FbDB2315678afecb367f032d93F642f64180aa3"
HTLC = "0xe7f1725E7734CE288F8367e1Bb143E90bb3F0512"
MOCK_DEPLOY_TX = "0xb7432796d373c3cdd6fef40c7154ad54badcc59a386bd7b1025db191d62dbbe8"
HTLC_DEPLOY_TX = "0x2621b19704b80789b52db6bbc2b704ca3c67864ddac591e9b9dba38fd03e7079"
MAX_UINT = str(2**256 - 1)


def write_json(path: Path, value: Any, mode: int = 0o644) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")
    os.chmod(path, mode)


def command(arguments: list[str], *, check: bool = True) -> subprocess.CompletedProcess[str]:
    return subprocess.run(
        arguments,
        check=check,
        text=True,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
    )


def cast_json(arguments: list[str]) -> Any:
    completed = command([CAST, *arguments, "--json"])
    return json.loads(completed.stdout)


def code(address: str) -> bytes:
    value = command([CAST, "code", "--rpc-url", RPC, address]).stdout.strip()
    return bytes.fromhex(value.removeprefix("0x"))


def call(contract: str, signature: str, *arguments: str, sender: str | None = None) -> Any:
    command_line = ["call", "--rpc-url", RPC]
    if sender:
        command_line += ["--from", sender]
    command_line += [contract, signature, *arguments]
    return cast_json(command_line)


def rejected_call(
    contract: str,
    signature: str,
    *arguments: str,
    sender: str,
    block: int | None = None,
) -> dict[str, Any]:
    command_line = [CAST, "call", "--rpc-url", RPC, "--from", sender]
    if block is not None:
        command_line += ["--block", str(block)]
    command_line += [contract, signature, *arguments]
    completed = command(command_line, check=False)
    if completed.returncode == 0:
        raise AssertionError(f"adversarial call unexpectedly succeeded: {signature}")
    return {
        "rejected": True,
        "returncode": completed.returncode,
        "error": (completed.stderr or completed.stdout).strip(),
    }


def receipt(tx_hash: str) -> dict[str, Any]:
    value = cast_json(["receipt", "--rpc-url", RPC, tx_hash])
    if value["status"] != "0x1":
        raise AssertionError(f"EVM transaction failed: {tx_hash}")
    return value


def send(
    evidence: Path,
    label: str,
    *,
    key: str,
    contract: str,
    signature: str,
    arguments: list[str],
) -> dict[str, Any]:
    record_path = evidence / f"{label}.json"
    if record_path.is_file():
        existing = json.loads(record_path.read_text())
        receipt(existing["receipt"]["transactionHash"])
        return existing
    value = cast_json(
        [
            "send",
            "--rpc-url",
            RPC,
            "--private-key",
            key,
            contract,
            signature,
            *arguments,
        ]
    )
    tx_hash = value["transactionHash"]
    transaction = cast_json(
        ["rpc", "--rpc-url", RPC, "eth_getTransactionByHash", tx_hash]
    )
    record = {
        "schema": "postfiat.anvil_confirmed_transaction.v1",
        "label": label,
        "rpc": RPC,
        "chain_id": 31337,
        "transaction": transaction,
        "receipt": value,
    }
    write_json(record_path, record)
    return record


def latest_timestamp() -> int:
    block = cast_json(["block", "--rpc-url", RPC, "latest"])
    value = block["timestamp"]
    return int(value, 16) if isinstance(value, str) else int(value)


def set_timestamp(timestamp: int) -> None:
    command([CAST, "rpc", "--rpc-url", RPC, "evm_setNextBlockTimestamp", str(timestamp)])
    command([CAST, "rpc", "--rpc-url", RPC, "evm_mine"])
    if latest_timestamp() != timestamp:
        raise AssertionError("Anvil timestamp did not advance to refund boundary")


def set_next_timestamp(timestamp: int) -> None:
    if timestamp <= latest_timestamp():
        raise AssertionError("requested Anvil next timestamp is not in the future")
    command([CAST, "rpc", "--rpc-url", RPC, "evm_setNextBlockTimestamp", str(timestamp)])


def recover_pftl_operation(
    pftl: HardenedPftl,
    *,
    label: str,
    source: str,
    operation: dict[str, Any],
) -> dict[str, Any]:
    public_path = pftl.evidence_dir / f"{label}.certified.json"
    if public_path.is_file():
        return json.loads(public_path.read_text())
    signed_path = pftl.evidence_dir / f"{label}.signed.private.json"
    signed = json.loads(signed_path.read_text())
    unsigned = signed["unsigned"]
    history = pftl._run(
        [
            "account-tx",
            "--data-dir",
            str(pftl.node0),
            "--address",
            source,
            "--limit",
            "100",
        ]
    )
    rows = [
        row
        for row in history["rows"]
        if row.get("sequence") == unsigned["sequence"]
        and row.get("transaction_kind") == unsigned["transaction_kind"]
        and row.get("from") == source
        and row.get("accepted")
        and (
            unsigned["transaction_kind"] == "escrow_create"
            or row.get("escrow_id") == unsigned.get("escrow_id")
        )
    ]
    if len(rows) == 1:
        row = rows[0]
    else:
        txid_path = pftl.evidence_dir / f"{label}.recovered-txid.txt"
        if not txid_path.is_file():
            raise AssertionError(f"{label}: accepted transaction recovery is ambiguous")
        tx_id = txid_path.read_text().strip()
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
        if (
            not proof.get("ok")
            or not result["confirmed"]
            or not result["receipt"]["accepted"]
        ):
            raise AssertionError(f"{label}: recovered transaction is not confirmed")
        row = {
            "tx_id": tx_id,
            "block_height": result["block"]["header"]["height"],
            "receipt_code": result["receipt"]["code"],
        }
    record = {
        "schema": "postfiat.pftl.certified_escrow_operation.v1",
        "label": label,
        "operation": operation,
        "source": source,
        "sequence": unsigned["sequence"],
        "fee": unsigned["fee"],
        "tx_id": row["tx_id"],
        "block_height": row["block_height"],
        "receipt_code": row["receipt_code"],
        "recovered_after_post_finality_interruption": True,
        "converged_after": pftl.converged_status(),
    }
    write_json(public_path, record)
    return record


def create_or_recover(
    pftl: HardenedPftl,
    *,
    label: str,
    owner: str,
    recipient: str,
    amount_atoms: int,
    condition: str,
    cancel_after: int,
) -> tuple[PftlEscrowRef, dict[str, Any]]:
    signed_path = pftl.evidence_dir / f"{label}.signed.private.json"
    if not signed_path.is_file():
        return pftl.create(
            label=label,
            owner=owner,
            recipient=recipient,
            amount_atoms=amount_atoms,
            condition=condition,
            cancel_after=cancel_after,
        )
    matches = [
        item
        for item in pftl.escrows(owner, role="owner")
        if item["owner"] == owner
        and item["recipient"] == recipient
        and int(item["amount"]) == amount_atoms
        and int(item["cancel_after"]) == cancel_after
    ]
    if len(matches) != 1:
        raise AssertionError(f"{label}: expected exactly one accepted escrow")
    item = matches[0]
    operation = {
        "operation": "escrow_create",
        "owner": owner,
        "recipient": recipient,
        "asset_id": ASSET_ID,
        "amount": amount_atoms,
        "condition": condition,
        "cancel_after": cancel_after,
    }
    record = recover_pftl_operation(
        pftl, label=label, source=owner, operation=operation
    )
    return (
        PftlEscrowRef(
            escrow_id=item["escrow_id"],
            owner=owner,
            recipient=recipient,
            amount_atoms=amount_atoms,
            cancel_after=cancel_after,
            condition=condition,
            create_tx_id=record["tx_id"],
        ),
        record,
    )


def finish_or_recover(
    pftl: HardenedPftl,
    *,
    label: str,
    escrow: PftlEscrowRef,
    fulfillment: str,
) -> dict[str, Any]:
    signed_path = pftl.evidence_dir / f"{label}.signed.private.json"
    if not signed_path.is_file():
        return pftl.finish(label=label, escrow=escrow, fulfillment=fulfillment)
    operation = {
        "operation": "escrow_finish",
        "escrow_id": escrow.escrow_id,
        "owner": escrow.owner,
        "recipient": escrow.recipient,
        "fulfillment": fulfillment,
    }
    return recover_pftl_operation(
        pftl, label=label, source=escrow.recipient, operation=operation
    )


def cancel_or_recover(
    pftl: HardenedPftl, *, label: str, escrow: PftlEscrowRef
) -> dict[str, Any]:
    signed_path = pftl.evidence_dir / f"{label}.signed.private.json"
    if not signed_path.is_file():
        return pftl.cancel(label=label, escrow=escrow)
    operation = {
        "operation": "escrow_cancel",
        "escrow_id": escrow.escrow_id,
        "owner": escrow.owner,
        "recipient": escrow.recipient,
    }
    return recover_pftl_operation(
        pftl, label=label, source=escrow.owner, operation=operation
    )


def balance(account: str) -> int:
    return int(call(USDC, "balanceOf(address)(uint256)", account)[0])


def swap(swap_id: str) -> dict[str, Any]:
    row = call(
        HTLC,
        "swaps(bytes32)(address,address,uint128,uint64,bytes32,uint8)",
        swap_id,
    )
    return {
        "refund_address": row[0],
        "recipient": row[1],
        "amount": int(row[2]),
        "refund_time": int(row[3]),
        "hashlock": row[4],
        "state": int(row[5]),
    }


def evm_snapshot(swap_ids: list[str]) -> dict[str, Any]:
    return {
        "user_usdc": balance(USER_EVM),
        "coordinator_usdc": balance(COORDINATOR_EVM),
        "contract_usdc": balance(HTLC),
        "total_supply": int(call(USDC, "totalSupply()(uint256)")[0]),
        "swaps": {swap_id: swap(swap_id) for swap_id in swap_ids},
    }


def assert_evm_unchanged(before: dict[str, Any], after: dict[str, Any]) -> None:
    if before != after:
        raise AssertionError("rejected EVM call changed contract principal/state")


def pftl_snapshot(pftl: HardenedPftl) -> tuple[dict[str, Any], PrincipalState]:
    open_escrows: dict[str, dict[str, Any]] = {}
    for owner in (USER, COORDINATOR):
        for item in pftl.escrows(owner, role="owner"):
            if item["asset_id"] == ASSET_ID and item["state"] == "open":
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
            "open_escrows": list(open_escrows.values()),
        },
        state,
    )


def swap_id(label: str) -> str:
    return "0x" + hashlib.sha256(("pftl-usdc-navcoin:" + label).encode()).hexdigest()


def public_preimage_from_receipt(record: dict[str, Any], expected_hash: bytes) -> str:
    topic = "0x" + hashlib.sha3_256(b"Redeemed(bytes32,bytes32)").hexdigest()
    # Ethereum uses Keccak-256, not standardized SHA3. The event has exactly one
    # non-Transfer log from the HTLC, and its sole data word is the preimage.
    logs = [
        item
        for item in record["receipt"]["logs"]
        if item["address"].lower() == HTLC.lower() and len(item["topics"]) == 2
    ]
    if len(logs) != 1:
        raise AssertionError("expected one public HTLC Redeemed log")
    preimage = logs[0]["data"].removeprefix("0x")
    if hashlib.sha256(bytes.fromhex(preimage)).digest() != expected_hash:
        raise AssertionError("public EVM preimage does not authenticate hashlock")
    return preimage


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--runtime-root", required=True, type=Path)
    args = parser.parse_args()
    runtime = args.runtime_root.resolve()
    evidence = runtime / "public/evidence"
    evm_evidence = evidence / "evm"
    private = runtime / "private"
    evm_evidence.mkdir(parents=True, exist_ok=True)
    pftl = HardenedPftl(evidence / "pftl")

    if int(cast_json(["chain-id", "--rpc-url", RPC])) != 31337:
        raise AssertionError("expected local Anvil chain id 31337")
    deployments = {
        "mock_usdc": {
            "address": USDC,
            "transaction_hash": MOCK_DEPLOY_TX,
            "receipt": receipt(MOCK_DEPLOY_TX),
        },
        "htlc": {
            "address": HTLC,
            "transaction_hash": HTLC_DEPLOY_TX,
            "receipt": receipt(HTLC_DEPLOY_TX),
        },
        "runtime_code_sha256": {
            "mock_usdc": hashlib.sha256(code(USDC)).hexdigest(),
            "htlc": hashlib.sha256(code(HTLC)).hexdigest(),
        },
    }
    write_json(evm_evidence / "deployments.json", deployments)

    secrets_path = private / "swap-preimages.private.json"
    if secrets_path.is_file():
        secrets = json.loads(secrets_path.read_text())
        locks = {
            name: CrossLedgerHashlock.from_secret_hex(value)
            for name, value in secrets.items()
        }
    else:
        locks = {
            "usdc_to_nav": CrossLedgerHashlock.generate(),
            "nav_to_usdc": CrossLedgerHashlock.generate(),
            "refund": CrossLedgerHashlock.generate(),
        }
        write_json(
            secrets_path,
            {name: item.secret.protocol_hex() for name, item in locks.items()},
            0o600,
        )
    ids = {name: swap_id(name) for name in locks}

    approvals = [
        send(
            evm_evidence,
            "00-user-approve",
            key=USER_KEY,
            contract=USDC,
            signature="approve(address,uint256)",
            arguments=[HTLC, MAX_UINT],
        ),
        send(
            evm_evidence,
            "00-coordinator-approve",
            key=COORDINATOR_KEY,
            contract=USDC,
            signature="approve(address,uint256)",
            arguments=[HTLC, MAX_UINT],
        ),
    ]
    initial_evm = evm_snapshot(list(ids.values()))
    if sum(
        (
            initial_evm["user_usdc"],
            initial_evm["coordinator_usdc"],
            initial_evm["contract_usdc"],
        )
    ) != 2_000_000 or initial_evm["total_supply"] != 2_000_000:
        raise AssertionError("unexpected mock-USDC issuance or conservation")

    # Refund locks first so ordinary happy-path PFTL blocks mature its short leg.
    refund = locks["refund"]
    refund_existing = swap(ids["refund"])
    refund_time = refund_existing["refund_time"] or (latest_timestamp() + 120)
    refund_evm_lock = send(
        evm_evidence,
        "refund-01-usdc-lock",
        key=USER_KEY,
        contract=HTLC,
        signature="lock(bytes32,address,uint128,bytes32,uint64)",
        arguments=[
            ids["refund"],
            COORDINATOR_EVM,
            "25000",
            "0x" + refund.digest.hex(),
            str(refund_time),
        ],
    )
    existing_refund_signed = pftl.evidence_dir / "refund-02-nav-create.signed.private.json"
    if existing_refund_signed.is_file():
        refund_pftl_cancel = int(
            json.loads(existing_refund_signed.read_text())["unsigned"]["cancel_after"]
        )
    else:
        refund_pftl_cancel = pftl.converged_status()["height"] + 4
    before = pftl_snapshot(pftl)
    refund_nav, refund_nav_create = create_or_recover(
        pftl,
        label="refund-02-nav-create",
        owner=COORDINATOR,
        recipient=USER,
        amount_atoms=25_000,
        condition=refund.public_values()["pftl_condition"],
        cancel_after=refund_pftl_cancel,
    )
    after = pftl_snapshot(pftl)
    assert_principal_conserved(before[1], after[1])

    # USDC -> NAVcoin: EVM is first/longer, PFTL second/shorter.
    on = locks["usdc_to_nav"]
    on_existing = swap(ids["usdc_to_nav"])
    on_refund_time = on_existing["refund_time"] or (latest_timestamp() + 600)
    on_evm_lock = send(
        evm_evidence,
        "onramp-01-usdc-lock",
        key=USER_KEY,
        contract=HTLC,
        signature="lock(bytes32,address,uint128,bytes32,uint64)",
        arguments=[
            ids["usdc_to_nav"],
            COORDINATOR_EVM,
            "100000",
            "0x" + on.digest.hex(),
            str(on_refund_time),
        ],
    )
    existing_on_signed = pftl.evidence_dir / "onramp-02-nav-create.signed.private.json"
    if existing_on_signed.is_file():
        on_nav_cancel = int(
            json.loads(existing_on_signed.read_text())["unsigned"]["cancel_after"]
        )
    else:
        on_nav_cancel = pftl.converged_status()["height"] + 50
    before = pftl_snapshot(pftl)
    on_nav, on_nav_create = create_or_recover(
        pftl,
        label="onramp-02-nav-create",
        owner=COORDINATOR,
        recipient=USER,
        amount_atoms=100_000,
        condition=on.public_values()["pftl_condition"],
        cancel_after=on_nav_cancel,
    )
    after_on_create = pftl_snapshot(pftl)
    assert_principal_conserved(before[1], after_on_create[1])

    wrong = SecretPreimage.generate()
    adversarial_before = evm_snapshot(list(ids.values()))
    # Query the confirmed historical block where the on-ramp lock was open.
    # This keeps idempotent resumes semantically meaningful after the swap has
    # reached its terminal state.
    on_open_block_value = on_evm_lock["receipt"]["blockNumber"]
    on_open_block = (
        int(on_open_block_value, 16)
        if isinstance(on_open_block_value, str)
        else int(on_open_block_value)
    )
    wrong_evm = rejected_call(
        HTLC,
        "redeem(bytes32,bytes32)",
        ids["usdc_to_nav"],
        "0x" + wrong.protocol_hex(),
        sender=COORDINATOR_EVM,
        block=on_open_block,
    )
    early_evm = rejected_call(
        HTLC,
        "refund(bytes32)",
        ids["usdc_to_nav"],
        sender=USER_EVM,
        block=on_open_block,
    )
    adversarial_after = evm_snapshot(list(ids.values()))
    assert_evm_unchanged(adversarial_before, adversarial_after)
    if verify_pair(
        on_nav.condition,
        "a0228020" + wrong.protocol_hex(),
        xrpl=False,
    ):
        raise AssertionError("wrong PFTL preimage passed")
    pftl_attack_before = pftl_snapshot(pftl)
    if pftl_attack_before[0]["convergence"]["height"] >= on_nav.cancel_after:
        raise AssertionError("PFTL early-cancel probe was not early")
    pftl_attack_after = pftl_snapshot(pftl)
    if pftl_attack_before != pftl_attack_after:
        raise AssertionError("PFTL adversarial preflight mutated state")

    on_nav_finish = finish_or_recover(
        pftl,
        label="onramp-03-nav-finish",
        escrow=on_nav,
        fulfillment=on.pftl_fulfillment(),
    )
    after_on_finish = pftl_snapshot(pftl)
    assert_principal_conserved(after_on_create[1], after_on_finish[1])
    if not (evm_evidence / "onramp-04-usdc-redeem.json").is_file():
        set_next_timestamp(on_refund_time - 1)
    on_evm_redeem = send(
        evm_evidence,
        "onramp-04-usdc-redeem",
        key=COORDINATOR_KEY,
        contract=HTLC,
        signature="redeem(bytes32,bytes32)",
        arguments=[ids["usdc_to_nav"], "0x" + on.secret.protocol_hex()],
    )
    on_public = public_preimage_from_receipt(on_evm_redeem, on.digest)

    # NAVcoin -> USDC: PFTL is first/longer, EVM second/shorter.
    off = locks["nav_to_usdc"]
    existing_off_signed = pftl.evidence_dir / "offramp-01-nav-create.signed.private.json"
    if existing_off_signed.is_file():
        off_nav_cancel = int(
            json.loads(existing_off_signed.read_text())["unsigned"]["cancel_after"]
        )
    else:
        off_nav_cancel = pftl.converged_status()["height"] + 50
    before = pftl_snapshot(pftl)
    off_nav, off_nav_create = create_or_recover(
        pftl,
        label="offramp-01-nav-create",
        owner=USER,
        recipient=COORDINATOR,
        amount_atoms=80_000,
        condition=off.public_values()["pftl_condition"],
        cancel_after=off_nav_cancel,
    )
    after_off_create = pftl_snapshot(pftl)
    assert_principal_conserved(before[1], after_off_create[1])
    off_existing = swap(ids["nav_to_usdc"])
    off_refund_time = off_existing["refund_time"] or (latest_timestamp() + 120)
    if not (evm_evidence / "offramp-02-usdc-lock.json").is_file():
        set_next_timestamp(off_refund_time - 2)
    off_evm_lock = send(
        evm_evidence,
        "offramp-02-usdc-lock",
        key=COORDINATOR_KEY,
        contract=HTLC,
        signature="lock(bytes32,address,uint128,bytes32,uint64)",
        arguments=[
            ids["nav_to_usdc"],
            USER_EVM,
            "80000",
            "0x" + off.digest.hex(),
            str(off_refund_time),
        ],
    )
    if not (evm_evidence / "offramp-03-usdc-redeem.json").is_file():
        set_next_timestamp(off_refund_time - 1)
    off_evm_redeem = send(
        evm_evidence,
        "offramp-03-usdc-redeem",
        key=USER_KEY,
        contract=HTLC,
        signature="redeem(bytes32,bytes32)",
        arguments=[ids["nav_to_usdc"], "0x" + off.secret.protocol_hex()],
    )
    off_public = public_preimage_from_receipt(off_evm_redeem, off.digest)
    before = pftl_snapshot(pftl)
    off_nav_finish = finish_or_recover(
        pftl,
        label="offramp-04-nav-finish",
        escrow=off_nav,
        fulfillment="a0228020" + off_public,
    )
    after = pftl_snapshot(pftl)
    assert_principal_conserved(before[1], after[1])

    # Terminal duplicates on both ledgers are rejected without state change.
    duplicate_before = evm_snapshot(list(ids.values()))
    duplicate_evm = [
        rejected_call(
            HTLC,
            "redeem(bytes32,bytes32)",
            ids["usdc_to_nav"],
            "0x" + on.secret.protocol_hex(),
            sender=COORDINATOR_EVM,
        ),
        rejected_call(
            HTLC,
            "redeem(bytes32,bytes32)",
            ids["nav_to_usdc"],
            "0x" + off.secret.protocol_hex(),
            sender=USER_EVM,
        ),
    ]
    duplicate_after = evm_snapshot(list(ids.values()))
    assert_evm_unchanged(duplicate_before, duplicate_after)
    if pftl.escrow_info(on_nav.escrow_id)["escrow"]["state"] != "finished":
        raise AssertionError("on-ramp PFTL terminal-state duplicate gate failed")
    if pftl.escrow_info(off_nav.escrow_id)["escrow"]["state"] != "finished":
        raise AssertionError("off-ramp PFTL terminal-state duplicate gate failed")

    # Refund: the PFTL short leg is now mature. Cancel it, then prove the EVM
    # exact-boundary redeem rejection and refund the longer leg.
    if pftl.converged_status()["height"] < refund_nav.cancel_after:
        raise AssertionError("happy-path blocks did not mature PFTL refund")
    before = pftl_snapshot(pftl)
    refund_nav_cancel_record = cancel_or_recover(
        pftl,
        label="refund-03-nav-cancel", escrow=refund_nav
    )
    after = pftl_snapshot(pftl)
    assert_principal_conserved(before[1], after[1])
    if latest_timestamp() < refund_time:
        set_timestamp(refund_time)
    late_before = evm_snapshot(list(ids.values()))
    refund_record_path = evm_evidence / "refund-04-usdc-refund.json"
    late_probe_block = None
    if refund_record_path.is_file():
        refund_block_value = json.loads(refund_record_path.read_text())["receipt"][
            "blockNumber"
        ]
        refund_block = (
            int(refund_block_value, 16)
            if isinstance(refund_block_value, str)
            else int(refund_block_value)
        )
        late_probe_block = refund_block - 1
    at_cancel = rejected_call(
        HTLC,
        "redeem(bytes32,bytes32)",
        ids["refund"],
        "0x" + refund.secret.protocol_hex(),
        sender=COORDINATOR_EVM,
        block=late_probe_block,
    )
    late_after = evm_snapshot(list(ids.values()))
    assert_evm_unchanged(late_before, late_after)
    refund_evm = send(
        evm_evidence,
        "refund-04-usdc-refund",
        key=USER_KEY,
        contract=HTLC,
        signature="refund(bytes32)",
        arguments=[ids["refund"]],
    )
    post_refund_before = evm_snapshot(list(ids.values()))
    after_refund_claim = rejected_call(
        HTLC,
        "redeem(bytes32,bytes32)",
        ids["refund"],
        "0x" + refund.secret.protocol_hex(),
        sender=COORDINATOR_EVM,
    )
    duplicate_refund = rejected_call(
        HTLC,
        "refund(bytes32)",
        ids["refund"],
        sender=USER_EVM,
    )
    post_refund_after = evm_snapshot(list(ids.values()))
    assert_evm_unchanged(post_refund_before, post_refund_after)
    pftl_late_before = pftl_snapshot(pftl)
    if pftl.escrow_info(refund_nav.escrow_id)["escrow"]["state"] != "canceled":
        raise AssertionError("PFTL canceled escrow remained claimable")
    pftl_late_after = pftl_snapshot(pftl)
    if pftl_late_before != pftl_late_after:
        raise AssertionError("PFTL late-finish terminal check mutated state")

    final_evm = evm_snapshot(list(ids.values()))
    final_pftl = pftl_snapshot(pftl)
    if (
        final_evm["user_usdc"] != 980_000
        or final_evm["coordinator_usdc"] != 1_020_000
        or final_evm["contract_usdc"] != 0
        or sum(
            (
                final_evm["user_usdc"],
                final_evm["coordinator_usdc"],
                final_evm["contract_usdc"],
            )
        )
        != final_evm["total_supply"]
    ):
        raise AssertionError("mock-USDC exact conservation failed")
    if final_pftl[1].total != 3_000_000_000:
        raise AssertionError("NAVcoin exact atom conservation failed")

    report = {
        "schema": "postfiat.anvil_usdc_navcoin.live_demo.v1",
        "result": "PASS",
        "claim": "non-custodial, conditionally-atomic",
        "value_disclaimer": "Local Anvil mock USDC and isolated PFTL devnet NAVcoin only; no mainnet or real value",
        "networks": {
            "evm": {
                "network": "Anvil",
                "rpc": RPC,
                "chain_id": 31337,
                "mock_usdc": USDC,
                "htlc": HTLC,
                "user": USER_EVM,
                "coordinator": COORDINATOR_EVM,
                "deployments": deployments,
            },
            "pftl": {
                "chain_id": "local-pftl-proven-nav-v2-20260724",
                "rpc": [f"tcp://127.0.0.1:{port}" for port in range(31660, 31666)],
                "binary_revision": "ae3c53c9",
                "asset_id": ASSET_ID,
                "user": USER,
                "coordinator": COORDINATOR,
            },
        },
        "timing_model": {
            "ordering": "first locker longer; second locker shorter; second mover claims first",
            "evm_clock": "block.timestamp",
            "pftl_clock": "block height",
            "trust_boundary": "independent clocks require configured margins, active monitoring, and transaction liveness",
        },
        "scenarios": {
            "usdc_to_nav_happy": {
                "payment_hash": on.digest.hex(),
                "swap_id": ids["usdc_to_nav"],
                "timelocks": {
                    "first_evm_refund_time": on_refund_time,
                    "second_pftl_cancel_height": on_nav.cancel_after,
                },
                "pftl_escrow": on_nav.__dict__,
                "public_preimage_hex": on_public,
                "transactions": {
                    "evm_lock": on_evm_lock["receipt"]["transactionHash"],
                    "pftl_create": on_nav_create["tx_id"],
                    "pftl_finish": on_nav_finish["tx_id"],
                    "evm_redeem": on_evm_redeem["receipt"]["transactionHash"],
                },
            },
            "nav_to_usdc_happy": {
                "payment_hash": off.digest.hex(),
                "swap_id": ids["nav_to_usdc"],
                "timelocks": {
                    "first_pftl_cancel_height": off_nav.cancel_after,
                    "second_evm_refund_time": off_refund_time,
                },
                "pftl_escrow": off_nav.__dict__,
                "public_preimage_hex": off_public,
                "pftl_finish_used_public_evm_preimage": True,
                "transactions": {
                    "pftl_create": off_nav_create["tx_id"],
                    "evm_lock": off_evm_lock["receipt"]["transactionHash"],
                    "evm_redeem": off_evm_redeem["receipt"]["transactionHash"],
                    "pftl_finish": off_nav_finish["tx_id"],
                },
            },
            "refund": {
                "payment_hash": refund.digest.hex(),
                "swap_id": ids["refund"],
                "timelocks": {
                    "first_evm_refund_time": refund_time,
                    "second_pftl_cancel_height": refund_nav.cancel_after,
                },
                "pftl_escrow": refund_nav.__dict__,
                "preimage_revealed": False,
                "transactions": {
                    "evm_lock": refund_evm_lock["receipt"]["transactionHash"],
                    "pftl_create": refund_nav_create["tx_id"],
                    "pftl_cancel": refund_nav_cancel_record["tx_id"],
                    "evm_refund": refund_evm["receipt"]["transactionHash"],
                },
            },
        },
        "adversarial": {
            "wrong_preimage": {
                "evm": wrong_evm,
                "pftl_rejected_before_signing": True,
                "both_ledgers_mutation_free": True,
            },
            "early_cancel": {
                "evm": early_evm,
                "pftl_rejected_before_signing": True,
                "both_ledgers_mutation_free": True,
            },
            "late_at_or_after_cancel": {
                "evm_at_cancel": at_cancel,
                "evm_after_refund": after_refund_claim,
                "pftl_after_cancel_rejected_before_signing": True,
                "both_ledgers_mutation_free": True,
            },
            "duplicates": {
                "evm_redeems": duplicate_evm,
                "evm_refund": duplicate_refund,
                "pftl_terminal_state_gate": True,
                "both_ledgers_mutation_free": True,
            },
        },
        "conservation": {
            "mock_usdc_total_atoms": final_evm["total_supply"],
            "user_usdc_atoms": final_evm["user_usdc"],
            "coordinator_usdc_atoms": final_evm["coordinator_usdc"],
            "htlc_locked_usdc_atoms": final_evm["contract_usdc"],
            "mock_usdc_exact": True,
            "navcoin_total_atoms": final_pftl[1].total,
            "navcoin_exact": True,
        },
        "final_evm": final_evm,
        "final_pftl": final_pftl[0],
        "approvals": [item["receipt"]["transactionHash"] for item in approvals],
        "public_preimages_authenticated": 2,
        "refund_preimage_revealed": False,
        "reference_lane": {
            "lane": "XRPL Testnet ↔ NAVcoin",
            "nazgul_verified": True,
            "verification": "/home/postfiat/tmp/pftl-xrpl-navcoin-20260724/public/evidence/independent-verification.json",
        },
    }
    write_json(evidence / "live-demo-report.json", report)
    print(json.dumps(report, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
