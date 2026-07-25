"""Pinned route and quote gates for a dust-capped mainnet demonstration."""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum
import hashlib
import json
import re
import time
from typing import Any, Mapping, Sequence
from urllib.parse import urlsplit

from ..coordinator.signing import encode_signed_quote, verify_signed_quote


POLICY_SCHEMA = "postfiat.lightning_real_value_policy.v2"
PRICE_SCHEMA = "postfiat.lightning_btc_price_observation.v1"
TRUST_CLASS = "CONTROLLED"
ATOMICITY_CLAIM = (
    "non-custodial, conditionally-atomic, COORDINATOR-TRUSTED timing"
)
MAX_DEMO_PER_RUN_USD_E8 = 5 * 100_000_000
MAX_DEMO_LIFETIME_USD_E8 = 20 * 100_000_000
MAX_PRICE_AGE_SECONDS = 300
MAX_QUOTE_LIFETIME_SECONDS = 300
MSAT_PER_BTC = 100_000_000_000
MAX_ASSET_PRECISION = 18
NAV_VALUATION_UNIT = "USD_PER_WHOLE_ASSET_UNIT"
NAV_VALUATION_SCALE = 100_000_000
HEX_32 = re.compile(r"^[0-9a-f]{64}$")
HEX_33 = re.compile(r"^(02|03)[0-9a-f]{64}$")
HEX_48 = re.compile(r"^[0-9a-f]{96}$")
GIT_REVISION = re.compile(r"^[0-9a-f]{7,64}$")
PFTL_ADDRESS = re.compile(r"^pf[0-9a-f]{40}$")


class RealValuePolicyError(ValueError):
    """A real-value route, quote, or price observation failed closed."""


class ExecutionMode(str, Enum):
    DRY_RUN = "DRY_RUN"
    ARMED = "ARMED"


def _strict_fields(value: Mapping[str, Any], expected: frozenset[str], name: str) -> None:
    fields = frozenset(value.keys())
    if fields != expected:
        raise RealValuePolicyError(
            f"{name} field set mismatch; "
            f"missing={sorted(expected - fields)}, unknown={sorted(fields - expected)}"
        )


def _text(value: Any, name: str, *, maximum: int = 4096) -> str:
    if (
        type(value) is not str
        or not value
        or len(value) > maximum
        or not value.isascii()
        or any(ord(character) < 0x20 or ord(character) == 0x7F for character in value)
    ):
        raise RealValuePolicyError(f"{name} must be bounded printable ASCII")
    return value


def _uint(value: Any, name: str, *, minimum: int = 0) -> int:
    if type(value) is not int or value < minimum or value > (1 << 63) - 1:
        raise RealValuePolicyError(f"{name} must be uint63 >= {minimum}")
    return value


def _hex(value: Any, name: str, pattern: re.Pattern[str]) -> str:
    text = _text(value, name)
    if pattern.fullmatch(text) is None:
        raise RealValuePolicyError(f"{name} is not canonical lowercase hex")
    return text


def _endpoint(value: Any, name: str) -> str:
    endpoint = _text(value, name, maximum=512)
    try:
        parsed = urlsplit(endpoint)
        port = parsed.port
    except ValueError as error:
        raise RealValuePolicyError(f"{name} is not a valid endpoint") from error
    if (
        parsed.scheme != "tcp"
        or not parsed.hostname
        or port is None
        or port <= 0
        or parsed.username is not None
        or parsed.password is not None
        or parsed.path
        or parsed.query
        or parsed.fragment
    ):
        raise RealValuePolicyError(f"{name} must be an explicit tcp://host:port endpoint")
    return endpoint


@dataclass(frozen=True)
class PriceObservation:
    """Operator-reviewed BTC/USD input bound into the executable quote gate."""

    btc_usd_e8: int
    observed_at_unix: int
    source: str

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "PriceObservation":
        if not isinstance(value, Mapping):
            raise RealValuePolicyError("price observation must be an object")
        _strict_fields(
            value,
            frozenset({"schema", "btc_usd_e8", "observed_at_unix", "source"}),
            "price observation",
        )
        if value["schema"] != PRICE_SCHEMA:
            raise RealValuePolicyError("unsupported price observation schema")
        return cls(
            btc_usd_e8=_uint(value["btc_usd_e8"], "btc_usd_e8", minimum=1),
            observed_at_unix=_uint(
                value["observed_at_unix"], "observed_at_unix", minimum=1
            ),
            source=_text(value["source"], "price source", maximum=256),
        )

    def to_dict(self) -> dict[str, Any]:
        return {
            "schema": PRICE_SCHEMA,
            "btc_usd_e8": self.btc_usd_e8,
            "observed_at_unix": self.observed_at_unix,
            "source": self.source,
        }


