use super::*;

/// AR-10: new private-primary issue and redeem actions must take the same
/// deterministic execution path under archive replay as under live
/// execution. Before the fix, replay of any private-primary batch outside a
/// hardcoded historical allowlist was rejected with
/// `*_archive_unsupported`, so every freshly finalized private-primary block
/// failed archive replay with a receipt-id mismatch (accelerated lifecycle
/// run 2, block 821, 2026-08-03).
fn ar10_payload(schema: &str) -> AssetOrchardPrivatePrimaryIssueActionPayload {
    serde_json::from_value(serde_json::json!({
        "version": 1,
        "schema": schema,
        "pool_id": "asset-orchard-v1",
        "route_id": "pftl-a666-ethereum-wA666-usdc-v1",
        "subscriber": "pffcb93d9f87a843a8aa34e1adf241f5d58143e81b",
        "ethereum_recipient": "0x4444444444444444444444444444444444444444",
        "reservation_id": "aa".repeat(48),
        "subscription_nonce": "ab".repeat(32),
        "route_epoch": 3,
        "policy_epoch": 3,
        "policy_hash": "ac".repeat(48),
        "pricing_nav_epoch": 8,
        "pricing_reserve_packet_hash": "ad".repeat(48),
        "mint_amount_atoms": 1_000_000,
        "settlement_value_atoms": 7_000_000,
        "expires_at_height": 900,
        "output_commitment": "ae".repeat(32),
        "encrypted_output": "af".repeat(64),
        "output_validity_action_json": "",
        "proof_system_id": "",
        "circuit_id": "",
        "pool_domain": "",
        "anchor": "b0".repeat(32),
        "nullifier": "b1".repeat(32),
        "randomized_verification_key": "b2".repeat(32),
        "settlement_asset_tag_lo": "00000000000000000000000000000000",
        "settlement_asset_tag_hi": "00000000000000000000000000000000",
        "native_nav_asset_tag_lo": "00000000000000000000000000000000",
        "native_nav_asset_tag_hi": "00000000000000000000000000000000",
        "primary_binding_hash": "b3".repeat(48),
        "proof": "",
        "spend_authorization_signature": "",
    }))
    .expect("build AR-10 private-primary payload")
}

fn ar10_execute_issue(archive_replay: bool) -> Receipt {
    let genesis = Genesis::new_with_validator_count("postfiat-wan-devnet-2", 6);
    let mut ledger = LedgerState::new(Vec::new());
    let mut shielded = ShieldedState::empty();
    crate::execution_actions::execute_asset_orchard_private_primary_issue_action(
        &genesis,
        &mut ledger,
        &mut shielded,
        &"cc".repeat(48),
        821,
        0,
        &ar10_payload("postfiat-asset-orchard-private-primary-issue-action-v1"),
        archive_replay,
    )
}

fn ar10_execute_redeem(archive_replay: bool) -> Receipt {
    let genesis = Genesis::new_with_validator_count("postfiat-wan-devnet-2", 6);
    let mut ledger = LedgerState::new(Vec::new());
    let mut shielded = ShieldedState::empty();
    crate::execution_actions::execute_asset_orchard_private_primary_redeem_action(
        &genesis,
        &mut ledger,
        &mut shielded,
        &"cd".repeat(48),
        822,
        0,
        &ar10_payload("postfiat-asset-orchard-private-primary-redeem-action-v1"),
        archive_replay,
    )
}

#[test]
fn ar10_private_primary_issue_replay_matches_live_execution_path() {
    let live = ar10_execute_issue(false);
    let replay = ar10_execute_issue(true);
    assert_ne!(
        replay.code, "asset_orchard_private_primary_issue_archive_unsupported",
        "new private-primary issue blocks must be archive-replayable: {replay:?}"
    );
    assert_eq!(
        live.code, replay.code,
        "live and archive-replay execution must take the same deterministic \
         path: live {live:?} vs replay {replay:?}"
    );
    assert_eq!(live.tx_id, replay.tx_id, "receipt ids must be identical");
    assert!(!live.accepted, "the AR-10 probe payload is invalid by design");
}

#[test]
fn ar10_private_primary_redeem_replay_matches_live_execution_path() {
    let live = ar10_execute_redeem(false);
    let replay = ar10_execute_redeem(true);
    assert_ne!(
        replay.code, "asset_orchard_private_primary_redeem_archive_unsupported",
        "new private-primary redeem blocks must be archive-replayable: {replay:?}"
    );
    assert_eq!(
        live.code, replay.code,
        "live and archive-replay execution must take the same deterministic \
         path: live {live:?} vs replay {replay:?}"
    );
    assert_eq!(live.tx_id, replay.tx_id, "receipt ids must be identical");
    assert!(!live.accepted, "the AR-10 probe payload is invalid by design");
}

#[test]
fn ar10_historical_private_primary_allowlist_stays_exact() {
    // The historical allowlist remains a narrowly scoped compatibility set;
    // it must never widen to new heights or other chains.
    let genesis = Genesis::new_with_validator_count("postfiat-wan-devnet-2", 6);
    let other_chain = Genesis::new_with_validator_count("postfiat-local", 6);
    let historical_batch =
        "1d971584eb5cf2752aed24b7128f0412517a12844549998a66879634d3c70fe73d6ae209317052f2fd696f17f08a8b11";
    assert!(archived_wan_devnet2_private_primary_execution_allowed(
        &genesis,
        378,
        historical_batch,
        false
    ));
    assert!(!archived_wan_devnet2_private_primary_execution_allowed(
        &other_chain,
        378,
        historical_batch,
        false
    ));
    assert!(
        !archived_wan_devnet2_private_primary_execution_allowed(&genesis, 821, historical_batch, false),
        "new heights must not inherit historical allowances"
    );
}
