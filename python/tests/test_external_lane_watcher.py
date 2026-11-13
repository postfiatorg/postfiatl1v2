"""The external lane watcher (design section 5.2) against mocked PFTL and Base RPCs."""

from __future__ import annotations

import contextlib
import io
import itertools
import json
import re
import tempfile
import unittest
import unittest.mock
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


def chain_block_hash(chain_id: int, number: int) -> str:
    """The fake hash of Base block `number` on chain `chain_id`: distinct per chain."""

    return "0x" + f"{chain_id:016x}" + f"{number:048x}"


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
            "source_block_hash": chain_block_hash(8453, source_block),
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
        self.fork = ""  # a fork of the same genesis: its blocks hash differently

    def block_header(self, height: int) -> dict[str, Any]:
        tag = f"{self.pftl_chain_id}/{self.genesis_hash}/{self.fork}/{height}"
        return {"height": height, "block_hash": keccak_hex(b"block/" + tag.encode()),
                "state_root": keccak_hex(b"root/" + tag.encode()), "certificate_id": keccak_hex(b"cert/" + tag.encode())}

    def status(self) -> dict[str, Any]:
        header = self.block_header(self.current_height)
        return {"chain_id": self.pftl_chain_id, "genesis_hash": self.genesis_hash,
                "block_height": self.current_height, "block_tip_hash": header["block_hash"],
                "state_root": header["state_root"]}

    def blocks(self, *, from_height: int | None = None, limit: int | None = None) -> list[dict[str, Any]]:
        if from_height is None or from_height > self.current_height:
            return []
        return [{"header": self.block_header(from_height), "receipt_ids": []}]

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
    """A Base JSON-RPC that serves only reads, and records the block of every
    state read. Each chain has its own block hashes; a state read may name
    its block by number or EIP-1898 `{"blockHash", "requireCanonical"}`, and
    a hash the node does not know, or knows only as a side block, is an RPC
    error (as Anvil and geth answer). `side_blocks` are blocks the node keeps
    from before a reorg: served by hash, never canonical by number or for
    state (geth's BlockByHash)."""

    def __init__(self, locks: dict[int, tuple[str, int, str]], *, chain_id: int = 8453,
                 code: bytes = LOCK_CODE, finalized: int = FINALIZED,
                 block_hashes: dict[int, str] | None = None, lock_from: dict[int, int] | None = None) -> None:
        self.locks = locks
        self.chain_id = chain_id
        self.code = code
        self.finalized = finalized
        # The canonical hash of each Base block (default: chain_block_hash),
        # and the first block at which each Lock exists (default: block 0).
        self.block_hashes = block_hashes or {}
        self.lock_from = lock_from or {}
        self.side_blocks: dict[str, int] = {}  # hash -> number, known by hash only
        self.state_read_tags: list[str] = []
        self.lock_read_tags: list[str] = []
        self.block_reads: list[str] = []
        self.hash_reads: list[str] = []
        self.unanchored_state_reads: list[Any] = []
        self.methods: list[str] = []

    def canonical(self, number: int) -> str:
        return self.block_hashes.get(number, chain_block_hash(self.chain_id, number))

    def number_of(self, block_hash: str) -> int | None:
        """The number of a canonical block this node knows, else None."""

        for number, known in self.block_hashes.items():
            if known == block_hash and number <= self.finalized:
                return number
        if block_hash[2:18] != f"{self.chain_id:016x}":
            return None
        number = int(block_hash[18:], 16)
        return number if number <= self.finalized and number not in self.block_hashes else None

    def block(self, number: int, block_hash: str | None = None) -> dict[str, Any]:
        return {"number": hex(number), "hash": block_hash or self.canonical(number),
                "parentHash": self.canonical(number - 1) if number else "0x" + "00" * 32}

    def state_block(self, method: str, tag: Any) -> int:
        """The block number a state read names, by number or by hash."""

        if isinstance(tag, dict):
            assert set(tag) == {"blockHash", "requireCanonical"} and tag["requireCanonical"] is True, tag
            number = self.number_of(tag["blockHash"])
            if number is None:
                raise watcher.pft_token.PftTokenError(f"{method}: RPC error: Resource not found")
            return number
        self.unanchored_state_reads.append((method, tag))
        return int(tag, 16)

    def call(self, method: str, params: list[Any]) -> Any:
        self.methods.append(method)
        if method == "eth_chainId":
            return hex(self.chain_id)
        if method == "eth_getBlockByNumber":
            tag, full = params
            assert full is False, params
            self.block_reads.append(tag)
            if tag == "finalized":
                return self.block(self.finalized)
            number = int(tag, 16)
            assert number <= self.finalized, "only finalized blocks are read by number"
            return self.block(number)
        if method == "eth_getBlockByHash":
            block_hash, full = params
            assert full is False, params
            self.hash_reads.append(block_hash)
            number = self.number_of(block_hash)
            if number is None and block_hash in self.side_blocks:
                return self.block(self.side_blocks[block_hash], block_hash)
            return None if number is None else self.block(number)
        if method == "eth_getCode":
            assert params[0].lower() == LOCK
            self.state_read_tags.append(hex(self.state_block(method, params[1])))
            return "0x" + self.code.hex()
        if method == "eth_call":
            call, tag = params
            number = self.state_block(method, tag)
            self.state_read_tags.append(hex(number))
            self.lock_read_tags.append(hex(number))
            assert call["to"].lower() == LOCK
            data = call["data"]
            assert data[:10] == "0x" + watcher.LOCK_RECORD_SELECTOR, data
            nonce = int(data[10:], 16)
            absent = ("0x" + "00" * 20, 0, "0x" + "00" * 32)
            exists = nonce in self.locks and number >= self.lock_from.get(nonce, 0)
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
        # The anchor, each source block by number, the anchor again at the end.
        self.assertEqual(base.block_reads, ["finalized"] + [hex(900)] * 3 + [hex(FINALIZED)])
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
                         (chain_block_hash(8453, 900), "0x" + "77" * 32))
        # The mismatched block's Lock is not read.
        self.assertEqual(base.lock_read_tags, [hex(800)])

    def test_a_not_yet_final_release_is_left_for_a_later_run(self) -> None:
        pending = release(0, source_block=FINALIZED + 1)
        base = FakeBase({})
        code, lines, _ = run(FakePftl([pending]), base)
        self.assertEqual((code, alarm_codes(lines)), (2, ["release_block_not_finalized"]))
        self.assertEqual((base.block_reads, base.lock_read_tags), (["finalized", hex(FINALIZED)], []))

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
                 withdrawal: str | None = None, burn: str | None = None, block: int = 950,
                 address: str = LOCK, transaction: str = "0x" + "ee" * 32, log_index: int = 0) -> dict[str, Any]:
    packet = entry["packet"]
    amount = packet["amount_atoms"] if amount is None else amount
    to = packet["base_recipient"] if to is None else to
    return {
        "address": address, "blockNumber": hex(block), "transactionHash": transaction, "logIndex": hex(log_index),
        "topics": [watcher.RELEASED_TOPIC,
                   withdrawal or watcher.commitment_of(entry["withdrawal_id_commitment"]),
                   burn or watcher.commitment_of(entry["burn_tx_commitment"]),
                   watcher.export_evm_digest(entry["packet_digest"])],
        "data": "0x" + to.removeprefix("0x").rjust(64, "0") + f"{amount:064x}",
    }


class ExportPftl(FakePftl):
    """`external_lane_info` paging exports as the node does: newest first, the
    cursor the page's oldest nonce, absent on the last stored page, and the
    lane state (`released_atoms`; `exported_atoms` and the pruned count and
    atoms, left out while zero) on every page."""

    def __init__(self, exports: list[dict[str, Any]], *, pruned: int = 0, pruned_atoms: int | None = None,
                 exported: int | None = None, released: int = 1_000_000, current_height: int = 500) -> None:
        super().__init__([], current_height=current_height)
        self.exports, self.pruned, self.released = exports, pruned, released
        self.pruned_atoms = 400_000 * pruned if pruned_atoms is None else pruned_atoms
        stored = sum(entry["packet"]["amount_atoms"] for entry in exports)
        self.exported = self.pruned_atoms + stored if exported is None else exported
        self.export_cursors: list[int | None] = []
        self.page_hook: Any = None

    def external_lane_info(self, lane_id: str | None = None, *, release_cursor: int | None = None,
                           release_limit: int | None = None, export_cursor: int | None = None,
                           export_limit: int | None = None) -> dict[str, Any]:
        saved, self.pruned = self.pruned, 0  # the release paging has no pruned releases
        info = super().external_lane_info(lane_id, release_cursor=release_cursor, release_limit=release_limit)
        self.pruned = saved
        self.export_cursors.append(export_cursor)
        count = self.pruned + len(self.exports)
        end = count if export_cursor is None else export_cursor
        start = max(self.pruned, end - (export_limit or 100))
        page = [entry for entry in self.exports if start <= entry["packet"]["export_nonce"] < end]
        page.sort(key=lambda entry: entry["packet"]["export_nonce"], reverse=True)
        state = {"released_atoms": self.released}
        for key, value in (("exported_atoms", self.exported), ("pruned_export_count", self.pruned),
                           ("pruned_exported_atoms", self.pruned_atoms)):
            if value:
                state[key] = value
        row = info["lanes"][0]
        row.update(state=state, export_count=count, pruned_export_count=self.pruned,
                   export_history_root="0x" + "cd" * 48 if count else None, exports=page)
        if start > self.pruned:
            row["next_export_cursor"] = start
        if self.page_hook is not None:
            self.page_hook(info, export_cursor)
        return info


class ExportBase(FakeBase):
    """A Base node that honours the log filter's block range and each state
    read's block tag: `totalReleased` at block N is the sum of the payouts up
    to N, and `latest` is no block number."""

    def __init__(self, logs: list[dict[str, Any]], *, locked: int = 1_000_000, released: int | None = None,
                 balance: int | None = None, finalized: int = FINALIZED, max_span: int | None = None) -> None:
        super().__init__({}, finalized=finalized)
        self.logs, self.locked, self.released, self.balance = logs, locked, released, balance
        self.max_span = max_span
        self.log_ranges: list[tuple[int, int]] = []
        self.total_tags: list[str] = []

    def paid_through(self, block: int) -> int:
        return sum(int(log["data"][-64:], 16) for log in self.logs
                   if log["address"].lower() == LOCK and int(log["blockNumber"], 16) <= block)

    def call(self, method: str, params: list[Any]) -> Any:
        if method == "eth_getLogs":
            (query,) = params
            assert query["address"].lower() == LOCK and query["topics"] == [watcher.RELEASED_TOPIC], query
            start, end = int(query["fromBlock"], 16), int(query["toBlock"], 16)
            assert start <= end <= self.finalized, "Released events are read in finalized blocks only"
            self.log_ranges.append((start, end))
            if self.max_span is not None and end - start + 1 > self.max_span:
                raise watcher.pft_token.PftTokenRefused("eth_getLogs: the answer is larger than 1048576 bytes")
            # A node that ignores the address filter: the watcher must check it.
            # Each log carries its block's hash on this node's chain.
            return [{"blockHash": self.canonical(int(log["blockNumber"], 16)), "removed": False, **log}
                    for log in self.logs if start <= int(log["blockNumber"], 16) <= end]
        if method == "eth_call" and not params[0]["data"].startswith("0x" + watcher.LOCK_RECORD_SELECTOR):
            call, tag = params
            block = self.state_block(method, tag)
            self.total_tags.append(hex(block))
            released = self.paid_through(block) if self.released is None else self.released
            balance = self.locked - released if self.balance is None else self.balance
            words = {watcher._selector("token()"): int(TOKEN, 16), watcher._selector("totalLocked()"): self.locked,
                     watcher._selector("totalReleased()"): released}
            if call["data"].startswith(watcher._selector("balanceOf(address)")):
                assert call["to"].lower() == TOKEN
                return "0x" + f"{balance:064x}"
            return "0x" + f"{words[call['data']]:064x}"
        return super().call(method, params)


def run_export_view(pftl: ExportPftl, base: ExportBase, *, warning: int | None = None,
                    current_height: int = 500, state: dict[str, Any] | None = None, released: int | None = None,
                    from_block: int = 0, accept: bool = False) -> tuple[list[dict[str, Any]], list[str], dict[str, Any], dict[str, Any]]:
    lines: list[dict[str, Any]] = []
    codes: list[str] = []

    def alarm(code: str, **fields: Any) -> None:
        assert code in watcher.ALARM_CODES, code
        codes.append(code)
        lines.append({"kind": "alarm", "code": code, **fields})

    identity = {"lane_id": LANE, "pftl_chain_id": pftl.pftl_chain_id, "genesis_hash": pftl.genesis_hash,
                "source_chain_id": pftl.chain_id, "lock": LOCK, "code_hash": pftl.code_hash}
    summary, kept = watcher.export_view(
        pftl, base, LANE, identity=identity, lock=LOCK, finalized=base.finalized, current_height=current_height,
        released_atoms=pftl.released if released is None else released, from_block=from_block, state=state,
        accept_baseline=accept, unpaid_warning_blocks=warning, alarm=alarm, lines=lines)
    return lines, codes, summary, kept


def run_exports(pftl: ExportPftl, base: ExportBase, *extra: str) -> tuple[int, list[dict[str, Any]], str]:
    return run(pftl, base, "--exports", "--lock-from-block", "0", *extra)


