"""Deterministic fixture & adversarial tests for Hyperliquid NAVCoin on Solana bridge."""

from __future__ import annotations

import hashlib
import json
import sys
from pathlib import Path

import pytest
from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from hyperliquid_navcoin import (  # noqa: E402
    HL_RECEIPT_EVENT_TOPIC0_HEX,
    DustSwapRequest,
    HyperCoreMarginSummary,
    HyperCorePerpSnapshot,
    HyperCorePolicy,
    HyperCoreSnapshotPayload,
    HyperCoreSpotSnapshot,
    NonIssuerAttestation,
    RegisteredAttestor,
    SolanaClusterReadiness,
    SpotTokenPolicy,
    SwapReadinessConfig,
    cli,
    compute_nav_per_unit,
    compute_snapshot_commitment,
    encode_hypercore_payload,
    rehearse_dust_swap,
    sign_attestation,
    verify_hypercore_snapshot,
    verify_non_issuer_attestor_quorum,
)
from hyperliquid_navcoin.solana_swap_bridge import _canonical_reserve_packet_hash  # noqa: E402

READER_ADDR = "0xddb4ed1edf1f0d81f7531cddb27810080601a2cb"
READER_CODE_HASH = "0xc252f32acd9fdcfe2b4f9b1d70c3de17acf83649a6313fc3ab9155bca1010db3"
RESERVE_OWNER = "0x1111111111111111111111111111111111111111"
ASSET_ISSUER = "pf1000000000000000000000000000000000000001"
ATTESTOR_1 = "pf2000000000000000000000000000000000000002"
ATTESTOR_2 = "pf3000000000000000000000000000000000000003"
ASSET_ID = "ab" * 48
PROFILE_HASH = "cd" * 48
NOW_UNIX = 1_795_750_000


def _ed25519_seed(label: bytes) -> tuple[bytes, str]:
    seed = hashlib.sha256(label).digest()
    pub = Ed25519PrivateKey.from_private_bytes(seed).public_key().public_bytes_raw().hex()
    return seed, pub


