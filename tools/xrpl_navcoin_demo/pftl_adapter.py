"""Adapter for the already-running six-validator hardened PFTL devnet."""

from __future__ import annotations

from dataclasses import dataclass
import json
import os
from pathlib import Path
import subprocess
from typing import Any


PFTL_ROOT = Path("/home/postfiat/tmp/pftl-proven-nav-v2-20260724")
PFTL_BIN = Path(
    "/home/postfiat/tmp/pftl-escrow-ae3c53c-00616722/postfiat-node"
)
ASSET_ID = (
    "f912599013445352dc064b8b07be3815db5f494eff7e7097b2d6a72ff333bbfc"
    "af51954e35fe28558525541f5fb945b5"
)
COORDINATOR = "pf795e0f3882f9986b303aadb864cee1e68fab6a86"
USER = "pf05225d912418ff1cdac84eff982002cf9ca915d4"


class PftlError(RuntimeError):
    pass


@dataclass(frozen=True)
class PftlEscrowRef:
    escrow_id: str
    owner: str
    recipient: str
    amount_atoms: int
    cancel_after: int
    condition: str
    create_tx_id: str


class HardenedPftl:
    def __init__(self, evidence_dir: Path) -> None:
        self.evidence_dir = evidence_dir
        self.evidence_dir.mkdir(parents=True, exist_ok=True)
        self.helper = PFTL_ROOT / "public/certify-signed-escrow.sh"
        if not PFTL_BIN.is_file() or not self.helper.is_file():
            raise PftlError("hardened binary or certification helper is missing")

    def _run(self, arguments: list[str]) -> dict[str, Any]:
        completed = subprocess.run(
            [str(PFTL_BIN), *arguments],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        try:
            return json.loads(completed.stdout)
        except json.JSONDecodeError as error:
            raise PftlError(f"non-JSON PFTL response: {completed.stdout}") from error

    @property
    def node0(self) -> Path:
        return PFTL_ROOT / "nodes/validator-0"

    def status(self, node: int = 0) -> dict[str, Any]:
        return self._run(
            ["status", "--data-dir", str(PFTL_ROOT / f"nodes/validator-{node}")]
        )

    def converged_status(self) -> dict[str, Any]:
        statuses = [self.status(index) for index in range(6)]
        pairs = {(item["block_height"], item["state_root"]) for item in statuses}
        if len(pairs) != 1:
            raise PftlError("PFTL validators are not converged")
        return {
            "height": statuses[0]["block_height"],
            "state_root": statuses[0]["state_root"],
            "validators": 6,
        }

    def _rpc(self, method: str, arguments: list[str]) -> dict[str, Any]:
        response = self._run(
            ["rpc", "--method", method, "--data-dir", str(self.node0), *arguments]
        )
        if not response.get("ok"):
            raise PftlError(f"PFTL RPC {method} failed: {response.get('error')}")
        return response["result"]

    def balance(self, account: str) -> int:
        result = self._rpc(
            "account_lines", ["--account", account, "--asset-id", ASSET_ID]
        )
        lines = result["lines"]
        if len(lines) != 1:
            raise PftlError("expected exactly one NAVcoin trustline")
        return int(lines[0]["balance"])

    def escrows(self, account: str, *, role: str | None = None) -> list[dict[str, Any]]:
        arguments = ["--account", account, "--limit", "100"]
        if role:
            arguments.extend(["--role", role])
        return self._rpc("account_escrows", arguments)["escrows"]

    def escrow_info(self, escrow_id: str) -> dict[str, Any]:
        return self._rpc("escrow_info", ["--escrow-id", escrow_id])

    @staticmethod
    def key_for(account: str) -> Path:
        role = {COORDINATOR: "coordinator", USER: "user"}.get(account)
        if role is None:
            raise PftlError("unknown demo signer")
        return PFTL_ROOT / f"private/wallets/{role}/transparent-0.key.json"

    def _sign_and_certify(
        self,
        *,
        label: str,
        source: str,
        operation: dict[str, Any],
    ) -> tuple[dict[str, Any], dict[str, Any]]:
        operation_json = json.dumps(operation, sort_keys=True, separators=(",", ":"))
        quote = self._run(
            [
                "escrow-fee-quote",
                "--data-dir",
                str(self.node0),
                "--source",
                source,
                "--operation-json",
                operation_json,
            ]
        )
        quote_path = self.evidence_dir / f"{label}.quote.json"
        quote_path.write_text(json.dumps(quote, indent=2, sort_keys=True) + "\n")
        signed = self._run(
            [
                "wallet-sign-escrow-transaction",
                "--key-file",
                str(self.key_for(source)),
                "--quote-file",
                str(quote_path),
            ]
        )
        signed_path = self.evidence_dir / f"{label}.signed.private.json"
        signed_path.write_text(json.dumps(signed, indent=2, sort_keys=True) + "\n")
        os.chmod(signed_path, 0o600)
        completed = subprocess.run(
            [str(self.helper), str(signed_path)],
            check=True,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        history = self._run(
            [
                "account-tx",
                "--data-dir",
                str(self.node0),
                "--address",
                source,
                "--limit",
                "100",
            ]
        )
        matching = [
            row
            for row in history["rows"]
            if row.get("sequence") == signed["unsigned"]["sequence"]
            and row.get("transaction_kind") == signed["unsigned"]["transaction_kind"]
        ]
        if len(matching) != 1 or not matching[0].get("accepted"):
            raise PftlError(f"{label}: certified transaction not found in account history")
        public = {
            "schema": "postfiat.pftl.certified_escrow_operation.v1",
            "label": label,
            "operation": operation,
            "source": source,
            "sequence": signed["unsigned"]["sequence"],
            "fee": signed["unsigned"]["fee"],
            "tx_id": matching[0]["tx_id"],
            "block_height": matching[0]["block_height"],
            "receipt_code": matching[0]["receipt_code"],
            "certification_stdout": completed.stdout.strip(),
            "converged_after": self.converged_status(),
        }
        public_path = self.evidence_dir / f"{label}.certified.json"
        public_path.write_text(json.dumps(public, indent=2, sort_keys=True) + "\n")
        os.chmod(public_path, 0o644)
        return signed, public

    def create(
        self,
        *,
        label: str,
        owner: str,
        recipient: str,
        amount_atoms: int,
        condition: str,
        cancel_after: int,
    ) -> tuple[PftlEscrowRef, dict[str, Any]]:
        before = {item["escrow_id"] for item in self.escrows(owner, role="owner")}
        operation = {
            "operation": "escrow_create",
            "owner": owner,
            "recipient": recipient,
            "asset_id": ASSET_ID,
            "amount": amount_atoms,
            "condition": condition,
            "cancel_after": cancel_after,
        }
        signed, report = self._sign_and_certify(
            label=label, source=owner, operation=operation
        )
        new = [
            item
            for item in self.escrows(owner, role="owner")
            if item["escrow_id"] not in before
        ]
        if len(new) != 1:
            raise PftlError(f"{label}: expected one new escrow, got {len(new)}")
        item = new[0]
        ref = PftlEscrowRef(
            escrow_id=item["escrow_id"],
            owner=owner,
            recipient=recipient,
            amount_atoms=amount_atoms,
            cancel_after=cancel_after,
            condition=condition,
            create_tx_id=report["tx_id"] or signed["unsigned"].get("transaction_id", ""),
        )
        return ref, report

    def finish(
        self, *, label: str, escrow: PftlEscrowRef, fulfillment: str
    ) -> dict[str, Any]:
        operation = {
            "operation": "escrow_finish",
            "escrow_id": escrow.escrow_id,
            "owner": escrow.owner,
            "recipient": escrow.recipient,
            "fulfillment": fulfillment,
        }
        _, report = self._sign_and_certify(
            label=label, source=escrow.recipient, operation=operation
        )
        return report

    def cancel(self, *, label: str, escrow: PftlEscrowRef) -> dict[str, Any]:
        operation = {
            "operation": "escrow_cancel",
            "escrow_id": escrow.escrow_id,
            "owner": escrow.owner,
            "recipient": escrow.recipient,
        }
        _, report = self._sign_and_certify(
            label=label, source=escrow.owner, operation=operation
        )
        return report