class ExportViewTests(unittest.TestCase):
    def test_a_paid_export(self):
        entry = export_entry(0)
        lines, codes, summary, kept = run_export_view(ExportPftl([entry]), ExportBase([released_log(entry)]))
        self.assertEqual(codes, [])
        exports = [line for line in lines if line["kind"] == "export"]
        self.assertEqual([(line["status"], line["base_block"]) for line in exports], [("paid", 950)])
        self.assertEqual(summary["totals"]["paid_sum"], 400_000)
        self.assertEqual((kept["scanned_through_block"], kept["payout_count"], kept["payout_sum"]),
                         (FINALIZED, 1, 400_000))

    def test_unpaid_below_and_above_the_threshold(self):
        entry = export_entry(0, height=490)
        lines, codes, _, _ = run_export_view(ExportPftl([entry]), ExportBase([]), warning=20)
        self.assertEqual(codes, [])
        self.assertEqual([line["status"] for line in lines if line["kind"] == "export"], ["unpaid"])
        self.assertFalse([line for line in lines if line["kind"] == "warning"])
        lines, codes, _, _ = run_export_view(ExportPftl([entry]), ExportBase([]), warning=5)
        self.assertEqual(codes, [])
        self.assertEqual([line["code"] for line in lines if line["kind"] == "warning"], ["export_unpaid_too_long"])

    def test_a_payout_with_no_packet(self):
        stranger = export_entry(7)
        _, codes, _, _ = run_export_view(ExportPftl([]), ExportBase([released_log(stranger)]))
        # With nothing pruned, the pruned bound adds nothing to payout_without_export.
        self.assertEqual(codes, ["payout_without_export", "exported_below_paid"])

    def test_a_payout_with_another_amount_recipient_withdrawal_or_burn(self):
        entry = export_entry(0)
        _, codes, _, _ = run_export_view(ExportPftl([entry]), ExportBase([released_log(entry, amount=400_001)]))
        self.assertIn("payout_amount_mismatch", codes)
        for field, value, code in (("to", "0x" + "5e" * 20, "payout_recipient_mismatch"),
                                   ("withdrawal", "0x" + "99" * 32, "payout_withdrawal_mismatch"),
                                   ("burn", "0x" + "98" * 32, "payout_burn_mismatch")):
            with self.subTest(code):
                _, codes, _, _ = run_export_view(ExportPftl([entry]),
                                                 ExportBase([released_log(entry, **{field: value})]))
                self.assertEqual(codes, [code])

    def test_a_second_payout_with_the_same_digest_is_an_alarm(self):
        entry = export_entry(0, amount=200_000)
        logs = [released_log(entry, block=900), released_log(entry, block=950)]
        _, codes, _, _ = run_export_view(ExportPftl([entry]), ExportBase(logs))
        self.assertEqual(codes, ["payout_duplicate", "exported_below_paid"])

    def test_broken_totals(self):
        entry = export_entry(0)
        _, codes, _, _ = run_export_view(ExportPftl([entry]), ExportBase([released_log(entry)], released=500_000))
        self.assertIn("total_released_mismatch", codes)
        _, codes, _, _ = run_export_view(ExportPftl([entry]), ExportBase([released_log(entry)], balance=1))
        self.assertEqual(codes, ["lock_balance_mismatch"])
        _, codes, _, _ = run_export_view(ExportPftl([entry], released=2_000_000), ExportBase([released_log(entry)]))
        self.assertEqual(codes, ["released_above_locked"])

    def test_released_above_locked_on_base_is_an_alarm_and_no_balance_matches_it(self):
        # PFTLockV1 never lets totalReleased pass totalLocked; if it ever did, the
        # expected balance would be negative and must not silence the balance check.
        entry = export_entry(0)  # 400,000 paid against 300,000 locked
        for balance in (0, 5, 1_000_000):
            with self.subTest(balance=balance):
                _, codes, summary, _ = run_export_view(
                    ExportPftl([entry], released=300_000),
                    ExportBase([released_log(entry)], locked=300_000, balance=balance))
                self.assertEqual(codes, ["base_released_above_locked", "lock_balance_mismatch"])
                self.assertEqual(summary["totals"]["lock_surplus"], 0)

    def test_a_balance_above_the_locked_remainder_is_a_surplus_not_an_alarm(self):
        # Tokens sent to the lock by a plain transfer: PFTLockV1 promises only "at least".
        entry = export_entry(0)
        _, codes, summary, _ = run_export_view(ExportPftl([entry]),
                                               ExportBase([released_log(entry)], balance=600_001))
        self.assertEqual(codes, [])
        self.assertEqual(summary["totals"]["lock_surplus"], 1)

    def test_a_pruned_packet_paid_before_any_check_is_an_alarm(self):
        pruned_entry = export_entry(0)
        lines, codes, summary, _ = run_export_view(ExportPftl([], pruned=1),
                                                   ExportBase([released_log(pruned_entry)]))
        self.assertEqual(codes, ["payout_pruned_before_check"])
        self.assertEqual([line["status"] for line in lines if line["kind"] == "payout"], ["alarm"])
        self.assertEqual(summary["pruned_export_count"], 1)

    def test_unknown_payouts_above_the_pruned_count_or_atoms_are_an_alarm(self):
        first, second = export_entry(0), export_entry(1)
        logs = [released_log(first), released_log(second, block=960)]
        _, codes, _, _ = run_export_view(ExportPftl([], pruned=1, pruned_atoms=800_000), ExportBase(logs))
        self.assertIn("payout_unknown_above_pruned", codes)
        _, codes, _, _ = run_export_view(ExportPftl([], pruned=2, pruned_atoms=500_000), ExportBase(logs))
        self.assertIn("payout_unknown_above_pruned", codes)

    def test_verified_payouts_are_not_read_again_and_a_new_unknown_one_is_an_alarm(self):
        first, second = export_entry(0), export_entry(1)
        pftl = ExportPftl([first, second])
        base = ExportBase([released_log(first, block=900)], finalized=920)
        _, codes, _, kept = run_export_view(pftl, base)
        self.assertEqual(codes, [])
        # The first packet is pruned; the second is paid in a later block.
        pftl = ExportPftl([second], pruned=1)
        base = ExportBase([released_log(first, block=900), released_log(second, block=950)])
        lines, codes, _, kept = run_export_view(pftl, base, state=kept)
        self.assertEqual(codes, [])
        self.assertEqual(base.log_ranges, [(921, FINALIZED)])
        self.assertEqual([line["status"] for line in lines if line["kind"] == "payout"], ["backed"])
        self.assertEqual((kept["payout_count"], kept["payout_sum"]), (2, 800_000))
        # A payout for a packet pruned before this watcher saw it paid.
        forged = export_entry(5)
        base = ExportBase([released_log(first, block=900), released_log(second, block=950),
                           released_log(forged, block=1_100)], finalized=1_200, locked=1_400_000)
        _, codes, _, _ = run_export_view(ExportPftl([second], pruned=1, pruned_atoms=800_000), base, state=kept)
        self.assertIn("payout_pruned_before_check", codes)

    def test_logs_are_read_in_chunks_halved_on_an_rpc_error(self):
        entry = export_entry(0)
        base = ExportBase([released_log(entry)], max_span=300)
        _, codes, _, kept = run_export_view(ExportPftl([entry]), base)
        self.assertEqual(codes, [])
        spans = [end - start + 1 for start, end in base.log_ranges]
        self.assertTrue(all(span <= 300 for start, end in base.log_ranges[-3:] for span in [end - start + 1]))
        self.assertEqual(base.log_ranges[-1][1], FINALIZED)
        self.assertGreater(len(spans), 2)
        # The floor: a node that fails every range fails the view.
        with self.assertRaises(watcher.WatcherRpcError):
            run_export_view(ExportPftl([entry]), ExportBase([released_log(entry)], max_span=0))

    def test_only_finalized_blocks_are_read(self):
        entry, late = export_entry(0), export_entry(1)
        # A payout above the finalized block is not counted, in the events or the totals.
        base = ExportBase([released_log(entry), released_log(late, block=FINALIZED + 5)])
        _, codes, summary, _ = run_export_view(ExportPftl([entry, late]), base)
        self.assertEqual(codes, [])
        self.assertEqual(summary["totals"]["total_released"], 400_000)
        self.assertEqual(max(end for _, end in base.log_ranges), FINALIZED)
        self.assertEqual(set(base.total_tags), {hex(FINALIZED)})

    def test_a_log_from_another_address_or_topic_is_malformed(self):
        entry = export_entry(0)
        stray = released_log(entry, address="0x" + "77" * 20)
        with self.assertRaises(watcher.WatcherError) as caught:
            run_export_view(ExportPftl([entry]), ExportBase([stray]))
        self.assertEqual(caught.exception.code, "base_answer_malformed")

    def test_upper_case_hex_in_topics_matches(self):
        entry = export_entry(0)
        log = released_log(entry)
        log["topics"] = ["0x" + topic[2:].upper() for topic in log["topics"]]
        log["address"] = LOCK.upper().replace("0X", "0x")
        _, codes, _, _ = run_export_view(ExportPftl([entry]), ExportBase([log]))
        self.assertEqual(codes, [])

    def test_exports_are_read_across_pages(self):
        entries = [export_entry(nonce, amount=1_000) for nonce in range(3, 3 + 150)]
        pftl = ExportPftl(entries, pruned=3, pruned_atoms=3_000, released=10**9)
        logs = [released_log(entry, block=500 + index) for index, entry in enumerate(entries)]
        lines, codes, summary, _ = run_export_view(pftl, ExportBase(logs, locked=10**9))
        self.assertEqual(codes, [])
        self.assertEqual(summary["exports_checked"], 150)
        self.assertEqual(pftl.export_cursors, [None, None, 53, 53])  # each page read twice

    def test_export_paging_is_checked_like_release_paging(self):
        entries = [export_entry(nonce, amount=1_000) for nonce in range(150)]

        def gap(info, cursor):
            info["lanes"][0]["exports"] = [entry for entry in info["lanes"][0]["exports"]
                                           if entry["packet"]["export_nonce"] != 120]

        def wrong_cursor(info, cursor):
            if cursor is None:
                info["lanes"][0]["next_export_cursor"] = 49

        def repeated_cursor(info, cursor):
            info["lanes"][0]["next_export_cursor"] = 50
            if cursor == 50:
                info["lanes"][0]["exports"] = []

        def changed(info, cursor):
            if cursor is not None:
                info["lanes"][0]["export_count"] += 1

        def missing(info, cursor):
            info["lanes"][0]["state"].pop("released_atoms")

        def string_count(info, cursor):
            info["lanes"][0]["pruned_export_count"] = "0"

        for name, hook in (("gap", gap), ("cursor", wrong_cursor), ("repeated", repeated_cursor),
                           ("snapshot", changed), ("released_atoms", missing), ("count", string_count)):
            with self.subTest(name):
                pftl = ExportPftl(entries, released=10**9)
                pftl.page_hook = hook
                with self.assertRaises(watcher.WatcherError):
                    run_export_view(pftl, ExportBase([], locked=10**9))

    def test_malformed_answers_are_error_lines_not_tracebacks(self):
        entry = export_entry(0)
        bad_digest = export_entry(0)
        bad_digest["packet_digest"] = "0x" + "zz" * 48
        log = released_log(entry)
        log["data"] = "0x" + "zz" * 64
        for name, pftl, base, code in (
            ("packet digest", ExportPftl([bad_digest]), ExportBase([]), "export_record_malformed"),
            ("log data", ExportPftl([entry]), ExportBase([log]), "base_answer_malformed"),
            ("log not an object", ExportPftl([entry]), ExportBase([]), "base_answer_malformed"),
        ):
            with self.subTest(name):
                if name == "log not an object":
                    base.logs = ["0x00"]
                    base.call = (lambda original: lambda method, params: ["0x00"] if method == "eth_getLogs"
                                 else original(method, params))(base.call)
                code_, lines, stderr = run_exports(pftl, base)
                self.assertEqual(code_, 1)
                self.assertIn(code, [json.loads(line)["code"] for line in stderr.splitlines()])
                self.assertEqual(lines[-1]["kind"], "summary")

    def test_an_export_view_failure_does_not_hide_a_release_alarm(self):
        entry = export_entry(0)
        pftl = ExportPftl([entry])
        pftl.releases = [release(0)]
        pftl.history_root = "0x" + "ab" * 48
        base = ExportBase([released_log(entry)], max_span=0)
        code, lines, stderr = run_exports(pftl, base)
        self.assertEqual(code, 2)
        self.assertEqual(alarm_codes(lines), ["release_without_lock"])
        self.assertEqual(json.loads(stderr)["code"], "base_rpc_failed")
        self.assertEqual(lines[-1]["export_error"], "base_rpc_failed")

    def test_the_command_line_wires_the_export_options(self):
        entry = export_entry(0, height=100)
        stranger = export_entry(9)
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            code, lines, _ = run_exports(ExportPftl([entry]), ExportBase([released_log(entry)]),
                                         "--state-file", str(state_file))
            self.assertEqual(code, 0)
            kept = json.loads(state_file.read_text())
            self.assertEqual(kept["exports"]["scanned_through_block"], FINALIZED)
            self.assertEqual(kept["exports"]["payout_count"], 1)
            code, lines, _ = run_exports(ExportPftl([entry]), ExportBase([released_log(stranger)]),
                                         "--unpaid-warning-blocks", "10")
            self.assertEqual(code, 2)
            self.assertIn("payout_without_export", alarm_codes(lines))
            self.assertIn("export_unpaid_too_long", [line["code"] for line in lines if line["kind"] == "warning"])
        base = ExportBase([released_log(entry, block=950)])
        code, _, _ = run(ExportPftl([entry]), base, "--exports", "--lock-from-block", "951")
        # The totals disagree, so the scan is read again (and agrees).
        self.assertEqual(base.log_ranges, [(951, FINALIZED)] * 2)
        self.assertEqual(code, 2)  # totalReleased counts the payout below the scan

    def test_a_lagging_finalized_answer_and_a_new_base_are_refused_not_hidden(self):
        entry = export_entry(0)
        _, _, _, kept = run_export_view(ExportPftl([entry]), ExportBase([released_log(entry)]))
        with self.assertRaises(watcher.WatcherError):
            run_export_view(ExportPftl([entry]), ExportBase([released_log(entry)], finalized=900), state=kept)


class ExportBaselineTests(unittest.TestCase):
    """--accept-export-baseline: the one-shot way past payouts whose packets
    were pruned before the export view ever ran."""

    def run_baseline(self, pftl: ExportPftl, base: ExportBase, state_file: Path, *extra: str):
        return run_exports(pftl, base, "--state-file", str(state_file), *extra)

    def late_lane(self, *, pruned_atoms: int = 400_000):
        old, live = export_entry(0), export_entry(1, amount=100_000)
        pftl = ExportPftl([live], pruned=1, pruned_atoms=pruned_atoms)
        base = ExportBase([released_log(old, block=900), released_log(live, block=950)])
        return pftl, base

    def test_a_baseline_inside_the_pruned_bounds_is_saved_and_reported_ever_after(self):
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            pftl, base = self.late_lane()
            code, lines, _ = self.run_baseline(pftl, base, state_file)
            self.assertEqual((code, alarm_codes(lines)), (2, ["payout_pruned_before_check"]))
            self.assertNotIn("exports", json.loads(state_file.read_text()), "only the release part advances")
            code, lines, _ = self.run_baseline(pftl, base, state_file, "--accept-export-baseline")
            self.assertEqual((code, alarm_codes(lines)), (watcher.BASELINE_EXIT, []))
            accepted = [line for line in lines if line["kind"] == "baseline_payout"]
            self.assertEqual([(line["block"], line["amount"]) for line in accepted], [(900, 400_000)])
            self.assertEqual(set(accepted[0]), {"schema", "kind", "lane_id", "block", "transaction", "amount",
                                                "recipient", "packet_digest"})
            kept = json.loads(state_file.read_text())["exports"]
            self.assertEqual((kept["scanned_through_block"], kept["baseline"]["unverified_payouts"],
                              kept["baseline"]["unverified_atoms"], kept["baseline"]["baseline_accepted"]),
                             (FINALIZED, 1, 400_000, True))
            # A later run reads only new blocks, is clean and repeats the baseline.
            code, lines, _ = self.run_baseline(pftl, base, state_file)
            self.assertEqual((code, alarm_codes(lines)), (0, []))
            self.assertEqual(lines[-1]["exports"]["baseline"], kept["baseline"])
            self.assertEqual(lines[-1]["exports"]["scanned_from_block"], FINALIZED + 1)
            self.assertEqual(json.loads(state_file.read_text())["exports"]["baseline"], kept["baseline"])

    def test_a_payout_above_the_pruned_bounds_stays_an_alarm_and_saves_nothing(self):
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            pftl, base = self.late_lane(pruned_atoms=399_999)
            pftl.exported = 399_999 + 100_000
            code, lines, _ = self.run_baseline(pftl, base, state_file, "--accept-export-baseline")
            self.assertEqual(code, 2)
            self.assertIn("payout_unknown_above_pruned", alarm_codes(lines))
            self.assertNotIn("exports", json.loads(state_file.read_text()))

    def test_a_payout_with_nothing_pruned_is_never_accepted(self):
        with tempfile.TemporaryDirectory() as directory:
            stranger = export_entry(7)
            code, lines, _ = self.run_baseline(ExportPftl([]), ExportBase([released_log(stranger)]),
                                               Path(directory) / "state.json", "--accept-export-baseline")
            self.assertEqual(code, 2)
            self.assertIn("payout_without_export", alarm_codes(lines))
            self.assertNotIn("exports", json.loads((Path(directory) / "state.json").read_text()))

    def test_the_flag_needs_exports_and_a_one_shot_run(self):
        base_argv = ["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc", "http://127.0.0.1:8545"]
        for extra in (["--accept-export-baseline"], ["--exports", "--accept-export-baseline", "--interval", "5"]):
            with self.subTest(extra=extra):
                with contextlib.redirect_stderr(io.StringIO()), contextlib.redirect_stdout(io.StringIO()):
                    with self.assertRaises(SystemExit) as caught:
                        watcher.main(base_argv + extra, pftl=ExportPftl([]), base=ExportBase([]))
                self.assertEqual(caught.exception.code, 2)


