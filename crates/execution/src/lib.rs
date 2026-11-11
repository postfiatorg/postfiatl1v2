#![allow(
    clippy::manual_checked_ops,
    clippy::manual_div_ceil,
    clippy::redundant_field_names,
    clippy::unnecessary_map_or
)]

pub use postfiat_types::pftl_uniswap_route_state_hash;

include!("entrypoints.rs");
include!("recipient_format.rs");
include!("fees_offer_planning.rs");
include!("rejected_fee_claim.rs");
pub mod market_policy {
    include!("market_policy.rs");
}
include!("nav_sp1_verifier.rs");
include!("yolo_collection_verifier.rs");
include!("yolo_target_verifier.rs");
pub mod vault_bridge_policy {
    include!("vault_bridge_policy.rs");
}
include!("nft_escrow_asset_state.rs");
include!("tx_hashing.rs");
include!("owned_transfer.rs");
include!("owned_transfer_recovery.rs");
mod external_lane_release;
pub use external_lane_release::dry_run_external_lane_export;
pub mod fastlane_primary;
pub mod fastswap;
pub mod fastswap_asset_control;
pub mod fastswap_bridge;
pub mod fastswap_checkpoint;
pub mod fastswap_control;
pub mod fastswap_decision;
mod pftl_uniswap_ethereum_verification;

#[cfg(test)]
mod tests {
    include!("tests.rs");
}

#[cfg(test)]
mod lock_minimum_binding_tests;

include!("pftl_source_settlement.rs");
