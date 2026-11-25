"""The governance wrapper collects the signatures the chain reports per action."""

import unittest

import exit_rules
import govern

BUCKET = exit_rules.BUCKET
REV9 = "4683dbe99fcb2aef277f095e459ff541e1ef0774222aa6c1962e091d8f18c14743b82a54518b1f7eb48c451c9c5b21ce"


def second_route(open_signatures=6):
    """A route report of the second release (`covered_bucket_ids`, `decision_signatures`)."""
    return dict(
        covered_bucket_ids=[BUCKET, REV9],
        decision_signatures=dict(open=open_signatures, close=5, paid=5, min_withdrawal=5, retire=6),
        min_withdrawal_atoms=300000,
    )


def lost(i):
    if i == 3:
        raise OSError("validator-3 unreachable")
    return f"auth-{i}"


class QuorumTest(unittest.TestCase):
    def test_first_release_chain_needs_every_validator(self):
        rules = exit_rules.of_route(dict(active=True, burns_open=False))
        self.assertEqual(rules, exit_rules.first_release())
        self.assertEqual(rules["covered"], (BUCKET,))
        for kind in ("exit-close", "exit-paid", "exit-open", "route", "activation"):
            self.assertEqual(govern.required_signatures(kind, rules=rules), 6, kind)
            self.assertEqual(govern.allowed_quorums(kind, rules=rules), {6}, kind)
            with self.assertRaisesRegex(RuntimeError, "6 required.*validator-3"):
                govern.collect(kind, lost, need=govern.required_signatures(kind, rules=rules))
        # No rules read at all is the first release too.
        self.assertEqual(govern.required_signatures("exit-close"), 6)
        self.assertEqual(exit_rules.canary_atoms(rules), 20_000)

    def test_second_release_chain_lets_five_close_pay_and_reopen(self):
        rules = exit_rules.of_route(second_route())
        self.assertEqual((rules["release"], rules["minimum"]), (2, 300000))
        self.assertEqual(rules["covered"], tuple(sorted([BUCKET, REV9])))
        self.assertEqual(exit_rules.canary_atoms(rules), 300000)
        for kind, need in (("exit-close", 5), ("exit-paid", 5), ("exit-open", 6), ("route", 6)):
            self.assertEqual(govern.required_signatures(kind, rules=rules), need, kind)
        for kind in ("exit-close", "exit-paid"):
            got = govern.collect(kind, lost, need=govern.required_signatures(kind, rules=rules))
            self.assertEqual(sorted(got), [0, 1, 2, 4, 5])
        # After a close the chain reports five for opening.
        reopen = exit_rules.of_route(second_route(open_signatures=5))
        self.assertEqual(govern.required_signatures("exit-open", rules=reopen), 5)
        self.assertEqual(govern.allowed_quorums("exit-open", rules=reopen), {5})

    def test_two_lost_validators_block_close(self):
        def sign(i):
            if i in (1, 4):
                raise OSError("down")
            return i

        rules = exit_rules.of_route(second_route())
        with self.assertRaisesRegex(RuntimeError, "4 of 6 .* 5 required.*validator-1, validator-4"):
            govern.collect("exit-close", sign, need=govern.required_signatures("exit-close", rules=rules))

    def test_partial_or_disagreeing_rules_are_refused(self):
        partial = dict(second_route())
        del partial["decision_signatures"]
        with self.assertRaisesRegex(ValueError, "only part"):
            exit_rules.of_route(partial)
        with self.assertRaisesRegex(ValueError, "disagree"):
            exit_rules.agreed([second_route(), dict(active=True)])
        bad = dict(second_route(), decision_signatures=dict(open=6, close=7, paid=5,
                                                             min_withdrawal=5, retire=6))
        with self.assertRaisesRegex(ValueError, "decision signatures"):
            exit_rules.of_route(bad)


if __name__ == "__main__":
    unittest.main()
