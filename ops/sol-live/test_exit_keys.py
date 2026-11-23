import os
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch


class ExitKeysTest(unittest.TestCase):
    def test_reject_unsafe_key_before_read(self):
        from exit_keys import create_key, load

        with tempfile.TemporaryDirectory() as tmp:
            p = Path(tmp) / "payer"
            address = create_key(p).pubkey()
            self.assertEqual(load(p).pubkey(), address)
            self.assertEqual(p.stat().st_mode & 0o777, 0o600)
            link = p.with_suffix(".link")
            link.symlink_to(p)
            with self.assertRaises((OSError, ValueError)):
                load(link)
            with patch("exit_keys.os.geteuid", return_value=os.geteuid() + 1):
                with self.assertRaises(ValueError):
                    load(p)
            p.chmod(0o640)
            with self.assertRaises(ValueError):
                load(p)
            with self.assertRaises(ValueError):
                load(Path(tmp))

    def test_unsigned_cluster_and_distinct_roles(self):
        import exit_setup as s

        with (
            patch.object(s.ExitCluster, "call", return_value=s.GENESIS),
            patch("exit_keys.load", side_effect=AssertionError),
        ):
            c = s.ExitCluster(s.MAINNET_PAYER)
            self.assertIsNone(c.payer)
        self.assertEqual(len({s.MAINNET_PAYER, s.FEE_PAYER, s.PAUSE, s.VERIFIER}), 4)
        self.assertEqual(s.read(s.CONFIG)["pause_authority"], bytes(s.PAUSE).hex())

    def test_payer_cannot_be_a_current_upgrade_authority(self):
        import base64
        from unittest.mock import Mock
        import exit_setup as s
        c=Mock()
        def loader(authority):
            raw=(3).to_bytes(4,"little")+bytes(8)+b"\x01"+bytes(authority)
            return dict(owner=str(s.LOADER),data=[base64.b64encode(raw).decode(),"base64"])
        c.call.return_value={"value":[loader(s.MAINNET_PAYER),loader(s.FEE_PAYER)]}
        with self.assertRaises(ValueError):s.check_payout_authorities(c)
        c.call.return_value={"value":[loader(s.MAINNET_PAYER)]*2}
        s.check_payout_authorities(c)
