"""Direct LND gRPC transport through the pinned official ``lncli`` client.

``lncli`` is used only as a generated-protobuf gRPC frontend. It connects
straight to the three local LND gRPC endpoints with TLS and macaroons; there is
no REST layer, hosted API, or public network dependency.
"""

from __future__ import annotations

from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import subprocess
from typing import Any, Callable, Mapping, Sequence

from .coordinator.protocol import (
    LndInvoiceFacts,
    SecretPreimage,
    payment_hash,
)


class LightningTransportError(RuntimeError):
    """A direct local LND gRPC operation failed or returned invalid data."""


def _uint(value: Any, field: str) -> int:
    if type(value) is int:
        parsed = value
    elif type(value) is str and value.isascii() and value.isdecimal():
        parsed = int(value)
    else:
        raise LightningTransportError(f"{field} is not an unsigned integer")
    if parsed < 0 or parsed > (1 << 63) - 1:
        raise LightningTransportError(f"{field} is outside uint63")
    return parsed


def _hex32(value: Any, field: str) -> str:
    if (
        type(value) is not str
        or len(value) != 64
        or any(character not in "0123456789abcdef" for character in value)
    ):
        raise LightningTransportError(f"{field} is not canonical 32-byte hex")
    return value


def _redact_preimages(value: Any) -> Any:
    if isinstance(value, Mapping):
        output: dict[str, Any] = {}
        for key, child in value.items():
            if str(key).lower() in {"preimage", "payment_preimage", "r_preimage"}:
                output[str(key)] = "<redacted>"
            else:
                output[str(key)] = _redact_preimages(child)
        return output
    if isinstance(value, list):
        return [_redact_preimages(child) for child in value]
    return value


@dataclass(frozen=True)
class Invoice:
    payment_request: str
    payment_hash: str
    add_index: int
    payment_addr: str
    facts: LndInvoiceFacts


@dataclass(frozen=True, repr=False)
class PaymentResult:
    payment_hash: str
    payment_preimage: SecretPreimage | None
    value_msat: int
    fee_msat: int
    status: str
    failure_reason: str
    payer_htlc_expiries: tuple[int, ...]
    public_response: Mapping[str, Any]

    def __repr__(self) -> str:
        return (
            f"PaymentResult(payment_hash={self.payment_hash!r}, "
            f"payment_preimage={'<redacted>' if self.payment_preimage else None}, "
            f"value_msat={self.value_msat}, fee_msat={self.fee_msat}, "
            f"status={self.status!r}, failure_reason={self.failure_reason!r})"
        )


Executor = Callable[[str, Sequence[str], float], Mapping[str, Any]]


