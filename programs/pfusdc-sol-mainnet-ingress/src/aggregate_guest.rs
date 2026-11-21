#![no_main]
use pfusdc_sol_ingress_program::shards;
use sha2::{Digest, Sha256};

sp1_zkvm::entrypoint!(main);

// HashableKey::hash_u32() of the pinned pfusdc-sol-delta-shard ELF.
const LEAF_KEY: [u32; 8] = [378302003, 786075656, 1238870432, 1472516439, 547322397, 446999786, 1289002868, 196127062];

pub fn main() {
    let frames = sp1_zkvm::io::read_vec();
    for journal in shards::frames(&frames).expect("invalid shard frames") {
        sp1_zkvm::lib::verify::verify_sp1_proof(&LEAF_KEY, &Sha256::digest(journal).into());
    }
    let header = sp1_zkvm::io::read_vec();
    let values = shards::aggregate(&header, &frames).expect("invalid deposit or partial sums");
    sp1_zkvm::io::commit_slice(&values.encode());
}
