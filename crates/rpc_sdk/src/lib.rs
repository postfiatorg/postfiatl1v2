#![allow(clippy::manual_is_multiple_of, clippy::needless_borrow)]

include!("protocol_requests.rs");
include!("response_validation.rs");

mod wallet_sdk;
pub use wallet_sdk::*;
mod atomic_swap_wallet;
pub use atomic_swap_wallet::*;
mod fastswap_wallet;
pub use fastswap_wallet::*;
mod fastswap_session;
pub use fastswap_session::*;
mod fastswap_client;
pub use fastswap_client::*;
mod fastswap_tcp;
pub use fastswap_tcp::*;
mod fastswap_signing;
pub use fastswap_signing::*;
mod fastswap_wire;
pub use fastswap_wire::*;
#[cfg(test)]
use wallet_sdk::{owned_transfer_signing_bytes, owned_unwrap_signing_bytes};

#[cfg(test)]
mod tests {
    include!("protocol_request_tests.rs");
    include!("response_validation_tests.rs");
    include!("atomic_swap_wallet_tests.rs");
}
