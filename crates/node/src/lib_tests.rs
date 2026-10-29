#[cfg(test)]
mod tests {
    include!("tests/helpers_orchard_transfer_escrow.rs");
    include!("tests/asset_orchard_issued_tests.rs");
    include!("tests/asset_nft_offer_tests.rs");
    include!("tests/fx_fix_query_tests.rs");
    include!("tests/asset_orchard_pricing_read_tests.rs");
    include!("tests/asset_orchard_pricing_enforcement_tests.rs");
    include!("tests/private_pool_read_tests.rs");
    include!("tests/private_readiness_read_tests.rs");
    include!("tests/yolo_target_receipt_tests.rs");
    include!("tests/nav_reserve_proof_status_tests.rs");
    include!("tests/private_action_admission_tests.rs");
    mod a666_recovery_regression_tests;
    mod account_tx_chunked_index;
    mod account_tx_commit_index;
    mod account_tx_exchange_history;
    mod account_tx_truncation;
    mod archive_bridge_supply;
    mod consensus_history;
    mod atomic_swap_consensus;
    mod fastpay_payment_safety;
    mod fastpay_non_signer_anchor;
    mod signer_committee_rotation_dry_run;
    mod fastpay_commitment_versions;
    mod orchard_transfer_escrow;
    mod replicated_state_activation;
    mod rejected_fee_require_memo;
    mod lifecycle_checkpoint_tests;
    mod private_primary_replay_gate_tests;
    mod recipient_format_activation;
    mod snapshot_deployment;
    mod validator_registry_continuation_tests;
    mod vault_bridge_governed_route;
    use consensus_history::{dummy_block_record, write_split_validator_key_files};
    include!("tests/governance_history_manifest_tests.rs");
    include!("tests/pftl_uniswap_bridge_rpc_tests.rs");

    #[test]
    fn private_primary_receipt_messages_preserve_a666_archive_replay() {
        assert_eq!(
            super::private_primary_receipt_message(
                "pftl-a666-ethereum-wA666-usdc-v1",
                false,
            ),
            "private pfUSDC was atomically consumed by the governed primary route and encrypted A666 was issued"
        );
        assert_eq!(
            super::private_primary_receipt_message(
                "pftl-a666-ethereum-wA666-usdc-v1",
                true,
            ),
            "private A666 was atomically retired by the governed primary route and encrypted pfUSDC was issued"
        );
        assert_eq!(
            super::private_primary_receipt_message("pftl-navcoin-generic-v1", false),
            "private settlement asset was atomically consumed by the governed primary route and an encrypted NAVCoin was issued"
        );
    }
}
