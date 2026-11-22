//! SP1 guest: reads one deposit witness, commits the public values.
#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    let witness = sp1_zkvm::io::read_vec();
    match pfusdc_sol_ingress_program::verify_deposit_witness(&witness) {
        Ok(values) => sp1_zkvm::io::commit_slice(&values.encode()),
        Err(error) => panic!("pfusdc-sol-ingress: {error:?}"),
    }
}