def _build_fixture_bundle() -> dict:
    policy = HyperCorePolicy(
        source_domain="eip155:999",
        aggregate_position_id="aggregate-spot-and-cash",
        hyperevm_chain_id=999,
        reader_contract=READER_ADDR,
        reader_code_hash=READER_CODE_HASH,
        required_perps=(1, 159),
        allowed_spot_tokens=(
            SpotTokenPolicy(token=150, wei_decimals=8),
            SpotTokenPolicy(token=404, wei_decimals=8),
        ),
        expected_account=RESERVE_OWNER,
    )

    # Perp notional check:
    # perp 1: |szi=200| * mark_px=500_000 = 100_000_000 -> *100 = 10_000_000_000 usd_e8
    # perp 159: |szi=-100| * mark_px=250_000 = 25_000_000 -> *100 = 2_500_000_000 usd_e8
    # total ntl_pos = 125_000_000 (usd_e6) -> *100 = 12_500_000_000 usd_e8
    payload = HyperCoreSnapshotPayload(
        account=RESERVE_OWNER,
        margin_summary=HyperCoreMarginSummary(
            account_value=500_000_000_000,  # $500,000.00 in usd_e6
            margin_used=100_000_000_000,
            ntl_pos=125_000_000,
            raw_usd=500_000_000_000,
        ),
        withdrawable_usd_e6=400_000_000_000,
        perps=(
            HyperCorePerpSnapshot(perp=1, szi=200, mark_px=500_000),
            HyperCorePerpSnapshot(perp=159, szi=-100, mark_px=250_000),
        ),
        spots=(
            # 10,000 HYPE (8 dec) @ $25.00 (25_00000000 e8) = $250,000 (500 locked, 9500 unlocked)
            HyperCoreSpotSnapshot(
                token=150,
                total=10_000 * 10**8,
                hold=500 * 10**8,
                wei_decimals=8,
                price_usd_e8=25 * 10**8,
            ),
            # 1,000 XMR1 (8 dec) @ $250.00 (250_00000000 e8) = $250,000
            HyperCoreSpotSnapshot(
                token=404,
                total=1_000 * 10**8,
                hold=0,
                wei_decimals=8,
                price_usd_e8=250 * 10**8,
            ),
        ),
    )
    payload_bytes = encode_hypercore_payload(payload)
    salt = "0x" + "77" * 32
    commitment = compute_snapshot_commitment(payload_bytes, salt)
    receipt = {
        "reader_contract": READER_ADDR,
        "reader_code_hash": READER_CODE_HASH,
        "event_topic0": "0x" + HL_RECEIPT_EVENT_TOPIC0_HEX,
        "block_number": 42_500_000,
        "block_time_ms": (NOW_UNIX - 20) * 1000,
        "commitment": commitment,
        "salt": salt,
        "payload_hex": "0x" + payload_bytes.hex(),
    }

    hc = verify_hypercore_snapshot(receipt, policy, now_unix=NOW_UNIX)
    # Gross assets = $500k cash + $250k HYPE + $250k XMR1 = $1,000,000.00 (1_000_000_000_000 usd_e6)
    assert hc.verified_net_assets_usd_e6 == 1_000_000_000_000

    config = SwapReadinessConfig(
        chain_id="postfiat-wan-devnet-2",
        asset_id=ASSET_ID,
        asset_code="HLNAV",
        asset_issuer=ASSET_ISSUER,
        asset_precision=6,
        circulating_supply=800_000_000_000,  # 800,000 units -> NAV = $1.25 (1_250_000 usd_e6)
        proof_profile="hyperliquid-hyperevm-receipt-v1",
        epoch=42,
        min_attestations=2,
    )
    nav_per_unit = compute_nav_per_unit(hc.verified_net_assets_usd_e6, config.circulating_supply, config.asset_precision)
    assert nav_per_unit == 1_250_000
    packet_hash = _canonical_reserve_packet_hash(
        asset_id=config.asset_id,
        epoch=config.epoch,
        verified_net_assets_usd_e6=hc.verified_net_assets_usd_e6,
        circulating_supply=config.circulating_supply,
        nav_per_unit_usd_e6=nav_per_unit,
        observation_root=hc.observation_root,
        snapshot_commitment=hc.commitment,
    )

    seed1, pub1 = _ed25519_seed(b"hl-nav-attestor-1-fixture")
    seed2, pub2 = _ed25519_seed(b"hl-nav-attestor-2-fixture")
    issuer_seed, issuer_pub = _ed25519_seed(b"hl-nav-issuer-fixture")

    reg_attestors = [
        RegisteredAttestor(address=ATTESTOR_1, domain="obs1.example", ed25519_public_key_hex=pub1, bond=100_000),
        RegisteredAttestor(address=ATTESTOR_2, domain="obs2.example", ed25519_public_key_hex=pub2, bond=100_000),
        RegisteredAttestor(address=ASSET_ISSUER, domain="issuer.example", ed25519_public_key_hex=issuer_pub, bond=0),
    ]
    att1 = sign_attestation(
        key_seed_32=seed1,
        attestor=ATTESTOR_1,
        asset_id=ASSET_ID,
        epoch=42,
        reserve_packet_hash=packet_hash,
        observation_root=hc.observation_root,
        snapshot_commitment=hc.commitment,
        verified_net_assets_usd_e6=hc.verified_net_assets_usd_e6,
        passed=True,
        attested_at_unix=NOW_UNIX - 10,
    )
    att2 = sign_attestation(
        key_seed_32=seed2,
        attestor=ATTESTOR_2,
        asset_id=ASSET_ID,
        epoch=42,
        reserve_packet_hash=packet_hash,
        observation_root=hc.observation_root,
        snapshot_commitment=hc.commitment,
        verified_net_assets_usd_e6=hc.verified_net_assets_usd_e6,
        passed=True,
        attested_at_unix=NOW_UNIX - 8,
    )
    issuer_self_att = sign_attestation(
        key_seed_32=issuer_seed,
        attestor=ASSET_ISSUER,
        asset_id=ASSET_ID,
        epoch=42,
        reserve_packet_hash=packet_hash,
        observation_root=hc.observation_root,
        snapshot_commitment=hc.commitment,
        verified_net_assets_usd_e6=hc.verified_net_assets_usd_e6,
        passed=True,
        attested_at_unix=NOW_UNIX - 5,
    )

    solana = SolanaClusterReadiness(
        route_id="solana-mainnet-usdc-v1",
        anchored_epoch=910,
        current_epoch=910,
        finalized_slot=393_120_500,
        observer_slot=393_120_480,
        epoch_end_slot=393_552_000,
        route_profile_hash=PROFILE_HASH,
        expected_profile_hash=PROFILE_HASH,
        prover_healthy=True,
        relay_ready=True,
        solana_navcoin_mint="HLNavMint1111111111111111111111111111111111",
        solana_usdc_mint="EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v",
        health_age_seconds=12,
    )
    swap = DustSwapRequest(
        side="buy_navcoin",
        usdc_atoms=5_000_000,  # $5.00 dust rehearsal
        spread_bps=25,
        solana_trader="Trader1111111111111111111111111111111111111",
        pftl_recipient="pf9999999999999999999999999999999999999999",
    )

    return {
        "policy": policy,
        "receipt": receipt,
        "config": config,
        "registered_attestors": reg_attestors,
        "attestations": [att1, att2],
        "issuer_self_att": issuer_self_att,
        "solana": solana,
        "swap": swap,
        "packet_hash": packet_hash,
        "hc": hc,
    }


