//! Qualification-only measurement of the affected retained-certificate commitments.
//! Input: private ledger.json from the captured height-1020 checkpoint export.
//! Full history verification is a separate gate; this probe is not an authenticator.
use postfiat_types::{FastPayRecoveryCommitmentVersion as Version, LedgerState};
use std::{hint::black_box, time::Instant};

fn encoded_sizes(ledger: &LedgerState, version: Version) -> (usize, usize) {
    let reveals = ledger.fastpay_recovery_reveals.iter().map(|entry| {
        entry.state_commitment_bytes_for_version(version).expect("valid reveal").len()
    }).sum();
    let fences = ledger.fastpay_version_fences.iter().map(|entry| {
        entry.state_commitment_bytes_for_version(version).expect("valid fence").len()
    }).sum();
    (reveals, fences)
}

fn main() {
    let path = std::env::args().nth(1).expect("private captured ledger.json path");
    let input = std::fs::read(path).expect("read ledger");
    // Legacy files may include an integrity footer after the JSON value.
    let ledger = serde_json::Deserializer::from_slice(&input).into_iter::<LedgerState>()
        .next().expect("ledger value").expect("decode ledger");
    let mut measurements = Vec::new();
    for (label, version) in [("v1", Version::V1), ("v2", Version::V2)] {
        let (reveals_bytes, fences_bytes) = encoded_sizes(&ledger, version);
        let started = Instant::now();
        for _ in 0..100 {
            black_box(encoded_sizes(black_box(&ledger), version));
        }
        measurements.push(serde_json::json!({
            "version": label, "reveal_commitment_bytes": reveals_bytes,
            "fence_commitment_bytes": fences_bytes, "iterations": 100,
            "mean_encoding_microseconds": started.elapsed().as_micros() as f64 / 100.0
        }));
    }
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({
        "scope": "Affected retained reveal/fence encodings only; excludes other ledger fields and installed committee",
        "source_commit": "1c435f4fb482ea7830bee5c7018d370a10a34bfd",
        "captured_checkpoint_height": 1020,
        "retained_reveals": ledger.fastpay_recovery_reveals.len(),
        "retained_fences": ledger.fastpay_version_fences.len(),
        "max_reveals": postfiat_types::MAX_FASTPAY_RECOVERY_REVEALS,
        "max_fences": postfiat_types::MAX_FASTPAY_VERSION_FENCES,
        "max_validators": postfiat_types::MAX_FASTPAY_RECOVERY_VALIDATORS,
        "measurements": measurements
    })).unwrap());
}
