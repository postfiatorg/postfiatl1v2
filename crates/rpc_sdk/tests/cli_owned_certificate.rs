//! Built-binary coverage for the `owned_certificate` CLI wiring, including
//! request-bound response validation through `--request-file`.
use std::fs;
use std::path::PathBuf;
use std::process::Command;

use postfiat_rpc_sdk::{
    owned_certificate_request, success_response, write_request_file, OwnedCertificateSelector,
};
use postfiat_types::{
    FastPayCertificateV1, FastPayOrderRecoveryV1, OwnedCertificateDomain, OwnedObjectRef,
    OwnedOutputSpec, OwnedTransferCertificateV3, OwnedTransferOrderV3, OwnedTransferVote,
    FASTPAY_ORDER_RECOVERY_SCHEMA_V1, OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3,
};

fn bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_postfiat-rpc-sdk"))
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "postfiat-rpc-sdk-owned-certificate-{}-{name}",
        std::process::id()
    ));
    path
}

fn transfer_certificate() -> FastPayCertificateV1 {
    let mut order = OwnedTransferOrderV3 {
        domain: OwnedCertificateDomain {
            schema: OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3.to_string(),
            chain_id: "rpc-sdk-cli-tests".to_string(),
            genesis_hash: "11".repeat(48),
            protocol_version: 3,
            registry_id: "22".repeat(48),
        },
        recovery: FastPayOrderRecoveryV1 {
            schema: FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
            committee_epoch: 7,
            lock_id: "00".repeat(48),
            valid_from_height: 100,
            expires_at_height: 110,
            recovery_closes_at_height: 120,
        },
        inputs: vec![OwnedObjectRef {
            id: "33".repeat(32),
            version: 9,
        }],
        outputs: vec![OwnedOutputSpec {
            owner_pubkey_hex: "44".repeat(32),
            value: 9,
            asset: "PFT".to_string(),
        }],
        fee: 1,
        nonce: 8,
        memos: Vec::new(),
    };
    order.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&order);
    FastPayCertificateV1::Transfer(OwnedTransferCertificateV3 {
        order,
        owner_pubkey_hex: "aa".repeat(32),
        owner_signature_hex: "bb".repeat(32),
        votes: vec![OwnedTransferVote {
            validator_id: "validator-0".to_string(),
            signature_hex: "cc".repeat(32),
        }],
    })
}

#[test]
fn request_subcommand_builds_owned_certificate_request_by_lock_id() {
    let output_path = temp_path("by-lock.json");
    let lock_id = "ab".repeat(48);
    let output = bin()
        .args([
            "request",
            "--method",
            "owned_certificate",
            "--id",
            "owned-certificate-cli-1",
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
    assert_eq!(written["method"], "owned_certificate");
    assert_eq!(written["params"]["lock_id"], lock_id);
    assert!(written["params"].get("certificate_digest").is_none());
    let _ = fs::remove_file(&output_path);
}

#[test]
fn request_subcommand_builds_owned_certificate_request_by_digest_to_stdout() {
    let digest = "cd".repeat(48);
    let output = bin()
        .args([
            "request",
            "--method",
            "owned_certificate",
            "--id",
            "owned-certificate-cli-2",
            "--certificate-digest",
            digest.as_str(),
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
    assert_eq!(printed["params"]["certificate_digest"], digest);
    assert!(printed["params"].get("lock_id").is_none());
}

#[test]
fn request_subcommand_requires_exactly_one_selector() {
    let output_path = temp_path("none.json");
    let output = bin()
        .args([
            "request",
            "--method",
            "owned_certificate",
            "--id",
            "owned-certificate-cli-3",
            "--output",
        ])
        .arg(&output_path)
        .output()
        .expect("run request");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("missing --lock-id or --certificate-digest"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output_path.exists());

    let output = bin()
        .args([
            "request",
            "--method",
            "owned_certificate",
            "--id",
            "owned-certificate-cli-4",
            "--lock-id",
            "ab".repeat(48).as_str(),
            "--certificate-digest",
            "cd".repeat(48).as_str(),
            "--output",
        ])
        .arg(&output_path)
        .output()
        .expect("run request");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("exactly one of --lock-id or --certificate-digest"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output_path.exists());
}

#[test]
fn validate_response_binds_certificate_to_request_file() {
    let certificate = transfer_certificate();
    let request_path = temp_path("bound-request.json");
    let response_path = temp_path("bound-response.json");
    write_request_file(
        &request_path,
        &owned_certificate_request(
            "owned-certificate-cli-5",
            OwnedCertificateSelector::LockId(certificate.recovery().lock_id.clone()),
        ),
    )
    .expect("write request");
    let response =
        success_response("owned-certificate-cli-5", &certificate, vec![]).expect("response");
    fs::write(
        &response_path,
        serde_json::to_vec_pretty(&response).expect("serialize response"),
    )
    .expect("write response");

    let output = bin()
        .args(["validate-response", "--input"])
        .arg(&response_path)
        .args(["--expect-kind", "owned_certificate", "--request-file"])
        .arg(&request_path)
        .output()
        .expect("run validate-response");
    assert!(
        output.status.success(),
        "validate-response failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Without the request there is nothing to bind to.
    let output = bin()
        .args(["validate-response", "--input"])
        .arg(&response_path)
        .args(["--expect-kind", "owned_certificate"])
        .output()
        .expect("run validate-response without request");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("--request-file"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // A request for a different lock must be rejected.
    let other_request_path = temp_path("other-request.json");
    write_request_file(
        &other_request_path,
        &owned_certificate_request(
            "owned-certificate-cli-5",
            OwnedCertificateSelector::LockId("ab".repeat(48)),
        ),
    )
    .expect("write other request");
    let output = bin()
        .args(["validate-response", "--input"])
        .arg(&response_path)
        .args(["--expect-kind", "owned_certificate", "--request-file"])
        .arg(&other_request_path)
        .output()
        .expect("run validate-response with other request");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("certificate.order.recovery.lock_id"),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    // A request with the right lock but another id must be rejected on the id.
    let other_id_request_path = temp_path("other-id-request.json");
    write_request_file(
        &other_id_request_path,
        &owned_certificate_request(
            "owned-certificate-cli-6",
            OwnedCertificateSelector::LockId(certificate.recovery().lock_id.clone()),
        ),
    )
    .expect("write other-id request");
    let output = bin()
        .args(["validate-response", "--input"])
        .arg(&response_path)
        .args(["--expect-kind", "owned_certificate", "--request-file"])
        .arg(&other_id_request_path)
        .output()
        .expect("run validate-response with other-id request");
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(
            "rpc response id `owned-certificate-cli-5` did not match expected `owned-certificate-cli-6`"
        ),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for path in [
        request_path,
        response_path,
        other_request_path,
        other_id_request_path,
    ] {
        let _ = fs::remove_file(path);
    }
}

#[test]
fn help_lists_owned_certificate() {
    let output = bin().arg("help").output().expect("run help");
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(text.contains("owned_certificate"), "help text: {text}");
    assert!(text.contains(
        "Owned_certificate request supports exactly one of --lock-id or --certificate-digest"
    ));
}
