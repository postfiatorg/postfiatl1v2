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

# This host's reviewer key: the reviewer holds the keypair, the signing step only its public key.
HOST_KEY = Keypair()
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
                 mint=None, program=None, fail=None, tx_slot=SLOT, prefund=False, crowd=0, stranger_upload=False):
        self.marker, self.tag, self.err, self.extra = marker, tag, err, extra_payout
        self.buffer = values() if buffer is None else buffer
        self.delta, self.mint, self.program, self.fail, self.tx_slot = delta, mint, program, fail, tx_slot
        payout = dict(signature=base58.b58encode(SIG).decode(), err=None, slot=tx_slot)
        # Each address's finalized history, newest first, as getSignaturesForAddress pages it.
        later = [dict(signature=f"later{n}", err=None, slot=tx_slot + 1 + n) for n in reversed(range(crowd))]
        self.histories = {
            str(rv.marker(BURN)): later + [payout] + ([dict(signature="prefund", err=None, slot=SLOT - 9)]
                                                    if prefund else []),
            BUFFER: later + [payout]
            + ([dict(signature="stranger", err=None, slot=SLOT - 3)] if stranger_upload else [])
            + [dict(signature="upload", err=None, slot=SLOT - 5)],
        }
        self.pages = 0

    def history(self, address, params):
        rows = self.histories.get(address, [])
        if params.get("before"):
            rows = rows[[r["signature"] for r in rows].index(params["before"]) + 1:]
        self.pages += 1
        return rows[:params["limit"]]

    def naming(self, signature, *named, program=None, data=b"\x02" + b"\0" * 11):
        """A successful transaction that only names addresses: a lamport transfer or a stranger's call."""
        keys = ["payer", *named, str(program or rv.SYSTEM)]
        return dict(slot=SLOT, meta=dict(err=None, innerInstructions=[]), transaction=dict(
            signatures=[signature], message=dict(accountKeys=keys, instructions=[
                dict(programIdIndex=len(keys) - 1, accounts=list(range(len(keys) - 1)),
                     data=base58.b58encode(data).decode())])))

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
            return self.history(params[0], params[1])
        if method == "getTransaction":
            if params[0] == "prefund" or params[0].startswith("later"):
                return self.naming(params[0], marker, BUFFER)
            if params[0] == "stranger":
                # Someone else's verifier upload into its own buffer, naming this buffer read-only.
                data = bytes([5]) + (0).to_bytes(2, "little") + values(amount=1)
                return self.naming("stranger", str(Keypair().pubkey()), BUFFER, program=rv.VERIFIER, data=data)
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

    def test_a_prefunded_or_crowded_marker_is_still_reviewed_by_its_creating_transaction(self):
        for case in (dict(prefund=True), dict(crowd=150), dict(prefund=True, crowd=150)):
            with self.subTest(**case):
                self.assertEqual(self.review(sol=Solana(**case))["decision"], PAID_KIND)
        # Pages of fewer signatures: the reviewer pages back to the creating transaction.
        from unittest.mock import patch
        sol = Solana(prefund=True, crowd=150)
        with patch.object(rv, "PAGE", 40):
            self.assertEqual(self.review(sol=sol)["decision"], PAID_KIND)
        self.assertGreater(sol.pages, 4)

    def test_a_history_beyond_the_stated_bound_is_refused_by_name(self):
        from unittest.mock import patch
        with patch.object(rv, "PAGE", 10), patch.object(rv, "MAX_PAGES", 3):
            with self.assertRaisesRegex(rv.Refused, "more than 30 transactions"):
                self.review(sol=Solana(crowd=150))

    def test_a_marker_crowded_after_its_first_review_is_still_reviewed_from_its_remembered_writer(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as d, patch.object(rv, "PAGE", 10), patch.object(rv, "MAX_PAGES", 3):
            memory = Path(d)
            self.assertEqual(rv.review_burn("paid", BURN, Node(), Solana(crowd=5), memory)["decision"], PAID_KIND)
            crowded = Solana(crowd=150)
            self.assertEqual(rv.review_burn("paid", BURN, Node(), crowded, memory)["decision"], PAID_KIND)
            self.assertEqual(crowded.pages, 1)  # only the buffer's uploads before the payout are paged
            # A remembered signature is checked like any other: one that did not create the marker is not used.
            (memory / f"{rv.marker(BURN)}.sig").write_text("later3\n")
            self.assertEqual(rv.review_burn("paid", BURN, Node(), Solana(crowd=5), memory)["decision"], PAID_KIND)
            with self.assertRaisesRegex(rv.Refused, "more than 30 transactions"):
                (memory / f"{rv.marker(BURN)}.sig").write_text("later3\n")
                rv.review_burn("paid", BURN, Node(), Solana(crowd=150), memory)

    def test_a_strangers_upload_naming_the_buffer_is_ignored(self):
        self.assertEqual(self.review(sol=Solana(stranger_upload=True, crowd=20))["decision"], PAID_KIND)

    def test_an_rpc_error_exits_non_zero_by_name(self):
        with tempfile.TemporaryDirectory() as d:
            cfg = rv.Config(validator="validator-3", node=Node(), solana=Solana(fail="getTransaction"),
                            review_dir=Path(d), key=HOST_KEY)
            code = rv.run_review(cfg, "paid", BURN)
            self.assertEqual(code, rv.EXIT_RPC)
            self.assertEqual(list(Path(d).iterdir()), [])


class SignTest(unittest.TestCase):
    def setUp(self):
        self.dir = tempfile.TemporaryDirectory()
        self.cfg = rv.Config(validator="validator-3", node=Node(), solana=Solana(), review_dir=Path(self.dir.name),
                             key=HOST_KEY)

    def tearDown(self):
        self.dir.cleanup()

    def amend(self, kind=PAID_KIND):
        return dict(kind=kind, value=1, paused=False, activation_height=1901)

    def test_the_verdict_file_names_the_decision_endpoint_slot_and_time(self):
        self.assertEqual(rv.run_review(self.cfg, "paid", BURN), 0)
        (path,) = Path(self.dir.name).glob("*.json")
        verdict = json.loads(path.read_text())
        self.assertEqual(path.name, hashlib.sha256(PAID_KIND.encode()).hexdigest() + ".json")
        self.assertEqual(verdict["decision_sha256"], hashlib.sha256(PAID_KIND.encode()).hexdigest())
        # The endpoint's host only: a provider's API key in the URL stays out of the verdict.
        self.assertEqual(verdict["solana_endpoint"], "solana.example")
        self.assertEqual(path.stat().st_mode & 0o777, 0o640)
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
                          key=Keypair())
        with self.assertRaisesRegex(rv.Refused, "not written by this host's reviewer"):
            rv.authorize(other, self.amend())
        (path,) = Path(self.dir.name).glob("*.json")
        doc = json.loads(path.read_text())
        doc["validator"] = "validator-0"
        path.write_text(json.dumps(doc))
        with self.assertRaisesRegex(rv.Refused, "not written by this host's reviewer"):
            rv.authorize(self.cfg, self.amend())
        os.chmod(path, 0o666)
        with self.assertRaisesRegex(rv.Refused, "unsafe"):
            rv.authorize(self.cfg, self.amend())

    def test_the_signing_step_can_check_a_verdict_but_not_produce_one(self):
        rv.run_review(self.cfg, "paid", BURN)
        signer = rv.Config(validator="validator-3", node=None, solana=None, review_dir=Path(self.dir.name),
                           key=None, public=HOST_KEY.pubkey())
        rv.authorize(signer, self.amend())
        with self.assertRaisesRegex(rv.Refused, "no reviewer key"):
            rv.write_verdict(signer, dict(action="refund", burn=BURN, decision=REFUND_KIND, finalized_slot=1))
        # A verdict sealed with any other key, as a forger without the reviewer key would, is refused.
        forger = rv.Config(validator="validator-3", node=Node(), solana=Solana(tag=10, marker=b"\x02"),
                           review_dir=Path(self.dir.name), key=Keypair())
        rv.run_review(forger, "refund", BURN)
        with self.assertRaisesRegex(rv.Refused, "not written by this host's reviewer"):
            rv.authorize(signer, self.amend(REFUND_KIND))

    def test_the_installer_keeps_the_reviewer_key_from_the_signing_user(self):
        text = (Path(rv.__file__).parent / "install-exit-review.sh").read_text()
        self.assertNotIn("root:postfiat", text)
        self.assertIn("pf-sol-exit-review", text)

    def test_an_amendment_that_is_not_a_plain_decision_is_refused(self):
        rv.run_review(self.cfg, "paid", BURN)
        with self.assertRaisesRegex(rv.Refused, "decision"):
            rv.authorize(self.cfg, dict(self.amend(), value=0))
        with self.assertRaisesRegex(rv.Refused, "decision"):
            rv.authorize(self.cfg, dict(self.amend(), paused=True))

    def test_the_node_omits_paused_when_false_and_the_decision_is_still_plain(self):
        rv.run_review(self.cfg, "paid", BURN)
        amend = {k: v for k, v in self.amend().items() if k != "paused"}
        rv.authorize(self.cfg, amend)

    def captured_amendment(self):
        """An amendment as a WAN validator serialized it (route activation at 1754, captured in the
        node's fixtures). `GovernanceAmendment` is unchanged in release 04600868."""
        batch = Path(rv.__file__).resolve().parents[2] / "crates/node/tests/fixtures/solana-exit/governance-batch.json"
        (activation,) = json.loads(batch.read_text())["vault_bridge_route_profile_activations"]
        return activation["amendment"]

    def test_a_captured_node_amendment_omits_paused_and_the_reviewer_signs_its_reviewed_decision(self):
        captured = self.captured_amendment()
        self.assertNotIn("paused", captured)
        rv.run_review(self.cfg, "paid", BURN)
        # Only the decision and its value differ from what the node sent.
        rv.authorize(self.cfg, dict(captured, kind=PAID_KIND, value=1))
        for paused in (True, 0, 1, None, "false", []):
            with self.assertRaisesRegex(rv.Refused, "decision"):
                rv.authorize(self.cfg, dict(captured, kind=PAID_KIND, value=1, paused=paused))

    def test_a_reply_that_is_not_an_amendment_is_refused_not_a_crash(self):
        rv.run_review(self.cfg, "paid", BURN)
        with self.assertRaisesRegex(rv.Refused, "decision"):
            rv.authorize(self.cfg, dict(self.captured_amendment(), kind=PAID_KIND, value=True))
        for reply in (None, [], "amendment", 1):
            with self.assertRaisesRegex(rv.Refused, "decision"):
                rv.authorize(self.cfg, reply)
        with self.assertRaisesRegex(rv.Refused, "decision"):
            rv.authorize(self.cfg, {k: v for k, v in dict(self.captured_amendment(), value=1).items() if k != "kind"})


