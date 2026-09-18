"""Offline Z3 cycle packet builder and evidence verifier.

A layout has ten named sections. The manifest section holds public metadata;
every other section maps artifact roles to packet-relative paths. Each record
is a public JSON projection of frozen readbacks, receipts and verified finality
reports. A projection may use {"$ref": role, "pointer": "/json/pointer"} to read
an original artifact without editing it. All originals must be in the layout.

This audits retained evidence and integer conservation; it does not execute a
proof system, authenticate an RPC server, or confer operator authorization.
Finality/proof verification reports must come from the qualified lineage.
No signer files are opened, copied, embedded, or needed.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any

SCHEMA = "postfiat.z3.cycle.v1"
VERDICT_SCHEMA = "postfiat.z3.cycle_verdict.v1"
ATTEMPT_SCHEMA = "postfiat.z3.attempt.v1"
ATTEMPT_REASONS = {
    "unsafe_preflight", "rejected_transaction", "proof_mismatch",
    "reconciliation_mismatch", "validator_disagreement", "missing_artifact",
    "timeout", "manual_intervention_after_submission", "environmental_interruption",
}
SECTIONS = (
    "manifest", "preflight", "deposit", "ingress", "subscription",
    "entitlement_release", "nav_route_epoch", "redemption", "egress",
    "final_convergence",
)
REQUIRED = {
    "preflight": ("record", "validators"),
    "deposit": ("record", "receipt", "finality_input"),
    "ingress": ("record", "witness", "proof", "public_values", "proof_report",
                "propose_receipt", "finalize_receipt", "claim_receipt", "finality"),
    "subscription": ("record", "quote", "reserve_receipt", "subscribe_receipt", "finality"),
    "entitlement_release": ("record", "release_receipt", "finality"),
    "nav_route_epoch": ("record", "reserve_packet", "proof", "public_values",
                        "proof_report", "nav_submit_receipt", "nav_receipt",
                        "pause_receipt", "route_receipt", "resume_receipt", "finality"),
    "redemption": ("record", "quote", "redeem_receipt", "finality"),
    "egress": ("record", "after_burn", "after_release", "burn_receipt", "settle_receipt",
               "finality", "withdrawal_packet",
               "witness", "proof", "public_values", "proof_report", "receipt", "replay"),
    "final_convergence": ("record", "validators", "conservation"),
}
PFTL_RECEIPTS = {
    "ingress": {"propose_receipt": "vault_bridge_deposit_propose",
                "finalize_receipt": "vault_bridge_deposit_finalize",
                "claim_receipt": "vault_bridge_deposit_claim"},
    "subscription": {"reserve_receipt": "pftl_uniswap_order_reserve",
                     "subscribe_receipt": "pftl_uniswap_primary_subscribe_v2"},
    "entitlement_release": {"release_receipt": "pftl_uniswap_order_release"},
    "nav_route_epoch": {"nav_submit_receipt": "nav_reserve_submit",
                        "nav_receipt": "nav_epoch_finalize",
                        "pause_receipt": "pftl_uniswap_route_pause",
                        "route_receipt": "pftl_uniswap_route_epoch_advance",
                        "resume_receipt": "pftl_uniswap_route_pause"},
    "redemption": {"redeem_receipt": "pftl_uniswap_primary_redeem"},
    "egress": {"burn_receipt": "vault_bridge_burn_to_redeem",
               "settle_receipt": "vault_bridge_redeem_settle"},
}
STATE_FIELDS = (
    "native_supply", "native_wallet", "external_native_supply",
    "family_supply", "series_supply", "pfusdc_wallet",
    "settlement_reserve", "source_principal", "source_spread", "source_escrow",
    "non_nav_spread", "entitlement_atoms", "entitlement_count", "reservations",
    "pending_orders", "pending_egress", "source_vault", "arc_wallet_wei",
    "issued_total", "counted_total", "redeemed_total", "uncredited_deposits", "released_unsettled",
)
HASH = re.compile(r"^(?:0x)?[0-9a-fA-F]{64}(?:[0-9a-fA-F]{32})?$")
ADDRESS = re.compile(r"^0x[0-9a-fA-F]{40}$")
ACCOUNT = re.compile(r"^pf[0-9a-f]{40}$")
SECRET = re.compile(
    r"private.?key|secret.?key|password|mnemonic|seed.?phrase|keystore|"
    r"ciphertext|(^|[_-])crypto($|[_-])|key.?file", re.I
)
IDENTITY_FIELDS = (
    "pftl_chain_id", "source_chain_id", "source_vault_address",
    "source_anchor_address", "source_token_address", "source_route_id",
    "source_route_epoch", "source_profile_hash", "source_bucket_id",
    "route_id", "native_nav_asset_id", "settlement_asset_id",
    "settlement_source_asset_id", "protocol_version",
    "vault_code_hash", "anchor_code_hash",
)


class CycleError(ValueError):
    """Public, fail-closed diagnostic; never includes artifact contents."""


def require(condition: bool, message: str) -> None:
    if not condition:
        raise CycleError(message)


def uint(value: Any, label: str, *, positive: bool = False) -> int:
    require(type(value) is int and 0 <= value < 2**256, f"invalid integer: {label}")
    require(not positive or value > 0, f"zero amount: {label}")
    return value


def same(actual: Any, expected: Any, label: str) -> None:
    require(actual == expected, f"mismatch: {label}")


def public(value: Any) -> None:
    """Reject sensitive field names without including their values in errors."""
    if isinstance(value, dict):
        for key, item in value.items():
            require(isinstance(key, str) and not SECRET.search(key),
                    "forbidden signer/secret field in public artifact")
            public(item)
    elif isinstance(value, list):
        for item in value:
            public(item)
    elif isinstance(value, str):
        require("PRIVATE KEY" not in value and not SECRET.search(value),
                "forbidden signer/secret text in public artifact")


def public_path(root: Path, name: str) -> Path:
    require(isinstance(name, str) and bool(name), "invalid artifact path")
    relative = Path(name)
    require(not relative.is_absolute() and ".." not in relative.parts,
            "artifact must stay inside packet directory")
    require(not SECRET.search(name), "signer files are not evidence")
    path = root
    for part in relative.parts:
        path = path / part
        require(not path.is_symlink(), "artifact symlinks are forbidden")
    require(path.resolve().is_relative_to(root.resolve()), "artifact escapes packet")
    return path


def read_json(path: Path) -> Any:
    require(not SECRET.search(str(path)) and not path.is_symlink(),
            "signer paths and symlinks are not public JSON inputs")
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, ValueError):
        raise CycleError("missing or invalid public JSON artifact") from None
    public(value)
    return value


def artifact_bytes(root: Path, name: str) -> bytes:
    path = public_path(root, name)
    require(path.suffix in {".json", ".bin"}, "only public JSON/proof binaries are evidence")
    try:
        raw = path.read_bytes()
    except OSError:
        raise CycleError("missing artifact") from None
    require(bool(raw), "empty artifact")
    if path.suffix == ".json":
        try:
            public(json.loads(raw))
        except (ValueError, UnicodeError):
            raise CycleError("invalid or non-public JSON artifact") from None
    return raw


def write_new(path: Path, value: Any) -> None:
    public(value)
    require(not path.is_symlink(), "output symlink forbidden")
    try:
        with path.open("x", encoding="utf-8") as stream:
            json.dump(value, stream, indent=2, sort_keys=True)
            stream.write("\n")
    except FileExistsError:
        raise CycleError("refusing to overwrite output") from None


def utc(value: Any) -> datetime:
    require(isinstance(value, str) and value.endswith("Z"), "UTC timestamp must end in Z")
    try:
        return datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        raise CycleError("invalid UTC timestamp") from None


def validate_metadata(meta: dict, *, skeleton: bool = False) -> None:
    public(meta)
    uint(meta["cycle_number"], "cycle number", positive=True)
    uint(meta["amount_atoms"], "cycle amount", positive=True)
    require(bool(meta["release_id"]), "missing release ID")
    require(re.fullmatch(r"[0-9a-f]{40}", meta["source_commit"]) is not None,
            "full source commit required")
    require(re.fullmatch(r"[0-9a-f]{64}", meta["binary_sha256"]) is not None,
            "binary SHA-256 required")
    start = utc(meta["utc_start"])
    if not skeleton:
        require(utc(meta["utc_end"]) >= start, "cycle ends before it starts")
    ids = meta["identities"]
    for field in IDENTITY_FIELDS:
        require(field in ids and ids[field] not in ("", None), f"missing identity: {field}")
    same(ids["source_chain_id"], 5042002, "Arc testnet chain")
    same(ids["source_token_address"].lower(),
         "0x3600000000000000000000000000000000000000", "Arc USDC")
    uint(ids["source_route_epoch"], "source route epoch", positive=True)
    for field in ("source_vault_address", "source_anchor_address", "source_token_address"):
        require(ADDRESS.fullmatch(ids[field]) is not None, f"full address required: {field}")
    for field in ("source_profile_hash", "source_bucket_id", "native_nav_asset_id",
                  "settlement_asset_id", "settlement_source_asset_id",
                  "vault_code_hash", "anchor_code_hash"):
        width = 64 if field in ("vault_code_hash", "anchor_code_hash") else 96
        require(re.fullmatch(r"(?:0x)?[0-9a-fA-F]{" + str(width) + "}", ids[field]) is not None,
                f"full hash required: {field}")
    for field in ("primary_route", "source_profile", "verifier_policy", "nav_valuation"):
        require(HASH.fullmatch(meta["policy_hashes"][field]) is not None,
                f"missing policy hash: {field}")
    same(meta["policy_hashes"]["source_profile"], ids["source_profile_hash"], "source policy")
    for field in ("ingress", "egress", "nav"):
        require(HASH.fullmatch(meta["proof_keys"][field]) is not None,
                f"missing proof key: {field}")
    require(ADDRESS.fullmatch(meta["accounts"]["arc_wallet"]) is not None,
            "full Arc wallet address required")
    for field in ("owner", "proposer", "finalizer", "route_operator"):
        require(ACCOUNT.fullmatch(meta["accounts"][field]) is not None,
                f"full PFTL account required: {field}")


def build_manifest(layout: dict, root: Path, output: Path, *, skeleton: bool = False) -> dict:
    require(not output.exists(), "refusing to overwrite output")
    same(set(layout), set(SECTIONS), "ten packet sections")
    validate_metadata(layout["manifest"], skeleton=skeleton)
    packet = {"schema": SCHEMA, "complete": not skeleton, "manifest": layout["manifest"]}
    for section, roles in REQUIRED.items():
        paths = layout[section]
        require(set(roles) <= set(paths), f"missing artifact role in {section}")
        packet[section] = {}
        for role, name in paths.items():
            public_path(root, name)
            digest = None if skeleton else hashlib.sha256(artifact_bytes(root, name)).hexdigest()
            packet[section][role] = {"path": name, "sha256": digest}
    same(output.parent.resolve(), root.resolve(), "manifest packet directory")
    write_new(output, packet)
    return packet


def attempt_outcome(attempt: dict) -> dict:
    """Classify a retained interruption/rejection; never qualify a clean cycle."""
    require(attempt["reason"] in ATTEMPT_REASONS, "unknown attempt reason")
    require(type(attempt["submission_started"]) is bool, "submission flag must be boolean")
    prior = uint(attempt["prior_consecutive"], "prior consecutive count")
    require(isinstance(attempt["step"], str) and bool(attempt["step"]), "missing attempt step")
    excluded = (attempt["reason"] == "environmental_interruption"
                and not attempt["submission_started"])
    return {
        "status": "not_a_cycle" if excluded else "unclean",
        "counts_as_cycle": not excluded,
        "pause_campaign": not excluded,
        "reset_required_after_correction": not excluded,
        "consecutive_after_correction": prior if excluded else 0,
    }


def build_attempt_manifest(meta: dict, root: Path, output: Path, attempt: dict,
                           evidence: dict[str, str]) -> dict:
    """Seal an operator projection of an incomplete attempt and its originals.

    This does not infer submission from process exit status. The caller must
    retain the checkpoint, request identity/marker and observed failure, and
    conservatively mark uncertain publication as submission_started.
    """
    validate_metadata(meta)
    require("observation" in evidence, "attempt observation required")
    packet = {"schema": ATTEMPT_SCHEMA, "manifest": meta,
              "attempt": {**attempt, **attempt_outcome(attempt)}, "artifacts": {}}
    for role, name in evidence.items():
        packet["artifacts"][role] = {
            "path": name, "sha256": hashlib.sha256(artifact_bytes(root, name)).hexdigest(),
        }
    same(output.parent.resolve(), root.resolve(), "manifest packet directory")
    write_new(output, packet)
    return packet


def verify_attempt(packet: dict, root: Path) -> dict:
    same(set(packet), {"schema", "manifest", "attempt", "artifacts"}, "attempt sections")
    validate_metadata(packet["manifest"])
    attempt = packet["attempt"]
    outcome = attempt_outcome(attempt)
    for field, expected in outcome.items():
        same(attempt[field], expected, f"attempt outcome/{field}")
    require("observation" in packet["artifacts"], "attempt observation required")
    for role, ref in packet["artifacts"].items():
        same(hashlib.sha256(artifact_bytes(root, ref["path"])).hexdigest(),
             ref["sha256"], f"attempt artifact hash/{role}")
    observation_ref = packet["artifacts"]["observation"]
    require(observation_ref["path"].endswith(".json"), "attempt observation must be JSON")
    observation = read_json(public_path(root, observation_ref["path"]))
    for field in ("step", "reason", "submission_started"):
        same(observation[field], attempt[field], f"attempt observation/{field}")
    if outcome["status"] == "not_a_cycle":
        # The ordered wrapper starts preflight, then deposit. A later
        # checkpoint or any value-step marker makes publication possible;
        # a caller cannot obtain the exception by changing only a boolean.
        require("checkpoint" in packet["artifacts"], "pre-submission checkpoint required")
        ref = packet["artifacts"]["checkpoint"]
        checkpoint = read_json(public_path(root, ref["path"]))
        same(checkpoint["ready_for"], attempt["step"], "interrupted checkpoint step")
        require(checkpoint["ready_for"] in ("preflight", "arc-deposit")
                and all(row["step"] == "preflight" for row in checkpoint["completed"])
                and len(checkpoint["completed"]) <= 1,
                "submission evidence prevents environmental exemption")
        if "marker" in packet["artifacts"]:
            ref = packet["artifacts"]["marker"]
            marker = read_json(public_path(root, ref["path"]))
            same(marker["step"], attempt["step"], "interrupted marker step")
            require(marker["kind"] == "prepare",
                    "uncertain publication prevents environmental exemption")
    return {"schema": VERDICT_SCHEMA,
            "verdict": "NOT_A_CYCLE" if outcome["status"] == "not_a_cycle" else "FAIL",
            "reason": attempt["reason"], **outcome,
            "scope": "offline retained attempt; no clean cycle or recovery authorization"}


def project(value: Any, artifacts: dict, depth: int = 0) -> Any:
    require(depth < 64, "projection nesting limit")
    if isinstance(value, dict):
        if "$ref" in value:
            same(set(value), {"$ref", "pointer"}, "artifact projection fields")
            result = artifacts[value["$ref"]]
            pointer = value["pointer"]
            require(isinstance(pointer, str) and (not pointer or pointer.startswith("/")),
                    "invalid JSON pointer")
            for part in pointer.split("/")[1:]:
                part = part.replace("~1", "/").replace("~0", "~")
                result = result[int(part)] if isinstance(result, list) else result[part]
            return result
        return {key: project(item, artifacts, depth + 1) for key, item in value.items()}
    if isinstance(value, list):
        return [project(item, artifacts, depth + 1) for item in value]
    return value


def load_section(packet: dict, root: Path, section: str) -> tuple[dict, dict]:
    refs = packet[section]
    require(set(REQUIRED[section]) <= set(refs), f"missing artifact role in {section}")
    artifacts = {}
    for role, ref in refs.items():
        raw = artifact_bytes(root, ref["path"])
        same(hashlib.sha256(raw).hexdigest(), ref["sha256"], f"artifact hash {section}/{role}")
        artifacts[role] = json.loads(raw) if ref["path"].endswith(".json") else raw
    raw_record = artifacts["record"]
    # Required receipts cannot be replaced by an inline success assertion while
    # the original rejected receipt merely sits unused in the artifact inventory.
    def bound(value: Any, role: str) -> None:
        require(isinstance(value, dict) and value.get("$ref") == role,
                f"record must reference original {section}/{role}")
    for role in PFTL_RECEIPTS.get(section, {}):
        bound(raw_record["receipts"][role]["receipt"], role)
        bound(raw_record["receipts"][role]["finality"], "finality")
    if section in ("ingress", "nav_route_epoch", "egress"):
        bound(raw_record["proof"], "proof_report")
    if section in ("deposit", "egress"):
        bound(raw_record["arc_receipt"], "receipt")
    if section in ("subscription", "redemption"):
        bound(raw_record["quote"], "quote")
    if section == "egress":
        bound(raw_record["replay"], "replay")
    return project(raw_record, artifacts), artifacts


def validate_state(state: dict) -> None:
    same(set(state), set(STATE_FIELDS), "accounting state fields")
    for field in STATE_FIELDS:
        uint(state[field], field)
    require(state["source_principal"] <= state["settlement_reserve"],
            "source principal exceeds aggregate reserve")
    same(state["series_supply"], state["counted_total"] - state["redeemed_total"],
         "source issued/count/redeemed identity")
    require(state["family_supply"] >= state["series_supply"], "series exceeds family supply")
    same(state["source_vault"], state["series_supply"] + state["uncredited_deposits"]
         + state["pending_egress"] - state["released_unsettled"],
         "source vault = live supply + uncredited + burned unsettled - released unsettled")
    require(state["native_supply"] >= state["native_wallet"] + state["external_native_supply"],
            "native wallet/external supply exceeds global supply")


def transition(before: dict, after: dict, changes: dict, label: str) -> None:
    for field in STATE_FIELDS:
        same(after[field] - before[field], changes.get(field, 0), f"{label}/{field}")


def convergence(rows: list, record: dict, expected_queues: dict | None = None) -> None:
    require(isinstance(rows, list) and len(rows) == 6, "six validators required")
    require(len({row["validator_id"] for row in rows}) == 6, "duplicate validator")
    required = ("height", "block_id", "state_root", "route_state_hash",
                "nav_state_hash", "asset_state_hash")
    for row in rows:
        for field in required:
            require(row[field] not in ("", None), f"missing validator {field}")
            same(row[field], rows[0][field], f"validator convergence/{field}")
        same(row["queues"], expected_queues or {"mempool": 0, "reservations": 0, "egress": 0},
             "validator queues")
    for field in required[:3]:
        same(record["finalized"][field], rows[0][field], f"snapshot finalized/{field}")


def accepted_receipts(section: str, record: dict, seen: set[str]) -> None:
    expected = PFTL_RECEIPTS.get(section, {})
    receipts = record.get("receipts", {})
    same(set(receipts), set(expected), f"required receipts/{section}")
    for role, kind in expected.items():
        item = receipts[role]
        receipt, finality = item["receipt"], item["finality"]
        same(receipt["accepted"], True, f"accepted receipt/{role}")
        require(type(receipt["accepted"]) is bool, "receipt acceptance must be boolean")
        same(receipt["transaction_kind"], kind, f"receipt kind/{role}")
        tx = receipt["tx_id"]
        require(HASH.fullmatch(tx) is not None, "full receipt transaction ID required")
        require(tx not in seen, "duplicate/replayed transaction ID")
        seen.add(tx)
        same(finality["verified"], True, f"verified finality/{role}")
        require(type(finality["verified"]) is bool, "finality result must be boolean")
        same(receipt["block_id"], finality["block_id"], f"receipt finality block/{role}")
        same(receipt["height"], finality["height"], f"receipt finality height/{role}")
        require(tx in finality["transaction_ids"], "receipt absent from finalized block")
        require(receipt["height"] <= record["finalized"]["height"],
                "receipt newer than readback")
        require(bool(finality["certificate_hash"]), "missing finality certificate hash")


def verify_proof(section: str, packet: dict, record: dict, key: str) -> None:
    report = record["proof"]
    same(report["verified"], True, f"proof verification/{section}")
    require(type(report["verified"]) is bool, "proof verification must be boolean")
    same(report["program_vkey"], packet["manifest"]["proof_keys"][key], "proof key")
    for role in ("proof", "public_values"):
        same(report[role + "_sha256"], packet[section][role]["sha256"],
             f"proof binding/{section}/{role}")
    require(report["valid_from_height"] <= record["finalized"]["height"]
            <= report["valid_until_height"], "stale proof")


def arc_receipt(record: dict, meta: dict, *, release: bool, seen: set[str]) -> None:
    receipt = record["arc_receipt"]
    same(receipt["status"], "0x1", "Arc status-1 receipt")
    same(receipt["from"].lower(), meta["accounts"]["arc_wallet"].lower(), "Arc sender")
    same(receipt["to"].lower(), meta["identities"]["source_vault_address"].lower(), "Arc vault")
    tx = receipt["transactionHash"]
    require(re.fullmatch(r"0x[0-9a-fA-F]{64}", tx) is not None, "full Arc transaction hash")
    require(tx not in seen, "duplicate Arc transaction")
    seen.add(tx)
    same(record["arc_finality"]["verified"], True, "Arc finality")
    same(record["arc_finality"]["block_hash"], receipt["blockHash"], "Arc finality block")
    same(record["arc_finality"]["transaction_hash"], tx, "Arc finality transaction")
    uint(record["log_index"], "Arc log index")
    require(HASH.fullmatch(record["withdrawal_id" if release else "deposit_id"]) is not None,
            "full deposit/withdrawal identity required")


def _verify(packet: dict, root: Path) -> dict:
    if packet.get("schema") == ATTEMPT_SCHEMA:
        return verify_attempt(packet, root)
    same(packet["schema"], SCHEMA, "manifest schema")
    same(packet["complete"], True, "complete packet (skeletons cannot pass)")
    same(set(packet), {"schema", "complete", *SECTIONS}, "packet sections")
    meta = packet["manifest"]
    validate_metadata(meta)
    records, artifacts = {}, {}
    for section in REQUIRED:
        record, raw = load_section(packet, root, section)
        same(record["identities"], meta["identities"], f"identity binding/{section}")
        same(record["accounts"], meta["accounts"], f"account binding/{section}")
        same(record["policy_hashes"], meta["policy_hashes"], f"policy binding/{section}")
        validate_state(record["state"])
        uint(record["finalized"]["height"], "finalized height")
        require(HASH.fullmatch(record["finalized"]["block_id"]) is not None, "finalized block ID")
        require(HASH.fullmatch(record["finalized"]["state_root"]) is not None, "finalized state root")
        records[section], artifacts[section] = record, raw
    for section in ("preflight", "final_convergence"):
        convergence(project(artifacts[section]["validators"], artifacts[section]), records[section])
    seen: set[str] = set()
    for section, record in records.items():
        accepted_receipts(section, record, seen)
    for section, key in (("ingress", "ingress"), ("nav_route_epoch", "nav"), ("egress", "egress")):
        verify_proof(section, packet, records[section], key)

    states = {name: record["state"] for name, record in records.items()}
    p, d, i, s, e, n, r, b, f = [states[name] for name in SECTIONS[1:]]
    for field in ("entitlement_atoms", "entitlement_count", "reservations",
                  "pending_orders", "pending_egress", "uncredited_deposits", "source_escrow",
                  "released_unsettled"):
        same(p[field], 0, f"preflight empty/{field}")
        same(f[field], 0, f"final empty/{field}")
    amount = meta["amount_atoms"]
    deposit, egress = records["deposit"], records["egress"]
    arc_receipt(deposit, meta, release=False, seen=seen)
    arc_receipt(egress, meta, release=True, seen=seen)
    same(deposit["amount_atoms"], amount, "deposit amount")
    same(records["ingress"]["deposit_id"], deposit["deposit_id"], "same-cycle deposit")
    same(records["ingress"]["mint_atoms"], amount, "mint amount")
    # Arc gas is USDC too; native gas accounting uses 18 decimals. Retain exact
    # wei and convert the six-decimal token amount without rounding gas away.
    deposit_gas = uint(deposit["gas_fee_wei"], "deposit gas")
    transition(p, d, {"source_vault": amount, "uncredited_deposits": amount,
                      "arc_wallet_wei": -amount * 10**12 - deposit_gas}, "deposit")
    transition(d, i, {"family_supply": amount, "series_supply": amount,
                      "pfusdc_wallet": amount, "issued_total": amount,
                      "counted_total": amount, "uncredited_deposits": -amount}, "ingress")
    quote = records["subscription"]["quote"]
    mint = uint(quote["mint_amount_atoms"], "mint amount", positive=True)
    base = uint(quote["base_value_atoms"], "issue base", positive=True)
    spread = uint(quote["issue_spread_atoms"], "issue spread")
    spend = uint(quote["settlement_value_atoms"], "subscription spend", positive=True)
    same(spend, base + spread, "issue base plus spread")
    issue_nav = uint(quote["nav_per_unit_usd_1e8"], "issue NAV", positive=True)
    issue_bps = uint(quote["issue_multiplier_bps"], "issue multiplier", positive=True)
    require(issue_bps >= 10000, "issue multiplier below par")
    same(base, (mint * issue_nav + 10**8 - 1) // 10**8, "issue NAV rounding")
    same(spend, (base * issue_bps + 9999) // 10000, "issue spread rounding")
    require(spend <= amount, "subscription exceeds same-cycle deposit")
    transition(i, s, {"native_supply": mint, "native_wallet": mint,
                      "pfusdc_wallet": -spend, "settlement_reserve": base,
                      "source_principal": base, "source_spread": spread,
                      "non_nav_spread": spread, "entitlement_atoms": mint,
                      "entitlement_count": 1}, "subscription")
    transition(s, e, {"entitlement_atoms": -mint, "entitlement_count": -1}, "entitlement release")
    transition(e, n, {}, "NAV/route maintenance")
    nav = records["nav_route_epoch"]["nav"]
    old = records["preflight"]["nav"]
    require(nav["epoch"] > old["epoch"] and nav["route_epoch"] > old["route_epoch"],
            "fresh NAV and route epoch required")
    require(nav["packet_hash"] != old["packet_hash"], "stale NAV packet")
    same(nav["overlay_reserve_atoms"], n["settlement_reserve"], "overlay reserve")
    same(nav["overlay_source_occurrences"], 1, "reserve counted once")
    same(nav["external_source_vault_atoms"], 0, "source vault double counted externally")
    same(nav["verified_net_assets_usd_1e8"],
         uint(nav["external_net_assets_usd_1e8"], "external NAV") + n["settlement_reserve"] * 100,
         "composite NAV overlay")
    same(nav["circulating_supply_atoms"], n["native_supply"], "fresh NAV supply")
    redeem = records["redemption"]["quote"]
    retired = uint(redeem["nav_amount_atoms"], "retired amount", positive=True)
    redeem_base = uint(redeem["base_value_atoms"], "redeem base", positive=True)
    output = uint(redeem["settlement_output_atoms"], "redemption output", positive=True)
    redeem_spread = uint(redeem["redemption_spread_atoms"], "redeem spread")
    same(redeem_base, output + redeem_spread, "redemption base = output + spread")
    redeem_nav = uint(redeem["nav_per_unit_usd_1e8"], "redeem NAV", positive=True)
    redeem_bps = uint(redeem["redeem_multiplier_bps"], "redeem multiplier", positive=True)
    require(redeem_bps <= 10000, "redeem multiplier above par")
    same(redeem_nav, nav["nav_per_unit_usd_1e8"], "fresh NAV pricing")
    same(redeem_base, (retired * redeem_nav + 10**8 - 1) // 10**8, "redeem NAV rounding")
    same(output, redeem_base * redeem_bps // 10000, "redemption spread rounding")
    require(retired <= mint and redeem_base <= min(base, n["source_principal"]),
            "insufficient same-cycle/source capacity")
    same(redeem["pricing_nav_epoch"], nav["epoch"], "redemption NAV epoch")
    same(redeem["route_epoch"], nav["route_epoch"], "redemption route epoch")
    same(redeem["pricing_reserve_packet_hash"], nav["packet_hash"], "redemption reserve packet")
    transition(n, r, {"native_supply": -retired, "native_wallet": -retired,
                      "pfusdc_wallet": output, "settlement_reserve": -redeem_base,
                      "source_principal": -redeem_base, "source_spread": redeem_spread,
                      "non_nav_spread": redeem_spread}, "redemption")
    same(egress["amount_atoms"], output, "burn/release redeemed pfUSDC")
    require(HASH.fullmatch(egress["nullifier"]) is not None, "withdrawal nullifier required")
    same(egress["replay"]["rejected"], True, "release replay rejected")
    same(egress["replay"]["nullifier"], egress["nullifier"], "replay nullifier")
    same(egress["replay"]["state_unchanged"], True, "replay changed state")
    release_gas = uint(egress["gas_fee_wei"], "release gas")
    burned = artifacts["egress"]["after_burn"]
    released = artifacts["egress"]["after_release"]
    validate_state(burned)
    validate_state(released)
    transition(r, burned, {"family_supply": -output, "series_supply": -output,
                           "pfusdc_wallet": -output, "redeemed_total": output,
                           "pending_egress": output}, "burn")
    transition(burned, released, {"source_vault": -output, "released_unsettled": output,
                                  "arc_wallet_wei": output * 10**12 - release_gas}, "Arc release")
    transition(released, b, {"pending_egress": -output, "released_unsettled": -output},
               "egress settlement")
    transition(r, b, {"family_supply": -output, "series_supply": -output,
                      "pfusdc_wallet": -output, "redeemed_total": output,
                      "source_vault": -output,
                      "arc_wallet_wei": output * 10**12 - release_gas}, "burn/release")
    transition(b, f, {}, "final convergence")
    audit = artifacts["final_convergence"]["conservation"]
    for field in ("native_supply", "family_supply", "series_supply", "issued_total",
                  "counted_total", "redeemed_total", "settlement_reserve", "source_vault",
                  "entitlement_atoms", "pending_orders", "pending_egress"):
        same(audit[field], f[field], f"final independent readback/{field}")
    last_height = -1
    for record in records.values():
        height = record["finalized"]["height"]
        require(height >= last_height, "out-of-order finalized evidence")
        last_height = height
    same(f["native_supply"] - p["native_supply"], mint - retired, "net native supply")
    same(f["settlement_reserve"] - p["settlement_reserve"], base - redeem_base, "net reserve")
    same(f["series_supply"] - p["series_supply"], amount - output, "net source supply")
    same(f["pfusdc_wallet"] - p["pfusdc_wallet"], amount - spend, "net source wallet")
    same(f["arc_wallet_wei"] - p["arc_wallet_wei"],
         (output - amount) * 10**12 - deposit_gas - release_gas, "net Arc wallet with gas")
    return {"schema": VERDICT_SCHEMA, "verdict": "PASS", "cycle_number": meta["cycle_number"],
            "artifact_count": sum(len(packet[name]) for name in REQUIRED),
            "receipt_count": len(seen), "retained_native_atoms": mint - retired,
            "retained_reserve_atoms": base - redeem_base,
            "checks": ["artifact_sha256", "required_receipts_and_finality", "identity_binding",
                       "proof_hashes_keys_freshness", "six_validator_convergence",
                       "exact_supply_balance_reserve_deltas", "reserve_counted_once",
                       "zero_entitlement_and_pending_state", "release_replay_rejected"],
            "scope": "offline evidence audit; no new cryptographic verification or authorization"}


def verify_manifest(path: Path) -> dict:
    try:
        return _verify(read_json(path), path.parent)
    except (CycleError, KeyError, TypeError, ValueError, IndexError, AttributeError) as error:
        message = str(error) if isinstance(error, CycleError) else "missing or malformed packet field"
        return {"schema": VERDICT_SCHEMA, "verdict": "FAIL", "error": message}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    build = sub.add_parser("build")
    build.add_argument("--layout", type=Path, required=True)
    build.add_argument("--output", type=Path, required=True)
    build.add_argument("--skeleton", action="store_true")
    verify = sub.add_parser("verify")
    verify.add_argument("manifest", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "build":
            build_manifest(read_json(args.layout), args.output.parent, args.output,
                           skeleton=args.skeleton)
            result = {"verdict": "SKELETON" if args.skeleton else "BUILT"}
        else:
            result = verify_manifest(args.manifest)
    except (CycleError, KeyError, TypeError, ValueError):
        result = {"verdict": "FAIL", "error": "invalid layout or output; build refused"}
    print(json.dumps(result, sort_keys=True))
    # A retained interruption is a successful audit record, never a successful
    # cycle verification. Shell consumers must not count NOT_A_CYCLE as PASS.
    return 1 if result["verdict"] == "FAIL" or (
        args.command == "verify" and result["verdict"] != "PASS") else 0


if __name__ == "__main__":
    raise SystemExit(main())
