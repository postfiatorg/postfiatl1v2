#![allow(clippy::large_enum_variant, clippy::too_many_arguments)]

include!("core_chain.rs");
include!("ledger_assets.rs");
include!("fastswap_types.rs");
include!("ethereum_bridge_types.rs");
include!("consensus_v2_types.rs");
include!("shielded_bridge_governance.rs");
include!("transactions_mempool_receipts.rs");
include!("pfusdc_tier4_types.rs");

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
