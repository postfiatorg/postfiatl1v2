#!/usr/bin/env python3
"""Check exact approvals and recovery without connecting to a chain or signer."""
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from web3 import Web3
from web3.providers.base import BaseProvider

spec = importlib.util.spec_from_file_location("allowances", Path(__file__).with_name("a666-mainnet-uniswap-allowances.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class FakeProvider(BaseProvider):
    def __init__(self):
        super().__init__()
        self.erc = {module.USDC.lower(): 123, module.WA666.lower(): 0}
        self.permit = {module.USDC.lower(): [456, 9999999999, 0], module.WA666.lower(): [0, 0, 0]}
        self.calls = []

    def is_connected(self, show_traceback=False):
        return True

    def make_request(self, method, params):
        if method == "eth_chainId":
            result = "0x1"
        elif method == "eth_call":
            tx = params[0]
            if tx["to"].lower() == module.PERMIT2.lower():
                args = Web3().codec.decode(["address", "address", "address"], bytes.fromhex(tx["data"][10:]))
                result = "0x" + Web3().codec.encode(["uint160", "uint48", "uint48"], self.permit[args[1].lower()]).hex()
            else:
                result = "0x" + Web3().codec.encode(["uint256"], [self.erc[tx["to"].lower()]]).hex()
        elif method == "eth_getTransactionReceipt":
            result = {"transactionHash": params[0], "transactionIndex": "0x0", "blockHash": "0x" + "ab" * 32,
                      "blockNumber": "0x1", "from": module.WALLET, "to": module.WA666,
                      "cumulativeGasUsed": "0x5208", "gasUsed": "0x5208", "effectiveGasPrice": "0x1",
                      "status": "0x1", "logs": [], "logsBloom": "0x" + "00" * 256}
        else:
            raise AssertionError(method)
        return {"jsonrpc": "2.0", "id": 1, "result": result}

    def agent(self, request, **kwargs):
        self.calls.append(request)
        if request["to"].lower() == module.PERMIT2.lower():
            token, spender, amount, expiry = Web3().codec.decode(["address", "address", "uint160", "uint48"], bytes.fromhex(request["data"][10:]))
            assert spender.lower() == module.ROUTER.lower()
            self.permit[token.lower()] = [amount, expiry, 0]
        else:
            spender, amount = Web3().codec.decode(["address", "uint256"], bytes.fromhex(request["data"][10:]))
            assert spender.lower() == module.PERMIT2.lower()
            self.erc[request["to"].lower()] = amount
        return {"ok": True, "tx": "0x" + format(len(self.calls), "064x")}


class ExactAllowanceTests(unittest.TestCase):
    def run_command(self, provider, output, agent=None):
        argv = ["allowances", "prepare", "--token", "wa666", "--amount-atoms", "1", "--ttl-seconds", "60", "--output", str(output)]
        with patch.object(module.Web3, "HTTPProvider", return_value=provider), patch.object(module, "agentd_call", side_effect=agent or provider.agent), patch("sys.argv", argv), patch("builtins.print"):
            module.main()

    def test_one_atom_and_short_valid_ttl_preserve_other_token(self):
        provider = FakeProvider()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "result.json"
            self.run_command(provider, output)
            self.assertEqual(provider.erc[module.WA666.lower()], 1)
            self.assertEqual(provider.permit[module.WA666.lower()][0], 1)
            self.assertEqual(provider.erc[module.USDC.lower()], 123)
            self.assertEqual(provider.permit[module.USDC.lower()][0], 456)
            self.assertEqual(len(provider.calls), 2)
            self.assertEqual(json.loads(output.read_text())["verdict"], "PASS")

    def test_expiration_overflow_rejects_before_any_approval(self):
        provider = FakeProvider()
        with tempfile.TemporaryDirectory() as directory, patch.object(module.time, "time", return_value=(1 << 48) - 1):
            with self.assertRaisesRegex(RuntimeError, "uint48"):
                self.run_command(provider, Path(directory) / "result.json")
        self.assertEqual(provider.calls, [])

    def test_ambiguous_broadcast_keeps_intent_and_refuses_resubmit(self):
        provider = FakeProvider()
        def ambiguous(request, **kwargs):
            provider.agent(request)
            raise TimeoutError("signer response lost after broadcast")
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "result.json"
            with self.assertRaises(TimeoutError):
                self.run_command(provider, output, ambiguous)
            journal = json.loads(output.with_suffix(".json.journal.json").read_text())
            self.assertEqual(journal["pending"]["target"], module.WA666)
            with self.assertRaisesRegex(RuntimeError, "reconcile"):
                self.run_command(provider, output)
            self.assertEqual(len(provider.calls), 1)


if __name__ == "__main__":
    unittest.main()
