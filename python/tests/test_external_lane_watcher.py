"""The external lane watcher (design section 5.2) against mocked PFTL and Base RPCs."""

from __future__ import annotations

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from typing import Any

from postfiat_rpc import external_lane_watcher as watcher

LANE = "base-erc20"
LOCK = "0x1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a1a"
HOLDER = "0x" + "11" * 20
LOCK_CODE = b"\x60\x80PFTLockV1 runtime"
FINALIZED = 1_000


def recipient(index: int) -> str:
    return "pf" + f"{index:040x}"


def keccak_hex(data: bytes) -> str:
    return "0x" + watcher.keccak256(data).hex()


def release(sequence: int, *, nonce: int | None = None, amount: int = 5_000, height: int | None = None,
            source_block: int = 900, to: str | None = None) -> dict[str, Any]:
    return {
        "sequence": sequence,
        "record": {
            "lock_nonce": sequence if nonce is None else nonce,
            "source_chain_id": 8453,
            "lock_contract_address": LOCK,
            "amount_atoms": amount,
            "recipient": recipient(sequence) if to is None else to,
            "source_block_number": source_block,
            "source_block_hash": "0x" + "6f" * 32,
            "released_at_height": 10 + sequence if height is None else height,
        },
    }


class FakePftl:
    """`external_lane_info` paged as the node pages it: newest first, a cursor
    of strictly older sequences, `next_release_cursor` absent on the last page."""

    def __init__(self, releases: list[dict[str, Any]], *, pruned: int = 0, current_height: int = 500,
                 code_hash: str | None = None, chain_id: int = 8453) -> None:
        self.releases = releases
        self.pruned = pruned
        self.current_height = current_height
        self.code_hash = code_hash or keccak_hex(LOCK_CODE)
        self.chain_id = chain_id
        self.calls: list[tuple[str | None, int | None, int | None]] = []

    def external_lane_info(self, lane_id: str | None = None, *, release_cursor: int | None = None,
                           release_limit: int | None = None) -> dict[str, Any]:
        self.calls.append((lane_id, release_cursor, release_limit))
        limit = release_limit or 100
        count = self.pruned + len(self.releases)
        end = count if release_cursor is None else release_cursor
        start = max(self.pruned, end - limit)
        page = [entry for entry in self.releases if start <= entry["sequence"] < end]
        page.sort(key=lambda entry: entry["sequence"], reverse=True)
        row = {
            "lane_id": LANE,
            "config": {
                "id": LANE,
                "reserved_atoms": 10**12,
                "release_window_blocks": 100,
                "release_cap_bps": 200,
                "policy": {
                    "chain_id": self.chain_id,
                    "lock_contract_address": LOCK,
                    "token_address": "0x" + "2b" * 20,
                    "token_runtime_code_hash": "0x" + "3c" * 32,
                    "lock_runtime_code_hash": self.code_hash,
                    "committee_epoch": 1,
                    "committee_root": "0x" + "5e" * 48,
                    "confirmation_depth": 64,
                },
            },
            "release_count": count,
            "pruned_release_count": self.pruned,
            "releases": page,
        }
        if start > self.pruned:
            row["next_release_cursor"] = start
        return {"schema": "postfiat-external-lane-info-v1", "current_height": self.current_height,
                "lane_count": 1, "lanes": [row]}


class FakeBase:
    """A Base JSON-RPC that serves only reads, and records the block tag of
    every state read."""

    def __init__(self, locks: dict[int, tuple[str, int, str]], *, chain_id: int = 8453,
                 code: bytes = LOCK_CODE, finalized: int = FINALIZED) -> None:
        self.locks = locks
        self.chain_id = chain_id
        self.code = code
        self.finalized = finalized
        self.state_read_tags: list[str] = []
        self.methods: list[str] = []

    def call(self, method: str, params: list[Any]) -> Any:
        self.methods.append(method)
        if method == "eth_chainId":
            return hex(self.chain_id)
        if method == "eth_getBlockByNumber":
            assert params == ["finalized", False], params
            return {"number": hex(self.finalized), "hash": "0x" + "ab" * 32}
        if method == "eth_getCode":
            assert params[0].lower() == LOCK
            self.state_read_tags.append(params[1])
            return "0x" + self.code.hex()
        if method == "eth_call":
            call, tag = params
            self.state_read_tags.append(tag)
            assert call["to"].lower() == LOCK
            data = call["data"]
            assert data[:10] == "0x" + watcher.LOCK_RECORD_SELECTOR, data
            nonce = int(data[10:], 16)
            holder, amount, recipient_hash = self.locks.get(nonce, ("0x" + "00" * 20, 0, "0x" + "00" * 32))
            words = holder[2:].rjust(64, "0") + f"{amount:064x}" + recipient_hash[2:]
            return "0x" + words
        raise AssertionError(f"the watcher called a non-read method {method}")


