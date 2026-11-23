// Additive read models. The bridge crate's shared and consensus types stay unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgeRouteReadRow {
    #[serde(flatten)]
    pub route: PftlUniswapRouteStatusRow,
    pub spread_recipient: Option<String>,
    pub funding_sequence: Option<u64>,
}
impl std::ops::Deref for NavcoinBridgeRouteReadRow {
    type Target = PftlUniswapRouteStatusRow;
    fn deref(&self) -> &Self::Target {
        &self.route
    }
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgeRoutesReadReport {
    pub schema: String,
    pub route_count: u64,
    pub routes: Vec<NavcoinBridgeRouteReadRow>,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NavcoinBridgeSupplyReadReport {
    #[serde(flatten)]
    pub supply: PftlUniswapSupplyStatusReport,
    pub spread_recipient: Option<String>,
    pub funding_sequence: Option<u64>,
}
impl std::ops::Deref for NavcoinBridgeSupplyReadReport {
    type Target = PftlUniswapSupplyStatusReport;
    fn deref(&self) -> &Self::Target {
        &self.supply
    }
}
fn b23_route_spread_recipient(route: &PftlUniswapConsensusRouteState) -> Option<String> {
    route
        .v2
        .as_ref()
        .and_then(|v2| v2.primary_market_policy.spread_recipient.clone())
}
fn b23_route_funding_sequence(route: &PftlUniswapConsensusRouteState) -> Option<u64> {
    route
        .v2
        .as_ref()
        .filter(|v2| v2.primary_market_policy.spread_recipient.is_some())
        .map(|v2| v2.funding_sequence.unwrap_or(0))
}
fn b23_routes_read_report(
    report: PftlUniswapRoutesStatusReport,
    ledger: Option<&LedgerState>,
) -> NavcoinBridgeRoutesReadReport {
    NavcoinBridgeRoutesReadReport {
        schema: report.schema,
        route_count: report.route_count,
        routes: report
            .routes
            .into_iter()
            .map(|route| {
                let spread_recipient = ledger
                    .and_then(|ledger| ledger.pftl_uniswap_route(&route.route_id))
                    .and_then(b23_route_spread_recipient);
                let funding_sequence = ledger
                    .and_then(|ledger| ledger.pftl_uniswap_route(&route.route_id))
                    .and_then(b23_route_funding_sequence);
                NavcoinBridgeRouteReadRow {
                    funding_sequence,
                    route,
                    spread_recipient,
                }
            })
            .collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapReceiptReadReport {
    pub schema: String,
    pub tx_id: String,
    pub block_height: u64,
    pub route_id: String,
    pub owner: String,
    pub redemption_nonce: String,
    pub accepted: bool,
    pub code: String,
    pub message: String,
    pub consensus_receipt: Option<PftlUniswapConsensusReceipt>,
}

fn b23_receipt_read_error(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
fn b23_unique_transaction_receipt<'a>(
    receipts: &'a [Receipt],
    tx_id: &str,
) -> io::Result<&'a Receipt> {
    let mut matches = receipts.iter().filter(|receipt| receipt.tx_id == tx_id);
    let receipt = matches.next().ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "transaction receipt unavailable")
    })?;
    if matches.next().is_some() {
        return Err(b23_receipt_read_error("duplicate transaction receipts"));
    }
    Ok(receipt)
}

