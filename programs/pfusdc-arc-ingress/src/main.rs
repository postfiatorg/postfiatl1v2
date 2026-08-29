#![no_main]

#[no_mangle]
pub extern "C" fn main() -> i32 {
    let witness_bytes = sp1_zkvm::io::read_vec();
    let witness: pfusdc_arc_ingress_program::ArcIngressWitnessV1 =
        serde_cbor::from_slice(&witness_bytes)
            .expect("Arc ingress witness must use the versioned CBOR transport");
    let public_values = pfusdc_arc_ingress_program::verify_arc_ingress_witness_v1(&witness)
        .expect("invalid pfUSDC Arc ingress witness");
    sp1_zkvm::io::commit_slice(&public_values.canonical_bytes());
    0
}
