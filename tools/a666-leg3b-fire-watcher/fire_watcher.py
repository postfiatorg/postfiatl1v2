#!/usr/bin/env python3
"""A666 leg-3b fire watcher, staged under the 2026-08-08 manager ruling.

The deployed copy lives at /tmp/a666-s1g/leg3b/fire_watcher.py. It polls every
30 seconds and triggers only when either:

* the constrained signer has at least the principal-authorized 0.01 ETH; or
* agentd's live whitelist contains the signer, allowing the exact 0.01 ETH
  leg-3b0 custody transfer.

The mutation sequence is receipt-gated and STOP-no-retry:

1. Reconcile or execute leg 3b0 exactly once.
2. Advance the PFTL checkpoint from 691 to 756.
3. Accept the finalized receipt and mint exactly 11,012,575 atoms.

A durable pre-broadcast intent prevents a watcher restart from issuing a second
leg-3b0 transfer. Every mutation receives a fresh 30-minute deadline-margin
check. Checkpoint recovery requires both height 756 and the exact target
commitment. Ethereum receipt gates use status=1; PFTL code=accepted does not
exist on the Ethereum side.
"""
from __future__ import annotations

import fcntl
import hashlib
import json
import os
import subprocess
import sys
import tempfile
import time
from pathlib import Path
from typing import Any
from urllib.request import Request, urlopen

BASE = Path("/tmp/a666-s1g/leg3b")
FIRE = BASE / "fire"
REPO = Path("/home/postfiat/repos/a666-eth-fast-lane-combined-20260724")
SIGNER = "0xe01eaf76f155b2759402b39fe126b5a81655f424"
OWNER = "0x1455bd7fbfbf92a171ef36025e13959e3b0ad8c0"
SIGNER_SOCKET = "/run/user/1000/postfiat-constrained-signer/a666-signer.sock"
STAKEHUB_HOME = "/home/postfiat/.stakehub"
AGENT_JOURNAL = Path(STAKEHUB_HOME) / "journal.jsonl"
RPC = "https://ethereum-rpc.publicnode.com"
VERIFIER = "0xb79FF97EcC11574a8A78d0b5a9D7C8c2A94bF96A"
CAST = "/home/postfiat/.foundry/bin/cast"
DEADLINE = 1786331925  # 2026-08-10 03:18:45 UTC
DEADLINE_MARGIN_S = 30 * 60
FUND_WEI = 10**16  # 0.01 ETH, exact principal-authorized external trigger/funding value.
REQUIRED_SIGNER_WEI = FUND_WEI
FUND_MAX_FEE_WEI = 5_666_645_628_000
PRIOR_BLOCK = "bc3aef9a3b38b0c3030d4350af43addbf285b681cfa8fe750a52a97c236b54e701b66292ba1607525410e3cdf285da26"
TARGET_BLOCK = "4d5195acdbe8b80dac875f35b4a45eb5b31071f5393f07b4f4d54a58bf2a418fe6c45f7fe86d5a937ded828d30770265"
PRIOR_COMMITMENT = "0x1afce4dcde4017f6b2354617178c6b6a520064a6f7780a85ff3989cf2d36c544"
TARGET_COMMITMENT = "0x3b7c8bde64bfb6e8f5c65b2cde016a658ca270d01d399548336d12c5c5ec5b12"
MINT_DELTA = 11_012_575
JOURNAL_GENESIS = "0" * 96
FUND_INTENT = FIRE / "leg3b0-intent.json"
FUND_REPORT = FIRE / "leg3b0-report.json"


def log(msg: str) -> None:
    line = f"{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())} {msg}"
    print(line, flush=True)


def atomic_write_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    descriptor, name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(name)
    try:
        with os.fdopen(descriptor, "w", encoding="utf-8") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        temporary.unlink(missing_ok=True)


def stop(reason: str) -> None:
    (BASE / "STOP.txt").write_text(
        f"{time.strftime('%Y-%m-%dT%H:%M:%SZ', time.gmtime())} STOP-no-retry: {reason}\n",
        encoding="utf-8",
    )
    log(f"STOP-no-retry: {reason}")
    raise SystemExit(2)


