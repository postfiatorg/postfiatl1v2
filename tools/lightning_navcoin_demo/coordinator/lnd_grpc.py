"""Direct LND gRPC adapter with injected generated-protobuf constructors.

Bootstrap may use ``lncli``, but runtime invoice/payment operations in this
adapter call LND's Lightning gRPC stub directly. Generated LND modules are
environment artifacts, so the coordinator accepts their message constructors
instead of importing an unpinned global module.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
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
MAX_CHANNELS = 65_536


class LndGrpcError(RuntimeError):
    """An LND request failed or returned inconsistent protocol data."""


class LndInvoiceNotFound(LndGrpcError):
    """LookupInvoice conclusively reported that the payment hash is absent."""


class LightningPaymentError(LndGrpcError):
    """LND conclusively rejected or failed a payment."""


@dataclass(frozen=True)
class LndRequestFactories:
    """Constructors from generated Lightning and Router protobuf modules."""

    invoice: Callable[..., Any]
    pay_req_string: Callable[..., Any]
    payment_hash: Callable[..., Any]
    send_payment_request: Callable[..., Any]
    get_info_request: Callable[..., Any] | None = None
    list_channels_request: Callable[..., Any] | None = None
    track_payment_request: Callable[..., Any] | None = None

    @classmethod
    def from_proto_modules(
        cls, lightning_pb2: Any, router_pb2: Any
    ) -> "LndRequestFactories":
        return cls(
            invoice=lightning_pb2.Invoice,
            pay_req_string=lightning_pb2.PayReqString,
            payment_hash=lightning_pb2.PaymentHash,
            send_payment_request=router_pb2.SendPaymentRequest,
            get_info_request=getattr(lightning_pb2, "GetInfoRequest", None),
            list_channels_request=getattr(
                lightning_pb2, "ListChannelsRequest", None
            ),
            track_payment_request=getattr(
                router_pb2, "TrackPaymentRequest", None
            ),
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
    fee_msat: int | None = None
    amount_msat: int | None = None

    def __repr__(self) -> str:
        return (
            "SettledPayment(payment_hash="
            f"{self.payment_hash.hex()}, payment_preimage=<redacted>, "
            f"fee_sat={self.fee_sat!r}, "
            f"fee_msat={self.fee_msat!r}, "
            f"payer_htlc_expiries={self.payer_htlc_expiries!r}, "
            f"amount_msat={self.amount_msat!r})"
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
    invoice_amount_msat: int | None = None
    payment_request: str | None = None
    payment_addr: bytes = b""

    @property
    def state_name(self) -> str:
        return _invoice_state_name(self.state)

    @property
    def terminal_unpaid(self) -> bool:
        return (
            self.state_name == "CANCELED"
            and not self.settled
            and self.amount_paid_msat == 0
            and self.settle_index == 0
        )


@dataclass(frozen=True)
class LndNodeInfo:
    identity_pubkey: str
    alias: str
    network: str
    block_height: int
    synced_to_chain: bool
    synced_to_graph: bool
    version: str
    commit_hash: str


@dataclass(frozen=True)
class LndLiquiditySummary:
    total_channels: int
    active_channels: int
    inactive_channels: int
    inbound_msat: int
    outbound_msat: int
    unconfirmed_active_channels: int = 0


@dataclass(frozen=True)
class LndNodePreflight:
    node: LndNodeInfo
    liquidity: LndLiquiditySummary


class PaymentReconciliationStatus(str, Enum):
    """Durable classification for a previously submitted payment."""

    SETTLED = "SETTLED"
    FAILED = "FAILED"
    UNCERTAIN = "UNCERTAIN"


@dataclass(frozen=True)
class PaymentReconciliation:
    payment_hash: bytes
    status: PaymentReconciliationStatus
    settled_payment: SettledPayment | None
    failure_reason: str | int | None
    last_lnd_status: str | int | None
    updates_seen: int

    @property
    def terminal(self) -> bool:
        return self.status in {
            PaymentReconciliationStatus.SETTLED,
            PaymentReconciliationStatus.FAILED,
        }


def _response_field(response: Any, name: str, default: Any = None) -> Any:
    if isinstance(response, Mapping):
        return response.get(name, default)
    return getattr(response, name, default)


def _has_response_field(response: Any, name: str) -> bool:
    if isinstance(response, Mapping):
        return name in response
    return hasattr(response, name)


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


def _bool(value: Any, field: str) -> bool:
    if type(value) is not bool:
        raise LndGrpcError(f"LND {field} is not a boolean")
    return value


def _canonical_pubkey(value: Any, field: str) -> str:
    if type(value) is not str:
        raise LndGrpcError(f"LND {field} is not a public-key string")
    try:
        decoded = bytes.fromhex(value)
    except ValueError as error:
        raise LndGrpcError(f"LND {field} is not hexadecimal") from error
    if (
        len(decoded) != 33
        or decoded[0] not in (2, 3)
        or value != decoded.hex()
    ):
        raise LndGrpcError(
            f"LND {field} is not canonical compressed secp256k1 hex"
        )
    return value


def _checked_add_uint63(left: int, right: int, field: str) -> int:
    result = left + right
    if result > (1 << 63) - 1:
        raise LndGrpcError(f"LND {field} sum is outside uint63")
    return result


def _lnd_network(network: str) -> str:
    return "mainnet" if network == "bitcoin" else network


def _invoice_state_is_settled(state: Any) -> bool:
    return _invoice_state_name(state) == "SETTLED"


def _invoice_state_name(state: Any) -> str:
    states = {
        0: "OPEN",
        1: "SETTLED",
        2: "CANCELED",
        3: "ACCEPTED",
        "OPEN": "OPEN",
        "SETTLED": "SETTLED",
        "CANCELED": "CANCELED",
        "ACCEPTED": "ACCEPTED",
    }
    try:
        return states[state]
    except (KeyError, TypeError) as error:
        raise LndGrpcError("LND invoice has an unknown state") from error


def _grpc_status_name(error: BaseException) -> str | None:
    """Extract one exact gRPC status name without depending on grpc at import."""

    code_method = getattr(error, "code", None)
    if not callable(code_method):
        return None
    try:
        code = code_method()
    except Exception:
        return None
    name = getattr(code, "name", None)
    if callable(name):
        try:
            name = name()
        except Exception:
            return None
    if type(name) is str:
        return name
    text = str(code)
    if "." in text:
        text = text.rsplit(".", 1)[-1]
    return text if text.isascii() else None


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
        expected_version: str | None = None,
        expected_commit_hash: str | None = None,
    ) -> None:
        if network not in {"regtest", "signet", "bitcoin"}:
            raise ValueError("unsupported Lightning network")
        if rpc_timeout_seconds <= 0:
            raise ValueError("RPC timeout must be positive")
        if (expected_version is None) != (expected_commit_hash is None):
            raise ValueError(
                "LND version and commit pins must be configured together"
            )
        for value, name in (
            (expected_version, "expected LND version"),
            (expected_commit_hash, "expected LND commit hash"),
        ):
            if value is not None and (
                type(value) is not str
                or not value
                or len(value) > 256
                or not value.isascii()
            ):
                raise ValueError(f"{name} must be bounded nonempty ASCII")
        self._stub = lightning_stub
        self._router_stub = router_stub
        self._messages = request_factories
        self.network = network
        self.rpc_timeout_seconds = rpc_timeout_seconds
        self.expected_version = expected_version
        self.expected_commit_hash = expected_commit_hash

    def _request_factory(self, name: str) -> Callable[..., Any]:
        factory = getattr(self._messages, name)
        if factory is None:
            raise LndGrpcError(
                f"LND protobuf factory {name} is unavailable in this runtime"
            )
        return factory

    def get_info(
        self,
        *,
        expected_identity_pubkey: str | None = None,
        require_synced: bool = True,
    ) -> LndNodeInfo:
        """Read and verify the connected node's identity and chain."""

        request = self._request_factory("get_info_request")()
        try:
            response = self._stub.GetInfo(
                request, timeout=self.rpc_timeout_seconds
            )
        except Exception as error:
            raise LndGrpcError("LND GetInfo gRPC failed") from error

        identity_pubkey = _canonical_pubkey(
            _response_field(response, "identity_pubkey"),
            "identity_pubkey",
        )
        if expected_identity_pubkey is not None:
            expected = _canonical_pubkey(
                expected_identity_pubkey, "expected identity_pubkey"
            )
            if not hmac.compare_digest(identity_pubkey, expected):
                raise LndGrpcError("connected LND identity pubkey is unexpected")

        raw_chains = _response_field(response, "chains", ())
        if raw_chains is None:
            raw_chains = ()
        try:
            chains = tuple(raw_chains)
        except TypeError as error:
            raise LndGrpcError("LND chains is not iterable") from error
        expected_network = _lnd_network(self.network)
        chain_networks: list[tuple[Any, Any]] = []
        for chain in chains:
            chain_name = _response_field(chain, "chain")
            chain_network = _response_field(chain, "network")
            chain_networks.append((chain_name, chain_network))
        if chain_networks != [("bitcoin", expected_network)]:
            raise LndGrpcError(
                "connected LND chain/network does not match configured network"
            )

        synced_to_chain = _bool(
            _response_field(response, "synced_to_chain"),
            "synced_to_chain",
        )
        synced_to_graph = _bool(
            _response_field(response, "synced_to_graph"),
            "synced_to_graph",
        )
        if require_synced and not (synced_to_chain and synced_to_graph):
            raise LndGrpcError("connected LND is not fully chain/graph synced")

        alias = _response_field(response, "alias", "")
        version = _response_field(response, "version", "")
        commit_hash = _response_field(response, "commit_hash", "")
        if (
            type(alias) is not str
            or type(version) is not str
            or type(commit_hash) is not str
        ):
            raise LndGrpcError("LND alias/version/commit hash has an invalid type")
        if (
            self.expected_version is not None
            and (
                not hmac.compare_digest(version, self.expected_version)
                or not hmac.compare_digest(
                    commit_hash, self.expected_commit_hash or ""
                )
            )
        ):
            raise LndGrpcError(
                "connected LND version or commit hash is not the reviewed release"
            )
        return LndNodeInfo(
            identity_pubkey=identity_pubkey,
            alias=alias,
            network=self.network,
            block_height=_uint(
                _response_field(response, "block_height"), "block_height"
            ),
            synced_to_chain=synced_to_chain,
            synced_to_graph=synced_to_graph,
            version=version,
            commit_hash=commit_hash,
        )

    def list_active_liquidity(self) -> LndLiquiditySummary:
        """Return conservative active-channel balances in millisatoshis."""

        request = self._request_factory("list_channels_request")(
            active_only=False,
            inactive_only=False,
            public_only=False,
            private_only=False,
        )
        try:
            response = self._stub.ListChannels(
                request, timeout=self.rpc_timeout_seconds
            )
        except Exception as error:
            raise LndGrpcError("LND ListChannels gRPC failed") from error
        raw_channels = _response_field(response, "channels", ())
        if raw_channels is None:
            raw_channels = ()
        try:
            channels = tuple(raw_channels)
        except TypeError as error:
            raise LndGrpcError("LND channels is not iterable") from error
        if len(channels) > MAX_CHANNELS:
            raise LndGrpcError("LND channel list exceeded safety bound")

        active_channels = 0
        inactive_channels = 0
        unconfirmed_active_channels = 0
        inbound_sat = 0
        outbound_sat = 0
        for channel in channels:
            active = _bool(_response_field(channel, "active"), "channel active")
            local_sat = _uint(
                _response_field(channel, "local_balance"),
                "channel local_balance",
            )
            remote_sat = _uint(
                _response_field(channel, "remote_balance"),
                "channel remote_balance",
            )
            local_reserve_sat = _uint(
                _response_field(channel, "local_chan_reserve_sat", 0),
                "channel local_chan_reserve_sat",
            )
            remote_reserve_sat = _uint(
                _response_field(channel, "remote_chan_reserve_sat", 0),
                "channel remote_chan_reserve_sat",
            )
            if not active:
                inactive_channels += 1
                continue
            zero_conf = _bool(
                _response_field(channel, "zero_conf", False),
                "channel zero_conf",
            )
            zero_conf_confirmed_scid = _uint(
                _response_field(channel, "zero_conf_confirmed_scid", 0),
                "channel zero_conf_confirmed_scid",
            )
            if zero_conf and zero_conf_confirmed_scid == 0:
                unconfirmed_active_channels += 1
                continue
            active_channels += 1
            local_sat = max(0, local_sat - local_reserve_sat)
            remote_sat = max(0, remote_sat - remote_reserve_sat)
            outbound_sat = _checked_add_uint63(
                outbound_sat, local_sat, "active outbound liquidity"
            )
            inbound_sat = _checked_add_uint63(
                inbound_sat, remote_sat, "active inbound liquidity"
            )
        if inbound_sat > ((1 << 63) - 1) // 1000:
            raise LndGrpcError("LND active inbound liquidity exceeds uint63 msat")
        if outbound_sat > ((1 << 63) - 1) // 1000:
            raise LndGrpcError("LND active outbound liquidity exceeds uint63 msat")
        return LndLiquiditySummary(
            total_channels=len(channels),
            active_channels=active_channels,
            inactive_channels=inactive_channels,
            inbound_msat=inbound_sat * 1000,
            outbound_msat=outbound_sat * 1000,
            unconfirmed_active_channels=unconfirmed_active_channels,
        )

    def preflight_node(
        self,
        *,
        expected_identity_pubkey: str,
        min_active_channels: int = 1,
        min_inbound_msat: int = 1,
        min_outbound_msat: int = 1,
    ) -> LndNodePreflight:
        """Fail closed unless node identity, sync, and active liquidity match."""

        for value, name in (
            (min_active_channels, "minimum active channels"),
            (min_inbound_msat, "minimum inbound liquidity"),
            (min_outbound_msat, "minimum outbound liquidity"),
        ):
            if type(value) is not int or value < 0 or value > (1 << 63) - 1:
                raise ValueError(f"{name} must be a uint63")
        node = self.get_info(
            expected_identity_pubkey=expected_identity_pubkey,
            require_synced=True,
        )
        liquidity = self.list_active_liquidity()
        if liquidity.active_channels < min_active_channels:
            raise LndGrpcError("LND has insufficient active channels")
        if liquidity.inbound_msat < min_inbound_msat:
            raise LndGrpcError("LND has insufficient active inbound liquidity")
        if liquidity.outbound_msat < min_outbound_msat:
            raise LndGrpcError("LND has insufficient active outbound liquidity")
        return LndNodePreflight(node=node, liquidity=liquidity)

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

    def lookup_invoice(
        self,
        payment_hash_value: bytes | str,
        *,
        require_settled: bool = False,
        expected_amount_msat: int | None = None,
        expected_add_index: int | None = None,
        expected_settle_index: int | None = None,
    ) -> InvoiceStatus:
        digest = _bytes32(payment_hash_value, "payment_hash")
        for value, name in (
            (expected_amount_msat, "expected amount_msat"),
            (expected_add_index, "expected add_index"),
            (expected_settle_index, "expected settle_index"),
        ):
            if value is not None and (
                type(value) is not int or value < 0 or value > (1 << 63) - 1
            ):
                raise ValueError(f"{name} must be a uint63")
        request = self._messages.payment_hash(r_hash=digest)
        try:
            response = self._stub.LookupInvoice(
                request, timeout=self.rpc_timeout_seconds
            )
        except Exception as error:
            if _grpc_status_name(error) == "NOT_FOUND":
                raise LndInvoiceNotFound(
                    "LND LookupInvoice payment hash was not found"
                ) from error
            raise LndGrpcError("LND LookupInvoice gRPC failed") from error
        response_hash_raw = _response_field(response, "r_hash", digest)
        response_hash = _bytes32(response_hash_raw, "lookup r_hash")
        if not hmac.compare_digest(response_hash, digest):
            raise LndGrpcError("LND LookupInvoice returned a different hash")
        settled = _bool(
            _response_field(response, "settled", False), "invoice settled"
        )
        state = _response_field(response, "state")
        state_name = _invoice_state_name(state)
        amount_paid_msat = _uint(
            _response_field(response, "amt_paid_msat", 0), "amt_paid_msat"
        )
        invoice_amount_msat = (
            _uint(_response_field(response, "value_msat"), "value_msat")
            if _has_response_field(response, "value_msat")
            else None
        )
        add_index = _uint(
            _response_field(response, "add_index", 0), "add_index"
        )
        settle_index = _uint(
            _response_field(response, "settle_index", 0), "settle_index"
        )
        is_amp = _bool(_response_field(response, "is_amp", False), "invoice is_amp")
        if is_amp:
            raise LndGrpcError("AMP invoices are unsupported")
        if settled != (state_name == "SETTLED"):
            raise LndGrpcError("LND invoice settled flag/state is inconsistent")
        if settled and settle_index == 0:
            raise LndGrpcError("settled LND invoice has no settle_index")
        if not settled and settle_index != 0:
            raise LndGrpcError("unsettled LND invoice has a settle_index")
        if require_settled and not settled:
            raise LndGrpcError("LND invoice is not settled")
        if (
            expected_amount_msat is not None
            and invoice_amount_msat != expected_amount_msat
        ):
            raise LndGrpcError("LND invoice face amount does not match expectation")
        if (
            settled
            and expected_amount_msat is not None
            and amount_paid_msat != expected_amount_msat
        ):
            raise LndGrpcError("LND invoice paid amount does not match expectation")
        if expected_add_index is not None and add_index != expected_add_index:
            raise LndGrpcError("LND invoice add_index does not match expectation")
        if (
            expected_settle_index is not None
            and settle_index != expected_settle_index
        ):
            raise LndGrpcError("LND invoice settle_index does not match expectation")
        return InvoiceStatus(
            payment_hash=response_hash,
            settled=settled,
            state=state,
            amount_paid_msat=amount_paid_msat,
            add_index=add_index,
            settle_index=settle_index,
            is_amp=is_amp,
            invoice_amount_msat=invoice_amount_msat,
            payment_request=(
                _response_field(response, "payment_request")
                if type(_response_field(response, "payment_request")) is str
                and _response_field(response, "payment_request")
                else None
            ),
            payment_addr=bytes(_response_field(response, "payment_addr", b"")),
        )

    def recover_created_invoice(
        self,
        payment_hash_value: bytes | str,
        *,
        expected_amount_msat: int,
        expected_payee: str,
        expected_min_final_cltv_delta: int,
    ) -> CreatedInvoice:
        """Recover a prior AddInvoice result by its durably persisted hash."""

        status = self.lookup_invoice(
            payment_hash_value,
            expected_amount_msat=expected_amount_msat,
        )
        if status.payment_request is None:
            raise LndGrpcError(
                "LND LookupInvoice returned no payment request for recovery"
            )
        facts = self.decode_invoice(status.payment_request)
        validate_invoice_binding(
            facts,
            expected_payment_hash=status.payment_hash,
            expected_amount_msat=expected_amount_msat,
            expected_payee=expected_payee,
            expected_expiry_unix=facts.expiry_unix,
            expected_min_final_cltv_delta=expected_min_final_cltv_delta,
            expected_network=self.network,
        )
        if facts.is_amp or status.is_amp:
            raise LndGrpcError("AMP invoices are unsupported")
        return CreatedInvoice(
            payment_request=status.payment_request,
            payment_hash=status.payment_hash,
            add_index=status.add_index,
            payment_addr=status.payment_addr,
            facts=facts,
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
        return self._parse_settled_payment(
            response,
            expected_payment_hash=facts.payment_hash,
            expected_amount_msat=facts.amount_msat,
            require_amount_field=False,
        )

    def track_payment(
        self,
        payment_hash_value: bytes | str,
        *,
        expected_amount_msat: int | None = None,
    ) -> PaymentReconciliation:
        """Reconcile a prior ``SendPaymentV2`` without risking a duplicate pay.

        Transport failure or a stream that ends without a terminal update is
        explicitly ``UNCERTAIN``. Terminal success is accepted only after the
        returned preimage hashes to the requested payment hash.
        """

        digest = _bytes32(payment_hash_value, "payment_hash")
        if expected_amount_msat is not None and (
            type(expected_amount_msat) is not int
            or expected_amount_msat <= 0
            or expected_amount_msat > (1 << 63) - 1
        ):
            raise ValueError("expected amount_msat must be a positive uint63")
        request = self._request_factory("track_payment_request")(
            payment_hash=digest,
            no_inflight_updates=False,
        )
        try:
            updates = iter(
                self._router_stub.TrackPaymentV2(
                    request, timeout=self.rpc_timeout_seconds
                )
            )
        except Exception:
            return PaymentReconciliation(
                payment_hash=digest,
                status=PaymentReconciliationStatus.UNCERTAIN,
                settled_payment=None,
                failure_reason="TrackPaymentV2 transport failed",
                last_lnd_status=None,
                updates_seen=0,
            )

        last_status: str | int | None = None
        updates_seen = 0
        while updates_seen < MAX_PAYMENT_UPDATES:
            try:
                update = next(updates)
            except StopIteration:
                return PaymentReconciliation(
                    payment_hash=digest,
                    status=PaymentReconciliationStatus.UNCERTAIN,
                    settled_payment=None,
                    failure_reason=(
                        "TrackPaymentV2 ended without a terminal update"
                    ),
                    last_lnd_status=last_status,
                    updates_seen=updates_seen,
                )
            except Exception:
                return PaymentReconciliation(
                    payment_hash=digest,
                    status=PaymentReconciliationStatus.UNCERTAIN,
                    settled_payment=None,
                    failure_reason="TrackPaymentV2 stream transport failed",
                    last_lnd_status=last_status,
                    updates_seen=updates_seen,
                )
            updates_seen += 1
            status = _response_field(update, "status", 0)
            if type(status) not in (int, str):
                raise LndGrpcError("LND payment status has an invalid type")
            last_status = status
            response_hash_value = _response_field(
                update, "payment_hash", None
            )
            if response_hash_value not in (None, "", b""):
                response_hash = _bytes32(
                    response_hash_value, "payment_hash"
                )
                if not hmac.compare_digest(response_hash, digest):
                    raise LndGrpcError(
                        "LND tracked payment returned a different hash"
                    )
            if status in (2, "SUCCEEDED"):
                settled = self._parse_settled_payment(
                    update,
                    expected_payment_hash=digest,
                    expected_amount_msat=expected_amount_msat,
                    require_response_hash=True,
                    require_amount_field=expected_amount_msat is not None,
                )
                return PaymentReconciliation(
                    payment_hash=digest,
                    status=PaymentReconciliationStatus.SETTLED,
                    settled_payment=settled,
                    failure_reason=None,
                    last_lnd_status=status,
                    updates_seen=updates_seen,
                )
            if status in (3, "FAILED"):
                failure = _response_field(
                    update, "failure_reason", "unknown failure"
                )
                if type(failure) not in (int, str):
                    raise LndGrpcError(
                        "LND payment failure_reason has an invalid type"
                    )
                return PaymentReconciliation(
                    payment_hash=digest,
                    status=PaymentReconciliationStatus.FAILED,
                    settled_payment=None,
                    failure_reason=failure,
                    last_lnd_status=status,
                    updates_seen=updates_seen,
                )
            if status not in (
                0,
                1,
                4,
                "UNKNOWN",
                "IN_FLIGHT",
                "INITIATED",
            ):
                raise LndGrpcError("LND payment status is unknown")

        return PaymentReconciliation(
            payment_hash=digest,
            status=PaymentReconciliationStatus.UNCERTAIN,
            settled_payment=None,
            failure_reason="TrackPaymentV2 update stream exceeded safety bound",
            last_lnd_status=last_status,
            updates_seen=updates_seen,
        )

    @staticmethod
    def _parse_settled_payment(
        response: Any,
        *,
        expected_payment_hash: bytes,
        expected_amount_msat: int | None,
        require_response_hash: bool = False,
        require_amount_field: bool = False,
    ) -> SettledPayment:
        response_hash_value = _response_field(response, "payment_hash", None)
        if response_hash_value in (None, "", b""):
            if require_response_hash:
                raise LndGrpcError(
                    "settled LND payment omitted its payment_hash"
                )
        else:
            response_hash = _bytes32(response_hash_value, "payment_hash")
            if not hmac.compare_digest(
                response_hash, expected_payment_hash
            ):
                raise LndGrpcError(
                    "LND payment update returned a different hash"
                )
        secret = SecretPreimage(
            _bytes32(
                _response_field(response, "payment_preimage"),
                "payment_preimage",
            )
        )
        if not hmac.compare_digest(
            payment_hash(secret), expected_payment_hash
        ):
            raise LndGrpcError(
                "settled payment preimage does not match payment hash"
            )

        amount_msat: int | None = None
        if _has_response_field(response, "value_msat"):
            amount_msat = _uint(
                _response_field(response, "value_msat"),
                "payment value_msat",
            )
        if expected_amount_msat is not None:
            if amount_msat is None and require_amount_field:
                raise LndGrpcError(
                    "settled LND payment omitted its value_msat"
                )
            if amount_msat is not None and amount_msat != expected_amount_msat:
                raise LndGrpcError(
                    "settled LND payment amount does not match expectation"
                )
        raw_fee = _response_field(response, "fee_sat", None)
        fee_sat = None if raw_fee is None else _uint(raw_fee, "payment fee_sat")
        raw_fee_msat = _response_field(response, "fee_msat", None)
        fee_msat = (
            None
            if raw_fee_msat is None
            else _uint(raw_fee_msat, "payment fee_msat")
        )
        if (
            fee_msat is not None
            and fee_sat is not None
            and fee_sat != fee_msat // 1000
        ):
            raise LndGrpcError(
                "settled payment fee_sat/fee_msat fields are inconsistent"
            )
        expiries: list[int] = []
        for attempt in _response_field(response, "htlcs", ()) or ():
            route = _response_field(attempt, "route", None)
            if route is None:
                continue
            expiry = _response_field(route, "total_time_lock", None)
            if expiry is not None:
                expiries.append(_uint(expiry, "HTLC route total_time_lock"))
        return SettledPayment(
            payment_hash=expected_payment_hash,
            payment_preimage=secret,
            fee_sat=fee_sat,
            payer_htlc_expiries=tuple(expiries),
            fee_msat=fee_msat,
            amount_msat=amount_msat,
        )
