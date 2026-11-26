//! Deterministic, bounded PFSH02 proof verification. Ledger rules run in the caller.
use pft_binius_shielded::{binius_pcs, field, wire::ShieldedTx};
pub use pft_binius_shielded::{
    circuit::PaymentStatement, shake256::MerkleTree24, wire::W1_ARCHIVAL_BUDGET_BYTES,
};

pub enum VerificationContext {
    StandaloneFixture,
}

#[derive(Debug, PartialEq, Eq)]
pub enum VerifyError {
    Encoding,
    InvalidPublicFlow,
    InvalidProof,
}

/// Warm the immutable field tables before validating the fixture chain.
pub fn initialize() {
    field::k_tables();
    binius_pcs::verifier_row_layout();
}

pub fn verify_payment(
    bytes: &[u8],
    context: VerificationContext,
) -> Result<PaymentStatement, VerifyError> {
    let tx = ShieldedTx::from_bytes(bytes).map_err(|_| VerifyError::Encoding)?;
    tx.check_public_flows()
        .map_err(|_| VerifyError::InvalidPublicFlow)?;
    let statement = match context {
        VerificationContext::StandaloneFixture => tx.statement(),
    };
    if tx.v_mint > 0 {
        let Some(auth) = tx.mint_auth.as_ref() else {
            return Err(VerifyError::InvalidProof);
        };
        if !auth.verify(&statement.tx_ctx) {
            return Err(VerifyError::InvalidProof);
        }
    }
    tx.verify_spend_auth(&statement.tx_ctx)
        .map_err(|_| VerifyError::InvalidProof)?;
    binius_pcs::verify(&statement, &tx.proof).map_err(|_| VerifyError::InvalidProof)?;
    Ok(statement)
}

/// Public inputs of an L1 payment, parsed without checking its signature or proof.
/// The fields used for ledger checks equal those `verify_l1` returns.
pub fn l1_public_statement(
    payment: &postfiat_types::pq_payment::PqPayment,
) -> Result<PaymentStatement, VerifyError> {
    let (_, _, inner) = payment.parts().map_err(|_| VerifyError::Encoding)?;
    let tx = ShieldedTx::from_l1_inner(inner).map_err(|_| VerifyError::Encoding)?;
    Ok(tx.statement())
}

/// Verify the settled-domain transcript, per-input external ML-DSA-65 spend authorization,
/// and the public deposit authorization, if present.
/// Stateful eligibility, sequence and debits are checked at settlement.
pub fn verify_l1(
    payment: &postfiat_types::pq_payment::PqPayment,
    chain: postfiat_types::pq_payment::Chain,
) -> Result<PaymentStatement, VerifyError> {
    verify_l1_abortable(payment, chain, None)
}

