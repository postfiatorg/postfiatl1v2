//! Offline replay of the frozen Z3 G2 selected-state read-back
//! (`docs/status/z3-g2-selected-state-20261111.json`): every derived identity is
//! recomputed with the canonical repository functions and compared with the read.

use postfiat_bridge::{pftl_uniswap_route_config_digest, PftlUniswapRouteConfig};
use postfiat_types::{
    pfusdc_source_series_id, vault_bridge_bucket_id, NavProofProfile,
    PftlUniswapPrimaryMarketPolicyV2,
};
use serde_json::Value;
use std::path::PathBuf;

fn repo_file(relative: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let text = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{relative}: {e}"));
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{relative}: {e}"))
}

fn fixture() -> Value {
    repo_file("docs/status/z3-g2-selected-state-20261111.json")
}

fn s<'a>(v: &'a Value, key: &str) -> &'a str {
    v[key]
        .as_str()
        .unwrap_or_else(|| panic!("{key} is not a string"))
}

fn n(v: &Value, key: &str) -> u64 {
    v[key]
        .as_u64()
        .unwrap_or_else(|| panic!("{key} is not a u64"))
}

fn nav_profile_id(profile: &Value) -> String {
    let built = NavProofProfile::new(
        s(profile, "registered_by"),
        s(profile, "verifier_kind"),
        s(profile, "source_class"),
        n(profile, "max_snapshot_age_blocks"),
        n(profile, "challenge_window_blocks"),
        n(profile, "max_epoch_gap_blocks"),
        n(profile, "settle_deadline_blocks"),
        n(profile, "min_challenge_bond"),
        n(profile, "min_attestations"),
        n(profile, "tolerance_bp"),
        s(profile, "valuation_policy_hash"),
        s(profile, "sp1_program_vkey"),
        s(profile, "sp1_proof_encoding"),
        n(profile, "max_proof_bytes"),
        n(profile, "max_public_values_bytes"),
    )
    .expect("profile");
    match profile
        .get("vault_bridge_route_policy_hash")
        .and_then(Value::as_str)
    {
        Some(route_policy) => {
            built
                .with_vault_bridge_route_policy_hash(route_policy)
                .expect("route policy")
                .profile_id
        }
        None => built.profile_id,
    }
}

#[test]
fn fixture_records_six_agreeing_validators_at_one_height() {
    let fleet = &fixture()["fleet"];
    let validators = fleet["validators"].as_array().expect("validators");
    assert_eq!(validators.len(), 6);
    for v in validators {
        for (field, expected) in [("tip_hash", "tip_hash"), ("state_root", "state_root")] {
            assert_eq!(v[field], fleet[expected], "{} {field}", v["validator"]);
        }
        assert_eq!(v["height_before"], fleet["height"]);
        assert_eq!(v["height_after"], fleet["height"]);
        assert_eq!(
            v["normalized_reads_sha256"],
            validators[0]["normalized_reads_sha256"]
        );
    }
}

#[test]
fn arc_source_series_and_bucket_replay_from_their_governed_tuple() {
    let f = fixture();
    let arc = &f["arc_source_series"];
    let input = &arc["derivation_inputs"];
    let series = pfusdc_source_series_id(
        s(input, "pftl_chain_id"),
        s(input, "asset_family_id"),
        n(input, "source_chain_id"),
        s(input, "vault_address"),
        s(input, "token_address"),
        n(input, "route_epoch") as u32,
        s(input, "policy_hash"),
    )
    .expect("series");
    assert_eq!(series, s(&arc["asset"], "asset_id"));
    assert_eq!(series, s(&arc["asset"], "source_series_id"));
    assert_eq!(series, s(&arc["bucket"], "source_series_id"));
    let domain = format!(
        "erc20_bridge_vault:{}:{}:{}",
        n(input, "source_chain_id"),
        s(input, "vault_address"),
        s(input, "token_address")
    );
    assert_eq!(domain, s(input, "source_domain"));
    let bucket = vault_bridge_bucket_id(
        s(input, "asset_family_id"),
        &domain,
        s(input, "policy_hash"),
    )
    .expect("bucket");
    assert_eq!(bucket, s(&arc["asset"], "source_bucket_id"));
    assert_eq!(bucket, s(&arc["bucket"], "bucket_id"));
    assert_eq!(
        s(&arc["asset"], "asset_family_id"),
        s(&f["pfusdc_family"]["asset"], "asset_id")
    );
    assert_eq!(
        s(&f["a666_primary_route"], "settlement_asset_id"),
        s(input, "asset_family_id")
    );
    // The source domain names the Arc pair read on Arc testnet.
    assert_eq!(
        s(input, "vault_address"),
        s(&f["arc_testnet"]["vault"], "address")
    );
    assert_eq!(
        s(input, "token_address"),
        s(&f["arc_testnet"]["vault"], "token")
    );
    assert_eq!(
        n(input, "route_epoch"),
        n(&f["arc_testnet"]["verifier"], "route_epoch")
    );
}

