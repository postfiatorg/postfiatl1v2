#![no_main]

use postfiat_pft_lane_proofs::{
    verify_external_lane_checkpoint_witness_v1, verify_external_lane_export_witness_v1,
};
use postfiat_types::ExternalLaneExportProgramInputV1;

/// SP1's runtime calls this C-ABI symbol from `__start`. Keep the call
/// unconditional: a proof ELF that links but skips the statement is invalid.
#[no_mangle]
pub extern "C" fn main() -> i32 {
    // The versioned CBOR input, as the pfUSDC egress program reads its own.
    let witness_bytes = sp1_zkvm::io::read_vec();
    let input = serde_cbor::from_slice::<ExternalLaneExportProgramInputV1>(&witness_bytes)
        .expect("lane export witness must use the versioned CBOR transport");
    let bytes = match input {
        ExternalLaneExportProgramInputV1::Withdrawal(witness) => {
            verify_external_lane_export_witness_v1(&witness)
                .expect("lane export witness must prove PFTL finality and the export")
                .canonical_bytes_without_commitment()
                .expect("lane export public values must encode canonically")
        }
        ExternalLaneExportProgramInputV1::Checkpoint(witness) => {
            verify_external_lane_checkpoint_witness_v1(&witness)
                .expect("checkpoint witness must prove exact PFTL finality")
                .canonical_bytes_without_commitment()
                .expect("checkpoint public values must encode canonically")
        }
    };
    sp1_zkvm::io::commit_slice(&bytes);
    0
}
