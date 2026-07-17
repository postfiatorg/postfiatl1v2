// SPDX-License-Identifier: MIT OR Apache-2.0
pragma solidity ^0.8.24;

/// @notice PFTL-finalized authorization packet for bounded NAVCoin market operations.
/// @dev Field order mirrors postfiat_types::MarketOpsEnvelope exactly. Solidity
///      address fields represent the Rust [u8; 20] adapter/vault/controller fields.
struct MarketOpsEnvelope {
    uint32 encoding_version;
    uint64 chain_id;
    address adapter_address;
    address vault_address;
    address mint_controller_address;
    bytes32 asset_id;
    uint64 epoch;
    bytes32 program_id;
    bytes32 policy_hash;
    bytes32 parameter_hash;
    bytes32 reserve_packet_hash;
    bytes32 supply_packet_hash;
    bytes32 evidence_root;
    bytes32 previous_market_state_hash;
    bytes32 venue_id;
    bytes32 pool_config_hash;
    bytes32 hook_code_hash;
    uint256 nav_floor_usd_e8;
    uint256 valid_global_supply_atoms;
    uint256 verified_net_assets_usd_e8;
    uint256 funded_alignment_reserve_usd_e8;
    uint256 required_alignment_reserve_usd_e8;
    uint256 max_reserve_deploy_usd_e8;
    uint256 max_mint_atoms;
    uint32 discount_trigger_bps;
    uint32 premium_trigger_bps;
    uint64 data_window_start;
    uint64 data_window_end;
    uint64 valid_after;
    uint64 expires_at;
    uint64 cooldown_seconds;
    bytes32 nonce;
}
