#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    let witness: pfusdc_ingress_program::PfUsdcIngressProofWitnessV1 = sp1_zkvm::io::read();
    let public_values = pfusdc_ingress_program::verify_ingress_witness_v1(&witness)
        .expect("invalid pfUSDC Tier-4 ingress witness");
    let bytes = public_values
        .canonical_bytes_without_commitment()
        .expect("pfUSDC ingress public values must encode canonically");
    sp1_zkvm::io::commit_slice(&bytes);
}
