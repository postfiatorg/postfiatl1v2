"""Synthetic complete packet: no RPC, wallet, key or proof generation."""
from __future__ import annotations

import copy
import hashlib
import json
from pathlib import Path

import pytest

from postfiat_rpc import z3_cycle as z3


def digest(value: str) -> str:
    return hashlib.sha256(value.encode()).hexdigest()


def metadata() -> dict:
    ids = {
        "pftl_chain_id": "postfiat-wan-devnet-2", "source_chain_id": 5042002,
        "source_vault_address": "0x160307f3efead79b6a3629c4b8d90e8301fc250f",
        "source_anchor_address": "0x92390d3a2102cb74e4746c05b4d91f61093475d0",
        "source_token_address": "0x3600000000000000000000000000000000000000",
        "source_route_id": "pfusdc-arc-testnet-tier4-epoch9", "source_route_epoch": 9,
        "source_profile_hash": "f7ce6d3cce3bd058a218db6bd829b01be13c576a2270aed362052d12654fc7a911a8423ee2d961ca45dbf72c08df6ae2",
        "source_bucket_id": "fcc209605f8cfda895acbf78047f83f97b0bc1cee3927582fb262efb46e7d136b098183d5f83e19a82d34c218bab67a7",
        "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
        "native_nav_asset_id": "521c6c630bb48d4a37ab4a7bd4900dd2caa2d9e99499e452da3c7ce75b3d74b62d20e18555642bec32174498cbee5e2c",
        "settlement_asset_id": "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b",
        "settlement_source_asset_id": "3923511d5be0557a61051e099b606d3decc11a5ba274c7d551168735accad8ed18d89c9200efc4bfbbbab6e85d4c173f",
        "protocol_version": "synthetic-v1", "vault_code_hash": digest("vault"),
        "anchor_code_hash": digest("anchor"),
    }
    return {
        "cycle_number": 1, "utc_start": "2026-09-17T00:00:00Z",
        "utc_end": "2026-09-17T00:05:00Z", "release_id": "synthetic-offline",
        "source_commit": "1" * 40, "binary_sha256": digest("binary"),
        "amount_atoms": 1005, "identities": ids,
        "policy_hashes": {"primary_route": digest("primary"),
                          "source_profile": ids["source_profile_hash"],
                          "verifier_policy": digest("verifier"), "nav_valuation": digest("valuation")},
        "proof_keys": {key: digest(key) for key in ("ingress", "egress", "nav")},
        "accounts": {"arc_wallet": "0xC75Bf05Ce82d6f4b6139dd9446D6De5F5994a4CB",
                     **{key: "pf" + "1" * 40 for key in ("owner", "proposer", "finalizer", "route_operator")}},
    }


def ref(role: str, pointer: str = "") -> dict:
    return {"$ref": role, "pointer": pointer}


def save(root: Path, path: str, value: object) -> None:
    target = root / path
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(json.dumps(value), encoding="utf-8")


