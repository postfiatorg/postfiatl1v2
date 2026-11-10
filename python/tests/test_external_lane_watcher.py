"""The external lane watcher (design section 5.2) against mocked PFTL and Base RPCs."""

from __future__ import annotations

import contextlib
import io
import json
import re
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
SOURCE_BLOCK_HASH = "0x" + "6f" * 32
GENESIS_HASH = "ce" * 48


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
            "source_block_hash": SOURCE_BLOCK_HASH,
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
        self.pftl_chain_id = "postfiat-local"
        self.genesis_hash = GENESIS_HASH
        self.history_root: str | None = "0x" + "ab" * 48 if releases else None
        self.protocol_version: Any = 1
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
        row["release_history_root"] = self.history_root
        return {"schema": "postfiat-external-lane-info-v1", "chain_id": self.pftl_chain_id,
                "genesis_hash": self.genesis_hash, "protocol_version": self.protocol_version,
                "current_height": self.current_height, "lane_count": 1, "lanes": [row]}


class FakeBase:
    """A Base JSON-RPC that serves only reads, and records the block tag of
    every state read."""

    def __init__(self, locks: dict[int, tuple[str, int, str]], *, chain_id: int = 8453,
                 code: bytes = LOCK_CODE, finalized: int = FINALIZED,
                 block_hashes: dict[int, str] | None = None, lock_from: dict[int, int] | None = None) -> None:
        self.locks = locks
        self.chain_id = chain_id
        self.code = code
        self.finalized = finalized
        # The canonical hash of each Base block (default: the release records'),
        # and the first block at which each Lock exists (default: block 0).
        self.block_hashes = block_hashes or {}
        self.lock_from = lock_from or {}
        self.state_read_tags: list[str] = []
        self.lock_read_tags: list[str] = []
        self.block_reads: list[str] = []
        self.methods: list[str] = []

    def call(self, method: str, params: list[Any]) -> Any:
        self.methods.append(method)
        if method == "eth_chainId":
            return hex(self.chain_id)
        if method == "eth_getBlockByNumber":
            tag, full = params
            assert full is False, params
            self.block_reads.append(tag)
            if tag == "finalized":
                return {"number": hex(self.finalized), "hash": "0x" + "ab" * 32}
            number = int(tag, 16)
            assert number <= self.finalized, "only finalized blocks are read by number"
            return {"number": tag, "hash": self.block_hashes.get(number, SOURCE_BLOCK_HASH)}
        if method == "eth_getCode":
            assert params[0].lower() == LOCK
            self.state_read_tags.append(params[1])
            return "0x" + self.code.hex()
        if method == "eth_call":
            call, tag = params
            self.state_read_tags.append(tag)
            self.lock_read_tags.append(tag)
            assert call["to"].lower() == LOCK
            data = call["data"]
            assert data[:10] == "0x" + watcher.LOCK_RECORD_SELECTOR, data
            nonce = int(data[10:], 16)
            absent = ("0x" + "00" * 20, 0, "0x" + "00" * 32)
            exists = nonce in self.locks and int(tag, 16) >= self.lock_from.get(nonce, 0)
            holder, amount, recipient_hash = self.locks[nonce] if exists else absent
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
    def test_every_backed_release_is_checked_at_its_own_source_block(self) -> None:
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
        # The code hash at the finalized block; each Lock at its release's own
        # source block, whose hash was read and matched first.
        self.assertEqual(base.state_read_tags[0], hex(FINALIZED))
        self.assertEqual(base.lock_read_tags, [hex(900)] * 3)
        self.assertEqual(base.block_reads, ["finalized"] + [hex(900)] * 3)
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

    def test_a_lock_made_after_the_release_does_not_back_it(self) -> None:
        # The Lock exists at the finalized head but not at the release's own
        # source block 900: it was made later and cannot back the release.
        false_release = release(0)
        base = FakeBase(backed(false_release), lock_from={0: 950})
        code, lines, _ = run(FakePftl([false_release]), base)
        self.assertEqual(code, 2)
        self.assertEqual(alarm_codes(lines), ["release_without_lock"])
        self.assertEqual(base.lock_read_tags, [hex(900)], "the Lock is read at the source block")

    def test_a_source_block_whose_hash_differs_is_an_alarm(self) -> None:
        good, false_release = release(0, source_block=800), release(1)
        base = FakeBase(backed(good, false_release), block_hashes={900: "0x" + "77" * 32})
        code, lines, _ = run(FakePftl([good, false_release]), base)
        self.assertEqual(code, 2)
        self.assertEqual(alarm_codes(lines), ["release_source_block_mismatch"])
        alarm = next(line for line in lines if line["kind"] == "alarm")
        self.assertEqual((alarm["sequence"], alarm["source_block_number"]), (1, 900))
        self.assertEqual((alarm["source_block_hash"], alarm["canonical_block_hash"]),
                         (SOURCE_BLOCK_HASH, "0x" + "77" * 32))
        # The mismatched block's Lock is not read.
        self.assertEqual(base.lock_read_tags, [hex(800)])

    def test_a_not_yet_final_release_is_left_for_a_later_run(self) -> None:
        pending = release(0, source_block=FINALIZED + 1)
        base = FakeBase({})
        code, lines, _ = run(FakePftl([pending]), base)
        self.assertEqual((code, alarm_codes(lines)), (2, ["release_block_not_finalized"]))
        self.assertEqual((base.block_reads, base.lock_read_tags), (["finalized"], []))

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
            # Bound to the chain, the genesis and the lane policy.
            self.assertEqual((state["pftl_chain_id"], state["genesis_hash"], state["lock_contract_address"],
                              state["source_chain_id"]), ("postfiat-local", GENESIS_HASH, LOCK, 8453))
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

    def test_a_state_file_of_another_network_or_out_of_range_is_refused(self) -> None:
        good = {"schema": watcher.STATE_SCHEMA, "lane_id": LANE, "pftl_chain_id": "postfiat-local",
                "genesis_hash": GENESIS_HASH, "lock_contract_address": LOCK, "source_chain_id": 8453,
                "checked_through_height": 100, "release_count": 2}
        releases = [release(0), release(1)]
        cases = {
            "another genesis": {"genesis_hash": "dd" * 48},
            "another PFTL chain": {"pftl_chain_id": "postfiat-other"},
            "another lock contract": {"lock_contract_address": "0x" + "99" * 20},
            "another source chain": {"source_chain_id": 84532},
            "height ahead of the node": {"checked_through_height": 501},
            "count ahead of the node": {"release_count": 3},
            "negative height": {"checked_through_height": -1},
            "boolean height": {"checked_through_height": True},
            "boolean count": {"release_count": False},
            "height above u64": {"checked_through_height": 1 << 64},
        }
        with tempfile.TemporaryDirectory() as root:
            state_file = Path(root) / "watcher.json"
            state_file.write_text(json.dumps(good))
            code, _, _ = run(FakePftl(releases, current_height=500), FakeBase(backed(*releases)),
                             "--state-file", str(state_file))
            self.assertEqual(code, 0, "the control state is accepted")
            for name, change in cases.items():
                with self.subTest(name):
                    state_file.write_text(json.dumps({**good, **change}))
                    code, lines, stderr = run(FakePftl(releases, current_height=500), FakeBase(backed(*releases)),
                                              "--state-file", str(state_file))
                    self.assertEqual((code, lines), (1, []))
                    self.assertEqual(json.loads(stderr)["code"], "state_file_invalid")

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

    def test_a_lane_report_that_changes_between_pages_fails_the_run(self) -> None:
        releases = [release(sequence) for sequence in range(250)]

        class MovingPftl(FakePftl):
            """The second page is served from a later state of the chain."""

            def external_lane_info(self, lane_id: str | None = None, **kwargs: Any) -> dict[str, Any]:
                answer = super().external_lane_info(lane_id, **kwargs)
                if len(self.calls) == 1:
                    self.current_height += 1
                return answer

        for name, change in {
            "height": lambda pftl: setattr(pftl, "current_height", pftl.current_height + 1),
            "history root": lambda pftl: setattr(pftl, "history_root", "0x" + "cd" * 48),
            "genesis": lambda pftl: setattr(pftl, "genesis_hash", "dd" * 48),
        }.items():
            with self.subTest(name):
                class Changing(FakePftl):
                    def external_lane_info(self, lane_id: str | None = None, **kwargs: Any) -> dict[str, Any]:
                        if len(self.calls) == 1:
                            change(self)
                        return super().external_lane_info(lane_id, **kwargs)

                code, lines, stderr = run(Changing(releases), FakeBase(backed(*releases)))
                self.assertEqual((code, [line for line in lines if line["kind"] == "summary"]), (1, []))
                self.assertEqual(json.loads(stderr)["code"], "external_lane_info_inconsistent")
        # Under --interval the run is retried; the next scan is consistent.
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = watcher.main(["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc",
                                 "http://127.0.0.1:8545", "--interval", "5"],
                                pftl=MovingPftl(releases), base=FakeBase(backed(*releases)),
                                sleep=lambda _: None, max_runs=2)
        self.assertEqual(code, 0)
        self.assertEqual(json.loads(stderr.getvalue())["code"], "external_lane_info_inconsistent")
        self.assertEqual(len([line for line in stdout.getvalue().splitlines() if '"summary"' in line]), 1)

    def test_a_sequence_gap_is_malformed(self) -> None:
        releases = [release(sequence) for sequence in range(250) if sequence != 120]
        code, lines, stderr = run(FakePftl(releases), FakeBase(backed(*releases)))
        self.assertEqual((code, [line for line in lines if line["kind"] == "summary"]), (1, []))
        self.assertEqual(json.loads(stderr)["code"], "external_lane_info_malformed")
        # A last page that stops above the pruned boundary is a gap too.
        releases = [release(sequence) for sequence in range(1, 4)]
        code, _, stderr = run(FakePftl(releases, pruned=0), FakeBase(backed(*releases)))
        self.assertEqual((code, json.loads(stderr)["code"]), (1, "external_lane_info_malformed"))

    def test_releases_pruned_before_they_were_checked_are_an_alarm(self) -> None:
        releases = [release(sequence) for sequence in range(3, 5)]
        code, lines, _ = run(FakePftl(releases, pruned=3), FakeBase(backed(*releases)))
        self.assertEqual(code, 2)
        self.assertEqual(alarm_codes(lines), ["release_pruned_before_check"])
        self.assertEqual(lines[0]["count"], 3)

    def test_nonces_and_amounts_at_their_u64_boundaries(self) -> None:
        top = (1 << 64) - 1
        for nonce, amount in ((0, 1), (0, top), (top, 1), (top, top)):
            with self.subTest(nonce=nonce, amount=amount):
                entry = release(0, nonce=nonce, amount=amount)
                base = FakeBase(backed(entry))
                code, lines, _ = run(FakePftl([entry]), base)
                self.assertEqual((code, alarm_codes(lines)), (0, []))
                checked = next(line for line in lines if line["kind"] == "release")
                self.assertEqual((checked["lock_nonce"], checked["amount_atoms"]), (nonce, amount))

    def test_a_zero_amount_record_is_malformed(self) -> None:
        # Committed records refuse a zero amount and the lock cannot make a
        # zero Lock, so a zero amount is a malformed read, never `backed`.
        entry = release(0, amount=0)
        code, lines, stderr = run(FakePftl([entry]), FakeBase(backed(entry)))
        self.assertEqual((code, [line for line in lines if line["kind"] == "release"]), (1, []))
        self.assertEqual(json.loads(stderr)["code"], "release_record_malformed")

    def test_malformed_snapshot_fields_are_refused_even_when_every_page_agrees(self) -> None:
        releases = [release(sequence) for sequence in range(150)]
        for name, field, value in (
            ("protocol version zero", "protocol_version", 0),
            ("protocol version boolean", "protocol_version", True),
            ("protocol version text", "protocol_version", "1"),
            ("root without 0x", "history_root", "ab" * 48),
            ("root upper case", "history_root", "0x" + "AB" * 48),
            ("root short", "history_root", "0x" + "ab" * 47),
            ("root not text", "history_root", 7),
        ):
            with self.subTest(name):
                pftl = FakePftl(releases)
                setattr(pftl, field, value)
                code, lines, stderr = run(pftl, FakeBase(backed(*releases)))
                self.assertEqual((code, lines), (1, []))
                self.assertEqual(json.loads(stderr)["code"], "external_lane_info_malformed")

    def test_the_history_root_is_present_exactly_when_releases_exist(self) -> None:
        # Releases without a root: paging would not be bound by it.
        releases = [release(0), release(1)]
        pftl = FakePftl(releases)
        pftl.history_root = None
        code, lines, stderr = run(pftl, FakeBase(backed(*releases)))
        self.assertEqual((code, lines), (1, []))
        self.assertEqual(json.loads(stderr)["code"], "external_lane_info_malformed")
        # A root with no release behind it.
        pftl = FakePftl([])
        pftl.history_root = "0x" + "ab" * 48
        code, lines, stderr = run(pftl, FakeBase({}))
        self.assertEqual((code, lines), (1, []))
        self.assertEqual(json.loads(stderr)["code"], "external_lane_info_malformed")
        # A lane with no release and no root is clean.
        code, lines, _ = run(FakePftl([]), FakeBase({}))
        self.assertEqual((code, lines[-1]["releases_checked"]), (0, 0))

    def test_from_height_replaces_a_state_file_of_an_earlier_schema(self) -> None:
        releases = [release(0), release(1, height=150)]
        with tempfile.TemporaryDirectory() as root:
            state_file = Path(root) / "watcher.json"
            old = {"schema": "postfiat.external_lane_watcher.state.v1", "lane_id": LANE,
                   "checked_through_height": 50, "release_count": 1}
            state_file.write_text(json.dumps(old))
            # Without --from-height the old schema is refused, and the file kept.
            code, lines, stderr = run(FakePftl(releases, current_height=200), FakeBase(backed(*releases)),
                                      "--state-file", str(state_file))
            self.assertEqual((code, lines), (1, []))
            self.assertEqual(json.loads(stderr)["code"], "state_file_invalid")
            self.assertEqual(json.loads(state_file.read_text()), old)
            # With --from-height the run starts there and a clean run replaces the file.
            code, lines, _ = run(FakePftl(releases, current_height=200), FakeBase(backed(*releases)),
                                 "--state-file", str(state_file), "--from-height", "100")
            self.assertEqual(code, 0)
            self.assertEqual([line["sequence"] for line in lines if line["kind"] == "release"], [1])
            state = json.loads(state_file.read_text())
            self.assertEqual((state["schema"], state["checked_through_height"]), (watcher.STATE_SCHEMA, 200))

    def test_a_source_block_answer_must_name_the_block_and_exist(self) -> None:
        entry = release(0)
        for name, answer, words in (
            ("another block", {"number": hex(901), "hash": SOURCE_BLOCK_HASH}, "is block 901"),
            ("no block", None, "does not serve block 900"),
        ):
            with self.subTest(name):
                class Answering(FakeBase):
                    def call(self, method: str, params: list[Any]) -> Any:
                        if method == "eth_getBlockByNumber" and params[0] != "finalized":
                            return answer
                        return super().call(method, params)

                code, lines, stderr = run(FakePftl([entry]), Answering(backed(entry)))
                self.assertEqual((code, [line for line in lines if line["kind"] == "summary"]), (1, []))
                error = json.loads(stderr)
                self.assertEqual(error["code"], "base_answer_malformed")
                self.assertIn(words, error["message"])

    def test_every_emitted_alarm_code_is_declared_and_counted(self) -> None:
        source = Path(watcher.__file__).read_text(encoding="utf-8")
        emitted = set(re.findall(r'(?:alarm|codes\.append)\("([a-z_]+)"', source))
        self.assertEqual(emitted, set(watcher.ALARM_CODES))
        good, false_release = release(0), release(1)
        code, lines, _ = run(FakePftl([good, false_release]), FakeBase(backed(good)))
        self.assertEqual(code, 2)
        counts = lines[-1]["alarm_codes"]
        self.assertEqual(set(counts), set(watcher.ALARM_CODES))
        self.assertEqual({code_: count for code_, count in counts.items() if count},
                         {"release_without_lock": 1})

    def test_a_state_file_that_cannot_be_written_fails_the_run_before_its_summary(self) -> None:
        releases = [release(0)]
        with tempfile.TemporaryDirectory() as root:
            locked = Path(root) / "locked"
            locked.mkdir()
            locked.chmod(0o500)
            try:
                for name, state_file in (
                    ("missing parent", Path(root) / "absent" / "watcher.json"),
                    ("read-only directory", locked / "watcher.json"),
                ):
                    with self.subTest(name):
                        code, lines, stderr = run(FakePftl(releases), FakeBase(backed(*releases)),
                                                  "--state-file", str(state_file))
                        self.assertEqual((code, lines), (1, []), "no summary without saved progress")
                        self.assertEqual(json.loads(stderr)["code"], "state_file_write_failed")
                        self.assertFalse(state_file.exists())
                # Under --interval the write is retried.
                sleeps: list[float] = []
                stdout, stderr = io.StringIO(), io.StringIO()
                with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
                    code = watcher.main(["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc",
                                         "http://127.0.0.1:8545", "--interval", "5", "--state-file",
                                         str(locked / "watcher.json")],
                                        pftl=FakePftl(releases), base=FakeBase(backed(*releases)),
                                        sleep=sleeps.append, max_runs=2)
                self.assertEqual((code, sleeps), (0, [5.0]))
                self.assertEqual([json.loads(line)["code"] for line in stderr.getvalue().splitlines()],
                                 ["state_file_write_failed"] * 2)
                self.assertEqual(stdout.getvalue(), "")
            finally:
                locked.chmod(0o700)

    def test_interval_mode_keeps_running_after_an_alarm_and_keeps_the_state(self) -> None:
        releases = [release(0)]
        with tempfile.TemporaryDirectory() as root:
            state_file = Path(root) / "watcher.json"
            sleeps: list[float] = []
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(io.StringIO()):
                code = watcher.main(["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc",
                                     "http://127.0.0.1:8545", "--interval", "7", "--state-file", str(state_file)],
                                    pftl=FakePftl(releases), base=FakeBase({}),
                                    sleep=sleeps.append, max_runs=2)
            lines = [json.loads(line) for line in stdout.getvalue().splitlines()]
            # The alarm line is emitted on every run; the loop continues and
            # the state does not advance.
            self.assertEqual(code, 0)
            self.assertEqual(sleeps, [7.0])
            self.assertEqual(alarm_codes(lines), ["release_without_lock"] * 2)
            self.assertEqual(len([line for line in lines if line["kind"] == "summary"]), 2)
            self.assertFalse(state_file.exists())

    def test_an_over_wide_amount_word_is_a_malformed_answer(self) -> None:
        entry = release(0)
        # 65 bits: a uint64 field cannot hold it, whatever the low bits say.
        base = FakeBase({0: (HOLDER, (1 << 64) | 5_000, keccak_hex(recipient(0).encode()))})
        code, lines, stderr = run(FakePftl([entry]), base)
        self.assertEqual((code, [line for line in lines if line["kind"] == "summary"]), (1, []))
        self.assertEqual(json.loads(stderr)["code"], "base_answer_malformed")

    def test_a_zero_holder_is_no_lock_even_when_amount_and_hash_match(self) -> None:
        entry = release(0)
        base = FakeBase({0: ("0x" + "00" * 20, 5_000, keccak_hex(recipient(0).encode()))})
        code, lines, _ = run(FakePftl([entry]), base)
        self.assertEqual((code, alarm_codes(lines)), (2, ["release_without_lock"]))

    def test_one_token_base_unit_is_one_atom(self) -> None:
        entry = release(0, amount=1)
        code, lines, _ = run(FakePftl([entry]), FakeBase(backed(entry)))
        self.assertEqual((code, alarm_codes(lines)), (0, []))
        # No six-decimal scaling: a Lock of 10**6 base units is not 1 atom.
        base = FakeBase({0: (HOLDER, 10**6, keccak_hex(recipient(0).encode()))})
        code, lines, _ = run(FakePftl([entry]), base)
        self.assertEqual((code, alarm_codes(lines)), (2, ["release_amount_mismatch"]))
        alarm = next(line for line in lines if line["kind"] == "release")
        self.assertEqual((alarm["amount_atoms"], alarm["lock_amount_atoms"]), (1, 10**6))

    def test_a_malformed_release_record_is_refused(self) -> None:
        cases = {
            "missing amount": lambda entry: entry["record"].pop("amount_atoms"),
            "string amount": lambda entry: entry["record"].update(amount_atoms="5000"),
            "negative nonce": lambda entry: entry["record"].update(lock_nonce=-1),
            "nonce above u64": lambda entry: entry["record"].update(lock_nonce=1 << 64),
            "recipient not text": lambda entry: entry["record"].update(recipient=7),
            "another lock contract": lambda entry: entry["record"].update(lock_contract_address="0x" + "99" * 20),
            "another source chain": lambda entry: entry["record"].update(source_chain_id=1),
            "source block hash missing": lambda entry: entry["record"].pop("source_block_hash"),
            "source block hash short": lambda entry: entry["record"].update(source_block_hash="0x" + "6f" * 31),
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


# --- The export view (--exports): Base payouts against PFTL export packets. ---

TOKEN = "0x" + "2b" * 20
BASE_TO = "0x" + "4d" * 20


def export_entry(nonce: int, *, amount: int = 400_000, to: str = BASE_TO, height: int = 20) -> dict[str, Any]:
    return {
        "packet": {"lane_id": LANE, "export_nonce": nonce, "sender": recipient(1), "base_recipient": to,
                   "amount_atoms": amount, "exported_at_height": height, "tx_id": f"{nonce:02x}" * 48},
        "packet_digest": "0x" + f"{nonce + 0x10:02x}" * 48,
        "withdrawal_id_commitment": "0x" + f"{nonce + 0x20:02x}" * 48,
        "burn_tx_commitment": "0x" + f"{nonce + 0x30:02x}" * 48,
    }


def released_log(entry: dict[str, Any], *, amount: int | None = None, to: str | None = None,
                 withdrawal: str | None = None, block: int = 950) -> dict[str, Any]:
    packet = entry["packet"]
    amount = packet["amount_atoms"] if amount is None else amount
    to = packet["base_recipient"] if to is None else to
    return {
        "address": LOCK, "blockNumber": hex(block), "transactionHash": "0x" + "ee" * 32,
        "topics": [watcher.RELEASED_TOPIC,
                   withdrawal or watcher.commitment_of(entry["withdrawal_id_commitment"]),
                   watcher.commitment_of(entry["burn_tx_commitment"]),
                   watcher.export_evm_digest(entry["packet_digest"])],
        "data": "0x" + to.removeprefix("0x").rjust(64, "0") + f"{amount:064x}",
    }


class ExportPftl:
    def __init__(self, exports: list[dict[str, Any]], *, pruned: int = 0, exported: int | None = None,
                 released: int = 1_000_000) -> None:
        self.exports, self.pruned, self.released = exports, pruned, released
        self.exported = sum(entry["packet"]["amount_atoms"] for entry in exports) if exported is None else exported

    def external_lane_info(self, lane_id: str | None = None, *, export_cursor: int | None = None,
                           export_limit: int | None = None, **_: Any) -> dict[str, Any]:
        page = [entry for entry in self.exports
                if export_cursor is None or entry["packet"]["export_nonce"] < export_cursor]
        page.sort(key=lambda entry: entry["packet"]["export_nonce"], reverse=True)
        page = page[: export_limit or 100]
        more = page and page[-1]["packet"]["export_nonce"] > self.pruned
        return {"lanes": [{"lane_id": LANE, "exports": page, "pruned_export_count": self.pruned,
                           "next_export_cursor": page[-1]["packet"]["export_nonce"] if more else None,
                           "state": {"exported_atoms": self.exported, "released_atoms": self.released}}]}


class ExportBase:
    def __init__(self, logs: list[dict[str, Any]], *, locked: int = 1_000_000, released: int | None = None,
                 balance: int | None = None) -> None:
        self.logs = logs
        self.locked = locked
        paid = sum(int(log["data"][-64:], 16) for log in logs)
        self.released = paid if released is None else released
        self.balance = self.locked - self.released if balance is None else balance

    def call(self, method: str, params: list[Any]) -> Any:
        if method == "eth_getLogs":
            return self.logs
        if method == "eth_call":
            data = params[0]["data"]
            words = {watcher._selector("token()"): int(TOKEN, 16), watcher._selector("totalLocked()"): self.locked,
                     watcher._selector("totalReleased()"): self.released}
            if data.startswith(watcher._selector("balanceOf(address)")):
                return "0x" + f"{self.balance:064x}"
            return "0x" + f"{words[data]:064x}"
        raise AssertionError(method)


def run_export_view(pftl: ExportPftl, base: ExportBase, *, warning: int | None = None,
                    current_height: int = 500) -> tuple[list[dict[str, Any]], list[str], dict[str, Any]]:
    lines: list[dict[str, Any]] = []
    codes: list[str] = []

    def alarm(code: str, **fields: Any) -> None:
        codes.append(code)
        lines.append({"kind": "alarm", "code": code, **fields})

    summary = watcher.export_view(pftl, base, LANE, lock=LOCK, finalized=FINALIZED, current_height=current_height,
                                  from_block=0, unpaid_warning_blocks=warning, alarm=alarm, lines=lines)
    return lines, codes, summary


class ExportViewTests(unittest.TestCase):
    def test_a_paid_export(self):
        entry = export_entry(0)
        lines, codes, summary = run_export_view(ExportPftl([entry]), ExportBase([released_log(entry)]))
        self.assertEqual(codes, [])
        exports = [line for line in lines if line["kind"] == "export"]
        self.assertEqual([(line["status"], line["base_block"]) for line in exports], [("paid", 950)])
        self.assertEqual(summary["totals"]["paid_sum"], 400_000)

    def test_unpaid_below_and_above_the_threshold(self):
        entry = export_entry(0, height=490)
        lines, codes, _ = run_export_view(ExportPftl([entry]), ExportBase([]), warning=20)
        self.assertEqual(codes, [])
        self.assertEqual([line["status"] for line in lines if line["kind"] == "export"], ["unpaid"])
        self.assertFalse([line for line in lines if line["kind"] == "warning"])
        lines, codes, _ = run_export_view(ExportPftl([entry]), ExportBase([]), warning=5)
        self.assertEqual(codes, [])
        self.assertEqual([line["code"] for line in lines if line["kind"] == "warning"], ["export_unpaid_too_long"])

    def test_a_payout_with_no_packet(self):
        stranger = export_entry(7)
        lines, codes, _ = run_export_view(ExportPftl([]), ExportBase([released_log(stranger)]))
        self.assertIn("payout_without_export", codes)
        self.assertIn("exported_below_paid", codes)

    def test_a_payout_with_another_amount_or_recipient_or_withdrawal(self):
        entry = export_entry(0)
        _, codes, _ = run_export_view(ExportPftl([entry]), ExportBase([released_log(entry, amount=400_001)]))
        self.assertIn("payout_amount_mismatch", codes)
        _, codes, _ = run_export_view(ExportPftl([entry]), ExportBase([released_log(entry, to="0x" + "5e" * 20)]))
        self.assertEqual(codes, ["payout_recipient_mismatch"])
        _, codes, _ = run_export_view(ExportPftl([entry]),
                                      ExportBase([released_log(entry, withdrawal="0x" + "99" * 32)]))
        self.assertEqual(codes, ["payout_withdrawal_mismatch"])

    def test_broken_totals(self):
        entry = export_entry(0)
        _, codes, _ = run_export_view(ExportPftl([entry]), ExportBase([released_log(entry)], released=500_000))
        self.assertIn("total_released_mismatch", codes)
        _, codes, _ = run_export_view(ExportPftl([entry]), ExportBase([released_log(entry)], balance=1))
        self.assertEqual(codes, ["lock_balance_mismatch"])
        _, codes, _ = run_export_view(ExportPftl([entry], released=2_000_000), ExportBase([released_log(entry)]))
        self.assertEqual(codes, ["released_above_locked"])

    def test_a_pruned_packet_is_unknown_not_backed(self):
        pruned_entry = export_entry(0)
        lines, codes, summary = run_export_view(ExportPftl([], pruned=1, exported=400_000),
                                                ExportBase([released_log(pruned_entry)]))
        self.assertEqual(codes, [])
        self.assertEqual([line["status"] for line in lines if line["kind"] == "payout"], ["unknown_pruned"])
        self.assertEqual([line["code"] for line in lines if line["kind"] == "warning"], ["payout_unknown_pruned"])
        self.assertEqual(summary["pruned_export_count"], 1)
