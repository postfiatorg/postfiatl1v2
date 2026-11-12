use super::*;
use sha2::{Digest, Sha384};

fn legacy_route() -> PftlUniswapConsensusRouteState {
    let mut route: PftlUniswapConsensusRouteState=serde_json::from_str(r#"{
  "route_id": "b23-legacy-commitment",
  "route_family": "primary_pftl_mint",
  "route_config_digest": "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
  "route_trust_class": "BFT_CHECKPOINT",
  "native_nav_asset_id": "222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222",
  "settlement_asset_id": "333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333",
  "handoff_controller": "0x4444444444444444444444444444444444444444",
  "settlement_adapter": "0x5555555555555555555555555555555555555555",
  "wrapped_navcoin_token": "0x6666666666666666666666666666666666666666",
  "ethereum_chain_id": 1,
  "route_supply_cap_atoms": 1000,
  "packet_notional_cap_atoms": 100,
  "latest_finalized_nav_epoch": 1,
  "return_finality_blocks": 12,
  "authorized_valid_supply_atoms": 100,
  "pftl_spendable_supply_atoms": 100,
  "native_spendable_balances_atoms": {
    "alice": 100
  },
  "ethereum_spendable_supply_atoms": 0,
  "other_registered_venue_supply_atoms": 0,
  "outstanding_bridge_claims_atoms": 0,
  "pending_return_import_claims_atoms": 0,
  "settlement_reserve_atoms": 100,
  "paused": false,
  "v2": {
    "route_schema_version": 2,
    "route_epoch": 1,
    "outbound_verification_class": "TRUSTLESS_FINALITY",
    "return_verification_class": "BFT_CHECKPOINT",
    "issue_capacity_used_atoms": 100,
    "redeem_capacity_used_atoms": 0,
    "non_nav_spread_atoms": 5,
    "primary_market_policy": {
      "policy_hash": "777777777777777777777777777777777777777777777777777777777777777777777777777777777777777777777777",
      "policy_epoch": 1,
      "issue_multiplier_bps": 10050,
      "redeem_multiplier_bps": 9995,
      "issue_capacity_atoms": 1000,
      "redeem_capacity_atoms": 1000,
      "max_order_atoms": 100,
      "min_order_atoms": 1,
      "valid_from_height": 1,
      "expires_at_height": 100,
      "max_nav_age_blocks": 10,
      "pricing_nav_epoch": 1,
      "pricing_reserve_packet_hash": "888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888"
    }
  }
}"#).unwrap();
    let policy = &mut route.v2.as_mut().unwrap().primary_market_policy;
    policy.policy_hash = policy.computed_hash();
    route.validate().unwrap();
    route
}

#[test]
fn b23_node_route_commitment_preserves_legacy_bytes_and_binds_recipient() {
    let mut route = legacy_route();
    let mut bytes = Vec::new();
    append_pftl_uniswap_route(&mut bytes, "route", &route);
    // Frozen from the byte-for-byte comparison against ada01630c's encoder.
    assert_eq!(format!("{:x}",Sha384::digest(&bytes)),
        "760f18f32fb93d302ba6c5b01f8be5373a3642291c72641960846601bc8df52ef7dfcb76621ed59f8938d79ea49814b3");
    route
        .v2
        .as_mut()
        .unwrap()
        .primary_market_policy
        .spread_recipient = Some("management".to_string());
    let mut opted_in = Vec::new();
    append_pftl_uniswap_route(&mut opted_in, "route", &route);
    assert_ne!(
        bytes, opted_in,
        "recipient is committed even before the cached policy hash changes"
    );
}

#[test]
fn b23_node_receipt_commitment_preserves_legacy_bytes_and_binds_both_credits() {
    let mut receipt: PftlUniswapConsensusReceipt=serde_json::from_value(serde_json::json!({
        "receipt_hash":"11".repeat(48),"transition":"primary_redeemed","route_id":"b23-legacy-commitment",
        "state_before_hash":"22".repeat(48),"state_after_hash":"33".repeat(48),
        "wallet":"alice","amount_atoms":1000,"block_height":5
    })).unwrap();
    let mut bytes = Vec::new();
    append_pftl_uniswap_receipt(&mut bytes, "receipt", &receipt);
    // Frozen from the byte-for-byte comparison against ada01630c's encoder.
    assert_eq!(format!("{:x}",Sha384::digest(&bytes)),
        "c6c7ddaef135e5c33460d303bdeb4601892664faf7f3fdee533325cff59cc5e9def22f3b76a46b8500b89aea8afff7fb");
    receipt.redemption_settlement = Some(postfiat_types::PftlUniswapRedemptionSettlement {
        settlement_asset_id: "44".repeat(48),
        settlement_recipient: "alice".to_string(),
        settlement_value_atoms: 998,
        spread_recipient: "management".to_string(),
        spread_value_atoms: 2,
    });
    let mut opted_in = Vec::new();
    append_pftl_uniswap_receipt(&mut opted_in, "receipt", &receipt);
    assert_ne!(bytes, opted_in);
    let base = opted_in;
    for field in 0..5 {
        let mut changed = receipt.clone();
        let settlement = changed.redemption_settlement.as_mut().unwrap();
        match field {
            0 => settlement.settlement_asset_id = "55".repeat(48),
            1 => settlement.settlement_recipient = "bob".to_string(),
            2 => settlement.settlement_value_atoms += 1,
            3 => settlement.spread_recipient = "other".to_string(),
            _ => settlement.spread_value_atoms += 1,
        }
        let mut modified = Vec::new();
        append_pftl_uniswap_receipt(&mut modified, "receipt", &changed);
        assert_ne!(base, modified);
    }
}

#[test]
fn b23_node_funding_sequence_is_committed_only_when_present() {
    let mut route = legacy_route();
    let mut before = Vec::new();
    append_pftl_uniswap_route(&mut before, "route", &route);
    assert_eq!(format!("{:x}",Sha384::digest(&before)),"760f18f32fb93d302ba6c5b01f8be5373a3642291c72641960846601bc8df52ef7dfcb76621ed59f8938d79ea49814b3");
    route.v2.as_mut().unwrap().funding_sequence = Some(1);
    let mut after = Vec::new();
    append_pftl_uniswap_route(&mut after, "route", &route);
    assert_ne!(before, after);
    route.v2.as_mut().unwrap().funding_sequence = None;
    let mut restored = Vec::new();
    append_pftl_uniswap_route(&mut restored, "route", &route);
    assert_eq!(before, restored);
}
