//! Built-binary coverage for the `owned_recovery_status` CLI wiring.
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_postfiat-rpc-sdk"))
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "postfiat-rpc-sdk-owned-recovery-status-{}-{name}",
        std::process::id()
    ));
    path
}

#[test]
fn request_subcommand_builds_owned_recovery_status_request() {
    let output_path = temp_path("request.json");
    let lock_id = "ab".repeat(48);
    let output = bin()
        .args([
            "request",
            "--method",
            "owned_recovery_status",
            "--id",
            "owned-recovery-status-cli-1",
            "--lock-id",
            lock_id.as_str(),
            "--output",
        ])
        .arg(&output_path)
        .output()
        .expect("run request");
    assert!(
        output.status.success(),
        "request failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let written: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&output_path).expect("read request"))
            .expect("parse request");
    assert_eq!(written["method"], "owned_recovery_status");
    assert_eq!(written["id"], "owned-recovery-status-cli-1");
    assert_eq!(written["params"]["lock_id"], lock_id);
    let _ = fs::remove_file(&output_path);
}

#[test]
fn request_subcommand_requires_lock_id() {
    let output_path = temp_path("missing.json");
    let output = bin()
        .args([
            "request",
            "--method",
            "owned_recovery_status",
            "--id",
            "owned-recovery-status-cli-2",
            "--output",
        ])
        .arg(&output_path)
        .output()
        .expect("run request");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing --lock-id"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output_path.exists());
}

#[test]
fn help_lists_owned_recovery_status() {
    let output = bin().arg("help").output().expect("run help");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("owned_recovery_status"), "help text: {text}");
    assert!(text.contains("Owned_recovery_status request supports --lock-id"));
}