#[test]
fn a666_route_config_digest_and_policy_hash_replay() {
    let f = fixture();
    let route = &f["a666_primary_route"];
    let config: PftlUniswapRouteConfig = serde_json::from_value(repo_file(
        "deployments/a666-mainnet-20260727/09-production-route-config.json",
    ))
    .expect("route config");
    assert_eq!(
        pftl_uniswap_route_config_digest(&config).expect("digest"),
        s(route, "route_config_digest")
    );
    assert_eq!(config.route_id, s(route, "route_id"));
    assert_eq!(config.native_nav_asset_id, s(route, "native_nav_asset_id"));
    assert_eq!(config.settlement_asset_id, s(route, "settlement_asset_id"));
    assert_eq!(
        config.wrapped_navcoin_token.to_lowercase(),
        s(route, "wrapped_navcoin_token")
    );
    assert_eq!(
        config.handoff_controller.to_lowercase(),
        s(route, "handoff_controller")
    );
    // Epoch 11 has had no order yet, so remaining capacity equals the policy capacity.
    let policy = PftlUniswapPrimaryMarketPolicyV2 {
        policy_hash: String::new(),
        policy_epoch: n(route, "policy_epoch"),
        issue_multiplier_bps: n(route, "issue_multiplier_bps") as u32,
        redeem_multiplier_bps: n(route, "redeem_multiplier_bps") as u32,
        issue_capacity_atoms: n(route, "issue_capacity_remaining_atoms"),
        redeem_capacity_atoms: n(route, "redeem_capacity_remaining_atoms"),
        max_order_atoms: n(route, "max_order_atoms"),
        min_order_atoms: n(route, "min_order_atoms"),
        valid_from_height: n(route, "policy_valid_from_height"),
        expires_at_height: n(route, "policy_expires_at_height"),
        max_nav_age_blocks: n(route, "max_nav_age_blocks"),
        pricing_nav_epoch: n(route, "pricing_nav_epoch"),
        pricing_reserve_packet_hash: s(route, "pricing_reserve_packet_hash").to_string(),
    };
    assert_eq!(policy.computed_hash(), s(route, "policy_hash"));
    assert_eq!(
        n(route, "pricing_nav_epoch"),
        n(&f["a666_nav"], "finalized_epoch")
    );
    assert_eq!(
        s(route, "pricing_reserve_packet_hash"),
        s(&f["a666_nav"]["finalized_packet"], "reserve_packet_hash")
    );
}

#[test]
fn nav_profile_ids_replay_and_a666_profile_lacks_reserve_bindings() {
    let f = fixture();
    let a666 = &f["a666_nav"]["active_profile"];
    assert_eq!(nav_profile_id(a666), s(a666, "profile_id"));
    let family = &f["pfusdc_family"]["nav"]["active_profile"];
    assert_eq!(nav_profile_id(family), s(family, "profile_id"));
    // The G2 gap, read back: legacy kind, no schema or source-manifest binding, no proof-verified value.
    assert_eq!(s(a666, "verifier_kind"), "sp1-groth16");
    assert!(f["a666_nav"]["public_values_schema"].is_null());
    assert!(f["a666_nav"]["source_manifest_hash"].is_null());
    assert_eq!(
        n(
            &f["a666_nav"]["finalized_packet"],
            "proof_verified_net_assets"
        ),
        0
    );
}

#[test]
fn arc_series_has_no_custody_row_on_the_a666_primary_route() {
    let f = fixture();
    let series = s(&f["arc_source_series"]["asset"], "asset_id");
    let rows = f["a666_primary_route"]["source_settlement_custody"]
        .as_array()
        .expect("rows");
    assert_eq!(rows.len(), 1);
    assert!(rows.iter().all(|row| row["asset_id"] != series));
}