def deadline_guard(step: str) -> None:
    remaining = DEADLINE - int(time.time())
    if remaining <= DEADLINE_MARGIN_S:
        stop(
            f"{step}: {remaining}s remain before export-packet deadline; "
            f"required margin is >{DEADLINE_MARGIN_S}s"
        )


def cast_call(args: list[str]) -> str:
    out = subprocess.run(
        [CAST, *args, "--rpc-url", RPC],
        capture_output=True,
        text=True,
        timeout=60,
        check=False,
    )
    if out.returncode != 0:
        raise RuntimeError(f"cast {' '.join(args[:2])} failed: {out.stderr.strip()[:200]}")
    return out.stdout.strip()


def rpc(method: str, params: list[Any]) -> Any:
    body = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    request = Request(
        RPC,
        data=body,
        headers={"content-type": "application/json", "user-agent": "a666-leg3b-watcher/1.0"},
    )
    with urlopen(request, timeout=60) as response:
        payload = json.loads(response.read().decode())
    if payload.get("error"):
        raise RuntimeError(f"{method} failed: {payload['error']}")
    return payload.get("result")


def parse_int(value: Any) -> int:
    if isinstance(value, str):
        return int(value, 16) if value.startswith("0x") else int(value)
    return int(value)


def signer_balance_wei() -> int:
    return int(cast_call(["balance", SIGNER]))


def owner_nonce(block: str) -> int:
    return int(cast_call(["nonce", OWNER, "--block", block]))


def verifier_checkpoint() -> tuple[int, str]:
    height = int(cast_call(["call", VERIFIER, "latestFinalizedHeight()(uint64)"]))
    commitment = cast_call(
        ["call", VERIFIER, "latestCheckpointCommitment()(bytes32)"]
    ).lower()
    return height, commitment


def whitelist_has_signer() -> bool:
    sys.path.insert(0, str(REPO / "scripts"))
    import native_agentd_leaf as leaf  # noqa: PLC0415

    status = leaf.session_status(STAKEHUB_HOME)
    if not status.get("unlocked"):
        raise RuntimeError("agentd not unlocked")
    return any(str(item).lower() == SIGNER for item in status["policy"]["whitelist"])


def verified_journal_entries() -> list[dict[str, Any]]:
    if not AGENT_JOURNAL.exists():
        raise RuntimeError(f"agent journal missing: {AGENT_JOURNAL}")
    entries: list[dict[str, Any]] = []
    previous = JOURNAL_GENESIS
    for line_number, line in enumerate(AGENT_JOURNAL.read_text(encoding="utf-8").splitlines(), 1):
        if not line.strip():
            continue
        entry = json.loads(line)
        body = {key: value for key, value in entry.items() if key != "entry_hash"}
        if body.get("prev_hash") != previous:
            raise RuntimeError(f"agent journal chain mismatch at line {line_number}")
        canonical = json.dumps(body, sort_keys=True, separators=(",", ":"))
        expected = hashlib.sha3_384(canonical.encode()).hexdigest()
        if entry.get("entry_hash") != expected:
            raise RuntimeError(f"agent journal entry hash mismatch at line {line_number}")
        previous = expected
        entries.append(entry)
    return entries


def journal_head(entries: list[dict[str, Any]]) -> str:
    return str(entries[-1]["entry_hash"]) if entries else JOURNAL_GENESIS


def entries_after_head(
    entries: list[dict[str, Any]], head: str
) -> list[dict[str, Any]]:
    if head == JOURNAL_GENESIS:
        return entries
    for index, entry in enumerate(entries):
        if entry.get("entry_hash") == head:
            return entries[index + 1 :]
    raise RuntimeError("funding intent journal head is absent from the verified journal")


def validate_funding_transaction_shape(
    transaction: dict[str, Any], expected_nonce: int
) -> dict[str, Any]:
    evidence = {
        "tx_hash": str(transaction.get("hash", "")),
        "from": str(transaction.get("from", "")).lower(),
        "to": str(transaction.get("to", "")).lower(),
        "value_wei": parse_int(transaction.get("value", 0)),
        "nonce": parse_int(transaction.get("nonce", -1)),
    }
    if evidence["from"] != OWNER:
        raise RuntimeError(f"leg 3b0 sender {evidence['from']} != expected owner")
    if evidence["to"] != SIGNER:
        raise RuntimeError(f"leg 3b0 recipient {evidence['to']} != expected signer")
    if evidence["value_wei"] != FUND_WEI:
        raise RuntimeError(
            f"leg 3b0 value {evidence['value_wei']} != exact {FUND_WEI}"
        )
    if evidence["nonce"] != expected_nonce:
        raise RuntimeError(
            f"leg 3b0 nonce {evidence['nonce']} != expected {expected_nonce}"
        )
    return evidence


