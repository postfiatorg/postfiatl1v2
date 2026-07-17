//! FastPay M1 benchmark: measure the per-order consensusless fast-path cost
//! under ML-DSA-65, and break down sign vs verify. This answers the M1 gate
//! question — is the per-order signature cost low enough that the consensusless
//! lane's wall-clock is dominated by network RTT (good) rather than crypto?
use postfiat_crypto_provider as crypto;
use postfiat_fastpay_prototype as fp;
use std::time::Instant;

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn summarize(label: &str, mut v: Vec<f64>) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mean = v.iter().sum::<f64>() / v.len() as f64;
    println!(
        "  {label:34} mean={mean:7.3}ms  p50={:.3}  p95={:.3}  p99={:.3}  (n={})",
        percentile(&v, 0.5),
        percentile(&v, 0.95),
        percentile(&v, 0.99),
        v.len()
    );
}

fn main() {
    let n_validators: usize = 3;
    let iters = 50;

    // --- keygen (outside timing) ---
    let owner = crypto::ml_dsa_65_keygen().expect("owner keygen");
    let owner_pk = owner.public_key.clone();
    let owner_sk = owner.private_key.to_vec();
    let validators: Vec<(u64, Vec<u8>, Vec<u8>)> = (0..n_validators)
        .map(|i| {
            let kp = crypto::ml_dsa_65_keygen().expect("validator keygen");
            (i as u64, kp.public_key, kp.private_key.to_vec())
        })
        .collect();
    let pks: Vec<(u64, Vec<u8>)> = validators
        .iter()
        .map(|(id, pk, _)| (*id, pk.clone()))
        .collect();

    let order = fp::OwnedTransferOrder {
        inputs: vec![fp::ObjectRef {
            id: [7u8; 32],
            version: 1,
        }],
        outputs: vec![fp::OwnedObjectSpec {
            owner_pubkey: owner_pk.clone(),
            value: 100,
            asset: "PFT".into(),
        }],
        fee: 1,
        nonce: 1,
    };

    // --- per-order full fast path: owner_sign + N validator_signs + aggregate + verify ---
    let mut per_order: Vec<f64> = Vec::with_capacity(iters);
    let mut cert_bytes = 0usize;
    for _ in 0..iters {
        let t0 = Instant::now();
        let owner_sig = fp::owner_sign(&owner_sk, &order).expect("owner sign");
        let votes: Vec<fp::ValidatorVote> = validators
            .iter()
            .map(|(id, _, sk)| fp::validator_sign(sk.as_slice(), *id, &order).expect("vsign"))
            .collect();
        let cert = fp::aggregate_certificate(order.clone(), owner_pk.clone(), owner_sig, votes);
        let verdict = fp::verify_certificate(&cert, &pks);
        per_order.push(t0.elapsed().as_secs_f64() * 1000.0);
        cert_bytes = cert.certificate_bytes();
        assert_eq!(
            verdict,
            fp::CertificateVerdict::Valid {
                votes: n_validators
            }
        );
    }

    // --- sign/verify micro-breakdown ---
    let m = 200;
    let sb = order.signing_bytes();
    let mut sign_t = Vec::with_capacity(m);
    let mut verify_t = Vec::with_capacity(m);
    for _ in 0..m {
        let t0 = Instant::now();
        let s = crypto::ml_dsa_65_sign_with_context(&owner_sk, &sb, fp::OWNED_TRANSFER_CONTEXT)
            .expect("sign");
        sign_t.push(t0.elapsed().as_secs_f64() * 1000.0);
        let t0 = Instant::now();
        let _ok =
            crypto::ml_dsa_65_verify_with_context(&owner_pk, &sb, &s, fp::OWNED_TRANSFER_CONTEXT);
        verify_t.push(t0.elapsed().as_secs_f64() * 1000.0);
    }

    println!(
        "FastPay M1 — consensusless owned-transfer, ML-DSA-65, {n_validators} validators (2f+1={q})",
        q = n_validators
    );
    println!("--- per-order fast path (owner_sign + {n_validators} validator_signs + aggregate + verify {n_validators}) ---");
    summarize("per-order total", per_order);
    println!(
        "  certificate byte cost: {cert_bytes} bytes (~{} KB)",
        cert_bytes / 1024
    );
    println!("--- ML-DSA-65 micro (single op) ---");
    summarize("sign", sign_t);
    summarize("verify", verify_t);
    println!(
        "  (sig={} B, pk={} B) — crypto cost only; on a network, FastPay wall-clock ≈ per-order + 2×RTT",
        crypto::ML_DSA_65_SIGNATURE_BYTES,
        crypto::ML_DSA_65_PUBLIC_KEY_BYTES
    );
}
