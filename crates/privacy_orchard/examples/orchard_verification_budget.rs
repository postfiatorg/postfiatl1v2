use orchard::circuit::VerifyingKey;
use postfiat_crypto_provider::hash_hex;
use postfiat_privacy_orchard::{
    orchard_build_output_action_test_vector, orchard_default_address_from_spending_key,
    orchard_empty_anchor, verify_serialized_orchard_action, OrchardAuthorizingDomain,
    ORCHARD_MEMO_BYTES,
};
use serde_json::json;

fn root(byte: char) -> String {
    std::iter::repeat_n(byte, 96).collect()
}

fn cpu_model() -> String {
    std::fs::read_to_string("/proc/cpuinfo")
        .ok()
        .and_then(|body| {
            body.lines()
                .find_map(|line| line.strip_prefix("model name\t: "))
                .map(str::to_string)
        })
        .unwrap_or_else(|| "unknown".to_string())
}

fn median(values: &mut [u128]) -> u128 {
    values.sort_unstable();
    values[values.len() / 2]
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let iterations = std::env::var("ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(5);
    let domain = OrchardAuthorizingDomain::new("postfiat-local", root('0'), 1, "orchard-v1")?;
    let address = orchard_default_address_from_spending_key([7u8; 32])?;
    let action_build_start = std::time::Instant::now();
    let action = orchard_build_output_action_test_vector(
        &domain,
        "orchard-v1",
        1,
        orchard_empty_anchor(),
        &address,
        42,
        [0u8; ORCHARD_MEMO_BYTES],
        Some(&"22".repeat(48)),
        [1u8; 32],
        [2u8; 32],
        [3u8; 32],
    )?;
    let action_build_ms = action_build_start.elapsed().as_millis();
    let build_start = std::time::Instant::now();
    let verifying_key = VerifyingKey::build();
    let verifying_key_build_ms = build_start.elapsed().as_millis();

    let warmup = verify_serialized_orchard_action(&action, &verifying_key, &domain)?;
    let mut verify_ms = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let start = std::time::Instant::now();
        let verified = verify_serialized_orchard_action(&action, &verifying_key, &domain)?;
        assert_eq!(verified, warmup);
        verify_ms.push(start.elapsed().as_millis());
    }
    let mut median_input = verify_ms.clone();
    let median_verify_ms = median(&mut median_input);
    let max_verify_ms = verify_ms.iter().copied().max().unwrap_or(0);
    let min_verify_ms = verify_ms.iter().copied().min().unwrap_or(0);

    let report = json!({
        "schema": "postfiat-orchard-verification-budget-v1",
        "status": "passed",
        "checker": "verify_serialized_orchard_action",
        "scope": "local-privacy-crate",
        "hardware": {
            "cpu_model": cpu_model(),
            "available_parallelism": std::thread::available_parallelism().map(|value| value.get()).unwrap_or(1)
        },
        "proof_system": {
            "proof_system_id": action.proof_system_id.as_str(),
            "circuit_id": action.circuit_id.as_str(),
            "trusted_setup": "none for Halo2 proving system; Orchard parameters are built from the upstream orchard crate",
            "action_count": warmup.action_count,
            "max_actions": 8,
            "proof_bytes": action.proof.byte_len(),
            "nullifiers": warmup.nullifiers.len(),
            "output_commitments": warmup.output_commitments.len(),
            "encrypted_outputs": warmup.encrypted_outputs.len()
        },
        "measurements": {
            "iterations": iterations,
            "action_build_and_prove_ms": action_build_ms,
            "verifying_key_build_ms": verifying_key_build_ms,
            "verify_ms": verify_ms,
            "median_verify_ms": median_verify_ms,
            "min_verify_ms": min_verify_ms,
            "max_verify_ms": max_verify_ms
        },
        "budget_interpretation": {
            "verification_is_cpu_bound": true,
            "validators_can_cache_verifying_key": true,
            "block_budget_requires_action_count_cap": true
        }
    });
    let report_hash = hash_hex(
        "postfiat.orchard.verification_budget.report.v1",
        &serde_json::to_vec(&report)?,
    );
    let report = json!({
        "report_hash": report_hash,
        "report": report
    });
    let body = serde_json::to_string_pretty(&report)? + "\n";
    if let Some(path) = std::env::var_os("REPORT") {
        let path = std::path::PathBuf::from(path);
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, &body)?;
    } else {
        print!("{body}");
    }
    Ok(())
}