def backed(*releases: dict[str, Any]) -> dict[int, tuple[str, int, str]]:
    return {
        entry["record"]["lock_nonce"]: (HOLDER, entry["record"]["amount_atoms"],
                                        keccak_hex(entry["record"]["recipient"].encode()))
        for entry in releases
    }


def run(pftl: FakePftl, base: FakeBase, *extra: str) -> tuple[int, list[dict[str, Any]], str]:
    stdout, stderr = io.StringIO(), io.StringIO()
    argv = ["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc", "http://127.0.0.1:8545", *extra]
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        code = watcher.main(argv, pftl=pftl, base=base)
    lines = [json.loads(line) for line in stdout.getvalue().splitlines() if line.strip()]
    return code, lines, stderr.getvalue()


def alarm_codes(lines: list[dict[str, Any]]) -> list[str]:
    return [line["code"] for line in lines if line["kind"] == "alarm"]


class KeccakTests(unittest.TestCase):
    def test_keccak256_matches_the_ethereum_vectors(self) -> None:
        self.assertEqual(watcher.keccak256(b"").hex(),
                         "c5d2460186f7233c927e7db2dcc703c0e500b653ca82273b7bfad8045d85a470")
        self.assertEqual(watcher.keccak256(b"Transfer(address,address,uint256)").hex(),
                         "ddf252ad1be2c89b69c2b068fc378daa952ba7f163c4a11628f55a4df523b3ef")
        self.assertEqual(watcher.keccak256(b"a" * 200).hex(),
                         watcher.keccak256(b"a" * 136 + b"a" * 64).hex())
        self.assertEqual(watcher.LOCK_RECORD_SELECTOR, watcher.keccak256(b"lockRecord(uint64)")[:4].hex())


