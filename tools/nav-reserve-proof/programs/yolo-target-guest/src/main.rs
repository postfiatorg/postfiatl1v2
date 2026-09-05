#![no_main]

use reserve_proof_types::yolo_target_proof::execute_yolo_target_proof;
use reserve_proof_types::yolo_witness::{decode_target_witness, MAX_TARGET_WITNESS_BYTES};

/// Only the validated fixed-width public values are journaled. Invalid private
/// inputs halt without exporting input bytes or detailed private error content.
#[no_mangle]
pub extern "C" fn main() -> i32 {
    let length = sp1_zkvm::syscalls::syscall_hint_len();
    if length == 0 || length > MAX_TARGET_WITNESS_BYTES {
        sp1_zkvm::lib::halt_invalid_hint();
    }
    let encoded = sp1_zkvm::io::read_vec();
    let result = decode_target_witness(&encoded)
        .and_then(|witness| execute_yolo_target_proof(&witness))
        .and_then(|public| public.encode());
    match result {
        Ok(public) => sp1_zkvm::io::commit_slice(&public),
        Err(_) => sp1_zkvm::lib::halt_invalid_hint(),
    }
    0
}
