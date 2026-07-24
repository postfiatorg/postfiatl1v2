"""Direct LND gRPC adapter with injected generated-protobuf constructors.

Bootstrap may use ``lncli``, but runtime invoice/payment operations in this
adapter call LND's Lightning gRPC stub directly. Generated LND modules are
environment artifacts, so the coordinator accepts their message constructors
instead of importing an unpinned global module.
"""

from __future__ import annotations

from dataclasses import dataclass
import hmac
from typing import Any, Callable, Mapping

from .protocol import (
    LndInvoiceFacts,
    ProtocolEncodingError,
    SecretPreimage,
    payment_hash,
    validate_invoice_binding,
)

MAX_PAYMENT_UPDATES = 4096


class LndGrpcError(RuntimeError):
    """An LND request failed or returned inconsistent protocol data."""


class LightningPaymentError(LndGrpcError):
    """LND conclusively rejected or failed a payment."""


@dataclass(frozen=True)
class LndRequestFactories:
    """Constructors from generated Lightning and Router protobuf modules."""

    invoice: Callable[..., Any]
    pay_req_string: Callable[..., Any]
    payment_hash: Callable[..., Any]
    send_payment_request: Callable[..., Any]

    @classmethod
    def from_proto_modules(
        cls, lightning_pb2: Any, router_pb2: Any
    ) -> "LndRequestFactories":
        return cls(
            invoice=lightning_pb2.Invoice,
            pay_req_string=lightning_pb2.PayReqString,
            payment_hash=lightning_pb2.PaymentHash,
            send_payment_request=router_pb2.SendPaymentRequest,
        )


@dataclass(frozen=True)
class CreatedInvoice:
    payment_request: str
    payment_hash: bytes
    add_index: int
    payment_addr: bytes
    facts: LndInvoiceFacts


@dataclass(frozen=True, repr=False)
class SettledPayment:
    payment_hash: bytes
    payment_preimage: SecretPreimage
    fee_sat: int | None
    payer_htlc_expiries: tuple[int, ...]

    def __repr__(self) -> str:
        return (
            "SettledPayment(payment_hash="
            f"{self.payment_hash.hex()}, payment_preimage=<redacted>, "
            f"fee_sat={self.fee_sat!r}, "
            f"payer_htlc_expiries={self.payer_htlc_expiries!r})"
        )


@dataclass(frozen=True)
class InvoiceStatus:
    payment_hash: bytes
    settled: bool
    state: int | str | None
    amount_paid_msat: int
    add_index: int
    settle_index: int
    is_amp: bool


def _response_field(response: Any, name: str, default: Any = None) -> Any:
    if isinstance(response, Mapping):
        return response.get(name, default)
    return getattr(response, name, default)


def _bytes32(value: Any, field: str) -> bytes:
    if type(value) is bytes:
        if len(value) != 32:
            raise LndGrpcError(f"LND {field} must be 32 bytes")
        return value
    if type(value) is str:
        try:
            decoded = bytes.fromhex(value)
        except ValueError as error:
            raise LndGrpcError(f"LND {field} is not hexadecimal") from error
        if len(decoded) != 32 or value != decoded.hex():
            raise LndGrpcError(f"LND {field} is not canonical 32-byte hex")
        return decoded
    raise LndGrpcError(f"LND {field} has unsupported type")


def _uint(value: Any, field: str) -> int:
    if type(value) is int:
        parsed = value
    elif type(value) is str and value.isascii() and value.isdecimal():
        parsed = int(value, 10)
    else:
        raise LndGrpcError(f"LND {field} is not an unsigned integer")
    if parsed < 0 or parsed > (1 << 63) - 1:
        raise LndGrpcError(f"LND {field} is outside uint63")
    return parsed


