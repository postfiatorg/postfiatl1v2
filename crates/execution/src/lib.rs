#![allow(
    clippy::manual_checked_ops,
    clippy::manual_div_ceil,
    clippy::redundant_field_names,
    clippy::unnecessary_map_or
)]

include!("entrypoints.rs");
include!("fees_offer_planning.rs");
pub mod market_policy {
    include!("market_policy.rs");
}
include!("nav_sp1_verifier.rs");
pub mod vault_bridge_policy {
    include!("vault_bridge_policy.rs");
}
include!("nft_escrow_asset_state.rs");
include!("tx_hashing.rs");
include!("owned_transfer.rs");
include!("owned_transfer_recovery.rs");
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