def validate_funding_transaction(
    tx_hash: str, *, expected_nonce: int
) -> dict[str, Any] | None:
    normalized_hash = tx_hash if tx_hash.startswith("0x") else f"0x{tx_hash}"
    transaction = rpc("eth_getTransactionByHash", [normalized_hash])
    if not isinstance(transaction, dict):
        return None
    evidence = validate_funding_transaction_shape(transaction, expected_nonce)
    receipt = rpc("eth_getTransactionReceipt", [normalized_hash])
    if receipt is None:
        evidence.update({"status": None, "pending": True})
        return evidence
    if not isinstance(receipt, dict):
        raise RuntimeError("leg 3b0 receipt RPC returned malformed data")
    evidence.update(
        {
            "status": parse_int(receipt.get("status", 0)),
            "pending": False,
            "block_number": parse_int(receipt.get("blockNumber", 0)),
            "gas_used": parse_int(receipt.get("gasUsed", 0)),
            "effective_gas_price": parse_int(
                receipt.get("effectiveGasPrice", 0)
            ),
        }
    )
    if evidence["status"] != 1:
        raise RuntimeError(f"leg 3b0 receipt status {evidence['status']} != 1")
    return evidence


def find_funding_transaction_by_nonce(intent: dict[str, Any]) -> str | None:
    expected_nonce = int(intent["owner_nonce_expected"])
    attempt_block = int(intent["ethereum_block_at_attempt"])
    latest_block = parse_int(rpc("eth_blockNumber", []))
    candidates: dict[str, dict[str, Any]] = {}

    pending_block = rpc("eth_getBlockByNumber", ["pending", True])
    blocks: list[dict[str, Any]] = []
    if isinstance(pending_block, dict):
        blocks.append(pending_block)
    for block_number in range(attempt_block, latest_block + 1):
        block = rpc("eth_getBlockByNumber", [hex(block_number), True])
        if isinstance(block, dict):
            blocks.append(block)

    for block in blocks:
        for transaction in block.get("transactions", []):
            if not isinstance(transaction, dict):
                continue
            if (
                str(transaction.get("from", "")).lower() == OWNER
                and parse_int(transaction.get("nonce", -1)) == expected_nonce
            ):
                tx_hash = str(transaction.get("hash", "")).lower()
                if not tx_hash:
                    stop("owner+nonce funding candidate omitted transaction hash")
                candidates[tx_hash] = transaction
    if len(candidates) > 1:
        stop(
            f"multiple transactions found for owner nonce {expected_nonce}; "
            "refusing ambiguous recovery"
        )
    if not candidates:
        return None
    transaction = next(iter(candidates.values()))
    validate_funding_transaction_shape(transaction, expected_nonce)
    return str(transaction["hash"])


def load_funding_intent() -> dict[str, Any] | None:
    if not FUND_INTENT.exists():
        return None
    intent = json.loads(FUND_INTENT.read_text(encoding="utf-8"))
    required = {
        "schema": "postfiat.a666.leg3b0.intent.v2",
        "owner": OWNER,
        "signer": SIGNER,
        "amount_wei": FUND_WEI,
        "chain_id": 1,
        "label": "leg3b0-signer-funding",
    }
    for key, expected in required.items():
        if intent.get(key) != expected:
            stop(f"leg 3b0 intent mismatch at {key}")
    if intent.get("phase") not in {
        "prepared",
        "broadcast_attempt_started",
        "receipt_verified",
        "skipped_external_funding",
    }:
        stop(f"leg 3b0 intent has unknown phase {intent.get('phase')!r}")
    if intent["phase"] in {"broadcast_attempt_started", "receipt_verified"}:
        for key in (
            "ethereum_block_at_attempt",
            "journal_head_at_attempt",
            "owner_nonce_expected",
        ):
            if key not in intent:
                stop(f"leg 3b0 started intent omitted {key}")
    return intent


