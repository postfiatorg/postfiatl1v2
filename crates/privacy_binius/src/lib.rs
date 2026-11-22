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
    binius_pcs::verify(&statement, &tx.proof).map_err(|_| VerifyError::InvalidProof)?;
    Ok(statement)
}

/// Verify the settled-domain transcript and the single public authorization, if present.
/// Stateful eligibility, sequence and debits are checked at settlement.
pub fn verify_l1(
    payment: &postfiat_types::pq_payment::PqPayment,
    chain: postfiat_types::pq_payment::Chain,
) -> Result<PaymentStatement, VerifyError> {
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
    binius_pcs::verify(&statement, &tx.proof).map_err(|_| VerifyError::InvalidProof)?;
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
}