@dataclass(frozen=True)
class RealValuePolicy:
    """Immutable identity and budget pins for one operator-reviewed route."""

    policy_id: str
    mode: ExecutionMode
    lightning_network: str
    expected_lnd_pubkey: str
    quote_signer_public_key_hex: str
    authorization_public_key_hex: str
    pftl_chain_id: str
    pftl_genesis_hash: str
    pftl_build_git_revision: str
    pftl_asset_id: str
    pftl_asset_precision: int
    pftl_rpc_endpoints: tuple[str, ...]
    pftl_nav_epoch: int
    pftl_nav_reserve_packet_hash: str
    pftl_nav_valuation_unit: str
    pftl_nav_valuation_scale: int
    pftl_nav_per_unit_usd_e8: int
    coordinator_pftl_address: str
    pftl_user_address: str
    trust_class: str
    atomicity_claim: str
    require_non_freezable: bool
    max_per_run_usd_e8: int
    max_lifetime_usd_e8: int
    max_fee_msat: int
    max_price_age_seconds: int
    max_quote_lifetime_seconds: int
    minimum_pftl_validators: int

    FIELDS = frozenset(
        {
            "schema",
            "policy_id",
            "mode",
            "lightning_network",
            "expected_lnd_pubkey",
            "quote_signer_public_key_hex",
            "authorization_public_key_hex",
            "pftl_chain_id",
            "pftl_genesis_hash",
            "pftl_build_git_revision",
            "pftl_asset_id",
            "pftl_asset_precision",
            "pftl_rpc_endpoints",
            "pftl_nav_epoch",
            "pftl_nav_reserve_packet_hash",
            "pftl_nav_valuation_unit",
            "pftl_nav_valuation_scale",
            "pftl_nav_per_unit_usd_e8",
            "coordinator_pftl_address",
            "pftl_user_address",
            "trust_class",
            "atomicity_claim",
            "require_non_freezable",
            "max_per_run_usd_e8",
            "max_lifetime_usd_e8",
            "max_fee_msat",
            "max_price_age_seconds",
            "max_quote_lifetime_seconds",
            "minimum_pftl_validators",
        }
    )

    @classmethod
    def from_mapping(cls, value: Mapping[str, Any]) -> "RealValuePolicy":
        if not isinstance(value, Mapping):
            raise RealValuePolicyError("real-value policy must be an object")
        _strict_fields(value, cls.FIELDS, "real-value policy")
        if value["schema"] != POLICY_SCHEMA:
            raise RealValuePolicyError("unsupported real-value policy schema")
        try:
            mode = ExecutionMode(value["mode"])
        except (TypeError, ValueError) as error:
            raise RealValuePolicyError("mode must be DRY_RUN or ARMED") from error
        endpoints_value = value["pftl_rpc_endpoints"]
        if (
            not isinstance(endpoints_value, Sequence)
            or isinstance(endpoints_value, (str, bytes))
            or len(endpoints_value) != 6
        ):
            raise RealValuePolicyError("exactly six PFTL RPC endpoints are required")
        endpoints = tuple(
            _endpoint(endpoint, f"pftl_rpc_endpoints[{index}]")
            for index, endpoint in enumerate(endpoints_value)
        )
        if len(set(endpoints)) != 6:
            raise RealValuePolicyError("PFTL RPC endpoints must be distinct")
        if value["lightning_network"] != "bitcoin":
            raise RealValuePolicyError("real-value policy requires Bitcoin mainnet")
        if value["trust_class"] != TRUST_CLASS:
            raise RealValuePolicyError("real NAVcoin route must remain CONTROLLED")
        if value["atomicity_claim"] != ATOMICITY_CLAIM:
            raise RealValuePolicyError("atomicity claim must preserve the timing qualifier")
        if type(value["require_non_freezable"]) is not bool:
            raise RealValuePolicyError("require_non_freezable must be boolean")
        if value["require_non_freezable"] is not True:
            raise RealValuePolicyError(
                "real BTC may not cross an issued-asset finish-freeze race"
            )
        per_run = _uint(
            value["max_per_run_usd_e8"], "max_per_run_usd_e8", minimum=1
        )
        lifetime = _uint(
            value["max_lifetime_usd_e8"], "max_lifetime_usd_e8", minimum=1
        )
        if per_run > MAX_DEMO_PER_RUN_USD_E8:
            raise RealValuePolicyError("per-run cap exceeds founder dust authorization")
        if lifetime > MAX_DEMO_LIFETIME_USD_E8:
            raise RealValuePolicyError("lifetime cap exceeds founder dust authorization")
        if per_run > lifetime:
            raise RealValuePolicyError("per-run cap exceeds lifetime cap")
        minimum_validators = _uint(
            value["minimum_pftl_validators"],
            "minimum_pftl_validators",
            minimum=1,
        )
        if minimum_validators != 6:
            raise RealValuePolicyError("real-value preflight requires six-of-six PFTL views")
        if value["pftl_nav_valuation_unit"] != NAV_VALUATION_UNIT:
            raise RealValuePolicyError(
                "PFTL NAV valuation unit must be USD per whole asset unit"
            )
        if value["pftl_nav_valuation_scale"] != NAV_VALUATION_SCALE:
            raise RealValuePolicyError("PFTL NAV valuation scale must be USD-e8")
        nav_per_unit_usd_e8 = _uint(
            value["pftl_nav_per_unit_usd_e8"],
            "pftl_nav_per_unit_usd_e8",
            minimum=1,
        )
        asset_precision = _uint(
            value["pftl_asset_precision"], "pftl_asset_precision"
        )
        if asset_precision > MAX_ASSET_PRECISION:
            raise RealValuePolicyError(
                f"pftl_asset_precision exceeds {MAX_ASSET_PRECISION}"
            )
        max_price_age_seconds = _uint(
            value["max_price_age_seconds"],
            "max_price_age_seconds",
            minimum=1,
        )
        if max_price_age_seconds > MAX_PRICE_AGE_SECONDS:
            raise RealValuePolicyError(
                "max_price_age_seconds exceeds the hard mainnet-demo bound"
            )
        max_quote_lifetime_seconds = _uint(
            value["max_quote_lifetime_seconds"],
            "max_quote_lifetime_seconds",
            minimum=1,
        )
        if max_quote_lifetime_seconds > MAX_QUOTE_LIFETIME_SECONDS:
            raise RealValuePolicyError(
                "max_quote_lifetime_seconds exceeds the hard mainnet-demo bound"
            )
        quote_signer_public_key_hex = _hex(
            value["quote_signer_public_key_hex"],
            "quote_signer_public_key_hex",
            HEX_32,
        )
        authorization_public_key_hex = _hex(
            value["authorization_public_key_hex"],
            "authorization_public_key_hex",
            HEX_32,
        )
        if authorization_public_key_hex == quote_signer_public_key_hex:
            raise RealValuePolicyError(
                "authorization and quote signer keys must be distinct"
            )
        return cls(
            policy_id=_hex(value["policy_id"], "policy_id", HEX_32),
            mode=mode,
            lightning_network="bitcoin",
            expected_lnd_pubkey=_hex(
                value["expected_lnd_pubkey"], "expected_lnd_pubkey", HEX_33
            ),
            quote_signer_public_key_hex=quote_signer_public_key_hex,
            authorization_public_key_hex=authorization_public_key_hex,
            pftl_chain_id=_text(value["pftl_chain_id"], "pftl_chain_id", maximum=128),
            pftl_genesis_hash=_hex(
                value["pftl_genesis_hash"], "pftl_genesis_hash", HEX_48
            ),
            pftl_build_git_revision=_hex(
                value["pftl_build_git_revision"],
                "pftl_build_git_revision",
                GIT_REVISION,
            ),
            pftl_asset_id=_hex(value["pftl_asset_id"], "pftl_asset_id", HEX_48),
            pftl_asset_precision=asset_precision,
            pftl_rpc_endpoints=endpoints,
            pftl_nav_epoch=_uint(value["pftl_nav_epoch"], "pftl_nav_epoch", minimum=1),
            pftl_nav_reserve_packet_hash=_hex(
                value["pftl_nav_reserve_packet_hash"],
                "pftl_nav_reserve_packet_hash",
                HEX_48,
            ),
            pftl_nav_valuation_unit=NAV_VALUATION_UNIT,
            pftl_nav_valuation_scale=NAV_VALUATION_SCALE,
            pftl_nav_per_unit_usd_e8=nav_per_unit_usd_e8,
            coordinator_pftl_address=_hex(
                value["coordinator_pftl_address"],
                "coordinator_pftl_address",
                PFTL_ADDRESS,
            ),
            pftl_user_address=_hex(
                value["pftl_user_address"],
                "pftl_user_address",
                PFTL_ADDRESS,
            ),
            trust_class=TRUST_CLASS,
            atomicity_claim=ATOMICITY_CLAIM,
            require_non_freezable=True,
            max_per_run_usd_e8=per_run,
            max_lifetime_usd_e8=lifetime,
            max_fee_msat=_uint(value["max_fee_msat"], "max_fee_msat"),
            max_price_age_seconds=max_price_age_seconds,
            max_quote_lifetime_seconds=max_quote_lifetime_seconds,
            minimum_pftl_validators=minimum_validators,
        )

    @classmethod
    def from_json_bytes(cls, encoded: bytes) -> "RealValuePolicy":
        if type(encoded) is not bytes or not encoded or len(encoded) > 64 * 1024:
            raise RealValuePolicyError("policy JSON is empty or oversized")
        try:
            value = json.loads(encoded.decode("ascii"))
        except (UnicodeDecodeError, json.JSONDecodeError) as error:
            raise RealValuePolicyError("policy is not valid ASCII JSON") from error
        return cls.from_mapping(value)

    def public_status(self) -> dict[str, Any]:
        """Return pins and budgets only; never credentials or authorization data."""

        return {
            "schema": POLICY_SCHEMA,
            "policy_id": self.policy_id,
            "mode": self.mode.value,
            "lightning_network": self.lightning_network,
            "expected_lnd_pubkey": self.expected_lnd_pubkey,
            "pftl_chain_id": self.pftl_chain_id,
            "pftl_genesis_hash": self.pftl_genesis_hash,
            "pftl_build_git_revision": self.pftl_build_git_revision,
            "pftl_asset_id": self.pftl_asset_id,
            "pftl_asset_precision": self.pftl_asset_precision,
            "pftl_user_address": self.pftl_user_address,
            "pftl_nav_epoch": self.pftl_nav_epoch,
            "pftl_nav_reserve_packet_hash": self.pftl_nav_reserve_packet_hash,
            "pftl_nav_valuation_unit": self.pftl_nav_valuation_unit,
            "pftl_nav_valuation_scale": self.pftl_nav_valuation_scale,
            "pftl_nav_per_unit_usd_e8": self.pftl_nav_per_unit_usd_e8,
            "trust_class": self.trust_class,
            "atomicity_claim": self.atomicity_claim,
            "require_non_freezable": self.require_non_freezable,
            "max_per_run_usd_e8": self.max_per_run_usd_e8,
            "max_lifetime_usd_e8": self.max_lifetime_usd_e8,
            "max_fee_msat": self.max_fee_msat,
            "minimum_pftl_validators": self.minimum_pftl_validators,
        }