def create_funding_intent() -> dict[str, Any]:
    entries = verified_journal_entries()
    intent: dict[str, Any] = {
        "schema": "postfiat.a666.leg3b0.intent.v2",
        "phase": "prepared",
        "created_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "created_epoch": int(time.time()),
        "chain_id": 1,
        "owner": OWNER,
        "signer": SIGNER,
        "amount_wei": FUND_WEI,
        "maximum_fee_wei": FUND_MAX_FEE_WEI,
        "label": "leg3b0-signer-funding",
        "journal_head_at_prepare": journal_head(entries),
        "owner_nonce_latest_at_prepare": owner_nonce("latest"),
        "owner_nonce_pending_at_prepare": owner_nonce("pending"),
        "signer_balance_wei_at_prepare": signer_balance_wei(),
    }
    atomic_write_json(FUND_INTENT, intent)
    return intent


def recover_funding_from_report_or_journal(
    intent: dict[str, Any], *, wait_seconds: int
) -> dict[str, Any] | None:
    deadline = time.monotonic() + wait_seconds
    expected_nonce = int(intent["owner_nonce_expected"])
    pending_evidence: dict[str, Any] | None = None
    while True:
        tx_hash: str | None = None
        if FUND_REPORT.exists():
            report = json.loads(FUND_REPORT.read_text(encoding="utf-8"))
            tx_hash = report.get("tx_hash")
            if not isinstance(tx_hash, str) or not tx_hash:
                stop("leg 3b0 report omitted transaction hash")

        entries = verified_journal_entries()
        head = str(
            intent.get("journal_head_at_attempt")
            or intent.get("journal_head_at_prepare")
            or ""
        )
        candidates = [
            entry
            for entry in entries_after_head(entries, head)
            if entry.get("type") == "agent_evm_send"
            and str(entry.get("chain", "")).lower() == "ethereum"
            and str(entry.get("asset", "")).lower() == "eth"
            and str(entry.get("dest", "")).lower() == SIGNER
        ]
        if len(candidates) > 1:
            stop("multiple post-intent agent_evm_send candidates target the signer")
        if len(candidates) == 1:
            journal_hash = candidates[0].get("tx")
            if not isinstance(journal_hash, str) or not journal_hash:
                stop("post-intent agent journal candidate omitted transaction hash")
            if tx_hash is not None and journal_hash.lower() != tx_hash.lower():
                stop("leg 3b0 report and agent journal transaction hashes disagree")
            tx_hash = journal_hash

        if tx_hash is None:
            tx_hash = find_funding_transaction_by_nonce(intent)
        if tx_hash is not None:
            evidence = validate_funding_transaction(
                tx_hash, expected_nonce=expected_nonce
            )
            if evidence is not None and evidence.get("status") == 1:
                return evidence
            if evidence is not None and evidence.get("pending"):
                pending_evidence = evidence

        if time.monotonic() >= deadline:
            if pending_evidence is not None:
                stop(
                    "exact owner+nonce leg 3b0 transaction remains pending; "
                    f"tx={pending_evidence['tx_hash']}; refusing any duplicate send"
                )
            return None
        time.sleep(5)


def persist_verified_funding(
    intent: dict[str, Any], evidence: dict[str, Any], *, recovered: bool
) -> int:
    intent.update(
        {
            "phase": "receipt_verified",
            "receipt_verified_utc": time.strftime(
                "%Y-%m-%dT%H:%M:%SZ", time.gmtime()
            ),
            "recovered_after_restart": recovered,
            "transaction": evidence,
        }
    )
    atomic_write_json(FUND_INTENT, intent)
    return signer_balance_wei()


