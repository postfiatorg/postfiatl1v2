"""The per-host settlement reviewer: every check, a named verdict, no placeholder on RPC errors,
and a signer that only signs from this host's own fresh review of exactly this decision."""

import hashlib
import json
import os
import tempfile
import time
import unittest
from pathlib import Path

import base58
from solders.keypair import Keypair

import exit_review as rv

BURN = "b1" * 48
PACKET = "a1" * 48
OWNER = Keypair().pubkey()
SIG = bytes(range(64))
AMOUNT = 300000
SLOT = 473300000
BUFFER = str(Keypair().pubkey())
CUSTODY = str(rv.pda(rv.VAULT, b"custody"))
ATA = str(rv.pda(rv.ATA_PROGRAM, bytes(OWNER), bytes(rv.TOKEN), bytes(rv.MINT)))


def values(burn=BURN, amount=AMOUNT, owner=OWNER, bucket=rv.BUCKET, family=rv.FAMILY, tag=b"SOLANA01"):
    lengths = [len(rv.EGRESS), 4, 10, 48, 4, 48, 8, 48, 48, 8, 48, 0, 8, 8, 48, 48, 48,
               48, 8, 48, 48, 8, 48, 48, 48, 48, 8, 20, 48, 48, 8, 8, 20, 32, 20, 32, 32, 48, 32]
    f = [b"\x11" * n for n in lengths]
    f[0], f[1] = rv.EGRESS, (1).to_bytes(4, "big")
    f[20] = f[23] = bytes.fromhex(burn)
    f[21] = b"accepted"
    f[22] = bytes.fromhex(family)
    f[25] = bytes.fromhex(bucket)
    f[26] = amount.to_bytes(8, "big")
    f[28] = rv.destination_hash(owner)
    f[31] = tag
    out = rv.MAGIC + len(rv.EGRESS).to_bytes(4, "big") + rv.EGRESS
    return out + b"".join((i + 1).to_bytes(2, "big") + len(v).to_bytes(4, "big") + v for i, v in enumerate(f))


class Node:
    def __init__(self, state="pending", destination=None, covered=(rv.BUCKET,)):
        self.red = dict(burn_tx_id=BURN, bucket_id=rv.BUCKET, amount_atoms=AMOUNT, state=state,
                        settled_atoms=0, withdrawal_packet_hash=PACKET,
                        destination_ref=destination or rv.PREFIX + bytes(OWNER).hex())
        self.covered = covered

    def __call__(self, method, **params):
        if method == "vault_bridge_status":
            return dict(redemptions=[self.red])
        if method == "vault_bridge_route":
            return dict(covered_bucket_ids=list(self.covered))
        if method == "status":
            return dict(block_height=1900, node_id="validator-3")
        raise AssertionError(method)


class Solana:
    endpoint = "https://solana.example/host-3"

    def __init__(self, marker=b"\x01", tag=2, err=None, extra_payout=False, buffer=None, delta=AMOUNT,
                 mint=None, program=None, fail=None, tx_slot=SLOT):
        self.marker, self.tag, self.err, self.extra = marker, tag, err, extra_payout
        self.buffer = values() if buffer is None else buffer
        self.delta, self.mint, self.program, self.fail, self.tx_slot = delta, mint, program, fail, tx_slot

    def __call__(self, method, params):
        if self.fail == method:
            raise rv.RpcError(f"{self.endpoint} {method}: 503 Service Unavailable")
        marker = str(rv.marker(BURN))
        if method == "getSlot":
            return SLOT + 40
        if method == "getAccountInfo":
            if self.marker is None:
                return dict(value=None)
            import base64
            return dict(value=dict(owner=str(rv.VERIFIER), data=[base64.b64encode(self.marker).decode(), "base64"]))
        if method == "getSignaturesForAddress":
            if params[0] == marker:
                return [dict(signature=base58.b58encode(SIG).decode(), err=None, slot=self.tx_slot)]
            return [dict(signature="upload", err=None, slot=SLOT - 5)]
        if method == "getTransaction":
            if params[0] == "upload":
                data = bytes([5]) + (0).to_bytes(2, "little") + self.buffer
                keys = [str(rv.VERIFIER), BUFFER]
                return dict(slot=SLOT - 5, meta=dict(err=None, innerInstructions=[]), transaction=dict(
                    signatures=["upload"], message=dict(accountKeys=keys, instructions=[
                        dict(programIdIndex=0, accounts=[1], data=base58.b58encode(data).decode())])))
            program = str(self.program or rv.VERIFIER)
            keys = [str(Keypair().pubkey()), str(rv.LIGHT), "prior", "result", BUFFER, str(rv.SYSTEM), marker,
                    str(rv.VAULT), "auth", "config", CUSTODY, ATA, str(OWNER), str(self.mint or rv.MINT),
                    str(rv.TOKEN), program]
            ix = dict(programIdIndex=15, accounts=list(range(15)),
                      data=base58.b58encode(bytes([self.tag]) + b"\x00" * 356).decode())
            instructions = [ix, ix] if self.extra else [ix]
            mint = str(self.mint or rv.MINT)
            bal = lambda i, a: dict(accountIndex=i, mint=mint, uiTokenAmount=dict(amount=str(a), decimals=6))  # noqa: E731
            return dict(slot=self.tx_slot, meta=dict(
                err=self.err, innerInstructions=[],
                preTokenBalances=[bal(10, 10_000_000), bal(11, 0)],
                postTokenBalances=[bal(10, 10_000_000 - self.delta), bal(11, self.delta)]),
                transaction=dict(signatures=[base58.b58encode(SIG).decode()],
                                 message=dict(accountKeys=keys, instructions=instructions)))
        raise AssertionError(method)


