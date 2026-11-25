"""Nothing can be set up or rehearsed against the deployed single-bucket verifier."""

from pathlib import Path
import unittest

import exit_setup
import rehearse_exit

OLD = "3b16fc309501814e0bba718ecadb80f02993f6f355153a13f37ae9d47d29354c"


class VerifierPinTest(unittest.TestCase):
    def test_setup_refuses_until_the_rebuilt_hash_is_pinned(self):
        self.assertIsNone(exit_setup.VERIFIER_SHA256)
        with self.assertRaisesRegex(ValueError, "the rebuilt verifier's hash is not pinned yet"):
            exit_setup.verifier_pin()

    def test_rehearsal_refuses_until_the_rebuilt_hash_is_pinned(self):
        with self.assertRaisesRegex(ValueError, "the rebuilt verifier's hash is not pinned yet"):
            rehearse_exit.image_pins()

    def test_no_code_pins_the_old_verifier(self):
        for path in Path(__file__).parent.glob("*.py"):
            if path.name == Path(__file__).name:
                continue
            for line in path.read_text().splitlines():
                if OLD in line:
                    self.assertTrue(line.lstrip().startswith("#"), f"{path.name}: {line}")


if __name__ == "__main__":
    unittest.main()
