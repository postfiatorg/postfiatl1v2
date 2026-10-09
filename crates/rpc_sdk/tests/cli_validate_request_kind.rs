//! `validate-request` must check a request's params against its own method,
//! not only when the operator repeats the method as `--expect-kind`.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const HEX96: &str =
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn sdk() -> Command {
    Command::new(env!("CARGO_BIN_EXE_postfiat-rpc-sdk"))
}

fn request_file(name: &str, body: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "postfiat-rpc-sdk-validate-{}-{name}.json",
        std::process::id()
    ));
    fs::write(&path, body).expect("write request fixture");
    path
}

fn run(args: &[&str]) -> Output {
    sdk().args(args).output().expect("run postfiat-rpc-sdk")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

fn envelope(id: &str, method: &str, params: &str) -> String {
    format!(
        "{{\"version\": \"postfiat-local-rpc-v1\", \"id\": \"{id}\", \"method\": \"{method}\", \"params\": {params}}}"
    )
}

#[test]
fn validate_request_checks_params_from_the_file_method() {
    let valid = request_file(
        "valid-tx",
        &envelope("tx-1", "tx", &format!("{{\"tx_id\": \"{HEX96}\"}}")),
    );
    let path = valid.to_string_lossy().into_owned();
    let without = run(&["validate-request", "--input", &path, "--expect-id", "tx-1"]);
    assert!(without.status.success(), "{}", text(&without.stderr));
    assert!(text(&without.stdout).contains("rpc_request=ok id=tx-1 method=tx"));
    assert!(!text(&without.stdout).contains("params=unchecked"));
    let with = run(&[
        "validate-request",
        "--input",
        &path,
        "--expect-id",
        "tx-1",
        "--expect-kind",
        "tx",
    ]);
    assert!(with.status.success(), "{}", text(&with.stderr));
    let _ = fs::remove_file(&valid);
}

#[test]
fn validate_request_rejects_malformed_params_without_expect_kind() {
    for (name, method, params, field) in [
        ("tx-bad-id", "tx", "{\"tx_id\": \"zz\"}", "tx_id"),
        ("blocks-zero-limit", "blocks", "{\"limit\": 0}", "limit"),
        (
            "account-tx-limit",
            "account_tx",
            "{\"address\": \"pfabc\", \"limit\": 999999}",
            "limit",
        ),
    ] {
        let file = request_file(name, &envelope("req-1", method, params));
        let path = file.to_string_lossy().into_owned();
        let output = run(&["validate-request", "--input", &path]);
        assert!(!output.status.success(), "{name}: must be rejected");
        assert_eq!(output.status.code(), Some(1), "{name}: exit code");
        let stderr = text(&output.stderr);
        assert!(
            stderr.contains("request validation failed at"),
            "{name}: {stderr}"
        );
        assert!(
            stderr.contains(field),
            "{name}: expected the error to name `{field}`: {stderr}"
        );
        let _ = fs::remove_file(&file);
    }
}

#[test]
fn validate_request_expect_kind_still_asserts_the_method() {
    let file = request_file(
        "tx-for-blocks",
        &envelope("tx-1", "tx", &format!("{{\"tx_id\": \"{HEX96}\"}}")),
    );
    let path = file.to_string_lossy().into_owned();
    let output = run(&[
        "validate-request",
        "--input",
        &path,
        "--expect-kind",
        "blocks",
    ]);
    assert!(!output.status.success());
    let stderr = text(&output.stderr);
    assert!(
        stderr.contains("blocks") && stderr.contains("tx"),
        "{stderr}"
    );
    let _ = fs::remove_file(&file);
}

#[test]
fn validate_request_reports_unchecked_params_for_an_unmapped_method() {
    let file = request_file("unmapped", &envelope("x-1", "no_such_method", "{}"));
    let path = file.to_string_lossy().into_owned();
    let output = run(&["validate-request", "--input", &path]);
    let stdout = text(&output.stdout);
    let stderr = text(&output.stderr);
    if output.status.success() {
        assert!(stdout.contains("params=unchecked"), "{stdout}");
    } else {
        // The protocol envelope itself may refuse an unknown method; either
        // outcome keeps the gap visible rather than silently passing params.
        assert!(
            stderr.contains("no_such_method") || stderr.contains("method"),
            "{stderr}"
        );
    }
    let _ = fs::remove_file(&file);
}