def reconcile_or_fund_signer() -> int:
    FIRE.mkdir(parents=True, exist_ok=True)
    intent = load_funding_intent()
    if intent is None:
        intent = create_funding_intent()

    if intent["phase"] in {"broadcast_attempt_started", "receipt_verified"}:
        evidence = recover_funding_from_report_or_journal(intent, wait_seconds=330)
        if evidence is None:
            latest = owner_nonce("latest")
            pending = owner_nonce("pending")
            stop(
                "leg 3b0 broadcast-attempt marker exists without a recoverable "
                f"report/journal transaction; owner nonces latest={latest}, "
                f"pending={pending}; refusing any duplicate send"
            )
        balance = persist_verified_funding(intent, evidence, recovered=True)
        height, _ = verifier_checkpoint()
        if height == 691 and balance < REQUIRED_SIGNER_WEI:
            stop(
                f"reconciled leg 3b0 but signer balance {balance} is below "
                f"{REQUIRED_SIGNER_WEI} before checkpoint"
            )
        return balance

    if intent["phase"] == "skipped_external_funding":
        if int(intent.get("signer_balance_wei", 0)) < REQUIRED_SIGNER_WEI:
            stop("external-funding intent does not prove the exact 0.01 ETH trigger")
        return signer_balance_wei()

    balance = signer_balance_wei()  # A-W1: fresh read immediately before funding.
    if balance >= REQUIRED_SIGNER_WEI:
        intent.update(
            {
                "phase": "skipped_external_funding",
                "skip_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
                "signer_balance_wei": balance,
            }
        )
        atomic_write_json(FUND_INTENT, intent)
        return balance
    if balance != 0:
        stop(
            f"signer has partial balance {balance} below exact external trigger "
            f"{REQUIRED_SIGNER_WEI}; refusing to add another 0.01 ETH"
        )

    deadline_guard("leg 3b0 funding")
    attempt_entries = verified_journal_entries()
    latest_nonce = owner_nonce("latest")
    pending_nonce = owner_nonce("pending")
    if latest_nonce != pending_nonce:
        stop(
            f"owner nonce collision before leg 3b0: latest={latest_nonce}, "
            f"pending={pending_nonce}"
        )
    attempt_block = int(cast_call(["block-number"]))
    intent.update(
        {
            "phase": "broadcast_attempt_started",
            "broadcast_attempt_utc": time.strftime(
                "%Y-%m-%dT%H:%M:%SZ", time.gmtime()
            ),
            "broadcast_attempt_epoch": int(time.time()),
            "ethereum_block_at_attempt": attempt_block,
            "journal_head_at_attempt": journal_head(attempt_entries),
            "owner_nonce_expected": latest_nonce,
            "owner_nonce_latest_at_attempt": latest_nonce,
            "owner_nonce_pending_at_attempt": pending_nonce,
            "signer_balance_wei_at_attempt": balance,
        }
    )
    atomic_write_json(FUND_INTENT, intent)

    proc = run_step(
        "leg3b0-funding",
        [
            "python3",
            "scripts/native_evm_leaf_send.py",
            "--stakehub-home",
            STAKEHUB_HOME,
            "--chain-id",
            "1",
            "--rpc-url",
            RPC,
            "--recipient",
            SIGNER,
            "--amount-wei",
            str(FUND_WEI),
            "--max-fee-wei",
            str(FUND_MAX_FEE_WEI),
            "--expected-recipient-balance-wei",
            "0",
            "--sender",
            OWNER,
            "--expected-sender-nonce",
            str(latest_nonce),
            "--label",
            "leg3b0-signer-funding",
            "--report",
            str(FUND_REPORT),
        ],
        mutation=True,
    )
    evidence = recover_funding_from_report_or_journal(intent, wait_seconds=330)
    if proc.returncode != 0:
        if evidence is not None:
            persist_verified_funding(intent, evidence, recovered=True)
            stop(
                f"leg 3b0 subprocess rc={proc.returncode}, but its exact status=1 "
                "transaction was reconciled; holding before checkpoint"
            )
        stop(
            f"leg 3b0 funding failed rc={proc.returncode}; no duplicate retry is "
            "permitted; see fire/leg3b0-funding.stderr.txt"
        )
    if evidence is None:
        stop("leg 3b0 returned success without a recoverable status=1 receipt")
    balance = persist_verified_funding(intent, evidence, recovered=False)
    if balance < REQUIRED_SIGNER_WEI:
        stop(
            f"leg 3b0 status=1 but signer balance {balance} is below "
            f"required {REQUIRED_SIGNER_WEI} before checkpoint"
        )
    return balance


