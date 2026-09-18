#![no_main]

#[no_mangle]
pub extern "C" fn main() -> i32 {
    let bytes = sp1_zkvm::io::read_vec();
    let witness: pfusdc_eth_mainnet_ingress_program::EthIngressWitnessV1 =
        serde_cbor::from_slice(&bytes).expect("versioned pfETH Ethereum ingress witness");
    let values = pfusdc_eth_mainnet_ingress_program::verify_weth_witness(&witness)
        .expect("invalid WETH-backed Ethereum L1 ingress witness");
    sp1_zkvm::io::commit_slice(
        &serde_cbor::to_vec(&values).expect("canonical pfETH Ethereum ingress public values"),
    );
    0
}
