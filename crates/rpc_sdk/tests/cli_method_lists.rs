//! The help text and the three "unsupported ..." errors must agree with the
//! methods the binary actually builds and validates.

use std::fs;
use std::process::{Command, Output};

const HEX96: &str =
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn run(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_postfiat-rpc-sdk"))
        .args(args)
        .output()
        .expect("run postfiat-rpc-sdk")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// The method names the help text advertises, from both request lists.
fn advertised_methods() -> Vec<String> {
    let help = stderr(&run(&["help"]));
    let mut methods = Vec::new();
    for prefix in ["Supported request methods: ", "FastSwap methods: "] {
        let line = help
            .lines()
            .find(|line| line.starts_with(prefix))
            .unwrap_or_else(|| panic!("help text lacks `{prefix}`"));
        let list = line[prefix.len()..].trim_end_matches('.');
        methods.extend(list.split(", ").map(str::to_string));
    }
    assert!(
        methods.len() > 80,
        "expected the full method list, got {}",
        methods.len()
    );
    methods
}

#[test]
fn every_advertised_method_has_a_request_builder() {
    for method in advertised_methods() {
        let output = run(&[
            "request", "--method", &method, "--id", "t-1", "--output", "-",
        ]);
        let text = stderr(&output);
        assert!(
            !text.contains("unsupported request builder method"),
            "`{method}` is advertised in help but the builder rejects it: {text}"
        );
    }
}

#[test]
fn unknown_method_error_lists_every_supported_method() {
    let output = run(&[
        "request",
        "--method",
        "definitely_not_a_method",
        "--id",
        "t-1",
        "--output",
        "-",
    ]);
    assert!(!output.status.success());
    let text = stderr(&output);
    assert!(
        text.contains("unsupported request builder method `definitely_not_a_method`"),
        "{text}"
    );
    for method in advertised_methods() {
        assert!(
            text.contains(&method),
            "error list omits `{method}`: {text}"
        );
    }
}

#[test]
fn unknown_kind_errors_list_every_supported_method() {
    let path = std::env::temp_dir().join(format!(
        "postfiat-rpc-sdk-lists-{}.json",
        std::process::id()
    ));
    fs::write(
        &path,
        format!(
            "{{\"version\": \"postfiat-local-rpc-v1\", \"id\": \"tx-1\", \"method\": \"tx\", \"params\": {{\"tx_id\": \"{HEX96}\"}}}}"
        ),
    )
    .expect("write request fixture");
    let input = path.to_string_lossy().into_owned();
    for subcommand in ["validate-request", "validate-response"] {
        let output = run(&[
            subcommand,
            "--input",
            &input,
            "--expect-kind",
            "definitely_not_a_kind",
        ]);
        assert!(!output.status.success());
        let text = stderr(&output);
        assert!(
            text.contains("definitely_not_a_kind"),
            "{subcommand}: {text}"
        );
        for method in [
            "account_tx",
            "fastswap_status",
            "tx",
            "nav_reserve_proof_status",
        ] {
            // nav_reserve_proof_status is only in the list once #76 lands; skip
            // it unless the help advertises it.
            if method == "nav_reserve_proof_status"
                && !advertised_methods().iter().any(|m| m == method)
            {
                continue;
            }
            assert!(
                text.contains(method),
                "{subcommand}: kind error omits `{method}`: {text}"
            );
        }
    }
    let _ = fs::remove_file(&path);
}
