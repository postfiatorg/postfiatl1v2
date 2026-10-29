"""The pft_token command reads the ERC-20 PFT token's state over JSON-RPC.

``urllib.request.urlopen`` is patched with a fake Base node whose eth_call
answers are ABI-encoded by hand (a string as offset, length and padded bytes;
uint256 and uint8 as one 32-byte word).
"""

from __future__ import annotations

import contextlib
import io
import json
import tempfile
import unittest
import urllib.error
from pathlib import Path
from unittest import mock

from postfiat_rpc import pft_token

TOKEN = "0x18fE4ca51A0cE042688D5A54917319d03B527D92"
TREASURY = "0x7424F4F4A620d37CFB54A4558a1Ec7f60b71A0CB"
OTHER = "0x00000000000000000000000000000000000000aa"
SUPPLY = 100_000_000_000_000_000
BLOCK = 51_305_977


def word(value: int) -> str:
    return f"{value:064x}"


def abi_string(text: str) -> str:
    data = text.encode()
    padded = data.hex().ljust(((len(data) + 31) // 32) * 64, "0")
    return "0x" + word(32) + word(len(data)) + padded


class FakeBase:
    """Answers JSON-RPC requests as a Base node holding the PFT token."""

    def __init__(self, chain_id: int = 84532, code: str = "0x6080604052", balance: int = SUPPLY) -> None:
        self.chain_id = chain_id
        self.code = code
        self.balance = balance
        self.overrides: dict[str, object] = {}
        self.methods: list[str] = []

    def result(self, method: str, params: list) -> object:
        if method == "eth_chainId":
            return hex(self.chain_id)
        if method == "eth_blockNumber":
            return hex(BLOCK)
        if method == "eth_getCode":
            assert params == [TOKEN, hex(BLOCK)], params
            return self.code
        assert method == "eth_call" and params[1] == hex(BLOCK), (method, params)
        data = params[0]["data"]
        selector = data[2:10]
        if selector == pft_token.SELECTORS["balanceOf"]:
            assert data[10:] == TREASURY[2:].lower().rjust(64, "0"), data
        answers = {
            pft_token.SELECTORS["name"]: abi_string("Post Fiat Token"),
            pft_token.SELECTORS["symbol"]: abi_string("PFT"),
            pft_token.SELECTORS["decimals"]: "0x" + word(6),
            pft_token.SELECTORS["totalSupply"]: "0x" + word(SUPPLY),
            pft_token.SELECTORS["balanceOf"]: "0x" + word(self.balance),
        }
        return self.overrides.get(selector, answers[selector])

    def urlopen(self, request, timeout=None):  # noqa: ANN001
        body = json.loads(request.data)
        self.methods.append(body["method"])
        answer = {"jsonrpc": "2.0", "id": body["id"], "result": self.result(body["method"], body["params"])}
        return contextlib.closing(io.BytesIO(json.dumps(answer).encode()))


class PftTokenTests(unittest.TestCase):
    def setUp(self) -> None:
        self.base = FakeBase()
        patcher = mock.patch.object(pft_token.urllib.request, "urlopen", side_effect=self.base.urlopen)
        patcher.start()
        self.addCleanup(patcher.stop)
        directory = tempfile.TemporaryDirectory()
        self.addCleanup(directory.cleanup)
        self.record = Path(directory.name) / "deployment.json"
        self.record.write_text(
            json.dumps({"chain_id": 84532, "token": TOKEN, "treasury": TREASURY, "symbol": "PFT"}), encoding="utf-8"
        )

    def run_main(self, *argv: str) -> tuple[int, str, str]:
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = pft_token.main(list(argv))
        return code, stdout.getvalue(), stderr.getvalue()

    def test_record_path_reads_the_whole_state(self) -> None:
        code, out, err = self.run_main("--record", str(self.record))
        self.assertEqual((code, err), (0, ""))
        state = json.loads(out)
        self.assertEqual(state["chain_id"], 84532)
        self.assertEqual(state["rpc_url"], "https://sepolia.base.org")
        self.assertEqual((state["token"], state["treasury"]), (TOKEN, TREASURY))
        self.assertEqual((state["name"], state["symbol"], state["decimals"]), ("Post Fiat Token", "PFT", 6))
        self.assertEqual(state["total_supply_units"], str(SUPPLY))
        self.assertEqual(state["treasury_balance_units"], str(SUPPLY))
        self.assertTrue(state["treasury_holds_whole_supply"])
        self.assertEqual((state["code_size_bytes"], state["block_number"]), (5, BLOCK))
        self.assertRegex(state["read_at"], r"^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$")
        self.assertEqual(self.base.methods[:3], ["eth_chainId", "eth_blockNumber", "eth_getCode"])

    def test_token_path_needs_a_treasury_and_a_base_chain(self) -> None:
        code, _, err = self.run_main("--token", TOKEN)
        self.assertEqual(code, 1)
        self.assertIn("--treasury is required", err)
        state = pft_token.read_token_state(token=TOKEN, treasury=TREASURY)
        self.assertEqual((state["token"], state["record"]), (TOKEN, None))
        self.base.chain_id = 8453
        self.assertEqual(pft_token.read_token_state(token=TOKEN, treasury=TREASURY)["chain_id"], 8453)

    def test_chain_id_mismatch_exits_2(self) -> None:
        self.base.chain_id = 8453
        code, out, err = self.run_main("--record", str(self.record))
        self.assertEqual((code, out), (2, ""))
        self.assertIn("on chain 8453, expected 84532", err)
        self.base.chain_id = 1
        code, _, err = self.run_main("--token", TOKEN, "--treasury", TREASURY)
        self.assertEqual(code, 2)
        self.assertIn("expected 8453 or 84532", err)

    def test_token_without_code_exits_2(self) -> None:
        self.base.code = "0x"
        code, _, err = self.run_main("--record", str(self.record))
        self.assertEqual(code, 2)
        self.assertIn("has no code", err)
        self.assertNotIn("eth_call", self.base.methods)

    def test_treasury_without_the_whole_supply(self) -> None:
        self.base.balance = SUPPLY - 1_500_000
        state = pft_token.read_token_state(record=self.record)
        self.assertFalse(state["treasury_holds_whole_supply"])
        self.assertEqual(state["treasury_balance_pft"], "99999999998.5")

    def test_table_output(self) -> None:
        code, out, _ = self.run_main("--record", str(self.record), "--table")
        self.assertEqual(code, 0)
        lines = out.splitlines()
        self.assertEqual([line.split()[0] for line in lines], list(pft_token.TABLE_FIELDS))
        self.assertIn("total_supply_pft             100000000000", lines)
        self.assertIn("treasury_holds_whole_supply  true", lines)
        self.assertEqual(len({line.index(line.split()[1]) for line in lines}), 1)

    def test_decimal_conversion(self) -> None:
        self.assertEqual(pft_token.units_to_decimal(SUPPLY, 6), "100000000000")
        self.assertEqual(pft_token.units_to_decimal(1, 6), "0.000001")
        self.assertEqual(pft_token.units_to_decimal(1_230_000, 6), "1.23")
        self.assertEqual(pft_token.units_to_decimal(7, 0), "7")
        self.assertEqual(pft_token.read_token_state(record=self.record)["total_supply_pft"], "100000000000")

    def test_malformed_answers_exit_2(self) -> None:
        for selector, answer in (
            (pft_token.SELECTORS["decimals"], "0x" + word(256)),
            (pft_token.SELECTORS["totalSupply"], "0x1234"),
            (pft_token.SELECTORS["name"], "0x" + word(32) + word(64)),
            (pft_token.SELECTORS["symbol"], "not hex"),
        ):
            with self.subTest(selector=selector):
                self.base.overrides = {selector: answer}
                code, out, err = self.run_main("--record", str(self.record))
                self.assertEqual((code, out), (2, ""))
                self.assertTrue(err.startswith("pft-token-refused: "), err)

    def test_rpc_errors_exit_1_without_a_traceback(self) -> None:
        def fail(request, timeout=None):  # noqa: ANN001
            raise urllib.error.URLError("connection refused")

        with mock.patch.object(pft_token.urllib.request, "urlopen", side_effect=fail):
            code, _, err = self.run_main("--record", str(self.record))
        self.assertEqual(code, 1)
        self.assertEqual(err, "pft-token-failed: eth_chainId: the RPC request failed: connection refused\n")

        def rpc_error(request, timeout=None):  # noqa: ANN001
            body = json.loads(request.data)
            answer = {"jsonrpc": "2.0", "id": body["id"], "error": {"code": -32000, "message": "header not found"}}
            return contextlib.closing(io.BytesIO(json.dumps(answer).encode()))

        with mock.patch.object(pft_token.urllib.request, "urlopen", side_effect=rpc_error):
            code, _, err = self.run_main("--record", str(self.record))
        self.assertEqual(code, 1)
        self.assertIn("RPC error: header not found", err)

    def test_rpc_url_is_redacted_after_the_host(self) -> None:
        self.assertEqual(pft_token.redact_rpc_url("https://sepolia.base.org"), "https://sepolia.base.org")
        self.assertEqual(pft_token.redact_rpc_url("https://base.example/v2/KEY?x=1"), "https://base.example/…")
        self.assertEqual(pft_token.redact_rpc_url("http://user:pw@127.0.0.1:8545"), "http://127.0.0.1:8545/…")
        # An IPv6 host keeps its brackets, and any userinfo is marked hidden (#176).
        self.assertEqual(pft_token.redact_rpc_url("http://[::1]:8545"), "http://[::1]:8545")
        self.assertEqual(pft_token.redact_rpc_url("https://[2001:db8::1]/v2/KEY"), "https://[2001:db8::1]/…")
        self.assertEqual(pft_token.redact_rpc_url("https://:pw@base.example"), "https://base.example/…")
        self.assertEqual(pft_token.redact_rpc_url("http://user:pw@[::1]:8545"), "http://[::1]:8545/…")
        state = pft_token.read_token_state("http://[::1]:8545", record=self.record)
        self.assertEqual(state["rpc_url"], "http://[::1]:8545")
        state = pft_token.read_token_state("https://base.example/v2/KEY", record=self.record)
        self.assertEqual(state["rpc_url"], "https://base.example/…")
        code, _, err = self.run_main("--record", str(self.record), "--rpc-url", "file:///etc/passwd")
        self.assertEqual(code, 1)
        self.assertIn("http(s) URL", err)


if __name__ == "__main__":
    unittest.main()