class PaidRecordTests(unittest.TestCase):
    """The state's record of paid exports: the reviewer's sequences B4-B6,
    its pruning, an old state file and a changed --lock-from-block."""

    def test_b4_a_payout_of_an_export_paid_in_an_earlier_run_is_a_duplicate(self):
        a, b = export_entry(0), export_entry(1, amount=600_000)
        pftl = ExportPftl([a, b])
        first = released_log(a, block=950)
        _, codes, _, kept = run_export_view(pftl, ExportBase([first]))
        self.assertEqual((codes, kept["scanned_through_block"], kept["payout_sum"]), ([], 1_000, 400_000))
        again = released_log(a, block=1_001, transaction="0x" + "ef" * 32)
        base = ExportBase([first, again], finalized=1_100)
        _, codes, summary, _ = run_export_view(pftl, base, state=kept)
        self.assertEqual(base.log_ranges, [(1_001, 1_100)])
        totals = summary["totals"]
        self.assertEqual((totals["total_released"], totals["lock_balance"], totals["exported_atoms"],
                          totals["released_atoms"]), (800_000, 200_000, 1_000_000, 1_000_000))
        self.assertEqual(codes, ["payout_duplicate"])

    def test_b5_a_paid_export_stays_paid_on_a_later_run(self):
        a, b = export_entry(0, height=100), export_entry(1, amount=600_000, height=100)
        pftl = ExportPftl([a, b])
        log = released_log(a, block=950, log_index=3)
        with tempfile.TemporaryDirectory() as directory:
            options = ("--state-file", str(Path(directory) / "state.json"), "--unpaid-warning-blocks", "50")
            code, _, _ = run_exports(pftl, ExportBase([log]), *options)
            self.assertEqual(code, 0)
            code, lines, _ = run_exports(pftl, ExportBase([log], finalized=1_100), *options)
        self.assertEqual(code, 0)
        self.assertEqual(lines[-1]["exports"]["scanned_from_block"], 1_001)
        exports = {line["export_nonce"]: line for line in lines if line["kind"] == "export"}
        self.assertEqual([(nonce, line["status"]) for nonce, line in exports.items()], [(0, "paid"), (1, "unpaid")])
        self.assertEqual((exports[0]["status"], exports[0]["base_block"], exports[0]["base_transaction"],
                          exports[0]["base_log_index"]), ("paid", 950, "0x" + "ee" * 32, 3))
        self.assertEqual(exports[1]["status"], "unpaid")
        self.assertEqual([line["export_nonce"] for line in lines if line["kind"] == "warning"], [1])

    def test_b6_a_baseline_is_taken_once_and_pruned_capacity_is_not_used_twice(self):
        old, live = export_entry(0), export_entry(1, amount=600_000)
        pftl = ExportPftl([live], pruned=1, pruned_atoms=400_000)
        first = released_log(old, block=900)
        second = released_log(export_entry(5), block=1_050, transaction="0x" + "ef" * 32)
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            options = ("--state-file", str(state_file))
            code, lines, _ = run_exports(pftl, ExportBase([first]), *options, "--accept-export-baseline")
            self.assertEqual((code, alarm_codes(lines)), (watcher.BASELINE_EXIT, []))
            baseline = json.loads(state_file.read_text())["exports"]["baseline"]
            self.assertEqual((baseline["unverified_payouts"], baseline["unverified_atoms"]), (1, 400_000))
            base = ExportBase([first, second], finalized=1_100)
            code, lines, stderr = run_exports(pftl, base, *options, "--accept-export-baseline")
            self.assertEqual(code, 1)
            self.assertEqual(lines[-1]["export_error"], "export_baseline_already_taken")
            self.assertEqual(json.loads(state_file.read_text())["exports"]["baseline"], baseline)
            code, lines, _ = run_exports(pftl, base, *options)
            self.assertEqual((code, sorted(alarm_codes(lines))),
                             (2, ["payout_pruned_before_check", "payout_unknown_above_pruned"]))
            self.assertEqual(json.loads(state_file.read_text())["exports"]["baseline"], baseline)
            # Once the lane has pruned more, the cumulative baseline may grow within it.
            code, lines, _ = run_exports(ExportPftl([], pruned=2, pruned_atoms=1_000_000), base, *options,
                                         "--accept-export-baseline")
            self.assertEqual((code, alarm_codes(lines)), (watcher.BASELINE_EXIT, []))
            grown = json.loads(state_file.read_text())["exports"]["baseline"]
            self.assertEqual((grown["unverified_payouts"], grown["unverified_atoms"], grown["pruned_export_count"],
                              grown["pruned_exported_atoms"]), (2, 800_000, 2, 1_000_000))

    def test_a_paid_export_whose_packet_is_pruned_stays_accounted(self):
        a, b = export_entry(0), export_entry(1, amount=600_000)
        log = released_log(a, block=950)
        _, codes, _, kept = run_export_view(ExportPftl([a, b]), ExportBase([log]))
        pftl = ExportPftl([b], pruned=1, pruned_atoms=400_000)
        _, codes, summary, kept = run_export_view(pftl, ExportBase([log], finalized=1_100), state=kept)
        self.assertEqual(codes, [])
        self.assertEqual((kept["paid"], kept["pruned_paid_count"], kept["pruned_paid_atoms"]), ([], 1, 400_000))
        # A new unknown payout of the same amount still alarms, and is not accepted as a baseline.
        base = ExportBase([log, released_log(export_entry(5), block=1_150)], finalized=1_200)
        for accept in (False, True):
            with self.subTest(accept=accept):
                lines, codes, _, _ = run_export_view(pftl, base, state=kept, accept=accept)
                self.assertEqual(sorted(codes), ["payout_pruned_before_check", "payout_unknown_above_pruned"])

    def test_an_old_state_file_rescans_once_and_rebuilds_the_record(self):
        a = export_entry(0)
        old = {"lock_from_block": 0, "scanned_through_block": 1_000, "payout_count": 1, "payout_sum": 400_000,
               "verified_payouts_root": "0x" + "ab" * 32}
        base = ExportBase([released_log(a, block=950)], finalized=1_100)
        _, codes, summary, kept = run_export_view(ExportPftl([a]), base, state=old)
        self.assertEqual(codes, [])
        self.assertEqual((base.log_ranges, summary["rescan"]), ([(0, 1_100)], "export_state_format_old"))
        self.assertEqual([(entry["export_nonce"], entry["block"]) for entry in kept["paid"]], [(0, 950)])
        self.assertNotIn("verified_payouts_root", kept)
        # The rescan never takes an old payout whose packet is pruned as verified.
        _, codes, _, _ = run_export_view(ExportPftl([], pruned=1), ExportBase([released_log(a)], finalized=1_100),
                                         state=old)
        self.assertEqual(codes, ["payout_pruned_before_check"])

    def test_a_changed_lock_from_block_rescans_and_rebuilds_the_record(self):
        a, b = export_entry(0), export_entry(1, amount=600_000)
        _, _, _, kept = run_export_view(ExportPftl([a, b]), ExportBase([released_log(a, block=950)]))
        base = ExportBase([released_log(a, block=950), released_log(b, block=1_050, log_index=1)], finalized=1_100)
        _, codes, summary, new = run_export_view(ExportPftl([a, b]), base, state=kept, from_block=900)
        self.assertEqual(codes, [])
        self.assertEqual((base.log_ranges, summary["rescan"]), ([(900, 1_100)], "lock_from_block_changed"))
        self.assertEqual([(entry["export_nonce"], entry["block"]) for entry in new["paid"]], [(0, 950), (1, 1_050)])
        self.assertEqual(new["lock_from_block"], 900)

    def test_the_round_trip_rehearsals_watcher_sequence_with_a_state_file(self):
        # scripts/rehearse-lane-roundtrip.py's export-view runs, resumed from a
        # state file: after export 1, the forged payout on a snapshot, after
        # export 2 (the snapshot reverted) and at the end.
        a, b = export_entry(0), export_entry(1, amount=600_000)
        paid_a = released_log(a, block=135, transaction="0x" + "a1" * 32)
        paid_b = released_log(b, block=264, transaction="0x" + "b2" * 32)
        forged = released_log(export_entry(9), block=200, transaction="0x" + "f0" * 32)
        with tempfile.TemporaryDirectory() as directory:
            options = ("--state-file", str(Path(directory) / "state.json"), "--unpaid-warning-blocks", "64")

            def watch(exports, logs, finalized):
                return run_exports(ExportPftl(exports), ExportBase(logs, finalized=finalized), *options)

            def paid(lines):
                return [(line["export_nonce"], line["status"], line.get("base_block"))
                        for line in lines if line["kind"] == "export"]

            code, lines, _ = watch([a], [paid_a], 150)
            self.assertEqual((code, alarm_codes(lines), paid(lines)), (0, [], [(0, "paid", 135)]))
            code, lines, _ = watch([a], [paid_a, forged], 250)
            self.assertEqual((code, sorted(alarm_codes(lines))), (2, ["exported_below_paid", "payout_without_export"]))
            code, lines, _ = watch([a, b], [paid_a, paid_b], 300)
            self.assertEqual((code, alarm_codes(lines), paid(lines)), (0, [], [(0, "paid", 135), (1, "paid", 264)]))
            self.assertEqual(lines[-1]["exports"]["scanned_from_block"], 151)
            code, lines, _ = watch([a, b], [paid_a, paid_b], 400)
            self.assertEqual((code, alarm_codes(lines), paid(lines)), (0, [], [(0, "paid", 135), (1, "paid", 264)]))
            self.assertEqual((lines[-1]["exports"]["scanned_from_block"], lines[-1]["exports"]["payouts_checked"]),
                             (301, 0))


class ExportSnapshotTests(unittest.TestCase):
    """F1: an export snapshot older than one the view acted on is refused."""

    # Window 100: A nonce 0 at PFTL 400, B nonce 1 at 450, C nonce 2 at 600.
    A = export_entry(0, height=400)
    B = export_entry(1, amount=600_000, height=450)
    C = export_entry(2, amount=500_000, height=600)
    FIRST = released_log(A, block=950, transaction="0x" + "a1" * 32)
    SECOND = released_log(A, block=1_150, transaction="0x" + "a2" * 32)

    def old(self, height):
        return ExportPftl([self.A, self.B], released=1_500_000, current_height=height)

    def new(self, height):
        return ExportPftl([self.C], pruned=2, pruned_atoms=1_000_000, released=1_500_000, current_height=height)

    def check(self, state_file, pftl, logs, finalized, *extra):
        return run_exports(pftl, ExportBase(logs, locked=1_500_000, finalized=finalized),
                           "--state-file", str(state_file), *extra)

    def test_f1_a_one_block_failover_to_an_older_export_snapshot_is_refused(self):
        test = self

        class AcrossOneBlock:
            # The first lane page at 599, the export pages at 600.
            def external_lane_info(self, *args, **kwargs):
                pftl = test.new(600) if "export_limit" in kwargs else test.old(599)
                return pftl.external_lane_info(*args, **kwargs)

            def status(self):
                return test.new(600).status()

            def blocks(self, **kwargs):
                return test.new(600).blocks(**kwargs)

        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            code, lines, _ = self.check(state_file, self.old(500), [self.FIRST], 1_000)
            self.assertEqual((code, alarm_codes(lines)), (0, []))
            code, lines, _ = self.check(state_file, AcrossOneBlock(), [self.FIRST], 1_100)
            self.assertEqual((code, alarm_codes(lines)), (0, []))
            saved = json.loads(state_file.read_text())
            exports = saved["exports"]
            self.assertEqual((saved["checked_through_height"], exports["snapshot"]["height"]), (599, 600))
            self.assertEqual((exports["pruned_paid_count"], exports["pruned_paid_atoms"], exports["paid"]),
                             (1, 400_000, []))
            self.assertEqual(exports["snapshot"], {"height": 600, "export_count": 3, "exported_atoms": 1_500_000,
                                                   "pruned_export_count": 2, "pruned_exported_atoms": 1_000_000})
            # The honest node one block behind: A and B retained, nothing pruned.
            code, lines, stderr = self.check(state_file, self.old(599), [self.FIRST, self.SECOND], 1_200)
            self.assertEqual((code, alarm_codes(lines)), (1, []))
            self.assertEqual(lines[-1]["export_error"], "export_snapshot_behind_state")
            self.assertIn("export_snapshot_behind_state", stderr)
            self.assertEqual([line for line in lines if line["kind"] in ("payout", "export")], [])
            self.assertEqual(json.loads(state_file.read_text())["exports"], exports)
            # A current node again: A's second payout alarms and A is not folded twice.
            code, lines, _ = self.check(state_file, self.new(601), [self.FIRST, self.SECOND], 1_300)
            self.assertEqual((code, alarm_codes(lines)), (2, ["payout_duplicate"]))
            summary = lines[-1]["exports"]
            self.assertEqual((summary["pruned_paid_count"], summary["pruned_paid_atoms"]), (1, 400_000))
            self.assertEqual(json.loads(state_file.read_text())["exports"], exports)

    def test_each_counter_below_its_high_water_mark_is_refused(self):
        a, b = export_entry(0, height=400), export_entry(1, amount=600_000, height=450)
        _, _, _, kept = run_export_view(ExportPftl([b], pruned=1, current_height=600), ExportBase([]),
                                        current_height=600)
        lagging = {"height": ExportPftl([b], pruned=1, current_height=599),
                   "export_count": ExportPftl([], pruned=1, current_height=600),
                   "exported_atoms": ExportPftl([], pruned=1, current_height=600),
                   "pruned_export_count": ExportPftl([a, b], current_height=600),
                   "pruned_exported_atoms": ExportPftl([b], pruned=1, pruned_atoms=300_000,
                                                       exported=900_000, current_height=600)}
        for field, pftl in lagging.items():
            with self.subTest(field=field), self.assertRaises(watcher.WatcherError) as caught:
                run_export_view(pftl, ExportBase([]), state=kept, current_height=600)
            self.assertEqual(caught.exception.code, "export_snapshot_behind_state")
            self.assertIn(field, str(caught.exception))

    def test_negative_room_with_a_zero_pruned_count_is_refused(self):
        a = export_entry(0)
        # A current state whose paid export was folded, on a node that reports nothing pruned.
        _, _, _, kept = run_export_view(ExportPftl([a]), ExportBase([released_log(a, block=950)]))
        kept = dict(kept, paid=[], pruned_paid_count=1, pruned_paid_atoms=400_000,
                    pruned_paid_recent=[], snapshot=dict(kept["snapshot"], height=0, export_count=0,
                                                         exported_atoms=0))
        base = ExportBase([released_log(a, block=950), released_log(a, block=1_050, transaction="0x" + "a2" * 32)],
                          finalized=1_100)
        with self.assertRaises(watcher.WatcherError) as caught:
            run_export_view(ExportPftl([a]), base, state=kept)
        self.assertEqual(caught.exception.code, "export_snapshot_behind_state")
        self.assertEqual(base.log_ranges, [])
        # The same entry of the earlier form (no format number) is not used: both payouts are seen.
        legacy = {key: value for key, value in kept.items() if key not in ("format", "snapshot", "pruned_paid_recent")}
        _, codes, summary, _ = run_export_view(ExportPftl([a]), base, state=legacy)
        self.assertEqual((codes, summary["rescan"]),
                         (["payout_duplicate", "exported_below_paid"], "export_state_format_old"))

    def test_a_nonce_is_folded_once(self):
        a, b = export_entry(0), export_entry(1, amount=600_000)
        log = released_log(a, block=950)
        _, _, _, kept = run_export_view(ExportPftl([a, b]), ExportBase([log]))
        for height, finalized in ((501, 1_100), (502, 1_200), (503, 1_300)):
            _, codes, _, kept = run_export_view(ExportPftl([b], pruned=1, current_height=height),
                                                ExportBase([log], finalized=finalized), state=kept,
                                                current_height=height)
            self.assertEqual((codes, kept["pruned_paid_count"], kept["pruned_paid_atoms"]), ([], 1, 400_000))
        # A record that holds a nonce below its own pruned high-water mark is refused, not folded again.
        forged = dict(kept, paid=[{"export_nonce": 0, "evm_digest": watcher.export_evm_digest(a["packet_digest"]),
                                   "block": 950, "transaction": "0x" + "ee" * 32, "log_index": 0,
                                   "amount": 400_000, "withdrawal_id_commitment":
                                   watcher.commitment_of(a["withdrawal_id_commitment"])}])
        with self.assertRaises(watcher.WatcherError) as caught:
            run_export_view(ExportPftl([b], pruned=1, current_height=504), ExportBase([log], finalized=1_400),
                            state=forged, current_height=504)
        self.assertEqual(caught.exception.code, "state_file_invalid")


