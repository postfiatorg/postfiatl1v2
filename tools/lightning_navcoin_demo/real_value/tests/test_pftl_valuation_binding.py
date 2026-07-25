from __future__ import annotations

import hashlib
import json
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest

from ..pftl_valuation_binding import (
    PftlValuationBinding,
    PftlValuationBindingError,
)


CHAIN_ID = "local-pftl-proven-nav-v2-20260724"
GENESIS = "81" * 48
TIP = "c3" * 48
ROOT = "08" * 48
ASSET_ID = "f9" * 48
RESERVE_HASH = "02" * 48
PROFILE_ID = "1f" * 48
ISSUER = "pf" + "b5" * 20
NAV_PER_UNIT_USD_E8 = 1_035_074_022
SUPPLY = 3_000_000_000
HEIGHT = 12


class PftlValuationBindingTests(unittest.TestCase):
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.handoff = SimpleNamespace(
            data_root=self.root,
            chain_id=CHAIN_ID,
            genesis_hash=GENESIS,
            asset_id=ASSET_ID,
            asset_issuer=ISSUER,
            profile_id=PROFILE_ID,
            nav_epoch=1,
            nav_per_unit_usd_e8=NAV_PER_UNIT_USD_E8,
            reserve_packet_hash=RESERVE_HASH,
            circulating_supply_atoms=SUPPLY,
            binary_path=self.root / "unused-test-binary",
            binary_sha256="00" * 32,
        )
        self.snapshot = SimpleNamespace(
            height=HEIGHT,
            block_tip_hash=TIP,
            state_root=ROOT,
            agreeing_validator_count=6,
            validator_count=6,
            asset_id=ASSET_ID,
            nav_epoch=1,
            nav_per_unit=NAV_PER_UNIT_USD_E8,
            nav_reserve_packet_hash=RESERVE_HASH,
        )
        for index in range(6):
            self._write_node(index)
        self.verifier_calls: list[int] = []

    def _binding(self, handoff: object | None = None) -> PftlValuationBinding:
        def verify(index: int, _node: Path, _snapshot: object) -> str:
            self.verifier_calls.append(index)
            return hashlib.sha256(f"verified-{index}".encode()).hexdigest()

        return PftlValuationBinding(
            handoff or self.handoff,
            state_verifier=verify,
        )

    def _write_node(
        self,
        index: int,
        *,
        valuation_unit: str = "usd_1e8",
        nav_per_unit: int = NAV_PER_UNIT_USD_E8,
        height: int = HEIGHT,
        state_root: str = ROOT,
    ) -> None:
        node = self.root / "nodes" / f"validator-{index}"
        node.mkdir(parents=True, exist_ok=True)
        (node / "chain_tip.json").write_text(
            json.dumps(
                {
                    "schema": "postfiat-chain-tip-v1",
                    "chain_id": CHAIN_ID,
                    "genesis_hash": GENESIS,
                    "height": height,
                    "block_hash": TIP,
                    "state_root": state_root,
                }
            ),
            encoding="ascii",
        )
        (node / "ledger.json").write_text(
            json.dumps(
                {
                    "nav_assets": [
                        {
                            "asset_id": ASSET_ID,
                            "issuer": ISSUER,
                            "proof_profile": PROFILE_ID,
                            "valuation_unit": valuation_unit,
                            "finalized_epoch": 1,
                            "nav_per_unit": nav_per_unit,
                            "circulating_supply": SUPPLY,
                            "finalized_reserve_packet_hash": RESERVE_HASH,
                            "halted": False,
                            "finalized_at_height": 5,
                        }
                    ]
                }
            ),
            encoding="ascii",
        )

    def test_binds_exact_six_ledgers_to_rpc_tip_and_usd_e8(self) -> None:
        binding = self._binding()
        evidence = binding.verify(self.snapshot)
        self.assertEqual(evidence.validator_count, 6)
        self.assertEqual(evidence.nav_per_unit_usd_e8, NAV_PER_UNIT_USD_E8)
        self.assertEqual(evidence.valuation_scale, 100_000_000)
        self.assertEqual(len(evidence.ledger_sha256), 6)
        self.assertEqual(len(evidence.chain_tip_sha256), 6)
        self.assertEqual(len(evidence.state_verification_sha256), 6)
        self.assertEqual(sorted(self.verifier_calls), list(range(6)))
        self.assertIs(binding.verify(self.snapshot), evidence)
        self.assertEqual(len(self.verifier_calls), 6)

    def test_usdc_e6_interpretation_is_rejected(self) -> None:
        self._write_node(3, valuation_unit="usdc_1e6")
        with self.assertRaisesRegex(
            PftlValuationBindingError,
            "USD-e8 pin",
        ):
            self._binding().verify(self.snapshot)

    def test_usd_label_cannot_hide_a_rescaled_raw_value(self) -> None:
        self._write_node(4, nav_per_unit=10_350_740)
        with self.assertRaisesRegex(
            PftlValuationBindingError,
            "USD-e8 pin",
        ):
            self._binding().verify(self.snapshot)

    def test_local_tip_must_match_the_six_rpc_snapshot(self) -> None:
        self._write_node(5, height=HEIGHT + 1, state_root="ff" * 48)
        with self.assertRaisesRegex(
            PftlValuationBindingError,
            "does not match",
        ):
            self._binding().verify(self.snapshot)

    def test_data_root_cannot_traverse_a_symlink(self) -> None:
        link = self.root.parent / f"{self.root.name}-link"
        link.symlink_to(self.root, target_is_directory=True)
        self.addCleanup(link.unlink)
        linked_handoff = SimpleNamespace(**vars(self.handoff))
        linked_handoff.data_root = link
        with self.assertRaisesRegex(
            PftlValuationBindingError,
            "canonical",
        ):
            self._binding(linked_handoff)

    def test_world_writable_ledger_is_rejected(self) -> None:
        ledger = self.root / "nodes" / "validator-2" / "ledger.json"
        ledger.chmod(0o666)
        with self.assertRaisesRegex(
            PftlValuationBindingError,
            "world writable",
        ):
            self._binding().verify(self.snapshot)

    def test_pinned_binary_verify_state_must_match_rpc_root(self) -> None:
        report = {
            "schema": "postfiat-state-verification-v1",
            "verified": True,
            "chain_id": CHAIN_ID,
            "genesis_hash": GENESIS,
            "protocol_version": 1,
            "block_log": {
                "verified": True,
                "block_count": HEIGHT,
                "tip_hash": TIP,
                "state_root": ROOT,
            },
        }
        binary = self.root / "verify-state-test"
        binary.write_text(
            "#!/usr/bin/python3\n"
            "import json\n"
            f"print(json.dumps({report!r}))\n",
            encoding="ascii",
        )
        binary.chmod(0o700)
        self.handoff.binary_path = binary
        self.handoff.binary_sha256 = hashlib.sha256(binary.read_bytes()).hexdigest()
        evidence = PftlValuationBinding(self.handoff).verify(self.snapshot)
        self.assertEqual(len(evidence.state_verification_sha256), 6)

        binary.write_text(
            "#!/usr/bin/python3\n"
            "import json\n"
            f"value = {report!r}\n"
            "value['block_log']['state_root'] = 'ff' * 48\n"
            "print(json.dumps(value))\n",
            encoding="ascii",
        )
        binary.chmod(0o700)
        self.handoff.binary_sha256 = hashlib.sha256(binary.read_bytes()).hexdigest()
        with self.assertRaisesRegex(
            PftlValuationBindingError,
            "does not match",
        ):
            PftlValuationBinding(self.handoff).verify(self.snapshot)


if __name__ == "__main__":
    unittest.main()
