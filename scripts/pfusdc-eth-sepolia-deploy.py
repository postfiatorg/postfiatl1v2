#!/usr/bin/env python3
"""Lane C Sepolia deployment driver — PFTLFinalityVerifierV1 + ERC20BridgeVaultL1.

Deploys the proof-native pfUSDC Ethereum L1 rail contracts on Ethereum
Sepolia (chain id 11155111) through the unlocked StakeHub agentd, which is
the only signing path: this script never reads, accepts, prints, or logs a
private key. Every deployed value is read back from chain (runtime code
hash + constructor immutables) and the deployment manifest is updated with
addresses and transaction hashes.

Scope guards: Ethereum Sepolia only; no Arbitrum markers; canonical Circle
Sepolia USDC; canonical Succinct SP1 verifier gateway.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import subprocess
import sys
from datetime import UTC, datetime
from pathlib import Path
from typing import Any

REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
PYTHON_ROOT = REPOSITORY_ROOT / "python"
if str(PYTHON_ROOT) not in sys.path:
    sys.path.insert(0, str(PYTHON_ROOT))

from postfiat_ops.deployment_auditor_authorization import (
    AuthorizationError,
    require_auditor_authorization,
    retain_prebroadcast_manifest,
)
DEFAULT_MANIFEST = REPOSITORY_ROOT / "deployments/pfusdc-eth-sepolia-20260725/manifest.json"
DEFAULT_STAKEHUB_REPO = REPOSITORY_ROOT.parent / "StakeHub"
DEFAULT_EVIDENCE_DIR = (
    REPOSITORY_ROOT / "docs/evidence/pfusdc-eth-campaign-20260725/lane-c/deploy"
)
ETHEREUM_SEPOLIA_CHAIN_ID = 11_155_111
DEFAULT_ETHEREUM_RPC = "https://ethereum-sepolia-rpc.publicnode.com"
CANONICAL_USDC = "0x1c7d4b196cb0c7b01d743fbc6116a902379c7238"
SP1_GATEWAY = "0x3b6041173b80e77f038f3f2c0f9744f04837185e"
FLEET_INVENTORY = REPOSITORY_ROOT.parent / "wan-vultr-all-fleet.txt"
COMMITTEE_ROOT_DOMAIN = "postfiat.consensus.committee-root.v2"


class DeploymentError(RuntimeError):
    """A fail-closed manifest, scope, chain, or readback failure."""


def _sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def _load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise DeploymentError(f"expected JSON object: {path}")
    return value


def _reject_arbitrum_scope(manifest: dict[str, Any]) -> None:
    blob = json.dumps(manifest).lower()
    if "arbitrum" in blob:
        raise DeploymentError("manifest carries an Arbitrum marker; Lane C is Ethereum Sepolia only")
    if manifest["network"]["source_chain_id"] != ETHEREUM_SEPOLIA_CHAIN_ID:
        raise DeploymentError("manifest source_chain_id is not Ethereum Sepolia")
    if manifest["network"]["token"]["address"].lower() != CANONICAL_USDC:
        raise DeploymentError("manifest token is not canonical Circle Sepolia USDC")
    if manifest["network"]["sp1_verifier_gateway"]["address"].lower() != SP1_GATEWAY:
        raise DeploymentError("manifest SP1 gateway is not the canonical Succinct Sepolia gateway")


def _require_type(document: dict[str, Any], path: tuple[str, ...], expected: type) -> Any:
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


def _validate_contract_artifacts(manifest: dict[str, Any]) -> None:
    artifacts = _require_type(manifest, ("contracts", "artifacts"), list)
    required_contracts = {"PFTLFinalityVerifierV1", "ERC20BridgeVaultL1"}
    seen: set[str] = set()
    for artifact in artifacts:
        if not isinstance(artifact, dict):
            raise DeploymentError("manifest contract artifact must be an object")
        name = artifact.get("contract")
        if name not in required_contracts or name in seen:
            raise DeploymentError("manifest contract artifacts do not exactly match the deployment pair")
        seen.add(name)
        for field in (
            "address", "path", "artifact_sha256", "creation_bytecode_keccak256",
            "unlinked_deployed_bytecode_keccak256",
        ):
            value = artifact.get(field)
            if not isinstance(value, str) or not value:
                raise DeploymentError(f"manifest {name}.{field} is missing or invalid")
        nonce = artifact.get("precomputed_create_nonce")
        if not isinstance(nonce, int) or isinstance(nonce, bool) or nonce < 0:
            raise DeploymentError(f"manifest {name} has an invalid precomputed deployer nonce")
        artifact_path = REPOSITORY_ROOT / artifact["path"]
        if not artifact_path.is_file():
            raise DeploymentError(f"manifest {name} artifact path is absent: {artifact_path}")
        if _sha256(artifact_path) != artifact["artifact_sha256"]:
            raise DeploymentError(f"manifest {name} artifact SHA-256 does not match disk")
    if seen != required_contracts:
        raise DeploymentError("manifest is missing a required deployment artifact")
    nonces = [artifact["precomputed_create_nonce"] for artifact in artifacts]
    if len(set(nonces)) != len(nonces):
        raise DeploymentError("manifest deployment nonces are not unique")


def validate_offline_manifest(manifest: dict[str, Any]) -> None:
    """Validate every static deploy-consumer input without network side effects."""
    _require_type(manifest, ("revision",), str)
    _require_type(manifest, ("deployer", "address"), str)
    for path, expected in {
        ("network", "source_chain_id"): int,
        ("network", "execution_rpc_default"): str,
        ("network", "beacon_rpc_default"): str,
        ("network", "sp1_verifier_gateway", "address"): str,
        ("network", "sp1_verifier_gateway", "runtime_code_hash"): str,
        ("network", "token", "address"): str,
        ("network", "token", "runtime_code_hash"): str,
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
        _require_type(manifest, path, expected)
    _reject_arbitrum_scope(manifest)
    _validate_contract_artifacts(manifest)
    build_verifier_constructor_inputs(manifest)


def build_verifier_constructor_inputs(manifest: dict[str, Any]) -> tuple[Any, ...]:
    """Assemble the verifier constructor tuple from local manifest data only."""
    network = manifest["network"]
    programs = manifest["programs"]
    pftl = manifest["pftl"]
    return (
        network["sp1_verifier_gateway"]["address"],
        programs["egress"]["program_vkey"],
        pftl["chain_id_hash"],
        pftl["genesis_hash_commitment"],
        int(pftl["protocol_version"]),
        pftl["route_profile_hash_commitment"],
        int(pftl["route_epoch"]),
        pftl["asset_id_commitment"],
        ETHEREUM_SEPOLIA_CHAIN_ID,
        pftl["vault_runtime_code_hash"],
        network["token"]["address"],
        network["token"]["runtime_code_hash"],
        int(programs["max_proof_bytes"]),
        int(programs["max_public_values_bytes"]),
        pftl["initial_checkpoint_commitment"],
        int(pftl["initial_finalized_height"]),
        pftl["initial_committee_root_commitment"],
    )


class SepoliaDeployer:
    def __init__(
        self,
        arguments: argparse.Namespace,
        manifest_path: Path,
        manifest: dict[str, Any],
        prebroadcast_manifest_path: Path,
        prebroadcast_manifest_sha256: str,
    ) -> None:
        self.args = arguments
        self.manifest_path = manifest_path
        self.manifest = manifest
        self.prebroadcast_manifest_path = prebroadcast_manifest_path
        self.prebroadcast_manifest_sha256 = prebroadcast_manifest_sha256
        self.rpc = arguments.rpc or manifest["network"]["execution_rpc_default"]
        self.agent_call = None
        self.web3 = None
        self.now = datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")
        self._live_runtime_bytes: dict[str, bytes] = {}

    def initialize_network_clients(self) -> None:
        """Initialize network-capable dependencies only after the digest gate."""
        self.agent_call = self._load_agent_call(self.args.stakehub_repo.resolve())
        self.web3 = self._load_web3()

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
        except ImportError as error:
            raise DeploymentError("web3 is required for ABI encoding/readback") from error
        client = Web3(Web3.HTTPProvider(self.rpc, request_kwargs={"timeout": 30}))
        if not client.is_connected():
            raise DeploymentError(f"cannot reach Ethereum Sepolia RPC: {self.rpc}")
        if client.eth.chain_id != ETHEREUM_SEPOLIA_CHAIN_ID:
            raise DeploymentError(f"RPC chain id {client.eth.chain_id} is not Ethereum Sepolia")
        return client

    # -- artifact loading -----------------------------------------------------
    def _artifact(self, contract: str) -> dict[str, Any]:
        path = REPOSITORY_ROOT / f"crates/ethereum-contracts/out/{contract}.sol/{contract}.json"
        if not path.is_file():
            raise DeploymentError(f"foundry artifact missing: {path} (run forge build)")
        return _load_json(path)

    # -- agent calls -----------------------------------------------------------
    def _open_session(self, deploys: list[dict[str, Any]]) -> str:
        if self.agent_call is None:
            raise DeploymentError("agentd was not initialized")
        session_id = f"lane-c-eth-sepolia-{datetime.now(UTC).strftime('%Y%m%dT%H%M%SZ')}"
        response = self.agent_call({
            "op": "open_launch_session",
            "session_id": session_id,
            "chain_id": ETHEREUM_SEPOLIA_CHAIN_ID,
            "usdc_address": CANONICAL_USDC,
            "usdc_budget": 0,
            "allowlist": [CANONICAL_USDC, SP1_GATEWAY],
            "expected_deploys": deploys,
            "ttl_seconds": 3600,
        }, timeout=60.0)
        if not response or not response.get("ok"):
            raise DeploymentError(f"open_launch_session failed: {response}")
        return session_id

    def _deploy(self, session_id: str, action: str, bytecode: str, label: str) -> dict[str, Any]:
        if self.agent_call is None:
            raise DeploymentError("agentd was not initialized")
        response = self.agent_call({
            "op": "evm_contract_tx",
            "to": None,
            "data": bytecode,
            "rpc_url": self.rpc,
            "chain_id": ETHEREUM_SEPOLIA_CHAIN_ID,
            "session_id": session_id,
            "session_action": action,
            "label": label,
        }, timeout=1200.0)
        if not response or not response.get("ok"):
            raise DeploymentError(f"{label} deployment failed: {response}")
        return response

    def _close_session(self, session_id: str) -> None:
        if self.agent_call is None:
            raise DeploymentError("agentd was not initialized")
        self.agent_call({"op": "close_launch_session", "session_id": session_id}, timeout=30.0)

    # -- encoding ----------------------------------------------------------------
    def _constructor(self, contract: str, types: list[str], values: list[Any]) -> str:
        artifact = self._artifact(contract)
        creation = artifact["bytecode"]["object"]
        encoded = self.web3.codec.encode(types, values).hex()
        return "0x" + creation.removeprefix("0x") + encoded

    def _expected_deploy(self, action: str, contract: str, types: list[str], values: list[Any]) -> tuple[dict[str, Any], str]:
        artifact = self._artifact(contract)
        creation = bytes.fromhex(artifact["bytecode"]["object"].removeprefix("0x"))
        data = self._constructor(contract, types, values)
        return (
            {
                "label": action,
                "bytecode_hash": self.web3.to_hex(self.web3.keccak(creation)).lower(),
                "bytecode_len": len(creation),
            },
            data,
        )

    # -- readback -----------------------------------------------------------------
    def _runtime_code_hash(self, address: str) -> str:
        code = self.web3.eth.get_code(self.web3.to_checksum_address(address))
        if not code:
            raise DeploymentError(f"no runtime code at {address}")
        return self.web3.to_hex(self.web3.keccak(code))

    def _runtime_matches_unlinked(self, contract: str, live_hash: str) -> bool:
        """Zero the immutable slots in live code and compare byte-for-byte
        against the unlinked artifact deployed bytecode (dispatch #145)."""
        artifact = self._artifact(contract)
        unlinked = artifact["deployedBytecode"]["object"].removeprefix("0x")
        refs = artifact["deployedBytecode"].get("immutableReferences", {})
        live = self._live_runtime_bytes[contract]
        if len(live) != len(unlinked) // 2:
            return False
        immutable_offsets = set()
        for positions in refs.values():
            for position in positions:
                start = int(position["start"])
                immutable_offsets.update(range(start, start + int(position["length"])))
        unlinked_bytes = bytes.fromhex(unlinked)
        return all(
            index in immutable_offsets or live[index] == unlinked_bytes[index]
            for index in range(len(unlinked_bytes))
        )

    def _existing_deploy(self, nonce: int, expected_address: str, label: str) -> dict[str, Any]:
        """Resume mode: recover the already-mined deploy transaction for the
        deterministic CREATE nonce instead of deploying again."""
        deployer = self.web3.to_checksum_address(self.manifest["deployer"]["address"])

        def nonce_at(block: int) -> int:
            return self.web3.eth.get_transaction_count(deployer, block_identifier=block)

        latest = self.web3.eth.block_number
        if nonce_at(latest) <= nonce:
            raise DeploymentError(f"{label} nonce {nonce} not yet mined")
        low, high = 0, latest
        while low < high:
            mid = (low + high) // 2
            if nonce_at(mid) >= nonce + 1:
                high = mid
            else:
                low = mid + 1
        block = self.web3.eth.get_block(low, full_transactions=True)
        for tx in block.transactions:
            if tx["from"] == deployer and tx["nonce"] == nonce:
                receipt = self.web3.eth.get_transaction_receipt(tx["hash"])
                if receipt["status"] != 1:
                    raise DeploymentError(f"{label} deploy transaction reverted: {tx['hash'].hex()}")
                address = receipt["contractAddress"]
                if address.lower() != expected_address.lower():
                    raise DeploymentError(
                        f"{label} mined at {address}, expected {expected_address}"
                    )
                return {
                    "tx": tx["hash"].hex().lower(),
                    "contract_address": address.lower(),
                    "resumed_readback_only": True,
                }
        raise DeploymentError(f"{label} deploy transaction not found in block {low}")

    @staticmethod
    def _create_address(deployer: str, nonce: int) -> str:
        from eth_utils import keccak as _keccak
        import rlp

        return "0x" + _keccak(rlp.encode([bytes.fromhex(deployer.removeprefix("0x")), nonce]))[-20:].hex()

    # -- PFTL checkpoint (read-only, Lane A fleet pattern) -------------------------
    @staticmethod
    def _append_str_field(buf: bytearray, label: str, value: str) -> None:
        buf.extend(label.encode())
        buf.append(ord("="))
        buf.extend(str(len(value)).encode())
        buf.append(ord(":"))
        buf.extend(value.encode())
        buf.append(ord("\n"))

    @classmethod
    def _append_usize_field(cls, buf: bytearray, label: str, value: int) -> None:
        buf.extend(label.encode())
        buf.append(ord("="))
        buf.extend(str(value).encode())
        buf.append(ord("\n"))

    @classmethod
    def _committee_root(cls, validators: list[dict[str, str]]) -> str:
        # Mirrors ConsensusV2ValidatorSet::try_new committee_root.
        ordered = sorted((v["node_id"], v["public_key_hex"]) for v in validators)
        quorum = (2 * len(ordered) // 3) + 1
        buf = bytearray()
        cls._append_usize_field(buf, "validator_count", len(ordered))
        cls._append_usize_field(buf, "quorum", quorum)
        for node_id, public_key_hex in ordered:
            cls._append_str_field(buf, "validator_id", node_id)
            cls._append_str_field(buf, "public_key_hex", public_key_hex)
        digest = hashlib.sha3_384(COMMITTEE_ROOT_DOMAIN.encode() + b"\x00" + bytes(buf))
        return digest.hexdigest()

    def _fleet_checkpoint(self) -> dict[str, Any]:
        if not FLEET_INVENTORY.is_file():
            raise DeploymentError(f"fleet inventory missing: {FLEET_INVENTORY}")
        host = None
        for line in FLEET_INVENTORY.read_text().splitlines():
            if line.startswith("validator-0"):
                host = line.split()[1]
                break
        if not host:
            raise DeploymentError("validator-0 host missing from fleet inventory")
        ssh = ["ssh", "-o", "BatchMode=yes", "-o", "StrictHostKeyChecking=no",
               "-o", "ConnectTimeout=15", "-i", str(Path.home() / ".ssh/id_ed25519"),
               f"root@{host}"]
        binary = "/opt/postfiat/releases/a666-primary-fill-8ae940f/postfiat-node"
        checkpoint_raw = subprocess.run(
            ssh + [f"{binary} verify-finalized-checkpoint --data-dir /var/lib/postfiat/validator-0"],
            capture_output=True, text=True, timeout=60,
        )
        if checkpoint_raw.returncode != 0:
            raise DeploymentError(f"finalized checkpoint read failed: {checkpoint_raw.stderr[:200]}")
        checkpoint = json.loads(checkpoint_raw.stdout[checkpoint_raw.stdout.index("{"):])
        if not checkpoint.get("verified"):
            raise DeploymentError("fleet finalized checkpoint not verified")
        registry_raw = subprocess.run(
            ssh + ["cat /var/lib/postfiat/validator-0/validator_registry.json"],
            capture_output=True, text=True, timeout=60,
        )
        if registry_raw.returncode != 0:
            raise DeploymentError(f"validator registry read failed: {registry_raw.stderr[:200]}")
        registry = json.loads(registry_raw.stdout)
        committee_root = self._committee_root(registry["validators"])
        block_hash = checkpoint["checkpoint_block_hash"]
        checkpoint_commitment = self.web3.to_hex(self.web3.keccak(bytes.fromhex(block_hash)))
        committee_commitment = self.web3.to_hex(self.web3.keccak(bytes.fromhex(committee_root)))
        return {
            "height": int(checkpoint["checkpoint_height"]),
            "block_hash": block_hash,
            "checkpoint_commitment": checkpoint_commitment,
            "committee_root": committee_root,
            "committee_root_commitment": committee_commitment,
            "committee_epoch": int(checkpoint["committee_epoch"]),
            "validator_count": int(checkpoint["validator_count"]),
        }

    # -- main ----------------------------------------------------------------------
    def run(self) -> None:
        programs = self.manifest["programs"]
        network = self.manifest["network"]
        pftl = self.manifest.get("pftl", {})
        owner = self.manifest["deployer"]["address"]

        checkpoint = self._fleet_checkpoint()
        pftl["initial_checkpoint_commitment"] = checkpoint["checkpoint_commitment"]
        pftl["initial_finalized_height"] = checkpoint["height"]
        pftl["initial_committee_root_commitment"] = checkpoint["committee_root_commitment"]

        verifier_action = "deploy-pftl-finality-verifier-v1"
        vault_action = "deploy-erc20-bridge-vault-l1"

        verifier_types = [
            "(address,bytes32,bytes32,bytes32,uint32,bytes32,uint64,bytes32,uint64,bytes32,address,bytes32,uint256,uint256,bytes32,uint64,bytes32)",
        ]
        verifier_values = list(build_verifier_constructor_inputs(self.manifest))
        verifier_values[0] = self.web3.to_checksum_address(verifier_values[0])
        verifier_values[10] = self.web3.to_checksum_address(verifier_values[10])
        verifier_config = tuple(verifier_values)

        # Fail closed when the deployer nonce drifted from the deterministic
        # CREATE plan recorded in the manifest (nonces 61/62).
        deployer_address = self.web3.to_checksum_address(owner)
        live_nonce = self.web3.eth.get_transaction_count(deployer_address)
        planned = {
            artifact["contract"]: artifact.get("precomputed_create_nonce")
            for artifact in self.manifest["contracts"]["artifacts"]
            if artifact.get("precomputed_create_nonce") is not None
        }
        verifier_nonce = planned.get("PFTLFinalityVerifierV1")
        vault_nonce = planned.get("ERC20BridgeVaultL1")
        if verifier_nonce is None or vault_nonce is None:
            raise DeploymentError("manifest missing precomputed deployer nonces")
        if self.args.resume:
            if live_nonce <= vault_nonce:
                raise DeploymentError(
                    f"deployer nonce {live_nonce} shows planned deploys not yet mined"
                )
        elif live_nonce != verifier_nonce:
            raise DeploymentError(
                f"deployer nonce {live_nonce} != planned verifier nonce {verifier_nonce}; "
                "refusing to drift from deterministic addresses"
            )
        expected_verifier = self._create_address(deployer_address, verifier_nonce)
        expected_vault = self._create_address(deployer_address, vault_nonce)
        manifest_verifier = next(
            a["address"] for a in self.manifest["contracts"]["artifacts"]
            if a["contract"] == "PFTLFinalityVerifierV1"
        )
        manifest_vault = next(
            a["address"] for a in self.manifest["contracts"]["artifacts"]
            if a["contract"] == "ERC20BridgeVaultL1"
        )
        if expected_verifier.lower() != manifest_verifier.lower():
            raise DeploymentError("predicted verifier address differs from manifest")
        if expected_vault.lower() != manifest_vault.lower():
            raise DeploymentError("predicted vault address differs from manifest")

        verifier_expected, verifier_data = self._expected_deploy(
            verifier_action, "PFTLFinalityVerifierV1", verifier_types, [verifier_config]
        )
        # Vault is deployed second; its constructor pins the verifier address.
        # The expected-deploy bytecode hash pins only the creation bytecode, so
        # the session entry is registered before the verifier address is known.
        vault_artifact = self._artifact("ERC20BridgeVaultL1")
        vault_creation = bytes.fromhex(vault_artifact["bytecode"]["object"].removeprefix("0x"))
        vault_expected = {
            "label": vault_action,
            "bytecode_hash": self.web3.to_hex(self.web3.keccak(vault_creation)).lower(),
            "bytecode_len": len(vault_creation),
        }

        deployed_txs: dict[str, str] = {}
        if not self.args.resume:
            session_id = self._open_session([verifier_expected, vault_expected])
        else:
            session_id = "resumed-readback-only"
        evidence = {
            "schema": "postfiat.pfusdc.eth_sepolia_deploy_evidence.v1",
            "session_id": session_id,
            "started_utc": self.now,
            "steps": [],
            "manifest_digests": {
                "pre_broadcast_input_path": str(self.prebroadcast_manifest_path),
                "pre_broadcast_input_sha256": self.prebroadcast_manifest_sha256,
            },
        }
        try:
            if self.args.resume:
                verifier = self._existing_deploy(
                    verifier_nonce, expected_verifier, "PFTLFinalityVerifierV1")
            else:
                verifier = self._deploy(session_id, verifier_action, verifier_data, "PFTLFinalityVerifierV1")
            verifier_address = verifier["contract_address"]
            if verifier_address.lower() != expected_verifier.lower():
                raise DeploymentError(
                    f"verifier deployed at {verifier_address}, expected {expected_verifier}"
                )
            evidence["steps"].append({"contract": "PFTLFinalityVerifierV1", **verifier})
            deployed_txs["PFTLFinalityVerifierV1"] = verifier["tx"]

            vault_types = ["address", "address", "bytes32", "address"]
            vault_values = [
                self.web3.to_checksum_address(network["token"]["address"]),
                self.web3.to_checksum_address(verifier_address),
                network["token"]["runtime_code_hash"],
                self.web3.to_checksum_address(owner),
            ]
            if self.args.resume:
                vault = self._existing_deploy(
                    vault_nonce, expected_vault, "ERC20BridgeVaultL1")
            else:
                _, vault_data = self._expected_deploy(vault_action, "ERC20BridgeVaultL1", vault_types, vault_values)
                vault = self._deploy(session_id, vault_action, vault_data, "ERC20BridgeVaultL1")
            vault_address = vault["contract_address"]
            if vault_address.lower() != expected_vault.lower():
                raise DeploymentError(
                    f"vault deployed at {vault_address}, expected {expected_vault}"
                )
            evidence["steps"].append({"contract": "ERC20BridgeVaultL1", **vault})
            deployed_txs["ERC20BridgeVaultL1"] = vault["tx"]

            # Readback: runtime code hash + constructor immutables + receipts.
            self._live_runtime_bytes = {
                "PFTLFinalityVerifierV1": bytes(
                    self.web3.eth.get_code(self.web3.to_checksum_address(verifier_address))
                ),
                "ERC20BridgeVaultL1": bytes(
                    self.web3.eth.get_code(self.web3.to_checksum_address(vault_address))
                ),
            }
            verifier_runtime = self._runtime_code_hash(verifier_address)
            vault_runtime = self._runtime_code_hash(vault_address)
            frozen = {
                a["contract"]: a
                for a in self.manifest["contracts"]["artifacts"]
            }
            # Dispatch #145: the frozen artifact hash is the *unlinked*
            # deployed bytecode hash (immutable slots zeroed by the compiler);
            # deployed runtime code embeds constructor immutables, so equality
            # is not expected. Fail closed only on a mismatch in the
            # non-immutable instruction region; record the live hash and
            # rebind the unregistered route profile to it.
            for contract_name, live_hash in [
                ("PFTLFinalityVerifierV1", verifier_runtime),
                ("ERC20BridgeVaultL1", vault_runtime),
            ]:
                frozen_hash = frozen[contract_name]["unlinked_deployed_bytecode_keccak256"].lower()
                evidence[f"{contract_name}_runtime_hash_matches_unlinked_artifact"] = (
                    live_hash.lower() == frozen_hash
                )
                if not self._runtime_matches_unlinked(contract_name, live_hash):
                    raise DeploymentError(
                        f"{contract_name} runtime code differs outside immutable slots"
                    )
            self._verify_verifier_immutables(verifier_address, verifier_config)
            self._verify_vault_immutables(vault_address, vault_values)
            verifier_receipt = self.web3.eth.get_transaction_receipt(verifier["tx"])
            vault_receipt = self.web3.eth.get_transaction_receipt(vault["tx"])
            evidence["readback"] = {
                "verifier_runtime_code_hash": verifier_runtime,
                "vault_runtime_code_hash": vault_runtime,
                "token": network["token"]["address"],
                "owner": owner,
                "verifier_block": int(verifier_receipt["blockNumber"]),
                "vault_block": int(vault_receipt["blockNumber"]),
                "verifier_gas_used": int(verifier_receipt["gasUsed"]),
                "vault_gas_used": int(vault_receipt["gasUsed"]),
                "constructor_immutables_verified": True,
                "replay_protection": "consumed-withdrawal/nullifier mappings present; negative replay covered by ERC20BridgeVaultL1Test::testWithdrawalReplayRevertsNegativeTest",
            }
        finally:
            if not self.args.resume:
                self._close_session(session_id)

        # Update the manifest with deployed addresses/tx hashes (Lane C path only).
        self._update_manifest(verifier_address, verifier, vault_address, vault, checkpoint, evidence)
        evidence["manifest_digests"]["postdeploy_enriched_manifest_sha256"] = _sha256(self.manifest_path)

        self.args.evidence_dir.mkdir(parents=True, exist_ok=True)
        out = self.args.evidence_dir / "deploy-result.json"
        out.write_text(json.dumps(evidence, indent=2) + "\n")
        print(json.dumps({"verifier": verifier_address, "vault": vault_address, "evidence": str(out)}, indent=2))

    # -- constructor immutable readback -------------------------------------------
    def _verify_verifier_immutables(self, address: str, config: tuple) -> None:
        artifact = self._artifact("PFTLFinalityVerifierV1")
        contract = self.web3.eth.contract(
            address=self.web3.to_checksum_address(address), abi=artifact["abi"]
        )
        expected = {
            "sp1Verifier": self.web3.to_checksum_address(config[0]),
            "programVKey": config[1],
            "pftlChainIdHash": config[2],
            "pftlGenesisHashCommitment": config[3],
            "pftlProtocolVersion": config[4],
            "routeProfileHashCommitment": config[5],
            "routeEpoch": config[6],
            "assetIdCommitment": config[7],
            "arbitrumChainId": config[8],
            "vaultRuntimeCodeHash": config[9],
            "token": self.web3.to_checksum_address(config[10]),
            "tokenRuntimeCodeHash": config[11],
            "maxProofBytes": config[12],
            "maxPublicValuesBytes": config[13],
            # Initial checkpoint values land in mutable latest* state, not
            # immutables; readback uses the storage getters.
            "latestCheckpointCommitment": config[14],
            "latestFinalizedHeight": config[15],
            "latestCommitteeRootCommitment": config[16],
        }
        for field, want in expected.items():
            got = getattr(contract.functions, field)().call()
            if isinstance(want, str) and want.startswith("0x") and len(want) == 66:
                got = self.web3.to_hex(got) if not isinstance(got, str) else got
                if got.lower() != want.lower():
                    raise DeploymentError(f"verifier immutable {field} readback drift: {got} != {want}")
            elif isinstance(want, str):
                if str(got).lower() != want.lower():
                    raise DeploymentError(f"verifier immutable {field} readback drift: {got} != {want}")
            else:
                if int(got) != int(want):
                    raise DeploymentError(f"verifier immutable {field} readback drift: {got} != {want}")

    def _verify_vault_immutables(self, address: str, values: list) -> None:
        artifact = self._artifact("ERC20BridgeVaultL1")
        contract = self.web3.eth.contract(
            address=self.web3.to_checksum_address(address), abi=artifact["abi"]
        )
        checks = {
            "token": self.web3.to_checksum_address(values[0]),
            "finalityVerifier": self.web3.to_checksum_address(values[1]),
            "tokenRuntimeCodeHash": values[2],
            "owner": self.web3.to_checksum_address(values[3]),
        }
        for field, want in checks.items():
            got = getattr(contract.functions, field)().call()
            if isinstance(got, bytes):
                got = self.web3.to_hex(got)
            if str(got).lower() != str(want).lower():
                raise DeploymentError(f"vault immutable {field} readback drift: {got} != {want}")

    # -- manifest update -----------------------------------------------------------
    def _update_manifest(self, verifier_address, verifier, vault_address, vault, checkpoint, evidence) -> None:
        for artifact in self.manifest["contracts"]["artifacts"]:
            if artifact["contract"] == "PFTLFinalityVerifierV1":
                artifact["address"] = verifier_address.lower()
                artifact["deploy_tx_hash"] = verifier["tx"]
                artifact["deploy_block"] = evidence["readback"]["verifier_block"]
                artifact["deploy_gas_used"] = evidence["readback"]["verifier_gas_used"]
                artifact["deployed_runtime_code_keccak256"] = evidence["readback"][
                    "verifier_runtime_code_hash"
                ]
            if artifact["contract"] == "ERC20BridgeVaultL1":
                artifact["address"] = vault_address.lower()
                artifact["deploy_tx_hash"] = vault["tx"]
                artifact["deploy_block"] = evidence["readback"]["vault_block"]
                artifact["deploy_gas_used"] = evidence["readback"]["vault_gas_used"]
                artifact["deployed_runtime_code_keccak256"] = evidence["readback"][
                    "vault_runtime_code_hash"
                ]
        self.manifest["pftl"]["initial_checkpoint_commitment"] = checkpoint["checkpoint_commitment"]
        self.manifest["pftl"]["initial_finalized_height"] = checkpoint["height"]
        self.manifest["pftl"]["initial_committee_root_commitment"] = checkpoint["committee_root_commitment"]
        self.manifest["pftl"]["checkpoint_block_hash"] = checkpoint["block_hash"]
        self.manifest["status"] = "deployed-not-activated"
        self.manifest["deployed_utc"] = datetime.now(UTC).strftime("%Y-%m-%dT%H:%M:%SZ")
        self.manifest_path.write_text(json.dumps(self.manifest, indent=2) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--manifest", type=Path, default=DEFAULT_MANIFEST)
    parser.add_argument("--stakehub-repo", type=Path, default=DEFAULT_STAKEHUB_REPO)
    parser.add_argument("--evidence-dir", type=Path, default=DEFAULT_EVIDENCE_DIR)
    parser.add_argument("--rpc", default=None)
    parser.add_argument("--check-only", action="store_true", help="validate manifest/scope/artifacts only")
    parser.add_argument(
        "--auditor-authorization",
        type=Path,
        help="required PASS auditor authorization JSON for any send-capable invocation",
    )
    parser.add_argument(
        "--resume",
        action="store_true",
        help="recover already-mined deterministic deploys via readback only; never sends a transaction",
    )
    arguments = parser.parse_args()
    try:
        manifest_path = arguments.manifest.resolve()
        manifest = _load_json(manifest_path)
        validate_offline_manifest(manifest)
        if arguments.check_only:
            print(json.dumps({
                "ok": True,
                "manifest_sha256": _sha256(manifest_path),
                "validation": "offline-only; no Web3, socket, agentd, credential, or sender initialized",
            }))
            return 0
        authorization = require_auditor_authorization(
            manifest_path,
            manifest,
            arguments.auditor_authorization,
            ETHEREUM_SEPOLIA_CHAIN_ID,
        )
        prebroadcast_path = retain_prebroadcast_manifest(
            manifest_path,
            arguments.evidence_dir,
            authorization["manifest_sha256"],
        )
        deployer = SepoliaDeployer(
            arguments,
            manifest_path,
            manifest,
            prebroadcast_path,
            authorization["manifest_sha256"],
        )
        deployer.initialize_network_clients()
        deployer.run()
        return 0
    except (AuthorizationError, DeploymentError) as error:
        print(f"DEPLOYMENT_BLOCKED: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
