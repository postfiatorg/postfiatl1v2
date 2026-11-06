//! The two-user units on a local chain. `transport-validator-serve
//! --finality-signer-socket` refuses a key file beside the socket.

use std::process::Command;

fn node_bin() -> String {
    std::env::var("POSTFIAT_SIGNER_TEST_NODE")
        .unwrap_or_else(|_| env!("CARGO_BIN_EXE_postfiat-node").to_string())
}

/// `--finality-signer-socket` replaces the key file: both together are
/// refused before anything else is read.
#[test]
fn transport_validator_serve_refuses_a_key_file_beside_the_signer_socket() {
    for extra in [
        &["--key-file", "/nonexistent/validator_keys.json"][..],
        &["--unsafe-devnet-file-signer"][..],
    ] {
        let output = Command::new(node_bin())
            .args([
                "transport-validator-serve",
                "--unsafe-devnet-json-storage",
                "--data-dir",
                "/nonexistent/node",
                "--topology",
                "/nonexistent/topology.json",
                "--vote-dir",
                "/nonexistent/votes",
                "--finality-signer-socket",
                "/nonexistent/validator-0.sock",
            ])
            .args(extra)
            .output()
            .expect("run transport-validator-serve");
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(
            stderr.contains("validator_signer_flags_conflict"),
            "{extra:?}: {stderr}"
        );
    }
}
