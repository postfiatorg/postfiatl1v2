"""Reference builder and verifier for the proposed genesis registry.

Independent Python implementation of the canonical
``ProposedGenesisRegistryV1`` objects defined in
``crates/types/src/genesis_registry.rs`` (work-sequence step 2 of
``docs/architecture/genesis-registry-proposal-path.md``). It shares no
canonicalization code with the Rust implementation and must reproduce the
same domain-separated content hashes over the golden vectors in
``benchmarks/genesis-registry/fixtures/``.

Everything here is ``SHADOW_ONLY`` schema work: the tool reads archived round
artifacts from disk, opens no socket, and grants no authority. Artifacts the
archived rounds omit (bundle CID, convergence report, anchor transaction,
receipt deadline) use the deterministic fixture derivations documented in the
fixtures README; a real genesis round supplies all of them as frozen
artifacts.

Subcommands: ``build`` (round artifacts + receipt set -> registry, canonical
CBOR, content hash), ``verify`` (payload against its inputs), ``explain``
(human summary of a payload). All support ``--json``.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Any, Callable, Mapping, Sequence

REGISTRY_VERSION_V1 = 1
IDENTITY_RECEIPT_VERSION_V1 = 1
EVIDENCE_RECORD_VERSION_V1 = 1

REGISTRY_DOMAIN_V1 = "L1V2_PROPOSED_GENESIS_REGISTRY_V1"
IDENTITY_RECEIPT_DOMAIN_V1 = "L1V2_IDENTITY_RECEIPT_V1"
EVIDENCE_RECORD_DOMAIN_V1 = "L1V2_GENESIS_EVIDENCE_RECORD_V1"

FORK_MASTER_KEY_LEN = 33
MLDSA65_PUBLIC_KEY_LEN = 1952
DIGEST_LEN = 32
MAX_ENTRIES = 4096
MAX_TEXT_BYTES = 256
MAX_SELECTION_INDEX = 65_535
MAX_ROUND_NUMBER = 1_000_000_000
MAX_LEDGER_SEQ = 1 << 62
MAX_CLOSE_TIME = 1 << 62

DEFAULT_CHAIN_ID = "postfiat-l1v2-testnet"
FIXTURE_MLDSA_DOMAIN = b"L1V2_GR_FIXTURE_MLDSA_V1"
ROUND_FILES = (
    "inputs/model_request.json",
    "inputs/previous_unl.json",
    "inputs/validator_map.json",
    "outputs/model_response.json",
    "outputs/selected_unl.json",
    "outputs/validator_scores.json",
    "runtime/execution_manifest.json",
)
RIPPLE_ALPHABET = "rpshnaf39wBUDNEGHJKLM4PQRST7VWXYZ2bcdeCg65jkm8oFqi1tuvAxyz"


class GenesisRegistryError(Exception):
    """Named canonical-encoding or validation error (closed code set)."""

    def __init__(self, code: str, detail: str = "") -> None:
        self.code = code
        super().__init__(f"{code}: {detail}" if detail else code)


def _sha256(data: bytes) -> bytes:
    return hashlib.sha256(data).digest()


def domain_digest(label: str, canonical_cbor: bytes) -> bytes:
    """``SHA-256(uint16_be(len(label)) || ASCII(label) || cbor)``."""
    raw = label.encode("ascii")
    return _sha256(len(raw).to_bytes(2, "big") + raw + canonical_cbor)


def decode_fork_master_key(encoded: str) -> bytes:
    """Base58 ``n...`` node public key -> 33-byte key material."""
    num = 0
    for ch in encoded:
        idx = RIPPLE_ALPHABET.find(ch)
        if idx < 0:
            raise ValueError(f"invalid base58 character in {encoded!r}")
        num = num * 58 + idx
    raw = num.to_bytes((num.bit_length() + 7) // 8, "big")
    pad = 0
    for ch in encoded:
        if ch != RIPPLE_ALPHABET[0]:
            break
        pad += 1
    raw = b"\x00" * pad + raw
    if len(raw) != 38 or raw[0] != 28:
        raise ValueError(f"not a node public key: {encoded!r}")
    if _sha256(_sha256(raw[:34]))[:4] != raw[34:]:
        raise ValueError(f"bad base58 checksum: {encoded!r}")
    return raw[1:34]


def deterministic_final_score(row: Mapping[str, Any]) -> int:
    """``min((50c+20r+10s+10d+10i)//100, c+25)`` over integer sub-scores."""
    subs = {}
    for field in ("consensus", "reliability", "software", "diversity", "identity"):
        value = row[field]
        if not isinstance(value, int) or isinstance(value, bool) or not 0 <= value <= 100:
            raise ValueError(f"sub-score {field} out of range: {value!r}")
        subs[field] = value
    c, r, s = subs["consensus"], subs["reliability"], subs["software"]
    d, i = subs["diversity"], subs["identity"]
    return min((50 * c + 20 * r + 10 * s + 10 * d + 10 * i) // 100, c + 25)


# ---------------------------------------------------------------------------
# Deterministic CBOR subset (RFC 8949 §4.2, definite lengths only)
# ---------------------------------------------------------------------------

_MAJOR_UINT, _MAJOR_BYTES, _MAJOR_TEXT, _MAJOR_ARRAY, _MAJOR_MAP = 0, 2, 3, 4, 5


def _cbor_head(major: int, arg: int) -> bytes:
    if arg < 24:
        return bytes([(major << 5) | arg])
    for info, size in ((24, 1), (25, 2), (26, 4), (27, 8)):
        if arg < 1 << (8 * size):
            return bytes([(major << 5) | info]) + arg.to_bytes(size, "big")
    raise ValueError("cbor argument out of range")


def _cbor_uint(value: int) -> bytes:
    return _cbor_head(_MAJOR_UINT, value)


def _cbor_bytes(value: bytes) -> bytes:
    return _cbor_head(_MAJOR_BYTES, len(value)) + value


def _cbor_text(value: str) -> bytes:
    raw = value.encode("utf-8")
    return _cbor_head(_MAJOR_TEXT, len(raw)) + raw


class _CborReader:
    def __init__(self, data: bytes) -> None:
        self.data = data
        self.pos = 0

    def _take(self, length: int) -> bytes:
        end = self.pos + length
        if end > len(self.data):
            raise GenesisRegistryError("truncated")
        out = self.data[self.pos : end]
        self.pos = end
        return out

    def _head(self, expected_major: int) -> int:
        initial = self._take(1)[0]
        major, info = initial >> 5, initial & 0x1F
        if major != expected_major:
            raise GenesisRegistryError("wrong_type")
        if info < 24:
            return info
        if info > 27:
            raise GenesisRegistryError("non_canonical_encoding")
        size = 1 << (info - 24)
        arg = int.from_bytes(self._take(size), "big")
        minimal = 24 if size == 1 else 1 << (8 * (size >> 1))
        if arg < minimal:
            raise GenesisRegistryError("non_canonical_encoding")
        return arg

    def uint(self) -> int:
        return self._head(_MAJOR_UINT)

    def bytes_(self) -> bytes:
        return self._take(self._head(_MAJOR_BYTES))

    def text(self) -> str:
        raw = self._take(self._head(_MAJOR_TEXT))
        try:
            return raw.decode("utf-8")
        except UnicodeDecodeError as exc:
            raise GenesisRegistryError("invalid_text_encoding") from exc

    def array_len(self) -> int:
        return self._head(_MAJOR_ARRAY)

    def map_len(self) -> int:
        return self._head(_MAJOR_MAP)

    def finish(self) -> None:
        if self.pos != len(self.data):
            raise GenesisRegistryError("trailing_bytes")


def _read_closed_map(
    reader: _CborReader, expected: int, field: Callable[[int, _CborReader], None]
) -> None:
    """Closed ascending integer labels ``1..=expected``; fail-closed."""
    length = reader.map_len()
    previous = None
    for _ in range(length):
        key = reader.uint()
        if previous is not None:
            if key == previous:
                raise GenesisRegistryError("duplicate_field")
            if key < previous:
                raise GenesisRegistryError("non_canonical_encoding")
        previous = key
        if key == 0 or key > expected:
            raise GenesisRegistryError("unknown_field")
        field(key, reader)
    if length != expected:
        raise GenesisRegistryError("missing_field")


# ---------------------------------------------------------------------------
# Registry dict representation (mirrors the golden-vector "registry" JSON)
# ---------------------------------------------------------------------------

_ROUND_DIGEST_FIELDS = (
    "bundle_digest_hex",
    "manifest_digest_hex",
    "final_scores_digest_hex",
    "selected_unl_digest_hex",
    "convergence_report_digest_hex",
    "anchor_tx_hash_hex",
)


def _require_text(value: str, allow_empty: bool, code: str) -> None:
    if (not allow_empty and not value) or len(value.encode("utf-8")) > MAX_TEXT_BYTES:
        raise GenesisRegistryError(code)


def _require_master_key(key: bytes) -> None:
    if len(key) != FORK_MASTER_KEY_LEN or key[0] not in (0x02, 0x03, 0xED):
        raise GenesisRegistryError("invalid_master_key")


def validate_registry(registry: Mapping[str, Any]) -> None:
    """Full structural validation with the shared named-error codes."""
    if registry["version"] != REGISTRY_VERSION_V1:
        raise GenesisRegistryError("unknown_version")
    _require_text(registry["chain_id"], False, "invalid_chain_id")
    round_ref = registry["genesis_round"]
    _require_text(round_ref["fork_network"], False, "invalid_fork_network")
    if not 1 <= round_ref["round_number"] <= MAX_ROUND_NUMBER:
        raise GenesisRegistryError("invalid_round_number")
    _require_text(round_ref["bundle_cid"], False, "invalid_bundle_cid")
    for field in _ROUND_DIGEST_FIELDS:
        if len(bytes.fromhex(round_ref[field])) != DIGEST_LEN:
            raise GenesisRegistryError("invalid_digest_length")
    deadline = registry["receipt_deadline"]
    if len(bytes.fromhex(deadline["fork_ledger_hash_hex"])) != DIGEST_LEN:
        raise GenesisRegistryError("invalid_digest_length")
    if not 1 <= deadline["fork_ledger_seq"] <= MAX_LEDGER_SEQ:
        raise GenesisRegistryError("invalid_ledger_seq")

    entries = registry["entries"]
    if not entries:
        raise GenesisRegistryError("empty_entries")
    if len(entries) > MAX_ENTRIES:
        raise GenesisRegistryError("too_many_entries")
    seen_indices: set[int] = set()
    previous_key = None
    for entry in entries:
        key = bytes.fromhex(entry["fork_master_key_hex"])
        _require_master_key(key)
        if entry["final_score"] > 100:
            raise GenesisRegistryError("score_out_of_range")
        if entry["cutoff"] > 100:
            raise GenesisRegistryError("cutoff_out_of_range")
        if entry["final_score"] < entry["cutoff"]:
            raise GenesisRegistryError("score_below_cutoff")
        if entry["selection_index"] > MAX_SELECTION_INDEX:
            raise GenesisRegistryError("invalid_selection_index")
        if len(bytes.fromhex(entry["identity_evidence_digest_hex"])) != DIGEST_LEN:
            raise GenesisRegistryError("invalid_digest_length")
        if len(bytes.fromhex(entry["identity_receipt_digest_hex"])) != DIGEST_LEN:
            raise GenesisRegistryError("invalid_digest_length")
        if len(bytes.fromhex(entry["mldsa_public_key_hex"])) != MLDSA65_PUBLIC_KEY_LEN:
            raise GenesisRegistryError("invalid_mldsa_key_length")
        if entry["cutoff"] != entries[0]["cutoff"]:
            raise GenesisRegistryError("cutoff_mismatch")
        if entry["selection_index"] in seen_indices:
            raise GenesisRegistryError("duplicate_selection_index")
        seen_indices.add(entry["selection_index"])
        if previous_key is not None:
            if key == previous_key:
                raise GenesisRegistryError("duplicate_master_key")
            if key < previous_key:
                raise GenesisRegistryError("unsorted_entries")
        previous_key = key

    graph = registry["template_trust_graph"]
    if graph["n_s"] != len(entries):
        raise GenesisRegistryError("trust_graph_mismatch")
    expected = template_trust_graph(graph["n_s"])
    if graph["q_s"] != expected["q_s"] or graph["t_s"] != expected["t_s"]:
        raise GenesisRegistryError("trust_graph_mismatch")


def template_trust_graph(n: int) -> dict[str, int]:
    """``q_S = ceil(4n/5)``, ``t_S = min(ceil(n/5), (q_S-1)//2, 2q_S-n-1)``."""
    if n <= 0:
        raise GenesisRegistryError("empty_entries")
    if n > MAX_ENTRIES:
        raise GenesisRegistryError("too_many_entries")
    q_s = -((-4 * n) // 5)
    t_s = min(-((-n) // 5), (q_s - 1) // 2, 2 * q_s - n - 1)
    if t_s < 1 or 2 * t_s >= q_s or t_s >= 2 * q_s - n:
        raise GenesisRegistryError("trust_graph_unsafe")
    return {"n_s": n, "q_s": q_s, "t_s": t_s}


def encode_registry(registry: Mapping[str, Any]) -> bytes:
    """Validated deterministic-CBOR encoding of the registry dict."""
    validate_registry(registry)
    round_ref = registry["genesis_round"]
    deadline = registry["receipt_deadline"]
    graph = registry["template_trust_graph"]
    out = [_cbor_head(_MAJOR_MAP, 6)]
    out += [_cbor_uint(1), _cbor_uint(registry["version"])]
    out += [_cbor_uint(2), _cbor_text(registry["chain_id"])]
    out += [_cbor_uint(3), _cbor_head(_MAJOR_MAP, 9)]
    out += [_cbor_uint(1), _cbor_text(round_ref["fork_network"])]
    out += [_cbor_uint(2), _cbor_uint(round_ref["round_number"])]
    out += [_cbor_uint(3), _cbor_text(round_ref["bundle_cid"])]
    for label, field in enumerate(_ROUND_DIGEST_FIELDS, start=4):
        out += [_cbor_uint(label), _cbor_bytes(bytes.fromhex(round_ref[field]))]
    out += [_cbor_uint(4), _cbor_head(_MAJOR_MAP, 2)]
    out += [_cbor_uint(1), _cbor_bytes(bytes.fromhex(deadline["fork_ledger_hash_hex"]))]
    out += [_cbor_uint(2), _cbor_uint(deadline["fork_ledger_seq"])]
    out += [_cbor_uint(5), _cbor_head(_MAJOR_ARRAY, len(registry["entries"]))]
    for entry in registry["entries"]:
        out += [_cbor_head(_MAJOR_MAP, 7)]
        out += [_cbor_uint(1), _cbor_bytes(bytes.fromhex(entry["fork_master_key_hex"]))]
        out += [_cbor_uint(2), _cbor_uint(entry["final_score"])]
        out += [_cbor_uint(3), _cbor_uint(entry["cutoff"])]
        out += [_cbor_uint(4), _cbor_uint(entry["selection_index"])]
        out += [_cbor_uint(5), _cbor_bytes(bytes.fromhex(entry["identity_evidence_digest_hex"]))]
        out += [_cbor_uint(6), _cbor_bytes(bytes.fromhex(entry["identity_receipt_digest_hex"]))]
        out += [_cbor_uint(7), _cbor_bytes(bytes.fromhex(entry["mldsa_public_key_hex"]))]
    out += [_cbor_uint(6), _cbor_head(_MAJOR_MAP, 3)]
    out += [_cbor_uint(1), _cbor_uint(graph["n_s"])]
    out += [_cbor_uint(2), _cbor_uint(graph["q_s"])]
    out += [_cbor_uint(3), _cbor_uint(graph["t_s"])]
    return b"".join(out)


def decode_registry(data: bytes) -> dict[str, Any]:
    """Strict canonical decode + validation + re-encode equality backstop."""
    reader = _CborReader(data)
    registry: dict[str, Any] = {}

    def read_round(r: _CborReader) -> dict[str, Any]:
        round_ref: dict[str, Any] = {}

        def field(key: int, rr: _CborReader) -> None:
            if key == 1:
                round_ref["fork_network"] = rr.text()
            elif key == 2:
                round_ref["round_number"] = rr.uint()
            elif key == 3:
                round_ref["bundle_cid"] = rr.text()
            else:
                raw = rr.bytes_()
                if len(raw) != DIGEST_LEN:
                    raise GenesisRegistryError("invalid_digest_length")
                round_ref[_ROUND_DIGEST_FIELDS[key - 4]] = raw.hex()

        _read_closed_map(r, 9, field)
        return round_ref

    def read_deadline(r: _CborReader) -> dict[str, Any]:
        deadline: dict[str, Any] = {}

        def field(key: int, rr: _CborReader) -> None:
            if key == 1:
                raw = rr.bytes_()
                if len(raw) != DIGEST_LEN:
                    raise GenesisRegistryError("invalid_digest_length")
                deadline["fork_ledger_hash_hex"] = raw.hex()
            else:
                deadline["fork_ledger_seq"] = rr.uint()

        _read_closed_map(r, 2, field)
        return deadline

    def read_entry(r: _CborReader) -> dict[str, Any]:
        entry: dict[str, Any] = {}

        def field(key: int, rr: _CborReader) -> None:
            if key == 1:
                raw = rr.bytes_()
                if len(raw) != FORK_MASTER_KEY_LEN:
                    raise GenesisRegistryError("invalid_master_key")
                entry["fork_master_key_hex"] = raw.hex()
            elif key == 2:
                entry["final_score"] = rr.uint()
            elif key == 3:
                entry["cutoff"] = rr.uint()
            elif key == 4:
                entry["selection_index"] = rr.uint()
            elif key in (5, 6):
                raw = rr.bytes_()
                if len(raw) != DIGEST_LEN:
                    raise GenesisRegistryError("invalid_digest_length")
                name = "identity_evidence_digest_hex" if key == 5 else "identity_receipt_digest_hex"
                entry[name] = raw.hex()
            else:
                raw = rr.bytes_()
                if len(raw) != MLDSA65_PUBLIC_KEY_LEN:
                    raise GenesisRegistryError("invalid_mldsa_key_length")
                entry["mldsa_public_key_hex"] = raw.hex()

        _read_closed_map(r, 7, field)
        return entry

    def read_graph(r: _CborReader) -> dict[str, Any]:
        graph: dict[str, Any] = {}
        names = {1: "n_s", 2: "q_s", 3: "t_s"}

        def field(key: int, rr: _CborReader) -> None:
            graph[names[key]] = rr.uint()

        _read_closed_map(r, 3, field)
        return graph

    def field(key: int, r: _CborReader) -> None:
        if key == 1:
            registry["version"] = r.uint()
        elif key == 2:
            registry["chain_id"] = r.text()
        elif key == 3:
            registry["genesis_round"] = read_round(r)
        elif key == 4:
            registry["receipt_deadline"] = read_deadline(r)
        elif key == 5:
            length = r.array_len()
            if length > MAX_ENTRIES:
                raise GenesisRegistryError("too_many_entries")
            registry["entries"] = [read_entry(r) for _ in range(length)]
        else:
            registry["template_trust_graph"] = read_graph(r)

    _read_closed_map(reader, 6, field)
    reader.finish()
    validate_registry(registry)
    if encode_registry(registry) != data:
        raise GenesisRegistryError("non_canonical_encoding")
    return registry


def registry_hash(registry: Mapping[str, Any]) -> bytes:
    """``digest("L1V2_PROPOSED_GENESIS_REGISTRY_V1", registry)``."""
    return domain_digest(REGISTRY_DOMAIN_V1, encode_registry(registry))


def encode_receipt_body(receipt: Mapping[str, Any]) -> bytes:
    """Canonical encoding of ``GenesisIdentityReceiptBodyV1``."""
    key = bytes.fromhex(receipt["fork_master_key_hex"])
    mldsa = bytes.fromhex(receipt["mldsa_public_key_hex"])
    if receipt["version"] != IDENTITY_RECEIPT_VERSION_V1:
        raise GenesisRegistryError("unknown_version")
    _require_master_key(key)
    if len(mldsa) != MLDSA65_PUBLIC_KEY_LEN:
        raise GenesisRegistryError("invalid_mldsa_key_length")
    _require_text(receipt["chain_id"], False, "invalid_chain_id")
    _require_text(receipt["genesis_round_id"], False, "invalid_genesis_round_id")
    deadline_hash = bytes.fromhex(receipt["deadline_ledger_hash_hex"])
    if len(deadline_hash) != DIGEST_LEN:
        raise GenesisRegistryError("invalid_digest_length")
    if not 1 <= receipt["deadline_ledger_seq"] <= MAX_LEDGER_SEQ:
        raise GenesisRegistryError("invalid_ledger_seq")
    if not 1 <= receipt["expiry_close_time"] <= MAX_CLOSE_TIME:
        raise GenesisRegistryError("invalid_expiry")
    out = [_cbor_head(_MAJOR_MAP, 8)]
    out += [_cbor_uint(1), _cbor_uint(receipt["version"])]
    out += [_cbor_uint(2), _cbor_bytes(key)]
    out += [_cbor_uint(3), _cbor_bytes(mldsa)]
    out += [_cbor_uint(4), _cbor_text(receipt["chain_id"])]
    out += [_cbor_uint(5), _cbor_text(receipt["genesis_round_id"])]
    out += [_cbor_uint(6), _cbor_bytes(deadline_hash)]
    out += [_cbor_uint(7), _cbor_uint(receipt["deadline_ledger_seq"])]
    out += [_cbor_uint(8), _cbor_uint(receipt["expiry_close_time"])]
    return b"".join(out)


def receipt_hash(receipt: Mapping[str, Any]) -> bytes:
    """``digest("L1V2_IDENTITY_RECEIPT_V1", body)``."""
    return domain_digest(IDENTITY_RECEIPT_DOMAIN_V1, encode_receipt_body(receipt))


def evidence_digest(record: Mapping[str, Any]) -> bytes:
    """``digest("L1V2_GENESIS_EVIDENCE_RECORD_V1", record)``."""
    key = bytes.fromhex(record["fork_master_key_hex"])
    _require_master_key(key)
    if record["domain_verified"] not in (0, 1):
        raise GenesisRegistryError("invalid_domain_flag")
    out = [_cbor_head(_MAJOR_MAP, 6)]
    out += [_cbor_uint(1), _cbor_uint(EVIDENCE_RECORD_VERSION_V1)]
    out += [_cbor_uint(2), _cbor_bytes(key)]
    out += [_cbor_uint(3), _cbor_text(record["domain"])]
    out += [_cbor_uint(4), _cbor_uint(record["domain_verified"])]
    out += [_cbor_uint(5), _cbor_text(record["provider"])]
    out += [_cbor_uint(6), _cbor_text(record["country"])]
    return domain_digest(EVIDENCE_RECORD_DOMAIN_V1, b"".join(out))


# ---------------------------------------------------------------------------
# Builder
# ---------------------------------------------------------------------------


def _load_json(path: Path) -> Any:
    return json.loads(path.read_text())


def _extract_evidence_rows(model_request: Mapping[str, Any]) -> dict[str, Any]:
    """VALIDATOR DATA array from the frozen model request, by validator id."""
    for message in model_request["messages"]:
        if message["role"] != "user":
            continue
        content = message["content"]
        marker = "VALIDATOR DATA:"
        start = content.find(marker)
        if start < 0:
            raise ValueError("model request has no VALIDATOR DATA section")
        rows, _ = json.JSONDecoder().raw_decode(content[start + len(marker) :].lstrip())
        return {row["validator_id"]: row for row in rows}
    raise ValueError("model request has no user message")


def build_registry(
    round_dir: Path,
    receipts_path: Path,
    rounds_manifest_path: Path | None = None,
    chain_id: str = DEFAULT_CHAIN_ID,
) -> dict[str, Any]:
    """Builds the proposed registry from one archived round + receipt set."""
    round_dir = round_dir.resolve()
    round_id = round_dir.name
    if not round_id.startswith("testnet-r"):
        raise ValueError(f"unrecognized round directory name: {round_id!r}")
    round_number = int(round_id.removeprefix("testnet-r"))
    if rounds_manifest_path is None:
        rounds_manifest_path = round_dir.parent.parent / "rounds-manifest.json"
    manifest = _load_json(rounds_manifest_path)

    # Verify the archived files against the manifest; the fixture bundle
    # digest covers the per-file digest map.
    file_digests = manifest["rounds"][str(round_number)]
    if set(file_digests) != set(ROUND_FILES):
        raise ValueError(f"unexpected round file inventory for {round_id}")
    digest_map: dict[str, str] = {}
    for name in ROUND_FILES:
        actual = _sha256((round_dir / name).read_bytes()).hex()
        if actual != file_digests[name]["sha256"]:
            raise ValueError(f"digest mismatch for {round_id}/{name}")
        digest_map[name] = actual
    bundle_digest = _sha256(
        json.dumps(digest_map, sort_keys=True, separators=(",", ":")).encode()
    )

    execution_manifest = _load_json(round_dir / "runtime/execution_manifest.json")
    cutoff = execution_manifest["code"]["selector"]["parameters"]["score_cutoff"]

    scores = _load_json(round_dir / "outputs/validator_scores.json")["validator_scores"]
    score_rows = sorted(
        (row["master_key"], deterministic_final_score(row)) for row in scores
    )
    final_scores_digest = _sha256(
        json.dumps(
            {
                "final_scores": [
                    {"final_score": score, "master_key": key} for key, score in score_rows
                ]
            },
            sort_keys=True,
            separators=(",", ":"),
        ).encode()
    )
    score_by_key = dict(score_rows)

    validator_map = _load_json(round_dir / "inputs/validator_map.json")
    evidence_rows = _extract_evidence_rows(_load_json(round_dir / "inputs/model_request.json"))
    vid_by_master = {rec["master_key"]: vid for vid, rec in validator_map.items()}

    receipts_doc = _load_json(receipts_path)
    receipts = receipts_doc["receipts"]
    for receipt in receipts:
        if receipt["chain_id"] != chain_id or receipt["genesis_round_id"] != round_id:
            raise ValueError("receipt chain or round mismatch")
    receipt_by_key = {receipt["fork_master_key_hex"]: receipt for receipt in receipts}
    if len(receipt_by_key) != len(receipts):
        raise ValueError("duplicate receipt master key")
    deadline_hash = receipts[0]["deadline_ledger_hash_hex"]
    deadline_seq = receipts[0]["deadline_ledger_seq"]

    selected = _load_json(round_dir / "outputs/selected_unl.json")["unl"]
    entries = []
    for index, b58 in enumerate(selected):
        key_hex = decode_fork_master_key(b58).hex()
        receipt = receipt_by_key.get(key_hex)
        if receipt is None:
            continue  # Selected ∩ Receipted
        row = evidence_rows[vid_by_master[b58]]
        record = {
            "fork_master_key_hex": key_hex,
            "domain": row.get("domain") or "",
            "domain_verified": 1 if row.get("domain_verified") else 0,
            "provider": (row.get("asn") or {}).get("as_name") or "",
            "country": (row.get("geolocation") or {}).get("country") or "",
        }
        entries.append(
            {
                "fork_master_key_hex": key_hex,
                "final_score": score_by_key[b58],
                "cutoff": cutoff,
                "selection_index": index,
                "identity_evidence_digest_hex": evidence_digest(record).hex(),
                "identity_receipt_digest_hex": receipt_hash(receipt).hex(),
                "mldsa_public_key_hex": receipt["mldsa_public_key_hex"],
            }
        )
    entries.sort(key=lambda entry: entry["fork_master_key_hex"])

    registry = {
        "version": REGISTRY_VERSION_V1,
        "chain_id": chain_id,
        "genesis_round": {
            "fork_network": manifest["network"],
            "round_number": round_number,
            "bundle_cid": f"fixture:dunl-subscorer-shadow-20260901/{round_id}",
            "bundle_digest_hex": bundle_digest.hex(),
            "manifest_digest_hex": _sha256(
                (round_dir / "runtime/execution_manifest.json").read_bytes()
            ).hex(),
            "final_scores_digest_hex": final_scores_digest.hex(),
            "selected_unl_digest_hex": _sha256(
                (round_dir / "outputs/selected_unl.json").read_bytes()
            ).hex(),
            "convergence_report_digest_hex": _sha256(
                f"fixture-convergence:{round_id}".encode()
            ).hex(),
            "anchor_tx_hash_hex": _sha256(f"fixture-anchor:{round_id}".encode()).hex(),
        },
        "receipt_deadline": {
            "fork_ledger_hash_hex": deadline_hash,
            "fork_ledger_seq": deadline_seq,
        },
        "entries": entries,
        "template_trust_graph": template_trust_graph(len(entries)),
    }
    canonical = encode_registry(registry)
    return {
        "registry": registry,
        "canonical_cbor_hex": canonical.hex(),
        "proposed_registry_hash_hex": domain_digest(REGISTRY_DOMAIN_V1, canonical).hex(),
        "domain": REGISTRY_DOMAIN_V1,
        "round": round_id,
    }


# ---------------------------------------------------------------------------
# Verifier
# ---------------------------------------------------------------------------


def _payload_cbor(payload_path: Path) -> bytes:
    payload = _load_json(payload_path)
    for field in ("canonical_cbor_hex", "cbor_hex"):
        if field in payload:
            return bytes.fromhex(payload[field])
    raise ValueError(f"{payload_path} has neither canonical_cbor_hex nor cbor_hex")


def verify_payload(
    payload_path: Path,
    round_dir: Path,
    receipts_path: Path,
    rounds_manifest_path: Path | None = None,
    chain_id: str = DEFAULT_CHAIN_ID,
) -> dict[str, Any]:
    """Decodes a payload, rebuilds from inputs, and compares byte-for-byte."""
    report: dict[str, Any] = {"ok": False, "payload": str(payload_path)}
    data = _payload_cbor(payload_path)
    try:
        registry = decode_registry(data)
    except GenesisRegistryError as error:
        report["error"] = error.code
        return report
    rebuilt = build_registry(round_dir, receipts_path, rounds_manifest_path, chain_id)
    payload_hash = domain_digest(REGISTRY_DOMAIN_V1, data).hex()
    report.update(
        {
            "proposed_registry_hash_hex": payload_hash,
            "rebuilt_hash_hex": rebuilt["proposed_registry_hash_hex"],
            "hash_match": payload_hash == rebuilt["proposed_registry_hash_hex"],
            "bytes_match": data.hex() == rebuilt["canonical_cbor_hex"],
            "entries": len(registry["entries"]),
            "template_trust_graph": registry["template_trust_graph"],
        }
    )
    report["ok"] = report["hash_match"] and report["bytes_match"]
    if not report["ok"]:
        report["error"] = "rebuild_mismatch"
    return report


def explain_payload(payload_path: Path) -> dict[str, Any]:
    """Decoded human-oriented summary of a payload."""
    registry = decode_registry(_payload_cbor(payload_path))
    round_ref = registry["genesis_round"]
    return {
        "chain_id": registry["chain_id"],
        "fork_network": round_ref["fork_network"],
        "round_number": round_ref["round_number"],
        "bundle_cid": round_ref["bundle_cid"],
        "entries": len(registry["entries"]),
        "cutoff": registry["entries"][0]["cutoff"],
        "score_range": [
            min(entry["final_score"] for entry in registry["entries"]),
            max(entry["final_score"] for entry in registry["entries"]),
        ],
        "template_trust_graph": registry["template_trust_graph"],
        "proposed_registry_hash_hex": registry_hash(registry).hex(),
    }


# ---------------------------------------------------------------------------
# CLI
# ---------------------------------------------------------------------------


def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(
        prog="postfiat_rpc.genesis_registry", description=__doc__
    )
    commands = parser.add_subparsers(dest="command", required=True)

    def common(sub: argparse.ArgumentParser, with_inputs: bool) -> None:
        if with_inputs:
            sub.add_argument("--round-dir", type=Path, required=True)
            sub.add_argument("--receipts", type=Path, required=True)
            sub.add_argument("--rounds-manifest", type=Path, default=None)
            sub.add_argument("--chain-id", default=DEFAULT_CHAIN_ID)
        sub.add_argument("--json", action="store_true", help="machine-readable output")

    build_cmd = commands.add_parser("build", help="build a proposed registry")
    common(build_cmd, True)
    build_cmd.add_argument("--output", type=Path, default=None)

    verify_cmd = commands.add_parser("verify", help="verify a payload against inputs")
    verify_cmd.add_argument("--payload", type=Path, required=True)
    common(verify_cmd, True)

    explain_cmd = commands.add_parser("explain", help="summarize a payload")
    explain_cmd.add_argument("--payload", type=Path, required=True)
    common(explain_cmd, False)
    return parser


def main(argv: Sequence[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    if args.command == "build":
        result = build_registry(args.round_dir, args.receipts, args.rounds_manifest, args.chain_id)
        if args.output:
            args.output.write_text(json.dumps(result, indent=1) + "\n")
        if args.json:
            print(json.dumps(result if not args.output else {k: result[k] for k in
                  ("round", "domain", "proposed_registry_hash_hex")}, indent=1))
        else:
            print(f"round: {result['round']}")
            print(f"entries: {len(result['registry']['entries'])}")
            print(f"trust graph: {result['registry']['template_trust_graph']}")
            print(f"proposed_registry_hash: {result['proposed_registry_hash_hex']}")
        return 0
    if args.command == "verify":
        report = verify_payload(
            args.payload, args.round_dir, args.receipts, args.rounds_manifest, args.chain_id
        )
        if args.json:
            print(json.dumps(report, indent=1))
        else:
            status = "OK" if report["ok"] else f"FAILED ({report.get('error')})"
            print(f"verify {args.payload}: {status}")
            if "proposed_registry_hash_hex" in report:
                print(f"payload hash: {report['proposed_registry_hash_hex']}")
        return 0 if report["ok"] else 1
    report = explain_payload(args.payload)
    if args.json:
        print(json.dumps(report, indent=1))
    else:
        for key, value in report.items():
            print(f"{key}: {value}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
