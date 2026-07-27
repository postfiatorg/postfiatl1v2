#![no_main]

#[no_mangle]
pub extern "C" fn main() -> i32 {
    let bytes = sp1_zkvm::io::read_vec();
    let witness: pfusdc_eth_ingress_program::EthIngressWitnessV1 =
        serde_cbor::from_slice(&bytes).expect("versioned Ethereum ingress witness");
    let values = pfusdc_eth_ingress_program::verify_witness(&witness)
        .expect("invalid Ethereum L1 ingress witness");
    sp1_zkvm::io::commit_slice(
        &serde_cbor::to_vec(&values).expect("canonical Ethereum ingress public values"),
    );
    0
}
