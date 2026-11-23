import unittest


class SegmentTest(unittest.TestCase):
    def test_rotation_is_inside_next_segment(self):
        from exit_segments import checkpoint_target, validate_segment

        def block(h, root):
            return {
                "header": dict(
                    height=h,
                    block_hash=f"{h:096x}",
                    parent_hash=f"{h - 1:096x}",
                    consensus_v2_commit={
                        "precommit_qc": {"domain": {"committee_root": root}}
                    },
                )
            }

        old, new = "ab" * 48, "cd" * 48
        blocks = [block(h, old if h <= 1818 else new) for h in range(1817, 1820)]
        self.assertEqual(checkpoint_target(1753, blocks)["height"], 1817)
        w = dict(
            prior_checkpoint_block_id=f"{1817:096x}",
            finality_ancestry=[{"block": blocks[1]}],
            block=blocks[2],
        )
        validate_segment(w, w["prior_checkpoint_block_id"], blocks[2]["header"], old)
        with self.assertRaises(ValueError):
            validate_segment(
                w, w["prior_checkpoint_block_id"], blocks[2]["header"], new
            )
        with self.assertRaises(ValueError):
            validate_segment(
                w, w["prior_checkpoint_block_id"], blocks[1]["header"], old
            )

    def test_public_rpc_witness_preserves_rotation_payload(self):
        from exit_segments import public_checkpoint
        import json

        old = [
            {
                "node_id": "v0",
                "public_key_hex": "ab",
                "algorithm_id": "ML-DSA-65",
                "active": True,
            }
        ]
        new = dict(old[0], public_key_hex="cd")
        payload = json.dumps(
            {
                "validator_registry_updates": [
                    dict(activation_height=1818, validators=["v0"], new_record=new)
                ]
            }
        )

        def block(h, root):
            return {
                "header": dict(
                    block_hash=f"{h:096x}",
                    parent_hash=f"{h - 1:096x}",
                    height=h,
                    batch_id="batch",
                    batch_kind="governance",
                    consensus_v2_commit={
                        "precommit_qc": {
                            "domain": {
                                "committee_root": root,
                                "committee_epoch": 3 if root == "old" else 4,
                            }
                        }
                    },
                )
            }

        blocks = [block(1818, "old"), block(1819, "new")]

        def fetch(_, method, **kw):
            if method == "blocks":
                return blocks
            return [dict(payload_json=payload, batch_id="batch")]

        w = public_checkpoint(
            fetch,
            1817,
            f"{1817:096x}",
            blocks[-1]["header"],
            old,
            dict(chain_id="chain", genesis_hash="ab", protocol_version=1),
        )
        self.assertEqual(w["committee"], [new])
        self.assertEqual(w["finality_ancestry"][0]["governance_payload_json"], payload)
        self.assertEqual(w["finality_ancestry"][0]["committee"], old)

    def test_historical_burn_skips_stranded_rotation_checkpoint(self):
        from exit_payout import compatible_prior, VERIFIER
        import base64

        before, after = "ab" * 48, "cd" * 48

        def header(h, root):
            return dict(
                height=h,
                block_hash=f"{h:096x}",
                parent_hash=f"{h - 1:096x}",
                consensus_v2_commit={
                    "precommit_qc": {"domain": {"committee_root": root}}
                },
            )

        def checkpoint(root):
            return dict(
                owner=str(VERIFIER),
                data=[base64.b64encode(bytes.fromhex(root)).decode(), "base64"],
            )

        headers = [header(1817, before), header(1818, before), header(1819, after)]
        self.assertEqual(
            compatible_prior(headers, [checkpoint(before), checkpoint(before)]),
            headers[0]["block_hash"],
        )
