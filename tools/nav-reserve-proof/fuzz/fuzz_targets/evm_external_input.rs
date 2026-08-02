#![no_main]

use libfuzzer_sys::fuzz_target;
use postfiat_reserve_proof::external_input_fuzz;

fuzz_target!(|data: &[u8]| external_input_fuzz::evm(data));
