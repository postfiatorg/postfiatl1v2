#![no_main]

extern crate alloc;

use reserve_proof_types::yolo_collection::{
    execute_yolo_collection_proof, YoloCollectionProofWitnessV1,
    YOLO_MAX_COLLECTION_WITNESS_BYTES_V1,
};

/// Commit only the fixed-width PFTL collection ABI. Licensed chain payloads
/// remain private witness data and are not journaled by the guest.
#[no_mangle]
pub extern "C" fn main() -> i32 {
    let encoded_witness = sp1_zkvm::io::read_vec();
    if encoded_witness.len() > YOLO_MAX_COLLECTION_WITNESS_BYTES_V1 {
        sp1_zkvm::io::commit_slice(b"YOLO collection witness exceeds its bounded maximum");
        sp1_zkvm::lib::halt_invalid_hint();
    }
    let witness: YoloCollectionProofWitnessV1 = match serde_cbor::from_slice(&encoded_witness) {
        Ok(value) => value,
        Err(_) => {
            sp1_zkvm::io::commit_slice(b"YOLO collection witness is not valid CBOR");
            sp1_zkvm::lib::halt_invalid_hint();
        }
    };
    let values = match execute_yolo_collection_proof(&witness) {
        Ok(value) => value,
        Err(_) => {
            sp1_zkvm::io::commit_slice(b"YOLO collection witness is incomplete or inconsistent");
            sp1_zkvm::lib::halt_invalid_hint();
        }
    };
    let public_values = match values.encode() {
        Ok(value) => value,
        Err(_) => {
            sp1_zkvm::io::commit_slice(b"YOLO collection public values are not canonical");
            sp1_zkvm::lib::halt_invalid_hint();
        }
    };
    sp1_zkvm::io::commit_slice(&public_values);
    0
}
