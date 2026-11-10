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
    EXTERNAL_LANE_EXPORT_ABOVE_RELEASED, EXTERNAL_LANE_EXPORT_HALTED,
    EXTERNAL_LANE_EXPORT_NOT_SUPPORTED, EXTERNAL_LANE_EXPORT_RECIPIENT_IS_LOCK,
    EXTERNAL_LANE_NOT_CONFIGURED,
};
use serde::{Deserialize, Serialize};

use crate::{
    asset_fee_quote, external_lane_info, read_key_file,
    submit_signed_asset_transaction_json_to_mempool, tx_finality, wallet_sign_asset_transaction,
    AssetFeeQuoteOptions, ExternalLaneExportEntry, ExternalLaneInfoOptions, ExternalLaneInfoRow,
    SignedAssetTransactionJsonSubmitOptions, TxFinalityQueryOptions,
    WalletSignAssetTransactionOptions, EXTERNAL_LANE_RELEASE_PAGE_SIZE,
};

/// The Base recipient is the lane's token contract, which a release would
/// strand the PFT in.
pub const EXTERNAL_LANE_EXPORT_RECIPIENT_IS_TOKEN: &str = "external_lane_export_recipient_is_token";
/// The line every status read ends with until the Base release relay exists.
pub const EXTERNAL_LANE_EXPORT_BASE_PAYOUT_NOTE: &str =
    "the payout on Base needs a release relay that does not exist yet; this packet is recorded on PFTL only";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLaneExportOptions {
    pub data_dir: PathBuf,
    pub key_file: PathBuf,
    pub lane_id: String,
    pub amount_atoms: u64,
    pub base_recipient: String,
    /// Sign and return the transaction without submitting it.
    pub dry_run: bool,
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
    pub tx_id: String,
    pub signed_asset_transaction: SignedAssetTransaction,
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

/// Checks the export against the lane and the sender's balance, then signs
/// it and, unless `dry_run`, submits it. Every refusal is returned before the
/// key signs anything.
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
    let exportable = lane
        .state
        .released_atoms
        .saturating_sub(lane.state.exported_atoms);
    if options.amount_atoms == 0 {
        return Err(refused(
            "external_lane_export_amount_zero",
            "the amount must be at least one atom".to_string(),
        ));
    }
    if options.amount_atoms > exportable {
        return Err(refused(
            EXTERNAL_LANE_EXPORT_ABOVE_RELEASED,
            format!(
                "{} atoms is above what external lane `{}` can take back: released {} minus exported {} is {exportable}",
                options.amount_atoms, lane.lane_id, lane.state.released_atoms, lane.state.exported_atoms
            ),
        ));
    }
    let sender = read_key_file(&options.key_file)?.address;
    let operation = ExternalLaneExportOperationV1 {
        sender: sender.clone(),
        lane_id: lane.lane_id.clone(),
        amount_atoms: options.amount_atoms,
        base_recipient: options.base_recipient.clone(),
    };
    operation
        .validate()
        .map_err(|error| refused("external_lane_export_recipient_invalid", error))?;
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
    let fee = quote.minimum_fee;
    let sequence = quote.sequence;
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
    let tx_id = if options.dry_run {
        postfiat_execution::asset_transaction_tx_id(&signed)
    } else {
        let signed_json = serde_json::to_string(&signed)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        submit_signed_asset_transaction_json_to_mempool(SignedAssetTransactionJsonSubmitOptions {
            data_dir: options.data_dir.clone(),
            signed_asset_transaction_json: signed_json,
        })?
        .tx_id
    };
    Ok(ExternalLaneExportReport {
        schema: "postfiat-external-lane-export-v1".to_string(),
        lane_id: lane.lane_id,
        sender,
        amount_atoms: options.amount_atoms,
        base_recipient: options.base_recipient,
        fee,
        sequence,
        dry_run: options.dry_run,
        submitted: !options.dry_run,
        tx_id,
        signed_asset_transaction: signed,
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
    /// The block carrying the export is in this node's committed log.
    pub block_final: bool,
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
    let ExternalLaneExportEntry {
        packet,
        packet_digest,
        withdrawal_id_commitment,
        burn_tx_commitment,
    } = entry;
    let block_final = tx_finality(TxFinalityQueryOptions {
        data_dir: options.data_dir.clone(),
        tx_id: packet.tx_id.clone(),
        audit_block_log: false,
    })
    .is_ok_and(|report| {
        report.confirmed && report.block.header.height == packet.exported_at_height
    });
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
        block_final,
        current_height,
        lane_exported_atoms: row.state.exported_atoms,
        lane_export_count: row.export_count,
        base_payout: EXTERNAL_LANE_EXPORT_BASE_PAYOUT_NOTE.to_string(),
    })
}
