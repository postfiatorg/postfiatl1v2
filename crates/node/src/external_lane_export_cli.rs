//! The holder's side of `external_lane_export_v1`
//! (docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md,
//! section 6): `external-lane-export` checks the lane and the sender's balance
//! before signing, signs through `wallet_sign_asset_transaction` and submits
//! through the mempool path; `external-lane-export-status` reads one packet
//! back from the lane's export pages.

use std::io;
use std::path::PathBuf;

use postfiat_types::{
    AssetTransactionOperation, ExternalLaneExportOperationV1, SignedAssetTransaction,
    EXTERNAL_LANE_EXPORT_HALTED, EXTERNAL_LANE_EXPORT_NOT_SUPPORTED,
    EXTERNAL_LANE_EXPORT_RECIPIENT_IS_LOCK, EXTERNAL_LANE_EXPORT_RECIPIENT_IS_TOKEN,
    EXTERNAL_LANE_NOT_CONFIGURED,
};
use serde::{Deserialize, Serialize};

use crate::{
    asset_fee_quote, external_lane_info, read_key_file,
    submit_signed_asset_transaction_json_to_mempool, tx_finality,
    tx_finality_error_is_transaction_not_found, wallet_sign_asset_transaction,
    AssetFeeQuoteOptions, AssetFeeQuoteReport, ExternalLaneExportEntry, ExternalLaneInfoOptions,
    ExternalLaneInfoRow, SignedAssetTransactionJsonSubmitOptions, TxFinalityQueryOptions,
    WalletSignAssetTransactionOptions, EXTERNAL_LANE_RELEASE_PAGE_SIZE,
};

/// The status read found a packet another sender's export holds.
pub const EXTERNAL_LANE_EXPORT_OTHER_SENDER: &str = "external_lane_export_other_sender";
/// The status read's `base_payout`: what must happen for the payout on Base.
pub const EXTERNAL_LANE_EXPORT_BASE_PAYOUT_NOTE: &str =
    "this packet is recorded on PFTL; it pays out on Base only once a release relay proves this packet to the lane's lock contract on Base and that release is confirmed there";

