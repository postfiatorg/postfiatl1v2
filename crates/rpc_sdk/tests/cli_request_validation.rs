//! The `request` subcommand must not write a request that the binary's own
//! `validate-request --expect-kind` would reject.

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

const HEX96: &str =
    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

fn sdk() -> Command {
    Command::new(env!("CARGO_BIN_EXE_postfiat-rpc-sdk"))
}

fn scratch(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "postfiat-rpc-sdk-cli-{}-{name}.json",
        std::process::id()
    ));
    let _ = fs::remove_file(&path);
    path
}

fn run(args: &[&str]) -> Output {
    sdk().args(args).output().expect("run postfiat-rpc-sdk")
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn request_writes_a_request_that_passes_its_own_validation() {
    let output_path = scratch("valid-tx");
    let output_arg = output_path.to_string_lossy().into_owned();
    let written = run(&[
        "request",
        "--method",
        "tx",
        "--id",
        "tx-1",
        "--output",
        &output_arg,
        "--tx-id",
        HEX96,
    ]);
    assert!(written.status.success(), "{}", stderr(&written));
    assert!(output_path.is_file(), "request file must be written");

    let validated = run(&[
        "validate-request",
        "--input",
        &output_arg,
        "--expect-id",
        "tx-1",
        "--expect-kind",
        "tx",
    ]);
    assert!(validated.status.success(), "{}", stderr(&validated));
    assert!(String::from_utf8_lossy(&validated.stdout).contains("rpc_request=ok id=tx-1 method=tx"));
    let _ = fs::remove_file(&output_path);
}

#[test]
fn request_refuses_params_its_own_validator_rejects_and_writes_nothing() {
    for (name, args, field) in [
        (
            "tx-bad-id",
            vec!["--method", "tx", "--id", "tx-1", "--tx-id", "zz"],
            "tx_id",
        ),
        (
            "blocks-zero-limit",
            vec!["--method", "blocks", "--id", "b-1", "--limit", "0"],
            "limit",
        ),
        (
            "account-tx-limit",
            vec![
                "--method",
                "account_tx",
                "--id",
                "a-1",
                "--address",
                "pfabc",
                "--limit",
                "999999",
            ],
            "limit",
        ),
    ] {
        let output_path = scratch(name);
        let output_arg = output_path.to_string_lossy().into_owned();
        let mut full = vec!["request"];
        full.extend(args);
        full.extend(["--output", &output_arg]);
        let output = run(&full);
        assert!(
            !output.status.success(),
            "{name}: a rejected request must fail"
        );
        assert_eq!(output.status.code(), Some(1), "{name}: exit code");
        let text = stderr(&output);
        assert!(
            text.contains("request validation failed for `"),
            "{name}: {text}"
        );
        assert!(
            text.contains(field),
            "{name}: expected the error to name `{field}`: {text}"
        );
        assert!(
            !output_path.exists(),
            "{name}: no request file may be written"
        );
    }
}

#[test]
fn request_to_stdout_is_validated_too() {
    let output = run(&[
        "request", "--method", "blocks", "--id", "b-1", "--output", "-", "--limit", "0",
    ]);
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stdout).trim().is_empty(),
        "nothing may be printed for a rejected request"
    );
    assert!(stderr(&output).contains("limit"));
}

#[test]
fn request_still_reports_missing_required_flags_first() {
    let output_path = scratch("tx-missing");
    let output_arg = output_path.to_string_lossy().into_owned();
    let output = run(&[
        "request",
        "--method",
        "tx",
        "--id",
        "tx-1",
        "--output",
        &output_arg,
    ]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("missing --tx-id"),
        "{}",
        stderr(&output)
    );
    assert!(!output_path.exists());
}
