#![no_main]

use libfuzzer_sys::fuzz_target;
use reserve_proof_types::{execute_reserve_proof, ReserveProofWitnessV1, MAX_WITNESS_BYTES};

fuzz_target!(|data: &[u8]| {
    if data.len() > MAX_WITNESS_BYTES {
        return;
    }
    if let Ok(witness) = serde_cbor::from_slice::<ReserveProofWitnessV1>(data) {
        let _ = execute_reserve_proof(&witness);
    }
});