/// The quoted minimum fee is above the caller's ceiling (`--max-fee-atoms`).
pub const EXTERNAL_LANE_EXPORT_FEE_ABOVE_CEILING: &str = "external_lane_export_fee_above_ceiling";
/// The largest fee an export can be charged today, in atoms. Its minimum fee
/// (`minimum_asset_transaction_fee_for_ledger`) is the base fee plus the
/// state-expansion part, and that part is 0 for `external_lane_export_v1` (it
/// creates no account, trustline, asset or entry). The base fee is
/// `MIN_TRANSFER_FEE.max(TRANSFER_FEE_PER_QUANTUM` (1) per started
/// `TRANSFER_FEE_BYTE_QUANTUM` (512 bytes) of the tx-id preimage`)`: the
/// signing bytes plus the ML-DSA-65 public key and signature in hex (3,904 +
/// 6,618 bytes). With every field at its longest (a 256-byte chain id, a
/// 32-byte lane id, 20-digit fee, sequence and amount) the preimage is 11,424
/// bytes, 23 quanta: 23 atoms. A typical export pays 22 (11,132 bytes).
pub const EXTERNAL_LANE_EXPORT_LARGEST_FEE_ATOMS: u64 = 23;
/// The default `--max-fee-atoms`, a safety ceiling against a quote that names
/// an inflated fee: 4x the largest fee an export can be charged today.
pub const EXTERNAL_LANE_EXPORT_DEFAULT_MAX_FEE_ATOMS: u64 =
    4 * EXTERNAL_LANE_EXPORT_LARGEST_FEE_ATOMS;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLaneExportOptions {
    pub data_dir: PathBuf,
    /// Read only when the export is signed; a dry run never opens it.
    pub key_file: PathBuf,
    /// The sender's address: required for a dry run, which opens no key
    /// file; when signing, it must be the key's address.
    pub sender: Option<String>,
    pub lane_id: String,
    pub amount_atoms: u64,
    pub base_recipient: String,
    /// Stop after the checks: print the quote and sign nothing.
    pub dry_run: bool,
    /// Refuse before signing when the quoted minimum fee is above this
    /// (`--max-fee-atoms`, default `EXTERNAL_LANE_EXPORT_DEFAULT_MAX_FEE_ATOMS`).
    pub max_fee_atoms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLaneExportReport {
    pub schema: String,
    pub lane_id: String,
    pub sender: String,
    pub amount_atoms: u64,
    pub base_recipient: String,
    pub fee: u64,
    pub sequence: u64,
    pub dry_run: bool,
    pub submitted: bool,
    /// Absent for a dry run, which signs nothing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tx_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signed_asset_transaction: Option<SignedAssetTransaction>,
    /// A dry run's quote: the unsigned fields (chain, source, sequence, fee,
    /// operation) a signature would cover.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quote: Option<AssetFeeQuoteReport>,
}

/// Refuses an export quote without `export_checked`: it comes from a node too
/// old to check an export, so its missing `export_refusal` proves nothing.
pub fn export_quote_was_checked(quote: &AssetFeeQuoteReport) -> io::Result<()> {
    if quote.export_checked {
        return Ok(());
    }
    Err(refused(
        "external_lane_export_not_checked",
        "the node is too old to check an export: its asset_fee_quote carries no export_checked"
            .to_string(),
    ))
}

/// Refuses a quoted minimum fee above the caller's ceiling, before signing.
pub fn export_fee_within_ceiling(minimum_fee: u64, max_fee_atoms: u64) -> io::Result<()> {
    if minimum_fee <= max_fee_atoms {
        return Ok(());
    }
    Err(refused(
        EXTERNAL_LANE_EXPORT_FEE_ABOVE_CEILING,
        format!(
            "the quoted minimum fee {minimum_fee} atoms is above the ceiling {max_fee_atoms} atoms (--max-fee-atoms)"
        ),
    ))
}

fn refused(code: &str, message: String) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("refused before signing: {code}: {message}"),
    )
}

fn lane_row(
    data_dir: &std::path::Path,
    lane_id: &str,
    export_cursor: Option<u64>,
    export_limit: Option<usize>,
) -> io::Result<Option<ExternalLaneInfoRow>> {
    let report = external_lane_info(ExternalLaneInfoOptions {
        data_dir: data_dir.to_path_buf(),
        lane_id: Some(lane_id.to_string()),
        release_cursor: None,
        release_limit: Some(1),
        export_cursor,
        export_limit,
    })?;
    Ok(report.lanes.into_iter().find(|row| row.lane_id == lane_id))
}

/// The code of an `ExternalLaneExportOperationV1::validate` error, by the
/// field its message names.
fn export_validate_code(error: &str) -> &'static str {
    if error.contains("external_lane_export_v1.base_recipient") {
        "external_lane_export_recipient_invalid"
    } else if error.contains("external_lane_export_v1.sender") {
        "external_lane_export_sender_invalid"
    } else if error.contains("external_lane_export_v1.amount_atoms") {
        "external_lane_export_amount_zero"
    } else {
        "external_lane_export_invalid"
    }
}

/// `--amount-atoms`: ASCII digits only (no sign, space or separator) that fit
/// a u64; `str::parse::<u64>` alone would take a leading `+`.
pub fn parse_export_amount_atoms(text: &str) -> Result<u64, String> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("--amount-atoms must be digits only".to_string());
    }
    text.parse::<u64>()
        .map_err(|_| "--amount-atoms must fit a u64".to_string())
}

/// `--max-fee-atoms`: ASCII digits only that fit a u64.
pub fn parse_export_max_fee_atoms(text: &str) -> Result<u64, String> {
    if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("--max-fee-atoms must be digits only".to_string());
    }
    text.parse::<u64>()
        .map_err(|_| "--max-fee-atoms must fit a u64".to_string())
}