def test_deterministic_dust_swap_rehearsal_buy_and_sell() -> None:
    fx = _build_fixture_bundle()
    buy_report = rehearse_dust_swap(
        config=fx["config"],
        policy=fx["policy"],
        snapshot_receipt=fx["receipt"],
        registered_attestors=fx["registered_attestors"],
        attestations=fx["attestations"],
        solana=fx["solana"],
        swap=fx["swap"],
        now_unix=NOW_UNIX,
    )
    assert buy_report["solana_readiness"]["ready"] is True
    assert buy_report["nav_per_unit_usd_e6"] == 1_250_000
    assert buy_report["attestor_quorum"]["quorum_met"] is True
    assert buy_report["attestor_quorum"]["valid_non_issuer_count"] == 2
    assert buy_report["dust_swap_plan"]["quote"]["navcoin_out_atoms"] > 0
    assert len(buy_report["l1_operations"]["nav_reserve_attest"]) == 2

    # Sell direction rehearsal
    sell_swap = DustSwapRequest(
        side="sell_navcoin",
        navcoin_atoms=4_000_000,  # 4.00 HLNAV (~$5.00 gross)
        spread_bps=25,
        solana_trader="Trader1111111111111111111111111111111111111",
        pftl_recipient="pf9999999999999999999999999999999999999999",
    )
    sell_report = rehearse_dust_swap(
        config=fx["config"],
        policy=fx["policy"],
        snapshot_receipt=fx["receipt"],
        registered_attestors=fx["registered_attestors"],
        attestations=fx["attestations"],
        solana=fx["solana"],
        swap=sell_swap,
        now_unix=NOW_UNIX,
    )
    # 4 * 1.25 = $5.00 gross, minus 25 bps = $4.9875 = 4_987_500 usdc atoms
    assert sell_report["dust_swap_plan"]["quote"]["usdc_out_atoms"] == 4_987_500


def test_issuer_self_attestation_forbidden_and_bad_signatures_rejected() -> None:
    fx = _build_fixture_bundle()
    # Replace attestor 2 with issuer self-attestation -> quorum fails!
    quorum = verify_non_issuer_attestor_quorum(
        asset_issuer=ASSET_ISSUER,
        asset_id=ASSET_ID,
        epoch=42,
        reserve_packet_hash=fx["packet_hash"],
        expected_observation_root=fx["hc"].observation_root,
        expected_snapshot_commitment=fx["hc"].commitment,
        expected_net_assets_usd_e6=fx["hc"].verified_net_assets_usd_e6,
        registered_attestors=fx["registered_attestors"],
        attestations=[fx["attestations"][0], fx["issuer_self_att"]],
        min_attestations=2,
        now_unix=NOW_UNIX,
    )
    assert quorum.quorum_met is False
    assert quorum.valid_non_issuer_count == 1
    assert any("nav_fee_issuer_self_attestation_forbidden" in r for r in quorum.rejected_reasons)

    # Duplicate attestor vote cannot satisfy quorum of 2.
    dup = verify_non_issuer_attestor_quorum(
        asset_issuer=ASSET_ISSUER,
        asset_id=ASSET_ID,
        epoch=42,
        reserve_packet_hash=fx["packet_hash"],
        expected_observation_root=fx["hc"].observation_root,
        expected_snapshot_commitment=fx["hc"].commitment,
        expected_net_assets_usd_e6=fx["hc"].verified_net_assets_usd_e6,
        registered_attestors=fx["registered_attestors"],
        attestations=[fx["attestations"][0], fx["attestations"][0]],
        min_attestations=2,
        now_unix=NOW_UNIX,
    )
    assert dup.quorum_met is False
    assert any("duplicate_attestation" in r for r in dup.rejected_reasons)

    # Tampered signature rejected.
    bad_sig = NonIssuerAttestation(
        **{**fx["attestations"][1].__dict__, "signature_hex": "00" * 64}
    )
    tampered = verify_non_issuer_attestor_quorum(
        asset_issuer=ASSET_ISSUER,
        asset_id=ASSET_ID,
        epoch=42,
        reserve_packet_hash=fx["packet_hash"],
        expected_observation_root=fx["hc"].observation_root,
        expected_snapshot_commitment=fx["hc"].commitment,
        expected_net_assets_usd_e6=fx["hc"].verified_net_assets_usd_e6,
        registered_attestors=fx["registered_attestors"],
        attestations=[fx["attestations"][0], bad_sig],
        min_attestations=2,
        now_unix=NOW_UNIX,
    )
    assert tampered.quorum_met is False
    assert any("invalid_ed25519_signature" in r for r in tampered.rejected_reasons)


