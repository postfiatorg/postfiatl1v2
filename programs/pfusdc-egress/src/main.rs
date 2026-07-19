#![no_main]

use postfiat_pfusdc_proofs::{verify_checkpoint_witness_v1, verify_egress_witness_v1};
use postfiat_types::PfUsdcEgressProgramInputV1;

/// SP1's runtime calls this C-ABI symbol from `__start`. Keep the call
/// unconditional: a proof ELF that links but skips the statement is invalid.
#[no_mangle]
pub extern "C" fn main() -> i32 {
    // Use an explicit, versioned wire encoding instead of coupling the proof
    // statement to SP1's internal bincode transport. The stdin item itself is
    // still length-delimited by the zkVM runtime.
    let witness_bytes = sp1_zkvm::io::read_vec();
    let input = serde_cbor::from_slice::<PfUsdcEgressProgramInputV1>(&witness_bytes)
        .expect("pfUSDC egress witness must use the versioned CBOR transport");
    let bytes = match input {
        PfUsdcEgressProgramInputV1::Withdrawal(witness) => verify_egress_witness_v1(&witness)
            .expect("pfUSDC egress witness must prove exact PFTL finality and bridge exit")
            .canonical_bytes_without_commitment()
            .expect("pfUSDC egress public values must encode canonically"),
        PfUsdcEgressProgramInputV1::Checkpoint(witness) => verify_checkpoint_witness_v1(&witness)
            .expect("pfUSDC checkpoint witness must prove exact PFTL finality")
            .canonical_bytes_without_commitment()
            .expect("pfUSDC checkpoint public values must encode canonically"),
    };
    sp1_zkvm::io::commit_slice(&bytes);
    0
}
