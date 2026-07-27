#!/usr/bin/env python3
"""Digest-gated Ethereum mainnet pfUSDC deployment driver.

This is the chain-1 counterpart of the Sepolia driver.  It intentionally has
no private-key input: StakeHub agentd is the sole signing path.  The manifest
and deployment authorization are validated entirely locally before Web3,
agentd, a launch session, or a sender can be initialized.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

from Crypto.Hash import keccak

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
PYTHON_ROOT = REPOSITORY_ROOT / "python"
if str(PYTHON_ROOT) not in sys.path:
    sys.path.insert(0, str(PYTHON_ROOT))

from postfiat_ops.deployment_auditor_authorization import (
    AuthorizationError,
    require_auditor_authorization,
    retain_prebroadcast_manifest,
)

DEFAULT_MANIFEST = REPOSITORY_ROOT / "deployments/pfusdc-eth-mainnet-20260726/manifest.mainnet-epoch3.json"
DEFAULT_EVIDENCE_DIR = REPOSITORY_ROOT / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/deploy"
DEFAULT_FUNDING_RECONCILIATION = REPOSITORY_ROOT / "docs/evidence/pfusdc-eth-campaign-20260725/lane-mainnet/gas-preflight/funding-reconciliation.json"
DEFAULT_STAKEHUB_REPO = REPOSITORY_ROOT.parent / "StakeHub"
ETHEREUM_MAINNET_CHAIN_ID = 1
CANONICAL_USDC = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48"
SP1_GATEWAY = "0x397a5f7f3dbd538f23de225b51f532c34448da9b"
REQUIRED_MANIFEST_SHA256 = "bf769e122a66274facd39023b3939e963cba4518044b22e4537bc351479471c2"
FALLBACK_AUTHORIZATION_SCHEMA = "postfiat.pfusdc.mainnet_epoch3_auditor_authorization.v1"
GAS_PRE_SEND_LIMIT_USD = 130.0
GAS_HARD_LIMIT_USD = 150.0


class DeploymentError(RuntimeError):
    """A fail-closed deployment precondition, scope, or receipt failure."""


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise DeploymentError(f"invalid JSON object {path}: {exc}") from exc
    if not isinstance(value, dict):
        raise DeploymentError(f"expected JSON object: {path}")
    return value


def _keccak(data: bytes) -> bytes:
    digest = keccak.new(digest_bits=256)
    digest.update(data)
    return digest.digest()


def _rlp_bytes(value: bytes) -> bytes:
    if len(value) == 1 and value[0] < 0x80:
        return value
    if len(value) < 56:
        return bytes([0x80 + len(value)]) + value
    size = len(value).to_bytes((len(value).bit_length() + 7) // 8, "big")
    return bytes([0xB7 + len(size)]) + size + value


def _rlp_list(values: list[bytes]) -> bytes:
    payload = b"".join(values)
    if len(payload) < 56:
        return bytes([0xC0 + len(payload)]) + payload
    size = len(payload).to_bytes((len(payload).bit_length() + 7) // 8, "big")
    return bytes([0xF7 + len(size)]) + size + payload


def create_address(deployer: str, nonce: int) -> str:
    if nonce < 0:
        raise DeploymentError("CREATE nonce cannot be negative")
    try:
        sender = bytes.fromhex(deployer.removeprefix("0x"))
    except ValueError as exc:
        raise DeploymentError("deployer address is not hexadecimal") from exc
    if len(sender) != 20:
        raise DeploymentError("deployer address must be 20 bytes")
    encoded_nonce = b"" if nonce == 0 else nonce.to_bytes((nonce.bit_length() + 7) // 8, "big")
    return "0x" + _keccak(_rlp_list([_rlp_bytes(sender), _rlp_bytes(encoded_nonce)]))[12:].hex()


def _required(document: dict[str, Any], path: tuple[str, ...], expected: type) -> Any:
    value: Any = document
    for key in path:
        if not isinstance(value, dict) or key not in value:
            raise DeploymentError("manifest missing " + ".".join(path))
        value = value[key]
    if expected is int and isinstance(value, bool):
        raise DeploymentError("manifest has invalid type at " + ".".join(path))
    if not isinstance(value, expected):
        raise DeploymentError("manifest has invalid type at " + ".".join(path))
    return value


def _contract_artifacts(manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    artifacts = _required(manifest, ("contracts", "artifacts"), list)
    expected = {"PFTLFinalityVerifierV1", "ERC20BridgeVaultL1"}
    result: dict[str, dict[str, Any]] = {}
    for item in artifacts:
        if not isinstance(item, dict) or not isinstance(item.get("contract"), str):
            raise DeploymentError("manifest has invalid contract artifact")
        name = item["contract"]
        if name not in expected or name in result:
            raise DeploymentError("manifest contract artifacts do not exactly match the deployment pair")
        for key in ("address", "path", "artifact_sha256", "creation_bytecode_keccak256", "unlinked_deployed_bytecode_keccak256"):
            if not isinstance(item.get(key), str) or not item[key]:
                raise DeploymentError(f"manifest {name}.{key} is missing or invalid")
        if not isinstance(item.get("precomputed_create_nonce"), int) or isinstance(item["precomputed_create_nonce"], bool):
            raise DeploymentError(f"manifest {name}.precomputed_create_nonce is invalid")
        artifact_path = REPOSITORY_ROOT / item["path"]
        if not artifact_path.is_file() or _sha256(artifact_path) != item["artifact_sha256"]:
            raise DeploymentError(f"manifest {name} artifact digest does not match disk")
        result[name] = item
    if set(result) != expected:
        raise DeploymentError("manifest is missing a deployment artifact")
    return result


def _validate_scope(manifest: dict[str, Any]) -> None:
    network = manifest.get("network")
    if not isinstance(network, dict):
        raise DeploymentError("manifest missing network")
    if network.get("source_chain_id") != ETHEREUM_MAINNET_CHAIN_ID:
        raise DeploymentError("manifest source_chain_id is not Ethereum mainnet")
    token = network.get("token")
    gateway = network.get("sp1_verifier_gateway")
    if not isinstance(token, dict) or str(token.get("address", "")).lower() != CANONICAL_USDC:
        raise DeploymentError("manifest token is not canonical Circle mainnet USDC")
    if not isinstance(gateway, dict) or str(gateway.get("address", "")).lower() != SP1_GATEWAY:
        raise DeploymentError("manifest SP1 gateway is not the canonical mainnet gateway")
    if manifest.get("revision") != "mainnet-epoch3":
        raise DeploymentError("manifest revision is not mainnet-epoch3")
    route = manifest.get("route")
    if not isinstance(route, dict) or route.get("route_id") != "ethereum-mainnet-usdc-v1" or route.get("route_epoch") != 3:
        raise DeploymentError("manifest route is not the pinned mainnet epoch-3 route")


def build_verifier_constructor_inputs(manifest: dict[str, Any]) -> tuple[Any, ...]:
    """Build the exact verifier constructor tuple solely from the manifest."""
    network, programs, pftl = manifest["network"], manifest["programs"], manifest["pftl"]
    return (
        network["sp1_verifier_gateway"]["address"],
        programs["egress"]["program_vkey"],
        pftl["chain_id_hash"], pftl["genesis_hash_commitment"], int(pftl["protocol_version"]),
        pftl["route_profile_hash_commitment"], int(pftl["route_epoch"]),
        pftl["asset_id_commitment"], ETHEREUM_MAINNET_CHAIN_ID,
        pftl["vault_runtime_code_hash"], network["token"]["address"],
        network["token"]["runtime_code_hash"], int(programs["max_proof_bytes"]),
        int(programs["max_public_values_bytes"]), pftl["initial_checkpoint_commitment"],
        int(pftl["initial_finalized_height"]), pftl["initial_committee_root_commitment"],
    )


def validate_offline_manifest(manifest_path: Path, manifest: dict[str, Any]) -> dict[str, dict[str, Any]]:
    """Validate all static fields without importing Web3, agentd, or credentials."""
    if manifest_path.resolve() != DEFAULT_MANIFEST.resolve():
        raise DeploymentError("manifest path is not the pinned mainnet epoch-3 manifest")
    if _sha256(manifest_path) != REQUIRED_MANIFEST_SHA256:
        raise DeploymentError("manifest digest is not the authorized mainnet epoch-3 digest")
    for path, expected in {
        ("deployer", "address"): str,
        ("network", "execution_rpc_default"): str,
        ("network", "beacon_rpc_default"): str,
        ("network", "token", "runtime_code_hash"): str,
        ("network", "sp1_verifier_gateway", "runtime_code_hash"): str,
        ("programs", "egress", "program_vkey"): str,
        ("programs", "max_proof_bytes"): int,
        ("programs", "max_public_values_bytes"): int,
        ("pftl", "chain_id_hash"): str,
        ("pftl", "genesis_hash_commitment"): str,
        ("pftl", "protocol_version"): int,
        ("pftl", "route_profile_hash_commitment"): str,
        ("pftl", "route_epoch"): int,
        ("pftl", "asset_id_commitment"): str,
        ("pftl", "vault_runtime_code_hash"): str,
        ("pftl", "initial_checkpoint_commitment"): str,
        ("pftl", "initial_finalized_height"): int,
        ("pftl", "initial_committee_root_commitment"): str,
        ("route", "vault_runtime_code_hash"): str,
    }.items():
        _required(manifest, path, expected)
    _validate_scope(manifest)
    artifacts = _contract_artifacts(manifest)
    verifier, vault = artifacts["PFTLFinalityVerifierV1"], artifacts["ERC20BridgeVaultL1"]
    if (verifier["precomputed_create_nonce"], vault["precomputed_create_nonce"]) != (153, 154):
        raise DeploymentError("manifest CREATE nonces are not the pinned 153/154 pair")
    deployer = manifest["deployer"]["address"]
    if create_address(deployer, 153).lower() != verifier["address"].lower():
        raise DeploymentError("manifest verifier address does not match CREATE(deployer,153)")
    if create_address(deployer, 154).lower() != vault["address"].lower():
        raise DeploymentError("manifest vault address does not match CREATE(deployer,154)")
    if manifest["route"].get("verifier_address", "").lower() != verifier["address"].lower():
        raise DeploymentError("manifest route verifier address disagrees with artifact plan")
    if manifest["route"].get("vault_address", "").lower() != vault["address"].lower():
        raise DeploymentError("manifest route vault address disagrees with artifact plan")
    build_verifier_constructor_inputs(manifest)
    return artifacts


def _load_fallback_authorization(path: Path, manifest_path: Path) -> dict[str, Any]:
    document = _load_json(path.resolve())
    if document.get("schema") != FALLBACK_AUTHORIZATION_SCHEMA:
        raise DeploymentError("fallback authorization schema is unsupported")
    if document.get("verdict") != "PASS" or document.get("rows") != "68/68" or document.get("c8") != "15/15":
        raise DeploymentError("fallback authorization is not a 68/68 PASS with C8 15/15")
    if document.get("authorized_manifest_sha256", "").lower() != REQUIRED_MANIFEST_SHA256:
        raise DeploymentError("fallback authorization does not bind the authorized manifest digest")
    if _sha256(manifest_path) != REQUIRED_MANIFEST_SHA256:
        raise DeploymentError("manifest digest does not match fallback authorization")
    for key in ("auditor", "audit_log", "audit_log_sha256", "timestamp_utc"):
        if not isinstance(document.get(key), str) or not document[key]:
            raise DeploymentError(f"fallback authorization missing {key}")
    return {"manifest_sha256": REQUIRED_MANIFEST_SHA256, "authorization": document}


def require_mainnet_authorization(manifest_path: Path, manifest: dict[str, Any], authorization_path: Path | None) -> dict[str, Any]:
    """Accept the approved fallback schema or the generic digest-gate schema."""
    if authorization_path is None:
        raise DeploymentError("--deployment-authorization is required for a send-capable invocation")
    try:
        schema = _load_json(authorization_path.resolve()).get("schema")
    except DeploymentError:
        raise
    if schema == FALLBACK_AUTHORIZATION_SCHEMA:
        return _load_fallback_authorization(authorization_path, manifest_path)
    try:
        result = require_auditor_authorization(manifest_path, manifest, authorization_path, ETHEREUM_MAINNET_CHAIN_ID)
    except AuthorizationError as exc:
        raise DeploymentError(str(exc)) from exc
    if result["manifest_sha256"] != REQUIRED_MANIFEST_SHA256:
        raise DeploymentError("generic authorization does not bind the authorized mainnet manifest digest")
    return result


class BudgetGate:
    """Local, fail-closed gas budget gate using the funding reconciliation."""
    SCHEMA = "postfiat.pfusdc.mainnet_gas_funding_reconciliation.v1"

    def __init__(self, reconciliation_path: Path):
        document = _load_json(reconciliation_path)
        if document.get("schema") != self.SCHEMA:
            raise DeploymentError("funding reconciliation schema is unsupported")
        basis = document.get("eth_usd_basis")
        projections = document.get("projected_remaining_gas_usd")
        if not isinstance(basis, dict) or not isinstance(projections, dict):
            raise DeploymentError("funding reconciliation lacks ETH/USD basis or gas projections")
        price = basis.get("usd_per_eth")
        if not isinstance(price, (int, float)) or isinstance(price, bool) or price <= 0:
            raise DeploymentError("funding reconciliation has invalid ETH/USD basis")
        self.eth_usd = float(price)
        self.basis = basis
        self.projected: dict[str, float] = {}
        for label in ("verifier", "vault"):
            value = projections.get(label)
            if not isinstance(value, (int, float)) or isinstance(value, bool) or value < 0:
                raise DeploymentError(f"funding reconciliation has invalid projected gas for {label}")
            self.projected[label] = float(value)
        self.actual: list[dict[str, Any]] = []

    @property
    def cumulative_actual_usd(self) -> float:
        return sum(float(entry["actual_usd"]) for entry in self.actual)

    def require_before_send(self, remaining: list[str]) -> None:
        projected = sum(self.projected[label] for label in remaining)
        total = self.cumulative_actual_usd + projected
        if self.cumulative_actual_usd >= GAS_HARD_LIMIT_USD:
            raise DeploymentError(f"actual gas USD reached hard ceiling {GAS_HARD_LIMIT_USD:.2f}")
        if total >= GAS_PRE_SEND_LIMIT_USD:
            raise DeploymentError(
                f"gas projection {total:.6f} USD is not below pre-send limit {GAS_PRE_SEND_LIMIT_USD:.2f}"
            )

    def record_receipt(self, label: str, receipt: Any) -> dict[str, Any]:
        gas_units = int(receipt["gasUsed"])
        effective_gas_price = int(receipt["effectiveGasPrice"])
        actual_usd = gas_units * effective_gas_price / 1_000_000_000_000_000_000 * self.eth_usd
        entry = {
            "label": label,
            "gas_units": gas_units,
            "effective_gas_price_wei": effective_gas_price,
            "eth_usd_basis": self.basis,
            "actual_usd": actual_usd,
        }
        self.actual.append(entry)
        entry["cumulative_actual_usd"] = self.cumulative_actual_usd
        return entry

    def require_after_receipt(self) -> None:
        if self.cumulative_actual_usd >= GAS_HARD_LIMIT_USD:
            raise DeploymentError(f"actual gas USD reached hard ceiling {GAS_HARD_LIMIT_USD:.2f}")

    def evidence(self) -> dict[str, Any]:
        return {
            "schema": "postfiat.pfusdc.mainnet_gas_accounting.v1",
            "eth_usd_basis": self.basis,
            "entries": self.actual,
            "cumulative_actual_usd": self.cumulative_actual_usd,
            "pre_send_limit_usd": GAS_PRE_SEND_LIMIT_USD,
            "hard_ceiling_usd": GAS_HARD_LIMIT_USD,
        }


class MainnetDeployer:
    def __init__(self, args: argparse.Namespace, manifest_path: Path, manifest: dict[str, Any], artifacts: dict[str, dict[str, Any]], prebroadcast_path: Path, authorization: dict[str, Any], budget: BudgetGate):
        self.args, self.manifest_path, self.manifest, self.artifacts = args, manifest_path, manifest, artifacts
        self.prebroadcast_path, self.authorization, self.budget = prebroadcast_path, authorization, budget
        self.rpc = args.rpc or manifest["network"]["execution_rpc_default"]
        self.web3: Any = None
        self.agent_call: Any = None

    @staticmethod
    def _load_agent_call(stakehub_repo: Path):
        package = stakehub_repo / "stakehub/agentd.py"
        if not package.is_file():
            raise DeploymentError(f"StakeHub agentd not found under {stakehub_repo}")
        sys.path.insert(0, str(stakehub_repo))
        from stakehub.agentd import call
        return call

    def _load_web3(self):
        try:
            from web3 import Web3
        except ImportError as exc:
            raise DeploymentError("web3 is required only for send-capable deployment") from exc
        client = Web3(Web3.HTTPProvider(self.rpc, request_kwargs={"timeout": 30}))
        if not client.is_connected() or client.eth.chain_id != ETHEREUM_MAINNET_CHAIN_ID:
            raise DeploymentError("RPC is not reachable Ethereum mainnet")
        return client

    def initialize_network_clients(self) -> None:
        self.agent_call = self._load_agent_call(self.args.stakehub_repo.resolve())
        self.web3 = self._load_web3()

    def _artifact(self, contract: str) -> dict[str, Any]:
        return _load_json(REPOSITORY_ROOT / self.artifacts[contract]["path"])

    def _constructor_data(self, contract: str, types: list[str], values: list[Any]) -> str:
        creation = self._artifact(contract)["bytecode"]["object"].removeprefix("0x")
        return "0x" + creation + self.web3.codec.encode(types, values).hex()

    def _expected_deploy(self, contract: str, label: str, types: list[str], values: list[Any]) -> tuple[dict[str, Any], str]:
        creation = bytes.fromhex(self._artifact(contract)["bytecode"]["object"].removeprefix("0x"))
        return ({"label": label, "bytecode_hash": self.web3.to_hex(self.web3.keccak(creation)).lower(), "bytecode_len": len(creation)}, self._constructor_data(contract, types, values))

    def _open_session(self, expected_deploys: list[dict[str, Any]]) -> str:
        response = self.agent_call({
            "op": "open_launch_session",
            "session_id": f"pfusdc-{self.manifest['revision']}-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}",
            "chain_id": ETHEREUM_MAINNET_CHAIN_ID,
            "usdc_address": CANONICAL_USDC,
            "usdc_budget": 0,
            "allowlist": [CANONICAL_USDC, SP1_GATEWAY],
            "expected_deploys": expected_deploys,
            "ttl_seconds": 3600,
        }, timeout=60.0)
        session = response.get("session") if isinstance(response, dict) else None
        session_id = None
        if isinstance(response, dict):
            session_id = response.get("session_id") or response.get("id")
            if session_id is None and isinstance(session, dict):
                session_id = session.get("id")
        if not response or not response.get("ok") or not isinstance(session_id, str):
            error = response.get("error") if isinstance(response, dict) else None
            detail = f": {error}" if isinstance(error, str) and error else ""
            raise DeploymentError(
                f"StakeHub agentd did not open the chain-1 allowlisted launch session{detail}"
            )
        return session_id

    def _send(self, session_id: str, action: str, data: str, label: str) -> dict[str, Any]:
        response = self.agent_call({
            "op": "evm_contract_tx", "to": None, "data": data, "rpc_url": self.rpc,
            "chain_id": ETHEREUM_MAINNET_CHAIN_ID, "session_id": session_id,
            "session_action": action, "label": label,
        }, timeout=1200.0)
        if not response or not response.get("ok"):
            raise DeploymentError(f"{label} deployment failed through agentd")
        if not isinstance(response.get("tx"), str) or not isinstance(response.get("contract_address"), str):
            raise DeploymentError(f"{label} agentd response lacks transaction or contract address")
        return response

    def _persist_gas_accounting(self) -> None:
        self.args.evidence_dir.mkdir(parents=True, exist_ok=True)
        (self.args.evidence_dir / "gas-accounting.json").write_text(
            json.dumps(self.budget.evidence(), indent=2, sort_keys=True) + "\n",
            encoding="utf-8",
        )

    def run(self) -> None:
        deployer = self.web3.to_checksum_address(self.manifest["deployer"]["address"])
        verifier, vault = self.artifacts["PFTLFinalityVerifierV1"], self.artifacts["ERC20BridgeVaultL1"]
        live_nonce = self.web3.eth.get_transaction_count(deployer)
        if live_nonce != verifier["precomputed_create_nonce"]:
            raise DeploymentError(f"deployer nonce {live_nonce} != planned verifier nonce {verifier['precomputed_create_nonce']}")
        if create_address(deployer, verifier["precomputed_create_nonce"]).lower() != verifier["address"].lower() or create_address(deployer, vault["precomputed_create_nonce"]).lower() != vault["address"].lower():
            raise DeploymentError("live deploy plan does not retain the manifest CREATE addresses")
        config = list(build_verifier_constructor_inputs(self.manifest))
        config[0] = self.web3.to_checksum_address(config[0])
        config[10] = self.web3.to_checksum_address(config[10])
        verifier_expected, verifier_data = self._expected_deploy(
            "PFTLFinalityVerifierV1", "deploy-pftl-finality-verifier-v1",
            ["(address,bytes32,bytes32,bytes32,uint32,bytes32,uint64,bytes32,uint64,bytes32,address,bytes32,uint256,uint256,bytes32,uint64,bytes32)"], [tuple(config)],
        )
        vault_values = [self.web3.to_checksum_address(CANONICAL_USDC), self.web3.to_checksum_address(verifier["address"]), self.manifest["network"]["token"]["runtime_code_hash"], deployer]
        vault_expected, vault_data = self._expected_deploy("ERC20BridgeVaultL1", "deploy-erc20-bridge-vault-l1", ["address", "address", "bytes32", "address"], vault_values)
        session = self._open_session([verifier_expected, vault_expected])
        evidence: dict[str, Any] = {"schema": "postfiat.pfusdc.eth_mainnet_deploy_evidence.v1", "session_id": session, "started_utc": datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ"), "manifest_digests": {"pre_broadcast_input_path": str(self.prebroadcast_path), "pre_broadcast_input_sha256": self.authorization["manifest_sha256"]}, "steps": []}
        try:
            self.budget.require_before_send(["verifier", "vault"])
            verifier_result = self._send(session, "deploy-pftl-finality-verifier-v1", verifier_data, "PFTLFinalityVerifierV1")
            if verifier_result["contract_address"].lower() != verifier["address"].lower():
                raise DeploymentError("verifier receipt address differs from manifest")
            verifier_receipt = self.web3.eth.get_transaction_receipt(verifier_result["tx"])
            if int(verifier_receipt["status"]) != 1:
                raise DeploymentError("verifier deployment receipt reverted")
            evidence["steps"].append({"contract": "PFTLFinalityVerifierV1", **verifier_result, "gas": self.budget.record_receipt("verifier", verifier_receipt)})
            self._persist_gas_accounting()
            self.budget.require_after_receipt()
            self.budget.require_before_send(["vault"])
            vault_result = self._send(session, "deploy-erc20-bridge-vault-l1", vault_data, "ERC20BridgeVaultL1")
            if vault_result["contract_address"].lower() != vault["address"].lower():
                raise DeploymentError("vault receipt address differs from manifest")
            vault_receipt = self.web3.eth.get_transaction_receipt(vault_result["tx"])
            if int(vault_receipt["status"]) != 1:
                raise DeploymentError("vault deployment receipt reverted")
            evidence["steps"].append({"contract": "ERC20BridgeVaultL1", **vault_result, "gas": self.budget.record_receipt("vault", vault_receipt)})
            self._persist_gas_accounting()
            self.budget.require_after_receipt()
        finally:
            self.agent_call({"op": "close_launch_session", "session_id": session}, timeout=30.0)
        self.args.evidence_dir.mkdir(parents=True, exist_ok=True)
        enriched = dict(self.manifest)
        enriched["status"] = "deployed-not-activated"
        enriched["deployments"] = evidence["steps"]
        enriched_path = self.args.evidence_dir / "manifest.postdeploy-enriched.json"
        enriched_path.write_text(json.dumps(enriched, indent=2, sort_keys=True) + "\n", encoding="utf-8")
        evidence["manifest_digests"]["postdeploy_enriched_manifest_sha256"] = _sha256(enriched_path)
        evidence["gas_accounting"] = self.budget.evidence()
        self._persist_gas_accounting()
        (self.args.evidence_dir / "deploy-result.json").write_text(json.dumps(evidence, indent=2, sort_keys=True) + "\n", encoding="utf-8")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument(
        "--deployment-authorization",
        "--auditor-authorization",
        dest="auditor_authorization",
        type=Path,
    )
    parser.add_argument("--evidence-dir", type=Path, default=DEFAULT_EVIDENCE_DIR)
    parser.add_argument("--gas-preflight", type=Path, default=DEFAULT_FUNDING_RECONCILIATION)
    parser.add_argument("--stakehub-repo", type=Path, default=DEFAULT_STAKEHUB_REPO)
    parser.add_argument("--rpc", default=None)
    parser.add_argument("--credential-file", "--private-key-file", "--key-file", type=Path, default=None, help=argparse.SUPPRESS)
    parser.add_argument("--check-only", action="store_true")
    args = parser.parse_args()
    try:
        if args.credential_file is not None:
            raise DeploymentError("credential input is prohibited; StakeHub agentd is the only signing path")
        manifest_path = args.manifest.resolve()
        manifest = _load_json(manifest_path)
        artifacts = validate_offline_manifest(manifest_path, manifest)
        if args.auditor_authorization is not None:
            authorization = require_mainnet_authorization(manifest_path, manifest, args.auditor_authorization)
        elif args.check_only:
            authorization = None
        else:
            raise DeploymentError("--deployment-authorization is required for a send-capable invocation")
        if args.check_only:
            print(json.dumps({"ok": True, "manifest_sha256": _sha256(manifest_path), "authorization_verified": authorization is not None, "validation": "offline-only; no Web3, socket, agentd, credential, or sender initialized"}))
            return 0
        if authorization is None:
            raise DeploymentError("missing deployment authorization")
        budget = BudgetGate(args.gas_preflight.resolve())
        retained = retain_prebroadcast_manifest(manifest_path, args.evidence_dir, authorization["manifest_sha256"])
        deployer = MainnetDeployer(args, manifest_path, manifest, artifacts, retained, authorization, budget)
        deployer.initialize_network_clients()
        deployer.run()
        return 0
    except (AuthorizationError, DeploymentError) as exc:
        print(f"DEPLOYMENT_BLOCKED: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