/// Read persisted evidence through a read-only store and non-repairing tip reader.
/// Missing directories/keys fail closed; legacy heads are verified without upgrade.
/// No replay, transaction submission, index writes or storage metadata repair.
/// Ambiguous same-route/owner/amount redemptions in a block fail closed because
/// the consensus receipt schema does not persist a transaction id or nonce.
pub fn pftl_uniswap_receipt(
    options: NodeOptions,
    tx_id: &str,
) -> io::Result<PftlUniswapReceiptReadReport> {
    if tx_id.len() != 96
        || !tx_id
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "tx_id must be 96 lowercase hexadecimal characters",
        ));
    }
    let store = NodeStore::try_new_read_only(&options.data_dir)?;
    let blocks = store.read_blocks()?;
    let receipts = store.read_receipts()?;
    let transaction_receipt = b23_unique_transaction_receipt(&receipts, tx_id)?;
    let matching_blocks = blocks
        .blocks
        .iter()
        .filter(|block| block.receipt_ids.iter().any(|id| id == tx_id))
        .collect::<Vec<_>>();
    let block = match matching_blocks.as_slice() {
        [block] => *block,
        _ => {
            return Err(b23_receipt_read_error(
                "transaction must be linked to exactly one persisted block",
            ))
        }
    };
    if block.header.batch_kind != "transparent" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "transaction is not a public primary redemption",
        ));
    }
    let genesis = store.read_genesis()?;
    let archive = store.read_batch_archive()?;
    let matching_archives = archive
        .batches
        .iter()
        .filter(|entry| {
            entry.batch_kind == block.header.batch_kind && entry.batch_id == block.header.batch_id
        })
        .collect::<Vec<_>>();
    let entry = match matching_archives.as_slice() {
        [entry] => *entry,
        _ => {
            return Err(b23_receipt_read_error(
                "unique archived transaction batch unavailable",
            ))
        }
    };
    verify_archived_payload(&genesis, block, entry)?;
    let batch: TransactionBatch = parse_archived_payload(block, entry)?;
    let mut target = None;
    for (index, transaction) in batch.asset_transactions.iter().enumerate() {
        let id = asset_transaction_tx_id(transaction);
        let position = batch.transactions.len() + batch.payments_v2.len() + index;
        if block.receipt_ids.get(position) != Some(&id) {
            return Err(b23_receipt_read_error(
                "archived asset transaction does not match block receipt position",
            ));
        }
        if id == tx_id {
            if target.is_some() {
                return Err(b23_receipt_read_error("duplicate archived transaction"));
            }
            if let postfiat_types::AssetTransactionOperation::PftlUniswapPrimaryRedeem(operation) =
                &transaction.unsigned.operation
            {
                target = Some(operation);
            }
        }
    }
    let operation = target.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "transaction is not a public primary redemption",
        )
    })?;
    let consensus_receipt = if transaction_receipt.accepted {
        let mut equivalent_accepted = 0;
        for transaction in &batch.asset_transactions {
            if let postfiat_types::AssetTransactionOperation::PftlUniswapPrimaryRedeem(other) =
                &transaction.unsigned.operation
            {
                if other.route_id == operation.route_id
                    && other.owner == operation.owner
                    && other.nav_amount_atoms == operation.nav_amount_atoms
                {
                    let receipt = b23_unique_transaction_receipt(
                        &receipts,
                        &asset_transaction_tx_id(transaction),
                    )?;
                    if receipt.accepted {
                        equivalent_accepted += 1;
                    }
                }
            }
        }
        if equivalent_accepted != 1 {
            return Err(b23_receipt_read_error("ambiguous_redemption_receipt: multiple accepted redemptions share route, owner and amount in this block"));
        }
        let ledger = store.read_ledger()?;
        let matching = ledger
            .pftl_uniswap_receipts
            .iter()
            .filter(|receipt| {
                receipt.block_height == block.header.height
                    && receipt.transition == "primary_redeemed"
                    && receipt.route_id == operation.route_id
                    && receipt.wallet.as_deref() == Some(operation.owner.as_str())
                    && receipt.amount_atoms == Some(operation.nav_amount_atoms)
            })
            .collect::<Vec<_>>();
        let receipt = match matching.as_slice() {
            [receipt] => *receipt,
            _ => {
                return Err(b23_receipt_read_error(
                    "unique persisted consensus redemption receipt unavailable",
                ))
            }
        };
        receipt.validate().map_err(|e| b23_receipt_read_error(&e))?;
        if receipt.receipt_hash
            != postfiat_types::pftl_uniswap_consensus_receipt_computed_hash(receipt)
        {
            return Err(b23_receipt_read_error(
                "consensus redemption receipt hash mismatch",
            ));
        }
        let historical_receipts = ledger
            .pftl_uniswap_receipts
            .iter()
            .filter(|receipt| receipt.block_height <= block.header.height)
            .cloned()
            .collect::<Vec<_>>();
        let root = postfiat_types::pftl_uniswap_consensus_receipt_root(&historical_receipts)
            .map_err(|e| b23_receipt_read_error(&e))?;
        if block.header.pftl_uniswap_receipt_root.as_deref() != Some(root.as_str()) {
            return Err(b23_receipt_read_error(
                "persisted consensus receipts do not match the block receipt root",
            ));
        }
        if let Some(settlement) = &receipt.redemption_settlement {
            if settlement.settlement_recipient != operation.settlement_recipient
                || settlement.settlement_value_atoms < operation.min_settlement_value_atoms
                || operation
                    .settlement_source_asset_id
                    .as_ref()
                    .is_some_and(|asset| *asset != settlement.settlement_asset_id)
            {
                return Err(b23_receipt_read_error(
                    "consensus settlement does not match archived redemption",
                ));
            }
        }
        Some(receipt.clone())
    } else {
        None
    };
    let current_tip = read_chain_tip_or_reconstruct_for_genesis_read_only(&store, &genesis)?;
    if store.read_blocks()?.tip_hash() != blocks.tip_hash()
        || current_tip.block_hash != blocks.tip_hash()
    {
        return Err(b23_receipt_read_error(
            "concurrent_commit_retry: finalized tip changed during receipt read",
        ));
    }
    Ok(PftlUniswapReceiptReadReport {
        schema: "postfiat-pftl-uniswap-receipt-read-v1".to_string(),
        tx_id: tx_id.to_string(),
        block_height: block.header.height,
        route_id: operation.route_id.clone(),
        owner: operation.owner.clone(),
        redemption_nonce: operation.redemption_nonce.clone(),
        accepted: transaction_receipt.accepted,
        code: transaction_receipt.code.clone(),
        message: transaction_receipt.message.clone(),
        consensus_receipt,
    })
}

/// RPC admission preview only: run the unchanged offer executor against the
/// same pending-mempool projection used by ordinary offer admission. The live
/// submission path must share admission's process-local redb handle/writer lease;
/// the offline read-only constructor opens a competing handle. Candidate execution
/// stays in memory and never persists ledger, mempool, receipts, or a repaired tip.
pub fn b23_facility_offer_preview(
    options: SignedOfferTransactionJsonSubmitOptions,
) -> io::Result<Receipt> {
    if options.signed_offer_transaction_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "signed offer transaction JSON exceeds admission size limit",
        ));
    }
    let signed: SignedOfferTransaction =
        serde_json::from_str(&options.signed_offer_transaction_json).map_err(invalid_data)?;
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let store = NodeStore::try_new(options.data_dir)?;
    let genesis = store.read_genesis()?;
    let ledger = read_fastpay_ledger(&store)?;
    let mempool = store.read_mempool()?;
    let height = read_chain_tip_or_reconstruct_for_genesis_read_only(&store, &genesis)?
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height exhausted"))?;
    let compatibility = asset_execution_compatibility_from_store(&store)?;
    let mut candidate = ledger_after_executable_mempool(
        &genesis,
        ledger,
        &mempool,
        height,
        compatibility,
        &crate::mempool_proposals::pending_asset_orchard_balances(&store, &mempool)?,
    )?;
    Ok(execute_offer_transaction(
        &genesis,
        &mut candidate,
        &signed,
        height,
    ))
}
