use postfiat_types::{
    VaultBridgeRouteProfileV1, NAV_PROFILE_VERIFIER_MULTI_FETCH,
    VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED, VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1,
};

pub(crate) fn route(
    epoch: u32,
    activation_height: u64,
    vault_hash_byte: &str,
) -> VaultBridgeRouteProfileV1 {
    VaultBridgeRouteProfileV1 {
        solana: None,
        schema: VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1.to_string(),
        route_id: "arbitrum-pfusdc".to_string(),
        asset_id: "21".repeat(48),
        source_chain_id: 42_161,
        vault_address: "0x1111111111111111111111111111111111111111".to_string(),
        vault_runtime_code_hash: format!("0x{}", vault_hash_byte.repeat(32)),
        token_address: "0x3333333333333333333333333333333333333333".to_string(),
        token_runtime_code_hash: format!("0x{}", "44".repeat(32)),
        route_epoch: epoch,
        verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
        evidence_tier: VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED.to_string(),
        verifier_policy_hash: String::new(),
        verifier_program_vkey: String::new(),
        verifier_proof_encoding: String::new(),
        max_proof_bytes: 0,
        max_public_values_bytes: 0,
        max_snapshot_age_blocks: 100,
        challenge_window_blocks: 6,
        max_epoch_gap_blocks: 1_000,
        settle_deadline_blocks: 1_000,
        min_challenge_bond: 1,
        min_attestations: 2,
        minimum_confirmations: 64,
        activation_height,
        expires_at_height: activation_height + 10_000,
    }
}
