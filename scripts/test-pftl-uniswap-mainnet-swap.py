#!/usr/bin/env python3
"""Exercise swap interruption recovery without a live signer or chain."""
import importlib.util
import json
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch

from web3 import Web3
from web3.providers.base import BaseProvider

spec = importlib.util.spec_from_file_location("swap", Path(__file__).with_name("pftl-uniswap-mainnet-swap.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class SwapProvider(BaseProvider):
    def __init__(self):
        super().__init__()
        self.balances = {module.WA666.lower(): 104, module.USDC.lower(): 500}
        self.sends = 0

    def is_connected(self, show_traceback=False):
        return True

    def make_request(self, method, params):
        if method == "eth_chainId":
            result = "0x1"
        elif method == "eth_blockNumber":
            result = "0x100"
        elif method == "eth_call":
            target = params[0]["to"].lower()
            result = "0x" if target == module.UNIVERSAL_ROUTER.lower() else "0x" + Web3().codec.encode(["uint256"], [self.balances[target]]).hex()
        elif method == "eth_getTransactionReceipt":
            result = {"transactionHash": params[0], "transactionIndex": "0x0", "blockHash": "0x" + "ab" * 32,
                      "blockNumber": "0x101", "from": module.WALLET, "to": module.UNIVERSAL_ROUTER,
                      "cumulativeGasUsed": "0x5208", "gasUsed": "0x5208", "effectiveGasPrice": "0x2",
                      "status": "0x1", "logs": [], "logsBloom": "0x" + "00" * 256}
        else:
            raise AssertionError(method)
        return {"jsonrpc": "2.0", "id": 1, "result": result}

    def agent(self, request, **kwargs):
        self.sends += 1
        self.balances[module.WA666.lower()] -= 1
        self.balances[module.USDC.lower()] += 1
        return {"ok": True, "tx": "0x" + "12" * 32}


class SwapRecoveryTests(unittest.TestCase):
    def execute(self, provider, output, agent=None, extra=None):
        argv = ["swap", "--direction", "wa666-to-usdc", "--amount-in-atoms", "1", "--min-out-atoms", "1",
                "--deadline-epoch", str(int(time.time()) + 600), "--packet-sha256", "ab" * 32, "--execute"]
        if output is not None:
            argv += ["--output", str(output)]
        argv += extra or []
        with patch.object(module.Web3, "HTTPProvider", return_value=provider), patch("stakehub.agentd.call", side_effect=agent or provider.agent), patch("sys.argv", argv), patch("builtins.print"):
            return module.main()

    def test_one_atom_trade_preserves_baseline_and_records_actual_gas(self):
        provider = SwapProvider()
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "swap.json"
            self.assertEqual(self.execute(provider, output), 0)
            report = json.loads(output.read_text())
            self.assertEqual(report["phase"], "swap-verified")
            self.assertEqual(report["input_spent_atoms"], 1)
            self.assertEqual(report["output_received_atoms"], 1)
            self.assertEqual(report["post_balances"]["token_in_atoms"], 103)
            self.assertEqual(report["effective_gas_price_wei"], 2)

    def test_lost_signer_response_cannot_repeat_trade(self):
        provider = SwapProvider()
        def ambiguous(request, **kwargs):
            provider.agent(request)
            raise TimeoutError("lost response after broadcast")
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "swap.json"
            with self.assertRaises(TimeoutError):
                self.execute(provider, output, ambiguous)
            journal = json.loads(output.with_suffix(".json.journal.json").read_text())
            self.assertEqual(journal["phase"], "submitting")
            self.assertEqual(journal["pre_balances"]["token_in_atoms"], 104)
            self.assertTrue(journal["calldata"].startswith("0x"))
            with self.assertRaisesRegex(RuntimeError, "reconcile"):
                self.execute(provider, output)
            self.assertEqual(provider.sends, 1)

    def test_output_and_positive_minimum_required_before_signing(self):
        provider = SwapProvider()
        with self.assertRaisesRegex(ValueError, "--output"):
            self.execute(provider, None)
        with tempfile.TemporaryDirectory() as directory:
            with self.assertRaisesRegex(ValueError, "positive uint128"):
                self.execute(provider, Path(directory) / "swap.json", extra=["--min-out-atoms", "0"])
        self.assertEqual(provider.sends, 0)


if __name__ == "__main__":
    unittest.main()
