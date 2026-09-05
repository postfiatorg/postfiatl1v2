#![no_main]

//! Qualification harness only. This is not a target-receipt program and must
//! never be registered as one. It exercises the actual patched Nitro verifier
//! on the complete shared positive/negative corpus, without expected answers.

use reserve_proof_types::yolo_cbor::{decode_strict_cbor, CborLimits};
use reserve_proof_types::yolo_nitro::verify_nitro_document;
use reserve_proof_types::yolo_witness::NitroProofPolicyV1;

type Case = (Vec<u8>, Vec<u8>, NitroProofPolicyV1, u64, String);
const MAX_BYTES: usize = 4 * 1024 * 1024;

fn execute(encoded: &[u8]) -> Result<Vec<u8>, String> {
    let value = decode_strict_cbor(
        encoded,
        CborLimits {
            bytes: MAX_BYTES,
            items: 100_000,
            depth: 32,
        },
    )?;
    let cases: Vec<Case> =
        serde_cbor::value::from_value(value).map_err(|_| "invalid test corpus")?;
    if cases.is_empty() || cases.len() > 64 {
        return Err("invalid case count".into());
    }
    Ok(cases
        .into_iter()
        .map(|(document, root, policy, time, digest)| {
            u8::from(verify_nitro_document(&document, &root, &policy, time, &digest).is_ok())
        })
        .collect())
}

#[no_mangle]
pub extern "C" fn main() -> i32 {
    let length = sp1_zkvm::syscalls::syscall_hint_len();
    if length == 0 || length > MAX_BYTES {
        sp1_zkvm::lib::halt_invalid_hint();
    }
    match execute(&sp1_zkvm::io::read_vec()) {
        Ok(results) => sp1_zkvm::io::commit_slice(&results),
        Err(_) => sp1_zkvm::lib::halt_invalid_hint(),
    }
    0
}