class RecentPrunedPaidTests(unittest.TestCase):
    """F3: a payout of a recently pruned paid export is a duplicate in every run."""

    def test_f3_a_duplicate_of_a_pruned_paid_export_is_never_accepted_as_a_baseline(self):
        a, b, c = export_entry(0), export_entry(1, amount=600_000), export_entry(2, amount=500_000)
        first = released_log(a, block=950, transaction="0x" + "a1" * 32)
        again = released_log(a, block=1_150, transaction="0x" + "a2" * 32)
        pruned = ExportPftl([c], pruned=2, pruned_atoms=1_000_000, released=1_500_000)
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            options = ("--state-file", str(state_file))

            def check(pftl, logs, finalized, *extra):
                return run_exports(pftl, ExportBase(logs, locked=1_500_000, finalized=finalized), *options, *extra)

            self.assertEqual(check(ExportPftl([a, b, c], released=1_500_000), [first], 1_000)[0], 0)
            self.assertEqual(check(pruned, [first], 1_100)[0], 0)
            kept = json.loads(state_file.read_text())["exports"]
            self.assertEqual((kept["paid"], kept["pruned_paid_count"],
                              [entry["evm_digest"] for entry in kept["pruned_paid_recent"]]),
                             ([], 1, [watcher.export_evm_digest(a["packet_digest"])]))
            for extra in ((), ("--accept-export-baseline",)):
                with self.subTest(extra=extra):
                    code, lines, _ = check(pruned, [first, again], 1_200, *extra)
                    self.assertEqual((code, alarm_codes(lines)), (2, ["payout_duplicate"]))
                    self.assertEqual([line for line in lines if line["kind"] == "baseline_payout"], [])
                    self.assertEqual(json.loads(state_file.read_text())["exports"], kept)

    def test_the_recent_list_is_bounded_and_an_older_duplicate_still_alarms(self):
        entries = [export_entry(nonce, amount=100_000) for nonce in range(4)]
        logs = [released_log(entry, block=900 + nonce, log_index=nonce) for nonce, entry in enumerate(entries)]
        digests = [watcher.export_evm_digest(entry["packet_digest"]) for entry in entries]
        with unittest.mock.patch.object(watcher, "RECENT_PRUNED_PAID", 2):
            _, _, _, kept = run_export_view(ExportPftl(entries, released=400_000), ExportBase(logs, locked=400_000))
            pftl = ExportPftl([entries[3]], pruned=3, pruned_atoms=300_000, released=400_000)
            _, codes, _, kept = run_export_view(pftl, ExportBase(logs, locked=400_000, finalized=1_100), state=kept)
            self.assertEqual((codes, kept["pruned_paid_count"],
                              [entry["evm_digest"] for entry in kept["pruned_paid_recent"]]), ([], 3, digests[1:3]))
            again = [released_log(entries[n], block=1_150, transaction="0x" + f"{n:02x}" * 32) for n in (0, 2)]
            # A second payout also exceeds the exports (exported_below_paid); the older one also exceeds
            # the pruned room left (every pruned packet was paid and verified).
            for paid, expected in ((again[1], ["payout_duplicate", "exported_below_paid"]),
                                   (again[0], ["payout_pruned_before_check", "payout_unknown_above_pruned",
                                               "exported_below_paid"])):
                with self.subTest(expected=expected):
                    _, codes, _, _ = run_export_view(
                        pftl, ExportBase([*logs, paid], locked=500_000, finalized=1_200), state=kept)
                    self.assertEqual(codes, expected)


class StateSizeTests(unittest.TestCase):
    """F2: a state above MAX_STATE_BYTES is never written over the last good one."""

    def state(self, size: int) -> dict[str, Any]:
        state = {"schema": watcher.STATE_SCHEMA, "lane_id": LANE, "pftl_chain_id": "postfiat-test",
                 "genesis_hash": GENESIS_HASH, "lock_contract_address": LOCK, "source_chain_id": 8453,
                 "checked_through_height": 1, "release_count": 0, "pad": ""}
        state["pad"] = "x" * (size - len(json.dumps(state, sort_keys=True) + "\n"))
        return state

    def test_the_exact_boundary_is_written_and_one_byte_more_is_refused(self):
        limit = watcher.MAX_STATE_BYTES
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "state.json"
            watcher._save_state(path, self.state(limit))
            self.assertEqual(path.stat().st_size, limit)
            self.assertEqual(len(watcher._load_state(path)["pad"]), len(self.state(limit)["pad"]))
            good = path.read_bytes()
            with self.assertRaises(watcher.WatcherError) as caught:
                watcher._save_state(path, self.state(limit + 1))
            self.assertEqual(caught.exception.code, "state_file_too_large")
            self.assertEqual(path.read_bytes(), good)
            self.assertEqual(sorted(p.name for p in Path(directory).iterdir()), ["state.json"])

    def test_a_run_that_would_cross_the_limit_fails_and_keeps_the_last_good_file(self):
        a, b = export_entry(0), export_entry(1, amount=600_000)
        pftl = ExportPftl([a, b])
        first, second = released_log(a, block=950), released_log(b, block=1_050, log_index=1)
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            options = ("--state-file", str(state_file))
            self.assertEqual(run_exports(pftl, ExportBase([first]), *options)[0], 0)
            good = state_file.read_bytes()
            # The limit at the good file's size: one more paid record crosses it.
            with unittest.mock.patch.object(watcher, "MAX_STATE_BYTES", len(good)):
                self.assertEqual(run_exports(pftl, ExportBase([first], finalized=1_001), *options)[0], 0)
                good = state_file.read_bytes()
                code, lines, stderr = run_exports(pftl, ExportBase([first, second], finalized=1_100), *options)
                self.assertEqual(code, 1)
                self.assertIn("state_file_too_large", stderr)
                self.assertEqual([line for line in lines if line["kind"] == "summary"], [])
                self.assertEqual(state_file.read_bytes(), good)
                # The last good file still loads and the next run starts where it did.
                code, lines, stderr = run_exports(pftl, ExportBase([first, second], finalized=1_100), *options)
                self.assertNotIn("state_file_invalid", stderr)
                self.assertEqual((code, json.loads(good)["exports"]["scanned_through_block"]), (1, 1_001))
            code, lines, _ = run_exports(pftl, ExportBase([first, second], finalized=1_100), *options)
            self.assertEqual((code, lines[-1]["exports"]["scanned_from_block"], lines[-1]["exports"]["paid_recorded"]),
                             (0, 1_002, 2))