def run_step(
    name: str,
    cmd: list[str],
    env: dict[str, str] | None = None,
    timeout: int = 1800,
    *,
    mutation: bool,
) -> subprocess.CompletedProcess[str]:
    FIRE.mkdir(parents=True, exist_ok=True)
    command_path = FIRE / f"{name}.cmd.txt"
    atomic_write_json(
        command_path,
        {
            "cmd": cmd,
            "cwd": str(REPO),
            "mutation": mutation,
            "phase": "prepared",
        },
    )
    log(f"FIRE {name}: {' '.join(cmd)}")
    full_env = dict(os.environ)
    if env:
        full_env.update(env)
    if mutation:
        deadline_guard(name)
    proc = subprocess.run(
        cmd,
        capture_output=True,
        text=True,
        timeout=timeout,
        cwd=REPO,
        env=full_env,
        check=False,
    )
    (FIRE / f"{name}.stdout.txt").write_text(proc.stdout, encoding="utf-8")
    (FIRE / f"{name}.stderr.txt").write_text(proc.stderr, encoding="utf-8")
    atomic_write_json(
        command_path,
        {
            "cmd": cmd,
            "cwd": str(REPO),
            "mutation": mutation,
            "phase": "returned",
            "rc": proc.returncode,
        },
    )
    log(f"{name} rc={proc.returncode}")
    return proc


def require_target_checkpoint() -> None:
    height, commitment = verifier_checkpoint()
    if height != 756 or commitment != TARGET_COMMITMENT:
        stop(
            "checkpoint terminal state mismatch: "
            f"height={height}, commitment={commitment}, "
            f"expected height=756 commitment={TARGET_COMMITMENT}"
        )


def fire_sequence(need_funding: bool) -> None:
    deadline_guard("fire sequence")

    if need_funding:
        balance = reconcile_or_fund_signer()
        log(f"leg 3b0/external-funding intent reconciled; current signer balance {balance}")
    else:
        balance = signer_balance_wei()
        if balance < REQUIRED_SIGNER_WEI:
            stop(
                f"signer balance {balance} below exact external trigger "
                f"{REQUIRED_SIGNER_WEI}"
            )
        log(f"signer balance {balance} wei meets exact 0.01 ETH external trigger")

    height, commitment = verifier_checkpoint()
    if height == 756:
        if commitment != TARGET_COMMITMENT:
            stop(
                f"verifier height is 756 but commitment {commitment} != "
                f"{TARGET_COMMITMENT}"
            )
        log("verifier already at exact height 756/target commitment; skipping advance")
    elif height != 691 or commitment != PRIOR_COMMITMENT:
        stop(
            f"unexpected verifier prestate height={height} commitment={commitment}; "
            f"expected 691/{PRIOR_COMMITMENT} or 756/{TARGET_COMMITMENT}"
        )
    else:
        proc = run_step(
            "checkpoint-advance",
            [
                "python3",
                "scripts/a666-mainnet-advance-pftl-checkpoint.py",
                "--execute",
                "--proof-dir",
                str(BASE / "checkpoint-691-756/proof-cuda"),
                "--prior-block-id",
                PRIOR_BLOCK,
                "--target-block-id",
                TARGET_BLOCK,
                "--prior-height",
                "691",
                "--target-height",
                "756",
                "--state-file",
                str(BASE / "checkpoint-691-756/ethereum-state.json"),
            ],
            env={
                "POSTFIAT_SIGNER_SOCKET": SIGNER_SOCKET,
                "POSTFIAT_MUTATION_NOT_AFTER_EPOCH": str(
                    DEADLINE - DEADLINE_MARGIN_S
                ),
            },
            mutation=True,
        )
        if proc.returncode != 0:
            stop(
                f"checkpoint advance failed rc={proc.returncode}; "
                "see fire/checkpoint-advance.stderr.txt"
            )
        require_target_checkpoint()
    log("GATE PASS: verifier height 756 and target commitment exact")

    proc = run_step(
        "leg3b-accept-mint",
        [
            "python3",
            "scripts/a666-mainnet-accept-and-mint.py",
            "--execute",
            "--receipt-witness",
            str(BASE / "receipt-witness.json"),
            "--proof-dir",
            str(BASE / "proof-cuda"),
            "--state-file",
            str(BASE / "mint-state.json"),
            "--expected-finalized-height",
            "787",
        ],
        env={
            "POSTFIAT_SIGNER_SOCKET": SIGNER_SOCKET,
            "POSTFIAT_MUTATION_NOT_AFTER_EPOCH": str(
                DEADLINE - DEADLINE_MARGIN_S
            ),
        },
        mutation=True,
    )
    if proc.returncode != 0:
        stop(
            f"leg 3b failed rc={proc.returncode}; "
            "see fire/leg3b-accept-mint.stderr.txt"
        )
    state = json.loads((BASE / "mint-state.json").read_text(encoding="utf-8"))
    pre = state["pre_state"]
    post = state.get("post_state", {})
    recipient_delta = int(post.get("recipient_balance_atoms", 0)) - int(
        pre["recipient_balance_atoms"]
    )
    supply_delta = int(post.get("token_total_supply", 0)) - int(
        pre["token_total_supply"]
    )
    if state.get("phase") != "minted-to-recipient":
        stop(f"leg 3b terminal phase {state.get('phase')!r} != minted-to-recipient")
    if not post.get("receipt_accepted") or not post.get("packet_consumed"):
        stop("leg 3b terminal accepted/consumed state is false")
    if recipient_delta != MINT_DELTA or supply_delta != MINT_DELTA:
        stop(
            f"leg 3b delta mismatch: recipient {recipient_delta}, "
            f"supply {supply_delta}, expected {MINT_DELTA}"
        )
    if int(post.get("migration_reserve_atoms", -1)) != int(
        pre["migration_reserve_atoms"]
    ):
        stop("leg 3b migration reserve changed")
    for transaction in state.get("transactions", []):
        if int(transaction.get("status", 0)) != 1:
            stop(
                f"leg 3b transaction {transaction.get('label')!r} "
                "does not have Ethereum receipt status=1"
            )
    log(f"GATE PASS: accepted+consumed; exact deltas +{MINT_DELTA}; receipts status=1")
    atomic_write_json(
        BASE / "LEG3B-DONE.txt",
        {
            "finished_utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "transactions": state.get("transactions", []),
            "recipient_delta": recipient_delta,
            "supply_delta": supply_delta,
        },
    )
    log(
        "SEQUENCE COMPLETE — next per ruling: PR 7 + master checkout + restart "
        "+ one unlock BEFORE 3c"
    )


