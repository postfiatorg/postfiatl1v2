"""Read-only inbound-liquidity discovery for the coordinator LND node.

This module deliberately implements only Magma's public ``get_info`` call.
Creating an order returns a payable Lightning invoice and is therefore kept
outside the unattended coordinator.  The operator may proceed only after the
node, PFTL route, regtest rehearsal, mainnet dry check, and a separately signed
``LIQUIDITY_SETUP`` value authorization are all green.
"""

from __future__ import annotations

from dataclasses import dataclass
import json
import re
import ssl
from typing import Any, Callable, Mapping
from urllib.request import Request, urlopen

from .policy import RealValuePolicyError


MAGMA_INFO_URLS = {
    "recommended": "https://magma.amboss.tech/api/lsp/v1/recommended/get_info",
    "cheapest": "https://magma.amboss.tech/api/lsp/v1/cheapest/get_info",
}
MAX_RESPONSE_BYTES = 64 * 1024
NODE_URI = re.compile(r"^(02|03)[0-9a-f]{64}@[A-Za-z0-9.:-]{1,255}$")


class LiquidityDiscoveryError(RealValuePolicyError):
    """The public LSP capability response was absent or unsafe."""


def _uint(value: Any, name: str, *, minimum: int = 0) -> int:
    if type(value) is int:
        parsed = value
    elif type(value) is str and value.isascii() and value.isdigit():
        if len(value) > 1 and value.startswith("0"):
            raise LiquidityDiscoveryError(f"{name} is not canonical decimal")
        parsed = int(value)
    else:
        raise LiquidityDiscoveryError(f"{name} must be an unsigned integer")
    if parsed < minimum or parsed > (1 << 63) - 1:
        raise LiquidityDiscoveryError(f"{name} is outside uint63 bounds")
    return parsed


