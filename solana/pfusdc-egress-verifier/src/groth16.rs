use hex_literal::hex;
use solana_bn254::prelude::{
    alt_bn128_g1_addition_be, alt_bn128_g1_multiplication_be, alt_bn128_pairing_be,
};
use solana_program_error::ProgramResult;
use solana_sha256_hasher::hash;

use crate::Error;

pub const EGRESS_KEY: [u8; 32] =
    hex!("0015b046ba4b80c0ca7e2d9429a1f5fd88bc6d1d328cca6acec29ffdf48a9d87");
const VK_ROOT: [u8; 32] = hex!("002f850ee998974d6cc00e50cd0814b098c05bfade466d28573240d057f25352");
const SCALAR_MODULUS: [u8; 32] =
    hex!("30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001");
const KEY: &[u8; 832] = include_bytes!("circuit-key.bin");

pub fn verify(values: &[u8], proof: &[u8]) -> ProgramResult {
    if proof.len() != 356
        || proof[..4] != hex!("4388a21c")
        || proof[4..36] != [0; 32]
        || proof[36..68] != VK_ROOT
        || &proof[68..100] >= SCALAR_MODULUS.as_slice()
    {
        return Err(Error::Proof.into());
    }
    let mut digest = hash(values).to_bytes();
    digest[0] &= 0x1f;
    let mut acc = KEY[..64].to_vec();
    // The exit code is pinned to zero. Nonce zero is common; omit zero terms.
    for (i, scalar) in [
        (0, EGRESS_KEY.as_slice()),
        (1, &digest),
        (3, &VK_ROOT),
        (4, &proof[68..100]),
    ] {
        if scalar == [0; 32] {
            continue;
        }
        let offset = 64 * (1 + i);
        let term = alt_bn128_g1_multiplication_be(&[&KEY[offset..offset + 64], scalar].concat())
            .map_err(|_| Error::Proof)?;
        acc = alt_bn128_g1_addition_be(&[acc, term].concat()).map_err(|_| Error::Proof)?;
    }
    // e(A,B) e(C,-delta) e(alpha,-beta) e(acc,-gamma) == 1.
    let pairs = [&proof[100..], &KEY[384..704], &acc, &KEY[704..]].concat();
    let result = alt_bn128_pairing_be(&pairs).map_err(|_| Error::Proof)?;
    if result.get(31) != Some(&1) {
        return Err(Error::Proof.into());
    }
    Ok(())
}
