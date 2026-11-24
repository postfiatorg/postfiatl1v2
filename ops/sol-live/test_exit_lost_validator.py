"""With one validator lost, close and paid run end to end and payouts keep flowing."""

import json
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import exit_payout as payout
import exit_canary
import exit_govern
import govern
import live
from exit_precheck import parent_of
from exit_setup import ASSET, EXIT, PROFILE, SERIES, read
from route_binding import CHAIN, PFTL_GENESIS

LOST = 3
REV = "rev-under-test"
HEIGHT = 1800
BIN = "/opt/postfiat/releases/r1/postfiat-node"
SUPPLY = 40000


class Fleet:
    """Six fake validators; validator `LOST` refuses every RPC and SSH call."""

    def __init__(self, status=None, closed=False, second=True):
        self.files = {}
        self.closed = closed
        # `second`: the chain reports the second release's rules; else the first release's.
        self.second = second
        self.opened = False
        self.kind = None
        self.status = status
        self.head = dict(
            chain_id=CHAIN.decode(),
            genesis_hash=PFTL_GENESIS.hex(),
            block_height=HEIGHT,
            state_root="root",
            block_tip_hash="tip",
            build_git_revision=REV,
        )

    def rpc(self, i, method, **params):
        if i == LOST:
            raise RuntimeError(f"validator-{i} unreachable")
        if method == "status":
            return dict(self.head, node_id=f"validator-{i}")
        if method == "vault_bridge_route" and params["asset_id"] == ASSET:
            return dict(profile_hash=exit_govern.ETH_HASH, profile={"route_epoch": 11})
        if method == "vault_bridge_route":
            route = dict(active=True, burns_open=self.opened, profile_hash=EXIT, profile=read(PROFILE),
                         current_height=HEIGHT,
                         source_backing=dict(revision=8, bucket_id=payout.BUCKET,
                                             policy_hash=payout.POLICY, series_supply_atoms=SUPPLY,
                                             outstanding_vault_bridge_atoms=SUPPLY))
            if self.second:
                route.update(
                    closed_since_registration=self.closed,
                    covered_bucket_ids=[payout.BUCKET],
                    min_withdrawal_atoms=300000,
                    decision_signatures=dict(open=5 if self.closed else 6, close=5, paid=5,
                                             min_withdrawal=5, retire=6),
                )
            return route
        if method == "tx":
            return dict(
                confirmed=True,
                receipt=dict(tx_id=params["tx_id"], accepted=True),
                block=dict(header=dict(height=HEIGHT + 1, parent_hash="tip", state_root="next")),
            )
        if method == "vault_bridge_status":
            return self.status
        raise AssertionError(method)

    def remote(self, i, args, data=None):
        if i == LOST:
            raise OSError(f"validator-{i} unreachable")
        args = [str(a) for a in args]
        if args[0] == "pgrep":
            return "4242\n"
        if args == ["cat", "/proc/4242/cmdline"]:
            return "\0".join(
                [BIN, "transport-validator-serve", "--data-dir", f"/var/lib/postfiat/validator-{i}",
                 "--topology", "/etc/postfiat/releases/topology.json"]
            )
        # The protected root-owned review reader of govern.read_operator_review.
        if args[-1] == "/etc/postfiat/sol-exit-paid-review.json":
            return json.dumps(dict(kind=self.kind, parent=parent_of(self.head)))
        if args[0] == "find":
            return f"height-{HEIGHT}.json\n"
        if args[-1] == "/etc/postfiat/sol-exit-rotation-clearance.json":
            return json.dumps(dict(
                validator=f"validator-{i}", parent=parent_of(self.head), revision=REV,
                profile_hash=EXIT, hold_through_open=True, outstanding_signed_rotation_rounds=[]))
        if args[0] == "cat":
            return self.files[(i, args[1])]
        rest = args[4:]
        if rest[:4] == ["/usr/bin/python3", "-I", govern.REVIEWER, "sign"]:
            # The host's own reviewer approved this decision; its wrapper runs the signing command.
            rest = rest[rest.index("--") + 1:]
        if rest[0] == "mkdir":
            return ""
        if rest[0] == "tee":
            self.files[(i, rest[1])] = data
            return data
        assert rest[0] == BIN, rest
        tool, flags = rest[1], rest[2:]
        value = {flags[k]: flags[k + 1] for k in range(len(flags) - 1) if flags[k].startswith("--")}
        if tool.startswith("ratify-solana-exit-"):
            if tool == "ratify-solana-exit-paid":
                self.kind = "solana_exit_r8_paid:{}:{}:{}:{}".format(
                    value["--packet-hash"], value["--solana-slot"],
                    value["--payout-index"], value["--signature-hex"],
                )
            else:
                self.kind = "solana_exit_r8_" + tool.rsplit("-", 1)[1]
            amend = dict(
                kind=self.kind, activation_height=int(value["--height"]), value=1,
                paused=False, quorum=6 if self.kind.endswith("_open") and not self.closed else 5,
                validators=value["--validators"].split(","),
            )
            self.files[(i, value["--amendment-file"])] = json.dumps(amend)
        elif tool == "governance-authorization-sign":
            self.files[(i, value["--authorization-file"])] = json.dumps(dict(v=i))
        elif tool == "governance-amendment-assemble":
            used = value["--authorization-files"].split(",")
            assert len(used) == 5 and not any(f.endswith(f"auth-{LOST}") for f in used)
            self.files[(i, value["--output"])] = json.dumps(dict(amendment_id="amend-" + self.kind[-8:]))
        elif tool == "governance-batch":
            self.files[(i, value["--batch-file"])] = json.dumps(dict(batch=True))
        elif tool == "transport-peer-certified-batch-round" and self.kind.endswith("_open"):
            self.opened = True
        return ""

    def patches(self, state):
        return [
            patch.object(live, "rpc", self.rpc),
            patch.object(exit_govern, "rpc", self.rpc),
            patch.object(exit_canary, "rpc", self.rpc),
            patch.object(govern, "remote", self.remote),
            patch.object(govern, "STATE", Path(state)),
        ]