PAID_KIND = f"solana_exit_r8_paid:{PACKET}:{SLOT}:0:{SIG.hex()}"
REFUND_KIND = f"solana_exit_refund:{PACKET}:{SLOT}:{SIG.hex()}"


class ReviewTest(unittest.TestCase):
    def review(self, node=None, sol=None, action="paid"):
        return rv.review_burn(action, BURN, node or Node(), sol or Solana(tag=2 if action == "paid" else 10,
                                                                          marker=b"\x01" if action == "paid" else b"\x02"))

    def test_a_correct_payout_and_cancel_are_approved_with_their_decision(self):
        self.assertEqual(self.review()["decision"], PAID_KIND)
        self.assertEqual(self.review(action="refund")["decision"], REFUND_KIND)

    def test_every_missing_check_refuses_by_name(self):
        cases = {
            "redemption state": dict(node=Node(state="settled")),
            "covered bucket": dict(node=Node(covered=("09" * 48,))),
            "canonical destination": dict(node=Node(destination=rv.PREFIX + bytes(OWNER).hex().upper())),
            "marker": dict(sol=Solana(marker=b"\x02")),
            "finalized": dict(sol=Solana(tx_slot=SLOT + 100)),
            "failed transaction": dict(sol=Solana(err={"InstructionError": [0, "x"]})),
            "exactly one": dict(sol=Solana(extra_payout=True)),
            "program": dict(sol=Solana(program=Keypair().pubkey())),
            "mint": dict(sol=Solana(mint=Keypair().pubkey())),
            "amount": dict(sol=Solana(delta=AMOUNT - 1)),
            "public values": dict(sol=Solana(buffer=values(amount=AMOUNT + 1))),
            "public values ": dict(sol=Solana(buffer=values(owner=Keypair().pubkey()))),
            "public values  ": dict(sol=Solana(buffer=values(burn="c1" * 48))),
        }
        for name, case in cases.items():
            with self.assertRaises(rv.Refused, msg=name) as caught:
                self.review(**case)
            self.assertIn(name.strip(), str(caught.exception))

    def test_an_rpc_error_exits_non_zero_by_name(self):
        with tempfile.TemporaryDirectory() as d:
            cfg = rv.Config(validator="validator-3", node=Node(), solana=Solana(fail="getTransaction"),
                            review_dir=Path(d), key=b"k" * 32)
            code = rv.run_review(cfg, "paid", BURN)
            self.assertEqual(code, rv.EXIT_RPC)
            self.assertEqual(list(Path(d).iterdir()), [])


class SignTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.cfg = rv.Config(validator="validator-3", node=Node(), solana=Solana(), review_dir=Path(self.dir.name),
                             key=b"k" * 32)

    def tearDown(self):
        self.dir.cleanup()

    def amend(self, kind=PAID_KIND):
        return dict(kind=kind, value=1, paused=False, activation_height=1901)

    def test_the_verdict_file_names_the_decision_endpoint_slot_and_time(self):
        self.assertEqual(rv.run_review(self.cfg, "paid", BURN), 0)
        (path,) = Path(self.dir.name).iterdir()
        verdict = json.loads(path.read_text())
        self.assertEqual(path.name, hashlib.sha256(PAID_KIND.encode()).hexdigest() + ".json")
        self.assertEqual(verdict["decision_sha256"], hashlib.sha256(PAID_KIND.encode()).hexdigest())
        self.assertEqual(verdict["solana_endpoint"], Solana.endpoint)
        self.assertEqual((verdict["validator"], verdict["verdict"]), ("validator-3", "approve"))
        self.assertGreater(verdict["finalized_slot"], 0)
        self.assertIn("time", verdict)

    def test_signs_only_from_its_own_fresh_review_of_exactly_this_decision(self):
        rv.run_review(self.cfg, "paid", BURN)
        rv.authorize(self.cfg, self.amend())
        with self.assertRaisesRegex(rv.Refused, "no review"):
            rv.authorize(self.cfg, self.amend(REFUND_KIND))
        with self.assertRaisesRegex(rv.Refused, "stale"):
            rv.authorize(self.cfg, self.amend(), now=time.time() + rv.REVIEW_MAX_AGE + 1)
        # A file written by anything but this host's reviewer (another host, a copy) is refused.
        other = rv.Config(validator="validator-3", node=Node(), solana=Solana(), review_dir=Path(self.dir.name),
                          key=b"x" * 32)
        with self.assertRaisesRegex(rv.Refused, "not written by this host's reviewer"):
            rv.authorize(other, self.amend())
        (path,) = Path(self.dir.name).iterdir()
        doc = json.loads(path.read_text())
        doc["validator"] = "validator-0"
        path.write_text(json.dumps(doc))
        with self.assertRaisesRegex(rv.Refused, "not written by this host's reviewer"):
            rv.authorize(self.cfg, self.amend())
        os.chmod(path, 0o666)
        with self.assertRaisesRegex(rv.Refused, "unsafe"):
            rv.authorize(self.cfg, self.amend())

    def test_an_amendment_that_is_not_a_plain_decision_is_refused(self):
        rv.run_review(self.cfg, "paid", BURN)
        with self.assertRaisesRegex(rv.Refused, "decision"):
            rv.authorize(self.cfg, dict(self.amend(), value=0))


class ConstantsAndClearanceTest(unittest.TestCase):
    def test_constants_equal_the_service_constants(self):
        import exit_payout
        import exit_setup

        self.assertEqual((rv.VERIFIER, rv.VAULT, rv.MINT, rv.ASSET, rv.SERIES, rv.BUCKET),
                         (exit_setup.VERIFIER, exit_setup.VAULT, exit_setup.MINT, exit_setup.ASSET,
                          exit_setup.SERIES, exit_setup.BUCKET))
        self.assertEqual((rv.PREFIX, str(rv.LIGHT), str(rv.marker(BURN))),
                         (exit_payout.PREFIX, str(exit_setup.LIGHT), str(exit_payout.marker(BURN))))

    def test_clearance_reads_this_hosts_own_signing_history(self):
        class Head(Node):
            def __call__(self, method, **params):
                return dict(block_height=1900, block_tip_hash="ab" * 48, state_root="cd" * 48,
                            build_git_revision="rev")

        with tempfile.TemporaryDirectory() as d:
            safety = Path(d) / "consensus-v2-safety"
            safety.mkdir()
            (safety / "height-1899.json").write_text("{}")
            cfg = rv.Config(validator="validator-3", node=Head(), solana=Solana(), review_dir=Path(d) / "r",
                            key=b"k" * 32)
            body = rv.review_clearance(cfg, data_dir=d)
            self.assertTrue(body["decision"].startswith("solana_exit_rotation_clearance:validator-3:1900:"))
            (safety / "height-1950.json").write_text("{}")
            with self.assertRaisesRegex(rv.Refused, "uncommitted round"):
                rv.review_clearance(cfg, data_dir=d)


if __name__ == "__main__":
    unittest.main()