@dataclass(frozen=True)
class MagmaCapabilities:
    pool: str
    min_required_channel_confirmations: int
    min_funding_confirms_within_blocks: int
    max_channel_expiry_blocks: int
    min_initial_client_balance_sat: int
    max_initial_client_balance_sat: int
    min_initial_lsp_balance_sat: int
    max_initial_lsp_balance_sat: int
    min_channel_balance_sat: int
    max_channel_balance_sat: int
    uris: tuple[str, ...]

    @classmethod
    def from_mapping(
        cls, pool: str, value: Mapping[str, Any]
    ) -> "MagmaCapabilities":
        if pool not in MAGMA_INFO_URLS or not isinstance(value, Mapping):
            raise LiquidityDiscoveryError("invalid Magma capability response")
        required = {
            "min_required_channel_confirmations",
            "min_funding_confirms_within_blocks",
            "max_channel_expiry_blocks",
            "min_initial_client_balance_sat",
            "max_initial_client_balance_sat",
            "min_initial_lsp_balance_sat",
            "max_initial_lsp_balance_sat",
            "min_channel_balance_sat",
            "max_channel_balance_sat",
            "uris",
        }
        if not required.issubset(value.keys()):
            raise LiquidityDiscoveryError("Magma capability response is incomplete")
        uris = value["uris"]
        if (
            not isinstance(uris, list)
            or not uris
            or len(uris) > 32
            or any(type(uri) is not str or NODE_URI.fullmatch(uri) is None for uri in uris)
        ):
            raise LiquidityDiscoveryError("Magma capability peer URIs are invalid")
        result = cls(
            pool=pool,
            min_required_channel_confirmations=_uint(
                value["min_required_channel_confirmations"],
                "min_required_channel_confirmations",
            ),
            min_funding_confirms_within_blocks=_uint(
                value["min_funding_confirms_within_blocks"],
                "min_funding_confirms_within_blocks",
                minimum=1,
            ),
            max_channel_expiry_blocks=_uint(
                value["max_channel_expiry_blocks"],
                "max_channel_expiry_blocks",
                minimum=1,
            ),
            min_initial_client_balance_sat=_uint(
                value["min_initial_client_balance_sat"],
                "min_initial_client_balance_sat",
            ),
            max_initial_client_balance_sat=_uint(
                value["max_initial_client_balance_sat"],
                "max_initial_client_balance_sat",
            ),
            min_initial_lsp_balance_sat=_uint(
                value["min_initial_lsp_balance_sat"],
                "min_initial_lsp_balance_sat",
                minimum=1,
            ),
            max_initial_lsp_balance_sat=_uint(
                value["max_initial_lsp_balance_sat"],
                "max_initial_lsp_balance_sat",
                minimum=1,
            ),
            min_channel_balance_sat=_uint(
                value["min_channel_balance_sat"],
                "min_channel_balance_sat",
                minimum=1,
            ),
            max_channel_balance_sat=_uint(
                value["max_channel_balance_sat"],
                "max_channel_balance_sat",
                minimum=1,
            ),
            uris=tuple(uris),
        )
        if (
            result.min_initial_client_balance_sat
            > result.max_initial_client_balance_sat
            or result.min_initial_lsp_balance_sat
            > result.max_initial_lsp_balance_sat
            or result.min_channel_balance_sat > result.max_channel_balance_sat
        ):
            raise LiquidityDiscoveryError("Magma capability ranges are inverted")
        return result

    def supports_provider_funded_inbound(self, amount_sat: int) -> bool:
        if type(amount_sat) is not int or amount_sat <= 0:
            raise LiquidityDiscoveryError("requested inbound must be positive")
        return (
            self.min_initial_client_balance_sat == 0
            and self.max_initial_client_balance_sat == 0
            and self.min_initial_lsp_balance_sat
            <= amount_sat
            <= self.max_initial_lsp_balance_sat
            and self.min_channel_balance_sat
            <= amount_sat
            <= self.max_channel_balance_sat
        )

    def public_evidence(self) -> dict[str, Any]:
        return {
            "schema": "postfiat.lightning_lsp_capabilities.v1",
            "provider": "Amboss Magma BLIP-0051",
            "pool": self.pool,
            "read_only": True,
            "order_created": False,
            "min_required_channel_confirmations": (
                self.min_required_channel_confirmations
            ),
            "min_funding_confirms_within_blocks": (
                self.min_funding_confirms_within_blocks
            ),
            "max_channel_expiry_blocks": self.max_channel_expiry_blocks,
            "min_initial_lsp_balance_sat": self.min_initial_lsp_balance_sat,
            "max_initial_lsp_balance_sat": self.max_initial_lsp_balance_sat,
            "peer_count": len(self.uris),
        }


def read_magma_capabilities(
    pool: str = "recommended",
    *,
    opener: Callable[..., Any] = urlopen,
) -> MagmaCapabilities:
    """Fetch one public capability document; never creates or pays an order."""

    try:
        url = MAGMA_INFO_URLS[pool]
    except KeyError as error:
        raise LiquidityDiscoveryError("unsupported Magma pool") from error
    request = Request(
        url,
        headers={
            "Accept": "application/json",
            "User-Agent": "PostFiat-Lightning-NAVcoin-dry-check/1",
        },
        method="GET",
    )
    try:
        response = opener(
            request,
            timeout=10,
            context=ssl.create_default_context(),
        )
        with response:
            final_url = response.geturl()
            status = response.getcode()
            content_length = response.headers.get("Content-Length")
            if final_url != url or status != 200:
                raise LiquidityDiscoveryError("Magma get_info redirected or failed")
            if content_length is not None and int(content_length) > MAX_RESPONSE_BYTES:
                raise LiquidityDiscoveryError("Magma get_info response is oversized")
            encoded = response.read(MAX_RESPONSE_BYTES + 1)
    except LiquidityDiscoveryError:
        raise
    except Exception as error:
        raise LiquidityDiscoveryError("Magma get_info request failed") from error
    if len(encoded) > MAX_RESPONSE_BYTES:
        raise LiquidityDiscoveryError("Magma get_info response is oversized")
    try:
        value = json.loads(encoded.decode("utf-8"))
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise LiquidityDiscoveryError("Magma get_info is not valid JSON") from error
    return MagmaCapabilities.from_mapping(pool, value)
