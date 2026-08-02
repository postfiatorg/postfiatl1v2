//! Public, provider-neutral NAV reserve collection library.
//!
//! The command-line binary is a thin caller of this library so external-input
//! parsing and collection logic can be exercised directly by fuzzing and
//! independent integrators. Private operator software is not part of this
//! boundary.

mod evm_adapter;
mod hyperliquid_adapter;
mod monero_adapter;
mod near_adapter;
mod solana_adapter;
mod source_checkpoint;

pub(crate) use evm_adapter::{read_json, write_new};

pub use evm_adapter::{run as run_adapter, AdapterCommand};
pub use source_checkpoint::{run as run_source_checkpoint, SourceCheckpointCommand};

/// Direct entry points for coverage-guided testing of attacker-controlled
/// source formats. These functions do not perform network access or mutate
/// state.
pub mod external_input_fuzz {
    /// Exercise EVM JSON-RPC quantities, hex data, state-proof nodes, and
    /// account/storage proof decoding.
    pub fn evm(data: &[u8]) {
        crate::evm_adapter::fuzz_external_input(data);
    }

    /// Exercise HyperEVM block-header, receipt, log, quantity, and data
    /// decoding plus receipt-trie construction.
    pub fn hyperliquid(data: &[u8]) {
        crate::hyperliquid_adapter::fuzz_external_input(data);
    }

    /// Exercise NEAR base58 hashes, block/header and light-client proof JSON,
    /// and Merkle-proof reconstruction.
    pub fn near(data: &[u8]) {
        crate::near_adapter::fuzz_external_input(data);
    }

    /// Exercise Solana immutable-program metadata, reader payload, and legacy
    /// transaction/shortvec parsing.
    pub fn solana(data: &[u8]) {
        crate::solana_adapter::fuzz_external_input(data);
    }

    /// Exercise Monero ReserveProofV2 text/base58 and canonical binary parsing.
    pub fn monero(data: &[u8]) {
        crate::monero_adapter::fuzz_external_input(data);
    }

    /// Exercise provider-neutral source-checkpoint committee, checkpoint,
    /// vote, and certificate decoding and validation.
    pub fn source_checkpoint(data: &[u8]) {
        crate::source_checkpoint::fuzz_external_input(data);
    }
}
