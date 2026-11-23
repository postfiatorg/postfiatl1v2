import base64
import copy
from pathlib import Path
import unittest
from unittest.mock import Mock

import exit_setup as s


class BootstrapTests(unittest.TestCase):
    def test_vault_snapshot_rejects_wrong_route_and_frozen_ata(self):
        from exit_precheck import inspect_vault, PHANTOM

        snapshot = s.read(Path(__file__).parent / "fixtures/mainnet-vault.json")
        rows = snapshot["value"]
        c = Mock()
        for idx, offset in [(0, 32), (1, 32), (1, 72), (2, 44), (2, 4), (3, 108)]:
            altered = copy.deepcopy(rows)
            data = bytearray(base64.b64decode(altered[idx]["data"][0]))
            data[offset] ^= 1
            altered[idx]["data"][0] = base64.b64encode(data).decode()
            c.call.return_value = dict(context={"slot": 42}, value=altered)
            with self.assertRaises(ValueError):
                inspect_vault(c, PHANTOM)
        c.call.return_value = dict(context={"slot": 43}, value=rows)
        self.assertEqual(inspect_vault(c, PHANTOM)["slot"], 43)
        c.call.return_value = dict(context={"slot": 44}, value=rows[:3])
        self.assertEqual(inspect_vault(c)["slot"], 44)
        self.assertEqual(len(c.call.call_args.args[1]), 3)
        self.assertEqual(c.call.call_args.args[2]["commitment"], "finalized")

    def test_six_way_and_archive_committee_gate(self):
        from exit_precheck import agreement, check_certificate

        anchor = s.read(s.CONFIG)
        cert = s.read(Path(s.__file__).parent / "fixtures/exit-checkpoint.json")
        row = dict(
            block=anchor["initial_checkpoint"],
            height=1753,
            root=cert["checkpoint_state_root"],
            committee=anchor["initial_committee"],
            certificate_id=cert["certificate_id"],
        )
        self.assertEqual(agreement([row] * 6), row)
        for different in [[row] * 5, [row] * 5 + [dict(row, committee="00" * 48)]]:
            with self.assertRaises(ValueError):
                agreement(different)
        check_certificate(cert, row, anchor["initial_committee"])
        with self.assertRaises(ValueError):
            check_certificate(cert, row, "00" * 48)

    def test_recover_exact_both_accounts_or_refuse(self):
        from exit_precheck import bootstrap_state

        cfg = s.read(s.CONFIG)
        empty = dict(owner=str(s.SYSTEM), data=b"", executable=False)
        simulated = s.read(Path(__file__).parent / "fixtures/mainnet-bootstrap-simulation.json")
        light, anchor = [dict(a, data=base64.b64decode(a["data"][0])) for a in simulated]
        self.assertEqual(bootstrap_state(cfg, [None, empty]), "empty")
        self.assertEqual(bootstrap_state(cfg, [light, anchor]), "initialized")
        for pair in (
            [light, empty],
            [light, dict(anchor, data=bytes(48))],
            [dict(light, owner=str(s.SYSTEM)), anchor],
        ):
            with self.assertRaises(ValueError):
                bootstrap_state(cfg, pair)

    def test_reviewed_instruction_binds_all_account_permissions(self):
        cfg = s.read(s.CONFIG)
        ix = s.build_initialize(cfg, s.MAINNET_PAYER)
        s.check_reviewed_instruction(cfg, ix)
        bad = s.Instruction(
            ix.program_id, ix.data, [s.rw(s.FEE_PAYER, True), *ix.accounts[1:]]
        )
        with self.assertRaises(ValueError):
            s.check_reviewed_instruction(cfg, bad)
        bad = s.Instruction(ix.program_id, ix.data, [*ix.accounts[:-1], s.rw(s.SYSTEM)])
        with self.assertRaises(ValueError):
            s.check_reviewed_instruction(cfg, bad)

    def test_finalized_bootstrap_history_requires_exact_tag_and_accounts(self):
        import base58
        from exit_precheck import bootstrap_history

        ix = s.build_initialize(s.read(s.CONFIG), s.MAINNET_PAYER)
        keys = [str(a.pubkey) for a in ix.accounts]
        item = dict(
            programIdIndex=1,
            accounts=list(range(5)),
            data=base58.b58encode(ix.data).decode(),
        )
        c = Mock(level={"commitment": "finalized"})
        tx = dict(
            meta={"err": None},
            transaction={
                "signatures": ["given"],
                "message": {"accountKeys": keys, "instructions": [item]},
            },
        )
        c.call.return_value = tx
        self.assertEqual(bootstrap_history(c, ix, "given"), "given")
        item["accounts"][2] = 3
        with self.assertRaises(ValueError):
            bootstrap_history(c, ix, "given")
        item["accounts"][2] = 2
        tx["transaction"]["signatures"][0] = "different"
        with self.assertRaises(ValueError):
            bootstrap_history(c, ix, "given")

    def test_advanced_bootstrap_recovers_history_and_checks_current_checkpoint(self):
        import exit_precheck as pre
        from unittest.mock import patch

        cfg = s.read(s.CONFIG)
        ix = s.build_initialize(cfg, s.MAINNET_PAYER)
        root = "ee" * 48
        block = "ab" * 48
        height = cfg["initial_height"] + 2

        def acc(data):
            return dict(owner=str(s.VERIFIER), executable=False, data=data)

        light = acc(
            s.expected_light(cfg)[:-57]
            + bytes.fromhex(block)
            + height.to_bytes(8, "little")
            + b"\0"
        )
        original = acc(bytes.fromhex(cfg["initial_committee"]))
        current = acc(bytes.fromhex(root))
        h = dict(
            height=height,
            block_hash=block,
            consensus_v2_commit={"precommit_qc": {"domain": {"committee_root": root}}},
        )
        with (
            patch.object(
                pre,
                "accounts",
                side_effect=lambda c, keys: (
                    70,
                    [light, original, current][: len(keys)],
                ),
            ),
            patch.object(pre, "bootstrap_history", return_value="sig") as history,
            patch.object(pre, "six", return_value=[[{"header": h}]] * 6),
        ):
            state = pre.recover_bootstrap(Mock(), cfg, ix, "sig")
            self.assertEqual(
                (
                    state["signature"],
                    state["height"],
                    state["block"],
                    state["committee"],
                ),
                ("sig", height, block, root),
            )
            history.assert_called_once()
            current["data"] = bytes(48)
            with self.assertRaises(ValueError):
                pre.recover_bootstrap(Mock(), cfg, ix, "sig")
            light["data"] = b"x" + light["data"][1:]
            with self.assertRaises(ValueError):
                pre.recover_bootstrap(Mock(), cfg, ix, "sig")
