#!/usr/bin/env python3

from __future__ import annotations

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch


SCRIPT = Path(__file__).with_name("pnok-private-fix-demo.py")
SPEC = importlib.util.spec_from_file_location("pnok_private_fix_demo", SCRIPT)
assert SPEC is not None and SPEC.loader is not None
demo = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(demo)


class PnokPrivateFixDemoTests(unittest.TestCase):
    def test_persist_progress_never_regresses_a_recovered_finalized_intent(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "private").mkdir()
            (root / "public").mkdir()
            state = {
                "schema": demo.SCHEMA,
                "stage": "finalized",
                "immutable": {
                    "intent_id": "pnok-run-01",
                    "chain_id": demo.EXPECTED_CHAIN_ID,
                    "genesis_hash": demo.EXPECTED_GENESIS_HASH,
                },
                "derived": {},
                "last_error": {"type": "RuntimeError", "message": "prior interruption"},
            }

            demo.persist_progress(root, state, "quote_verified")

            persisted = json.loads((root / "private/intent.json").read_text())
            self.assertEqual(persisted["stage"], "finalized")
            self.assertIsNone(persisted["last_error"])
            public = json.loads((root / "public/status.json").read_text())
            self.assertEqual(public["stage"], "finalized")

    def test_reservation_id_matches_canonical_domain_and_fields(self) -> None:
        operation = {
            "operator": "pf" + "11" * 20,
            "fix_packet_hash": "22" * 48,
            "action_binding_hash": "33" * 64,
            "base_atoms": 20_000_000,
            "quote_atoms": 210,
            "wallet_intent_hash": "44" * 48,
            "reservation_nonce": "55" * 48,
        }
        self.assertEqual(
            demo.reservation_id(operation),
            "b1ca42c657d42037af4c701b9b22e88e2c904c7181f5650af3237c04a59aa9cc"
            "5cd4c62e302b337f9aa6370330feac47",
        )

    def test_public_status_excludes_private_owner_amount_and_note_handles(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            state = {
                "schema": demo.SCHEMA,
                "stage": "action_built",
                "immutable": {
                    "intent_id": "pnok-run-01",
                    "chain_id": demo.EXPECTED_CHAIN_ID,
                    "genesis_hash": demo.EXPECTED_GENESIS_HASH,
                    "wallet_address": "pf" + "11" * 20,
                    "facility_operator": "pf" + "22" * 20,
                    "facility_key_file": "/secret/facility.json",
                    "base_atoms": 20_000_000,
                    "expected_quote_atoms": 210,
                    "wallet_note_commitment": "33" * 32,
                    "liquidity_commitment": "44" * 32,
                },
                "derived": {
                    "fix_packet_hash": "55" * 48,
                    "fix_source_label": "pnok_demo_fix",
                    "action_binding_hash": "66" * 64,
                },
                "updated_at_unix_ms": 1,
            }
            demo.publish_redacted_status(root, state)
            encoded = (root / "public/status.json").read_text()
            for private_value in (
                "pf" + "11" * 20,
                "pf" + "22" * 20,
                "/secret/facility.json",
                "20000000",
                "210",
                "33" * 32,
                "44" * 32,
            ):
                self.assertNotIn(private_value, encoded)

    def test_retried_action_reuses_identical_request_fingerprint_fields(self) -> None:
        quote = {
            "pricing_claim": {
                "nav_epoch": 1,
                "reserve_packet_hash": "55" * 48,
                "ratio_numerator": 21,
                "ratio_denominator": 2_000_000,
                "mode": "negotiated",
                "band_bps": 0,
                "base_asset_tag_lo": "01" * 16,
                "base_asset_tag_hi": "02" * 16,
                "quote_asset_tag_lo": "03" * 16,
                "quote_asset_tag_hi": "04" * 16,
            }
        }
        action = {
            "schema": "postfiat-asset-orchard-swap-action-v2",
            "pool_id": "asset-orchard-v1",
            "proof_system_id": "postfiat.privacy.asset-orchard-halo2.v1",
            "circuit_id": "asset_orchard.swap.pricing_bound.v4",
            "nullifiers": ["11" * 32, "22" * 32],
            "output_commitments": ["33" * 32, "44" * 32],
            "pricing_claim": quote["pricing_claim"],
            "swap_binding_hash": "66" * 64,
            "fee": 0,
        }
        response = {
            "ok": True,
            "schema": "postfiat-asset-orchard-local-swap-action-v1",
            "request_id": "pnok-run-01",
            "verification": {"verified": True},
            "action_json": json.dumps(action),
            "swap_id": "77" * 48,
            "vault_update": {
                "wallet_output_commitment": "33" * 32,
                "pool_output_commitment": "44" * 32,
            },
        }
        state = {
            "immutable": {
                "intent_id": "pnok-run-01",
                "wallet_address": "pf" + "11" * 20,
                "facility_operator": "pf" + "22" * 20,
                "base_asset_id": "88" * 48,
                "quote_asset_id": "99" * 48,
                "base_atoms": 20_000_000,
                "expected_quote_atoms": 210,
                "wallet_note_commitment": "aa" * 32,
                "liquidity_commitment": "bb" * 32,
                "wallet_input_note_path": "/resident/imports/bob-pfusdc.json",
                "liquidity_input_note_path": "/resident/imports/facility-pnok.json",
            },
            "derived": {},
        }
        args = type(
            "Args",
            (),
            {
                "local_quote_lifetime_ms": 900_000,
                "service_url": "http://127.0.0.1:1",
                "prover_timeout_seconds": 1,
            },
        )()
        requests = []

        def fake_service(_url, _method, _path, body, _timeout):
            requests.append(body)
            return response

        with tempfile.TemporaryDirectory() as temporary, patch.object(
            demo, "service_json", side_effect=fake_service
        ):
            root = Path(temporary)
            demo.create_or_recover_action(args, root, state, quote)
            demo.create_or_recover_action(args, root, state, quote)

        self.assertEqual(requests[0], requests[1])
        self.assertEqual(requests[0]["wallet_commitment"], "aa" * 32)
        self.assertEqual(requests[0]["liquidity_commitment"], "bb" * 32)
        self.assertEqual(requests[0]["liquidity_wallet_address"], "pf" + "22" * 20)
        self.assertEqual(
            requests[0]["input_note_path_a"], "/resident/imports/bob-pfusdc.json"
        )
        self.assertEqual(
            requests[0]["input_note_path_b"], "/resident/imports/facility-pnok.json"
        )


if __name__ == "__main__":
    unittest.main()
