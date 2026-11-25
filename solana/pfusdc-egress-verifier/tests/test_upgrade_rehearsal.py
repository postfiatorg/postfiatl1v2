"""The rehearsal's own checks, without a validator: run with python3 -m unittest from tests/."""
import base64
import json
import tempfile
import unittest
from pathlib import Path

import upgrade_rehearsal as ur

PINNED = bytes.fromhex((ur.HERE / "mainnet-light.hex").read_text().strip())


def at_height(raw, height):
    return raw[:280] + height.to_bytes(8, "little") + raw[288:]


class DumpedLightTest(unittest.TestCase):
    def snapshot(self, raw):
        snap = Path(self.dir.name)
        (snap / "accounts").mkdir()
        value = {"lamports": 1, "data": [base64.b64encode(raw).decode(), "base64"], "owner": "x", "executable": False}
        (snap / "accounts" / f"{ur.LIGHT}.json").write_text(json.dumps(ur.account_file(str(ur.LIGHT), value)))
        return snap

    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()

    def tearDown(self):
        self.dir.cleanup()

    def test_the_loaded_light_is_compared_with_the_light_the_snapshot_dumped(self):
        moved = at_height(PINNED, 1853)
        outcome, expected = ur.light_check(moved, self.snapshot(moved))
        self.assertEqual(outcome, expected)
        self.assertEqual(outcome["height"], 1853)

    def test_a_light_that_differs_from_the_dump_names_both_checkpoint_heights(self):
        outcome, expected = ur.light_check(PINNED, self.snapshot(at_height(PINNED, 1853)))
        self.assertNotEqual(outcome, expected)
        self.assertEqual((outcome["height"], expected["height"]), (1851, 1853))

    def test_the_pinned_fixture_is_the_light_at_1851(self):
        self.assertEqual(ur.light_height(PINNED), 1851)


if __name__ == "__main__":
    unittest.main()
