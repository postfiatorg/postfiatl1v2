#![no_main]
use reserve_proof_types::b23_nav::{decode, execute, MAX_WITNESS};

#[no_mangle]
pub extern "C" fn main() -> i32 {
    let length = sp1_zkvm::syscalls::syscall_hint_len();
    if length == 0 || length > MAX_WITNESS {
        sp1_zkvm::lib::halt_invalid_hint();
    }
    let result = decode(&sp1_zkvm::io::read_vec())
        .and_then(|w| execute(&w))
        .and_then(|v| v.encode());
    match result {
        Ok(bytes) => sp1_zkvm::io::commit_slice(&bytes),
        Err(_) => sp1_zkvm::lib::halt_invalid_hint(),
    }
    0
}
