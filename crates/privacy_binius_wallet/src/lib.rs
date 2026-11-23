//! User-owned PFSH02 wallet. No networking or validator-side key handling.
pub mod keys;
pub mod storage;
pub mod tree;
pub mod wallet;
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
pub mod service;
