//! Built-binary coverage for the `market_ops_status` CLI wiring.
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_postfiat-rpc-sdk"))
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "postfiat-rpc-sdk-market-ops-status-{}-{name}",
        std::process::id()
    ));
    path
}

fn read_request(path: &PathBuf) -> serde_json::Value {
    serde_json::from_str(&fs::read_to_string(path).expect("read request")).expect("parse request")
}

#[test]
fn request_subcommand_builds_market_ops_status_request_without_epoch() {
    let output_path = temp_path("latest.json");
    let asset_id = "ab".repeat(48);
    let output = bin()
        .args([
            "request",
            "--method",
            "market_ops_status",
            "--id",
            "market-ops-status-cli-1",
            "--asset-id",
            asset_id.as_str(),
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
    assert_eq!(written["method"], "market_ops_status");
    assert_eq!(written["id"], "market-ops-status-cli-1");
    assert_eq!(written["params"]["asset_id"], asset_id);
    assert!(written["params"].get("epoch").is_none());
    let _ = fs::remove_file(&output_path);
}

#[test]
fn request_subcommand_builds_market_ops_status_request_with_epoch() {
    let output_path = temp_path("epoch.json");
    let asset_id = "ab".repeat(48);
    let output = bin()
        .args([
            "request",
            "--method",
            "market_ops_status",
            "--id",
            "market-ops-status-cli-2",
            "--asset-id",
            asset_id.as_str(),
            "--epoch",
            "7",
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
    assert_eq!(written["params"]["epoch"], 7);
    let _ = fs::remove_file(&output_path);
}

#[test]
fn request_subcommand_requires_asset_id() {
    let output_path = temp_path("missing.json");
    let output = bin()
        .args([
            "request",
            "--method",
            "market_ops_status",
            "--id",
            "market-ops-status-cli-3",
            "--output",
        ])
        .arg(&output_path)
        .output()
        .expect("run request");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("missing --asset-id"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output_path.exists());
}

#[test]
fn help_lists_market_ops_status() {
    let output = bin().arg("help").output().expect("run help");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("market_ops_status"), "help text: {text}");
    assert!(text.contains("Market_ops_status request supports --asset-id"));
}
