"""Nothing can be set up or rehearsed against the deployed single-bucket verifier."""

from pathlib import Path
import unittest
from unittest.mock import patch

import exit_setup
import rehearse_exit

OLD = "3b16fc309501814e0bba718ecadb80f02993f6f355153a13f37ae9d47d29354c"


class VerifierPinTest(unittest.TestCase):
    def test_setup_refuses_while_no_hash_is_pinned(self):
        with patch.object(exit_setup, "VERIFIER_SHA256", None):
            with self.assertRaisesRegex(ValueError, "the rebuilt verifier's hash is not pinned yet"):
                exit_setup.verifier_pin()

    def test_rehearsal_refuses_while_no_hash_is_pinned(self):
        with patch.object(exit_setup, "VERIFIER_SHA256", None):
            with self.assertRaisesRegex(ValueError, "the rebuilt verifier's hash is not pinned yet"):
                rehearse_exit.image_pins()

    def test_the_pin_is_the_upgraded_program_data_of_24_november(self):
        # sha256 of the 123,152-byte build 4091ee16... zero-filled to the 127,168-byte ProgramData tail.
        self.assertEqual(exit_setup.verifier_pin(), "0x3406becc6db7ba7c250f8db55dc43965656093bb4ceb3e330d2f452edd465fdf")

    def test_no_code_pins_the_old_verifier(self):
        for path in Path(__file__).parent.glob("*.py"):
            if path.name == Path(__file__).name:
                continue
            for line in path.read_text().splitlines():
                if OLD in line:
                    self.assertTrue(line.lstrip().startswith("#"), f"{path.name}: {line}")


if __name__ == "__main__":
    unittest.main()
