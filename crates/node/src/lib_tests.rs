#[cfg(test)]
mod tests {
    include!("tests/helpers_orchard_transfer_escrow.rs");
    include!("tests/asset_orchard_issued_tests.rs");
    include!("tests/asset_nft_offer_tests.rs");
    mod consensus_history;
    mod atomic_swap_consensus;
    mod fastpay_payment_safety;
    mod orchard_transfer_escrow;
    mod replicated_state_activation;
    mod snapshot_deployment;
    mod vault_bridge_governed_route;
    use consensus_history::{dummy_block_record, write_split_validator_key_files};
    include!("tests/governance_history_manifest_tests.rs");
    include!("tests/pftl_uniswap_bridge_rpc_tests.rs");
}
