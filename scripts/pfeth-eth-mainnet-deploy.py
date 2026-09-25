#!/usr/bin/env python3
"""Preflight or deploy the digest-bound pfETH Ethereum-mainnet package.

The default mode is read-only. Live execution additionally requires an external
review gate bound to the exact manifest, an exact approval phrase, live chain
and nonce checks, and the unlocked StakeHub agent launch session. The script
never reads an EVM private key.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import socket
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from Crypto.Hash import keccak

ROOT = Path(__file__).resolve().parents[1]
DEFAULT_PACKAGE = ROOT / "deployments/pfeth-eth-mainnet-20260904"
DEFAULT_RPC = "https://ethereum-rpc.publicnode.com"
DEFAULT_AGENT_SOCKET = (
    Path(os.environ.get("STAKEHUB_HOME", "~/.stakehub")).expanduser() / "agent.sock"
)
CHAIN_ID = 1
REQUIRED_CONTRACTS = {
    "ExitExecutorV1",
    "WETHBridgeVaultL1",
    "PFTLFinalityVerifierV1",
}
REQUIRED_PROGRAMS = {"pfeth-eth-mainnet-ingress", "pfusdc-egress"}
CHECKPOINT_MAX_AGE_SECONDS = 900
CHECKPOINT_MAX_FUTURE_SKEW_SECONDS = 60


class DeploymentError(RuntimeError):
    pass


def read_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise DeploymentError(f"cannot read {path}: {error}") from error
    if not isinstance(value, dict):
        raise DeploymentError(f"JSON object required: {path}")
    return value


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def k256(data: bytes) -> str:
    digest = keccak.new(digest_bits=256)
    digest.update(data)
    return "0x" + digest.hexdigest()


def read_hex(path: Path) -> bytes:
    try:
        text = path.read_text(encoding="ascii").strip()
        return bytes.fromhex(text.removeprefix("0x"))
    except (OSError, UnicodeDecodeError, ValueError) as error:
        raise DeploymentError(f"invalid hex file {path}: {error}") from error


def write_private_json(path: Path, value: dict[str, Any]) -> None:
    path.parent.mkdir(mode=0o700, parents=True, exist_ok=True)
    payload = (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()
    temporary = path.with_suffix(path.suffix + ".tmp")
    descriptor = os.open(temporary, os.O_WRONLY | os.O_CREAT | os.O_TRUNC, 0o600)
    with os.fdopen(descriptor, "wb") as handle:
        handle.write(payload)
        handle.flush()
        os.fsync(handle.fileno())
    os.replace(temporary, path)


def rpc_call(url: str, method: str, params: list[Any]) -> Any:
    payload = json.dumps(
        {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
    ).encode()
    request = urllib.request.Request(
        url,
        data=payload,
        headers={"Content-Type": "application/json", "User-Agent": "PostFiat-pfETH/1"},
    )
    try:
        with urllib.request.urlopen(request, timeout=45) as response:
            value = json.load(response)
    except OSError as error:
        raise DeploymentError(f"{method} RPC failed: {error}") from error
    if value.get("error") is not None:
        raise DeploymentError(f"{method} RPC error: {value['error']}")
    if "result" not in value:
        raise DeploymentError(f"{method} RPC result missing")
    return value["result"]


def agent_call(socket_path: Path, request: dict[str, Any]) -> dict[str, Any]:
    client = socket.socket(socket.AF_UNIX)
    client.settimeout(600)
    try:
        client.connect(str(socket_path))
        client.sendall((json.dumps(request) + "\n").encode())
        payload = b""
        while not payload.endswith(b"\n"):
            chunk = client.recv(65536)
            if not chunk:
                break
            payload += chunk
    except OSError as error:
        raise DeploymentError(f"StakeHub agent unavailable: {error}") from error
    finally:
        client.close()
    try:
        result = json.loads(payload)
    except json.JSONDecodeError as error:
        raise DeploymentError("StakeHub agent returned invalid JSON") from error
    if not result.get("ok"):
        raise DeploymentError(
            f"StakeHub agent {request.get('op')} failed: {result.get('error')}"
        )
    return result


def validate_review(review: dict[str, Any], manifest_digest: str) -> None:
    if review.get("schema") != "postfiat.pfeth.external_review_gate.v1":
        raise DeploymentError("external-review schema mismatch")
    status = review.get("status")
    if status not in ("PASS", "WAIVED"):
        raise DeploymentError("external contract/circuit review is not PASS or WAIVED")
    if review.get("manifest_sha256") != manifest_digest:
        raise DeploymentError("external review is bound to a different manifest")
    if status == "WAIVED":
        # Manager-authorised deployment of an unreviewed release. The waiver is
        # recorded verbatim in the deployment output so the artifact can never
        # be mistaken for a reviewed one.
        for field in ("waived_by", "waived_at", "waiver_statement"):
            if not isinstance(review.get(field), str) or not review[field].strip():
                raise DeploymentError(f"review waiver {field} is absent")
        if "unreviewed" not in review["waiver_statement"].lower():
            raise DeploymentError("review waiver statement must say the release is unreviewed")
        return
    if not REQUIRED_CONTRACTS.issubset(set(review.get("reviewed_contracts", []))):
        raise DeploymentError("external review does not name all three contracts")
    if not REQUIRED_PROGRAMS.issubset(set(review.get("reviewed_programs", []))):
        raise DeploymentError("external review does not name both SP1 programs")
    for field in ("reviewer", "reviewed_at", "signature_or_report"):
        if not isinstance(review.get(field), str) or not review[field].strip():
            raise DeploymentError(f"external review {field} is absent")


def validate_checkpoint_gate(
    gate: dict[str, Any],
    manifest: dict[str, Any],
    manifest_digest: str,
    *,
    now: datetime | None = None,
) -> None:
    """Require an authenticated, recent PFTL checkpoint bound to this package.

    The egress guest accepts at most a bounded ancestry window after its initial
    checkpoint. A historical checkpoint can therefore deploy successfully and
    still make the first withdrawal unprovable. This gate forces the operator
    to rebase the package against the active fleet immediately before deploy.
    """
    if gate.get("schema") != "postfiat.pfeth.pftl_checkpoint_gate.v1":
        raise DeploymentError("PFTL checkpoint gate schema mismatch")
    if gate.get("status") != "PASS":
        raise DeploymentError("PFTL checkpoint gate is not PASS")
    if gate.get("manifest_sha256") != manifest_digest:
        raise DeploymentError("PFTL checkpoint gate is bound to a different manifest")
    expected = {
        "chain_id": manifest["pftl"]["chain_id"],
        "genesis_hash": manifest["pftl"]["genesis_hash"],
        "checkpoint_block_id": manifest["pftl"]["initial_checkpoint_block_id"],
        "finalized_height": int(manifest["pftl"]["initial_finalized_height"]),
        "committee_root": manifest["pftl"]["initial_committee_root"],
    }
    for field, value in expected.items():
        observed = gate.get(field)
        if field == "finalized_height":
            try:
                observed = int(observed)
            except (TypeError, ValueError) as error:
                raise DeploymentError("PFTL checkpoint gate finalized_height is invalid") from error
        if observed != value:
            raise DeploymentError(f"PFTL checkpoint gate {field} mismatch")
    receipt = gate.get("source_receipt")
    if not isinstance(receipt, str) or not receipt.strip():
        raise DeploymentError("PFTL checkpoint gate source_receipt is absent")
    observed_at = gate.get("observed_at")
    if not isinstance(observed_at, str) or not observed_at.strip():
        raise DeploymentError("PFTL checkpoint gate observed_at is absent")
    try:
        observed_time = datetime.fromisoformat(observed_at.replace("Z", "+00:00"))
    except ValueError as error:
        raise DeploymentError("PFTL checkpoint gate observed_at is not RFC3339") from error
    if observed_time.tzinfo is None:
        raise DeploymentError("PFTL checkpoint gate observed_at must include a timezone")
    current = now or datetime.now(timezone.utc)
    current = current.astimezone(timezone.utc)
    age = (current - observed_time.astimezone(timezone.utc)).total_seconds()
    if age < -CHECKPOINT_MAX_FUTURE_SKEW_SECONDS:
        raise DeploymentError("PFTL checkpoint gate observed_at is in the future")
    if age > CHECKPOINT_MAX_AGE_SECONDS:
        raise DeploymentError("PFTL checkpoint gate is stale; rebase the deployment package")


def preflight(package: Path, rpc_url: str) -> dict[str, Any]:
    manifest_path = package / "manifest.json"
    manifest = read_json(manifest_path)
    if manifest.get("schema") != "postfiat.pfeth.ethereum_mainnet_deployment_manifest.v1":
        raise DeploymentError("deployment manifest schema mismatch")
    if manifest.get("status") != "generated-not-reviewed-not-deployed":
        raise DeploymentError("deployment manifest is not a pre-deploy package")
    digest = sha256(manifest_path)
    verifier_init = read_hex(package / "verifier-init-code.hex")
    vault_init = read_hex(package / "vault-init-code.hex")
    expected_init_hashes = {
        "verifier": manifest["contracts"]["PFTLFinalityVerifierV1"][
            "init_code_keccak256"
        ],
        "vault": manifest["contracts"]["WETHBridgeVaultL1"][
            "init_code_keccak256"
        ],
    }
    actual_init_hashes = {
        "verifier": k256(verifier_init),
        "vault": k256(vault_init),
    }
    if actual_init_hashes != expected_init_hashes:
        raise DeploymentError("deployment init code does not match the reviewed manifest")
    bootstrap_hashes = manifest.get("pftl", {}).get("bootstrap", {}).get(
        "artifact_sha256"
    )
    if not isinstance(bootstrap_hashes, dict) or not bootstrap_hashes:
        raise DeploymentError("manifest lacks PFTL bootstrap artifact hashes")
    for name, expected_hash in bootstrap_hashes.items():
        relative = Path(name)
        if relative.name != name or relative.is_absolute() or ".." in relative.parts:
            raise DeploymentError("manifest contains an unsafe PFTL bootstrap artifact path")
        artifact_path = package / "pftl-bootstrap" / relative
        if not artifact_path.is_file() or sha256(artifact_path) != expected_hash:
            raise DeploymentError(f"PFTL bootstrap artifact mismatch: {name}")
    chain_id = int(rpc_call(rpc_url, "eth_chainId", []), 16)
    if chain_id != CHAIN_ID:
        raise DeploymentError(f"expected Ethereum chain 1, got {chain_id}")
    deployer = manifest["deployer"]
    live_nonce = int(
        rpc_call(rpc_url, "eth_getTransactionCount", [deployer, "latest"]), 16
    )
    if live_nonce != int(manifest["deployer_nonce"]):
        raise DeploymentError(
            f"deployer nonce drift: package={manifest['deployer_nonce']} live={live_nonce}"
        )

    code_checks: dict[str, Any] = {}
    for label, address, expected_hash in (
        (
            "sp1_verifier",
            manifest["sp1_verifier"]["address"],
            manifest["sp1_verifier"]["runtime_code_hash"],
        ),
        (
            "weth",
            manifest["ingress_policy"]["token_address"],
            manifest["ingress_policy"]["token_runtime_code_hash"],
        ),
    ):
        code = bytes.fromhex(
            rpc_call(rpc_url, "eth_getCode", [address, "latest"]).removeprefix("0x")
        )
        actual_hash = k256(code)
        if not code or actual_hash.lower() != expected_hash.lower():
            raise DeploymentError(
                f"{label} runtime mismatch: expected={expected_hash} actual={actual_hash}"
            )
        code_checks[label] = {
            "address": address,
            "runtime_code_hash": actual_hash,
            "code_bytes": len(code),
        }

    for label, address in manifest["predicted_addresses"].items():
        code = rpc_call(rpc_url, "eth_getCode", [address, "latest"])
        if code != "0x":
            raise DeploymentError(f"predicted {label} address already has code: {address}")

    return {
        "schema": "postfiat.pfeth.ethereum_mainnet_deployment_preflight.v1",
        "ok": True,
        "manifest_sha256": digest,
        "chain_id": chain_id,
        "deployer": deployer,
        "live_nonce": live_nonce,
        "predicted_addresses": manifest["predicted_addresses"],
        "init_code": {
            "verifier": {
                "bytes": len(verifier_init),
                "keccak256": k256(verifier_init),
            },
            "vault": {"bytes": len(vault_init), "keccak256": k256(vault_init)},
        },
        "code_checks": code_checks,
    }


def deploy(
    package: Path,
    rpc_url: str,
    socket_path: Path,
    approval: str | None,
) -> dict[str, Any]:
    manifest = read_json(package / "manifest.json")
    digest = sha256(package / "manifest.json")
    validate_review(read_json(package / "external-review.json"), digest)
    validate_checkpoint_gate(
        read_json(package / "pftl-checkpoint-gate.json"), manifest, digest
    )
    expected_approval = f"DEPLOY PFETH {digest}"
    if approval != expected_approval:
        raise DeploymentError(
            "live deployment approval mismatch; required exact phrase: "
            + expected_approval
        )
    before = preflight(package, rpc_url)
    verifier_init = read_hex(package / "verifier-init-code.hex")
    vault_init = read_hex(package / "vault-init-code.hex")
    session_id = f"pfeth-ethereum-mainnet-{digest[:16]}"

    agent_call(socket_path, {"op": "close_launch_session"})
    agent_call(
        socket_path,
        {
            "op": "open_launch_session",
            "session_id": session_id,
            "chain_id": CHAIN_ID,
            "allowlist": [],
            "usdc_address": manifest["ingress_policy"]["token_address"],
            "usdc_budget": 0,
            "native_wei_budget": 0,
            "expected_deploys": [
                {
                    "label": "pfeth_finality_verifier_v1",
                    "bytecode_hash": k256(verifier_init),
                    "bytecode_len": len(verifier_init),
                },
                {
                    "label": "pfeth_weth_vault_v1",
                    "bytecode_hash": k256(vault_init),
                    "bytecode_len": len(vault_init),
                },
            ],
            "ttl_seconds": 1200,
        },
    )
    results: list[dict[str, Any]] = []
    try:
        for label, action, init_code in (
            ("verifier", "pfeth_finality_verifier_v1", verifier_init),
            ("vault", "pfeth_weth_vault_v1", vault_init),
        ):
            result = agent_call(
                socket_path,
                {
                    "op": "evm_contract_tx",
                    "to": None,
                    "data": "0x" + init_code.hex(),
                    "rpc_url": rpc_url,
                    "chain_id": CHAIN_ID,
                    "session_id": session_id,
                    "session_action": action,
                    "gas_usd": 10.0,
                    "label": f"pfETH {label} Ethereum mainnet",
                },
            )
            expected = manifest["predicted_addresses"][label]
            if result.get("contract_address", "").lower() != expected.lower():
                raise DeploymentError(
                    f"{label} deployed at {result.get('contract_address')}, expected {expected}"
                )
            results.append(
                {
                    "label": label,
                    "contract_address": result["contract_address"],
                    "tx": result["tx"],
                    "gas_used": result.get("gas_used"),
                }
            )
    finally:
        agent_call(socket_path, {"op": "close_launch_session"})

    code_readback = {}
    for result in results:
        code = bytes.fromhex(
            rpc_call(
                rpc_url,
                "eth_getCode",
                [result["contract_address"], "latest"],
            ).removeprefix("0x")
        )
        if not code:
            raise DeploymentError(f"{result['label']} deployment has no runtime code")
        runtime_hash = k256(code)
        if (
            result["label"] == "vault"
            and runtime_hash.lower()
            != manifest["contracts"]["WETHBridgeVaultL1"][
                "deployed_runtime_code_keccak256"
            ].lower()
        ):
            raise DeploymentError("vault deployed runtime does not match the reviewed manifest")
        code_readback[result["label"]] = {
            "address": result["contract_address"],
            "runtime_code_hash": runtime_hash,
            "code_bytes": len(code),
        }
    receipt = {
        "schema": "postfiat.pfeth.ethereum_mainnet_deployment_receipt.v1",
        "ok": True,
        "manifest_sha256": digest,
        "approval": expected_approval,
        "preflight": before,
        "deployments": results,
        "runtime_readback": code_readback,
        "route_activation": "PENDING",
    }
    write_private_json(package / "deployment-receipt.json", receipt)
    return receipt


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--package", type=Path, default=DEFAULT_PACKAGE)
    parser.add_argument("--rpc-url", default=DEFAULT_RPC)
    parser.add_argument("--agent-socket", type=Path, default=DEFAULT_AGENT_SOCKET)
    parser.add_argument("--execute", action="store_true")
    parser.add_argument("--approval")
    args = parser.parse_args()
    try:
        if args.execute:
            result = deploy(args.package, args.rpc_url, args.agent_socket, args.approval)
        else:
            result = preflight(args.package, args.rpc_url)
            result["status"] = "read-only-preflight; no transaction submitted"
    except DeploymentError as error:
        raise SystemExit(f"pfeth_deploy=failed: {error}") from error
    print(json.dumps(result, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