/// Fail-fast L1 verifier with optional cooperative cancellation (`abort`) for
/// parallel multi-proof block validation.
pub fn verify_l1_abortable(
    payment: &postfiat_types::pq_payment::PqPayment,
    chain: postfiat_types::pq_payment::Chain,
    abort: Option<&std::sync::atomic::AtomicBool>,
) -> Result<PaymentStatement, VerifyError> {
    use std::sync::atomic::Ordering;
    if abort.is_some_and(|f| f.load(Ordering::Relaxed)) {
        return Err(VerifyError::InvalidProof);
    }
    let (terms, signature, inner) = payment.parts().map_err(|_| VerifyError::Encoding)?;
    let tx = ShieldedTx::from_l1_inner(inner).map_err(|_| VerifyError::Encoding)?;
    let mut statement = tx.statement();
    if let (Some(a), Some(sig)) = (&terms.account, signature) {
        let address = postfiat_crypto_provider::address_from_public_key(a.public_key.as_ref());
        if address != format!("pf{}", postfiat_crypto_provider::bytes_to_hex(&a.id))
            || !postfiat_crypto_provider::ml_dsa_65_verify(
                a.public_key.as_ref(),
                &terms.signing_message(chain, statement.tx_ctx).unwrap(),
                sig,
            )
        {
            return Err(VerifyError::InvalidPublicFlow);
        }
    }
    statement.tx_ctx = terms.statement_context(chain, statement.tx_ctx);
    tx.verify_spend_auth(&statement.tx_ctx)
        .map_err(|_| VerifyError::InvalidProof)?;
    binius_pcs::verify_abortable(&statement, &tx.proof, abort)
        .map_err(|_| VerifyError::InvalidProof)?;
    Ok(statement)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn published_statements_verify_on_this_arithmetic_backend() {
        for vector in [
            &include_bytes!("../../../tools/pft-binius-shielded-pq/payments/tx_mint.bin")[..],
            include_bytes!("../../../tools/pft-binius-shielded-pq/payments/tx_hop1.bin"),
            include_bytes!("../../../tools/pft-binius-shielded-pq/payments/tx_dummy_1in.bin"),
            include_bytes!("../../../tools/pft-binius-shielded-pq/payments/tx_hop2.bin"),
        ] {
            verify_payment(vector, VerificationContext::StandaloneFixture).unwrap();
            let mut changed = vector.to_vec();
            // Deposit authorization is outside the proof. Change the envelope, which every proof binds.
            changed[500] ^= 1;
            assert_eq!(
                verify_payment(&changed, VerificationContext::StandaloneFixture).unwrap_err(),
                VerifyError::InvalidProof
            );
        }
    }

    #[test]
    fn malformed_public_outflow_is_rejected_before_circuit_evaluation() {
        let mut bytes =
            include_bytes!("../../../tools/pft-binius-shielded-pq/payments/tx_hop1.bin").to_vec();
        bytes[64..72].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(
            verify_payment(&bytes, VerificationContext::StandaloneFixture).unwrap_err(),
            VerifyError::InvalidPublicFlow
        );
    }

    #[test]
    fn fail_fast_dos_rejects_bad_sig_bad_pow_and_aborts_siblings() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::time::Instant;
        initialize();

        let valid = include_bytes!("../../../tools/pft-binius-shielded-pq/payments/tx_hop1.bin");
        // 1. Bad ML-DSA-65 spend signature (offset 7548 = first byte of sig_in0)
        let mut bad_sig = valid.to_vec();
        bad_sig[7548] ^= 0x01;
        // 2. Valid ML-DSA-65 spend signatures + corrupted Binius proof byte (breaks PoW / transcript)
        let mut bad_pow = valid.to_vec();
        let proof_start = 14_170;
        bad_pow[proof_start + 59_860] ^= 0x01; // flips pow_nonce inside fixed proof header
                                               // 3. Valid ML-DSA-65 sigs + valid PoW + corrupted final Merkle sibling (worst-case DoS)
        let mut bad_merkle = valid.to_vec();
        let last = bad_merkle.len() - 1;
        bad_merkle[last] ^= 0x01;

        let measure = |buf: &[u8]| -> f64 {
            let mut times = Vec::with_capacity(20);
            for _ in 0..20 {
                let t0 = Instant::now();
                assert_eq!(
                    verify_payment(buf, VerificationContext::StandaloneFixture).unwrap_err(),
                    VerifyError::InvalidProof
                );
                times.push(t0.elapsed().as_secs_f64() * 1e3);
            }
            times.sort_by(f64::total_cmp);
            times[10]
        };

        let ms_bad_sig = measure(&bad_sig);
        let ms_bad_pow = measure(&bad_pow);
        let ms_bad_merkle = measure(&bad_merkle);
        println!(
            "DOS_FAIL_FAST med_ms: bad_sig={ms_bad_sig:.3} bad_pow={ms_bad_pow:.3} bad_last_merkle={ms_bad_merkle:.3}"
        );
        assert!(ms_bad_sig < 5.0, "bad_sig took {ms_bad_sig:.3} ms");
        assert!(ms_bad_pow < 5.0, "bad_pow took {ms_bad_pow:.3} ms");

        // 4. Cooperative early-abort stops immediately when sibling sets abort=true
        let flag = AtomicBool::new(true);
        let tx = ShieldedTx::from_bytes(valid).unwrap();
        let stmt = tx.statement();
        let t0 = Instant::now();
        assert!(binius_pcs::verify_abortable(&stmt, &tx.proof, Some(&flag)).is_err());
        let ms_abort = t0.elapsed().as_secs_f64() * 1e3;
        println!("DOS_COOP_ABORT ms={ms_abort:.4}");
        assert!(ms_abort < 1.0);
        assert!(flag.load(Ordering::Relaxed));
    }
}

/// Local input to shield authorization. The signer retains the blinders and
/// the ciphertexts it sealed for the two intended recipients.
pub struct ShieldOutputIntent<'a> {
    pub owner: &'a [u8; 48],
    pub value: u64,
    pub blinder: &'a [u8; 78],
    pub envelope: &'a [u8; postfiat_types::pq_payment::ENVELOPE_BYTES],
}

pub fn sign_l1_shield(
    chain: postfiat_types::pq_payment::Chain,
    terms: &postfiat_types::pq_payment::Terms,
    inner: &[u8],
    mint: u64,
    inner_fee: u64,
    outputs: [ShieldOutputIntent<'_>; 2],
    private_key: &[u8],
) -> Result<postfiat_types::pq_payment::PqPayment, &'static str> {
    use postfiat_types::pq_payment::{Mode, PqPayment};
    let tx = ShieldedTx::from_l1_inner(inner).map_err(|_| "invalid shield header")?;
    if terms.mode != Mode::Shield
        || mint == 0
        || tx.v_mint != mint
        || tx.v_unshield != 0
        || tx.fee != inner_fee
    {
        return Err("shield debit differs from intent");
    }
    for (output, (coin, envelope)) in outputs
        .iter()
        .zip([(&tx.coin_out0, &tx.env0), (&tx.coin_out1, &tx.env1)])
    {
        if pft_binius_shielded::shake256::coin_commit(output.owner, output.value, output.blinder)
            != *coin
            || output.envelope != envelope
        {
            return Err("shield output differs from intent");
        }
    }
    let msg = terms
        .signing_message(chain, tx.statement().tx_ctx)
        .ok_or("missing shield source")?;
    let signature = postfiat_crypto_provider::ml_dsa_65_sign(private_key, &msg)
        .map_err(|_| "shield signing failed")?
        .try_into()
        .map_err(|_| "bad signature length")?;
    PqPayment::encode(terms, Some(&signature), inner)
}