class LndGrpcAdapter:
    """Fixed-amount, non-AMP LND operations over a Lightning gRPC stub."""

    def __init__(
        self,
        lightning_stub: Any,
        router_stub: Any,
        request_factories: LndRequestFactories,
        *,
        network: str,
        rpc_timeout_seconds: float = 30.0,
    ) -> None:
        if network not in {"regtest", "signet", "bitcoin"}:
            raise ValueError("unsupported Lightning network")
        if rpc_timeout_seconds <= 0:
            raise ValueError("RPC timeout must be positive")
        self._stub = lightning_stub
        self._router_stub = router_stub
        self._messages = request_factories
        self.network = network
        self.rpc_timeout_seconds = rpc_timeout_seconds

    def add_invoice(
        self,
        secret: SecretPreimage,
        *,
        amount_msat: int,
        expiry_seconds: int,
        min_final_cltv_delta: int,
        memo: str = "",
    ) -> CreatedInvoice:
        if amount_msat <= 0 or amount_msat > (1 << 63) - 1:
            raise ValueError("invoice amount must be a positive uint63")
        if expiry_seconds <= 0 or min_final_cltv_delta <= 0:
            raise ValueError("invoice expiry and CLTV delta must be positive")
        request = self._messages.invoice(
            memo=memo,
            r_preimage=secret.reveal_for_protocol(),
            value_msat=amount_msat,
            expiry=expiry_seconds,
            cltv_expiry=min_final_cltv_delta,
            private=True,
            is_amp=False,
        )
        try:
            response = self._stub.AddInvoice(
                request, timeout=self.rpc_timeout_seconds
            )
        except Exception as error:
            raise LndGrpcError("LND AddInvoice gRPC failed") from error
        response_hash = _bytes32(_response_field(response, "r_hash"), "r_hash")
        expected_hash = payment_hash(secret)
        if not hmac.compare_digest(response_hash, expected_hash):
            raise LndGrpcError("LND AddInvoice returned the wrong payment hash")
        payment_request = _response_field(response, "payment_request")
        if type(payment_request) is not str or not payment_request:
            raise LndGrpcError("LND AddInvoice returned no payment request")
        facts = self.decode_invoice(payment_request)
        validate_invoice_binding(
            facts,
            expected_payment_hash=expected_hash,
            expected_amount_msat=amount_msat,
            expected_payee=facts.payee,
            expected_expiry_unix=facts.expiry_unix,
            expected_min_final_cltv_delta=min_final_cltv_delta,
            expected_network=self.network,
        )
        return CreatedInvoice(
            payment_request=payment_request,
            payment_hash=response_hash,
            add_index=_uint(_response_field(response, "add_index", 0), "add_index"),
            payment_addr=bytes(_response_field(response, "payment_addr", b"")),
            facts=facts,
        )

    def decode_invoice(
        self, payment_request: str, *, is_amp: bool | None = None
    ) -> LndInvoiceFacts:
        if type(payment_request) is not str or not payment_request:
            raise ValueError("payment request must be nonempty")
        request = self._messages.pay_req_string(pay_req=payment_request)
        try:
            response = self._stub.DecodePayReq(
                request, timeout=self.rpc_timeout_seconds
            )
        except Exception as error:
            raise LndGrpcError("LND DecodePayReq gRPC failed") from error
        decoded = {
            "payment_hash": _response_field(response, "payment_hash"),
            "num_msat": _response_field(response, "num_msat"),
            "destination": _response_field(response, "destination"),
            "timestamp": _response_field(response, "timestamp"),
            "expiry": _response_field(response, "expiry"),
            "cltv_expiry": _response_field(response, "cltv_expiry"),
            "features": _response_field(response, "features", {}),
            "is_amp": _response_field(response, "is_amp", False),
        }
        try:
            return LndInvoiceFacts.from_decode_pay_req(
                decoded, network=self.network, is_amp=is_amp
            )
        except ProtocolEncodingError as error:
            raise LndGrpcError(str(error)) from error

    def lookup_invoice(self, payment_hash_value: bytes | str) -> InvoiceStatus:
        digest = _bytes32(payment_hash_value, "payment_hash")
        request = self._messages.payment_hash(r_hash=digest)
        try:
            response = self._stub.LookupInvoice(
                request, timeout=self.rpc_timeout_seconds
            )
        except Exception as error:
            raise LndGrpcError("LND LookupInvoice gRPC failed") from error
        response_hash_raw = _response_field(response, "r_hash", digest)
        response_hash = _bytes32(response_hash_raw, "lookup r_hash")
        if not hmac.compare_digest(response_hash, digest):
            raise LndGrpcError("LND LookupInvoice returned a different hash")
        return InvoiceStatus(
            payment_hash=response_hash,
            settled=bool(_response_field(response, "settled", False)),
            state=_response_field(response, "state"),
            amount_paid_msat=_uint(
                _response_field(response, "amt_paid_msat", 0), "amt_paid_msat"
            ),
            add_index=_uint(_response_field(response, "add_index", 0), "add_index"),
            settle_index=_uint(
                _response_field(response, "settle_index", 0), "settle_index"
            ),
            is_amp=bool(_response_field(response, "is_amp", False)),
        )

    def send_payment(
        self,
        payment_request: str,
        *,
        fee_limit_msat: int,
        max_total_cltv_delta: int,
        timeout_seconds: int,
    ) -> SettledPayment:
        """Pay with direct Router ``SendPaymentV2`` after intent is durable."""

        facts = self.decode_invoice(payment_request)
        if facts.is_amp:
            raise LightningPaymentError("AMP invoices are unsupported")
        if fee_limit_msat < 0 or max_total_cltv_delta <= 0 or timeout_seconds <= 0:
            raise ValueError("payment limits are invalid")
        request = self._messages.send_payment_request(
            payment_request=payment_request,
            fee_limit_msat=fee_limit_msat,
            timeout_seconds=timeout_seconds,
            cltv_limit=max_total_cltv_delta,
            allow_self_payment=False,
            max_parts=1,
            no_inflight_updates=False,
            amp=False,
        )
        try:
            updates = self._router_stub.SendPaymentV2(
                request, timeout=float(timeout_seconds) + self.rpc_timeout_seconds
            )
            response = None
            for update_number, update in enumerate(updates, start=1):
                if update_number > MAX_PAYMENT_UPDATES:
                    raise LndGrpcError(
                        "LND payment update stream exceeded safety bound"
                    )
                status = _response_field(update, "status", 0)
                if status in (2, "SUCCEEDED"):
                    response = update
                    break
                if status in (3, "FAILED"):
                    failure = _response_field(
                        update, "failure_reason", "unknown failure"
                    )
                    raise LightningPaymentError(f"LND payment failed: {failure}")
        except Exception as error:
            if isinstance(error, LightningPaymentError):
                raise
            raise LndGrpcError(
                "LND SendPaymentV2 outcome is uncertain; reconcile by payment hash"
            ) from error
        if response is None:
            raise LndGrpcError(
                "LND SendPaymentV2 ended without a terminal payment update"
            )
        response_hash_value = _response_field(response, "payment_hash", None)
        if response_hash_value:
            response_hash = _bytes32(response_hash_value, "payment_hash")
            if not hmac.compare_digest(response_hash, facts.payment_hash):
                raise LndGrpcError("LND payment update returned a different hash")
        secret = SecretPreimage(
            _bytes32(
                _response_field(response, "payment_preimage"),
                "payment_preimage",
            )
        )
        if not hmac.compare_digest(payment_hash(secret), facts.payment_hash):
            raise LndGrpcError("settled payment preimage does not match invoice")
        raw_fee = _response_field(response, "fee_sat", None)
        fee_sat = None if raw_fee is None else _uint(raw_fee, "payment fee_sat")
        expiries: list[int] = []
        for attempt in _response_field(response, "htlcs", ()) or ():
            route = _response_field(attempt, "route", None)
            if route is None:
                continue
            expiry = _response_field(route, "total_time_lock", None)
            if expiry is not None:
                expiries.append(_uint(expiry, "HTLC route total_time_lock"))
        return SettledPayment(
            payment_hash=facts.payment_hash,
            payment_preimage=secret,
            fee_sat=fee_sat,
            payer_htlc_expiries=tuple(expiries),
        )
