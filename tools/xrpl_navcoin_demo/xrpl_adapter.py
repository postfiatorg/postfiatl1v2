"""Small, evidence-oriented XRPL Testnet adapter.

Every transaction is autofilled, signed, and durably persisted before submit.
Success means a validated ``tesSUCCESS`` result, never a preliminary submit
response.  Wallet secrets are accepted by this module but never returned from
its public evidence methods.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import os
from pathlib import Path
import time
from typing import Any

from xrpl.clients import JsonRpcClient
from xrpl.core.binarycodec import encode
from xrpl.models.requests import AccountInfo, AccountObjects, Ledger, Tx
from xrpl.models.transactions import EscrowCancel, EscrowCreate, EscrowFinish
from xrpl.transaction import autofill_and_sign, submit_and_wait
from xrpl.wallet import Wallet


TESTNET_URL = "https://s.altnet.rippletest.net:51234"
RIPPLE_EPOCH_UNIX = 946_684_800


class XrplError(RuntimeError):
    pass


@dataclass(frozen=True)
class XrplEscrowRef:
    owner: str
    destination: str
    offer_sequence: int
    amount_drops: int
    condition: str
    cancel_after: int
    create_tx_hash: str


def _atomic_private_json(path: Path, value: Any) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    temporary = path.with_suffix(path.suffix + ".tmp")
    descriptor = os.open(
        temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600
    )
    with os.fdopen(descriptor, "w", encoding="utf-8") as stream:
        json.dump(value, stream, indent=2, sort_keys=True)
        stream.write("\n")
        stream.flush()
        os.fsync(stream.fileno())
    os.replace(temporary, path)
    os.chmod(path, 0o600)


def save_wallet(path: Path, wallet: Wallet) -> None:
    _atomic_private_json(
        path,
        {
            "schema": "postfiat.xrpl_testnet_wallet.v1",
            "network": "XRPL Testnet",
            "classic_address": wallet.classic_address,
            "public_key": wallet.public_key,
            "seed": wallet.seed,
        },
    )


def load_wallet(path: Path) -> Wallet:
    raw = json.loads(path.read_text(encoding="utf-8"))
    if raw.get("network") != "XRPL Testnet":
        raise XrplError("wallet is not explicitly scoped to XRPL Testnet")
    wallet = Wallet.from_seed(raw["seed"])
    if wallet.classic_address != raw["classic_address"]:
        raise XrplError("wallet address does not match seed")
    return wallet


class XrplTestnet:
    def __init__(self, evidence_dir: Path, endpoint: str = TESTNET_URL) -> None:
        if endpoint != TESTNET_URL:
            raise XrplError("this prototype is pinned to the XRPL Testnet endpoint")
        self.endpoint = endpoint
        self.client = JsonRpcClient(endpoint)
        self.evidence_dir = evidence_dir
        self.evidence_dir.mkdir(parents=True, exist_ok=True)

    def _result(self, request: Any) -> dict[str, Any]:
        response = self.client.request(request)
        value = response.result
        if not isinstance(value, dict):
            raise XrplError("XRPL response is not an object")
        if value.get("error"):
            raise XrplError(f"XRPL error: {value['error']}")
        return value

    def ledger_clock(self) -> dict[str, Any]:
        result = self._result(Ledger(ledger_index="validated"))
        ledger = result["ledger"]
        close_time = int(ledger["close_time"])
        return {
            "ledger_index": int(ledger["ledger_index"]),
            "ledger_hash": ledger["ledger_hash"],
            "close_time": close_time,
            "close_time_unix": close_time + RIPPLE_EPOCH_UNIX,
            "validated": bool(result["validated"]),
        }

    def account(self, address: str) -> dict[str, Any]:
        result = self._result(
            AccountInfo(account=address, ledger_index="validated", strict=True)
        )
        data = result["account_data"]
        return {
            "address": address,
            "balance_drops": int(data["Balance"]),
            "sequence": int(data["Sequence"]),
            "owner_count": int(data["OwnerCount"]),
            "ledger_index": int(result["ledger_index"]),
            "validated": bool(result["validated"]),
        }

    def escrows(self, owner: str) -> list[dict[str, Any]]:
        result = self._result(
            AccountObjects(
                account=owner,
                ledger_index="validated",
                type="escrow",
                limit=400,
            )
        )
        return list(result.get("account_objects", []))

    def tx(self, transaction_hash: str) -> dict[str, Any]:
        result = self._result(Tx(transaction=transaction_hash))
        if not result.get("validated"):
            raise XrplError("transaction is not validated")
        return result

    def _persist_and_submit(
        self, *, label: str, transaction: Any, wallet: Wallet
    ) -> dict[str, Any]:
        signed = autofill_and_sign(transaction, self.client, wallet)
        signed_json = signed.to_xrpl()
        blob = encode(signed_json)
        tx_hash = signed.get_hash()
        intent = {
            "schema": "postfiat.xrpl_signed_intent.v1",
            "network": "XRPL Testnet",
            "endpoint": self.endpoint,
            "tx_hash": tx_hash,
            "account": signed.account,
            "sequence": signed.sequence,
            "last_ledger_sequence": signed.last_ledger_sequence,
            "signed_transaction": signed_json,
            "tx_blob": blob,
        }
        intent_path = self.evidence_dir / f"{label}.signed-intent.private.json"
        _atomic_private_json(intent_path, intent)

        response = submit_and_wait(
            signed,
            self.client,
            autofill=False,
            check_fee=False,
            fail_hard=True,
        )
        result = response.result
        if not isinstance(result, dict) or not result.get("validated"):
            raise XrplError(f"{label}: XRPL result is not validated")
        meta = result.get("meta")
        code = meta.get("TransactionResult") if isinstance(meta, dict) else None
        if code != "tesSUCCESS":
            raise XrplError(f"{label}: validated result is {code!r}")
        if result.get("hash") != tx_hash:
            raise XrplError(f"{label}: validated hash does not match signed intent")

        public = {
            "schema": "postfiat.xrpl_validated_transaction.v1",
            "network": "XRPL Testnet",
            "endpoint": self.endpoint,
            "hash": tx_hash,
            "ledger_index": int(result["ledger_index"]),
            "validated": True,
            "transaction_result": code,
            "fee_drops": int(result["tx_json"]["Fee"]),
            "tx_json": result["tx_json"],
            "meta": meta,
            "signed_intent_file": intent_path.name,
        }
        path = self.evidence_dir / f"{label}.validated.json"
        path.write_text(json.dumps(public, indent=2, sort_keys=True) + "\n")
        os.chmod(path, 0o644)
        return public

    def create(
        self,
        *,
        label: str,
        wallet: Wallet,
        destination: str,
        amount_drops: int,
        condition: str,
        cancel_after: int,
    ) -> tuple[XrplEscrowRef, dict[str, Any]]:
        tx = EscrowCreate(
            account=wallet.classic_address,
            destination=destination,
            amount=str(amount_drops),
            condition=condition,
            cancel_after=cancel_after,
        )
        result = self._persist_and_submit(label=label, transaction=tx, wallet=wallet)
        tx_json = result["tx_json"]
        ref = XrplEscrowRef(
            owner=wallet.classic_address,
            destination=destination,
            offer_sequence=int(tx_json["Sequence"]),
            amount_drops=amount_drops,
            condition=condition,
            cancel_after=cancel_after,
            create_tx_hash=result["hash"],
        )
        return ref, result

    def finish(
        self,
        *,
        label: str,
        wallet: Wallet,
        escrow: XrplEscrowRef,
        fulfillment: str,
    ) -> dict[str, Any]:
        return self._persist_and_submit(
            label=label,
            wallet=wallet,
            transaction=EscrowFinish(
                account=wallet.classic_address,
                owner=escrow.owner,
                offer_sequence=escrow.offer_sequence,
                condition=escrow.condition,
                fulfillment=fulfillment,
            ),
        )

    def cancel(
        self, *, label: str, wallet: Wallet, escrow: XrplEscrowRef
    ) -> dict[str, Any]:
        return self._persist_and_submit(
            label=label,
            wallet=wallet,
            transaction=EscrowCancel(
                account=wallet.classic_address,
                owner=escrow.owner,
                offer_sequence=escrow.offer_sequence,
            ),
        )

    def wait_until_close_time(
        self, target: int, *, deadline_seconds: int = 180
    ) -> dict[str, Any]:
        deadline = time.monotonic() + deadline_seconds
        while time.monotonic() < deadline:
            clock = self.ledger_clock()
            if clock["close_time"] >= target:
                return clock
            time.sleep(2)
        raise XrplError("XRPL validated close time did not reach target")