def packet_fixture(root: Path) -> dict:
    meta = metadata()
    state = dict.fromkeys(z3.STATE_FIELDS, 0)
    state.update(native_supply=10000, family_supply=20000, series_supply=20000,
                 source_vault=20000, counted_total=20000, issued_total=20000,
                 arc_wallet_wei=10**20)
    changes = {
        "preflight": {},
        "deposit": {"source_vault": 1005, "uncredited_deposits": 1005,
                    "arc_wallet_wei": -1005 * 10**12 - 17},
        "ingress": {"family_supply": 1005, "series_supply": 1005, "pfusdc_wallet": 1005,
                    "counted_total": 1005, "issued_total": 1005, "uncredited_deposits": -1005},
        "subscription": {"native_supply": 1000, "native_wallet": 1000, "pfusdc_wallet": -1005,
                         "settlement_reserve": 1000, "source_principal": 1000,
                         "source_spread": 5, "non_nav_spread": 5,
                         "entitlement_atoms": 1000, "entitlement_count": 1},
        "entitlement_release": {"entitlement_atoms": -1000, "entitlement_count": -1},
        "nav_route_epoch": {},
        "redemption": {"native_supply": -900, "native_wallet": -900, "pfusdc_wallet": 899,
                       "settlement_reserve": -900, "source_principal": -900,
                       "source_spread": 1, "non_nav_spread": 1},
        "egress": {"family_supply": -899, "series_supply": -899, "pfusdc_wallet": -899,
                   "redeemed_total": 899, "source_vault": -899,
                   "arc_wallet_wei": 899 * 10**12 - 23},
        "final_convergence": {},
    }
    layout = {"manifest": meta}
    for index, (section, roles) in enumerate(z3.REQUIRED.items()):
        layout[section] = {role: f"{section}/{role}.json" for role in roles}
        for field, delta in changes[section].items():
            state[field] += delta
        final = {"height": 100 + index, "block_id": digest(section), "state_root": digest(section + "state")}
        record = {"identities": meta["identities"], "accounts": meta["accounts"],
                  "policy_hashes": meta["policy_hashes"], "state": copy.deepcopy(state),
                  "finalized": final, "receipts": {}}
        values = {role: {"synthetic": True, "role": role} for role in roles}
        finalities = {}
        for role, kind in z3.PFTL_RECEIPTS.get(section, {}).items():
            tx = digest(section + role)
            values[role] = {"accepted": True, "transaction_kind": kind, "tx_id": tx,
                            "block_id": final["block_id"], "height": final["height"]}
            finalities[role] = {"verified": True, "block_id": final["block_id"],
                                "height": final["height"], "transaction_ids": [tx],
                                "certificate_hash": digest("certificate" + tx)}
            record["receipts"][role] = {"receipt": ref(role), "finality": ref("finality", "/" + role)}
        if finalities:
            values["finality"] = finalities
        if section in ("preflight", "final_convergence"):
            values["validators"] = [
                {"validator_id": f"validator-{i}", **final,
                 "route_state_hash": digest("route"), "nav_state_hash": digest("nav"),
                 "asset_state_hash": digest("asset"),
                 "queues": {"mempool": 0, "reservations": 0, "egress": 0}}
                for i in range(6)]
        if section in ("deposit", "egress"):
            tx = "0x" + digest(section)
            values["receipt"] = {"status": "0x1", "from": meta["accounts"]["arc_wallet"],
                                 "to": meta["identities"]["source_vault_address"],
                                 "transactionHash": tx, "blockHash": digest("arc" + section)}
            record.update(arc_receipt=ref("receipt"), log_index=65,
                          arc_finality={"verified": True, "block_hash": digest("arc" + section),
                                        "transaction_hash": tx},
                          amount_atoms=1005 if section == "deposit" else 899,
                          gas_fee_wei=17 if section == "deposit" else 23)
        if section in ("deposit", "ingress"):
            record["deposit_id"] = digest("deposit-id")
        if section == "ingress":
            record["mint_atoms"] = 1005
        if section == "subscription":
            values["quote"] = {"mint_amount_atoms": 1000, "base_value_atoms": 1000,
                               "issue_spread_atoms": 5, "settlement_value_atoms": 1005,
                               "nav_per_unit_usd_1e8": 10**8, "issue_multiplier_bps": 10050}
            record["quote"] = ref("quote")
        if section == "preflight":
            record["nav"] = {"epoch": 7, "route_epoch": 2, "packet_hash": digest("old")}
        if section == "nav_route_epoch":
            record["nav"] = {"epoch": 8, "route_epoch": 3, "packet_hash": digest("fresh"),
                             "overlay_reserve_atoms": 1000, "overlay_source_occurrences": 1,
                             "external_source_vault_atoms": 0,
                             "external_net_assets_usd_1e8": 1000000,
                             "verified_net_assets_usd_1e8": 1100000,
                             "circulating_supply_atoms": 11000, "nav_per_unit_usd_1e8": 10**8}
        if section == "redemption":
            values["quote"] = {"nav_amount_atoms": 900, "base_value_atoms": 900,
                               "settlement_output_atoms": 899, "redemption_spread_atoms": 1,
                               "pricing_nav_epoch": 8, "route_epoch": 3,
                               "pricing_reserve_packet_hash": digest("fresh"),
                               "nav_per_unit_usd_1e8": 10**8, "redeem_multiplier_bps": 9995}
            record["quote"] = ref("quote")
        if section == "egress":
            record.update(withdrawal_id=digest("withdrawal"), nullifier=digest("nullifier"),
                          replay=ref("replay"))
            values["replay"] = {"rejected": True, "nullifier": digest("nullifier"),
                                "state_unchanged": True}
        if section in ("ingress", "nav_route_epoch", "egress"):
            for role in ("proof", "public_values"):
                save(root, layout[section][role], values[role])
            key = "nav" if section == "nav_route_epoch" else section
            values["proof_report"] = {
                "verified": True, "program_vkey": meta["proof_keys"][key],
                "valid_from_height": 100, "valid_until_height": 200,
                **{role + "_sha256": hashlib.sha256((root / layout[section][role]).read_bytes()).hexdigest()
                   for role in ("proof", "public_values")}}
            record["proof"] = ref("proof_report")
        if section == "final_convergence":
            values["conservation"] = copy.deepcopy(state)
        values["record"] = record
        for role, value in values.items():
            save(root, layout[section][role], value)
    return layout


