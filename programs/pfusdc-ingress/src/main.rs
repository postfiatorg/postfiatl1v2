#![no_main]

/// SP1's runtime calls this C-ABI symbol from `__start`. Keep the call
/// unconditional: a proof ELF that links but skips the statement is invalid.
#[no_mangle]
pub extern "C" fn main() -> i32 {
    // Keep the proof boundary independent of SP1's internal bincode version.
    let witness_bytes = sp1_zkvm::io::read_vec();
    let witness: pfusdc_ingress_program::PfUsdcIngressProofWitnessV1 =
        serde_cbor::from_slice(&witness_bytes)
            .expect("pfUSDC ingress witness must use the versioned CBOR transport");
    let public_values = pfusdc_ingress_program::verify_ingress_witness_v1(&witness)
        .expect("invalid pfUSDC Tier-4 ingress witness");
    let bytes = public_values
        .canonical_bytes_without_commitment()
        .expect("pfUSDC ingress public values must encode canonically");
    sp1_zkvm::io::commit_slice(&bytes);
    0
}