class WatcherTests(unittest.TestCase):
    def test_every_backed_release_is_checked_at_the_finalized_block(self) -> None:
        releases = [release(0), release(1), release(2)]
        base = FakeBase(backed(*releases))
        code, lines, _ = run(FakePftl(releases), base)
        self.assertEqual(code, 0)
        checked = [line for line in lines if line["kind"] == "release"]
        self.assertEqual([line["sequence"] for line in checked], [0, 1, 2], "oldest first")
        self.assertTrue(all(line["status"] == "backed" for line in checked))
        self.assertEqual(alarm_codes(lines), [])
        summary = lines[-1]
        self.assertEqual(summary["kind"], "summary")
        self.assertEqual((summary["releases_checked"], summary["alarms"]), (3, 0))
        self.assertEqual(summary["finalized_block"], FINALIZED)
        self.assertEqual(set(base.state_read_tags), {hex(FINALIZED)}, "every state read at the finalized block")
        self.assertTrue(set(base.methods) <= {"eth_chainId", "eth_getBlockByNumber", "eth_getCode", "eth_call"})

    def test_each_false_release_is_a_typed_alarm(self) -> None:
        good = release(0)
        cases = {
            "release_without_lock": (release(1), {}),
            "release_amount_mismatch": (release(1), {1: (HOLDER, 4_999, keccak_hex(recipient(1).encode()))}),
            "release_recipient_mismatch": (release(1), {1: (HOLDER, 5_000, keccak_hex(recipient(9).encode()))}),
            "release_block_not_finalized": (release(1, source_block=FINALIZED + 1), {}),
        }
        for expected, (false_release, locks) in cases.items():
            with self.subTest(expected):
                code, lines, _ = run(FakePftl([good, false_release]), FakeBase({**backed(good), **locks}))
                self.assertEqual(code, 2)
                self.assertEqual(alarm_codes(lines), [expected])
                alarm = next(line for line in lines if line["kind"] == "alarm")
                self.assertEqual((alarm["lane_id"], alarm["lock_nonce"], alarm["sequence"]), (LANE, 1, 1))
                statuses = {line["sequence"]: line["status"] for line in lines if line["kind"] == "release"}
                self.assertEqual(statuses, {0: "backed", 1: "alarm"})
                self.assertEqual(lines[-1]["alarms"], 1)

    def test_a_release_whose_amount_and_recipient_both_differ_raises_both(self) -> None:
        false_release = release(0)
        code, lines, _ = run(FakePftl([false_release]),
                             FakeBase({0: (HOLDER, 1, keccak_hex(b"someone else"))}))
        self.assertEqual(code, 2)
        self.assertEqual(alarm_codes(lines), ["release_amount_mismatch", "release_recipient_mismatch"])

    def test_a_base_rpc_on_another_chain_is_refused(self) -> None:
        releases = [release(0)]
        code, lines, stderr = run(FakePftl(releases), FakeBase(backed(*releases), chain_id=84532))
        self.assertEqual(code, 1)
        self.assertEqual(lines, [])
        error = json.loads(stderr)
        self.assertEqual(error["code"], "base_chain_id_mismatch")

    def test_a_lock_contract_whose_code_hash_differs_from_the_pin_is_an_alarm(self) -> None:
        releases = [release(0)]
        code, lines, _ = run(FakePftl(releases), FakeBase(backed(*releases), code=b"another lock"))
        self.assertEqual(code, 2)
        self.assertEqual(alarm_codes(lines), ["lock_code_hash_mismatch"])
        alarm = lines[0]
        self.assertEqual(alarm["expected"], keccak_hex(LOCK_CODE))
        self.assertEqual(alarm["observed"], keccak_hex(b"another lock"))
        # A lock contract with no code at all is the same alarm.
        code, lines, _ = run(FakePftl(releases), FakeBase(backed(*releases), code=b""))
        self.assertEqual((code, alarm_codes(lines)), (2, ["lock_code_hash_mismatch"]))

    def test_the_state_file_resumes_after_the_last_checked_height(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            state_file = str(Path(root) / "watcher.json")
            releases = [release(0), release(1)]
            pftl = FakePftl(releases, current_height=100)
            code, lines, _ = run(pftl, FakeBase(backed(*releases)), "--state-file", state_file)
            self.assertEqual(code, 0)
            state = json.loads(Path(state_file).read_text())
            self.assertEqual((state["lane_id"], state["checked_through_height"], state["release_count"]),
                             (LANE, 100, 2))
            # A new release lands above the checked height: only it is checked.
            releases.append(release(2, height=150))
            pftl = FakePftl(releases, current_height=200)
            code, lines, _ = run(pftl, FakeBase(backed(*releases)), "--state-file", state_file)
            self.assertEqual(code, 0)
            self.assertEqual([line["sequence"] for line in lines if line["kind"] == "release"], [2])
            self.assertEqual(lines[-1]["checked_from_height"], 101)
            self.assertEqual(json.loads(Path(state_file).read_text())["checked_through_height"], 200)
            # An alarm keeps the state where it was, so the next run raises it again.
            releases.append(release(3, height=250))
            pftl = FakePftl(releases, current_height=300)
            for _ in range(2):
                code, lines, _ = run(pftl, FakeBase(backed(*releases[:3])), "--state-file", state_file)
                self.assertEqual((code, alarm_codes(lines)), (2, ["release_without_lock"]))
            self.assertEqual(json.loads(Path(state_file).read_text())["checked_through_height"], 200)
            # --from-height starts past it.
            code, lines, _ = run(pftl, FakeBase(backed(*releases[:3])), "--state-file", state_file,
                                 "--from-height", "251")
            self.assertEqual(code, 0)
            self.assertEqual(json.loads(Path(state_file).read_text())["checked_through_height"], 300)

    def test_a_state_file_of_another_lane_is_refused(self) -> None:
        with tempfile.TemporaryDirectory() as root:
            state_file = Path(root) / "watcher.json"
            state_file.write_text(json.dumps({"schema": watcher.STATE_SCHEMA, "lane_id": "other-lane",
                                              "checked_through_height": 5, "release_count": 0}))
            code, lines, stderr = run(FakePftl([]), FakeBase({}), "--state-file", str(state_file))
            self.assertEqual((code, lines), (1, []))
            self.assertEqual(json.loads(stderr)["code"], "state_file_invalid")

    def test_releases_are_read_across_pages(self) -> None:
        releases = [release(sequence) for sequence in range(250)]
        pftl = FakePftl(releases)
        code, lines, _ = run(pftl, FakeBase(backed(*releases)))
        self.assertEqual(code, 0)
        self.assertEqual([line["sequence"] for line in lines if line["kind"] == "release"], list(range(250)))
        self.assertEqual([cursor for _, cursor, _ in pftl.calls], [None, 150, 50])
        # Paging stops below the first height asked for.
        pftl = FakePftl(releases)
        code, lines, _ = run(pftl, FakeBase(backed(*releases)), "--from-height", str(10 + 200))
        self.assertEqual(code, 0)
        self.assertEqual([line["sequence"] for line in lines if line["kind"] == "release"], list(range(200, 250)))
        self.assertEqual([cursor for _, cursor, _ in pftl.calls], [None])

    def test_releases_pruned_before_they_were_checked_are_an_alarm(self) -> None:
        releases = [release(sequence) for sequence in range(3, 5)]
        code, lines, _ = run(FakePftl(releases, pruned=3), FakeBase(backed(*releases)))
        self.assertEqual(code, 2)
        self.assertEqual(alarm_codes(lines), ["release_pruned_before_check"])
        self.assertEqual(lines[0]["count"], 3)

    def test_a_malformed_release_record_is_refused(self) -> None:
        cases = {
            "missing amount": lambda entry: entry["record"].pop("amount_atoms"),
            "string amount": lambda entry: entry["record"].update(amount_atoms="5000"),
            "negative nonce": lambda entry: entry["record"].update(lock_nonce=-1),
            "nonce above u64": lambda entry: entry["record"].update(lock_nonce=1 << 64),
            "recipient not text": lambda entry: entry["record"].update(recipient=7),
            "another lock contract": lambda entry: entry["record"].update(lock_contract_address="0x" + "99" * 20),
            "another source chain": lambda entry: entry["record"].update(source_chain_id=1),
        }
        for name, corrupt in cases.items():
            with self.subTest(name):
                entry = release(0)
                corrupt(entry)
                code, lines, stderr = run(FakePftl([entry]), FakeBase({}))
                self.assertEqual(code, 1)
                self.assertFalse([line for line in lines if line["kind"] == "release"])
                self.assertEqual(json.loads(stderr)["code"], "release_record_malformed")

    def test_a_lane_the_node_does_not_report_is_refused(self) -> None:
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = watcher.main(["--pftl-rpc", "127.0.0.1:27650", "--lane-id", "other-lane",
                                 "--base-rpc", "http://127.0.0.1:8545"], pftl=FakePftl([]), base=FakeBase({}))
        self.assertEqual(code, 1)
        self.assertEqual(json.loads(stderr.getvalue())["code"], "external_lane_not_reported")

    def test_interval_mode_repeats_and_keeps_watching_after_an_rpc_failure(self) -> None:
        releases = [release(0)]

        class FlakyBase(FakeBase):
            failed = False

            def call(self, method: str, params: list[Any]) -> Any:
                if method == "eth_chainId" and not FlakyBase.failed:
                    FlakyBase.failed = True
                    raise watcher.pft_token.PftTokenError("eth_chainId: the RPC request failed")
                return super().call(method, params)

        sleeps: list[float] = []
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = watcher.main(["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc",
                                 "http://127.0.0.1:8545", "--interval", "30"],
                                pftl=FakePftl(releases), base=FlakyBase(backed(*releases)),
                                sleep=sleeps.append, max_runs=2)
        self.assertEqual(code, 0)
        self.assertEqual(sleeps, [30.0])
        self.assertEqual(json.loads(stderr.getvalue())["code"], "base_rpc_failed")
        summaries = [json.loads(line) for line in stdout.getvalue().splitlines() if '"summary"' in line]
        self.assertEqual(len(summaries), 1)

    def test_the_command_line_refuses_bad_input(self) -> None:
        for argv in (
            ["--lane-id", LANE, "--base-rpc", "http://127.0.0.1:8545"],
            ["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc", "ftp://host"],
            ["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc", "http://h", "--interval", "0"],
            ["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc", "http://h", "--once",
             "--interval", "5"],
        ):
            with self.subTest(argv=argv):
                stderr = io.StringIO()
                with contextlib.redirect_stderr(stderr), contextlib.redirect_stdout(io.StringIO()):
                    try:
                        code = watcher.main(argv)
                    except SystemExit as exit_:
                        code = exit_.code
                self.assertNotEqual(code, 0)


if __name__ == "__main__":
    unittest.main()