/// Checks the export against the lane and the sender's balance, then, unless
/// `dry_run` (which stops after the checks, opens no key file and signs
/// nothing), signs and submits it. Every refusal the command can know from
/// the node's current state is returned before the key signs anything,
/// against the ledger asset admission executes the export on: the pending
/// plain transfers, `payment_v2` entries and asset transactions
/// (`asset_fee_quote`'s `export_refusal`, after admission's mempool limits),
/// and a quoted minimum fee above `max_fee_atoms`
/// (`external_lane_export_fee_above_ceiling`). The cases that can still come
/// after signing, all at admission and without a fee, are: a pending escrow,
/// NFT or offer entry the export makes stale (admission re-runs them after
/// it), and the holder sends the export again once that entry is in a block
/// or has left the mempool; a transaction another submitter gets admitted
/// between the check and the submission; and, only if the fee rule changes, a
/// block committed between the quote and the submission that raises the
/// minimum fee (`fee_too_low`), and the holder runs the command again. Under
/// the current fee rule an export's minimum fee does not depend on the ledger
/// (its state-expansion part is zero), so that last case cannot happen for an
/// export now.
pub fn external_lane_export(
    options: ExternalLaneExportOptions,
) -> io::Result<ExternalLaneExportReport> {
    let lane = lane_row(&options.data_dir, &options.lane_id, None, Some(1))?.ok_or_else(|| {
        refused(
            EXTERNAL_LANE_NOT_CONFIGURED,
            format!(
                "external lane `{}` does not exist on this chain",
                options.lane_id
            ),
        )
    })?;
    let policy = &lane.config.policy;
    if !policy.exports_enabled {
        return Err(refused(
            EXTERNAL_LANE_EXPORT_NOT_SUPPORTED,
            format!(
                "exports are switched off on external lane `{}`",
                lane.lane_id
            ),
        ));
    }
    if lane.release_cap.active_cap_bps == 0 {
        return Err(refused(
            EXTERNAL_LANE_EXPORT_HALTED,
            format!(
                "external lane `{}` is halted: its active release cap is zero",
                lane.lane_id
            ),
        ));
    }
    if options.amount_atoms == 0 {
        return Err(refused(
            "external_lane_export_amount_zero",
            "the amount must be at least one atom".to_string(),
        ));
    }
    // Whether the amount fits is the dry run's (below): the committed
    // `released - exported` misses pending releases and exports.
    let sender = if options.dry_run {
        options.sender.clone().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "a dry run opens no key file: pass the sender's address with --sender",
            )
        })?
    } else {
        let address = read_key_file(&options.key_file)?.address;
        if let Some(sender) = options
            .sender
            .as_deref()
            .filter(|sender| *sender != address)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("--sender `{sender}` is not the key file's address `{address}`"),
            ));
        }
        address
    };
    let operation = ExternalLaneExportOperationV1 {
        sender: sender.clone(),
        lane_id: lane.lane_id.clone(),
        amount_atoms: options.amount_atoms,
        base_recipient: options.base_recipient.clone(),
    };
    operation
        .validate()
        .map_err(|error| refused(export_validate_code(&error), error))?;
    if operation.base_recipient == policy.lock_contract_address {
        return Err(refused(
            EXTERNAL_LANE_EXPORT_RECIPIENT_IS_LOCK,
            format!(
                "{} is external lane `{}`'s lock contract, which a release can never pay",
                operation.base_recipient, lane.lane_id
            ),
        ));
    }
    if operation.base_recipient == policy.token_address {
        return Err(refused(
            EXTERNAL_LANE_EXPORT_RECIPIENT_IS_TOKEN,
            format!(
                "{} is external lane `{}`'s token contract; a release to it would strand the PFT",
                operation.base_recipient, lane.lane_id
            ),
        ));
    }
    let operation = AssetTransactionOperation::ExternalLaneExportV1(operation);
    let operation_json = serde_json::to_string(&operation)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let quote = asset_fee_quote(AssetFeeQuoteOptions {
        data_dir: options.data_dir.clone(),
        source: sender.clone(),
        operation_json,
        sequence: None,
    })
    .map_err(|error| match error.kind() {
        // No account holds nothing, so nothing covers the export.
        io::ErrorKind::NotFound => refused("insufficient_funds", error.to_string()),
        _ => error,
    })?;
    export_quote_was_checked(&quote)?;
    export_fee_within_ceiling(quote.minimum_fee, options.max_fee_atoms)?;
    let needed = u128::from(options.amount_atoms)
        + u128::from(quote.minimum_fee)
        + u128::from(quote.account_reserve);
    if u128::from(quote.sender_balance) < needed {
        return Err(refused(
            "insufficient_funds",
            format!(
                "balance {} cannot cover the amount {}, the fee {} and the account reserve {}",
                quote.sender_balance,
                options.amount_atoms,
                quote.minimum_fee,
                quote.account_reserve
            ),
        ));
    }
    // The lane rules on the ledger asset admission executes the export on
    // (pending transfers, payment_v2 entries and asset transactions), by
    // execution's own export path: pending releases and exports count as
    // admission counts them; later families do not.
    if let Some(refusal) = &quote.export_refusal {
        return Err(refused(&refusal.code, refusal.message.clone()));
    }
    let fee = quote.minimum_fee;
    let sequence = quote.sequence;
    if options.dry_run {
        return Ok(ExternalLaneExportReport {
            schema: "postfiat-external-lane-export-v1".to_string(),
            lane_id: lane.lane_id,
            sender,
            amount_atoms: options.amount_atoms,
            base_recipient: options.base_recipient,
            fee,
            sequence,
            dry_run: true,
            submitted: false,
            tx_id: None,
            signed_asset_transaction: None,
            quote: Some(quote),
        });
    }
    let signed = wallet_sign_asset_transaction(WalletSignAssetTransactionOptions {
        key_file: options.key_file.clone(),
        chain_id: quote.chain_id,
        genesis_hash: quote.genesis_hash,
        protocol_version: quote.protocol_version,
        fee,
        sequence,
        expected_source: Some(quote.source),
        operation: quote.operation,
    })?;
    let signed_json = serde_json::to_string(&signed)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let tx_id =
        submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
            data_dir: options.data_dir.clone(),
            signed_asset_transaction_json: signed_json,
        })?
        .tx_id;
    Ok(ExternalLaneExportReport {
        schema: "postfiat-external-lane-export-v1".to_string(),
        lane_id: lane.lane_id,
        sender,
        amount_atoms: options.amount_atoms,
        base_recipient: options.base_recipient,
        fee,
        sequence,
        dry_run: false,
        submitted: true,
        tx_id: Some(tx_id),
        signed_asset_transaction: Some(signed),
        quote: None,
    })
}

