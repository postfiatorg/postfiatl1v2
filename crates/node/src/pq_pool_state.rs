//! PFSH02 settlement on the pinned disposable chain. The ordered batch owns
//! the outer transaction; this transition prepares one proof, pair and account
//! delta before making any of them visible to the next action.
use super::*;
use postfiat_types::pq_payment::{
    Chain, Mode, PqOutput, PqPair, PqPayment, PqPool, ENVELOPE_BYTES, INNER_PREFIX,
};

pub(super) fn prepare_pair(
    pool: &PqPool,
    payment: &PqPayment,
    chain: Chain<'_>,
    height: u64,
) -> Result<(PqPool, Receipt), &'static str> {
    let (terms, _, inner) = payment.parts().map_err(|_| "pq_encoding")?;
    let statement =
        postfiat_privacy_binius::verify_l1(payment, chain).map_err(|_| "pq_invalid_proof")?;
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
    let receipt = Receipt::accepted(payment.digest(), "PQ payment pair appended").with_fee_policy(
        statement.fee,
        statement.fee,
        postfiat_execution::MIN_TRANSFER_FEE,
        postfiat_execution::ACCOUNT_RESERVE,
    );
    Ok((next, receipt))
}

pub(super) fn apply(
    genesis: &Genesis,
    genesis_digest: &str,
    ledger: &mut LedgerState,
    shielded: &mut ShieldedState,
    payment: &PqPayment,
    height: u64,
) -> Result<Receipt, &'static str> {
    if genesis_digest != pq_payment_fixture::GENESIS_HASH {
        return Err("pq_settlement_inactive");
    }
    let (terms, _, inner) = payment.parts().map_err(|_| "pq_encoding")?;
    // Issued-asset settlement requires the governed policy and fee payer in M4.
    if terms.asset != [0; 48] {
        return Err("pq_settlement_inactive");
    }
    let chain = Chain {
        id: &genesis.chain_id,
        genesis: hex_to_bytes(genesis_digest)
            .map_err(|_| "pq_genesis")?
            .try_into()
            .map_err(|_| "pq_genesis")?,
        protocol: genesis.protocol_version,
    };
    let empty = PqPool::default();
    let (next, receipt) = prepare_pair(
        shielded.pq_pool_v1.as_ref().unwrap_or(&empty),
        payment,
        chain,
        height,
    )?;
    if receipt.fee_burned < postfiat_execution::MIN_TRANSFER_FEE {
        return Err("pq_minimum_fee");
    }
    let pair = next.pending.back().ok_or("pq_missing_pair")?;
    let account_write = match terms.mode {
        Mode::Shield => {
            let auth = terms.account.ok_or("pq_missing_source")?;
            let address = format!("pf{}", bytes_to_hex(&auth.id));
            let position = ledger
                .accounts
                .iter()
                .position(|a| a.address == address)
                .ok_or("pq_unfunded_source")?;
            let mut account = ledger.accounts[position].clone();
            let key = bytes_to_hex(auth.public_key.as_ref());
            if account.sequence.checked_add(1) != Some(auth.sequence)
                || account.public_key_hex.as_ref().is_some_and(|p| p != &key)
            {
                return Err("pq_source_authorization");
            }
            account.balance = account
                .balance
                .checked_sub(pair.mint)
                .filter(|v| *v >= postfiat_execution::ACCOUNT_RESERVE)
                .ok_or("pq_unfunded_source")?;
            account.public_key_hex = Some(key);
            account.sequence = auth.sequence;
            Some((position, account))
        }
        Mode::Exit => {
            let address = format!("pf{}", bytes_to_hex(&inner[84..104]));
            let position = ledger
                .accounts
                .iter()
                .position(|a| a.address == address)
                .ok_or("pq_existing_recipient_required")?;
            let mut account = ledger.accounts[position].clone();
            if account.flags != 0 {
                return Err("pq_recipient_policy");
            }
            account.balance = account
                .balance
                .checked_add(pair.exit)
                .ok_or("pq_recipient_overflow")?;
            Some((position, account))
        }
        Mode::Internal => None,
    };
    if let Some((position, account)) = account_write {
        ledger.accounts[position] = account;
    }
    shielded.pq_pool_v1 = Some(next);
    Ok(receipt)
}
