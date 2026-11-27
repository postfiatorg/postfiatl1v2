//! PFSH02 settlement on the pinned disposable chain. The ordered batch owns
//! the outer transaction; this transition prepares one proof, pair and account
//! delta before making any of them visible to the next action.
use super::*;
use postfiat_types::pq_payment::{
    Chain, PqOutput, PqPair, PqPayment, PqPool, ENVELOPES_END, ENVELOPE_BYTES,
};

pub(super) fn prepare_pair(
    pool: &PqPool,
    payment: &PqPayment,
    chain: Chain<'_>,
    height: u64,
) -> Result<(PqPool, Receipt), &'static str> {
    payment.parts().map_err(|_| "pq_encoding")?;
    // The pool checks read only public inputs, so they run before the proof.
    let public =
        postfiat_privacy_binius::l1_public_statement(payment).map_err(|_| "pq_invalid_proof")?;
    let next = pool.append_pair(record(pool, payment, &public, height)?)?;
    pq_payment_actions::verify_statement(payment, chain).map_err(|_| "pq_invalid_proof")?;
    let receipt = Receipt::accepted(payment.digest(), "PQ payment pair appended");
    Ok((next, receipt))
}

/// Pool checks of an atomic batch's PQ payments in batch order, without proof work.
/// Execution applies the same checks to the same pool, so a failure here is a
/// failure the batch would meet anyway; malformed bodies are left to verification.
pub(super) fn precheck_batch(
    pool: Option<&PqPool>,
    batch: &ShieldedActionBatch,
    height: u64,
) -> Result<(), &'static str> {
    if !batch.atomic
        || !batch
            .actions
            .iter()
            .any(|a| matches!(a, ShieldedAction::PqPaymentV1(_)))
    {
        return Ok(());
    }
    let mut view = pool.cloned().unwrap_or_default();
    for action in &batch.actions {
        let ShieldedAction::PqPaymentV1(payment) = action else {
            continue;
        };
        let Ok(public) = postfiat_privacy_binius::l1_public_statement(payment) else {
            return Ok(());
        };
        let Ok(pair) = record(&view, payment, &public, height) else {
            return Ok(());
        };
        view = view.append_pair(pair)?;
    }
    Ok(())
}

fn record(
    pool: &PqPool,
    payment: &PqPayment,
    statement: &postfiat_privacy_binius::PaymentStatement,
    height: u64,
) -> Result<PqPair, &'static str> {
    let (terms, _, inner) = payment.parts().map_err(|_| "pq_encoding")?;
    let asset = bytes_to_hex(&terms.asset);
    let writable = pool.head.assets.get(&asset);
    let envelopes_end = ENVELOPES_END; // The two ML-KEM envelopes precede spend_auth0/1 and proof_len.
    let middle = envelopes_end - ENVELOPE_BYTES;
    Ok(PqPair {
        previous: pool.head.last_page.clone(),
        asset,
        read_epoch: terms.epoch,
        anchor: bytes_to_hex(&statement.anchor),
        epoch: writable.map_or(0, |s| s.epoch),
        first_index: writable.map_or(0, |s| s.writable.count),
        height,
        kind: terms.mode,
        mint: statement.v_mint,
        exit: statement.v_unshield,
        fee: statement.fee,
        serials: [bytes_to_hex(&statement.sn0), bytes_to_hex(&statement.sn1)],
        outputs: [
            PqOutput::new(
                &statement.coin_out0,
                &inner[middle - ENVELOPE_BYTES..middle],
            ),
            PqOutput::new(&statement.coin_out1, &inner[middle..envelopes_end]),
        ],
    })
}