/// One export, by the accepted transaction's id or by its export nonce.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExternalLaneExportSelector {
    TxId(String),
    Nonce(u64),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLaneExportStatusOptions {
    pub data_dir: PathBuf,
    pub lane_id: String,
    pub selector: ExternalLaneExportSelector,
    /// Replay this node's block log (`tx --audit-block-log`) and report
    /// `block_log_verified`.
    pub audit: bool,
    /// Refuse when the packet found belongs to another sender: the export
    /// nonce is the lane's, not the sender's.
    pub sender: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLaneExportStatusReport {
    pub schema: String,
    pub lane_id: String,
    pub export_nonce: u64,
    pub sender: String,
    pub amount_atoms: u64,
    pub base_recipient: String,
    pub exported_at_height: u64,
    pub tx_id: String,
    pub packet_digest: String,
    pub withdrawal_id_commitment: String,
    pub burn_tx_commitment: String,
    /// The receipt is linked from a block in this node's committed log. Not
    /// a replay of that log: see `block_log_verified`.
    pub in_committed_block: bool,
    /// With `audit`: this node replayed its whole block log and it verified.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_log_verified: Option<bool>,
    pub current_height: u64,
    /// The lane's `exported_atoms` and export count now.
    pub lane_exported_atoms: u64,
    pub lane_export_count: u64,
    pub base_payout: String,
}

/// Finds the packet in the lane's stored export pages, newest first.
pub fn external_lane_export_status(
    options: ExternalLaneExportStatusOptions,
) -> io::Result<ExternalLaneExportStatusReport> {
    let not_configured = || {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!(
                "{EXTERNAL_LANE_NOT_CONFIGURED}: external lane `{}` does not exist on this chain",
                options.lane_id
            ),
        )
    };
    let mut cursor = match &options.selector {
        ExternalLaneExportSelector::Nonce(nonce) => Some(nonce.saturating_add(1)),
        ExternalLaneExportSelector::TxId(_) => None,
    };
    let (row, entry) = loop {
        let row = lane_row(
            &options.data_dir,
            &options.lane_id,
            cursor,
            Some(EXTERNAL_LANE_RELEASE_PAGE_SIZE),
        )?
        .ok_or_else(not_configured)?;
        let found = row
            .exports
            .iter()
            .find(|entry| match &options.selector {
                ExternalLaneExportSelector::Nonce(nonce) => entry.packet.export_nonce == *nonce,
                ExternalLaneExportSelector::TxId(tx_id) => entry.packet.tx_id == *tx_id,
            })
            .cloned();
        match (found, row.next_export_cursor, &options.selector) {
            (Some(entry), _, _) => break (row, entry),
            (None, Some(next), ExternalLaneExportSelector::TxId(_)) => cursor = Some(next),
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    format!(
                        "no stored export on external lane `{}` matches {}; {} exports, {} pruned",
                        options.lane_id,
                        match &options.selector {
                            ExternalLaneExportSelector::Nonce(nonce) => format!("nonce {nonce}"),
                            ExternalLaneExportSelector::TxId(tx_id) => format!("tx `{tx_id}`"),
                        },
                        row.export_count,
                        row.pruned_export_count
                    ),
                ))
            }
        }
    };
    if let Some(sender) = options.sender.as_deref() {
        if entry.packet.sender != sender {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "{EXTERNAL_LANE_EXPORT_OTHER_SENDER}: export nonce {} on external lane `{}` belongs to `{}`, not `{sender}`; the export nonce is the lane's, not the sender's",
                    entry.packet.export_nonce, options.lane_id, entry.packet.sender
                ),
            ));
        }
    }
    let ExternalLaneExportEntry {
        packet,
        packet_digest,
        withdrawal_id_commitment,
        burn_tx_commitment,
    } = entry;
    let (in_committed_block, block_log_verified) = match tx_finality(TxFinalityQueryOptions {
        data_dir: options.data_dir.clone(),
        tx_id: packet.tx_id.clone(),
        audit_block_log: options.audit,
    }) {
        Ok(report) => (
            report.confirmed && report.block.header.height == packet.exported_at_height,
            options.audit.then_some(report.block_log_verified),
        ),
        Err(error) if tx_finality_error_is_transaction_not_found(&error) => {
            (false, options.audit.then_some(false))
        }
        Err(error) => return Err(error),
    };
    let current_height = external_lane_info(ExternalLaneInfoOptions {
        data_dir: options.data_dir.clone(),
        lane_id: Some(options.lane_id.clone()),
        release_cursor: None,
        release_limit: Some(1),
        export_cursor: None,
        export_limit: Some(1),
    })?
    .current_height;
    Ok(ExternalLaneExportStatusReport {
        schema: "postfiat-external-lane-export-status-v1".to_string(),
        lane_id: row.lane_id,
        export_nonce: packet.export_nonce,
        sender: packet.sender,
        amount_atoms: packet.amount_atoms,
        base_recipient: packet.base_recipient,
        exported_at_height: packet.exported_at_height,
        tx_id: packet.tx_id,
        packet_digest,
        withdrawal_id_commitment,
        burn_tx_commitment,
        in_committed_block,
        block_log_verified,
        current_height,
        lane_exported_atoms: row.state.exported_atoms,
        lane_export_count: row.export_count,
        base_payout: EXTERNAL_LANE_EXPORT_BASE_PAYOUT_NOTE.to_string(),
    })
}