def run(fleet, state, argv):
    with tempfile.TemporaryDirectory():
        ctx = fleet.patches(state)
        for p in ctx:
            p.start()
        try:
            with patch.object(sys, "argv", ["exit_govern.py", *argv]):
                exit_govern.main()
        finally:
            for p in reversed(ctx):
                p.stop()


class LostValidatorTest(unittest.TestCase):
    def test_close_runs_end_to_end_with_one_validator_lost(self):
        with tempfile.TemporaryDirectory() as d:
            out = Path(d) / "close.json"
            run(Fleet(), d, ["close", "--revision", REV, "--release-height", "1", "--output", str(out)])
            saved = read(out)
            self.assertEqual(len(saved["signers"]), 5)
            self.assertNotIn(f"validator-{LOST}", saved["signers"])
            self.assertEqual(len(saved["validators"]), 5)
            accepted = list(Path(d).glob("governance/*/accepted.json"))
            self.assertEqual(len(accepted), 1)

    def test_accepted_is_saved_before_the_receipt_survey(self):
        with tempfile.TemporaryDirectory() as d:
            fleet = Fleet()
            with patch.object(govern, "receipt_at", side_effect=RuntimeError("survey failed")):
                with self.assertRaises(RuntimeError):
                    run(fleet, d, ["close", "--revision", REV, "--release-height", "1",
                                   "--output", str(Path(d) / "close.json")])
            (accepted,) = Path(d).glob("governance/*/accepted.json")
            self.assertTrue(read(accepted)["tx_id"])

    def test_two_burns_are_paid_and_settled_with_one_validator_lost(self):
        burns = ["a1" * 48, "b2" * 48]
        bucket = dict(
            bucket_id=payout.BUCKET, policy_hash=payout.POLICY, source_domain=payout.SOURCE_DOMAIN,
            status="active", redeemable_at_par=True, impairment_factor_bps=10000,
            outstanding_vault_bridge_atoms=0, redemption_queue_atoms=600000,
            other_allocations_atoms=0, nav_subscription_allocations_atoms=0,
            counted_value_atoms=600000,
        )
        reds = [
            dict(burn_tx_id=b, bucket_id=payout.BUCKET, amount_atoms=300000, state="pending",
                 settled_atoms=0, settlement_receipt_hash="", owner=exit_canary.OWNER,
                 destination_ref=exit_canary.DEST, withdrawal_packet_hash=f"{k}7" * 48)
            for k, b in enumerate(burns)
        ]
        status = dict(buckets=[bucket], redemptions=reds, allocations=[], receipts=[])

        # Both are settled through the wrapper with validator 3 lost (the custody
        # check that lets the second be paid first is in test_exit_custody_paid.py).
        fleet = Fleet(status)
        with tempfile.TemporaryDirectory() as d:
            for k, burn in enumerate(burns):
                paid = dict(burn_tx_id=burn, solana_tx="sig", withdrawal_packet_hash=reds[k]["withdrawal_packet_hash"], amount_atoms=300000)
                ref = dict(kind=None, packet=reds[k]["withdrawal_packet_hash"], slot=9, ordinal=0, signature_hex="ab" * 64)
                ref["kind"] = "solana_exit_r8_paid:{}:9:0:{}".format(ref["packet"], ref["signature_hex"])
                Path(d, f"paid-{k}.json").write_text(json.dumps(paid))
                with (
                    patch.object(exit_canary, "cluster"),
                    patch.object(exit_canary, "finalized_reference", return_value=ref),
                    patch.object(exit_canary, "signature", return_value="sig"),
                ):
                    run(fleet, d, ["paid", "--revision", REV, "--release-height", "1",
                                   "--paid", str(Path(d, f"paid-{k}.json")),
                                   "--output", str(Path(d, f"paid-round-{k}.json"))])
                self.assertEqual(len(read(Path(d, f"paid-round-{k}.json"))["signers"]), 5)

    def qualify(self, fleet):
        # Open qualification comes from the worker's authenticated Unix socket.
        state = dict(payout_ready=True, burns_open=False, source_domain=payout.SOURCE_DOMAIN,
                     qualified_parent=parent_of(fleet.head),
                     series_supply_atoms=str(SUPPLY), bucket_outstanding_atoms=str(SUPPLY))
        return patch.object(exit_govern, "qualify", return_value=state)

    def test_reopen_after_a_close_runs_end_to_end_with_one_validator_lost(self):
        fleet = Fleet(closed=True)
        with tempfile.TemporaryDirectory() as d, self.qualify(fleet):
            out = Path(d) / "open.json"
            run(fleet, d, ["open", "--revision", REV, "--release-height", "1", "--output", str(out)])
            saved = read(out)
            self.assertEqual(len(saved["signers"]), 5)
            self.assertNotIn(f"validator-{LOST}", saved["signers"])
            self.assertTrue(all(r["burns_open"] for r in saved["routes"]))

    def test_first_release_chain_needs_every_validator_for_close_and_reopen(self):
        # A chain on the first release reports no rules: close and re-open need all six.
        for action in ("close", "open"):
            fleet = Fleet(closed=True, second=False)
            with tempfile.TemporaryDirectory() as d, self.qualify(fleet):
                out = str(Path(d) / f"{action}.json")
                with self.assertRaisesRegex(RuntimeError, f"6 required; unreachable: validator-{LOST}"):
                    run(fleet, d, [action, "--revision", REV, "--release-height", "1", "--output", out])
            self.assertFalse(fleet.opened)

    def test_first_open_still_needs_every_validator(self):
        fleet = Fleet()
        with tempfile.TemporaryDirectory() as d, self.qualify(fleet):
            with self.assertRaisesRegex(RuntimeError, f"6 required; unreachable: validator-{LOST}"):
                run(fleet, d, ["open", "--revision", REV, "--release-height", "1"])
            self.assertFalse(fleet.opened)

    def test_a_lagging_validator_does_not_block_five_agreeing_reads(self):
        closed = dict(active=True, burns_open=False, profile_hash=EXIT, profile=read(PROFILE))
        rows = [closed] * 5 + [dict(closed, burns_open=True)]
        exit_govern.require_closed(rows, 5)
        with self.assertRaises(ValueError):
            exit_govern.require_closed(rows[1:], 5)
        eth = dict(profile_hash=exit_govern.ETH_HASH, profile={"route_epoch": 11})
        stale = dict(profile_hash="old", profile={"route_epoch": 10})
        with patch.object(exit_govern, "routes", return_value=[eth] * 5 + [stale]):
            self.assertEqual(exit_govern.ethereum(5), {"route_epoch": 11})
        with patch.object(exit_govern, "routes", return_value=[eth] * 4 + [stale] * 2):
            with self.assertRaises((ValueError, RuntimeError)):
                exit_govern.ethereum(5)

    def test_fleet_and_receipts_take_a_quorum_and_name_the_lost(self):
        fleet = Fleet()
        with patch.object(live, "rpc", fleet.rpc):
            self.assertEqual(live.fleet(need=5)["block_tip_hash"], "tip")
            with self.assertRaisesRegex(RuntimeError, f"validator-{LOST}"):
                live.fleet()
            self.assertEqual(len(live.receipt_at("tx", HEIGHT + 1, need=5)), 5)


class ValidatorZeroLostTest(LostValidatorTest):
    """The same runs with validator 0, the usual first reader, lost."""

    def setUp(self):
        p = patch.object(sys.modules[__name__], "LOST", 0)
        p.start()
        self.addCleanup(p.stop)


if __name__ == "__main__":
    unittest.main()
