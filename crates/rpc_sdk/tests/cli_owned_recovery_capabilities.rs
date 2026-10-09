//! Built-binary coverage for the `owned_recovery_capabilities` CLI wiring.
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_postfiat-rpc-sdk"))
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "postfiat-rpc-sdk-owned-recovery-capabilities-{}-{name}",
        std::process::id()
    ));
    path
}

#[test]
fn request_subcommand_builds_owned_recovery_capabilities_request_file() {
    let output_path = temp_path("request.json");
    let output = bin()
        .args([
            "request",
            "--method",
            "owned_recovery_capabilities",
            "--id",
            "owned-recovery-capabilities-cli-1",
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
    assert_eq!(written["method"], "owned_recovery_capabilities");
    assert_eq!(written["id"], "owned-recovery-capabilities-cli-1");
    assert!(
        written["params"].is_null()
            || written["params"]
                .as_object()
                .is_some_and(|params| params.is_empty()),
        "params: {}",
        written["params"]
    );
    let _ = fs::remove_file(&output_path);
}

#[test]
fn request_subcommand_prints_owned_recovery_capabilities_request_to_stdout() {
    let output = bin()
        .args([
            "request",
            "--method",
            "owned_recovery_capabilities",
            "--id",
            "owned-recovery-capabilities-cli-2",
            "--output",
            "-",
        ])
        .output()
        .expect("run request");
    assert!(
        output.status.success(),
        "request failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let printed: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("parse stdout request");
    assert_eq!(printed["method"], "owned_recovery_capabilities");
    assert_eq!(printed["id"], "owned-recovery-capabilities-cli-2");
}

#[test]
fn help_lists_owned_recovery_capabilities() {
    let output = bin().arg("help").output().expect("run help");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        text.contains("owned_recovery_capabilities"),
        "help text: {text}"
    );
    assert!(text.contains("Owned_recovery_capabilities request takes no flags."));
}
