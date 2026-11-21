//! SP1 guest for the key table witness.
#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    let witness = sp1_zkvm::io::read_vec();
    let values = pfusdc_sol_ingress_program::verify_key_table_witness(&witness)
        .expect("invalid key table witness");
    sp1_zkvm::io::commit_slice(&values.encode());
}
