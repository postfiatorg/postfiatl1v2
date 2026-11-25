"""Nothing can be set up or rehearsed against the deployed single-bucket verifier."""

import hashlib
import os
from pathlib import Path
import unittest
from unittest.mock import patch

import exit_setup
import rehearse_exit

OLD = "3b16fc309501814e0bba718ecadb80f02993f6f355153a13f37ae9d47d29354c"
ELF = Path(__file__).resolve().parents[2] / "solana/pfusdc-egress-verifier/target/deploy/pfusdc_egress_verifier.so"


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
        # Derived, not restated: the reproducible build's ELF (sha256 4091ee16...), zero-filled to the
        # 127,168-byte ProgramData tail, hashes to the pin.
        elf = Path(os.environ.get("EXIT_VERIFIER_ELF", ELF))
        if not elf.is_file():
            self.skipTest(f"the reproducible verifier build is not on this machine ({elf}); "
                          "run build-exit-verifier.sh or set EXIT_VERIFIER_ELF")
        data = elf.read_bytes()
        self.assertEqual(hashlib.sha256(data).hexdigest(), exit_setup.VERIFIER_ELF_SHA256)
        self.assertEqual(exit_setup.program_data_pin(data), exit_setup.verifier_pin())

    def test_the_pin_hashes_the_elf_zero_filled_to_the_program_data_size(self):
        elf = b"\x7fELF" + bytes(range(256)) * 3
        want = hashlib.sha256(elf + bytes(exit_setup.PROGRAM_DATA_SIZE - len(elf))).hexdigest()
        self.assertEqual(exit_setup.program_data_pin(elf), "0x" + want)
        self.assertNotEqual(exit_setup.program_data_pin(elf + b"\0"), "0x" + hashlib.sha256(elf).hexdigest())
        with self.assertRaisesRegex(ValueError, "larger than the program data"):
            exit_setup.program_data_pin(bytes(exit_setup.PROGRAM_DATA_SIZE + 1))

    def test_no_code_pins_the_old_verifier(self):
        for path in Path(__file__).parent.glob("*.py"):
            if path.name == Path(__file__).name:
                continue
            for line in path.read_text().splitlines():
                if OLD in line:
                    self.assertTrue(line.lstrip().startswith("#"), f"{path.name}: {line}")


if __name__ == "__main__":
    unittest.main()