class DirectLncliGrpc:
    """Typed calls to local LND using its pinned official gRPC client."""

    def __init__(
        self,
        env_script: Path,
        *,
        executor: Executor | None = None,
        command_timeout_seconds: float = 120,
    ) -> None:
        self.env_script = env_script.resolve()
        if executor is None and not self.env_script.is_file():
            raise ValueError(f"environment script does not exist: {self.env_script}")
        if command_timeout_seconds <= 0:
            raise ValueError("command timeout must be positive")
        self._executor = executor or self._execute
        self.command_timeout_seconds = command_timeout_seconds

    def _execute(
        self, node: str, arguments: Sequence[str], timeout: float
    ) -> Mapping[str, Any]:
        if node not in {"user", "coordinator", "router"}:
            raise ValueError("unknown LND node")
        completed = subprocess.run(
            [
                str(self.env_script),
                "host-lncli",
                node,
                *arguments,
            ],
            cwd=self.env_script.resolve().parents[1],
            check=False,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
        )
        if completed.returncode != 0:
            # Never interpolate arguments: AddInvoice arguments contain the
            # synthetic preimage and must not escape into general logs. The
            # client stderr is hashed rather than copied for the same reason.
            stderr_bytes = completed.stderr.encode("utf-8", errors="replace")
            raise LightningTransportError(
                f"direct LND gRPC call failed for node={node}; "
                f"rc={completed.returncode}; "
                f"stderr_bytes={len(stderr_bytes)}; "
                f"stderr_sha256={hashlib.sha256(stderr_bytes).hexdigest()}"
            )
        try:
            result = json.loads(completed.stdout)
        except json.JSONDecodeError as error:
            raise LightningTransportError("direct LND gRPC returned non-JSON") from error
        if not isinstance(result, Mapping):
            raise LightningTransportError("direct LND gRPC JSON is not an object")
        return result

    def _call(
        self,
        node: str,
        *arguments: str,
        timeout: float | None = None,
    ) -> Mapping[str, Any]:
        return self._executor(
            node,
            arguments,
            self.command_timeout_seconds if timeout is None else timeout,
        )

    def get_info(self, node: str) -> Mapping[str, Any]:
        value = self._call(node, "getinfo")
        chains = value.get("chains")
        if chains != [{"chain": "bitcoin", "network": "regtest"}]:
            raise LightningTransportError(f"{node} is not on Bitcoin regtest")
        if value.get("synced_to_chain") is not True:
            raise LightningTransportError(f"{node} is not synchronized")
        return value

    def decode_invoice_response(
        self, node: str, payment_request: str
    ) -> tuple[LndInvoiceFacts, Mapping[str, Any]]:
        if not payment_request:
            raise ValueError("payment_request is required")
        decoded = self._call(node, "decodepayreq", payment_request)
        try:
            facts = LndInvoiceFacts.from_decode_pay_req(decoded, network="regtest")
        except ValueError as error:
            raise LightningTransportError(str(error)) from error
        if facts.is_amp:
            raise LightningTransportError("AMP invoices are unsupported")
        return facts, decoded

    def decode_invoice(self, node: str, payment_request: str) -> LndInvoiceFacts:
        facts, _ = self.decode_invoice_response(node, payment_request)
        return facts

    def add_invoice(
        self,
        node: str,
        secret: SecretPreimage,
        *,
        amount_msat: int,
        expiry_seconds: int = 900,
        min_final_cltv_delta: int = 144,
        memo: str,
    ) -> Invoice:
        if type(amount_msat) is not int or amount_msat <= 0:
            raise ValueError("amount_msat must be a positive integer")
        if expiry_seconds <= 0 or min_final_cltv_delta <= 0:
            raise ValueError("invoice expiry and CLTV delta must be positive")
        response = self._call(
            node,
            "addinvoice",
            f"--preimage={secret.protocol_hex()}",
            f"--amt_msat={amount_msat}",
            f"--expiry={expiry_seconds}",
            f"--cltv_expiry_delta={min_final_cltv_delta}",
            "--private",
            f"--memo={memo}",
        )
        observed_hash = _hex32(response.get("r_hash"), "r_hash")
        expected_hash = payment_hash(secret).hex()
        if observed_hash != expected_hash:
            raise LightningTransportError("LND invoice hash differs from supplied preimage")
        payment_request = response.get("payment_request")
        if type(payment_request) is not str or not payment_request:
            raise LightningTransportError("LND returned no payment request")
        facts = self.decode_invoice(node, payment_request)
        if facts.payment_hash.hex() != expected_hash:
            raise LightningTransportError("decoded BOLT11 payment hash mismatch")
        if facts.amount_msat != amount_msat:
            raise LightningTransportError("decoded BOLT11 amount mismatch")
        if facts.min_final_cltv_delta != min_final_cltv_delta:
            raise LightningTransportError("decoded BOLT11 final CLTV mismatch")
        return Invoice(
            payment_request=payment_request,
            payment_hash=observed_hash,
            add_index=_uint(response.get("add_index"), "add_index"),
            payment_addr=_hex32(response.get("payment_addr"), "payment_addr"),
            facts=facts,
        )

    def add_invoice_generated(
        self,
        node: str,
        *,
        amount_msat: int,
        expiry_seconds: int = 900,
        min_final_cltv_delta: int = 144,
        memo: str,
    ) -> Invoice:
        """Let the receiver's LND generate and retain the invoice preimage."""

        if type(amount_msat) is not int or amount_msat <= 0:
            raise ValueError("amount_msat must be a positive integer")
        if expiry_seconds <= 0 or min_final_cltv_delta <= 0:
            raise ValueError("invoice expiry and CLTV delta must be positive")
        response = self._call(
            node,
            "addinvoice",
            f"--amt_msat={amount_msat}",
            f"--expiry={expiry_seconds}",
            f"--cltv_expiry_delta={min_final_cltv_delta}",
            "--private",
            f"--memo={memo}",
        )
        observed_hash = _hex32(response.get("r_hash"), "r_hash")
        payment_request = response.get("payment_request")
        if type(payment_request) is not str or not payment_request:
            raise LightningTransportError("LND returned no payment request")
        facts = self.decode_invoice(node, payment_request)
        if facts.payment_hash.hex() != observed_hash:
            raise LightningTransportError("decoded BOLT11 payment hash mismatch")
        if facts.amount_msat != amount_msat:
            raise LightningTransportError("decoded BOLT11 amount mismatch")
        if facts.min_final_cltv_delta != min_final_cltv_delta:
            raise LightningTransportError("decoded BOLT11 final CLTV mismatch")
        return Invoice(
            payment_request=payment_request,
            payment_hash=observed_hash,
            add_index=_uint(response.get("add_index"), "add_index"),
            payment_addr=_hex32(response.get("payment_addr"), "payment_addr"),
            facts=facts,
        )

    def add_amp_invoice_for_rejection_test(
        self,
        node: str,
        *,
        amount_msat: int,
        expiry_seconds: int = 900,
        min_final_cltv_delta: int = 144,
    ) -> str:
        """Create an AMP invoice solely to prove the adapter rejects it.

        The returned request must be passed to ``decode_invoice`` and must
        never be paid by this harness.
        """

        if type(amount_msat) is not int or amount_msat <= 0:
            raise ValueError("amount_msat must be a positive integer")
        response = self._call(
            node,
            "addinvoice",
            "--amp",
            f"--amt_msat={amount_msat}",
            f"--expiry={expiry_seconds}",
            f"--cltv_expiry_delta={min_final_cltv_delta}",
            "--private",
            "--memo=synthetic-amp-rejection-test",
        )
        payment_request = response.get("payment_request")
        if type(payment_request) is not str or not payment_request:
            raise LightningTransportError("LND returned no AMP payment request")
        return payment_request

    def pay_invoice(
        self,
        node: str,
        payment_request: str,
        *,
        fee_limit_sat: int = 20,
        max_total_cltv_delta: int = 288,
        timeout_seconds: int = 30,
    ) -> PaymentResult:
        if fee_limit_sat < 0 or max_total_cltv_delta <= 0 or timeout_seconds <= 0:
            raise ValueError("payment safety limits are invalid")
        facts = self.decode_invoice(node, payment_request)
        response = self._call(
            node,
            "payinvoice",
            "--force",
            f"--fee_limit={fee_limit_sat}",
            f"--timeout={timeout_seconds}s",
            f"--cltv_limit={max_total_cltv_delta}",
            "--max_parts=1",
            "--json",
            payment_request,
            timeout=float(timeout_seconds) + 30,
        )
        return self._payment_result(
            response,
            expected_payment_hash=facts.payment_hash.hex(),
        )

    def track_payment(
        self,
        node: str,
        payment_hash_hex: str,
        *,
        timeout_seconds: int = 30,
    ) -> PaymentResult:
        """Reconcile an already-started payer-side payment by its durable hash."""

        payment_hash_hex = _hex32(payment_hash_hex, "payment_hash")
        if timeout_seconds <= 0:
            raise ValueError("payment tracking timeout must be positive")
        response = self._call(
            node,
            "trackpayment",
            "--json",
            payment_hash_hex,
            timeout=float(timeout_seconds) + 5,
        )
        return self._payment_result(
            response,
            expected_payment_hash=payment_hash_hex,
        )

    @staticmethod
    def _payment_result(
        response: Mapping[str, Any],
        *,
        expected_payment_hash: str,
    ) -> PaymentResult:
        status = str(response.get("status", ""))
        observed_hash = _hex32(
            response.get("payment_hash", expected_payment_hash),
            "payment_hash",
        )
        if observed_hash != expected_payment_hash:
            raise LightningTransportError(
                "payment response hash differs from expected payment"
            )
        preimage: SecretPreimage | None = None
        raw_preimage = response.get("payment_preimage")
        if raw_preimage not in (None, "", "0" * 64):
            preimage = SecretPreimage.from_hex(_hex32(raw_preimage, "payment_preimage"))
            if (
                hashlib.sha256(preimage.reveal_for_protocol()).hexdigest()
                != expected_payment_hash
            ):
                raise LightningTransportError("settled preimage does not satisfy invoice")
        if status == "SUCCEEDED" and preimage is None:
            raise LightningTransportError("successful payment did not reveal a preimage")
        if status != "SUCCEEDED" and preimage is not None:
            raise LightningTransportError("failed payment unexpectedly revealed a preimage")

        expiries: list[int] = []
        htlcs = response.get("htlcs", [])
        if not isinstance(htlcs, list):
            raise LightningTransportError("payment htlcs must be a list")
        for attempt in htlcs:
            if not isinstance(attempt, Mapping):
                raise LightningTransportError("payment HTLC attempt must be an object")
            route = attempt.get("route")
            if not isinstance(route, Mapping):
                continue
            total_time_lock = route.get("total_time_lock")
            if total_time_lock is not None:
                expiries.append(
                    _uint(total_time_lock, "HTLC route total_time_lock")
                )
                continue
            hops = route.get("hops", [])
            if not isinstance(hops, list):
                raise LightningTransportError("payment route hops must be a list")
            if hops and isinstance(hops[0], Mapping) and "expiry" in hops[0]:
                # The first hop is the payer's outgoing HTLC. Later-hop
                # expiries are smaller and do not define the payer refund
                # boundary.
                expiries.append(_uint(hops[0]["expiry"], "payer HTLC expiry"))

        return PaymentResult(
            payment_hash=observed_hash,
            payment_preimage=preimage,
            value_msat=_uint(response.get("value_msat", 0), "value_msat"),
            fee_msat=_uint(response.get("fee_msat", 0), "fee_msat"),
            status=status,
            failure_reason=str(response.get("failure_reason", "")),
            payer_htlc_expiries=tuple(expiries),
            public_response=_redact_preimages(response),
        )

    def lookup_invoice(
        self, node: str, payment_hash_hex: str
    ) -> Mapping[str, Any]:
        payment_hash_hex = _hex32(payment_hash_hex, "payment_hash")
        response = self._call(node, "lookupinvoice", payment_hash_hex)
        observed = response.get("r_hash")
        if observed not in (None, "") and _hex32(observed, "r_hash") != payment_hash_hex:
            raise LightningTransportError("lookup returned a different invoice hash")
        return _redact_preimages(response)

    def list_channels(self, node: str) -> Mapping[str, Any]:
        return self._call(node, "listchannels")
