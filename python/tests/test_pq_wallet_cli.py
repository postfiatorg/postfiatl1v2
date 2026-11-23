"""Merge gate. Run with PYTHONPATH=python python3 -m unittest discover -s python/tests -p test_pq_wallet_cli.py."""
import base64
import contextlib
import hashlib
import hmac
import json
import os
from pathlib import Path
import socket
import socketserver
import shutil
import statistics
import subprocess
import sys
import tempfile
import threading
import time
import unittest

ROOT = Path(__file__).resolve().parents[2]
NATIVE = "00"*48


class PqIssuedFee(unittest.TestCase):
    def test_private_spends_omit_the_depositor_from_review_and_relay(self):
        from postfiat_rpc.pq_browser import BrowserWallet, PQ_POOL_ID
        from postfiat_rpc.pq_wallet import parser

        browser = object.__new__(BrowserWallet)
        browser.issue = "cd" * 48
        browser.account = "pf" + "11" * 20
        browser.overview = lambda db, *, public: dict(synced=True, pending=None, sequence="7", issued_fee="7")
        vectors = ROOT / "tools/pft-binius-shielded-pq/l1-vectors"
        for command, mode, recipient, vector in [
            ("pay", "Internal", "x" * 2241, "pfusdc-pay.bin"),
            ("unshield", "Exit", "pf" + "22" * 20, "pfusdc-exit.bin"),
        ]:
            args = parser().parse_args(["--wallet", ".", command, "--asset", browser.issue,
                                        "--amount", "5", "--to", recipient])
            self.assertTrue({"payer", "account_key", "sequence"}.isdisjoint(vars(args)))
            order = dict(mode=mode, asset=browser.issue, amount="5", fee="7",
                         recipient=recipient, migration=False)
            with self.assertRaisesRegex(ValueError, "fee changed"):
                browser.dispatch(None, "review", {**order, "fee":"3"})
            terms = browser.dispatch(None, "review", order)["terms"]
            self.assertIsNone(terms["payer"])
            self.assertIsNone(terms["sequence"])
            action = dict(kind="pq_payment_v1", body=base64.b64encode((vectors / vector).read_bytes()).decode())
            request = browser.request(dict(pool=PQ_POOL_ID, intent="example", wire=json.dumps(action)))
            self.assertIsNone(request["address"])


    def test_cli_refuses_a_caller_selected_issued_fee(self):
        from postfiat_rpc.pq_wallet import parser, run
        for command in ("pay", "unshield"):
            args = parser().parse_args(["--wallet", ".", "--endpoint", "127.0.0.1:1",
                command, "--asset", "cd"*48, "--amount", "1", "--fee", "2", "--to", "unused"])
            with self.assertRaisesRegex(ValueError, "Omit --fee"):
                run(args)

    def test_only_confirmed_pre_reservation_refusal_releases_pending_payment(self):
        import sqlite3
        from unittest.mock import Mock, patch
        from postfiat_rpc.pq_browser import BrowserWallet, AdmissionRefused, PQ_POOL_ID
        from postfiat_rpc.client import private_action_hash

        vector = ROOT / "tools/pft-binius-shielded-pq/l1-vectors/pfusdc-pay.bin"
        wire = json.dumps(dict(kind="pq_payment_v1", body=base64.b64encode(vector.read_bytes()).decode()))
        row = dict(intent="test", phase="prepared", wire=wire, pool=PQ_POOL_ID)
        wallet = object.__new__(BrowserWallet)
        wallet.account, wallet.home = "pf" + "11"*20, Path(".")
        wallet.gateway = Mock()
        with contextlib.closing(sqlite3.connect(":memory:")) as db:
            db.execute("CREATE TABLE payments (intent,phase)")
            for exc in [ConnectionResetError(), AdmissionRefused("pq_fee_mismatch")]:
                db.execute("DELETE FROM payments")
                db.execute("INSERT INTO payments VALUES ('test', 'prepared')")
                wallet.gateway.call.side_effect = exc
                with patch("postfiat_rpc.pq_browser.local") as daemon:
                    with self.assertRaises((ConnectionResetError, ValueError)):
                        wallet.send(db, row)
                    saved = db.execute("SELECT phase FROM payments").fetchone()
                    if isinstance(exc, ConnectionResetError):
                        daemon.assert_not_called()
                        self.assertEqual(saved, ("submitted",))
                    else:
                        daemon.assert_called_once_with(Path("."), "rejected", action_hash=private_action_hash(wire))
                        self.assertIsNone(saved)

    def test_rehearsal_release_contract_is_visible_to_wallets_and_reviewers(self):
        spec = (ROOT / "docs/specs/pq-private-payments-integration.md").read_text()
        status = (ROOT / "docs/quantum/private-payments.md").read_text()
        package = (ROOT / "docs/evidence/pq-private-payments-m8/external-review-package.md").read_text()
        for document in (spec, status, package):
            self.assertIn("0.000001 pfUSDC", document)
            self.assertIn("profile 2", document)
        self.assertIn("pq_issued_fee", spec)
        self.assertIn("wind-down", spec)
        history = (ROOT / "docs/rpc/account-history.md").read_text()
        self.assertIn("collector withdrawal", history)
        self.assertNotIn("reads the same kinds", history)


