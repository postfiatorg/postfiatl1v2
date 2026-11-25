"""The settlement loop's restricted way into a host (exit_review_door.py, the forced command of the
reviewer's own SSH key line) and the node user's signing of what it handed in
(exit_review.py sign-door). Two requests and nothing else; the host's own fresh review decides."""

import hashlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import exit_review_door as door

KIND = "solana_exit_r8_paid:" + "ab" * 48 + ":9:0:" + "cd" * 64
SHA = hashlib.sha256(KIND.encode()).hexdigest()
AMEND = dict(kind=KIND, value=1, quorum=5, activation_height=10,
             validators=[f"validator-{i}" for i in range(6)])


class Door(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.inbox, self.outbox = self.root / "inbox", self.root / "outbox"
        self.inbox.mkdir()
        self.outbox.mkdir()

    def tearDown(self):
        self.tmp.cleanup()

    def ask(self, request, original=""):
        raw = request if isinstance(request, (bytes, str)) else json.dumps(request)
        raw = raw.encode() if isinstance(raw, str) else raw
        out, err = io.StringIO(), io.StringIO()
        with patch("sys.stdout", out), patch("sys.stderr", err):
            code = door.main(io.BytesIO(raw), dict(SSH_ORIGINAL_COMMAND=original),
                             inbox=self.inbox, outbox=self.outbox)
        return code, out.getvalue(), err.getvalue()

    def review(self, **changes):
        return dict(dict(request="review", amendment=AMEND, proposal_slot=10, expires_at_height=40), **changes)

    def test_a_handed_in_amendment_waits_for_this_hosts_own_review(self):
        code, out, _ = self.ask(self.review())
        self.assertEqual((code, json.loads(out)), (0, dict(decision_sha256=SHA, state="handed in")))
        saved = json.loads((self.inbox / f"{SHA}.json").read_text())
        self.assertEqual(saved, dict(amendment=AMEND, proposal_slot=10, expires_at_height=40))
        code, out, _ = self.ask(dict(request="result", decision_sha256=SHA))
        self.assertEqual((code, json.loads(out)), (0, dict(decision_sha256=SHA, state="pending")))

    def test_the_inbox_is_bounded_and_old_entries_are_removed(self):
        import os, time
        kinds = ["solana_exit_r8_paid:" + f"{n:02x}" * 48 + ":9:0:" + "cd" * 64 for n in range(door.MAX_ENTRIES + 1)]
        for kind in kinds[:-1]:
            self.assertEqual(self.ask(self.review(amendment=dict(AMEND, kind=kind)))[0], 0)
        code, _, err = self.ask(self.review(amendment=dict(AMEND, kind=kinds[-1])))
        self.assertEqual(code, door.REFUSED)
        self.assertIn("already wait", err)
        # A decision that already waits may be handed in again.
        self.assertEqual(self.ask(self.review(amendment=dict(AMEND, kind=kinds[0]), proposal_slot=11))[0], 0)
        # After a day an entry is removed and there is room again.
        first = self.inbox / (hashlib.sha256(kinds[1].encode()).hexdigest() + ".json")
        old = time.time() - door.RETAIN_SECONDS - 60
        os.utime(first, (old, old))
        self.assertEqual(self.ask(self.review(amendment=dict(AMEND, kind=kinds[-1])))[0], 0)
        self.assertFalse(first.exists())
        self.assertEqual([p.name for p in self.inbox.iterdir() if p.name.endswith(".tmp")], [])

    def test_the_result_is_the_hosts_signature_or_its_refusal(self):
        for result in (dict(authorization={"v": 3}), dict(refused="no review of this decision on validator-3")):
            (self.outbox / f"{SHA}.json").write_text(json.dumps(dict(result, decision_sha256=SHA,
                                                                     proposal_slot=10, expires_at_height=40)))
            code, out, _ = self.ask(dict(request="result", decision_sha256=SHA))
            self.assertEqual(code, 0)
            self.assertEqual(json.loads(out), dict(result, decision_sha256=SHA, proposal_slot=10,
                                                   expires_at_height=40))
        code, out, _ = self.ask(dict(request="result", decision_sha256="ef" * 32))
        self.assertEqual(json.loads(out)["state"], "unknown")

    def test_anything_else_is_refused_and_writes_nothing(self):
        cases = [
            ("a command", dict(request="result", decision_sha256=SHA), "cat /etc/shadow"),
            ("another request", dict(request="shell"), ""),
            ("a path", dict(request="result", decision_sha256="../../etc/passwd"), ""),
            ("an extra field", dict(self.review(), file="/tmp/x"), ""),
            ("not an exit decision", self.review(amendment=dict(AMEND, kind="route:abc")), ""),
            ("a paused decision", self.review(amendment=dict(AMEND, paused=True)), ""),
            ("a value other than one", self.review(amendment=dict(AMEND, value=0)), ""),
            ("a slot that is not an integer", self.review(proposal_slot="10; rm"), ""),
            ("oversized", b"{" + b" " * door.LIMIT + b"}", ""),
            ("not JSON", b"\x00\xff", ""),
        ]
        for name, request, original in cases:
            with self.subTest(name):
                code, out, err = self.ask(request, original)
                self.assertEqual(code, door.REFUSED)
                self.assertEqual(out, "")
                self.assertTrue(err.startswith("refused: "), err)
        self.assertEqual(list(self.inbox.iterdir()), [])


if __name__ == "__main__":
    unittest.main()
