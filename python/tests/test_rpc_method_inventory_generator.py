"""Unit tests for the Python-coverage detector of scripts/testnet-rpc-method-inventory.

The freshness test checks the committed artifacts against the generator; this
file checks the generator's own extractor against known wrapper shapes, so a
wrapper style the detector does not recognise fails here instead of silently
reporting python_client=false in the launch-surface map (or, worse, a literal
that is not a method name becoming a phantom inventory row).
"""

from __future__ import annotations

import importlib.machinery
import importlib.util
import re
import tempfile
import unittest
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
GENERATOR = REPO / "scripts" / "testnet-rpc-method-inventory"
REAL_CLIENT = REPO / "python" / "postfiat_rpc" / "client.py"


def _load_generator():
    loader = importlib.machinery.SourceFileLoader("testnet_rpc_method_inventory_gen", str(GENERATOR))
    spec = importlib.util.spec_from_loader(loader.name, loader)
    module = importlib.util.module_from_spec(spec)
    loader.exec_module(module)
    return module


HELPERS = '''
    def _object_read(self, method: str, params: dict[str, Any]) -> dict[str, Any]:
        result = self._call(method, params)
        return result

    def _verification_report(self, method: str) -> dict[str, Any]:
        return self._object_read(method, {})

    def _emit(self, event: str) -> None:
        pass
'''


class PythonClientDetectionTests(unittest.TestCase):
    def setUp(self) -> None:
        self.generator = _load_generator()
        self._tmp = tempfile.TemporaryDirectory()
        self.addCleanup(self._tmp.cleanup)

    def _detect(self, source: str) -> set[str]:
        """Run the detector against `source` as if it were client.py."""
        fake_client = Path(self._tmp.name) / "client.py"
        fake_client.write_text(source, encoding="utf-8")
        self.generator.PY_CLIENT = fake_client
        return self.generator.extract_python_client_methods()

    def test_detects_single_line_transport_calls(self) -> None:
        source = '''
    def status(self) -> dict[str, Any]:
        return self._call("status")

    def blocks(self, limit: int) -> list[dict[str, Any]]:
        return self._call("blocks", self._limit_params(limit))
'''
        self.assertEqual(self._detect(source), {"status", "blocks"})

    def test_detects_transport_calls_with_the_literal_on_the_next_line(self) -> None:
        # black wraps the longer calls like this; client.py on main has 15 such
        # sites (mempool_submit_*, owned_sign*, navcoin_bridge_packet, ...).
        source = '''
    def submit_signed_transfer(self, signed_transfer_json: str) -> dict[str, Any]:
        result = self._call(
            "mempool_submit_signed_transfer",
            {"signed_transfer_json": signed_transfer_json},
        )
        return result
'''
        self.assertEqual(self._detect(source), {"mempool_submit_signed_transfer"})

    def test_detects_calls_routed_through_forwarding_helpers(self) -> None:
        source = HELPERS + '''
    def vault_bridge_status(self, asset_id: str) -> dict[str, Any]:
        return self._object_read(
            "vault_bridge_status", {"asset_id": self._required_text(asset_id, "asset_id")}
        )

    def nav_reserve_proof_status(self, asset_id: str) -> dict[str, Any]:
        return self._object_read("nav_reserve_proof_status", {"asset_id": asset_id})

    def verify_blocks(self) -> dict[str, Any]:
        return self._verification_report("verify_blocks")
'''
        self.assertEqual(
            self._detect(source),
            {"vault_bridge_status", "nav_reserve_proof_status", "verify_blocks"},
        )

    def test_ignores_literals_handed_to_non_forwarding_helpers(self) -> None:
        # `_required_text` takes the value first, `_emit` takes an event name:
        # neither forwards a method, so "owner" and "started" must not appear.
        source = HELPERS + '''
    def shield_scan(self, owner: str) -> list[dict[str, Any]]:
        self._emit("started")
        return self._call("shield_scan", {"owner": self._required_text(owner, "owner")})

    def helper(self, value: str) -> str:
        return self._required_text(value, "value")
'''
        self.assertEqual(self._detect(source), {"shield_scan"})

    def test_source_parameter_overrides_the_client_file(self) -> None:
        self.generator.PY_CLIENT = Path(self._tmp.name) / "does-not-exist.py"
        self.assertEqual(self.generator.extract_python_client_methods(), set())
        self.assertEqual(
            self.generator.extract_python_client_methods('return self._call("status")'),
            {"status"},
        )

    def test_real_client_detection_is_a_superset_of_the_single_line_form(self) -> None:
        source = REAL_CLIENT.read_text(encoding="utf-8")
        single_line = set(re.findall(r'self\._call\("([^"]+)"', source))
        detected = self._detect(source)
        self.assertTrue(single_line, "expected single-line self._call(...) wrappers in client.py")
        self.assertEqual(single_line - detected, set())
        # The multi-line sites are the ones the single-line form misses.
        self.assertIn("mempool_submit_signed_transfer", detected)


if __name__ == "__main__":
    unittest.main()
