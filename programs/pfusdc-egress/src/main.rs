#![no_main]

sp1_zkvm::entrypoint!(main);

use postfiat_pfusdc_proofs::verify_egress_witness_v1;
use postfiat_types::PfUsdcEgressProofWitnessV1;

pub fn main() {
    let witness = sp1_zkvm::io::read::<PfUsdcEgressProofWitnessV1>();
    let public_values = verify_egress_witness_v1(&witness)
        .expect("pfUSDC egress witness must prove exact PFTL finality and bridge exit");
    let bytes = public_values
        .canonical_bytes_without_commitment()
        .expect("pfUSDC egress public values must encode canonically");
    sp1_zkvm::io::commit_slice(&bytes);
}