def build(root: Path, layout: dict) -> Path:
    output = root / "cycle.json"
    z3.build_manifest(layout, root, output)
    return output


def test_success_complete_synthetic_packet(tmp_path):
    verdict = z3.verify_manifest(build(tmp_path, packet_fixture(tmp_path)))
    assert verdict["verdict"] == "PASS", verdict
    assert verdict["retained_native_atoms"] == 100
    assert verdict["retained_reserve_atoms"] == 100
    assert verdict["receipt_count"] == 12


def test_missing_artifact(tmp_path):
    layout = packet_fixture(tmp_path)
    manifest = build(tmp_path, layout)
    (tmp_path / layout["egress"]["proof"]).unlink()
    assert z3.verify_manifest(manifest)["verdict"] == "FAIL"


def test_hash_mismatch(tmp_path):
    layout = packet_fixture(tmp_path)
    manifest = build(tmp_path, layout)
    save(tmp_path, layout["deposit"]["receipt"], {"changed": True})
    assert "artifact hash" in z3.verify_manifest(manifest)["error"]


def test_rejected_receipt(tmp_path):
    layout = packet_fixture(tmp_path)
    path = layout["ingress"]["claim_receipt"]
    receipt = json.loads((tmp_path / path).read_text())
    receipt["accepted"] = False
    save(tmp_path, path, receipt)
    assert "accepted receipt" in z3.verify_manifest(build(tmp_path, layout))["error"]


def test_conservation_mismatch(tmp_path):
    layout = packet_fixture(tmp_path)
    path = layout["redemption"]["record"]
    record = json.loads((tmp_path / path).read_text())
    record["state"]["settlement_reserve"] += 1
    save(tmp_path, path, record)
    assert "redemption/settlement_reserve" in z3.verify_manifest(build(tmp_path, layout))["error"]


def test_overwrite_refused(tmp_path):
    layout = packet_fixture(tmp_path)
    manifest = build(tmp_path, layout)
    original = manifest.read_bytes()
    with pytest.raises(z3.CycleError, match="overwrite"):
        z3.build_manifest(layout, tmp_path, manifest)
    assert manifest.read_bytes() == original


@pytest.mark.parametrize("change", ["secret_field", "symlink", "outside", "short_address", "skeleton"])
def test_public_packet_boundaries(tmp_path, change):
    layout = packet_fixture(tmp_path)
    if change == "secret_field":
        layout["manifest"]["private_key"] = "synthetic-forbidden-field"
    elif change == "symlink":
        path = tmp_path / layout["deposit"]["receipt"]
        path.unlink()
        path.symlink_to(tmp_path / layout["deposit"]["record"])
    elif change == "outside":
        layout["deposit"]["receipt"] = "../outside.json"
    elif change == "short_address":
        layout["manifest"]["accounts"]["arc_wallet"] = "0x1234"
    else:
        z3.build_manifest(layout, tmp_path, tmp_path / "cycle.json", skeleton=True)
        assert z3.verify_manifest(tmp_path / "cycle.json")["verdict"] == "FAIL"
        return
    with pytest.raises(z3.CycleError):
        build(tmp_path, layout)


def test_inline_success_cannot_hide_rejected_original(tmp_path):
    layout = packet_fixture(tmp_path)
    path = layout["ingress"]["record"]
    record = json.loads((tmp_path / path).read_text())
    record["receipts"]["claim_receipt"]["receipt"] = {"accepted": True}
    save(tmp_path, path, record)
    assert "must reference original" in z3.verify_manifest(build(tmp_path, layout))["error"]