def run_triggered_sequence(*, need_funding: bool) -> None:
    try:
        fire_sequence(need_funding=need_funding)
    except SystemExit:
        raise
    except Exception as error:  # noqa: BLE001 - every post-trigger error is terminal.
        stop(
            "triggered leg 3b sequence raised an unexpected error; "
            f"{type(error).__name__}: {error}"
        )


def main() -> None:
    lock_path = BASE / "fire_watcher.lock"
    lock = open(lock_path, "w", encoding="utf-8")
    try:
        fcntl.flock(lock, fcntl.LOCK_EX | fcntl.LOCK_NB)
    except BlockingIOError:
        print("another fire_watcher instance holds the lock; exiting", flush=True)
        raise SystemExit(1)
    if (BASE / "STOP.txt").exists():
        print("STOP.txt present; refusing to run until an operator removes it", flush=True)
        raise SystemExit(2)
    if (BASE / "LEG3B-DONE.txt").exists():
        print("LEG3B-DONE.txt present; nothing to do", flush=True)
        raise SystemExit(0)
    log("fire watcher armed: waiting for exact 0.01 ETH funding or whitelist entry")
    while True:
        need_funding: bool | None = None
        try:
            deadline_guard("poll")
            balance = signer_balance_wei()
            if balance >= REQUIRED_SIGNER_WEI:
                log(f"TRIGGER: signer funded externally ({balance} wei)")
                need_funding = False
            elif whitelist_has_signer():
                log("TRIGGER: signer entered agentd whitelist")
                need_funding = True
        except SystemExit:
            raise
        except Exception as error:  # noqa: BLE001 - only read-only poll errors retry.
            log(f"poll error (retryable): {error}")
        if need_funding is not None:
            run_triggered_sequence(need_funding=need_funding)
            return
        time.sleep(30)


if __name__ == "__main__":
    main()
