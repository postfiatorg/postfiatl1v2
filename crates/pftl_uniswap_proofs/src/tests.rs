use super::*;

#[test]
fn receipt_public_values_use_exact_static_solidity_abi_layout() {
    let values = PftlUniswapReceiptPublicValuesV1 {
        proof_program_version: 1,
        pftl_chain_id_hash: [1; 32],
        pftl_genesis_hash_commitment: [2; 32],
        pftl_protocol_version: 3,
        committee_root_commitment: [18; 32],
        committee_transition_commitment: [19; 32],
        finalized_block_commitment: [20; 32],
        finalized_state_root_commitment: [21; 32],
        route_epoch: 4,
        policy_hash_commitment: [5; 32],
        route_id_commitment: [22; 32],
        route_trust_class: [23; 32],
        route_config_digest_commitment: [6; 32],
        native_nav_asset_id_commitment: [24; 32],
        settlement_asset_id_commitment: [25; 32],
        pricing_nav_epoch: 26,
        pricing_reserve_packet_hash_commitment: [27; 32],
        source_wallet_commitment: [28; 32],
        source_receipt_root_commitment: [7; 32],
        source_receipt_hash_commitment: [8; 32],
        accepted_receipt_code: [29; 32],
        packet_digest: [9; 32],
        destination_chain_id: 1,
        controller: [10; 20],
        wrapped_token: [11; 20],
        recipient: [12; 20],
        mint_amount_atoms: 250_000_000_000,
        settlement_value_atoms: 251_250_000_000,
        packet_nonce: [30; 32],
        deadline: 1_800_000_000,
        source_height: 13,
        prior_checkpoint_commitment: [14; 32],
        resulting_checkpoint_commitment: [15; 32],
        finalized_height: 16,
        proof_nullifier: [17; 32],
    };
    let encoded = values.abi_encode();
    assert_eq!(encoded.len(), 35 * 32);
    assert_eq!(&encoded[23 * 32..23 * 32 + 12], &[0; 12]);
    assert_eq!(&encoded[23 * 32 + 12..24 * 32], &[10; 20]);
    assert_eq!(&encoded[34 * 32..35 * 32], &[17; 32]);
}

#[test]
fn domain_nullifier_changes_with_any_finality_binding() {
    let base = keccak_domain(b"postfiat.pftl_uniswap.receipt_proof_nullifier.v1", b"a");
    assert_ne!(
        base,
        keccak_domain(b"postfiat.pftl_uniswap.receipt_proof_nullifier.v1", b"b")
    );
    assert_ne!(base, keccak_domain(b"other", b"a"));
}

#[test]
fn checkpoint_public_values_use_exact_static_solidity_abi_layout() {
    let values = PftlUniswapCheckpointPublicValuesV1 {
        proof_program_version: 1,
        pftl_chain_id_hash: [1; 32],
        pftl_genesis_hash_commitment: [2; 32],
        pftl_protocol_version: 3,
        prior_checkpoint_commitment: [4; 32],
        resulting_checkpoint_commitment: [5; 32],
        finalized_height: 6,
        proof_nullifier: [7; 32],
    };
    let encoded = values.abi_encode();
    assert_eq!(encoded.len(), 8 * 32);
    assert_eq!(&encoded[4 * 32..5 * 32], &[4; 32]);
    assert_eq!(&encoded[7 * 32..8 * 32], &[7; 32]);
}
