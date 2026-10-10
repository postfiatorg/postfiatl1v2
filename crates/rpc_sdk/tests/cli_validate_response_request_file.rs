//! Built-binary coverage for `validate-response --request-file` binding on
//! every response kind, not only the request-bound atomic-swap kinds.
use std::fs;
use std::path::PathBuf;
use std::process::{Command, Output};

use postfiat_rpc_sdk::{status_request, success_response, write_request_file, write_response_file};
use serde_json::{json, Value};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_postfiat-rpc-sdk"))
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "postfiat-rpc-sdk-validate-response-request-file-{}-{name}",
        std::process::id()
    ));
    path
}

fn status_result() -> Value {
    json!({
        "chain_id": "rpc-sdk-cli-tests",
        "genesis_hash": "11".repeat(48),
        "protocol_version": 1,
        "validator_count": 6,
        "node_id": "node-0",
        "status": "running",
        "last_run_unix": 1,
        "state_root": "22".repeat(48),
        "block_height": 0,
        "block_tip_hash": "genesis",
        "mempool_pending": 0
    })
}

struct Fixture {
    response_path: PathBuf,
    request_path: PathBuf,
    other_request_path: PathBuf,
}

impl Fixture {
    fn write(tag: &str, result: &Value) -> Self {
        let fixture = Self {
            response_path: temp_path(&format!("{tag}-response.json")),
            request_path: temp_path(&format!("{tag}-request.json")),
            other_request_path: temp_path(&format!("{tag}-other-request.json")),
        };
        let response = success_response("status-1", result, vec![]).expect("build response");
        write_response_file(&fixture.response_path, &response).expect("write response");
        write_request_file(&fixture.request_path, &status_request("status-1"))
            .expect("write request");
        write_request_file(&fixture.other_request_path, &status_request("status-2"))
            .expect("write other request");
        fixture
    }

    fn validate(&self, extra: &[&str]) -> Output {
        bin()
            .args(["validate-response", "--input"])
            .arg(&self.response_path)
            .args(extra)
            .output()
            .expect("run validate-response")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        for path in [
            &self.response_path,
            &self.request_path,
            &self.other_request_path,
        ] {
            let _ = fs::remove_file(path);
        }
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn request_file_with_matching_id_passes_and_agrees_with_expect_kind() {
    let fixture = Fixture::write("matching", &status_result());
    let request = fixture.request_path.display().to_string();
    let output = fixture.validate(&["--request-file", &request]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(String::from_utf8_lossy(&output.stdout).contains("rpc_response=ok id=status-1"));
    let output = fixture.validate(&["--expect-kind", "status", "--request-file", &request]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let output = fixture.validate(&["--expect-id", "status-1", "--request-file", &request]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
}

#[test]
fn request_file_rejects_a_response_for_another_request_id() {
    let fixture = Fixture::write("other-id", &status_result());
    let other = fixture.other_request_path.display().to_string();
    let output = fixture.validate(&["--request-file", &other]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("rpc response id `status-1` did not match expected `status-2`"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn expect_id_conflicting_with_request_file_is_rejected() {
    let fixture = Fixture::write("conflict", &status_result());
    let request = fixture.request_path.display().to_string();
    let output = fixture.validate(&["--expect-id", "status-2", "--request-file", &request]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains(
            "--expect-id `status-2` conflicts with request id `status-1` in --request-file"
        ),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn request_file_derives_the_expected_kind_from_the_request_method() {
    // Not a status result: without a kind nothing inspects it, with the
    // request file the status validator runs and names the missing field.
    let fixture = Fixture::write("derived", &json!({ "schema": "not-a-status-result" }));
    let output = fixture.validate(&[]);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let request = fixture.request_path.display().to_string();
    let output = fixture.validate(&["--request-file", &request]);
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("chain_id"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn expect_kind_must_match_the_request_method() {
    let fixture = Fixture::write("kind-mismatch", &status_result());
    let request = fixture.request_path.display().to_string();
    let output = fixture.validate(&["--expect-kind", "tx", "--request-file", &request]);
    assert!(!output.status.success());
    assert!(
        stderr(&output)
            .contains("--expect-kind does not match request method `status` in --request-file"),
        "stderr: {}",
        stderr(&output)
    );
}

#[test]
fn help_describes_request_file_binding() {
    let output = bin().arg("help").output().expect("run help");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("Any validate-response call given --request-file is bound to that request"),
        "help text: {text}"
    );
}
