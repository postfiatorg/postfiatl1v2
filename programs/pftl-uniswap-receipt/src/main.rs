#![no_main]

use postfiat_pftl_uniswap_proofs::{
    verify_pftl_uniswap_checkpoint_witness_v1, verify_pftl_uniswap_receipt_witness_v1,
    PftlUniswapProofInputV1,
};

/// SP1 guest entrypoint. The proof commits the exact receipt or checkpoint
/// static Solidity ABI tuple consumed by `PFTLReceiptFinalityVerifierV1`.
#[no_mangle]
pub extern "C" fn main() -> i32 {
    let witness_bytes = sp1_zkvm::io::read_vec();
    let input = serde_cbor::from_slice::<PftlUniswapProofInputV1>(&witness_bytes)
        .expect("PFTL-Uniswap proof input must use versioned CBOR");
    let encoded = match input {
        PftlUniswapProofInputV1::Receipt(witness) => {
            verify_pftl_uniswap_receipt_witness_v1(&witness)
                .expect("PFTL-Uniswap witness must prove exact receipt finality")
                .abi_encode()
        }
        PftlUniswapProofInputV1::Checkpoint(witness) => {
            verify_pftl_uniswap_checkpoint_witness_v1(&witness)
                .expect("PFTL-Uniswap witness must prove checkpoint finality")
                .abi_encode()
        }
    };
    sp1_zkvm::io::commit_slice(&encoded);
    0
}
