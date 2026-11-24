use orchard::{
    builder::{Builder, BundleType},
    bundle::BatchValidator,
    circuit::{ProvingKey, VerifyingKey},
    keys::{FullViewingKey, Scope, SpendingKey},
    value::NoteValue,
};
use postfiat_crypto_provider::hash_hex;
use postfiat_privacy_orchard::{
    orchard_action_from_authorized_bundle, orchard_authorizing_sighash, orchard_bundle_from_action,
    orchard_empty_anchor, verify_serialized_orchard_action, OrchardAuthorizingDomain,
    ORCHARD_MEMO_BYTES,
};
use rand::{rngs::StdRng, SeedableRng};
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

fn pct(sorted: &[f64], p: f64) -> f64 {
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn stats(samples: &[f64]) -> serde_json::Value {
    let mut s = samples.to_vec();
    s.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let med = if s.len() % 2 == 1 {
        s[s.len() / 2]
    } else {
        (s[s.len() / 2 - 1] + s[s.len() / 2]) / 2.0
    };
    json!({
        "n": s.len(),
        "min_ms": (s[0] * 100.0).round() / 100.0,
        "median_ms": (med * 100.0).round() / 100.0,
        "p95_ms": (pct(&s, 0.95) * 100.0).round() / 100.0,
        "max_ms": (s[s.len() - 1] * 100.0).round() / 100.0,
    })
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let prove_iters = std::env::var("PROVE_ITERS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(50);
    let verify_iters = std::env::var("VERIFY_ITERS")
        .ok()
        .and_then(|v| v.parse::<usize>().ok())
        .filter(|v| *v > 0)
        .unwrap_or(50);

    let domain = OrchardAuthorizingDomain::new("postfiat-local", root('0'), 1, "orchard-v1")?;
    let sk = SpendingKey::from_bytes([7u8; 32]).unwrap();
    let fvk = FullViewingKey::from(&sk);
    let recipient = fvk.address_at(0u32, Scope::External);
    let anchor = orchard_empty_anchor().to_orchard()?;

    let pk_start = std::time::Instant::now();
    let pk = ProvingKey::build();
    let pk_build_ms = pk_start.elapsed().as_secs_f64() * 1000.0;

    let vk_start = std::time::Instant::now();
    let vk = VerifyingKey::build();
    let vk_build_ms = vk_start.elapsed().as_secs_f64() * 1000.0;

    // Pre-build 4 distinct 2-action unsigned bundles + sighashes
    let mut actions = Vec::with_capacity(4);
    let mut bundles = Vec::with_capacity(4);
    let mut sighashes = Vec::with_capacity(4);

    // Measure proving-only (cached ProvingKey, excluding action/key build & signatures)
    let mut prove_ms = Vec::with_capacity(prove_iters);
    let mut build_ms = Vec::with_capacity(prove_iters);
    for i in 0..prove_iters {
        let mut build_rng = StdRng::from_seed([10u8.wrapping_add(i as u8); 32]);
        let mut proof_rng = StdRng::from_seed([80u8.wrapping_add(i as u8); 32]);
        let mut sig_rng = StdRng::from_seed([150u8.wrapping_add(i as u8); 32]);

        let b_start = std::time::Instant::now();
        let mut builder = Builder::new(BundleType::DEFAULT, anchor);
        builder
            .add_output(
                None,
                recipient,
                NoteValue::from_raw(42 + i as u64),
                [0u8; ORCHARD_MEMO_BYTES],
            )
            .map_err(|e| format!("add_output: {e:?}"))?;
        let (unsigned_bundle, _) = builder
            .build::<i64>(&mut build_rng)
            .map_err(|e| format!("build: {e:?}"))?
            .ok_or("empty orchard bundle")?;
        let sighash = orchard_authorizing_sighash(&domain, 1, &unsigned_bundle)?;
        build_ms.push(b_start.elapsed().as_secs_f64() * 1000.0);

        let p_start = std::time::Instant::now();
        let proven = unsigned_bundle
            .create_proof(&pk, &mut proof_rng)
            .map_err(|e| format!("create_proof: {e:?}"))?;
        prove_ms.push(p_start.elapsed().as_secs_f64() * 1000.0);

        if i < 4 {
            let authorized = proven
                .apply_signatures(&mut sig_rng, sighash, &[])
                .map_err(|e| format!("apply_signatures: {e:?}"))?;
            let action = orchard_action_from_authorized_bundle("orchard-v1", 1, &authorized)?;
            let parsed_bundle = orchard_bundle_from_action(&action)?;
            actions.push(action);
            bundles.push(parsed_bundle);
            sighashes.push(sighash);
        }
    }

    // Warmup & measure single-bundle (2-action) verify_serialized_orchard_action
    let _ = verify_serialized_orchard_action(&actions[0], &vk, &domain)?;
    let mut single_verify_ms = Vec::with_capacity(verify_iters);
    for _ in 0..verify_iters {
        let start = std::time::Instant::now();
        let _ = verify_serialized_orchard_action(&actions[0], &vk, &domain)?;
        single_verify_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    // Warmup & measure 4-bundle (8-action) BatchValidator (Halo 2 + RedPallas batching)
    let mut batch_rng = StdRng::from_seed([200u8; 32]);
    {
        let mut bv = BatchValidator::new();
        for j in 0..4 {
            bv.add_bundle(&bundles[j], sighashes[j]);
        }
        assert!(bv.validate(&vk, &mut batch_rng));
    }
    let mut batch4_verify_ms = Vec::with_capacity(verify_iters);
    for _ in 0..verify_iters {
        let start = std::time::Instant::now();
        let mut bv = BatchValidator::new();
        for j in 0..4 {
            bv.add_bundle(&bundles[j], sighashes[j]);
        }
        assert!(bv.validate(&vk, &mut batch_rng));
        batch4_verify_ms.push(start.elapsed().as_secs_f64() * 1000.0);
    }

    let report = json!({
        "schema": "postfiat-orchard-proving-and-batch-budget-v1",
        "cpu_model": cpu_model(),
        "rayon_num_threads": std::env::var("RAYON_NUM_THREADS").unwrap_or_else(|_| "default".into()),
        "action_count_per_bundle": actions[0].nullifiers.len(),
        "proof_bytes": actions[0].proof.byte_len(),
        "proving_key_build_ms": (pk_build_ms * 100.0).round() / 100.0,
        "verifying_key_build_ms": (vk_build_ms * 100.0).round() / 100.0,
        "action_build_only_ms": stats(&build_ms),
        "proving_only_cached_pk_ms": stats(&prove_ms),
        "single_bundle_verify_cached_vk_ms": stats(&single_verify_ms),
        "four_bundle_batch_validator_cached_vk_ms": stats(&batch4_verify_ms),
    });
    let report_hash = hash_hex(
        "postfiat.orchard.proving_and_batch_budget.v1",
        &serde_json::to_vec(&report)?,
    );
    println!(
        "{}",
        serde_json::to_string_pretty(&json!({
            "report_hash": report_hash,
            "report": report,
        }))?
    );
    Ok(())
}
