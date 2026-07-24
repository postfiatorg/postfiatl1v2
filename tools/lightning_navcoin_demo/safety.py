"""Hard safety boundary for the no-value demo environment.

The harness is deliberately unusable against public Bitcoin, ce22, or a
production issued asset.  These checks run before any process is started or
any ledger operation is built.
"""

from __future__ import annotations

from dataclasses import dataclass
from ipaddress import ip_address
from pathlib import Path
from urllib.parse import urlsplit


ALLOWED_DOCKER_HOSTS = frozenset(
    {
        "bitcoind",
        "lnd-user",
        "lnd-coordinator",
        "lnd-router",
        "pftl-validator-1",
        "pftl-validator-2",
        "pftl-validator-3",
        "pftl-validator-4",
        "pftl-validator-5",
        "pftl-validator-6",
    }
)
ALLOWED_DOCKER_IPS = frozenset(
    {"172.30.24.10", "172.30.24.11", "172.30.24.12", "172.30.24.13"}
)
PUBLIC_BITCOIN_NETWORKS = frozenset({"mainnet", "bitcoin", "testnet", "signet"})
FORBIDDEN_CHAIN_MARKERS = frozenset({"ce22", "mainnet", "production", "prod"})


class SafetyViolation(RuntimeError):
    """The requested run is outside the synthetic, no-value boundary."""


def _is_loopback(host: str) -> bool:
    if host == "localhost":
        return True
    try:
        return ip_address(host).is_loopback
    except ValueError:
        return False


def _validate_local_endpoint(endpoint: str, field: str) -> None:
    if not endpoint or any(character.isspace() for character in endpoint):
        raise SafetyViolation(f"{field} must be a nonempty endpoint")
    candidate = endpoint if "://" in endpoint else f"tcp://{endpoint}"
    parsed = urlsplit(candidate)
    host = parsed.hostname
    if host is None:
        raise SafetyViolation(f"{field} has no host")
    if not (
        _is_loopback(host)
        or host in ALLOWED_DOCKER_HOSTS
        or host in ALLOWED_DOCKER_IPS
    ):
        raise SafetyViolation(f"{field} host is not local: {host}")


@dataclass(frozen=True)
class SafetyEnvelope:
    """Values that must be proven synthetic before orchestration begins."""

    bitcoin_network: str
    pftl_chain_id: str
    pftl_genesis_hash: str
    pftl_asset_symbol: str
    pftl_asset_id: str
    run_root: Path
    bitcoin_rpc_endpoint: str
    lnd_endpoints: tuple[str, str, str]
    pftl_rpc_endpoints: tuple[str, str, str, str, str, str]

    def validate(self) -> None:
        if self.bitcoin_network != "regtest":
            public = " (public network forbidden)" if self.bitcoin_network in PUBLIC_BITCOIN_NETWORKS else ""
            raise SafetyViolation(
                f"bitcoin_network must be exactly regtest, got {self.bitcoin_network!r}{public}"
            )
        normalized_chain_id = self.pftl_chain_id.lower()
        if not normalized_chain_id.startswith(("local-", "devnet-", "regtest-")):
            raise SafetyViolation("PFTL chain id must explicitly identify a local devnet")
        if any(marker in normalized_chain_id for marker in FORBIDDEN_CHAIN_MARKERS):
            raise SafetyViolation("production/ce22 PFTL chain identifiers are forbidden")
        if len(self.pftl_genesis_hash) != 96 or any(
            character not in "0123456789abcdef"
            for character in self.pftl_genesis_hash
        ):
            raise SafetyViolation("PFTL genesis hash must be canonical 48-byte hex")
        if self.pftl_asset_symbol != "LNNAVTEST":
            raise SafetyViolation("only the non-value LNNAVTEST asset is permitted")
        if len(self.pftl_asset_id) != 96 or any(
            character not in "0123456789abcdef" for character in self.pftl_asset_id
        ):
            raise SafetyViolation("PFTL test asset id must be canonical 48-byte hex")
        root = self.run_root.resolve()
        if root == Path("/") or len(root.parts) < 3:
            raise SafetyViolation("run root is too broad")
        _validate_local_endpoint(self.bitcoin_rpc_endpoint, "bitcoin_rpc_endpoint")
        if len(self.lnd_endpoints) != 3:
            raise SafetyViolation("exactly three LND endpoints are required")
        if len(set(self.lnd_endpoints)) != 3:
            raise SafetyViolation("LND endpoints must be distinct")
        for index, endpoint in enumerate(self.lnd_endpoints, start=1):
            _validate_local_endpoint(endpoint, f"lnd_endpoints[{index}]")
        if len(self.pftl_rpc_endpoints) != 6:
            raise SafetyViolation("exactly six PFTL validator endpoints are required")
        if len(set(self.pftl_rpc_endpoints)) != 6:
            raise SafetyViolation("PFTL validator endpoints must be distinct")
        for index, endpoint in enumerate(self.pftl_rpc_endpoints, start=1):
            _validate_local_endpoint(endpoint, f"pftl_rpc_endpoints[{index}]")
