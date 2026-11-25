"""With validator 0 lost, readiness qualifies the finalized path (no mocked qualification)
and the light checkpoint advances from the other validators."""

import unittest
from unittest.mock import Mock, patch

import exit_payout as payout
import exit_precheck as pre
import live
from route_binding import CHAIN, PFTL_GENESIS

LATEST, TIP = 1753, 1900
LOST, LAGGING = 0, None


def block_hash(h):
    return f"{h:096x}"


def block(h):
    return dict(
        header=dict(
            height=h,
            block_hash=block_hash(h),
            parent_hash=block_hash(h - 1),
            state_root=f"state{h}",
            consensus_v2_commit=dict(precommit_qc=dict(domain=dict(committee_root="aa", committee_epoch=1))),
            batch_kind="asset",
            batch_id="batch",
        )
    )


BUCKET = dict(
    bucket_id=payout.BUCKET, policy_hash=payout.POLICY, source_series_id=payout.SERIES,
    status="active", outstanding_vault_bridge_atoms=40000, source_domain=payout.SOURCE_DOMAIN,
    redeemable_at_par=True, impairment_factor_bps=10000, redemption_queue_atoms=0,
    other_allocations_atoms=0, nav_subscription_allocations_atoms=0, counted_value_atoms=40000,
)


BACKING = dict(revision=8, bucket_id=payout.BUCKET, policy_hash=payout.POLICY,
               series_supply_atoms=40000, outstanding_vault_bridge_atoms=40000)


def rpc(i, method, **params):
    """Validator `LOST` is lost; `LAGGING` answers one block behind, still open
    under an older minimum; the others answer."""
    if i == LOST:
        raise RuntimeError(f"validator-{i} unreachable")
    if i == LAGGING and method == "vault_bridge_route":
        return dict(active=True, profile_hash=payout.EXIT, profile={"route_epoch": 8},
                    current_height=TIP - 1, burns_open=False, min_withdrawal_atoms=20000, covered_bucket_ids=[payout.BUCKET], decision_signatures=dict(open=6, close=5, paid=5, min_withdrawal=5, retire=6))
    if method == "status":
        return dict(chain_id=CHAIN.decode(), genesis_hash=PFTL_GENESIS.hex(), block_height=TIP,
                    state_root=f"state{TIP}", block_tip_hash=block_hash(TIP), build_git_revision="r",
                    node_id=f"validator-{i}")
    if method == "vault_bridge_route":
        return dict(active=True, profile_hash=payout.EXIT, profile={"route_epoch": 8},
                    current_height=TIP, burns_open=True, min_withdrawal_atoms=300000, covered_bucket_ids=[payout.BUCKET], decision_signatures=dict(open=6, close=5, paid=5, min_withdrawal=5, retire=6),
                    source_backing=BACKING)
    if method == "vault_bridge_status":
        return dict(buckets=[BUCKET], issuer="issuer", finalized_epoch=7,
                    finalized_reserve_packet_hash="15" * 48)
    if method == "blocks":
        first = params["from_height"]
        return [block(h) for h in range(first, min(first + params["limit"], TIP + 1))]
    raise AssertionError(method)


def guest(w, out):
    headers = [s["block"]["header"] for s in w.get("finality_ancestry", [])] + [w["block"]["header"]]
    end = headers[-1]
    return dict(prior=w["prior_checkpoint_block_id"], start="aa", result=end["block_hash"],
                end="aa", height=end["height"], execution={})


class ValidatorZeroLostTest(unittest.TestCase):
    def setUp(self):
        self.patches = [
            patch.object(payout, "rpc", side_effect=rpc),
            patch.object(pre, "rpc", side_effect=rpc),
            patch.object(live, "rpc", side_effect=rpc),
            patch.object(pre, "execute_guest", side_effect=guest),
        ]
        for p in self.patches:
            p.start()
        self.addCleanup(lambda: [p.stop() for p in reversed(self.patches)])

    def test_readiness_counts_a_lagging_validator_as_the_one_not_agreeing(self):
        import sys

        module = sys.modules[__name__]
        with patch.object(module, "LOST", None), patch.object(module, "LAGGING", 2):
            self.test_readiness_qualifies_the_path_with_validator_0_lost()

    def test_readiness_qualifies_the_path_with_validator_0_lost(self):
        c = Mock()

        def account(key):
            if key == payout.checkpoint_pda(block_hash(LATEST)):
                return dict(executable=False, owner=str(payout.VERIFIER), data=bytes.fromhex("aa"))
            return dict(owner=str(payout.TOKEN),
                        data=bytes(payout.MINT) + bytes(32) + (40000).to_bytes(8, "little"))

        c.account.side_effect = account
        path = payout.FinalizedPath()
        path.cursor = dict(chain_id="net", genesis_hash="gen", protocol_version=1, height=LATEST,
                           block=block_hash(LATEST), root=f"state{LATEST}", committee="aa")
        path.members = []
        with (
            patch.object(payout, "cluster", return_value=c),
            patch.object(payout, "light", return_value=(LATEST, block_hash(LATEST))),
            patch.object(payout, "inspect_vault", return_value={"custody_atoms": 40000}),
            patch.object(payout, "payout_budget", return_value=1000),
            patch.object(payout, "balance", return_value=payout.FLOOR + 10000000),
            patch.object(payout, "available"),
            patch.object(payout, "FINALIZED_PATH", path),
            patch.object(payout, "read", return_value=dict(initial_height=LATEST)),
        ):
            value = payout.readiness()
        self.assertTrue(value["ready"], value)
        self.assertEqual(value["qualified_parent"]["block_height"], TIP)
        self.assertEqual(path.cursor["height"], TIP)

    def test_checkpoint_target_and_witness_come_from_another_validator(self):
        self.assertEqual(payout.proof_count(LATEST, TIP + 1, TIP), 3)
        target = payout.select_target(LATEST, payout.fetch_any)
        domain = dict(chain_id="net", genesis_hash="gen", protocol_version=1)
        w = payout.public_checkpoint(payout.fetch_any, LATEST, block_hash(LATEST), target, [], domain)
        self.assertEqual(w["block"]["header"]["height"], target["height"])


if __name__ == "__main__":
    unittest.main()
