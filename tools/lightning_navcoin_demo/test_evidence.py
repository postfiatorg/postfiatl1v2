from __future__ import annotations

import hashlib
from pathlib import Path
import tempfile
import unittest

from tools.lightning_navcoin_demo.evidence import (
    EvidenceBundle,
    EvidenceError,
    REDACTED,
    verify_bundle,
)
from tools.lightning_navcoin_demo.safety import SafetyEnvelope, SafetyViolation


class SafetyEnvelopeTests(unittest.TestCase):
    def valid(self, root: Path) -> SafetyEnvelope:
        return SafetyEnvelope(
            bitcoin_network="regtest",
            pftl_chain_id="local-lightning-demo",
            pftl_genesis_hash="a" * 96,
            pftl_asset_symbol="LNNAVTEST",
            pftl_asset_id="b" * 96,
            run_root=root,
            bitcoin_rpc_endpoint="127.0.0.1:28443",
            lnd_endpoints=(
                "172.30.24.11:10009",
                "172.30.24.12:10009",
                "172.30.24.13:10009",
            ),
            pftl_rpc_endpoints=tuple(
                f"127.0.0.1:{30000 + index}" for index in range(6)
            ),
        )

    def test_local_six_validator_regtest_is_accepted(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            self.valid(Path(directory)).validate()

    def test_public_or_ce22_endpoint_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            valid = self.valid(Path(directory))
            with self.assertRaises(SafetyViolation):
                SafetyEnvelope(
                    **{
                        **valid.__dict__,
                        "bitcoin_network": "bitcoin",
                    }
                ).validate()
            with self.assertRaises(SafetyViolation):
                SafetyEnvelope(
                    **{
                        **valid.__dict__,
                        "pftl_chain_id": "ce22",
                    }
                ).validate()
            with self.assertRaises(SafetyViolation):
                SafetyEnvelope(
                    **{
                        **valid.__dict__,
                        "pftl_rpc_endpoints": (
                            "rpc.example.com:443",
                            *valid.pftl_rpc_endpoints[1:],
                        ),
                    }
                ).validate()


class EvidenceBundleTests(unittest.TestCase):
    def test_hash_chain_and_manifest_verify(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bundle = EvidenceBundle(root, "test-run")
            bundle.record(
                "invoice_created",
                {"payment_hash": "1" * 64, "preimage": REDACTED},
            )
            bundle.write_json("pftl/convergence.json", {"validator_count": 6})
            preimage = bytes(range(32))
            payment_hash = hashlib.sha256(preimage).hexdigest()
            bundle.write_test_vector(
                preimage_hex=preimage.hex(),
                payment_hash_hex=payment_hash,
                condition=f"a0258020{payment_hash}810120",
                fulfillment=f"a0228020{preimage.hex()}",
            )
            bundle.finalize({"result": "PASS", "preimage": REDACTED})
            verified = verify_bundle(root)
            self.assertEqual(verified["event_count"], 1)
            self.assertEqual(verified["summary"]["result"], "PASS")

    def test_general_artifact_rejects_secret(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            bundle = EvidenceBundle(Path(directory), "test-run")
            with self.assertRaises(EvidenceError):
                bundle.record("unsafe", {"payment_preimage": "0" * 64})

    def test_nonempty_root_and_post_finalize_extra_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "unexpected.txt").write_text("x", encoding="ascii")
            with self.assertRaisesRegex(EvidenceError, "must be empty"):
                EvidenceBundle(root, "test-run")
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            bundle = EvidenceBundle(root, "test-run")
            bundle.record("safe", {"value": 1})
            bundle.finalize({"result": "PASS"})
            (root / "unlisted.txt").write_text("x", encoding="ascii")
            with self.assertRaisesRegex(EvidenceError, "file set"):
                verify_bundle(root)


if __name__ == "__main__":
    unittest.main()