class StateLifeTests(unittest.TestCase):
    """R1: an exports entry of an earlier format is never used. R2: a rescan
    never forgets a payment it knew."""

    # Window 100: A nonce 0 at PFTL 400, B nonce 1 at 450, C nonce 2 at 600, D nonce 3 at 701.
    A = export_entry(0, height=400)
    B = export_entry(1, amount=600_000, height=450)
    C = export_entry(2, amount=500_000, height=600)
    D = export_entry(3, amount=500_000, height=701)

    def lane(self, height, retained, pruned, pruned_atoms):
        return ExportPftl(retained, pruned=pruned, pruned_atoms=pruned_atoms, released=2_000_000,
                          current_height=height)

    def check(self, state_file, pftl, logs, finalized, *extra):
        return run_exports(pftl, ExportBase(logs, locked=2_000_000, finalized=finalized),
                           "--state-file", str(state_file), *extra)

    def test_r1_an_earlier_format_entry_is_never_used_and_both_payouts_are_seen(self):
        first = released_log(self.C, block=950, transaction="0x" + "c1" * 32)
        second = released_log(self.C, block=1_150, transaction="0x" + "c2" * 32)
        # The previous commit's file after the reviewer's run 2: C folded, `paid=[]`, no snapshot.
        legacy = {"lock_from_block": 0, "scanned_through_block": 1_100, "payout_count": 1, "payout_sum": 500_000,
                  "paid": [], "pruned_paid_count": 1, "pruned_paid_atoms": 500_000}
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            state_file.write_text(json.dumps({
                "schema": watcher.STATE_SCHEMA, "lane_id": LANE, "pftl_chain_id": "postfiat-local",
                "genesis_hash": GENESIS_HASH, "lock_contract_address": LOCK, "source_chain_id": 8453,
                "checked_through_height": 700, "release_count": 0, "exports": legacy}))
            # Run 3: an honest node at 700, C retained again.
            code, lines, _ = self.check(state_file, self.lane(700, [self.C], 2, 1_000_000), [first, second], 1_200)
            self.assertEqual((code, alarm_codes(lines)), (2, ["payout_duplicate"]))
            self.assertEqual(lines[-1]["exports"]["rescan"], "export_state_format_old")
            self.assertEqual(lines[-1]["exports"]["scanned_from_block"], 0)
            self.assertNotIn("backed", [line["status"] for line in lines
                                        if line["kind"] == "payout" and line["block"] == 1_150])
            self.assertEqual(json.loads(state_file.read_text())["exports"], legacy)
            # Run 4: height 702, D retained: still never used, still the duplicate.
            code, lines, _ = self.check(state_file, self.lane(702, [self.D], 3, 1_500_000), [first, second], 1_300)
            self.assertEqual(code, 2)
            self.assertIn("payout_duplicate", alarm_codes(lines))
            self.assertEqual(json.loads(state_file.read_text())["exports"], legacy)

    def test_a_first_run_from_an_earlier_format_saves_only_what_it_read(self):
        first = released_log(self.C, block=950, transaction="0x" + "c1" * 32)
        old = {"lock_from_block": 0, "scanned_through_block": 1_100, "payout_count": 1, "payout_sum": 500_000,
               "paid": [], "pruned_paid_count": 1, "pruned_paid_atoms": 500_000,
               "snapshot": {"height": 701, "export_count": 4, "exported_atoms": 2_000_000,
                            "pruned_export_count": 3, "pruned_exported_atoms": 1_500_000}}
        pftl = self.lane(700, [self.C], 2, 1_000_000)
        _, codes, summary, kept = run_export_view(pftl, ExportBase([first], locked=2_000_000, finalized=1_200),
                                                  state=old, current_height=700)
        self.assertEqual((codes, summary["rescan"]), ([], "export_state_format_old"))
        self.assertEqual(kept["format"], watcher.EXPORT_STATE_FORMAT)
        self.assertEqual((kept["pruned_paid_count"], [entry["export_nonce"] for entry in kept["paid"]]), (0, [2]))
        self.assertEqual(kept["snapshot"], {"height": 700, "export_count": 3, "exported_atoms": 1_500_000,
                                            "pruned_export_count": 2, "pruned_exported_atoms": 1_000_000})

    def r2_lane(self):
        a, b, c = export_entry(0), export_entry(1, amount=600_000), export_entry(2, amount=500_000)
        first = released_log(a, block=950, transaction="0x" + "a1" * 32)
        again = released_log(a, block=1_250, transaction="0x" + "a2" * 32)
        return a, b, c, first, again

    def test_r2_a_changed_scan_start_keeps_what_it_knew(self):
        a, b, c, first, again = self.r2_lane()
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"

            def check(pftl, logs, finalized, *extra):
                return run(pftl, ExportBase(logs, locked=1_500_000, finalized=finalized), "--exports",
                           "--state-file", str(state_file), *extra)

            pruned = ExportPftl([c], pruned=2, pruned_atoms=1_000_000, released=1_500_000)
            self.assertEqual(check(ExportPftl([a, b, c], released=1_500_000), [first], 1_000,
                                   "--lock-from-block", "0")[0], 0)
            self.assertEqual(check(pruned, [first], 1_100, "--lock-from-block", "0")[0], 0)
            # The rescan from block 1 sees A's original payout: verified earlier, not unknown.
            code, lines, _ = check(pruned, [first], 1_200, "--lock-from-block", "1")
            self.assertEqual((code, alarm_codes(lines)), (0, []))
            self.assertEqual(lines[-1]["exports"]["rescan"], "lock_from_block_changed")
            self.assertEqual([line["status"] for line in lines if line["kind"] == "payout"], ["verified_earlier"])
            kept = json.loads(state_file.read_text())["exports"]
            self.assertEqual((kept["pruned_paid_count"], kept["pruned_paid_atoms"]), (1, 400_000))
            self.assertEqual([(entry["evm_digest"], entry["block"]) for entry in kept["pruned_paid_recent"]],
                             [(watcher.export_evm_digest(a["packet_digest"]), 950)])
            # C pruned too (new room): A's second payout is a duplicate, under the baseline step as well.
            later = ExportPftl([export_entry(3, amount=100_000)], pruned=3, pruned_atoms=1_500_000,
                               released=1_500_000)
            for extra in ((), ("--accept-export-baseline",)):
                with self.subTest(extra=extra):
                    code, lines, _ = check(later, [first, again], 1_300, "--lock-from-block", "2", *extra)
                    self.assertEqual(code, 2)
                    self.assertIn("payout_duplicate", alarm_codes(lines))
                    self.assertEqual([line for line in lines if line["kind"] == "baseline_payout"], [])

    def test_r2_from_height_keeps_what_it_knew(self):
        a, b, c, first, again = self.r2_lane()
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"

            def check(pftl, logs, finalized, *extra):
                return run_exports(pftl, ExportBase(logs, locked=1_500_000, finalized=finalized),
                                   "--state-file", str(state_file), *extra)

            pruned = ExportPftl([c], pruned=2, pruned_atoms=1_000_000, released=1_500_000)
            self.assertEqual(check(ExportPftl([a, b, c], released=1_500_000), [first], 1_000)[0], 0)
            self.assertEqual(check(pruned, [first], 1_100)[0], 0)
            for extra in ((), ("--accept-export-baseline",)):
                with self.subTest(extra=extra):
                    code, lines, _ = check(pruned, [first, again], 1_300, "--from-height", "0", *extra)
                    self.assertEqual((code, alarm_codes(lines)), (2, ["payout_duplicate"]))
                    self.assertEqual(lines[-1]["exports"]["rescan"], "from_height")

    def test_discarding_the_history_is_explicit_and_counted(self):
        a, b, c, first, again = self.r2_lane()
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"

            def check(pftl, logs, finalized, *extra):
                return run_exports(pftl, ExportBase(logs, locked=1_500_000, finalized=finalized),
                                   "--state-file", str(state_file), *extra)

            pruned = ExportPftl([c], pruned=2, pruned_atoms=1_000_000, released=1_500_000)
            self.assertEqual(check(ExportPftl([a, b, c], released=1_500_000), [first], 1_000)[0], 0)
            self.assertEqual(check(pruned, [first], 1_100)[0], 0)
            code, lines, _ = check(pruned, [first], 1_200, "--discard-export-history", "--accept-export-baseline")
            self.assertEqual(code, watcher.BASELINE_EXIT)
            (discarded,) = [line for line in lines if line["kind"] == "history_discarded"]
            self.assertEqual((discarded["identities"], discarded["verified_payouts"], discarded["verified_atoms"]),
                             (1, 1, 400_000))
            self.assertEqual(lines[-1]["exports"]["rescan"], "export_history_discarded")
            stderr = io.StringIO()
            with contextlib.redirect_stderr(stderr), self.assertRaises(SystemExit):
                watcher.main(["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc",
                              "http://127.0.0.1:8545", "--exports", "--interval", "5", "--discard-export-history"],
                             pftl=pruned, base=ExportBase([]))
            self.assertIn("--discard-export-history", stderr.getvalue())

    def test_n1_from_height_never_carries_history_into_another_genesis(self):
        c = export_entry(2, amount=500_000)
        paid = released_log(c, block=950, transaction="0x" + "c1" * 32)
        g2 = ExportPftl([export_entry(3, amount=500_000)], pruned=3, pruned_atoms=1_500_000, released=2_000_000)
        g2.genesis_hash = "d0" * 48
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            g1 = ExportPftl([self.A, self.B, c], released=2_000_000)
            self.assertEqual(self.check(state_file, g1, [paid], 1_000)[0], 0)
            before = state_file.read_bytes()
            for extra in ((), ("--from-height", "0"), ("--lock-from-block", "1")):
                with self.subTest(extra=extra):
                    code, lines, stderr = self.check(state_file, g2, [paid], 1_100, *extra)
                    self.assertEqual(code, 1)
                    self.assertIn("state_file_invalid", stderr)
                    self.assertNotIn("verified_earlier", [line.get("status") for line in lines])
                    self.assertEqual(state_file.read_bytes(), before)
            # The control: a fresh file on G2 alarms on C's payout.
            code, lines, _ = self.check(Path(directory) / "fresh.json", g2, [paid], 1_100)
            self.assertEqual((code, alarm_codes(lines)[:1]), (2, ["payout_pruned_before_check"]))

    def test_n1_a_changed_lock_source_chain_or_lane_is_refused_on_every_path(self):
        c = export_entry(2, amount=500_000)
        paid = released_log(c, block=950, transaction="0x" + "c1" * 32)
        lane = ExportPftl([self.A, self.B, c], released=2_000_000)
        cases = {"lock_contract_address": "0x" + "9f" * 20, "source_chain_id": 84532, "lane_id": "other-lane",
                 "genesis_hash": "d0" * 48, "pftl_chain_id": "postfiat-other"}
        for key, value in cases.items():
            for extra in ((), ("--from-height", "0"), ("--lock-from-block", "1"), ("--discard-export-history",)):
                with self.subTest(key=key, extra=extra), tempfile.TemporaryDirectory() as directory:
                    state_file = Path(directory) / "state.json"
                    self.assertEqual(self.check(state_file, lane, [paid], 1_000)[0], 0)
                    state_file.write_text(json.dumps({**json.loads(state_file.read_text()), key: value}))
                    before = state_file.read_bytes()
                    code, lines, stderr = self.check(state_file, lane, [paid], 1_100, *extra)
                    self.assertEqual(code, 1)
                    self.assertIn("state_file_invalid", stderr)
                    self.assertEqual(state_file.read_bytes(), before)

    def test_n1_an_unreadable_file_is_refused_under_from_height(self):
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            state_file.write_text("{not json")
            code, _, stderr = self.check(state_file, ExportPftl([self.A], released=2_000_000), [], 1_000,
                                         "--from-height", "0")
            self.assertEqual(code, 1)
            self.assertIn("state_file_invalid", stderr)
            self.assertEqual(state_file.read_text(), "{not json")

    def test_n2_a_rescan_on_a_lagging_base_node_is_refused_and_the_original_stays_the_original(self):
        paid = released_log(self.A, block=950, transaction="0x" + "a1" * 32)
        lane = ExportPftl([self.A, self.B], released=2_000_000)
        for extra in (("--from-height", "0"), ("--lock-from-block", "1"), ("--discard-export-history",)):
            with self.subTest(extra=extra), tempfile.TemporaryDirectory() as directory:
                state_file = Path(directory) / "state.json"
                self.assertEqual(self.check(state_file, lane, [paid], 1_000)[0], 0)
                before = json.loads(state_file.read_text())["exports"]
                code, lines, stderr = self.check(state_file, lane, [paid], 900, *extra)
                self.assertEqual((code, alarm_codes(lines)), (1, []))
                self.assertIn("base_finalized_behind_state", stderr)
                self.assertEqual(json.loads(state_file.read_text())["exports"], before)
                code, lines, _ = self.check(state_file, lane, [paid], 1_000)
                self.assertEqual((code, alarm_codes(lines)), (0, []))

    def test_n2_a_recorded_event_seen_again_in_a_normal_run_is_the_original(self):
        paid = released_log(self.A, block=950, transaction="0x" + "a1" * 32)
        lane = ExportPftl([self.A, self.B], released=2_000_000)
        _, codes, _, kept = run_export_view(lane, ExportBase([paid], locked=2_000_000, finalized=1_000))
        self.assertEqual(codes, [])
        # A file whose cursor lies before its own recorded payout (as the earlier rescan saved it).
        behind = {**kept, "scanned_through_block": 900, "payout_count": 0, "payout_sum": 0}
        lines, codes, _, again = run_export_view(lane, ExportBase([paid], locked=2_000_000, finalized=1_000),
                                                 state=behind)
        self.assertEqual(codes, [])
        self.assertEqual([line["status"] for line in lines if line["kind"] == "payout"], ["backed"])
        self.assertEqual(again["paid"], kept["paid"])
        # The same with A pruned and in the recent list: verified earlier, not a duplicate, not folded twice.
        pruned = ExportPftl([self.B], pruned=1, pruned_atoms=400_000, released=2_000_000)
        _, codes, _, folded = run_export_view(pruned, ExportBase([paid], locked=2_000_000, finalized=1_000),
                                              state=kept)
        self.assertEqual((codes, folded["pruned_paid_count"]), ([], 1))
        lines, codes, _, again = run_export_view(
            pruned, ExportBase([paid], locked=2_000_000, finalized=1_000),
            state={**folded, "scanned_through_block": 900, "payout_count": 0, "payout_sum": 0})
        self.assertEqual(codes, [])
        self.assertEqual([line["status"] for line in lines if line["kind"] == "payout"], ["verified_earlier"])
        self.assertEqual((again["pruned_paid_count"], again["pruned_paid_atoms"]), (1, 400_000))

    def test_n3_an_incomplete_scan_never_prints_backed_or_paid(self):
        first = released_log(self.A, block=950, transaction="0x" + "a1" * 32)
        second = released_log(self.A, block=1_150, transaction="0x" + "a2" * 32)
        lane = ExportPftl([self.A, self.B], released=2_000_000)
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            self.assertEqual(self.check(state_file, lane, [], 900)[0], 0)
            before = json.loads(state_file.read_text())["exports"]
            for extra in (("--lock-from-block", "1000"), ("--lock-from-block", "1000", "--from-height", "0")):
                with self.subTest(extra=extra):
                    code, lines, _ = self.check(state_file, lane, [first, second], 1_200, *extra)
                    self.assertEqual((code, alarm_codes(lines)), (2, ["total_released_mismatch"]))
                    self.assertEqual([(line["block"], line["status"]) for line in lines if line["kind"] == "payout"],
                                     [(1_150, "matched_incomplete_scan")])
                    self.assertEqual({line["export_nonce"]: line["status"] for line in lines
                                      if line["kind"] == "export"},
                                     {0: "matched_incomplete_scan", 1: "unpaid"})
                    # The export part of the state is unchanged; the clean release view may advance (below).
                    self.assertEqual(json.loads(state_file.read_text())["exports"], before)
            # A complete scan still prints `backed`.
            code, lines, _ = self.check(Path(directory) / "fresh.json", lane, [first], 1_200)
            self.assertEqual((code, [line["status"] for line in lines if line["kind"] == "payout"]), (0, ["backed"]))
            self.assertIn("paid", [line["status"] for line in lines if line["kind"] == "export"])

    def test_n3_an_incomplete_scan_keeps_the_export_part_while_the_release_part_advances(self):
        """The release view is clean, so its height is saved; only the export
        part of the state is unchanged, not the whole file."""

        first = released_log(self.A, block=950, transaction="0x" + "a1" * 32)
        second = released_log(self.A, block=1_150, transaction="0x" + "a2" * 32)
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            self.assertEqual(self.check(state_file, self.lane(500, [self.A, self.B], 0, 0), [], 900)[0], 0)
            before = json.loads(state_file.read_text())
            code, lines, _ = self.check(state_file, self.lane(501, [self.A, self.B], 0, 0), [first, second], 1_200,
                                        "--lock-from-block", "1000")
            self.assertEqual((code, alarm_codes(lines)), (2, ["total_released_mismatch"]))
            self.assertEqual([line["status"] for line in lines if line["kind"] == "payout"],
                             ["matched_incomplete_scan"])
            after = json.loads(state_file.read_text())
            self.assertEqual(after["exports"], before["exports"])
            self.assertEqual((before["checked_through_height"], after["checked_through_height"]), (500, 501))

    def test_the_recent_list_share_of_the_size_limit(self):
        entry = {"export_nonce": U64_MAX, "evm_digest": "0x" + "ab" * 32, "block": U64_MAX,
                 "transaction": "0x" + "cd" * 32, "log_index": U64_MAX, "amount": U64_MAX}
        size = len(json.dumps([entry] * watcher.RECENT_PRUNED_PAID, sort_keys=True))
        self.assertEqual((size, 100 * size // watcher.MAX_STATE_BYTES), (1_257_472, 7))


OTHER_LOCK = "0x" + "9f" * 20
IDENTITY_FIELDS = ("genesis_hash", "pftl_chain_id", "lock", "source_chain_id")


def foreign(node: ExportPftl, field: str) -> ExportPftl:
    """The same lane answers from a node of another network or policy."""

    if field == "genesis_hash":
        node.genesis_hash = "d0" * 48
    elif field == "pftl_chain_id":
        node.pftl_chain_id = "postfiat-other"
    elif field == "source_chain_id":
        node.chain_id = 84532
    else:
        node.page_hook = lambda info, cursor: info["lanes"][0]["config"]["policy"].update(
            lock_contract_address=OTHER_LOCK)
    return node


class Failover:
    """One endpoint: the release read from `release`; export page k
    (0-based) from `export` before `switch`, from `later` from it on."""

    def __init__(self, release: Any, export: Any, later: Any = None, switch: int = 0) -> None:
        self.release, self.export, self.later, self.switch, self.pages = release, export, later, switch, 0

    def external_lane_info(self, *args: Any, **kwargs: Any) -> dict[str, Any]:
        if "export_limit" not in kwargs:
            return self.release.external_lane_info(*args, **kwargs)
        self.pages += 1
        # Each page is read twice: page k is reads 2k and 2k + 1.
        node = self.later if self.later is not None and (self.pages + 1) // 2 > self.switch else self.export
        return node.external_lane_info(*args, **kwargs)

    def status(self) -> dict[str, Any]:
        return self.export.status()

    def blocks(self, **kwargs: Any) -> list[dict[str, Any]]:
        return self.export.blocks(**kwargs)


class OneForkPftlRead:
    """One PFTL endpoint: export-page read number `index` (0-based) is
    answered by `fork`, a node of a fork that shares this chain's genesis and
    lane; every other read (release pages, the tip, blocks) by `home`."""

    def __init__(self, home: ExportPftl, fork: ExportPftl, index: int) -> None:
        self.home, self.fork, self.index, self.reads = home, fork, index, 0

    def external_lane_info(self, *args: Any, **kwargs: Any) -> dict[str, Any]:
        if "export_limit" not in kwargs:
            return self.home.external_lane_info(*args, **kwargs)
        self.reads += 1
        return (self.fork if self.reads - 1 == self.index else self.home).external_lane_info(*args, **kwargs)

    def status(self) -> dict[str, Any]:
        return self.home.status()

    def blocks(self, **kwargs: Any) -> list[dict[str, Any]]:
        return self.home.blocks(**kwargs)


class OnePftlForkRead:
    """One PFTL endpoint behind which read number `index` (0-based) of the
    export view (the tip, a block, each read of an export page) is answered by
    `fork`; release pages and every other read by `home`."""

    def __init__(self, home: Any, fork: ExportPftl, index: int) -> None:
        self.home, self.fork, self.index, self.reads, self.used = home, fork, index, 0, None

    def pick(self, method: str) -> Any:
        node = self.home
        if self.reads == self.index:
            node, self.used = self.fork, method
        self.reads += 1
        return node

    def external_lane_info(self, *args: Any, **kwargs: Any) -> dict[str, Any]:
        if "export_limit" not in kwargs:
            return self.home.external_lane_info(*args, **kwargs)
        return self.pick("external_lane_info").external_lane_info(*args, **kwargs)

    def status(self) -> dict[str, Any]:
        return self.pick("status").status()

    def blocks(self, **kwargs: Any) -> list[dict[str, Any]]:
        return self.pick("blocks").blocks(**kwargs)


def fork_export(nonce: int) -> dict[str, Any]:
    """An export the validated chain never made: its own packet digest."""

    entry = export_entry(nonce, amount=50_000, height=505)
    entry["packet_digest"] = "0x" + "f7" * 48
    return entry


class PftlSnapshotTests(unittest.TestCase):
    """R7-N1: an export page answered by a node of a same-genesis fork, or
    from another block, must not back a payout; a carried payout is joined to
    its packet by digest, not by nonce."""

    E0, E1 = export_entry(0, height=100), export_entry(1, amount=600_000, height=200)
    P0 = released_log(E0, block=950)

    def home(self, height: int = 510) -> ExportPftl:
        return ExportPftl([self.E0, self.E1], current_height=height)

    def fork(self, height: int = 510) -> ExportPftl:
        node = ExportPftl([self.E0, self.E1, fork_export(2)], current_height=height)
        node.fork = "b"
        return node

    def two_runs(self, second: Any, logs: list[dict[str, Any]]) -> tuple[int, list[dict[str, Any]], str, bytes, bytes]:
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            options = ("--state-file", str(state_file))
            code, lines, _ = run_exports(self.home(500), ExportBase([self.P0]), *options)
            self.assertEqual((code, alarm_codes(lines)), (0, []))
            before = state_file.read_bytes()
            code, lines, stderr = run_exports(second, ExportBase(logs, finalized=1_100), *options)
            return code, lines, stderr, before, state_file.read_bytes()

    def assert_refused(self, result: tuple[int, list[dict[str, Any]], str, bytes, bytes], code: str) -> None:
        exit_code, lines, stderr, before, after = result
        self.assertEqual(exit_code, 1)
        self.assertEqual([line for line in lines if line["kind"] in ("payout", "export", "summary")], [])
        self.assertEqual(json.loads(stderr)["code"], code)
        self.assertEqual(after, before)

    def test_the_reviewers_sequence_a_fork_page_backing_an_orphan_payout_is_refused(self):
        orphan = released_log(fork_export(2), block=1_050, transaction="0x" + "ef" * 32)
        for height in (510, 511):  # the fork at the snapshot's height, and one block ahead
            with self.subTest(height=height):
                pftl = OneForkPftlRead(self.home(), self.fork(height), 0)
                self.assert_refused(self.two_runs(pftl, [self.P0, orphan]), "pftl_snapshot_mismatch")

    def test_honest_progress_between_two_runs_passes(self):
        later = ExportPftl([self.E0, self.E1, export_entry(2, amount=50_000, height=505)], current_height=510)
        payout = released_log(export_entry(2, amount=50_000), block=1_050, transaction="0x" + "ef" * 32)
        code, lines, stderr, _, _ = self.two_runs(later, [self.P0, payout])
        self.assertEqual((code, alarm_codes(lines), stderr), (0, [], ""))
        self.assertEqual([line["status"] for line in lines if line["kind"] == "payout"], ["backed"])
        snapshot = lines[-1]["exports"]["pftl_snapshot"]
        self.assertEqual((snapshot["height"], snapshot["block_hash"]), (510, later.block_header(510)["block_hash"]))

    def test_a_page_from_another_block_at_the_same_height_is_refused(self):
        exports = [export_entry(n, amount=1_000, height=100 + n) for n in range(5)]
        fork_exports = [*exports[:4], fork_export(4)]
        fork_exports[4]["packet"]["amount_atoms"] = 1_000
        home, fork = ExportPftl(exports, current_height=510), ExportPftl(fork_exports, current_height=510)
        fork.fork = "b"
        # Its history root chains the divergent export, so every page of it differs.
        fork.page_hook = lambda info, cursor: info["lanes"][0].update(export_history_root="0x" + "ce" * 48)
        with unittest.mock.patch.object(watcher, "PAGE_SIZE", 2):
            for index in range(6):  # either read of each of the three pages
                with self.subTest(read=index):
                    pftl = OneForkPftlRead(home, fork, index)
                    code, lines, stderr = run_exports(pftl, ExportBase([], finalized=1_100))
                    self.assertEqual((code, [line for line in lines if line["kind"] in ("export", "summary")]),
                                     (1, []))
                    self.assertEqual(json.loads(stderr)["code"], "pftl_snapshot_mismatch")
            code, _, stderr = run_exports(home, ExportBase([], finalized=1_100))
            self.assertEqual((code, stderr), (0, ""))

    def test_a_tip_the_block_log_does_not_confirm_or_a_block_that_changes_is_refused(self):
        class Moved(ExportPftl):
            def __init__(self, change: str) -> None:
                super().__init__([PftlSnapshotTests.E0], current_height=510)
                self.change, self.block_reads = change, 0

            def status(self):
                answer = super().status()
                if self.change == "status":
                    answer["state_root"] = "0x" + "00" * 32
                return answer

            def blocks(self, **kwargs):
                self.block_reads += 1
                if self.change == "end" and self.block_reads == 2:
                    self.fork = "b"
                return super().blocks(**kwargs)

        for change in ("status", "end"):
            with self.subTest(change):
                code, lines, stderr = run_exports(Moved(change), ExportBase([self.P0], finalized=1_100))
                self.assertEqual((code, [line for line in lines if line["kind"] in ("payout", "summary")]), (1, []))
                self.assertEqual(json.loads(stderr)["code"], "pftl_snapshot_mismatch")

    def test_a_retained_nonce_naming_another_packet_is_refused(self):
        other = export_entry(0, amount=400_000, height=100)
        other["packet_digest"] = "0x" + "f0" * 48
        node = ExportPftl([other, self.E1], current_height=510)
        self.assert_refused(self.two_runs(node, [self.P0]), "pftl_export_digest_mismatch")

    def test_the_reviewers_sequence_a_fork_release_page_ahead_of_the_snapshot_is_refused(self):
        """R-2: the release page from a same-genesis fork at 600, every other
        read from the home node at 500. The run is refused before any state is
        saved, so the next honest run at 500 is not `state_file_invalid`."""

        class ForkReleasePage(ExportPftl):
            def __init__(self, fork: ExportPftl) -> None:
                super().__init__([PftlSnapshotTests.E0, PftlSnapshotTests.E1], current_height=500)
                self.fork_node = fork

            def external_lane_info(self, *args: Any, **kwargs: Any) -> dict[str, Any]:
                if "export_limit" not in kwargs:
                    return self.fork_node.external_lane_info(*args, **kwargs)
                return super().external_lane_info(*args, **kwargs)

        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            options = ("--state-file", str(state_file))
            code, lines, stderr = run_exports(ForkReleasePage(self.fork(600)), ExportBase([self.P0]), *options)
            self.assertEqual((code, [line for line in lines if line["kind"] in ("payout", "export", "summary")]),
                             (1, []))
            error = json.loads(stderr)
            self.assertEqual(error["code"], "pftl_snapshot_behind_release")
            self.assertIn("pftl_snapshot_behind_release", watcher.PFTL_SNAPSHOT_CODES)
            self.assertFalse(state_file.exists())
            code, lines, stderr = run_exports(self.home(500), ExportBase([self.P0]), *options)
            self.assertEqual((code, alarm_codes(lines), stderr), (0, [], ""))
            self.assertEqual(json.loads(state_file.read_text())["checked_through_height"], 500)
            self.assertEqual([line["age_blocks"] for line in lines if line.get("status") == "unpaid"], [300])


class AdvancingPftl(ExportPftl):
    """An honest node that makes a block right after answering `status` on
    each of its first `advances` attempts, so those attempts' pages report
    the next height."""

    def __init__(self, advances: int) -> None:
        super().__init__([export_entry(0, height=100)], current_height=500)
        self.advances = advances

    def status(self) -> dict[str, Any]:
        answer = super().status()
        if self.advances:
            self.advances -= 1
            self.current_height += 1
        return answer


def run_interval(pftl: ExportPftl, runs: int) -> tuple[int, list[dict[str, Any]], list[dict[str, Any]], list[float]]:
    sleeps: list[float] = []
    stdout, stderr = io.StringIO(), io.StringIO()
    with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
        code = watcher.main(["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc",
                             "http://127.0.0.1:8545", "--exports", "--lock-from-block", "0", "--interval", "30"],
                            pftl=pftl, base=ExportBase([released_log(export_entry(0, height=100))]),
                            sleep=sleeps.append, max_runs=runs)
    return (code, [json.loads(line) for line in stdout.getvalue().splitlines() if line.strip()],
            [json.loads(line) for line in stderr.getvalue().splitlines() if line.strip()], sleeps)


class PftlSnapshotIntervalTests(unittest.TestCase):
    """R-1: snapshot refusals stay fail-closed, but in interval mode the next
    attempt after one is immediate once, and a run of them is a warning."""

    def test_an_advancing_chain_gives_a_no_progress_warning_after_n_refusals(self):
        n = watcher.PFTL_NO_PROGRESS_REFUSALS
        self.assertEqual(n, 5)
        code, out, err, sleeps = run_interval(AdvancingPftl(advances=n), n)
        self.assertEqual(code, 0)
        self.assertEqual([line["code"] for line in err], ["pftl_snapshot_mismatch"] * n)
        self.assertEqual([line for line in out if line["kind"] == "summary"], [])
        # Immediate after a refusal, then the interval: two attempts per interval.
        self.assertEqual(sleeps, [30.0] * ((n - 1) // 2))
        warnings = [line for line in out if line["kind"] == "warning"]
        self.assertEqual(len(warnings), 1)
        self.assertEqual({key: warnings[0][key] for key in ("code", "consecutive_refusals", "last_refusal",
                                                            "release_height", "pftl_snapshot_height", "page_height")},
                         {"code": "pftl_snapshot_no_progress", "consecutive_refusals": n,
                          "last_refusal": "pftl_snapshot_mismatch", "release_height": 504,
                          "pftl_snapshot_height": 504, "page_height": 505})

    def test_a_chain_that_pauses_once_succeeds_on_the_immediate_retry(self):
        code, out, err, sleeps = run_interval(AdvancingPftl(advances=1), 2)
        self.assertEqual((code, sleeps), (0, []))
        self.assertEqual([line["code"] for line in err], ["pftl_snapshot_mismatch"])
        self.assertEqual([line["kind"] for line in out if line["kind"] in ("summary", "warning")], ["summary"])
        self.assertEqual(out[-1]["exports"]["pftl_snapshot"]["height"], 501)


class ExportIdentityTests(unittest.TestCase):
    """F1: the export pages must come from the network, lane and policy the
    release view validated, the first page and every later one, whatever
    the run's start; honest progress between the two reads still passes."""

    STARTS = {"normal": (), "from_height": ("--from-height", "0"), "lock_from_block": ("--lock-from-block", "1"),
              "both": ("--from-height", "0", "--lock-from-block", "1")}
    E = export_entry(0, height=400)
    PAYOUT = released_log(E, block=950)

    def refused(self, code, lines, stderr, state_file, before):
        self.assertEqual(code, 1)
        self.assertIn("export_identity_mismatch", stderr)
        self.assertEqual([line for line in lines if line["kind"] in ("payout", "export")], [])
        self.assertEqual(lines[-1]["export_error"], "export_identity_mismatch")
        after = json.loads(state_file.read_text())
        self.assertEqual(after.get("exports"), (before or {}).get("exports"))
        self.assertEqual(after["genesis_hash"], GENESIS_HASH)

    def seeded(self, directory):
        state_file = Path(directory) / "state.json"
        a = ExportPftl([], released=2_000_000)
        self.assertEqual(run_exports(a, ExportBase([], locked=2_000_000, finalized=900),
                                     "--state-file", str(state_file))[0], 0)
        return state_file, json.loads(state_file.read_text())

    def test_the_reviewers_failover_between_the_two_reads_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            state_file, before = self.seeded(directory)
            a = ExportPftl([], released=2_000_000, current_height=500)
            b = ExportPftl([self.E], released=2_000_000, current_height=501)
            b.genesis_hash = "d0" * 48
            code, lines, stderr = run_exports(Failover(a, b), ExportBase([self.PAYOUT], locked=2_000_000,
                                                                         finalized=1_000),
                                              "--state-file", str(state_file), "--from-height", "0")
            self.refused(code, lines, stderr, state_file, before)

    def test_every_identity_field_on_every_start_is_refused(self):
        for field, (start, extra) in itertools.product(IDENTITY_FIELDS, self.STARTS.items()):
            with self.subTest(field=field, start=start), tempfile.TemporaryDirectory() as directory:
                state_file, before = self.seeded(directory)
                a = ExportPftl([], released=2_000_000, current_height=500)
                b = foreign(ExportPftl([self.E], released=2_000_000, current_height=501), field)
                code, lines, stderr = run_exports(Failover(a, b), ExportBase([self.PAYOUT], locked=2_000_000,
                                                                             finalized=1_000),
                                                  "--state-file", str(state_file), *extra)
                self.refused(code, lines, stderr, state_file, before)

    def test_a_later_export_page_of_another_identity_is_refused(self):
        entries = [export_entry(n, height=400 + n) for n in range(5)]
        for field, switch in itertools.product(IDENTITY_FIELDS, (1, 2)):
            with self.subTest(field=field, page=switch + 1), tempfile.TemporaryDirectory() as directory, \
                    unittest.mock.patch.object(watcher, "PAGE_SIZE", 2):
                state_file, before = self.seeded(directory)
                a = ExportPftl(entries, released=2_000_000)
                pftl = Failover(a, a, foreign(ExportPftl(entries, released=2_000_000), field), switch)
                code, lines, stderr = run_exports(pftl, ExportBase([released_log(entries[0], block=950)],
                                                                   locked=2_000_000, finalized=1_000),
                                                  "--state-file", str(state_file))
                self.assertEqual(pftl.pages, 2 * switch + 1)  # each earlier page read twice
                self.refused(code, lines, stderr, state_file, before)

    def test_a_first_run_without_a_state_file_is_refused_and_saves_no_export_part(self):
        for field in IDENTITY_FIELDS:
            with self.subTest(field=field), tempfile.TemporaryDirectory() as directory:
                state_file = Path(directory) / "state.json"
                a = ExportPftl([], released=2_000_000)
                b = foreign(ExportPftl([self.E], released=2_000_000, current_height=501), field)
                code, lines, stderr = run_exports(Failover(a, b), ExportBase([self.PAYOUT], locked=2_000_000,
                                                                             finalized=1_000),
                                                  "--state-file", str(state_file))
                self.refused(code, lines, stderr, state_file, None)
                self.assertNotIn("exports", json.loads(state_file.read_text()))

    def test_honest_progress_between_the_two_reads_passes(self):
        with tempfile.TemporaryDirectory() as directory:
            state_file, _ = self.seeded(directory)
            a = ExportPftl([], released=1_500_000, current_height=500)
            later = ExportPftl([self.E], released=2_000_000, current_height=501)
            code, lines, stderr = run_exports(Failover(a, later), ExportBase([self.PAYOUT], locked=2_000_000,
                                                                             finalized=1_000),
                                              "--state-file", str(state_file))
            self.assertEqual((code, stderr), (0, ""))
            self.assertEqual([line["status"] for line in lines if line["kind"] in ("payout", "export")],
                             ["backed", "paid"])
            self.assertEqual(len(json.loads(state_file.read_text())["exports"]["paid"]), 1)

    def test_g2_an_export_made_between_the_two_reads_is_printed(self):
        made = export_entry(0, height=501)
        for paid in (True, False):
            with self.subTest(paid=paid), tempfile.TemporaryDirectory() as directory:
                state_file, _ = self.seeded(directory)
                a = ExportPftl([], released=1_500_000, current_height=500)
                later = ExportPftl([made], released=2_000_000, current_height=501)
                logs = [released_log(made, block=950)] if paid else []
                code, lines, stderr = run_exports(Failover(a, later), ExportBase(logs, locked=2_000_000,
                                                                                 finalized=1_000),
                                                  "--state-file", str(state_file))
                self.assertEqual((code, stderr), (0, ""))
                exported = [line for line in lines if line["kind"] == "export"]
                self.assertEqual([(line["export_nonce"], line["status"]) for line in exported],
                                 [(0, "paid" if paid else "unpaid")])
                if not paid:
                    self.assertEqual(exported[0]["age_blocks"], 0)
                self.assertEqual(lines[-1]["exports"]["unpaid_exports"], 0 if paid else 1)


FOREIGN_CHAIN = 84532


class OneForeignRead:
    """One Base endpoint behind which read number `index` (0-based, every
    method counted) is answered by `foreign`, an honest node of another
    network, and every other read by `home`."""

    def __init__(self, home: Any, foreign: Any, index: int | None = None, method: str | None = None,
                 data: str | None = None, match: Any = None) -> None:
        self.home, self.foreign, self.index, self.method, self.data = home, foreign, index, method, data
        self.match, self.reads, self.used = match, 0, None

    def call(self, method: str, params: list[Any]) -> Any:
        if self.index is not None:
            pick = self.reads == self.index
        elif self.match is not None:
            pick = self.used is None and self.match(method, params)
        else:
            pick = (self.used is None and method == self.method
                    and (self.data is None or params[0]["data"] == self.data))
        self.reads += 1
        if pick:
            self.used = method
            return self.foreign.call(method, params)
        return self.home.call(method, params)


def foreign_base(base: FakeBase) -> FakeBase:
    base.chain_id = FOREIGN_CHAIN
    return base


FORK_TAG = 0xF0F0  # the fork's own block hashes, above the height it shares


def fork_of(fork: FakeBase, home: FakeBase, height: int) -> FakeBase:
    """`fork` as an honest node of a network that shares the home chain's
    history (chain id, blocks, state) through `height` and has its own blocks
    above it. Each node keeps the other's blocks above `height` as side
    blocks (seen before a reorg): known by hash, not canonical."""

    fork.chain_id = home.chain_id
    for number in range(height + 1, max(fork.finalized, home.finalized) + 1):
        fork.block_hashes[number] = chain_block_hash(FORK_TAG, number)
        home.side_blocks[chain_block_hash(FORK_TAG, number)] = number
        fork.side_blocks[home.canonical(number)] = number
    return fork


class BaseAnchorTests(unittest.TestCase):
    """G1: every Base answer a run uses is bound to one anchor block (the
    finalized block, by hash) of the chain the run validated. A single read
    answered by an honest node of another network (same lock address, its
    own history) ends in a refusal or in exactly the all-honest result."""

    def seed(self, directory: str, lane: ExportPftl, finalized: int = 900) -> Path:
        state_file = Path(directory) / "state.json"
        self.assertEqual(run_exports(lane, ExportBase([], locked=2_000_000, finalized=finalized),
                                     "--state-file", str(state_file))[0], 0)
        return state_file

    def refused(self, outcome, state_file: Path, before: bytes, code: str = "base_anchor_unknown") -> None:
        status, lines, stderr = outcome
        self.assertEqual(status, 1, stderr)
        self.assertIn(code, stderr)
        self.assertEqual([line for line in lines if line["kind"] in ("payout", "export", "summary")], [])
        self.assertEqual(state_file.read_bytes(), before)

    def test_sequence_a_a_foreign_total_released_never_backs_a_second_payout(self):
        with tempfile.TemporaryDirectory() as directory:
            e = export_entry(0, height=400)
            lane = ExportPftl([e, export_entry(1, amount=600_000, height=450)], released=2_000_000)
            state_file = self.seed(directory, lane)
            before = state_file.read_bytes()
            p1 = released_log(e, block=950, transaction="0x" + "a1" * 32)
            p2 = released_log(e, block=1_150, transaction="0x" + "a2" * 32)
            a = ExportBase([p1, p2], locked=2_000_000, finalized=1_200, balance=1_600_000)
            b = foreign_base(ExportBase([p1], locked=2_000_000, finalized=1_200))
            endpoint = OneForeignRead(a, b, method="eth_call", data=watcher._selector("totalReleased()"))
            outcome = run_exports(lane, endpoint, "--state-file", str(state_file), "--lock-from-block", "1000",
                                  "--from-height", "0")
            self.assertEqual(endpoint.used, "eth_call")
            self.refused(outcome, state_file, before)
            # All from A: the late scan is incomplete, and says so.
            code, lines, _ = run_exports(lane, a, "--state-file", str(state_file), "--lock-from-block", "1000",
                                         "--from-height", "0")
            self.assertEqual(code, 2)
            self.assertIn("total_released_mismatch", alarm_codes(lines))

    def test_sequence_b_a_foreign_log_answer_never_hides_an_orphan(self):
        with tempfile.TemporaryDirectory() as directory:
            e = export_entry(0, height=400)
            lane = ExportPftl([e], released=2_000_000)
            state_file = self.seed(directory, lane)
            before = state_file.read_bytes()
            orphan = released_log(export_entry(9), block=950, transaction="0x" + "0a" * 32)
            b_payout = released_log(e, block=950, transaction="0x" + "0b" * 32)
            a = ExportBase([orphan], locked=2_000_000, finalized=1_000)
            b = foreign_base(ExportBase([b_payout], locked=2_000_000, finalized=1_000))
            endpoint = OneForeignRead(a, b, method="eth_getLogs")
            self.refused(run_exports(lane, endpoint, "--state-file", str(state_file)), state_file, before)
            # The next run, all from A, still sees the orphan.
            code, lines, _ = run_exports(lane, ExportBase([orphan], locked=2_000_000, finalized=1_100),
                                         "--state-file", str(state_file))
            self.assertEqual(code, 2)
            self.assertIn("payout_without_export", alarm_codes(lines))
            self.assertFalse([line for line in lines if line.get("status") in ("backed", "paid", "verified_earlier")])

    def test_sequence_c_a_fork_log_answer_never_hides_an_orphan(self):
        """A fork that shares Base's history through 949 pays export E at
        950; Base pays an orphan of the same amount there and keeps the
        fork's 950 as a side block. One log answer from the fork is refused."""

        with tempfile.TemporaryDirectory() as directory:
            e = export_entry(0, height=400)
            lane = ExportPftl([e], released=2_000_000)
            state_file = self.seed(directory, lane)
            before = state_file.read_bytes()
            orphan = released_log(export_entry(9), block=950, transaction="0x" + "0a" * 32)
            a = ExportBase([orphan], locked=2_000_000, finalized=1_000)
            b = fork_of(ExportBase([released_log(e, block=950, transaction="0x" + "0b" * 32)], locked=2_000_000,
                                   finalized=1_000), a, 949)
            side = chain_block_hash(FORK_TAG, 950)
            # The fixture is honest: A serves the side block by hash, never as canonical state.
            self.assertEqual(a.call("eth_getBlockByHash", [side, False])["number"], hex(950))
            with self.assertRaises(watcher.pft_token.PftTokenError):
                a.call("eth_getCode", [LOCK, watcher.at_block(side)])
            endpoint = OneForeignRead(a, b, method="eth_getLogs")
            self.refused(run_exports(lane, endpoint, "--state-file", str(state_file)), state_file, before,
                         "base_anchor_mismatch")
            self.assertEqual(endpoint.used, "eth_getLogs")
            # The next run, all from A, still sees the orphan.
            code, lines, _ = run_exports(lane, ExportBase([orphan], locked=2_000_000, finalized=1_100),
                                         "--state-file", str(state_file))
            self.assertEqual(code, 2)
            self.assertIn("payout_without_export", alarm_codes(lines))
            self.assertFalse([line for line in lines if line.get("status") in ("backed", "paid", "verified_earlier")])

    def test_sequence_d_a_fork_log_answer_never_hides_a_second_payout(self):
        """Base pays E0 twice (950, 1150); a fork sharing history through 949
        pays E0 and E1 once each at the same blocks, kept at home as side
        blocks. One log answer from the fork is refused."""

        with tempfile.TemporaryDirectory() as directory:
            e0, e1 = export_entry(0, height=400), export_entry(1, height=410)
            lane = ExportPftl([e0, e1], released=2_000_000)
            state_file = self.seed(directory, lane)
            before = state_file.read_bytes()
            home_logs = [released_log(e0, block=950, transaction="0x" + "a1" * 32),
                         released_log(e0, block=1_150, transaction="0x" + "a2" * 32)]
            a = ExportBase(home_logs, locked=2_000_000, finalized=1_200)
            b = fork_of(ExportBase([released_log(e0, block=950, transaction="0x" + "b1" * 32),
                                    released_log(e1, block=1_150, transaction="0x" + "b2" * 32)],
                                   locked=2_000_000, finalized=1_200), a, 949)
            endpoint = OneForeignRead(a, b, method="eth_getLogs")
            self.refused(run_exports(lane, endpoint, "--state-file", str(state_file)), state_file, before,
                         "base_anchor_mismatch")
            # The next run, all from A, sees the duplicate.
            code, lines, _ = run_exports(lane, ExportBase(home_logs, locked=2_000_000, finalized=1_300),
                                         "--state-file", str(state_file))
            self.assertEqual(code, 2)
            self.assertIn("payout_duplicate", alarm_codes(lines))

    def test_a_fork_source_block_answer_never_raises_a_false_release_alarm(self):
        """The release's source block 900 read by number from a fork that
        shares history through 899 (its 900 kept at home as a side block):
        refused, not `release_source_block_mismatch`; so is a fork's answer
        to the second, confirming read of a real mismatch."""

        good = release(0)
        cases = {
            "first read": ({}, 0),
            "confirming read": ({900: "0x" + "77" * 32}, 1),
        }
        for name, (home_hashes, nth) in cases.items():
            with self.subTest(name), tempfile.TemporaryDirectory() as directory:
                state_file = Path(directory) / "state.json"
                a = FakeBase(backed(good), block_hashes=dict(home_hashes))
                b = fork_of(FakeBase(backed(good)), a, 899)
                seen = []

                def nth_read(method, params, nth=nth):
                    if method == "eth_getBlockByNumber" and params[0] == hex(900):
                        seen.append(params)
                        return len(seen) == nth + 1
                    return False

                endpoint = OneForeignRead(a, b, match=nth_read)
                code, lines, stderr = run(FakePftl([good]), endpoint, "--state-file", str(state_file))
                self.assertEqual(endpoint.used, "eth_getBlockByNumber")
                self.assertEqual(code, 1, stderr)
                self.assertIn("base_anchor_mismatch", stderr)
                self.assertEqual(alarm_codes(lines), [])
                self.assertFalse(state_file.exists())
                # All from A afterwards: the honest result.
                code, lines, _ = run(FakePftl([good]), FakeBase(backed(good), block_hashes=dict(home_hashes)),
                                     "--state-file", str(state_file))
                expected = (2, ["release_source_block_mismatch"]) if home_hashes else (0, [])
                self.assertEqual((code, alarm_codes(lines)), expected)

    def test_every_state_read_names_the_anchor_by_hash(self):
        releases = [release(0), release(1, source_block=800)]
        e = export_entry(0, height=400)
        base = ExportBase([released_log(e, block=950)], locked=2_000_000, finalized=1_000)
        base.locks = backed(*releases)
        pftl = ExportPftl([e], released=2_000_000)
        pftl.releases, pftl.history_root = releases, "0x" + "ab" * 48
        code, lines, stderr = run_exports(pftl, base)
        self.assertEqual((code, stderr), (0, ""))
        self.assertEqual(base.unanchored_state_reads, [])
        self.assertEqual(base.hash_reads, [chain_block_hash(8453, 950)])
        self.assertEqual(base.block_reads[0], "finalized")
        self.assertEqual(base.block_reads[-1], hex(1_000), "the anchor is read again by number at the end")

    def test_a_log_not_bound_to_the_anchored_chain_is_refused(self):
        e = export_entry(0, height=400)
        cases = {
            "removed": ({"removed": True}, "base_anchor_mismatch"),
            "no hash": ({"blockHash": None}, "base_anchor_mismatch"),
            "another block's hash": ({"blockHash": chain_block_hash(8453, 951)}, "base_anchor_mismatch"),
            "an unknown hash": ({"blockHash": "0x" + "5e" * 32}, "base_anchor_unknown"),
        }
        for name, (fields, expected) in cases.items():
            with self.subTest(name), tempfile.TemporaryDirectory() as directory:
                lane = ExportPftl([e], released=2_000_000)
                state_file = self.seed(directory, lane)
                before = state_file.read_bytes()
                log = {**released_log(e, block=950), **fields}
                self.refused(run_exports(lane, ExportBase([log], locked=2_000_000, finalized=1_000),
                                         "--state-file", str(state_file)), state_file, before, expected)

    def test_an_anchor_that_changes_before_the_end_of_the_run_is_refused(self):
        e = export_entry(0, height=400)
        with tempfile.TemporaryDirectory() as directory:
            lane = ExportPftl([e], released=2_000_000)
            state_file = self.seed(directory, lane)
            before = state_file.read_bytes()

            class Reorged(ExportBase):
                def call(self, method, params):
                    if method == "eth_getBlockByNumber" and params[0] == hex(self.finalized):
                        return {**self.block(self.finalized), "hash": "0x" + "4e" * 32}
                    return super().call(method, params)

            self.refused(run_exports(lane, Reorged([released_log(e, block=950)], locked=2_000_000,
                                                   finalized=1_000), "--state-file", str(state_file)),
                         state_file, before, "base_anchor_mismatch")

    def test_any_single_foreign_read_of_a_release_run_is_refused_or_changes_nothing(self):
        """Every read of a release-view run (with --exports or without),
        answered in turn by another network whose Locks back every release
        or none, or by a fork that shares Base's history (and Locks) through
        block 899, each keeping the other's later blocks as side blocks,
        against a Base whose Locks back all or all but one."""

        releases = [release(0), release(1, source_block=800), release(2, source_block=FINALIZED + 1)]
        e = export_entry(0, height=400)
        counted = 0
        for exports, home_locks, foreign_locks in itertools.product(
                (False, True), ("all", "one missing"), ("all", "none", "fork")):
            def node(chain_id, locks):
                base = ExportBase([released_log(e, block=950)], locked=2_000_000)
                base.chain_id = chain_id
                base.locks = backed(*releases) if locks == "all" else (
                    backed(*releases[:1], *releases[2:]) if locks == "one missing" else {})
                return base

            def attempt(endpoint, directory):
                state_file = Path(directory) / "state.json"
                pftl = ExportPftl([e], released=2_000_000)
                pftl.releases, pftl.history_root = releases, "0x" + "ab" * 48
                outcome = run(pftl, endpoint, "--state-file", str(state_file),
                              *(("--exports", "--lock-from-block", "0") if exports else ()))
                return outcome, state_file.read_bytes() if state_file.exists() else None

            with tempfile.TemporaryDirectory() as directory:
                honest = attempt(node(8453, home_locks), directory)
            index = 0
            while True:
                home = node(8453, home_locks)
                other = (fork_of(node(8453, home_locks), home, 899) if foreign_locks == "fork"
                         else node(FOREIGN_CHAIN, foreign_locks))
                endpoint = OneForeignRead(home, other, index)
                with tempfile.TemporaryDirectory() as directory:
                    (code, lines, stderr), saved = attempt(endpoint, directory)
                if endpoint.used is None:
                    break
                with self.subTest(exports=exports, home=home_locks, foreign=foreign_locks, read=index,
                                  method=endpoint.used):
                    if code == 1:
                        self.assertIsNone(saved)
                        self.assertFalse([line for line in lines if line["kind"] in ("payout", "export")])
                    else:
                        self.assertTrue(((code, lines, stderr), saved) == honest, "not the all-honest result")
                counted += 1
                index += 1
        self.assertGreater(counted, 50)


U64_MAX = (1 << 64) - 1
NOT_USED = object()  # the model's foreign read is past the run's last Base read
MODEL_AMOUNTS = (400_000, 300_000, 200_000, 100_000)  # older unpaid exports leave room in atoms too


class ExportModelTests(unittest.TestCase):
    """The property the merge depends on, over every sequence of honest
    answers up to a depth: no bad payout (a second payout of one export, or a
    payout with no export) is ever printed `backed`, `verified_earlier` or
    `paid`, in that run or a later one; no run that scans one exits 0; none
    is saved. A legitimate payout of an export the run's export pages retain
    is never an alarm, and a lagging input (a Base node below the kept scan,
    a PFTL snapshot below the kept marks) is an error, never an alarm, and
    leaves the export state unchanged.

    The lane: exports 0..3, export n at PFTL time n (height 100 + n), each
    pruned two steps later, the newest never. Base may finalize a legitimate
    payout (the newest unpaid export, so older unpaid ones leave pruned
    room), a second payout of a paid export, or a payout with no export. The
    state is carried from run to run as the CLI carries its file. Three
    alphabets, each exhaustive (nothing random):

    - PFTL answers: the PFTL time advances or stays and the export pages may
      come from a node one or two steps behind; normal runs; depth 4.
    - Rescans: each run is normal, `--from-height 0`, a `--lock-from-block`
      alternating between 1 and 2 (before every payout) or a late one after
      every earlier payout (after an unseen one, if any); the Base finalized
      block may go back 20 blocks, below the previous run's; the PFTL time
      advances and the export pages may come from a node one step behind;
      depth 3.
    - Networks: the export pages come from the release view's network or
      from one whose genesis, PFTL chain id, lock or source chain id
      differs (a failover between the two reads); each run is normal,
      `--from-height 0` or an alternating `--lock-from-block`; the PFTL time
      advances; depth 3. Another network's pages are refused
      (`export_identity_mismatch`) with no payout or export line and the
      export state kept.
    - Base reads: any single Base read of a run (the chain id, the
      finalized block, the code, a log chunk, a block read by hash, each
      `eth_call`, the end re-read) may be answered by an honest node of
      another Base network (chain 84532, the same lock address, its own
      payouts of exports 0 and 2), or of a fork that shares the home chain's
      history through block 1000 with those payouts above it, each node
      keeping the other's blocks as side blocks (known by hash, not
      canonical); at most one such read per sequence; each
      run is normal, `--from-height 0` or an alternating `--lock-from-block`;
      depth 3. Such a run is refused (exit 1, the state kept) or gives
      exactly the all-honest run's lines and state.
    - PFTL reads: any single PFTL read of the export view (the tip, a block,
      either read of an export page) may be answered by a node of a fork
      that shares this chain's genesis and lane at the same height, holding
      one more export, the orphan payout's packet; at most one per
      sequence; depth 3. Refused or exactly the all-honest result, as above."""

    AMOUNTS = MODEL_AMOUNTS
    ENTRIES = [export_entry(n, amount=MODEL_AMOUNTS[n], height=100 + n) for n in range(4)]
    ORPHAN = export_entry(9, amount=50_000)
    PAYOUTS = ("none", "legit", "duplicate", "orphan")
    BASE_BACK = 20

    def pftl_fork(self, home: ExportPftl) -> ExportPftl:
        """A node of a fork that shares this chain's genesis and lane, at the
        same height: the validated chain's exports and one more it never
        made, the orphan payout's packet, so its counters do not go back."""

        extra = json.loads(json.dumps(self.ORPHAN))
        extra["packet"].update(export_nonce=home.pruned + len(home.exports), exported_at_height=home.current_height)
        fork = ExportPftl([*home.exports, extra], pruned=home.pruned, pruned_atoms=home.pruned_atoms,
                          released=home.released, current_height=home.current_height)
        fork.fork = "b"
        return fork

    def snapshot(self, time: int) -> ExportPftl:
        exported = min(time, 3) + 1
        pruned = min(max(0, time - 1), exported - 1)
        return ExportPftl(self.ENTRIES[pruned:exported], pruned=pruned, pruned_atoms=sum(self.AMOUNTS[:pruned]),
                          released=1_000_000, current_height=100 + time)

    def model(self, depth, steps, kinds, bases, least, networks=("same",), base_reads=()):
        import functools
        counted = {"sequences": 0, "runs": 0, "foreign_runs": 0, "refused": 0, "fork_runs": 0}
        alphabet = (steps, kinds, bases, networks, base_reads)
        with unittest.mock.patch.object(watcher, "keccak256", functools.lru_cache(maxsize=None)(watcher.keccak256)):
            self.explore(alphabet, depth, 0, 0, None, [], frozenset(), [], counted)
        print(f"\nexport model: {len(steps)} PFTL x {len(self.PAYOUTS)} payout x {len(kinds)} start x "
              f"{len(bases)} Base x {len(networks)} network choices"
              f"{' x any single read foreign (' + ', '.join(base_reads) + ')' if base_reads else ''}, "
              f"depth {depth}: {counted['sequences']} sequences, {counted['runs']} runs"
              + (f" ({counted['foreign_runs']} with a foreign read, {counted['fork_runs']} of them from the fork, "
                 f"{counted['refused']} refused)" if base_reads else ""))
        self.assertGreater(counted["sequences"], least)

    def test_no_sequence_of_honest_answers_backs_a_bad_payout(self):
        self.model(4, ("advance", "stay", "lag1", "lag2"), ("normal",), ("ok",), 1_000)

    def test_no_rescan_late_start_or_lagging_base_node_backs_a_bad_payout(self):
        self.model(3, ("advance", "lag1"), ("normal", "from_height", "lock_alt", "late"), ("ok", "back"), 5_000)

    def test_no_export_view_of_another_network_backs_a_payout(self):
        self.model(3, ("advance",), ("normal", "from_height", "lock_alt"), ("ok",), 50_000,
                   networks=("same", *IDENTITY_FIELDS))

    def explore(self, alphabet, depth, run_index, time, state, logs, paid, bad, counted, trail=(), spent=False):
        if run_index == depth:
            counted["sequences"] += 1
            return
        steps, kinds, bases, networks, base_reads = alphabet
        for step, payout, kind, base_step, network in itertools.product(steps, self.PAYOUTS, kinds, bases, networks):
            # No foreign read, then each Base read of the run in turn from each
            # foreign network, until one is past the run's last read.
            if not self.explore_one(alphabet, depth, run_index, time, state, logs, paid, bad, counted, trail,
                                    spent, step, payout, kind, base_step, network, None):
                continue
            for foreign_kind in () if spent else base_reads:
                for index in itertools.count():
                    if not self.explore_one(alphabet, depth, run_index, time, state, logs, paid, bad, counted,
                                            trail, spent, step, payout, kind, base_step, network,
                                            (foreign_kind, index)):
                        break

    def explore_one(self, alphabet, depth, run_index, time, state, logs, paid, bad, counted, trail, spent,
                    step, payout, kind, base_step, network, foreign_read):
        """One run of the tree; False once `foreign_read` is past the run's last Base read."""

        now = time + 1 if step == "advance" else time
        answer = now - {"lag1": 1, "lag2": 2}.get(step, 0)
        if answer < 0 or run_index == 0 and (kind == "late" or base_step == "back"):
            return False
        block = 1_000 + 10 * run_index + 5
        transaction = "0x" + f"{run_index * 16 + len(logs):064x}"
        exists = [n for n in range(min(now, 3) + 1) if n not in paid]
        if payout == "legit" and not exists or payout == "duplicate" and not paid:
            return False
        new_logs, new_paid, new_bad = list(logs), paid, list(bad)
        if payout == "legit":
            new_logs.append(released_log(self.ENTRIES[exists[-1]], block=block, transaction=transaction))
            new_paid = paid | {exists[-1]}
        elif payout != "none":
            entry = self.ENTRIES[min(paid)] if payout == "duplicate" else self.ORPHAN
            new_logs.append(released_log(entry, block=block, transaction=transaction))
            new_bad.append((block, transaction))
        kept_from = ((state or {}).get("exports") or {}).get("lock_from_block", 0)
        lock_from = {"lock_alt": 2 if kept_from == 1 else 1, "late": 1_000 + 10 * run_index}.get(kind, kept_from)
        finalized = 1_000 + 10 * run_index + 9 - (self.BASE_BACK if base_step == "back" else 0)
        path = (*trail, (step, payout, kind, base_step, network, foreign_read))
        new_state = self.run_once(state, now, answer, new_logs, finalized, new_bad, new_paid, path,
                                  from_height=0 if kind == "from_height" else None, lock_from=lock_from,
                                  network=network, foreign_read=foreign_read, counted=counted)
        if new_state is NOT_USED:
            return False
        counted["runs"] += 1
        self.explore(alphabet, depth, run_index + 1, now, new_state, new_logs, new_paid, new_bad, counted, path,
                     spent or foreign_read is not None)
        return True

    def answers(self, now, answer, network="same"):
        current, lagging = self.snapshot(now), self.snapshot(answer)
        if network != "same":
            foreign(lagging, network)

        class Answers:
            # The first lane page from a current node, the export pages from one `answer` behind,
            # of the same network or of another (`network`: the identity field that differs).
            def external_lane_info(self, *args, **kwargs):
                return (lagging if "export_limit" in kwargs else current).external_lane_info(*args, **kwargs)

            def status(self):
                return lagging.status()

            def blocks(self, **kwargs):
                return lagging.blocks(**kwargs)

        return Answers(), lagging

    # Another Base network's own history: payouts of exports 0 and 2, at the
    # blocks of the first and third runs, in its own transactions.
    FOREIGN_LOGS = [released_log(ENTRIES[0], block=1_005, transaction="0x" + "b0" * 32),
                    released_log(ENTRIES[2], block=1_025, transaction="0x" + "b2" * 32)]
    FORK_HEIGHT = 1_000  # the fork shares the home chain through this block

    def run_once(self, state, now, answer, logs, finalized, bad, paid, path, *, from_height=None, lock_from=0,
                 network="same", foreign_read=None, counted=None):
        def attempt(base, wrap=None):
            pftl, lagging = self.answers(now, answer, network)
            if wrap is not None:
                pftl = wrap(pftl, lagging)
            loaded = json.loads(json.dumps(state)) if state is not None else None
            try:
                return lagging, watcher.watch_once(pftl, base, LANE, state=loaded, exports=True,
                                                   from_height=from_height, lock_from_block=lock_from)
            except watcher.WatcherError:
                return lagging, None  # exit 1: the file is kept

        lagging, outcome = attempt(ExportBase(logs, locked=10_000_000, finalized=finalized))
        if foreign_read is not None and foreign_read[0] == "pftl_fork":
            made: list[OnePftlForkRead] = []

            def wrap(pftl, lagging):
                made.append(OnePftlForkRead(pftl, self.pftl_fork(lagging), foreign_read[1]))
                return made[-1]

            honest, (lagging, outcome) = outcome, attempt(ExportBase(logs, locked=10_000_000, finalized=finalized),
                                                          wrap)
            endpoint = made[-1]
            foreign_kind = "pftl_fork"
        elif foreign_read is not None:
            foreign_kind, index = foreign_read
            home = ExportBase(logs, locked=10_000_000, finalized=finalized)
            other = ExportBase(self.FOREIGN_LOGS, locked=10_000_000, finalized=finalized)
            if foreign_kind == "fork":
                fork_of(other, home, self.FORK_HEIGHT)
            else:
                foreign_base(other)
            endpoint = OneForeignRead(home, other, index)
            honest, (lagging, outcome) = outcome, attempt(endpoint)
        if foreign_read is not None:
            if endpoint.used is None:
                return NOT_USED
            counted["foreign_runs"] += 1
            counted["fork_runs"] += foreign_kind in ("fork", "pftl_fork")
            if outcome is None:
                counted["refused"] += 1
            else:
                # Not refused: exactly the all-honest run, lines and state.
                self.assertTrue(outcome == honest, f"{path} {endpoint.used}: not the all-honest result")
        if outcome is None:
            return state
        lines, alarms, new_state = outcome
        failed = any(line["kind"] == "error" for line in lines)
        code = 2 if alarms else 1 if failed else 0
        summary, error = lines[-1].get("exports"), lines[-1].get("export_error")
        kept = (state or {}).get("exports")
        retained = {watcher.export_evm_digest(entry["packet_digest"])
                    for entry in self.ENTRIES[lagging.pruned:lagging.pruned + len(lagging.exports)]}
        for line in lines:
            if line["kind"] == "payout" and (line["block"], line["transaction"]) in bad:
                self.assertNotIn(line["status"], ("backed", "verified_earlier"), path)
            elif line["kind"] == "payout" and line["packet_digest"] in retained:
                self.assertNotEqual(line["status"], "alarm", path)
            elif line["kind"] == "export" and line["status"] == "paid":
                self.assertNotIn((line["base_block"], line["base_transaction"]), bad, path)
        if summary is not None and any(summary["scanned_from_block"] <= block <= finalized for block, _ in bad):
            self.assertNotEqual(code, 0, path)
        if network != "same":
            # Another network's export pages: refused before any packet is used.
            self.assertEqual(error, "export_identity_mismatch", path)
            self.assertFalse([line for line in lines if line["kind"] in ("payout", "export")], path)
        elif kept and kept.get("format") == watcher.EXPORT_STATE_FORMAT and kept["scanned_through_block"] > finalized:
            self.assertEqual(error, "base_finalized_behind_state", path)
        if error is not None:
            # A lagging input is an error, never an alarm, and keeps the export state.
            self.assertEqual((alarms, (new_state or {}).get("exports")), (0, kept), path)
        saved = (new_state or state or {}).get("exports") or {}
        kept_transactions = {entry["transaction"] for entry in [*saved.get("paid", []),
                                                                 *saved.get("pruned_paid_recent", [])]}
        self.assertFalse(kept_transactions & {transaction for _, transaction in bad}, path)
        self.assertLessEqual(saved.get("pruned_paid_count", 0) + len(saved.get("paid", [])), len(paid), path)
        return new_state if new_state is not None else state

    def test_no_single_foreign_base_read_backs_a_payout_or_changes_the_result(self):
        self.model(3, ("advance",), ("normal", "from_height", "lock_alt"), ("ok",), 40_000,
                   base_reads=("disjoint", "fork"))

    def test_no_single_pftl_read_from_a_same_genesis_fork_backs_a_payout(self):
        self.model(3, ("advance",), ("normal", "from_height", "lock_alt"), ("ok",), 15_000,
                   base_reads=("pftl_fork",))

    def test_a_state_of_another_identity_is_refused_on_every_start(self):
        """Directed, not exhaustive: through the command line, a saved model
        state whose genesis, lock or source chain id is not the run's is
        refused before any of it is used and left unchanged, whatever the
        run's start, also on a lagging Base node."""

        logs = [released_log(self.ENTRIES[1], block=1_005, transaction="0x" + "a1" * 32)]
        pftl, _ = self.answers(1, 1)
        changes = {"genesis_hash": "d0" * 48, "lock_contract_address": "0x" + "9f" * 20, "source_chain_id": 84532}
        starts = {"normal": (), "from_height": ("--from-height", "0"), "lock_alt": ("--lock-from-block", "1"),
                  "late": ("--lock-from-block", "1010"), "discard": ("--discard-export-history",)}
        with tempfile.TemporaryDirectory() as directory:
            state_file = Path(directory) / "state.json"
            code, _, _ = run(pftl, ExportBase(logs, locked=10_000_000, finalized=1_009), "--exports",
                             "--state-file", str(state_file))
            saved = json.loads(state_file.read_text())
            self.assertEqual((code, len(saved["exports"]["paid"])), (0, 1))
            for (key, value), (start, extra), finalized in itertools.product(changes.items(), starts.items(),
                                                                            (1_019, 999)):
                with self.subTest(key=key, start=start, finalized=finalized):
                    state_file.write_text(json.dumps({**saved, key: value}))
                    before = state_file.read_bytes()
                    code, lines, stderr = run(pftl, ExportBase(logs, locked=10_000_000, finalized=finalized),
                                              "--exports", "--state-file", str(state_file), *extra)
                    self.assertEqual((code, lines), (1, []))
                    self.assertIn("state_file_invalid", stderr)
                    self.assertEqual(state_file.read_bytes(), before)


class IntervalMalformedTests(unittest.TestCase):
    def test_a_malformed_answer_in_interval_mode_is_an_error_line_and_the_loop_goes_on(self):
        entry = export_entry(0)

        class Garbled(ExportBase):
            rounds = 0

            def call(self, method: str, params: list[Any]) -> Any:
                if method == "eth_chainId":
                    Garbled.rounds += 1
                if method == "eth_chainId" and Garbled.rounds == 1:
                    return "not hex"
                return super().call(method, params)

        sleeps: list[float] = []
        stdout, stderr = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(stdout), contextlib.redirect_stderr(stderr):
            code = watcher.main(["--pftl-rpc", "127.0.0.1:27650", "--lane-id", LANE, "--base-rpc",
                                 "http://127.0.0.1:8545", "--exports", "--interval", "30"],
                                pftl=ExportPftl([entry]), base=Garbled([released_log(entry)]),
                                sleep=sleeps.append, max_runs=2)
        self.assertEqual(code, 0)
        self.assertEqual((Garbled.rounds, sleeps), (2, [30.0]))
        self.assertEqual(json.loads(stderr.getvalue())["code"], "base_answer_malformed")
        summaries = [json.loads(line) for line in stdout.getvalue().splitlines() if '"summary"' in line]
        self.assertEqual([summary["alarms"] for summary in summaries], [0])


if __name__ == "__main__":
    unittest.main()