@dataclass(frozen=True)
class MainnetQuoteView:
    swap_id: str
    direction: str
    quote_sha256: str
    invoice_amount_msat: int
    maximum_all_in_msat: int
    maximum_all_in_usd_e8: int
    pftl_amount_atoms: int
    quote_expires_unix: int
    nav_epoch: int
    nav_reserve_packet_hash: str


def _ceil_div(numerator: int, denominator: int) -> int:
    if numerator < 0 or denominator <= 0:
        raise RealValuePolicyError("invalid checked division")
    return (numerator + denominator - 1) // denominator


def msat_to_usd_e8_ceil(amount_msat: int, btc_usd_e8: int) -> int:
    amount = _uint(amount_msat, "amount_msat")
    price = _uint(btc_usd_e8, "btc_usd_e8", minimum=1)
    return _ceil_div(amount * price, MSAT_PER_BTC)


def validate_mainnet_quote(
    signed_quote: Mapping[str, Any],
    policy: RealValuePolicy,
    price: PriceObservation,
    *,
    now_unix: int | None = None,
) -> MainnetQuoteView:
    """Bind a coordinator quote to mainnet identity, proven NAV, and dust caps."""

    now = int(time.time()) if now_unix is None else _uint(now_unix, "now_unix")
    if price.observed_at_unix > now:
        raise RealValuePolicyError("BTC price observation is from the future")
    if now - price.observed_at_unix > policy.max_price_age_seconds:
        raise RealValuePolicyError("BTC price observation is stale")
    try:
        quote = verify_signed_quote(
            signed_quote,
            expected_public_key=bytes.fromhex(policy.quote_signer_public_key_hex),
        )
        encoded = encode_signed_quote(signed_quote)
    except Exception as error:
        raise RealValuePolicyError("signed quote validation failed") from error
    if quote["lightning_network"] != "bitcoin":
        raise RealValuePolicyError("quote is not bound to Bitcoin mainnet")
    invoice = quote["invoice"]
    if invoice != invoice.lower() or not invoice.startswith("lnbc"):
        raise RealValuePolicyError("quote invoice must be canonical lowercase lnbc")
    if (
        quote["direction"] == "lightning_to_pftl"
        and quote["invoice_payee"] != policy.expected_lnd_pubkey
    ):
        raise RealValuePolicyError("quote invoice payee is not the pinned LND node")
    for field, expected in (
        ("pftl_chain_id", policy.pftl_chain_id),
        ("pftl_genesis_hash", policy.pftl_genesis_hash),
        ("pftl_asset_id", policy.pftl_asset_id),
        ("nav_epoch", policy.pftl_nav_epoch),
        ("nav_reserve_packet_hash", policy.pftl_nav_reserve_packet_hash),
        ("asset_control_class", "CONTROLLED_ISSUED_ASSET"),
        ("timeout_clock_class", "OFFCHAIN_CROSS_LEDGER_POLICY"),
        ("custody_class", "NON_CUSTODIAL_HASHLOCK"),
        ("atomicity_class", "CONDITIONAL_HTLC"),
    ):
        if quote[field] != expected:
            raise RealValuePolicyError(f"quote {field} does not match pinned policy")
    if quote["quote_expires_unix"] <= now:
        raise RealValuePolicyError("quote is expired")
    if quote["latest_lightning_start_unix"] <= now:
        raise RealValuePolicyError("safe Lightning start cutoff has passed")
    if quote["invoice_expiry_unix"] <= now:
        raise RealValuePolicyError("invoice is expired")
    if quote["quote_expires_unix"] - now > policy.max_quote_lifetime_seconds:
        raise RealValuePolicyError("quote lifetime exceeds policy")

    principal_msat = quote["invoice_amount_msat"]
    all_in_msat = principal_msat + policy.max_fee_msat
    if all_in_msat > (1 << 63) - 1:
        raise RealValuePolicyError("all-in Lightning amount overflows uint63")
    all_in_usd = msat_to_usd_e8_ceil(all_in_msat, price.btc_usd_e8)
    if all_in_usd > policy.max_per_run_usd_e8:
        raise RealValuePolicyError("quote exceeds per-run real-value cap")

    fee_atoms = quote["coordinator_fee_atoms"]
    amount_atoms = quote["pftl_amount_atoms"]
    if quote["direction"] == "lightning_to_pftl":
        priced_atoms = amount_atoms + fee_atoms
    else:
        if fee_atoms >= amount_atoms:
            raise RealValuePolicyError("off-ramp fee consumes PFTL principal")
        priced_atoms = amount_atoms - fee_atoms
    if (
        principal_msat * quote["rate_numerator"]
        != priced_atoms * quote["rate_denominator"]
    ):
        raise RealValuePolicyError("signed rate does not exactly price fixed quote legs")

    return MainnetQuoteView(
        swap_id=quote["swap_id"],
        direction=quote["direction"],
        quote_sha256=hashlib.sha256(encoded).hexdigest(),
        invoice_amount_msat=principal_msat,
        maximum_all_in_msat=all_in_msat,
        maximum_all_in_usd_e8=all_in_usd,
        pftl_amount_atoms=amount_atoms,
        quote_expires_unix=quote["quote_expires_unix"],
        nav_epoch=quote["nav_epoch"],
        nav_reserve_packet_hash=quote["nav_reserve_packet_hash"],
    )
