#![no_main]
sp1_zkvm::entrypoint!(main);

pub fn main() {
    let input = sp1_zkvm::io::read_vec();
    let journal = pfusdc_sol_ingress_program::shards::leaf(&input).expect("invalid shard");
    sp1_zkvm::io::commit_slice(&journal);
}