FAKE_NODE = """#!/usr/bin/env python3
import json, sys
from pathlib import Path
args = sys.argv[1:]
flag = {args[k]: args[k + 1] for k in range(len(args) - 1) if args[k].startswith("--")}
Path(sys.argv[0]).with_name("argv.json").write_text(json.dumps(sys.argv))
if Path(sys.argv[0]).with_name("mutate").exists():
    Path(flag["--amendment-file"]).write_text(json.dumps(dict(kind="solana_exit_r8_paid:OTHER", value=1, paused=False)))
Path(flag["--authorization-file"]).write_text("auth")
"""


class SignCommandTest(unittest.TestCase):
    """`sign` runs the node's own signing command for exactly the reviewed amendment file."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        root = Path(self.tmp.name)
        self.data = root / "var/lib/postfiat/validator-3"
        (self.data / "sol-live/round").mkdir(parents=True)
        self.binary = root / "opt/postfiat/releases/r1/postfiat-node"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_text(FAKE_NODE)
        self.binary.chmod(0o755)
        for directory in (self.binary.parent, self.binary.parent.parent):
            directory.chmod(0o755)  # a release only its owner can replace
        proc = root / "proc/4242"
        proc.mkdir(parents=True)
        (proc / "cmdline").write_bytes("\0".join([str(self.binary), "transport-validator-serve", "--data-dir",
                                                    str(self.data), "--topology", "t"]).encode() + b"\0")
        (proc / "exe").symlink_to(self.binary)
        self.cfg = rv.Config(validator="validator-3", node=Node(), solana=Solana(), review_dir=root / "review",
                             key=HOST_KEY, data_dir=self.data, proc=root / "proc",
                             releases=root / "opt/postfiat/releases")
        self.cfg.release_owner = os.getuid()  # root on a host
        self.assertEqual(rv.run_review(self.cfg, "paid", BURN), 0)
        self.amend = self.data / "sol-live/round/amend"
        self.amend.write_text(json.dumps(dict(kind=PAID_KIND, value=1, paused=False, activation_height=1901)))
        self.auth = self.data / "sol-live/round/auth"

    def tearDown(self):
        self.tmp.cleanup()

    def test_signs_exactly_the_reviewed_file_with_its_own_command(self):
        rv.sign(self.cfg, self.amend, 1901, 1931, self.auth)
        argv = json.loads(self.binary.with_name("argv.json").read_text())
        self.assertEqual(argv, [str(self.binary), "governance-authorization-sign", "--data-dir", str(self.data),
                                "--amendment-file", str(self.amend), "--validator", "validator-3",
                                "--validator-key-file", str(self.data / "validator_keys.json"),
                                "--proposal-slot", "1901", "--expires-at-height", "1931",
                                "--authorization-file", str(self.auth)])
        self.assertEqual(self.auth.read_text(), "auth")

    def add_process(self, pid, argv, exe):
        proc = Path(self.tmp.name) / "proc" / str(pid)
        proc.mkdir(parents=True)
        (proc / "cmdline").write_bytes("\0".join(argv).encode() + b"\0")
        (proc / "exe").symlink_to(exe)

    def fake_node(self, path):
        path.parent.mkdir(mode=0o755, parents=True, exist_ok=True)
        path.parent.chmod(0o755)
        path.write_text(FAKE_NODE)
        path.chmod(0o755)
        return path

    def test_the_binary_is_the_processs_own_executable_not_the_name_in_its_command_line(self):
        # Any local process may name a release binary in its command line; only /proc/PID/exe counts,
        # and only processes of the node user are candidates.
        from unittest.mock import patch
        other = self.fake_node(self.binary.parent.parent / "r0/postfiat-node")
        self.add_process(4243, [str(other), "transport-validator-serve", "--data-dir", str(self.data)], "/bin/sleep")
        self.add_process(4244, [str(self.binary), "transport-validator-serve", "--data-dir"], "/bin/sleep")
        stranger = lambda path: 4321 if path.name == "4243" else os.getuid()  # noqa: E731
        with patch.object(rv, "uid_of", stranger):
            rv.sign(self.cfg, self.amend, 1901, 1931, self.auth)
        self.assertEqual(json.loads(self.binary.with_name("argv.json").read_text())[0], str(self.binary))
        self.assertFalse(other.with_name("argv.json").exists())
        # The node user's own process running something else is not ignored: signing stops.
        with self.assertRaisesRegex(rv.Refused, "resident validator"):
            rv.sign(self.cfg, self.amend, 1901, 1931, self.auth)

    def test_a_binary_outside_the_releases_or_writable_by_others_or_of_another_user_is_refused(self):
        import shutil
        from unittest.mock import patch
        outside = self.fake_node(Path(self.tmp.name) / "opt/postfiat/postfiat-node")
        shutil.rmtree(Path(self.tmp.name) / "proc/4242")
        self.add_process(4245, [str(self.cfg.releases / ".." / "postfiat-node"), "transport-validator-serve",
                                "--data-dir", str(self.data)], outside)
        with self.assertRaisesRegex(rv.Refused, "resident validator"):
            rv.sign(self.cfg, self.amend, 1901, 1931, self.auth)
        self.assertFalse(outside.with_name("argv.json").exists())
        shutil.rmtree(Path(self.tmp.name) / "proc/4245")
        self.add_process(4242, [str(self.binary), "transport-validator-serve", "--data-dir", str(self.data)],
                         self.binary)
        self.binary.chmod(0o777)
        with self.assertRaisesRegex(rv.Refused, "resident validator"):
            rv.sign(self.cfg, self.amend, 1901, 1931, self.auth)
        self.binary.chmod(0o755)
        with patch.object(rv, "uid_of", lambda path: 4321 if "proc" in str(path) else os.getuid()):
            with self.assertRaisesRegex(rv.Refused, "resident validator"):
                rv.sign(self.cfg, self.amend, 1901, 1931, self.auth)
        self.assertFalse(self.binary.with_name("argv.json").exists())

    def test_a_caller_supplied_command_or_any_other_argument_is_refused(self):
        for extra in (["--", "/bin/echo", "signed"], ["--validator-key-file", "/tmp/k"], ["extra"]):
            with self.subTest(extra=extra), self.assertRaises(SystemExit) as stop:
                rv.main(["sign", "--amendment-file", str(self.amend), "--proposal-slot", "1901",
                         "--expires-at-height", "1931", "--authorization-file", str(self.auth), *extra])
            self.assertEqual(stop.exception.code, 2)
        self.assertFalse(self.binary.with_name("argv.json").exists())

    def test_an_amendment_changed_while_it_is_signed_is_refused_and_its_signature_removed(self):
        self.binary.with_name("mutate").write_text("")
        with self.assertRaisesRegex(rv.Refused, "changed"):
            rv.sign(self.cfg, self.amend, 1901, 1931, self.auth)
        self.assertFalse(self.auth.exists())

    def test_an_unreviewed_amendment_or_a_file_outside_the_data_directory_is_refused(self):
        self.amend.write_text(json.dumps(dict(kind=REFUND_KIND, value=1, paused=False)))
        with self.assertRaisesRegex(rv.Refused, "no review"):
            rv.sign(self.cfg, self.amend, 1901, 1931, self.auth)
        outside = Path(self.tmp.name) / "amend"
        outside.write_text(json.dumps(dict(kind=PAID_KIND, value=1, paused=False)))
        with self.assertRaisesRegex(rv.Refused, "data directory"):
            rv.sign(self.cfg, outside, 1901, 1931, self.auth)
        self.assertFalse(self.binary.with_name("argv.json").exists())


class SigningConfigTest(unittest.TestCase):
    """The signing step trusts the public key and the config only when the signing user cannot replace them."""

    def test_a_public_key_or_config_the_signing_user_could_replace_is_refused(self):
        with tempfile.TemporaryDirectory() as d:
            etc = Path(d) / "etc"
            etc.mkdir(mode=0o755)
            etc.chmod(0o755)
            config, public = etc / "sol-exit-review.json", etc / "sol-exit-review.pub"
            config.write_text(json.dumps(dict(validator="validator-3", node=["127.0.0.1", 1], solana_rpc="https://x")))
            public.write_text(str(HOST_KEY.pubkey()) + "\n")
            for path in (config, public):
                path.chmod(0o644)
            cfg = rv.load_config(config, public=public, reviewer=False, owner=os.getuid())
            self.assertEqual(cfg.public, HOST_KEY.pubkey())
            with self.assertRaisesRegex(rv.Refused, "not owned by root"):
                rv.load_config(config, public=public, reviewer=False, owner=4321)
            etc.chmod(0o775)
            with self.assertRaisesRegex(rv.Refused, "writable"):
                rv.load_config(config, public=public, reviewer=False, owner=os.getuid())
            etc.chmod(0o755)
            public.chmod(0o666)
            with self.assertRaisesRegex(rv.Refused, "writable"):
                rv.load_config(config, public=public, reviewer=False, owner=os.getuid())


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
                            key=HOST_KEY)
            body = rv.review_clearance(cfg, data_dir=d)
            self.assertTrue(body["decision"].startswith("solana_exit_rotation_clearance:validator-3:1900:"))
            (safety / "height-1950.json").write_text("{}")
            with self.assertRaisesRegex(rv.Refused, "uncommitted round"):
                rv.review_clearance(cfg, data_dir=d)


class ClearanceAccessTest(unittest.TestCase):
    def test_a_signing_history_the_reviewer_cannot_read_is_a_named_error_with_a_failing_exit(self):
        import contextlib
        import io

        with tempfile.TemporaryDirectory() as d:
            safety = Path(d) / "consensus-v2-safety"
            safety.mkdir()
            (safety / "height-1899.json").write_text("{}")
            safety.chmod(0)
            try:
                cfg = rv.Config(validator="validator-3", node=Node(), solana=Solana(), review_dir=Path(d) / "r",
                                key=HOST_KEY, data_dir=Path(d))
                with self.assertRaisesRegex(rv.AccessError, "cannot read"):
                    rv.review_clearance(cfg, data_dir=d)
                err = io.StringIO()
                with contextlib.redirect_stderr(err):
                    self.assertEqual(rv.run_review(cfg, "clearance", None), rv.EXIT_ACCESS)
                self.assertIn("access error: clearance: cannot read", err.getvalue())
                self.assertFalse((Path(d) / "r").exists())
            finally:
                safety.chmod(0o700)

    def test_the_unit_counts_only_a_named_refusal_as_success(self):
        unit = (Path(rv.__file__).parent / "pft-sol-exit-review.service").read_text()
        self.assertIn("SuccessExitStatus=2\n", unit)
        self.assertNotEqual(rv.EXIT_ACCESS, rv.EXIT_REFUSED)


class RobustScanTest(unittest.TestCase):
    """Every failure is a named exit 2, 3 or 4 with one line; one bad burn stops nothing else."""

    def cfg(self, d, node=None, sol=None):
        safety = Path(d) / "consensus-v2-safety"
        safety.mkdir(exist_ok=True)
        (safety / "height-1899.json").write_text("{}")
        return rv.Config(validator="validator-3", node=node or Node(), solana=sol or Solana(),
                         review_dir=Path(d) / "r", key=HOST_KEY, data_dir=Path(d))

    def test_an_unexpected_failure_is_a_named_exit_without_a_traceback(self):
        import contextlib
        import io

        class Broken(Solana):
            def __call__(self, method, params):
                if method == "getTransaction":
                    return dict(slot=SLOT)  # a reply of an unexpected shape
                return super().__call__(method, params)

        with tempfile.TemporaryDirectory() as d:
            err = io.StringIO()
            with contextlib.redirect_stderr(err):
                code = rv.run_review(self.cfg(d, sol=Broken()), "paid", BURN)
            self.assertEqual(code, rv.EXIT_RPC)
            self.assertNotIn("Traceback", err.getvalue())
            self.assertEqual(len(err.getvalue().strip().splitlines()), 1)
            self.assertIn("KeyError", err.getvalue())

    def test_one_bad_burn_does_not_stop_the_other_reviews_or_the_clearance(self):
        class TwoBurns(Node):
            def __call__(self, method, **params):
                if method == "vault_bridge_status":
                    return dict(redemptions=[dict(self.red, burn_tx_id="zz"), self.red])
                if method == "status":
                    return dict(block_height=1900, block_tip_hash="ab" * 48, state_root="cd" * 48,
                                build_git_revision="rev")
                return super().__call__(method, **params)

        with tempfile.TemporaryDirectory() as d:
            cfg = self.cfg(d, node=TwoBurns())
            code = rv.scan(cfg)
            self.assertEqual(code, rv.EXIT_RPC)
            names = sorted(p.name for p in (Path(d) / "r").glob("*.json"))
            self.assertIn(hashlib.sha256(PAID_KIND.encode()).hexdigest() + ".json", names)
            self.assertEqual(len(names), 2, "the good burn and the clearance were still reviewed")

    def test_verdicts_older_than_a_day_are_pruned(self):
        with tempfile.TemporaryDirectory() as d:
            cfg = self.cfg(d)
            rv.run_review(cfg, "paid", BURN)
            (old,) = (Path(d) / "r").glob("*.json")
            past = time.time() - rv.PRUNE_AGE - 60
            os.utime(old, (past, past))
            rv.prune(cfg)
            self.assertFalse(old.exists())

    def test_endpoints_prints_the_hosts_and_warns_when_fewer_than_three_differ(self):
        import contextlib
        import io

        configs = [dict(validator=f"validator-{i}", solana_rpc=url) for i, url in enumerate(
            ["https://a.example/k1", "https://a.example/k2", "https://b.example/", "https://a.example/",
             "https://b.example/x", "https://a.example/y"])]
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(rv.endpoints(configs), 1)
        self.assertIn("validator-0 a.example", out.getvalue())
        self.assertIn("WARNING: only 2 different Solana providers", out.getvalue())
        self.assertNotIn("k1", out.getvalue())
        configs[3]["solana_rpc"] = "https://c.example/"
        with contextlib.redirect_stdout(io.StringIO()):
            self.assertEqual(rv.endpoints(configs), 0)


if __name__ == "__main__":
    unittest.main()
