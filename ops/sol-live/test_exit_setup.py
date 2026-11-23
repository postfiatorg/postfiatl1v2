import unittest

from exit_setup import config_from_records, build_initialize, MAINNET_PAYER, VERIFIER, LIGHT, BUCKET


class SetupTest(unittest.TestCase):
    def test_checked_checkpoint_and_route_are_required(self):
        with self.assertRaises(ValueError):
            config_from_records({}, {}, {}, {})

    def test_account_order_and_signers(self):
        from pathlib import Path
        import json
        config = json.loads((Path(__file__).parent / "fixtures/exit-setup.json").read_text())
        ix = build_initialize(config, MAINNET_PAYER)
        self.assertEqual(ix.program_id, VERIFIER)
        self.assertEqual([str(a.pubkey) for a in ix.accounts[:3]], [str(MAINNET_PAYER), str(VERIFIER), str(LIGHT)])
        self.assertEqual([a.is_signer for a in ix.accounts], [True, True, False, False, False])
        self.assertEqual((ix.data[0], len(ix.data)), (3,317))
