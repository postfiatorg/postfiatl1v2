import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

import exit_precheck as pre
import exit_setup as setup
import govern


def fixture(height, rotation=None):
    def block(h):
        root = "aa" if rotation is None or h <= rotation else "bb"
        qc = {
            "domain": {
                "committee_root": root,
                "committee_epoch": 1 if root == "aa" else 2,
            }
        }
        return {
            "header": {
                "height": h,
                "block_hash": str(h),
                "parent_hash": str(h - 1),
                "state_root": "state" + str(h),
                "consensus_v2_commit": {"precommit_qc": qc},
                "batch_kind": "governance" if h == rotation else "asset",
                "batch_id": "batch",
            }
        }

    return block(height)


class QualificationTest(unittest.TestCase):
    def test_guest_executes_every_segment_and_binds_tip_root(self):
        anchor = dict(
            height=1,
            block="1",
            committee="aa",
            root="state1",
            chain_id="net",
            genesis_hash="gen",
            protocol_version=1,
        )
        head = dict(block_height=132, block_tip_hash="132", state_root="state132")

        def fetch(i, method, from_height, limit):
            return [fixture(h) for h in range(from_height, from_height + limit)]

        ends = []

        def execute(w, out):
            h = w["block"]["header"]["height"]
            ends.append(h)
            return dict(
                prior=w["prior_checkpoint_block_id"],
                result=str(h),
                height=h,
                start="aa",
                end="aa",
                execution={},
            )

        with (
            tempfile.TemporaryDirectory() as d,
            patch.object(pre, "rpc", side_effect=fetch),
            patch.object(pre, "execute_guest", side_effect=execute),
        ):
            result = pre.qualify_path([], anchor, head, Path(d))
            self.assertEqual(ends, [66, 131, 132])
            self.assertEqual(result["qualified_parent"], head)
            with self.assertRaises(ValueError):
                pre.qualify_path([], anchor, dict(head, state_root="fork"), Path(d))

    def test_forged_old_quorum_is_rejected_after_first_compatible_block(self):
        anchor = dict(
            height=1,
            block="1",
            committee="aa",
            root="state1",
            chain_id="net",
            genesis_hash="gen",
            protocol_version=1,
        )
        head = dict(block_height=5, block_tip_hash="5", state_root="state5")

        def fetch(i, method, **kwargs):
            if method == "blocks":
                return [
                    fixture(h, 3)
                    for h in range(
                        kwargs["from_height"], kwargs["from_height"] + kwargs["limit"]
                    )
                ]
            update = dict(
                activation_height=3, validators=["v"], signed_authorizations=["invalid"]
            )
            return [
                dict(
                    batch_id="batch",
                    payload_json=json.dumps(dict(validator_registry_updates=[update])),
                )
            ]

        def reject(w, out):
            self.assertEqual(w["finality_ancestry"][1]["block"]["header"]["height"], 3)
            raise ValueError("frozen checker refused")

        with (
            tempfile.TemporaryDirectory() as d,
            patch.object(pre, "rpc", side_effect=fetch),
            patch.object(pre, "execute_guest", side_effect=reject),
        ):
            with self.assertRaisesRegex(ValueError, "frozen checker refused"):
                pre.qualify_path([{"node_id": "v"}], anchor, head, Path(d))

    def test_governance_refuses_parent_change_before_remote_work(self):
        head = dict(
            block_height=9,
            block_tip_hash="new",
            state_root="newroot",
            build_git_revision="rev",
        )
        with (
            patch.object(govern, "fleet", return_value=head),
            patch.object(govern, "remote") as remote,
        ):
            with self.assertRaises(govern.ParentChanged):
                govern.govern(
                    "exit-open",
                    compatible="rev",
                    qualified_parent=dict(head, block_tip_hash="old"),
                )
            remote.assert_not_called()

    def test_tip_registry_update_needs_a_verified_successor(self):
        h = fixture(3, 3)["header"]
        payload = '{"validator_registry_updates":[{}]}'
        domain = dict(chain_id="net", genesis_hash="gen", protocol_version=1)
        encoded = json.dumps(
            ["net", "gen", 1, "governance", "batch", payload], separators=(",", ":")
        ).encode()
        h["consensus_v2_commit"]["precommit_qc"]["block"] = {
            "payload_hash": hashlib.sha3_384(
                b"postfiat.batch_archive_payload.v1\0" + encoded
            ).hexdigest()
        }
        with patch.object(
            pre, "rpc", return_value=[dict(batch_id="batch", payload_json=payload)]
        ):
            with self.assertRaisesRegex(ValueError, "rotation"):
                pre.check_unproved_updates(dict(domain, block={"header": h}))

    def test_open_receipts_bind_all_six_to_qualified_parent(self):
        import exit_govern

        parent = dict(block_height=17, block_tip_hash="parent", state_root="prev")
        tx = dict(
            confirmed=True,
            receipt=dict(tx_id="id", accepted=True),
            block={
                "header": {
                    "height": 18,
                    "parent_hash": "parent",
                    "state_root": "next",
                    "block_hash": "hash",
                }
            },
        )
        rows = [tx] * 6
        exit_govern.verify_open_parent(rows, "id", parent)
        with self.assertRaises(ValueError):
            exit_govern.verify_open_parent(rows[:5], "id", parent)
        with self.assertRaises(ValueError):
            exit_govern.verify_open_parent(
                rows, "id", dict(parent, block_tip_hash="other")
            )

    def test_stationary_service_executes_anchor_once(self):
        import exit_payout as payout
        import os

        cfg = setup.read(setup.CONFIG)
        cert = setup.read(setup.CONFIG.with_name("exit-checkpoint.json"))
        anchor = {
            "block": {
                "header": {
                    "block_hash": cfg["initial_checkpoint"],
                    "consensus_v2_commit": {
                        "precommit_qc": {
                            "domain": {"committee_root": cfg["initial_committee"]}
                        }
                    },
                }
            },
            "committee": [],
        }
        values = dict(
            result=cfg["initial_checkpoint"],
            height=cfg["initial_height"],
            end=cfg["initial_committee"],
        )
        head = dict(
            block_height=cfg["initial_height"],
            block_tip_hash=cfg["initial_checkpoint"],
            state_root=cert["checkpoint_state_root"],
        )
        with tempfile.TemporaryDirectory() as d:
            file = Path(d) / "witness.json"
            setup.save(file, anchor)
            with (
                patch.dict(os.environ, EXIT_ANCHOR_WITNESS=str(file)),
                patch.object(pre, "execute_guest", return_value=values) as execute,
            ):
                gate = payout.FinalizedPath()
                self.assertEqual(gate.check(head), head)
                gate.check(head)
                execute.assert_called_once()

    def test_stranded_current_checkpoint_cannot_advertise_readiness(self):
        import exit_payout as payout

        c = Mock()
        h = fixture(3, 3)["header"]
        c.account.return_value = dict(
            owner=str(payout.VERIFIER), data=bytes.fromhex("aa"), executable=False
        )
        with (
            patch.object(payout, "read", return_value={"initial_height": 1}),
            patch.object(payout, "checkpoint_pda"),
            patch.object(payout, "rpc", return_value=[{"header": h}, fixture(4, 3)]),
        ):
            with self.assertRaisesRegex(ValueError, "successor"):
                payout.check_light_path(c, 3, h["block_hash"], 5)

    def test_open_requalifies_if_tip_moves_during_service_scan(self):
        import exit_govern as g

        old = dict(
            block_height=8,
            block_tip_hash="old",
            state_root="a",
            build_git_revision="rev",
        )
        new = dict(old, block_height=9, block_tip_hash="new", state_root="b")
        route = dict(active=True, burns_open=True, profile_hash=setup.EXIT, current_height=9,
                     source_backing=dict(revision=8, bucket_id=setup.BUCKET, policy_hash=setup.POLICY,
                                         series_supply_atoms=100, outstanding_vault_bridge_atoms=100))
        responses = [dict(payout_ready=False), dict(
            payout_ready=True,
            burns_open=False,
            bucket_id=setup.BUCKET,
            qualified_parent=pre.parent_of(new),
            series_supply_atoms="100", bucket_outstanding_atoms="100",
        )]
        current = [old]

        def get(*args, **kw):
            current[0] = new
            return responses.pop(0)

        with (
            patch(
                "sys.argv",
                ["govern", "open", "--release-height", "8", "--revision", "rev"],
            ),
            patch.object(g, "fleet", side_effect=lambda: current[0]),
            patch.object(g, "ethereum"),
            patch.object(g, "require_closed"),
            patch.object(g, "six", return_value=[route] * 6),
            patch.object(g, "qualify", side_effect=get),
            patch.object(g, "verify_open_parent"),
            patch.object(g, "rotation_clearance") as clearance,
            patch.object(g, "govern", return_value={"tx_id": "id"}) as submit,
        ):
            g.main()
            clearance.assert_called_once_with(new)
            submit.assert_called_once_with(
                "exit-open", compatible="rev", qualified_parent=pre.parent_of(new)
            )

    def test_tag3_waits_for_same_parent_after_getting_solana_blockhash(self):
        c = Mock(owner=setup.MAINNET_PAYER, level={"commitment": "finalized"})
        c.call.return_value = {"value": {"blockhash": str(setup.Hash.default())}}
        parent = dict(block_height=10, block_tip_hash="old", state_root="root")
        with (
            patch("exit_payout.check_custody"),
            patch.object(setup, "fleet", return_value=dict(parent, block_height=11)),
            patch.object(setup, "Transaction") as tx,
        ):
            with self.assertRaisesRegex(ValueError, "Tip advanced"):
                setup.send_initialize(c, Mock(), Mock(), parent)
            tx.assert_not_called()
            self.assertEqual(
                [call.args[0] for call in c.call.call_args_list], ["getLatestBlockhash"]
            )
