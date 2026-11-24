"""The tag-3 route binds the shared custody, not one ingress revision's bucket."""

import hashlib
import json
import sys
import unittest
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE.parents[1] / "solana/pfusdc-egress-verifier"))
from bootstrap import encode, packet_alias  # noqa: E402

CONFIG = json.loads((HERE / "fixtures/exit-setup.json").read_text())
MINT = bytes.fromhex("c6fa7af3bedbad3a3d65f36aabc97431b1bbe4c2d2f6e0e47ca60203452f5d61")


def route(config):
    return encode(config)[116:148]


class ExitRouteTest(unittest.TestCase):
    def test_route_is_the_same_for_every_ingress_revision(self):
        later = dict(CONFIG, ingress_profile_hash="09" * 48, ingress_epoch=9)
        earlier = dict(CONFIG, ingress_profile_hash="03" * 48, ingress_epoch=3)
        self.assertEqual(route(CONFIG), route(later))
        self.assertEqual(route(CONFIG), route(earlier))

    def test_route_preimage_matches_the_verifier(self):
        vault = bytes.fromhex(CONFIG["vault_program"])
        preimage = (
            bytes.fromhex(CONFIG["exit_profile_hash"])
            + CONFIG["exit_epoch"].to_bytes(8, "big")
            + bytes.fromhex(CONFIG["asset_id"])
            + b"SOLANA01"
            + packet_alias(vault)
            + bytes.fromhex(CONFIG["vault_code_hash"].removeprefix("0x"))
            + packet_alias(MINT)
            + bytes.fromhex(CONFIG["token_code_hash"].removeprefix("0x"))
        )
        self.assertEqual(route(CONFIG), hashlib.sha256(preimage).digest())


    def test_route_matches_the_shared_cross_language_vector(self):
        # The same vector is checked by solana/pfusdc-egress-verifier/src/values.rs.
        text = (HERE.parents[1] / "solana/pfusdc-egress-verifier/tests/route-vector.txt")
        vector = dict(
            line.split("=", 1)
            for line in text.read_text().splitlines()
            if line and not line.startswith("#")
        )
        vault = bytes.fromhex(CONFIG["vault_program"])
        self.assertEqual(
            [vector[f"f{i}"] for i in (6, 7, 23, 32, 33, 34, 35, 36)],
            [
                CONFIG["exit_profile_hash"],
                CONFIG["exit_epoch"].to_bytes(8, "big").hex(),
                CONFIG["asset_id"],
                b"SOLANA01".hex(),
                packet_alias(vault).hex(),
                CONFIG["vault_code_hash"].removeprefix("0x"),
                packet_alias(MINT).hex(),
                CONFIG["token_code_hash"].removeprefix("0x"),
            ],
        )
        self.assertEqual(route(CONFIG).hex(), vector["route"])


if __name__ == "__main__":
    unittest.main()