class PqWalletMerge(unittest.TestCase):
    def test_two_wallets_restore_and_spend(self):
        cargo = ["cargo", "build", "--offline", "--locked", "--release", "-p", "postfiat-node",
                 "--features", "pq-wallet", "--bin", "pftl-pq-walletd"]
        subprocess.run(cargo, cwd=ROOT, check=True)
        subprocess.run(["cargo", "test", "--offline", "--locked", "--release", "-p", "postfiat-node",
                        "--lib", "--features", "pq-wallet", "tests::pq_payment_accounting::wallet_cli::",
                        "--", "--nocapture"], cwd=ROOT, check=True)


def exercise(root, stack):
    faults = []
    packets = []
    transcript = []
    stop_daemons = []
    known = set()
    host, port = os.environ["PQ_ENDPOINT"].rsplit(":", 1)

    class Tap(socketserver.StreamRequestHandler):
        def handle(self):
            raw = self.rfile.readline(1_048_577)
            packets.append(raw)
            with socket.create_connection((host, int(port))) as s:
                s.sendall(raw)
                s.shutdown(socket.SHUT_WR)
                with s.makefile("rb") as server:
                    reply = server.read()
                if self.server is proxy and faults and json.loads(raw)["params"].get("section") == "pq_pairs":
                    message = json.loads(reply)
                    page = message["result"]
                    fault = faults.pop(0)
                    if fault == "corrupt":
                        pair = page["items"][0]
                        pair["serials"].reverse()
                    elif fault == "expire":
                        page.update(status="refused", refusal={"code":"snapshot_expired"})
                    reply = json.dumps(message).encode()
                self.wfile.write(reply)

    proxy = stack.enter_context(socketserver.TCPServer(("127.0.0.1", 0), Tap))
    verified = stack.enter_context(socketserver.TCPServer(("127.0.0.1", 0), Tap))
    for server in (proxy, verified):
        threading.Thread(target=server.serve_forever, daemon=True).start()
        stack.callback(server.shutdown)
    endpoint = f"127.0.0.1:{proxy.server_address[1]}"
    env = {**os.environ, "PYTHONPATH": str(ROOT / "python")}
    target = Path(env.get("CARGO_TARGET_DIR", ROOT / "target"))
    binary = root / "pftl-pq-walletd"
    shutil.copy2(target / "release/pftl-pq-walletd", binary)
    cpu = int(subprocess.check_output([sys.executable, str(ROOT/"tools/pft-binius-shielded-pq/idle_cpu.py")]))
    print(f"wallet benchmark CPU {cpu}", flush=True)

    def args(w, *words):
        return [sys.executable, "-m", "postfiat_rpc", "pq-wallet", "--wallet", str(root/w),
                "--endpoint", endpoint, "--validator-endpoint", f"127.0.0.1:{verified.server_address[1]}", *map(str, words)]

    def cli(w, *words, fail=False):
        r = subprocess.run(args(w, *words), env=env, capture_output=True, timeout=240)
        transcript.extend([r.stderr, r.stdout])
        if fail:
            assert r.returncode != 0
            return
        if r.returncode != 0:
            raise AssertionError(f"{words[0]} failed: {r.stderr.decode()[:250]}")
        return json.loads(r.stdout)

    def daemon(w, k):
        p = subprocess.Popen(["taskset", "-c", str(cpu), *args(w, "daemon", "--binary", binary, "--key-file", k)],
                             env={**env, "RAYON_NUM_THREADS": "1"}, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        def close():
            if p.stdout.closed:
                return
            if p.poll() is None:
                p.terminate()
            transcript.extend(p.communicate(timeout=10))
        stop_daemons.append(close)
        stack.callback(close)
        for _ in range(150):
            if (root/w/"wallet.sock").exists():
                try:
                    with socket.socket(socket.AF_UNIX) as sock:
                        sock.connect(str(root/w/"wallet.sock"))
                    break
                except OSError:
                    pass
            if p.poll() is not None:
                raise AssertionError("daemon failed to start")
            time.sleep(.02)
        return p

    from cryptography.hazmat.primitives.ciphers.aead import AESGCM

    def inspect(w, key):
        enc = (root/w/"wallet.enc").read_bytes()
        wrapping = key.read_bytes()
        state = json.loads(AESGCM(wrapping).decrypt(enc[:12], enc[12:], b"postfiat/pq-wallet/v1/state"))
        rec = state["recovery"]
        seed = bytes.fromhex(rec["seed"])
        net = rec["birthday"]["network"]
        known.update([wrapping, seed])
        for ix in range(rec["first"], rec["first"]+rec["count"]):
            for role, length in [("kem", 64), ("spend", 48)]:
                fields = [f"postfiat/pq-wallet/v1/{role}".encode(), net["chain_id"].encode(),
                          bytes.fromhex(net["genesis_hash"]), rec["account"].to_bytes(4,"big"), ix.to_bytes(4,"big")]
                info = b"".join(len(v).to_bytes(4, "big")+v for v in fields)
                prk = hmac.digest(b"postfiat/pq-wallet/v1/hkdf-salt", seed, "sha384")
                a = hmac.digest(prk, info+b"\x01", "sha384")
                known.add((a+hmac.digest(prk, a+info+b"\x02","sha384"))[:length])
        for note in state["notes"].values():
            known.add(bytes.fromhex(note["blind"]))
        assert (root/w/"wallet.enc").stat().st_mode & 0o077 == 0
        return state

    alice, bob = "alice", "bob"
    akey, bkey, backup = root/"a.key", root/"b.key", root/"bob.backup"
    daemons = {}
    for w, key in [(alice, akey), (bob, bkey)]:
        cli(w, "keygen", "--key-file", key)
        daemons[w] = daemon(w, key)
        cli(w, "init", "--count", 2)
        assert (root/w/"wallet.sock").stat().st_mode & 0o077 == 0
    cli(bob, "backup", "--file", backup)
    cli(bob, "backup", "--file", bkey, fail=True)
    cli(bob, "backup", "--file", root/bob/"wallet.enc", fail=True)
    cli(bob, "backup", "--file", backup, fail=True)
    aa = cli(alice, "address")["address"]
    ba = cli(bob, "address", "--index", 1)["address"]
    issue = env["PQ_ISSUE"]
    metrics = []

    def public(who, seq):
        return ["--payer", env[f"PQ_{who}"], "--account-key", env[f"PQ_{who}_KEY"], "--sequence", seq]

    def pay(w, mode, asset, amount, *more):
        fee_args = ["--fee", 1] if asset == "PFT" or mode == "shield" else []
        r = cli(w, mode, "--asset", asset, "--amount", amount, *fee_args, *more)
        assert r["state"] == "accepted"
        assert cli(w, "receipt", r["intent_id"])["outcome"]["accepted"]
        metrics.append((r["prove_seconds"],r["worker_peak_rss_kib"]))
        print(f"{mode} {asset[:8]} {amount}: accepted; prove {metrics[-1][0]:.3f} s, worker peak {metrics[-1][1]} KiB", flush=True)
        return r

    # Cancel while the native worker is proving, before an intent can be sent.
    with subprocess.Popen(args(alice, "shield", "--asset", "PFT", "--amount", 100, "--fee", 1,
                               *public("ALICE", 1)), env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as interrupted:
        for _ in range(50):
            stage = cli(alice, "status")["active"]
            if stage and stage["stage"] == "prove":
                break
            time.sleep(.05)
        else:
            raise AssertionError("prover never became active")
        assert cli(alice, "cancel")["cancel_requested"]
        transcript.extend(interrupted.communicate(timeout=10))
        assert interrupted.returncode
    assert cli(alice, "balance")["pending"] is None

    first = pay(alice, "shield", "PFT", 100, *public("ALICE", 1))
    assert cli(alice, "balance")["pending"]["delta"] == 99
    # A lost reply or daemon restart replays the exact saved wire.
    daemons[alice].terminate()
    daemons[alice].wait(timeout=10)
    daemon(alice, akey)
    retry = cli(alice, "submit-pending")
    assert retry["intent_id"] == first["intent_id"]
    pay(alice, "pay", "PFT", 30, "--to", ba)
    pay(alice, "shield", issue, 80, *public("ALICE", 2))
    pay(alice, "pay", issue, 25, "--to", ba)
    cli(bob, "sync")
    original = inspect(bob, bkey)
    assert cli(bob, "balance")["finalized"] == {NATIVE:30, issue:25}

    restored = "restored"
    daemon(restored, bkey)
    cli(restored,"restore","--file",backup)
    assert cli(restored,"address","--index",1)["address"] == ba
    assert cli(restored,"balance")["finalized"] == {}
    before = (root/restored/"wallet.enc").read_bytes()
    faults.append("corrupt")
    cli(restored,"sync",fail=True)
    assert (root/restored/"wallet.enc").read_bytes() == before
    faults.append("expire")
    started = time.monotonic()
    scan = cli(restored,"sync")
    elapsed = time.monotonic()-started
    assert not faults
    assert scan["outputs"]==8 and scan["keys"]==2 and scan["sync_lag_blocks"]==0
    state = inspect(restored,bkey)
    assert state["notes"] == original["notes"] and state["point"]["assets"][NATIVE]["epoch"] == 1
    assert sorted(n["value"] for n in state["notes"].values()) == [25,30]
    print(f"restored at height {scan['height']}: 30 PFT, 25 pfUSDC; {scan['pages']} page, "
          f"{scan['outputs']} outputs x {scan['keys']} keys in {scan['trial_seconds']:.6f} s "
          f"({scan['outputs']*scan['keys']/scan['trial_seconds']:.0f} trials/s), full sync with retry {elapsed:.3f} s",flush=True)
    pay(restored,"pay","PFT",10,"--to",aa)
    pay(restored,"unshield","PFT",18,"--to",env["PQ_ALICE"])
    pay(restored,"pay",issue,5,"--to",aa)
    pay(restored,"unshield",issue,14,"--to",env["PQ_SPONSOR"])
    cli(restored,"sync")
    cli(alice,"sync")
    assert cli(restored,"balance")["finalized"] == {}
    assert cli(alice,"balance")["finalized"] == {NATIVE:78,issue:57}
    inspect(alice,akey)
    for who in ("ALICE","SPONSOR"):
        known.add(bytes.fromhex(json.loads(Path(env[f"PQ_{who}_KEY"]).read_bytes())["private_key_hex"]))
    wires = [base64.b64decode(json.loads(req["params"]["action_json"])["body"], validate=True)
             for req in map(json.loads,packets) if req["method"]=="private_action_submit"]
    for request in map(json.loads, packets):
        if request["method"] == "private_action_submit":
            params = request["params"]
            if base64.b64decode(json.loads(params["action_json"])["body"])[71] == 0:
                assert params["session_scope"] == "pq-private-spend"
    for stop in stop_daemons:
        stop()
    traffic = b"\n".join(packets+transcript+wires)
    for secret in known:
        for encoding in (secret,secret.hex().encode(),base64.b64encode(secret)):
            if encoding in traffic:
                raise AssertionError("secret in captured RPC or process output")
    for name in (b'"seed"',b'"witness"',b'"leaf_index"',b'"scan_key"',b'"spend_secret"',b'"account_key"'):
        assert name not in traffic
    print(f"8 finalized payments; identical retry; cancellation and restore passed; "
          f"{len(packets)} RPC requests, signed bodies, CLI and daemon output checked for secret leakage",flush=True)
    print(f"8 pinned proofs: median {statistics.median(x for x,_ in metrics):.3f} s, "
          f"max {max(x for x,_ in metrics):.3f} s, peak worker RSS {max(m for _,m in metrics)} KiB",flush=True)


if __name__ == "__main__":
    if sys.argv[1:]==["--exercise"]:
        with tempfile.TemporaryDirectory(prefix="pq-cli-") as tmp, contextlib.ExitStack() as stack:
            exercise(Path(tmp), stack)
    else:
        unittest.main()
