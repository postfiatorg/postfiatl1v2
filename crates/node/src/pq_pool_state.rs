//! PFSH02 settlement on the pinned disposable chain. The ordered batch owns
//! the outer transaction; this transition prepares one proof, pair and account
//! delta before making any of them visible to the next action.
use super::*;
use postfiat_types::pq_payment::{
    Chain, PqOutput, PqPair, PqPayment, PqPool, ENVELOPE_BYTES, INNER_PREFIX,
};

pub(super) fn prepare_pair(
    pool: &PqPool,
    payment: &PqPayment,
    chain: Chain<'_>,
    height: u64,
) -> Result<(PqPool, Receipt), &'static str> {
    let (terms, _, inner) = payment.parts().map_err(|_| "pq_encoding")?;
    let statement =
        pq_payment_actions::verify_statement(payment, chain).map_err(|_| "pq_invalid_proof")?;
    let asset = bytes_to_hex(&terms.asset);
    let writable = pool.head.assets.get(&asset);
    let envelopes_end = INNER_PREFIX - 4; // The proof length follows the two envelopes.
    let middle = envelopes_end - ENVELOPE_BYTES;
    let record = PqPair {
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
    };
    let next = pool.append_pair(record)?;
    let receipt = Receipt::accepted(payment.digest(), "PQ payment pair appended");
    Ok((next, receipt))
}
