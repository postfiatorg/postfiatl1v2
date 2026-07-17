use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{SystemTime, UNIX_EPOCH};

const MASTER_SEED: &str = "a00102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f";
const SIGNATURE_SEED: &str = "b01e1d1c1b1a191817161514131211100f0e0d0c0b0a09080706050403020100";

fn unique_dir() -> PathBuf {
    std::env::temp_dir().join(format!(
        "postfiat-wallet-vector-redaction-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock")
            .as_nanos()
    ))
}

#[cfg(unix)]
fn protect(path: &Path) {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).expect("protect secret fixture");
}

#[cfg(not(unix))]
fn protect(_path: &Path) {}

fn assert_redacted(label: &str, bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    for forbidden in [
        MASTER_SEED,
        SIGNATURE_SEED,
        "master_seed_hex",
        "signature_seed_hex",
        "private_key_hex",
    ] {
        assert!(
            !text.contains(forbidden),
            "{label} disclosed forbidden wallet material `{forbidden}`"
        );
    }
}

fn run_vector(root: &Path, to: &str, amount: &str) -> (Vec<String>, Output) {
    let args = vec![
        "wallet-test-vector".to_string(),
        "--chain-id".to_string(),
        "postfiat-wallet-vector-redaction".to_string(),
        "--validators".to_string(),
        "4".to_string(),
        "--master-seed-hex-file".to_string(),
        root.join("master.seed").display().to_string(),
        "--signature-seed-hex-file".to_string(),
        root.join("signature.seed").display().to_string(),
        "--to".to_string(),
        to.to_string(),
        "--amount".to_string(),
        amount.to_string(),
    ];
    let output = Command::new(env!("CARGO_BIN_EXE_postfiat-node"))
        .args(&args)
        .current_dir(root)
        .env("RUST_BACKTRACE", "1")
        .env("TMPDIR", root)
        .output()
        .expect("run shipping wallet-test-vector binary");
    (args, output)
}

#[test]
fn shipping_wallet_vector_never_leaks_seeds_in_success_failure_or_artifacts() {
    let root = unique_dir();
    fs::create_dir_all(&root).expect("create isolated working directory");
    let master_path = root.join("master.seed");
    let signature_path = root.join("signature.seed");
    fs::write(&master_path, format!("{MASTER_SEED}\n")).expect("write master seed fixture");
    fs::write(&signature_path, format!("{SIGNATURE_SEED}\n"))
        .expect("write signature seed fixture");
    protect(&master_path);
    protect(&signature_path);

    let (success_args, success) =
        run_vector(&root, "pf0123456789abcdef0123456789abcdef01234567", "17");
    assert!(
        success.status.success(),
        "shipping success invocation failed: {}",
        String::from_utf8_lossy(&success.stderr)
    );
    assert_redacted("success stdout", &success.stdout);
    assert_redacted("success stderr", &success.stderr);
    let report: serde_json::Value =
        serde_json::from_slice(&success.stdout).expect("parse redacted vector report");
    assert_eq!(
        report.get("schema").and_then(serde_json::Value::as_str),
        Some("postfiat-wallet-test-vector-v2")
    );

    // Zero amount fails after both secret files have been parsed, exercising
    // the post-ingress error path without assuming that every non-account
    // recipient namespace is invalid.
    let (failure_args, failure) =
        run_vector(&root, "pf0123456789abcdef0123456789abcdef01234567", "0");
    assert!(!failure.status.success(), "zero amount must fail");
    assert_redacted("failure stdout", &failure.stdout);
    assert_redacted("failure stderr", &failure.stderr);

    for (label, args) in [
        ("success argv", success_args),
        ("failure argv", failure_args),
    ] {
        assert_redacted(label, args.join("\0").as_bytes());
    }

    for entry in fs::read_dir(&root).expect("scan isolated working directory") {
        let path = entry.expect("working-directory entry").path();
        if path == master_path || path == signature_path || path.is_dir() {
            continue;
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("");
        assert!(
            !name.starts_with("core") && !name.contains("crash") && !name.contains("panic"),
            "shipping CLI left a crash artifact: {}",
            path.display()
        );
        assert_redacted(
            &format!("working artifact {}", path.display()),
            &fs::read(&path).expect("read working artifact"),
        );
    }

    fs::remove_dir_all(root).expect("remove isolated working directory");
}
