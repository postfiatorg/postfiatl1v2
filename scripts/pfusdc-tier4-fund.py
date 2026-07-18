#!/usr/bin/env python3
"""Fund the frozen pfUSDC Tier-4 Sepolia wallet without exposing its key."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import sys
import tempfile
import time
import urllib.error
import urllib.parse
import urllib.request
import uuid
from datetime import UTC, datetime
from decimal import Decimal
from pathlib import Path
from typing import Any, Callable


REPOSITORY_ROOT = Path(__file__).resolve().parents[1]
DEFAULT_DEPLOYMENT_DIR = REPOSITORY_ROOT / "deployments/pfusdc-tier4-sepolia-20260718"
DEFAULT_STAKEHUB_REPO = REPOSITORY_ROOT.parent / "StakeHub"
DEFAULT_EVIDENCE_DIR = REPOSITORY_ROOT / "docs/evidence/pfusdc-tier4-funding-live"
EXPECTED_ROUTE_SHA256 = "c65edb1d09dc46e7e589888d57381633037b40a1e86e7dba916775bb8431bf3d"
ZERO_ADDRESS = "0x0000000000000000000000000000000000000000"


class FundingError(RuntimeError):
    """A fail-closed funding route, policy, balance, or delivery failure."""


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def _load_json(path: Path) -> dict[str, Any]:
    value = json.loads(path.read_text())
    if not isinstance(value, dict):
        raise FundingError(f"expected JSON object: {path}")
    return value


def _hex(value: Any) -> str:
    raw = value.removeprefix("0x") if isinstance(value, str) else bytes(value).hex()
    return "0x" + raw.lower()


def _assert_equal(label: str, actual: Any, expected: Any) -> None:
    if isinstance(actual, (bytes, bytearray)):
        actual = _hex(actual)
    if isinstance(actual, str) and actual.startswith("0x"):
        actual = actual.lower()
    if isinstance(expected, str) and expected.startswith("0x"):
        expected = expected.lower()
    if actual != expected:
        raise FundingError(f"{label} mismatch: got {actual!r}, expected {expected!r}")


def _ceil_div(numerator: int, denominator: int) -> int:
    if denominator <= 0:
        raise FundingError("division denominator must be positive")
    return (numerator + denominator - 1) // denominator


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


class FundingDriver:
    def __init__(self, arguments: argparse.Namespace) -> None:
        try:
            from eth_abi import decode, encode
            from web3 import Web3
        except ImportError as error:
            raise FundingError(
                "web3/eth_abi unavailable; run with the StakeHub virtualenv Python"
            ) from error

        self.Web3 = Web3
        self.decode = decode
        self.encode = encode
        self.arguments = arguments
        self.route_path = arguments.deployment_dir.resolve() / "funding-route.json"
        if _sha256(self.route_path) != EXPECTED_ROUTE_SHA256:
            raise FundingError("frozen funding route SHA-256 mismatch")
        self.route = _load_json(self.route_path)
        _assert_equal(
            "funding route schema",
            self.route.get("schema"),
            "postfiat.pfusdc.tier4_funding_route.v1",
        )
        self.paid = self.route["paid_native_gas_route"]
        self.authorization = self.route["authorization"]
        self.deployer = Web3.to_checksum_address(self.authorization["approved_wallet"])
        self.state_path = arguments.evidence_dir.resolve() / "state.json"
        self.mainnet = Web3(
            Web3.HTTPProvider(arguments.mainnet_rpc, request_kwargs={"timeout": 30})
        )
        self.ethereum = Web3(
            Web3.HTTPProvider(arguments.ethereum_rpc, request_kwargs={"timeout": 30})
        )
        self.arbitrum = Web3(
            Web3.HTTPProvider(arguments.arbitrum_rpc, request_kwargs={"timeout": 30})
        )
        self.agent_call = self._load_agent_call(arguments.stakehub_repo.resolve())

    @staticmethod
    def _load_agent_call(stakehub_repo: Path) -> Callable[..., dict[str, Any] | None]:
        if not (stakehub_repo / "stakehub/agentd.py").is_file():
            raise FundingError(f"StakeHub agentd not found under {stakehub_repo}")
        sys.path.insert(0, str(stakehub_repo))
        from stakehub.agentd import call

        return call

    def _call(self, address: str, signature: str, output_types: list[str]) -> tuple[Any, ...]:
        target = self.Web3.to_checksum_address(address)
        data = self.Web3.keccak(text=signature)[:4]
        result = self.mainnet.eth.call({"to": target, "data": data})
        return self.decode(output_types, result)

    def _quote(self, target: dict[str, Any]) -> dict[str, Any]:
        query = urllib.parse.urlencode(
            {"chainId": str(target["chain_id"]), "symbol": target["asset"]}
        )
        request = urllib.request.Request(
            f"{self.paid['quote_endpoint']}?{query}",
            headers={"User-Agent": "postfiat-pfusdc-tier4/1"},
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                quote = json.loads(response.read())
        except (urllib.error.URLError, json.JSONDecodeError) as error:
            raise FundingError(f"Drip.Tools quote request failed: {error}") from error
        _assert_equal("quote chain ID", str(quote.get("chainId")), str(target["chain_id"]))
        _assert_equal("quote symbol", quote.get("symbol"), target["asset"])
        _assert_equal("quote asset", quote.get("address"), target["asset_address"])
        if not quote.get("available"):
            raise FundingError(f"{target['chain']} provider inventory is unavailable")
        if int(quote.get("balance", 0)) <= 0:
            raise FundingError(f"{target['chain']} provider inventory is empty")
        return quote

    def _target(self, chain_id: int) -> dict[str, Any]:
        for target in self.paid["targets"]:
            if int(target["chain_id"]) == chain_id:
                return target
        raise FundingError(f"target chain {chain_id} is absent from the frozen route")

    def _target_client(self, chain_id: int) -> Any:
        if chain_id == 11155111:
            return self.ethereum
        if chain_id == 421614:
            return self.arbitrum
        raise FundingError(f"unsupported target chain: {chain_id}")

    def _load_state(self) -> dict[str, Any]:
        if not self.state_path.exists():
            return {
                "schema": "pfusdc-tier4-funding.v1",
                "route_sha256": EXPECTED_ROUTE_SHA256,
                "deployer": self.deployer,
                "orders": {},
                "events": [],
            }
        state = _load_json(self.state_path)
        _assert_equal("funding state schema", state.get("schema"), "pfusdc-tier4-funding.v1")
        _assert_equal("funding state route", state.get("route_sha256"), EXPECTED_ROUTE_SHA256)
        _assert_equal("funding state deployer", state.get("deployer"), self.deployer)
        return state

    def _write_state(self, state: dict[str, Any], event: dict[str, Any]) -> None:
        state["events"].append({"at": _utc_now(), **event})
        _atomic_write_json(self.state_path, state)

    def _validate_contract(self) -> None:
        _assert_equal("source chain ID", int(self.mainnet.eth.chain_id), 1)
        vault = self.Web3.to_checksum_address(self.paid["vault"])
        code = bytes(self.mainnet.eth.get_code(vault))
        _assert_equal("vault runtime length", len(code), self.paid["vault_runtime_code_length"])
        _assert_equal(
            "vault runtime hash",
            _hex(self.Web3.keccak(code)),
            self.paid["vault_runtime_code_hash"],
        )
        getters = [
            ("minNativeAmount()", ["uint256"], self.paid["vault_min_native_amount_wei"]),
            (
                "minStablecoinAmount()",
                ["uint256"],
                self.paid["vault_min_stablecoin_amount_atoms"],
            ),
            ("paused()", ["bool"], self.paid["vault_paused"]),
            ("usdcAddress()", ["address"], self.paid["vault_usdc"]),
            ("usdtAddress()", ["address"], self.paid["vault_usdt"]),
            ("owner()", ["address"], self.paid["vault_owner"]),
        ]
        for signature, types, expected in getters:
            _assert_equal(signature, self._call(self.paid["vault"], signature, types)[0], expected)
        if self.paid["vault_paused"]:
            raise FundingError("paid funding vault is paused")

    def _source_price(self) -> dict[str, int]:
        feed = self.Web3.to_checksum_address(self.paid["source_price_feed"])
        decimals = self._call(feed, "decimals()", ["uint8"])[0]
        _assert_equal("source price decimals", decimals, self.paid["source_price_feed_decimals"])
        round_id, answer, _started_at, updated_at, answered_in_round = self._call(
            feed,
            "latestRoundData()",
            ["uint80", "int256", "uint256", "uint256", "uint80"],
        )
        if answer <= 0 or updated_at <= 0 or answered_in_round < round_id:
            raise FundingError("invalid source ETH/USD oracle round")
        age = int(time.time()) - int(updated_at)
        if age < 0 or age > self.arguments.max_price_age:
            raise FundingError(f"source ETH/USD price is stale by {age} seconds")
        return {"answer": int(answer), "decimals": int(decimals), "updated_at": int(updated_at)}

    def _order(self, chain_id: int) -> dict[str, Any]:
        target = self._target(chain_id)
        client = self._target_client(chain_id)
        if not client.is_connected():
            raise FundingError(f"{target['chain']} RPC is unavailable")
        _assert_equal("target chain ID", int(client.eth.chain_id), chain_id)
        dispenser_code = bytes(
            client.eth.get_code(self.Web3.to_checksum_address(target["dispenser"]))
        )
        _assert_equal(
            f"{target['chain']} dispenser runtime length",
            len(dispenser_code),
            target["dispenser_runtime_code_length"],
        )
        _assert_equal(
            f"{target['chain']} dispenser runtime hash",
            _hex(self.Web3.keccak(dispenser_code)),
            target["dispenser_runtime_code_hash"],
        )
        quote = self._quote(target)
        source_price = self._source_price()
        usd_micros = int(self.paid["planned_order_usd_micros"])
        amount_wei = _ceil_div(
            usd_micros * 10 ** (18 + source_price["decimals"]),
            1_000_000 * source_price["answer"],
        )
        if amount_wei < int(self.paid["vault_min_native_amount_wei"]):
            raise FundingError("planned source value is below the live vault minimum")
        selector = self.Web3.keccak(text=self.paid["vault_buy_signature"])[:4]
        arguments = self.encode(
            ["(address,uint256,address)"],
            [(self.deployer, chain_id, self.Web3.to_checksum_address(target["asset_address"]))],
        )
        data = selector + arguments
        transaction = {
            "from": self.deployer,
            "to": self.Web3.to_checksum_address(self.paid["vault"]),
            "data": data,
            "value": amount_wei,
        }
        self.mainnet.eth.call(transaction)
        gas = int(self.mainnet.eth.estimate_gas(transaction) * 13 // 10)
        latest = self.mainnet.eth.get_block("latest")
        base_fee = int(latest.get("baseFeePerGas", self.mainnet.eth.gas_price))
        priority_fee = max(int(self.mainnet.eth.max_priority_fee), 10_000_000)
        max_fee_per_gas = base_fee * 2 + priority_fee
        gas_fee_wei = gas * max_fee_per_gas
        source_value_usd_micros = _ceil_div(
            amount_wei * source_price["answer"] * 1_000_000,
            10 ** (18 + source_price["decimals"]),
        )
        gas_usd_micros = _ceil_div(
            gas_fee_wei * source_price["answer"] * 1_000_000,
            10 ** (18 + source_price["decimals"]),
        )
        quote_price_micros = int(Decimal(str(quote["price"])) * Decimal(1_000_000))
        expected_target_atoms = (
            source_value_usd_micros * (10 ** int(quote["decimals"])) // quote_price_micros
        )
        balance_wei = int(client.eth.get_balance(self.deployer))
        return {
            "chain": target["chain"],
            "chain_id": chain_id,
            "recipient": self.deployer,
            "target_asset": target["asset"],
            "target_asset_address": target["asset_address"],
            "target_balance_before": balance_wei,
            "required_target_balance": int(target["required_wallet_balance_wei"]),
            "provider_inventory_atoms": int(quote["balance"]),
            "provider_quote_price_usd": str(quote["price"]),
            "expected_target_atoms": expected_target_atoms,
            "source_value_wei": amount_wei,
            "source_value_usd_micros": source_value_usd_micros,
            "estimated_gas": gas,
            "estimated_gas_fee_wei": gas_fee_wei,
            "estimated_gas_usd_micros": gas_usd_micros,
            "max_fee_per_gas": max_fee_per_gas,
            "source_price_answer": source_price["answer"],
            "source_price_updated_at": source_price["updated_at"],
            "calldata": _hex(data),
            "calldata_hash": _hex(self.Web3.keccak(data)),
        }

    def quote(self) -> dict[str, Any]:
        if not self.mainnet.is_connected():
            raise FundingError("Ethereum mainnet RPC is unavailable")
        self._validate_contract()
        status = self.agent_call({"op": "status"})
        if not status or not status.get("ok"):
            raise FundingError("StakeHub agent is unavailable")
        whitelist = {
            str(address).lower() for address in status.get("policy", {}).get("whitelist", [])
        }
        orders = [self._order(11155111), self._order(421614)]
        total = sum(
            order["source_value_usd_micros"] + order["estimated_gas_usd_micros"]
            for order in orders
        )
        if total > int(self.authorization["aggregate_max_usd_micros"]):
            raise FundingError("quoted native-gas funding exceeds the authorized aggregate cap")
        return {
            "route_sha256": EXPECTED_ROUTE_SHA256,
            "provider": self.paid["provider"],
            "vault": self.Web3.to_checksum_address(self.paid["vault"]),
            "vault_allowlisted": self.paid["vault"].lower() in whitelist,
            "allowlist_command": self.paid["policy_allowlist_command"],
            "agent_unlocked": bool(status.get("unlocked")),
            "spent_today_usd": status.get("spent_today_usd"),
            "source_balance_wei": int(self.mainnet.eth.get_balance(self.deployer)),
            "orders": orders,
            "quoted_total_usd_micros": total,
            "authorized_aggregate_max_usd_micros": int(
                self.authorization["aggregate_max_usd_micros"]
            ),
        }

    def _delivery(self, order: dict[str, Any]) -> dict[str, Any]:
        balance = int(self._target_client(order["chain_id"]).eth.get_balance(self.deployer))
        return {
            "delivered": balance >= int(order["required_target_balance"]),
            "target_balance_wei": balance,
            "required_target_balance_wei": int(order["required_target_balance"]),
        }

    def buy(self, chain_id: int) -> dict[str, Any]:
        if not self.arguments.confirm_live_funds:
            raise FundingError("live funding requires --confirm-live-funds")
        preflight = self.quote()
        if not preflight["agent_unlocked"]:
            raise FundingError("StakeHub agent is locked")
        if not preflight["vault_allowlisted"]:
            raise FundingError(
                "paid funding vault is not allowlisted; run the recorded passphrase-gated command"
            )
        order = next(item for item in preflight["orders"] if item["chain_id"] == chain_id)
        delivery = self._delivery(order)
        if delivery["delivered"]:
            return {"already_funded": True, "order": order, **delivery}

        state = self._load_state()
        key = str(chain_id)
        prior = state["orders"].get(key)
        if prior:
            result = self._delivery(prior)
            if result["delivered"] and prior.get("status") != "delivered":
                prior.update({"status": "delivered", **result})
                self._write_state(
                    state, {"phase": "delivered", "chain_id": chain_id, **result}
                )
            return {"already_ordered": True, "order": prior, **result}

        conservative_total = sum(
            int(item.get("source_value_usd_micros", 0))
            + int(item.get("estimated_gas_usd_micros", 0))
            for item in state["orders"].values()
        )
        conservative_total += order["source_value_usd_micros"] + order["estimated_gas_usd_micros"]
        if conservative_total > int(self.authorization["aggregate_max_usd_micros"]):
            raise FundingError("live order would exceed the authorized aggregate cap")

        persisted = {key_name: value for key_name, value in order.items() if key_name != "calldata"}
        persisted["status"] = "prepared"
        state["orders"][key] = persisted
        self._write_state(
            state,
            {
                "phase": "prepared",
                "chain_id": chain_id,
                "source_value_wei": order["source_value_wei"],
                "calldata_hash": order["calldata_hash"],
            },
        )

        response = self.agent_call(
            {
                "op": "evm_contract_tx",
                "to": self.Web3.to_checksum_address(self.paid["vault"]),
                "data": order["calldata"],
                "rpc_url": self.arguments.mainnet_rpc,
                "chain_id": 1,
                "label": f"pfUSDC Tier-4 {order['chain']} gas purchase",
                "value_wei": order["source_value_wei"],
                "gas_usd": order["estimated_gas_usd_micros"] / 1_000_000,
            },
            timeout=1200.0,
        )
        if not response or not response.get("ok"):
            raise FundingError(
                "StakeHub agent did not accept the funding transaction: "
                + str(response.get("error") if response else "no response")
            )
        persisted.update(
            {
                "status": "source_confirmed",
                "source_tx": response["tx"],
                "source_gas_used": response["gas_used"],
                "agent_charged_usd": response.get("charged_usd"),
            }
        )
        self._write_state(
            state,
            {
                "phase": "source_confirmed",
                "chain_id": chain_id,
                "source_tx": response["tx"],
                "source_gas_used": response["gas_used"],
            },
        )

        deadline = time.monotonic() + min(max(self.arguments.delivery_wait, 0), 60)
        result = self._delivery(persisted)
        while not result["delivered"] and time.monotonic() < deadline:
            time.sleep(min(5, max(0, deadline - time.monotonic())))
            result = self._delivery(persisted)
        if result["delivered"]:
            persisted.update({"status": "delivered", **result})
            self._write_state(
                state, {"phase": "delivered", "chain_id": chain_id, **result}
            )
        return {"order": persisted, **result}

    def check(self) -> dict[str, Any]:
        state = self._load_state()
        deliveries = {}
        for key, order in state["orders"].items():
            result = self._delivery(order)
            deliveries[key] = result
            if result["delivered"] and order.get("status") != "delivered":
                order.update({"status": "delivered", **result})
                self._write_state(
                    state, {"phase": "delivered", "chain_id": int(key), **result}
                )
        return {"orders": state["orders"], "deliveries": deliveries}

    def circle_request(self) -> dict[str, Any]:
        token_address = self.route["canonical_usdc_route"]["target_token"]
        token = self.arbitrum.eth.contract(
            address=self.Web3.to_checksum_address(token_address),
            abi=[
                {
                    "type": "function",
                    "name": "balanceOf",
                    "stateMutability": "view",
                    "inputs": [{"name": "account", "type": "address"}],
                    "outputs": [{"name": "", "type": "uint256"}],
                }
            ],
        )
        balance = int(token.functions.balanceOf(self.deployer).call())
        if balance > 0:
            return {"already_funded": True, "canonical_usdc_atoms": balance}
        if not self.arguments.confirm_circle_request:
            raise FundingError("Circle faucet request requires --confirm-circle-request")
        api_key = os.environ.get("CIRCLE_API_KEY")
        if not api_key:
            raise FundingError("CIRCLE_API_KEY is not set; use the official browser faucet instead")
        body = json.dumps(self.route["canonical_usdc_route"]["request"]).encode()
        request_id = str(
            uuid.uuid5(uuid.NAMESPACE_URL, self.route["deployment_id"] + ":circle-arb-usdc")
        )
        request = urllib.request.Request(
            self.route["canonical_usdc_route"]["api_url"],
            method="POST",
            data=body,
            headers={
                "Authorization": f"Bearer {api_key}",
                "Content-Type": "application/json",
                "X-Request-Id": request_id,
                "User-Agent": "postfiat-pfusdc-tier4/1",
            },
        )
        try:
            with urllib.request.urlopen(request, timeout=30) as response:
                status = int(response.status)
        except urllib.error.HTTPError as error:
            detail = error.read().decode(errors="replace")[:500]
            raise FundingError(f"Circle faucet rejected request ({error.code}): {detail}") from error
        if status != 204:
            raise FundingError(f"Circle faucet returned unexpected HTTP {status}")
        state = self._load_state()
        self._write_state(
            state,
            {
                "phase": "circle_usdc_requested",
                "chain_id": 421614,
                "request_id": request_id,
                "api_http_status": status,
            },
        )
        return {"requested": True, "request_id": request_id, "api_http_status": status}


def _parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "action",
        choices=("quote", "buy-ethereum-gas", "buy-arbitrum-gas", "check", "circle-usdc"),
    )
    parser.add_argument("--deployment-dir", type=Path, default=DEFAULT_DEPLOYMENT_DIR)
    parser.add_argument("--stakehub-repo", type=Path, default=DEFAULT_STAKEHUB_REPO)
    parser.add_argument("--evidence-dir", type=Path, default=DEFAULT_EVIDENCE_DIR)
    parser.add_argument(
        "--mainnet-rpc",
        default=os.environ.get("ETHEREUM_MAINNET_RPC_URL", "https://ethereum-rpc.publicnode.com"),
    )
    parser.add_argument(
        "--ethereum-rpc",
        default=os.environ.get(
            "ETHEREUM_SEPOLIA_RPC_URL", "https://ethereum-sepolia-rpc.publicnode.com"
        ),
    )
    parser.add_argument(
        "--arbitrum-rpc",
        default=os.environ.get(
            "ARBITRUM_SEPOLIA_RPC_URL", "https://arbitrum-sepolia-rpc.publicnode.com"
        ),
    )
    parser.add_argument("--max-price-age", type=int, default=3600)
    parser.add_argument("--delivery-wait", type=int, default=60)
    parser.add_argument("--confirm-live-funds", action="store_true")
    parser.add_argument("--confirm-circle-request", action="store_true")
    return parser.parse_args()


def main() -> int:
    arguments = _parse_arguments()
    try:
        driver = FundingDriver(arguments)
        if arguments.action == "quote":
            result = driver.quote()
        elif arguments.action == "buy-ethereum-gas":
            result = driver.buy(11155111)
        elif arguments.action == "buy-arbitrum-gas":
            result = driver.buy(421614)
        elif arguments.action == "check":
            result = driver.check()
        else:
            result = driver.circle_request()
        print(json.dumps(result, indent=2, sort_keys=True))
        return 0
    except Exception as error:  # noqa: BLE001 - bounded CLI error surface.
        message = str(error)
        for secret in (arguments.mainnet_rpc, arguments.ethereum_rpc, arguments.arbitrum_rpc):
            if secret:
                message = message.replace(secret, "<redacted-rpc>")
        print(json.dumps({"ok": False, "error": f"{type(error).__name__}: {message}"}))
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
