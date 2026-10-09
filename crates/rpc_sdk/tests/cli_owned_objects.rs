//! Built-binary coverage for the `owned_objects` CLI wiring.
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_postfiat-rpc-sdk"))
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "postfiat-rpc-sdk-owned-objects-{}-{name}",
        std::process::id()
    ));
    path
}

fn read_request(path: &PathBuf) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read request")).expect("parse request")
}

#[test]
fn request_subcommand_builds_owned_objects_request_without_optional_flags() {
    let output_path = temp_path("plain.json");
    let owner = "ab".repeat(1952);
    let output = bin()
        .args([
            "request",
            "--method",
            "owned_objects",
            "--id",
            "owned-objects-cli-1",
            "--owner-public-key-hex",
            owner.as_str(),
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
    let written = read_request(&output_path);
    assert_eq!(written["method"], "owned_objects");
    assert_eq!(written["id"], "owned-objects-cli-1");
    assert_eq!(written["params"]["owner_public_key_hex"], owner);
    assert!(written["params"].get("asset").is_none());
    assert!(written["params"].get("limit").is_none());
    let _ = fs::remove_file(&output_path);
}

#[test]
fn request_subcommand_builds_owned_objects_request_with_optional_flags() {
    let output_path = temp_path("filtered.json");
    let owner = "ab".repeat(1952);
    let output = bin()
        .args([
            "request",
            "--method",
            "owned_objects",
            "--id",
            "owned-objects-cli-2",
            "--owner-public-key-hex",
            owner.as_str(),
            "--asset",
            "USDX",
            "--limit",
            "25",
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
    let written = read_request(&output_path);
    assert_eq!(written["params"]["asset"], "USDX");
    assert_eq!(written["params"]["limit"], 25);
    let _ = fs::remove_file(&output_path);
}

#[test]
fn request_subcommand_requires_owner_public_key_hex() {
    let output_path = temp_path("missing.json");
    let output = bin()
        .args([
            "request",
            "--method",
            "owned_objects",
            "--id",
            "owned-objects-cli-3",
            "--output",
        ])
        .arg(&output_path)
        .output()
        .expect("run request");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing --owner-public-key-hex"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output_path.exists());
}

#[test]
fn help_lists_owned_objects() {
    let output = bin().arg("help").output().expect("run help");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("owned_objects"), "help text: {text}");
    assert!(text.contains("Owned_objects request supports --owner-public-key-hex"));
}
