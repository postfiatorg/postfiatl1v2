#!/usr/bin/env python3
"""Crash-resumable pfUSDC Tier-4 Sepolia deployment through StakeHub agentd.

The script never reads or accepts a private key.  It validates the frozen
manifest and Foundry artifacts, checks the live chains, asks the already-
unlocked StakeHub agent to sign the exact constructor transactions, and then
reads every committed value back from chain.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import tempfile
from dataclasses import dataclass
from datetime import UTC, datetime
from pathlib import Path
from typing import Any, Callable


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DEPLOYMENT_DIR = REPOSITORY_ROOT / "deployments/pfusdc-tier4-sepolia-20260718"
DEFAULT_STAKEHUB_REPO = REPOSITORY_ROOT.parent / "StakeHub"
DEFAULT_EVIDENCE_DIR = REPOSITORY_ROOT / "docs/evidence/pfusdc-tier4-deployment-live"
EXPECTED_MANIFEST_SHA256 = "efc94f6f426a89f6e8581af95e6f95e0138a312bf3b06ac7113134ffd0af3ada"
EXPECTED_INPUT_SHA256 = "7a507e956198c3f35f4ea1e22e68629ced5118866237e51fa9fd0ca57ddd5bc9"
DEFAULT_ETHEREUM_RPC = "https://ethereum-sepolia-rpc.publicnode.com"
DEFAULT_ARBITRUM_RPC = "https://arbitrum-sepolia-rpc.publicnode.com"


class DeploymentError(RuntimeError):
    """A fail-closed manifest, chain, policy, or readback failure."""


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


def _hex(value: Any) -> str:
    if isinstance(value, str):
        raw = value.removeprefix("0x")
    else:
        raw = bytes(value).hex()
    return "0x" + raw.lower()


def _normalize_expected(value: Any) -> Any:
    if isinstance(value, str) and value.startswith("0x"):
        return value.lower()
    return value


def _normalize_actual(value: Any) -> Any:
    if isinstance(value, (bytes, bytearray)):
        return _hex(value)
    if isinstance(value, str) and value.startswith("0x"):
        return value.lower()
    return value


def _assert_equal(label: str, actual: Any, expected: Any) -> None:
    normalized_actual = _normalize_actual(actual)
    normalized_expected = _normalize_expected(expected)
    if normalized_actual != normalized_expected:
        raise DeploymentError(
            f"{label} mismatch: got {normalized_actual!r}, expected {normalized_expected!r}"
        )


def _utc_now() -> str:
    return datetime.now(UTC).replace(microsecond=0).isoformat().replace("+00:00", "Z")


def _atomic_write_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, temporary_name = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
    temporary = Path(temporary_name)
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump(value, handle, indent=2, sort_keys=True)
            handle.write("\n")
            handle.flush()
            os.fsync(handle.fileno())
        os.replace(temporary, path)
        directory_fd = os.open(path.parent, os.O_RDONLY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    finally:
        if temporary.exists():
            temporary.unlink()


def _contract_address(deployer: str, nonce: int, web3_type: Any) -> str:
    if nonce < 0 or nonce > 127:
        raise DeploymentError("deployment nonce outside the supported frozen range")
    address = bytes.fromhex(deployer.removeprefix("0x"))
    if len(address) != 20:
        raise DeploymentError("deployer must be a 20-byte address")
    encoded_nonce = b"\x80" if nonce == 0 else bytes([nonce])
    encoded = bytes([0xD6]) + bytes([0x94]) + address + encoded_nonce
    return web3_type.to_checksum_address(web3_type.keccak(encoded)[-20:])


@dataclass(frozen=True)
class ContractPlan:
    key: str
    action: str
    address: str
    nonce: int
    artifact: dict[str, Any]
    artifact_manifest: dict[str, Any]
    constructor_args: tuple[Any, ...]
    runtime_hash: str
    readback: Callable[[Any], None]

    @property
    def base_bytecode(self) -> bytes:
        return bytes.fromhex(self.artifact["bytecode"]["object"].removeprefix("0x"))

    @property
    def base_bytecode_hash(self) -> str:
        from web3 import Web3

        return _hex(Web3.keccak(self.base_bytecode))

    @property
    def init_code(self) -> str:
        from web3 import Web3

        contract = Web3().eth.contract(
            abi=self.artifact["abi"], bytecode=self.artifact["bytecode"]["object"]
        )
        return contract.constructor(*self.constructor_args).data_in_transaction


class DeploymentDriver:
    def __init__(self, arguments: argparse.Namespace) -> None:
        try:
            from web3 import Web3
        except ImportError as error:
            raise DeploymentError(
                "web3 is unavailable; run with the StakeHub virtualenv Python"
            ) from error

        self.Web3 = Web3
        self.arguments = arguments
        self.deployment_dir = arguments.deployment_dir.resolve()
        self.manifest_path = self.deployment_dir / "manifest.json"
        self.input_path = self.deployment_dir / "input.json"
        self.state_path = arguments.evidence_dir.resolve() / "state.json"
        self.manifest = self._load_and_validate_manifest()
        self.deployer = Web3.to_checksum_address(self.manifest["deployer"])
        self.ethereum = Web3(
            Web3.HTTPProvider(arguments.ethereum_rpc, request_kwargs={"timeout": 30})
        )
        self.arbitrum = Web3(
            Web3.HTTPProvider(arguments.arbitrum_rpc, request_kwargs={"timeout": 30})
        )
        self.agent_call = self._load_agent_call(arguments.stakehub_repo.resolve())
        self.artifacts = self._load_artifacts()
        self.plans = self._build_plans()

    def _load_and_validate_manifest(self) -> dict[str, Any]:
        if _sha256(self.manifest_path) != EXPECTED_MANIFEST_SHA256:
            raise DeploymentError("frozen manifest SHA-256 mismatch")
        if _sha256(self.input_path) != EXPECTED_INPUT_SHA256:
            raise DeploymentError("frozen deployment input SHA-256 mismatch")
        manifest = _load_json(self.manifest_path)
        _assert_equal(
            "manifest schema", manifest.get("schema"), "postfiat.pfusdc.tier4_deployment_manifest.v1"
        )
        return manifest

    @staticmethod
    def _load_agent_call(stakehub_repo: Path) -> Callable[..., dict[str, Any] | None]:
        package = stakehub_repo / "stakehub/agentd.py"
        if not package.is_file():
            raise DeploymentError(f"StakeHub agentd not found under {stakehub_repo}")
        sys.path.insert(0, str(stakehub_repo))
        from stakehub.agentd import call

        return call

    def _load_artifacts(self) -> dict[str, tuple[dict[str, Any], dict[str, Any]]]:
        artifacts: dict[str, tuple[dict[str, Any], dict[str, Any]]] = {}
        for frozen in self.manifest["contracts"]["artifacts"]:
            path = REPOSITORY_ROOT / frozen["path"]
            if _sha256(path) != frozen["artifact_sha256"]:
                raise DeploymentError(f"artifact SHA-256 mismatch: {frozen['contract']}")
            artifact = _load_json(path)
            settings = artifact.get("metadata", {}).get("settings", {})
            optimizer = settings.get("optimizer", {})
            _assert_equal(f"{frozen['contract']} optimizer enabled", optimizer.get("enabled"), True)
            _assert_equal(
                f"{frozen['contract']} optimizer runs",
                optimizer.get("runs"),
                frozen["optimizer_runs"],
            )
            _assert_equal(
                f"{frozen['contract']} EVM version", settings.get("evmVersion"), frozen["evm_version"]
            )
            _assert_equal(
                f"{frozen['contract']} metadata bytecode hash",
                settings.get("metadata", {}).get("bytecodeHash"),
                frozen["metadata_bytecode_hash"],
            )
            _assert_equal(
                f"{frozen['contract']} compiler version",
                artifact.get("metadata", {}).get("compiler", {}).get("version"),
                frozen["compiler"],
            )
            base = bytes.fromhex(artifact["bytecode"]["object"].removeprefix("0x"))
            _assert_equal(
                f"{frozen['contract']} creation bytecode hash",
                _hex(self.Web3.keccak(base)),
                frozen["creation_bytecode_keccak256"],
            )
            artifacts[frozen["contract"]] = (artifact, frozen)
        return artifacts

    def _artifact(self, name: str) -> tuple[dict[str, Any], dict[str, Any]]:
        try:
            return self.artifacts[name]
        except KeyError as error:
            raise DeploymentError(f"missing frozen contract artifact: {name}") from error

    def _build_plans(self) -> dict[str, list[ContractPlan]]:
        constructors = self.manifest["contracts"]["constructors"]
        configuration = self.manifest["contracts"]["configuration"]
        sequences = self.manifest["deployment_sequence"]
        address = self.Web3.to_checksum_address

        finality = constructors["finality_verifier"]
        finality_args = (
            (
                address(finality["sp1_verifier"]),
                bytes.fromhex(finality["program_vkey"].removeprefix("0x")),
                bytes.fromhex(finality["pftl_chain_id_hash"].removeprefix("0x")),
                bytes.fromhex(finality["pftl_genesis_hash_commitment"].removeprefix("0x")),
                finality["pftl_protocol_version"],
                bytes.fromhex(finality["route_profile_hash_commitment"].removeprefix("0x")),
                finality["route_epoch"],
                bytes.fromhex(finality["asset_id_commitment"].removeprefix("0x")),
                finality["arbitrum_chain_id"],
                bytes.fromhex(finality["vault_runtime_code_hash"].removeprefix("0x")),
                address(finality["token"]),
                bytes.fromhex(finality["token_runtime_code_hash"].removeprefix("0x")),
                finality["max_proof_bytes"],
                finality["max_public_values_bytes"],
                bytes.fromhex(finality["initial_checkpoint_commitment"].removeprefix("0x")),
                finality["initial_finalized_height"],
                bytes.fromhex(
                    finality["initial_committee_root_commitment"].removeprefix("0x")
                ),
            ),
        )
        finality_artifact, finality_frozen = self._artifact("PFTLFinalityVerifierV1")

        vault = constructors["vault"]
        vault_args = (
            address(vault["token"]),
            address(vault["finality_verifier"]),
            bytes.fromhex(vault["token_runtime_code_hash"].removeprefix("0x")),
            address(vault["arb_sys"]),
            address(vault["ingress_anchor"]),
            address(vault["initial_owner"]),
        )
        vault_artifact, vault_frozen = self._artifact("ERC20BridgeVaultV2")

        anchor = constructors["ingress_anchor"]
        anchor_args = (
            address(anchor["bridge"]),
            address(anchor["l2_vault"]),
            address(anchor["l2_token"]),
            anchor["l2_chain_id"],
            bytes.fromhex(anchor["governed_route_binding"].removeprefix("0x")),
        )
        anchor_artifact, anchor_frozen = self._artifact("PfUsdcIngressAnchorV1")

        def sequence(chain: str, contract: str) -> dict[str, Any]:
            return next(item for item in sequences[chain] if item["contract"] == contract)

        finality_sequence = sequence("arbitrum", "PFTLFinalityVerifierV1")
        vault_sequence = sequence("arbitrum", "ERC20BridgeVaultV2")
        anchor_sequence = sequence("ethereum", "PfUsdcIngressAnchorV1")

        plans = {
            "arbitrum": [
                ContractPlan(
                    "finality_verifier",
                    "deploy_finality_verifier",
                    self.Web3.to_checksum_address(finality_sequence["address"]),
                    finality_sequence["nonce"],
                    finality_artifact,
                    finality_frozen,
                    finality_args,
                    configuration["finality_verifier_runtime_code_hash"],
                    lambda contract: self._readback_finality(contract, finality),
                ),
                ContractPlan(
                    "vault",
                    "deploy_vault",
                    self.Web3.to_checksum_address(vault_sequence["address"]),
                    vault_sequence["nonce"],
                    vault_artifact,
                    vault_frozen,
                    vault_args,
                    configuration["vault_runtime_code_hash"],
                    lambda contract: self._readback_vault(contract, vault),
                ),
            ],
            "ethereum": [
                ContractPlan(
                    "ingress_anchor",
                    "deploy_ingress_anchor",
                    self.Web3.to_checksum_address(anchor_sequence["address"]),
                    anchor_sequence["nonce"],
                    anchor_artifact,
                    anchor_frozen,
                    anchor_args,
                    configuration["ingress_anchor_runtime_code_hash"],
                    lambda contract: self._readback_anchor(contract, anchor),
                )
            ],
        }

        for chain_plans in plans.values():
            for plan in chain_plans:
                derived = _contract_address(self.deployer, plan.nonce, self.Web3)
                _assert_equal(f"{plan.key} CREATE address", derived, plan.address)
                if not bytes.fromhex(plan.init_code.removeprefix("0x")).startswith(plan.base_bytecode):
                    raise DeploymentError(f"{plan.key} constructor encoding lost bytecode prefix")
        return plans

    @staticmethod
    def _call_getter(contract: Any, name: str, expected: Any, *arguments: Any) -> None:
        actual = getattr(contract.functions, name)(*arguments).call()
        _assert_equal(name, actual, expected)

    def _readback_finality(self, contract: Any, expected: dict[str, Any]) -> None:
        getters = {
            "sp1Verifier": "sp1_verifier",
            "programVKey": "program_vkey",
            "pftlChainIdHash": "pftl_chain_id_hash",
            "pftlGenesisHashCommitment": "pftl_genesis_hash_commitment",
            "pftlProtocolVersion": "pftl_protocol_version",
            "routeProfileHashCommitment": "route_profile_hash_commitment",
            "routeEpoch": "route_epoch",
            "assetIdCommitment": "asset_id_commitment",
            "arbitrumChainId": "arbitrum_chain_id",
            "vaultRuntimeCodeHash": "vault_runtime_code_hash",
            "token": "token",
            "tokenRuntimeCodeHash": "token_runtime_code_hash",
            "maxProofBytes": "max_proof_bytes",
            "maxPublicValuesBytes": "max_public_values_bytes",
            "latestCheckpointCommitment": "initial_checkpoint_commitment",
            "latestFinalizedHeight": "initial_finalized_height",
            "latestCommitteeRootCommitment": "initial_committee_root_commitment",
        }
        for getter, key in getters.items():
            self._call_getter(contract, getter, expected[key])
        checkpoint = bytes.fromhex(expected["initial_checkpoint_commitment"].removeprefix("0x"))
        self._call_getter(contract, "acceptedCheckpointCommitment", True, checkpoint)
        self._call_getter(
            contract,
            "checkpointCommitteeRootCommitment",
            expected["initial_committee_root_commitment"],
            checkpoint,
        )

    def _readback_vault(self, contract: Any, expected: dict[str, Any]) -> None:
        getters = {
            "token": "token",
            "finalityVerifier": "finality_verifier",
            "tokenRuntimeCodeHash": "token_runtime_code_hash",
            "arbSys": "arb_sys",
            "ingressAnchor": "ingress_anchor",
            "owner": "initial_owner",
        }
        for getter, key in getters.items():
            self._call_getter(contract, getter, expected[key])
        self._call_getter(contract, "paused", False)

    def _readback_anchor(self, contract: Any, expected: dict[str, Any]) -> None:
        getters = {
            "bridge": "bridge",
            "l2Vault": "l2_vault",
            "l2Token": "l2_token",
            "l2ChainId": "l2_chain_id",
            "governedRouteBinding": "governed_route_binding",
        }
        for getter, key in getters.items():
            self._call_getter(contract, getter, expected[key])

    def _require_rpc(self, chain: str, client: Any, expected_chain_id: int) -> None:
        if not client.is_connected():
            raise DeploymentError(f"{chain} RPC is not connected")
        _assert_equal(f"{chain} chain ID", int(client.eth.chain_id), expected_chain_id)

    def _verify_code_hash(self, client: Any, label: str, address: str, expected: str) -> None:
        code = bytes(client.eth.get_code(self.Web3.to_checksum_address(address)))
        if not code:
            raise DeploymentError(f"{label} has no live code")
        _assert_equal(f"{label} runtime code hash", _hex(self.Web3.keccak(code)), expected)

    def _verify_system_contracts(self) -> None:
        network = self.manifest["network"]
        self._verify_code_hash(
            self.ethereum,
            "Ethereum Sepolia Arbitrum bridge",
            network["ethereum_arbitrum_bridge"],
            network["ethereum_arbitrum_bridge_runtime_code_hash"],
        )
        self._verify_code_hash(
            self.ethereum,
            "Ethereum Sepolia Arbitrum rollup",
            network["arbitrum_rollup"],
            network["arbitrum_rollup_runtime_code_hash"],
        )
        self._verify_code_hash(
            self.arbitrum,
            "Arbitrum Sepolia canonical USDC",
            network["arbitrum_token"],
            network["arbitrum_token_runtime_code_hash"],
        )
        self._verify_code_hash(
            self.arbitrum,
            "Arbitrum Sepolia SP1 verifier",
            network["arbitrum_sp1_verifier"],
            network["arbitrum_sp1_verifier_runtime_code_hash"],
        )
        self._verify_code_hash(
            self.arbitrum,
            "Arbitrum Sepolia ArbSys",
            network["arbitrum_arb_sys"],
            network["arbitrum_arb_sys_runtime_code_hash"],
        )

    def _verify_deployed(self, client: Any, plan: ContractPlan) -> None:
        code = bytes(client.eth.get_code(plan.address))
        if not code:
            raise DeploymentError(f"{plan.key} is not deployed")
        _assert_equal(
            f"{plan.key} runtime code hash", _hex(self.Web3.keccak(code)), plan.runtime_hash
        )
        contract = client.eth.contract(address=plan.address, abi=plan.artifact["abi"])
        plan.readback(contract)

    def _chain_status(self, chain: str, client: Any) -> dict[str, Any]:
        chain_plans = self.plans[chain]
        deployed: list[str] = []
        saw_empty = False
        for plan in chain_plans:
            has_code = bool(bytes(client.eth.get_code(plan.address)))
            if has_code:
                if saw_empty:
                    raise DeploymentError(f"{chain} deployment sequence has a gap before {plan.key}")
                self._verify_deployed(client, plan)
                deployed.append(plan.key)
            else:
                saw_empty = True
        expected_nonce = len(deployed)
        latest_nonce = int(client.eth.get_transaction_count(self.deployer, "latest"))
        pending_nonce = int(client.eth.get_transaction_count(self.deployer, "pending"))
        _assert_equal(f"{chain} latest deployer nonce", latest_nonce, expected_nonce)
        if pending_nonce != latest_nonce:
            raise DeploymentError(
                f"{chain} has an unresolved pending deployer transaction: latest nonce "
                f"{latest_nonce}, pending nonce {pending_nonce}"
            )
        return {
            "chain_id": int(client.eth.chain_id),
            "balance_wei": int(client.eth.get_balance(self.deployer)),
            "deployer_nonce": latest_nonce,
            "deployed": deployed,
            "remaining": [plan.key for plan in chain_plans[len(deployed) :]],
        }

    def preflight(self) -> dict[str, Any]:
        network = self.manifest["network"]
        self._require_rpc(
            "ethereum", self.ethereum, int(network["ethereum_chain_id"])
        )
        self._require_rpc(
            "arbitrum", self.arbitrum, int(network["arbitrum_chain_id"])
        )
        self._verify_system_contracts()
        status = self.agent_call({"op": "status"})
        if not status or not status.get("ok"):
            raise DeploymentError("StakeHub agent is unavailable")
        ethereum = self._chain_status("ethereum", self.ethereum)
        arbitrum = self._chain_status("arbitrum", self.arbitrum)
        contract_plans = {
            plan.key: {
                "address": plan.address,
                "nonce": plan.nonce,
                "base_bytecode_hash": plan.base_bytecode_hash,
                "base_bytecode_len": len(plan.base_bytecode),
                "init_code_hash": _hex(
                    self.Web3.keccak(bytes.fromhex(plan.init_code.removeprefix("0x")))
                ),
                "init_code_len": len(bytes.fromhex(plan.init_code.removeprefix("0x"))),
                "runtime_code_hash": plan.runtime_hash,
            }
            for plans in self.plans.values()
            for plan in plans
        }
        return {
            "manifest_sha256": EXPECTED_MANIFEST_SHA256,
            "deployer": self.deployer,
            "agent_unlocked": bool(status.get("unlocked")),
            "spent_today_usd": status.get("spent_today_usd"),
            "system_contracts_verified": True,
            "contract_plans": contract_plans,
            "ethereum": ethereum,
            "arbitrum": arbitrum,
            "ready_to_deploy_ethereum": bool(status.get("unlocked"))
            and bool(ethereum["balance_wei"])
            and bool(ethereum["remaining"]),
            "ready_to_deploy_arbitrum": bool(status.get("unlocked"))
            and bool(arbitrum["balance_wei"])
            and bool(arbitrum["remaining"]),
        }

    def _record(self, event: dict[str, Any]) -> None:
        if self.state_path.exists():
            state = _load_json(self.state_path)
            _assert_equal("deployment state schema", state.get("schema"), "pfusdc-tier4-deploy.v1")
            _assert_equal(
                "deployment state manifest",
                state.get("manifest_sha256"),
                EXPECTED_MANIFEST_SHA256,
            )
        else:
            state = {
                "schema": "pfusdc-tier4-deploy.v1",
                "manifest_sha256": EXPECTED_MANIFEST_SHA256,
                "deployer": self.deployer,
                "events": [],
            }
        state["events"].append({"at": _utc_now(), **event})
        _atomic_write_json(self.state_path, state)

    def _agent_request(self, request: dict[str, Any], event: dict[str, Any]) -> dict[str, Any]:
        self._record({"phase": "prepared", **event})
        response = self.agent_call(request, timeout=1200.0)
        if not response:
            raise DeploymentError("StakeHub agent did not respond")
        if not response.get("ok"):
            raise DeploymentError(f"StakeHub agent refused {event['action']}: {response.get('error')}")
        result_event = {"phase": "accepted", **event}
        for key in ("tx", "contract_address", "gas_used", "charged_usd", "closed"):
            if key in response:
                result_event[key] = response[key]
        self._record(result_event)
        return response

    def deploy(self, chain: str) -> dict[str, Any]:
        before = self.preflight()
        chain_status = before[chain]
        if not before["agent_unlocked"]:
            raise DeploymentError("StakeHub agent is locked")
        if not chain_status["remaining"]:
            return {"chain": chain, "already_complete": True, "status": chain_status}
        if not chain_status["balance_wei"]:
            raise DeploymentError(f"{chain} deployer has zero gas balance")

        client = self.ethereum if chain == "ethereum" else self.arbitrum
        missing = [plan for plan in self.plans[chain] if plan.key in chain_status["remaining"]]
        session_id = f"{self.manifest['deployment_id']}-{chain}"

        close = self.agent_call({"op": "close_launch_session", "session_id": session_id})
        if not close or not close.get("ok"):
            raise DeploymentError(
                "another StakeHub launch session is active; refusing to disturb it"
            )
        self._record(
            {
                "phase": "session_recovery",
                "action": "close_prior_session",
                "chain": chain,
                "closed": bool(close.get("closed")),
            }
        )

        network = self.manifest["network"]
        allowlist = [self.deployer]
        if chain == "arbitrum":
            allowlist.extend(
                [
                    network["arbitrum_token"],
                    network["arbitrum_sp1_verifier"],
                    network["arbitrum_arb_sys"],
                ]
            )
            usdc_address = network["arbitrum_token"]
        else:
            allowlist.extend(
                [network["ethereum_arbitrum_bridge"], network["arbitrum_rollup"]]
            )
            usdc_address = self.deployer

        self._agent_request(
            {
                "op": "open_launch_session",
                "session_id": session_id,
                "chain_id": int(client.eth.chain_id),
                "allowlist": allowlist,
                "usdc_address": usdc_address,
                "usdc_budget": 0,
                "expected_deploys": [
                    {
                        "label": plan.action,
                        "bytecode_hash": plan.base_bytecode_hash,
                        "bytecode_len": len(plan.base_bytecode),
                    }
                    for plan in missing
                ],
                "close_after_action": missing[-1].action,
                "ttl_seconds": 1800,
            },
            {"action": "open_launch_session", "chain": chain, "session_id": session_id},
        )

        transactions: list[dict[str, Any]] = []
        for plan in missing:
            current_nonce = int(client.eth.get_transaction_count(self.deployer, "latest"))
            _assert_equal(f"{chain} nonce before {plan.key}", current_nonce, plan.nonce)
            if bytes(client.eth.get_code(plan.address)):
                self._verify_deployed(client, plan)
                continue
            response = self._agent_request(
                {
                    "op": "evm_contract_tx",
                    "to": None,
                    "data": plan.init_code,
                    "rpc_url": self.arguments.ethereum_rpc
                    if chain == "ethereum"
                    else self.arguments.arbitrum_rpc,
                    "chain_id": int(client.eth.chain_id),
                    "label": f"pfUSDC Tier-4 {plan.key} deploy",
                    "session_id": session_id,
                    "session_action": plan.action,
                    "value_wei": 0,
                    "gas_usd": 0,
                },
                {
                    "action": plan.action,
                    "chain": chain,
                    "expected_address": plan.address,
                    "expected_nonce": plan.nonce,
                    "base_bytecode_hash": plan.base_bytecode_hash,
                },
            )
            _assert_equal(f"{plan.key} deployed address", response.get("contract_address"), plan.address)
            self._verify_deployed(client, plan)
            transactions.append(
                {
                    "contract": plan.key,
                    "address": plan.address,
                    "tx": response["tx"],
                    "gas_used": response["gas_used"],
                }
            )

        after = self._chain_status(chain, client)
        if after["remaining"]:
            raise DeploymentError(f"{chain} deployment ended with missing contracts")
        return {"chain": chain, "transactions": transactions, "status": after}


def _parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "action", choices=("preflight", "readback", "deploy-arbitrum", "deploy-ethereum")
    )
    parser.add_argument("--deployment-dir", type=Path, default=DEFAULT_DEPLOYMENT_DIR)
    parser.add_argument("--stakehub-repo", type=Path, default=DEFAULT_STAKEHUB_REPO)
    parser.add_argument("--evidence-dir", type=Path, default=DEFAULT_EVIDENCE_DIR)
    parser.add_argument(
        "--ethereum-rpc",
        default=os.environ.get("ETHEREUM_SEPOLIA_RPC_URL", DEFAULT_ETHEREUM_RPC),
    )
    parser.add_argument(
        "--arbitrum-rpc",
        default=os.environ.get("ARBITRUM_SEPOLIA_RPC_URL", DEFAULT_ARBITRUM_RPC),
    )
    return parser.parse_args()


def main() -> int:
    arguments = _parse_arguments()
    try:
        driver = DeploymentDriver(arguments)
        if arguments.action in ("preflight", "readback"):
            result = driver.preflight()
        elif arguments.action == "deploy-arbitrum":
            result = driver.deploy("arbitrum")
        else:
            result = driver.deploy("ethereum")
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except Exception as error:  # noqa: BLE001 - CLI must return one bounded error.
        message = str(error)
        for secret in (arguments.ethereum_rpc, arguments.arbitrum_rpc):
            if secret:
                message = message.replace(secret, "<redacted-rpc>")
        print(json.dumps({"ok": False, "error": f"{type(error).__name__}: {message}"}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