def test_hypercore_snapshot_tampering_and_unlisted_perp_rejected() -> None:
    fx = _build_fixture_bundle()

    # Wrong commitment rejected
    bad_commit = {**fx["receipt"], "commitment": "0x" + "11" * 32}
    with pytest.raises(ValueError, match="bad_commitment"):
        verify_hypercore_snapshot(bad_commit, fx["policy"], now_unix=NOW_UNIX)

    # Wrong code hash rejected
    bad_code = {**fx["receipt"], "reader_code_hash": "0x" + "99" * 32}
    with pytest.raises(ValueError, match="reader_code_hash_mismatch"):
        verify_hypercore_snapshot(bad_code, fx["policy"], now_unix=NOW_UNIX)

    # Hidden unlisted perp (ntl_pos > sum of declared perps) rejected
    hidden_payload = HyperCoreSnapshotPayload(
        account=RESERVE_OWNER,
        margin_summary=HyperCoreMarginSummary(
            account_value=500_000_000_000,
            margin_used=100_000_000_000,
            ntl_pos=999_999_999,  # mismatch vs sum=125_000_000
            raw_usd=500_000_000_000,
        ),
        withdrawable_usd_e6=400_000_000_000,
        perps=(
            HyperCorePerpSnapshot(perp=1, szi=200, mark_px=500_000),
            HyperCorePerpSnapshot(perp=159, szi=-100, mark_px=250_000),
        ),
        spots=(
            HyperCoreSpotSnapshot(token=150, total=0, hold=0, wei_decimals=8, price_usd_e8=0),
            HyperCoreSpotSnapshot(token=404, total=0, hold=0, wei_decimals=8, price_usd_e8=0),
        ),
    )
    hidden_bytes = encode_hypercore_payload(hidden_payload)
    salt = "0x" + "88" * 32
    hidden_receipt = {
        **fx["receipt"],
        "salt": salt,
        "payload_hex": "0x" + hidden_bytes.hex(),
        "commitment": compute_snapshot_commitment(hidden_bytes, salt),
    }
    with pytest.raises(ValueError, match="perp_notional_mismatch"):
        verify_hypercore_snapshot(hidden_receipt, fx["policy"], now_unix=NOW_UNIX)


def test_solana_epoch_boundary_lag_and_dust_cap_fail_closed() -> None:
    fx = _build_fixture_bundle()

    # Epoch boundary closed
    near_boundary = SolanaClusterReadiness(
        **{**fx["solana"].__dict__, "finalized_slot": fx["solana"].epoch_end_slot - 100}
    )
    with pytest.raises(ValueError, match="solana_epoch_boundary_closed"):
        rehearse_dust_swap(
            config=fx["config"],
            policy=fx["policy"],
            snapshot_receipt=fx["receipt"],
            registered_attestors=fx["registered_attestors"],
            attestations=fx["attestations"],
            solana=near_boundary,
            swap=fx["swap"],
            now_unix=NOW_UNIX,
        )

    # Observer lag > 150 slots blocked
    lagging = SolanaClusterReadiness(
        **{**fx["solana"].__dict__, "observer_slot": fx["solana"].finalized_slot - 200}
    )
    with pytest.raises(ValueError, match="solana_observer_lag"):
        rehearse_dust_swap(
            config=fx["config"],
            policy=fx["policy"],
            snapshot_receipt=fx["receipt"],
            registered_attestors=fx["registered_attestors"],
            attestations=fx["attestations"],
            solana=lagging,
            swap=fx["swap"],
            now_unix=NOW_UNIX,
        )

    # Dust cap exceeded ($50 > $10 cap) blocked
    oversized_swap = DustSwapRequest(**{**fx["swap"].__dict__, "usdc_atoms": 50_000_000})
    with pytest.raises(ValueError, match="dust_swap_cap_exceeded"):
        rehearse_dust_swap(
            config=fx["config"],
            policy=fx["policy"],
            snapshot_receipt=fx["receipt"],
            registered_attestors=fx["registered_attestors"],
            attestations=fx["attestations"],
            solana=fx["solana"],
            swap=oversized_swap,
            now_unix=NOW_UNIX,
        )


def test_committed_fixture_cli_rehearsal(tmp_path: Path) -> None:
    fixture_path = Path(__file__).resolve().parents[1] / "fixtures" / "deterministic_fixture.json"
    out_path = tmp_path / "rehearsal.json"
    rc = cli.main(["rehearse-dust-swap", "--bundle", str(fixture_path), "--output", str(out_path)])
    assert rc == 0
    report = json.loads(out_path.read_text(encoding="utf-8"))
    assert report["schema"] == "postfiat.hyperliquid_solana_nav_rehearsal.v1"
    assert report["solana_readiness"]["ready"] is True
    assert report["attestor_quorum"]["quorum_met"] is True
