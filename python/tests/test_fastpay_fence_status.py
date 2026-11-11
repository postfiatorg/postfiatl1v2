import copy
import hashlib
import json
import unittest
from pathlib import Path
from unittest import mock

from postfiat_rpc.client import PostFiatRpcClient
from postfiat_rpc.fastpay_fence_status import (
    FENCE_STATUS_SIGNING_TAG,
    FenceStatusRefused,
    committee_root,
    signing_bytes,
    verify_fence_status_quorum,
)

LOCK = "22" * 48
VECTORS = Path(__file__).resolve().parents[2] / "crates/types/test-vectors/fastpay_fence_status_acceptance_v2.json"


def make_committee(count=6, epoch=3):
    committee = {
        "schema": "postfiat-fastpay-recovery-committee-v2",
        "chain_id": "fence-status",
        "genesis_hash": "11" * 48,
        "protocol_version": 1,
        "committee_epoch": epoch,
        "valid_from_height": 2,
        "new_orders_through_height": 100,
        "registry_root": "",
        "quorum": count - (count - 1) // 3,
        "validators": [
            {"validator_id": f"validator-{i}", "algorithm_id": "ML-DSA-65", "public_key_hex": f"{i:02x}" * 8}
            for i in range(count)
        ],
    }
    committee["registry_root"] = committee_root(committee)
    return committee


COMMITTEE = make_committee()
PINNED_ROOT = COMMITTEE["registry_root"]


def status(index, anchored_at_or_before_height=7, observed_height=9):
    terminal = anchored_at_or_before_height is not None
    return {
        "schema": "postfiat-fastpay-fence-status-v2",
        "chain_id": "fence-status",
        "genesis_hash": "11" * 48,
        "protocol_version": 1,
        "committee_epoch": 3,
        "registry_root": PINNED_ROOT,
        "lock_id": LOCK,
        "decision": "cancelled" if terminal else None,
        "operation": "unwrap" if terminal else None,
        "fence_digest": "33" * 48 if terminal else None,
        "anchored_at_or_before_height": anchored_at_or_before_height,
        "observed_height": observed_height,
        "validator_id": f"validator-{index}",
        # Test signature: the member key; `verify` compares them.
        "signature_hex": f"{index:02x}" * 8,
    }


def check(statuses, committee=COMMITTEE, pinned=PINNED_ROOT):
    return verify_fence_status_quorum(
        statuses, committee, LOCK, lambda key, _m, sig: key == sig, pinned_registry_root=pinned
    )


class FenceStatusQuorumTests(unittest.TestCase):
    def test_encoding_matches_the_rust_vector(self) -> None:
        # Pinned in postfiat_types::fence_status_encoding_matches_the_python_vector.
        one = status(0)
        one["registry_root"] = "44" * 48
        one["decision"] = {"confirmed": {"order_digest": "55" * 48, "certificate_digest": "66" * 48}}
        message = signing_bytes(one)
        self.assertTrue(message.startswith(FENCE_STATUS_SIGNING_TAG))
        self.assertEqual(
            hashlib.sha256(message).hexdigest(),
            RUST_VECTOR_SHA256,
        )

    def test_four_of_six_is_refused_and_five_of_six_accepted(self) -> None:
        statuses = [status(i, observed_height=9 + i) for i in range(6)]
        with self.assertRaises(FenceStatusRefused) as refused:
            check(statuses[:4])
        self.assertEqual(refused.exception.reason, "below_quorum")
        proved = check(statuses[:5])
        self.assertEqual((proved["anchored_at_or_before_height"], len(proved["signers"])), (7, 5))

    def test_each_faulty_status_is_reported_beside_the_quorum(self) -> None:
        honest = [status(i) for i in range(6)]
        cases = {
            "non_member": (0, lambda s: s.update(validator_id="validator-9")),
            "wrong_epoch": (1, lambda s: s.update(committee_epoch=4)),
            "wrong_domain": (2, lambda s: s.update(chain_id="other")),
            "bad_signature": (3, lambda s: s.update(signature_hex="00")),
            "malformed": (4, lambda s: s.update(fence_digest=None)),
            "wrong_lock": (5, lambda s: s.update(lock_id="77" * 48)),
        }
        for reason, (index, mutate) in cases.items():
            statuses = copy.deepcopy(honest)
            mutate(statuses[index])
            proved = check(statuses)
            self.assertEqual(len(proved["signers"]), 5, reason)
            self.assertEqual(proved["invalid"], [{"validator_id": statuses[index]["validator_id"], "reason": reason}])
        proved = check(honest[:5] + [copy.deepcopy(honest[0])])
        self.assertEqual((len(proved["signers"]), proved["invalid"][0]["reason"]), (5, "duplicate_validator"))

    def test_minority_contradiction_is_reported_not_a_veto(self) -> None:
        # P1-N1, the reviewer's case: five terminal, the sixth "not terminal"
        # at the anchored height.
        five = [status(i) for i in range(5)]
        late = status(5, None, 7)
        proved = check(five + [late])
        self.assertEqual((len(proved["signers"]), proved["contradicting"]), (5, [late]))
        other = dict(status(5), fence_digest="77" * 48)
        self.assertEqual(check(five + [other])["contradicting"], [other])
        self.assertEqual(check(five + [status(5, None, 6)])["contradicting"], [])
        equivocated = [dict(status(i), fence_digest="77" * 48) for i in range(1, 6)]
        with self.assertRaises(FenceStatusRefused) as refused:
            check(five + equivocated)
        self.assertEqual(refused.exception.reason, "conflicting")

    def test_shared_acceptance_vectors(self) -> None:
        # The same file runs in postfiat_types::fence_status_shared_acceptance_vectors.
        cases = json.loads(VECTORS.read_text())["cases"]
        self.assertGreaterEqual(len(cases), 30)
        for case in cases:
            with self.subTest(case["name"]):
                try:
                    got = {"accepted": check(case["statuses"], case["committee"], case["pinned_registry_root"])}
                except FenceStatusRefused as refused:
                    got = {"refused": {"reason": refused.reason}}
                    if refused.validator_id:
                        got["refused"]["validator_id"] = refused.validator_id
                    got["refused"].update(refused.detail if refused.reason == "below_quorum" else {})
                self.assertEqual(got, case["expected"])

    def test_reviewer_quorum_one_probe_is_refused(self) -> None:
        # Real members and pinned root, `quorum` lowered to 1 in the object.
        lowered = copy.deepcopy(COMMITTEE)
        lowered["quorum"] = 1
        with self.assertRaises(FenceStatusRefused) as refused:
            check([status(0)], lowered)
        self.assertEqual((refused.exception.reason, refused.exception.detail), ("invalid_committee", {"problem": "quorum"}))

    def test_pinned_root_is_required(self) -> None:
        with self.assertRaises(TypeError):
            verify_fence_status_quorum([status(0)], COMMITTEE, LOCK, lambda *_: True)

    def test_client_sends_only_the_lock(self) -> None:
        client = PostFiatRpcClient("127.0.0.1:1234")
        with mock.patch.object(client, "_call", return_value={}) as call:
            client.owned_fence_status(LOCK)
        call.assert_called_once_with("owned_fence_status", {"lock_id": LOCK})


RUST_VECTOR_SHA256 = "78a70cda3b47c57f2ce98138145aae1fde90fa4374a057620bc1a40f3c05ea89"

if __name__ == "__main__":
    unittest.main()
