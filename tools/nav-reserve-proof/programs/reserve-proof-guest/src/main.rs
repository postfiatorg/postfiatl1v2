#![no_main]

extern crate alloc;

use reserve_proof_types::{
    execute_reserve_proof, ReserveProofWitnessV1, MAX_WITNESS_BYTES,
};

/// The guest commits the exact fixed-width ABI consumed by PostFiat L1.
#[no_mangle]
pub extern "C" fn main() -> i32 {
    let encoded_witness = sp1_zkvm::io::read_vec();
    if encoded_witness.len() > MAX_WITNESS_BYTES {
        sp1_zkvm::io::commit_slice(b"reserve witness exceeds its bounded maximum");
        sp1_zkvm::lib::halt_invalid_hint();
    }
    let witness: ReserveProofWitnessV1 = match serde_cbor::from_slice(&encoded_witness) {
        Ok(value) => value,
        Err(_) => {
            sp1_zkvm::io::commit_slice(b"reserve witness is not canonical CBOR");
            sp1_zkvm::lib::halt_invalid_hint();
        }
    };
    let values = match execute_reserve_proof(&witness) {
        Ok(value) => value,
        Err(_) => {
            sp1_zkvm::io::commit_slice(b"reserve witness violates its manifest or context");
            sp1_zkvm::lib::halt_invalid_hint();
        }
    };
    let public_values = match values.encode() {
        Ok(value) => value,
        Err(_) => {
            sp1_zkvm::io::commit_slice(b"reserve public values are not canonical");
            sp1_zkvm::lib::halt_invalid_hint();
        }
    };
    sp1_zkvm::io::commit_slice(&public_values);
    0
}
