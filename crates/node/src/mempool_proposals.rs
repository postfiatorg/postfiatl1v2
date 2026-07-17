use super::execution_actions::execute_fastlane_primary_for_chain;
use super::*;

pub fn transfer(options: TransferOptions) -> io::Result<Receipt> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let mut ledger = store.read_ledger()?;
    let signed = build_signed_transfer(
        &genesis,
        &ledger,
        &options.data_dir,
        options.key_file,
        options.to,
        options.amount,
    )?;

    let receipt = execute_transfer(&genesis, &mut ledger, &signed);
    if receipt.accepted {
        store.write_ledger(&ledger)?;
    }
    store.append_receipt(receipt.clone())?;
    Ok(receipt)
}

pub fn create_transfer_batch(options: BatchTransferOptions) -> io::Result<TransactionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let signed = build_signed_transfer(
        &genesis,
        &ledger,
        &options.data_dir,
        options.key_file,
        options.to,
        options.amount,
    )?;
    let batch_domain = mempool_batch_domain(&genesis);
    let batch = build_transaction_batch(&batch_domain, vec![signed])
        .map_err(invalid_data)?
        .batch;
    write_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn submit_transfer_to_mempool(options: TransferOptions) -> io::Result<MempoolEntry> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let key_file = read_transfer_key_file(&options.data_dir, options.key_file)?;
    let sender = ledger.account(&key_file.address).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("sender account `{}` not found", key_file.address),
        )
    })?;
    let mempool = store.read_mempool()?;
    enforce_mempool_admission_limits(&mempool, &key_file.address)?;
    let sequence = next_pending_sender_sequence(&mempool, &key_file.address, sender.sequence)?;
    let signed = build_signed_transfer_for_key(
        &genesis,
        &ledger,
        &key_file,
        options.to,
        options.amount,
        sequence,
    )?;
    admit_signed_transfer_to_mempool(&options.data_dir, signed)
}

pub fn submit_signed_transfer_to_mempool(
    options: SignedTransferSubmitOptions,
) -> io::Result<MempoolEntry> {
    let signed = read_signed_transfer_file(&options.transfer_file)?;
    admit_signed_transfer_to_mempool(&options.data_dir, signed)
}

pub fn submit_signed_transfer_json_to_mempool(
    options: SignedTransferJsonSubmitOptions,
) -> io::Result<MempoolEntry> {
    if options.signed_transfer_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("signed transfer JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let signed: SignedTransfer =
        serde_json::from_str(&options.signed_transfer_json).map_err(invalid_data)?;
    admit_signed_transfer_to_mempool(&options.data_dir, signed)
}

pub fn submit_signed_payment_v2_json_to_mempool(
    options: SignedPaymentV2JsonSubmitOptions,
) -> io::Result<MempoolPaymentV2Entry> {
    if options.signed_payment_v2_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("signed payment_v2 JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let signed: SignedPaymentV2 =
        serde_json::from_str(&options.signed_payment_v2_json).map_err(invalid_data)?;
    admit_signed_payment_v2_to_mempool(&options.data_dir, signed)
}

pub fn submit_signed_asset_transaction_json_to_mempool(
    options: SignedAssetTransactionJsonSubmitOptions,
) -> io::Result<MempoolAssetTransactionEntry> {
    if options.signed_asset_transaction_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("signed asset transaction JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let signed: SignedAssetTransaction =
        serde_json::from_str(&options.signed_asset_transaction_json).map_err(invalid_data)?;
    admit_signed_asset_transaction_to_mempool(&options.data_dir, signed)
}

pub fn submit_signed_escrow_transaction_json_to_mempool(
    options: SignedEscrowTransactionJsonSubmitOptions,
) -> io::Result<MempoolEscrowTransactionEntry> {
    if options.signed_escrow_transaction_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "signed escrow transaction JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"
            ),
        ));
    }
    let signed: SignedEscrowTransaction =
        serde_json::from_str(&options.signed_escrow_transaction_json).map_err(invalid_data)?;
    admit_signed_escrow_transaction_to_mempool(&options.data_dir, signed)
}

pub fn submit_signed_nft_transaction_json_to_mempool(
    options: SignedNftTransactionJsonSubmitOptions,
) -> io::Result<MempoolNftTransactionEntry> {
    if options.signed_nft_transaction_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("signed nft transaction JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let signed: SignedNftTransaction =
        serde_json::from_str(&options.signed_nft_transaction_json).map_err(invalid_data)?;
    admit_signed_nft_transaction_to_mempool(&options.data_dir, signed)
}

pub fn submit_signed_offer_transaction_json_to_mempool(
    options: SignedOfferTransactionJsonSubmitOptions,
) -> io::Result<MempoolOfferTransactionEntry> {
    if options.signed_offer_transaction_json.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("signed offer transaction JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let signed: SignedOfferTransaction =
        serde_json::from_str(&options.signed_offer_transaction_json).map_err(invalid_data)?;
    admit_signed_offer_transaction_to_mempool(&options.data_dir, signed)
}

fn admit_signed_transfer_to_mempool(
    data_dir: &Path,
    signed: SignedTransfer,
) -> io::Result<MempoolEntry> {
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    if signed.unsigned.chain_id != genesis.chain_id
        || signed.unsigned.genesis_hash != genesis_hash(&genesis)
        || signed.unsigned.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "signed transfer chain domain does not match local node",
        ));
    }
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    enforce_mempool_admission_limits(&mempool, &signed.unsigned.from)?;
    let tx_id = transfer_tx_id(&signed);
    if mempool_has_tx_id(&mempool, &tx_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("transaction `{tx_id}` is already pending"),
        ));
    }
    if mempool.has_sender_sequence(&signed.unsigned.from, signed.unsigned.sequence) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "sender `{}` already has pending sequence {}",
                signed.unsigned.from, signed.unsigned.sequence
            ),
        ));
    }
    let mut dry_run_ledger = ledger.clone();
    for pending in &mempool.pending {
        let pending_receipt = execute_transfer(&genesis, &mut dry_run_ledger, &pending.transfer);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    let dry_run_receipt = execute_transfer(&genesis, &mut dry_run_ledger, &signed);
    if !dry_run_receipt.accepted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mempool admission rejected `{}`: {}",
                dry_run_receipt.code, dry_run_receipt.message
            ),
        ));
    }
    for pending in &mempool.pending_payment_v2 {
        let pending_receipt = execute_payment_v2(&genesis, &mut dry_run_ledger, &pending.payment);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_asset_transactions {
        let pending_receipt = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
            asset_execution_compatibility,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    apply_pending_atomic_and_fastlane(
        &genesis,
        &ledger,
        &mut dry_run_ledger,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    for pending in &mempool.pending_escrow_transactions {
        let pending_receipt = execute_escrow_transaction(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_nft_transactions {
        let pending_receipt =
            execute_nft_transaction(&genesis, &mut dry_run_ledger, &pending.transaction);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_offer_transactions {
        let pending_receipt = execute_offer_transaction(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    let entry = MempoolEntry::new(tx_id, signed);
    store.append_mempool_entry(entry.clone())?;
    Ok(entry)
}

fn admit_signed_payment_v2_to_mempool(
    data_dir: &Path,
    signed: SignedPaymentV2,
) -> io::Result<MempoolPaymentV2Entry> {
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    if signed.unsigned.chain_id != genesis.chain_id
        || signed.unsigned.genesis_hash != genesis_hash(&genesis)
        || signed.unsigned.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "signed payment_v2 chain domain does not match local node",
        ));
    }
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    enforce_mempool_admission_limits(&mempool, &signed.unsigned.from)?;
    let tx_id = payment_v2_tx_id(&signed);
    if mempool_has_tx_id(&mempool, &tx_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("transaction `{tx_id}` is already pending"),
        ));
    }
    if mempool.has_sender_sequence(&signed.unsigned.from, signed.unsigned.sequence) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "sender `{}` already has pending sequence {}",
                signed.unsigned.from, signed.unsigned.sequence
            ),
        ));
    }
    let mut dry_run_ledger = ledger.clone();
    for pending in &mempool.pending {
        let pending_receipt = execute_transfer(&genesis, &mut dry_run_ledger, &pending.transfer);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_payment_v2 {
        let pending_receipt = execute_payment_v2(&genesis, &mut dry_run_ledger, &pending.payment);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    let dry_run_receipt = execute_payment_v2(&genesis, &mut dry_run_ledger, &signed);
    if !dry_run_receipt.accepted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mempool admission rejected `{}`: {}",
                dry_run_receipt.code, dry_run_receipt.message
            ),
        ));
    }
    for pending in &mempool.pending_asset_transactions {
        let pending_receipt = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
            asset_execution_compatibility,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    apply_pending_atomic_and_fastlane(
        &genesis,
        &ledger,
        &mut dry_run_ledger,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    for pending in &mempool.pending_escrow_transactions {
        let pending_receipt = execute_escrow_transaction(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_nft_transactions {
        let pending_receipt =
            execute_nft_transaction(&genesis, &mut dry_run_ledger, &pending.transaction);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_offer_transactions {
        let pending_receipt = execute_offer_transaction(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    let entry = MempoolPaymentV2Entry::new(tx_id, signed);
    store.append_mempool_payment_v2_entry(entry.clone())?;
    Ok(entry)
}

fn admit_signed_asset_transaction_to_mempool(
    data_dir: &Path,
    signed: SignedAssetTransaction,
) -> io::Result<MempoolAssetTransactionEntry> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility =
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);
    signed
        .validate_with_legacy_vault_bridge_consume_supply_operator(
            !asset_execution_compatibility.bridge_verification_rules_active(block_height),
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if signed.unsigned.chain_id != genesis.chain_id
        || signed.unsigned.genesis_hash != genesis_hash(&genesis)
        || signed.unsigned.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "signed asset transaction chain domain does not match local node",
        ));
    }
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    enforce_mempool_admission_limits(&mempool, &signed.unsigned.source)?;
    let tx_id = asset_transaction_tx_id(&signed);
    if mempool_has_tx_id(&mempool, &tx_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("transaction `{tx_id}` is already pending"),
        ));
    }
    if mempool.has_sender_sequence(&signed.unsigned.source, signed.unsigned.sequence) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "sender `{}` already has pending sequence {}",
                signed.unsigned.source, signed.unsigned.sequence
            ),
        ));
    }
    let mut dry_run_ledger = ledger.clone();
    for pending in &mempool.pending {
        let pending_receipt = execute_transfer(&genesis, &mut dry_run_ledger, &pending.transfer);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_payment_v2 {
        let pending_receipt = execute_payment_v2(&genesis, &mut dry_run_ledger, &pending.payment);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_asset_transactions {
        let pending_receipt = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
            asset_execution_compatibility,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    let dry_run_receipt = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut dry_run_ledger,
        &signed,
        block_height,
        asset_execution_compatibility,
    );
    if !dry_run_receipt.accepted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mempool admission rejected `{}`: {}",
                dry_run_receipt.code, dry_run_receipt.message
            ),
        ));
    }
    apply_pending_atomic_and_fastlane(
        &genesis,
        &ledger,
        &mut dry_run_ledger,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    for pending in &mempool.pending_escrow_transactions {
        let pending_receipt = execute_escrow_transaction(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_nft_transactions {
        let pending_receipt =
            execute_nft_transaction(&genesis, &mut dry_run_ledger, &pending.transaction);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_offer_transactions {
        let pending_receipt = execute_offer_transaction(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    let shielded = store.read_shielded()?;
    verify_global_issued_asset_supply_caps(&dry_run_ledger, &shielded).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("mempool admission violates global issued supply: {error}"),
        )
    })?;
    let entry = MempoolAssetTransactionEntry::new(tx_id, signed);
    store.append_mempool_asset_transaction_entry(entry.clone())?;
    Ok(entry)
}

#[allow(dead_code)] // P4 exposes the already-consensus-gated parsed admission path.
pub(super) fn admit_signed_atomic_swap_to_mempool(
    data_dir: &Path,
    signed: SignedAtomicSwapTransaction,
) -> io::Result<MempoolAtomicSwapEntry> {
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let encoded = serde_json::to_vec(&signed).map_err(invalid_data)?;
    if encoded.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("signed atomic swap JSON exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }

    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    if signed.unsigned.chain_id != genesis.chain_id
        || signed.unsigned.genesis_hash != genesis_hash(&genesis)
        || signed.unsigned.protocol_version != genesis.protocol_version
        || signed.unsigned.address_namespace != ADDRESS_NAMESPACE
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "signed atomic swap chain domain does not match local node",
        ));
    }
    let governance = store.read_governance()?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let compatibility =
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);
    if compatibility.atomic_swap_paused {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic_swap_paused: atomic swap admission is disabled by governance",
        ));
    }
    if !compatibility.atomic_swap_active(block_height) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("atomic_swap_not_active: atomic swap is not active at height {block_height}"),
        ));
    }

    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let tx_id = atomic_swap_transaction_tx_id(&signed);
    if mempool_has_tx_id(&mempool, &tx_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("transaction `{tx_id}` is already pending"),
        ));
    }
    for owner in [&signed.unsigned.leg_0.owner, &signed.unsigned.leg_1.owner] {
        enforce_mempool_admission_limits(&mempool, owner)?;
        if mempool_pending_count_for_sender(&mempool, owner) != 0 {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "atomic swap owner `{owner}` already has a pending transaction; sequence chains across atomic swaps are not admitted"
                ),
            ));
        }
    }
    for leg in [&signed.unsigned.leg_0, &signed.unsigned.leg_1] {
        if mempool.has_sender_sequence(&leg.owner, leg.sequence) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!(
                    "sender `{}` already has pending sequence {}",
                    leg.owner, leg.sequence
                ),
            ));
        }
    }

    let entry = MempoolAtomicSwapEntry::new(tx_id, signed);
    let mut prospective = mempool;
    prospective.pending_atomic_swaps.push(entry.clone());
    let shielded = store.read_shielded()?;
    verify_mempool_state(
        &genesis,
        &ledger,
        &shielded,
        &prospective,
        block_height,
        compatibility,
    )?;
    store.append_mempool_atomic_swap_entry(entry.clone())?;
    Ok(entry)
}

pub fn admit_fastlane_primary_to_mempool(
    data_dir: &Path,
    transaction: postfiat_types::FastLanePrimaryTransactionV1,
) -> io::Result<MempoolFastLanePrimaryEntry> {
    let encoded = transaction.canonical_bytes().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("FastLane primary encoding failed: {error:?}"),
        )
    })?;
    if encoded.len() > MAX_SIGNED_TRANSFER_JSON_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("FastLane primary transaction exceeds {MAX_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let mempool = store.read_mempool()?;
    if mempool.len() >= MAX_MEMPOOL_PENDING_TRANSACTIONS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "mempool pending limit reached",
        ));
    }
    let tx_id = bytes_to_hex(
        &transaction
            .tx_id()
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("FastLane tx id failed: {error:?}"),
                )
            })?
            .0,
    );
    if mempool_has_tx_id(&mempool, &tx_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("transaction `{tx_id}` is already pending"),
        ));
    }
    let account_debit = match &transaction.operation {
        postfiat_types::FastLanePrimaryOperationV1::Deposit { signed } => {
            Some((&signed.deposit.source_address, signed.deposit.sequence))
        }
        postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit { signed } => {
            Some((&signed.deposit.source_address, signed.deposit.sequence))
        }
        _ => None,
    };
    if let Some((source_address, sequence)) = account_debit {
        enforce_mempool_admission_limits(&mempool, source_address)?;
        if mempool.has_sender_sequence(source_address, sequence) {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "FastLane primary sender sequence is already pending",
            ));
        }
    }
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let compatibility = asset_execution_compatibility_from_store(&store)?;
    let mut prefix = mempool.clone();
    prefix.pending_escrow_transactions.clear();
    prefix.pending_nft_transactions.clear();
    prefix.pending_offer_transactions.clear();
    let mut dry_run_ledger = ledger_after_executable_mempool(
        &genesis,
        ledger.clone(),
        &prefix,
        block_height,
        compatibility,
    )?;
    let receipt = execute_fastlane_primary_for_chain(
        &genesis,
        &mut dry_run_ledger,
        &transaction,
        block_height,
    );
    if !receipt.accepted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "FastLane primary admission rejected `{}`: {}",
                receipt.code, receipt.message
            ),
        ));
    }
    verify_global_issued_asset_supply_caps(&dry_run_ledger, &shielded).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("FastLane admission violates global issued supply: {error}"),
        )
    })?;
    let entry = MempoolFastLanePrimaryEntry::new(tx_id, transaction);
    store.append_mempool_fastlane_primary_entry(entry.clone())?;
    Ok(entry)
}

fn admit_signed_escrow_transaction_to_mempool(
    data_dir: &Path,
    signed: SignedEscrowTransaction,
) -> io::Result<MempoolEscrowTransactionEntry> {
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    if signed.unsigned.chain_id != genesis.chain_id
        || signed.unsigned.genesis_hash != genesis_hash(&genesis)
        || signed.unsigned.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "signed escrow transaction chain domain does not match local node",
        ));
    }
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    enforce_mempool_admission_limits(&mempool, &signed.unsigned.source)?;
    let tx_id = escrow_transaction_tx_id(&signed);
    if mempool_has_tx_id(&mempool, &tx_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("transaction `{tx_id}` is already pending"),
        ));
    }
    if mempool.has_sender_sequence(&signed.unsigned.source, signed.unsigned.sequence) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "sender `{}` already has pending sequence {}",
                signed.unsigned.source, signed.unsigned.sequence
            ),
        ));
    }
    let mut dry_run_ledger = ledger.clone();
    for pending in &mempool.pending {
        let pending_receipt = execute_transfer(&genesis, &mut dry_run_ledger, &pending.transfer);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_payment_v2 {
        let pending_receipt = execute_payment_v2(&genesis, &mut dry_run_ledger, &pending.payment);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_asset_transactions {
        let pending_receipt = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
            asset_execution_compatibility,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    apply_pending_atomic_and_fastlane(
        &genesis,
        &ledger,
        &mut dry_run_ledger,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    for pending in &mempool.pending_escrow_transactions {
        let pending_receipt = execute_escrow_transaction(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    let dry_run_receipt =
        execute_escrow_transaction(&genesis, &mut dry_run_ledger, &signed, block_height);
    if !dry_run_receipt.accepted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mempool admission rejected `{}`: {}",
                dry_run_receipt.code, dry_run_receipt.message
            ),
        ));
    }
    for pending in &mempool.pending_nft_transactions {
        let pending_receipt =
            execute_nft_transaction(&genesis, &mut dry_run_ledger, &pending.transaction);
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_offer_transactions {
        let pending_receipt = execute_offer_transaction(
            &genesis,
            &mut dry_run_ledger,
            &pending.transaction,
            block_height,
        );
        if !pending_receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}",
                    pending.tx_id, pending_receipt.message
                ),
            ));
        }
    }
    let entry = MempoolEscrowTransactionEntry::new(tx_id, signed);
    store.append_mempool_escrow_transaction_entry(entry.clone())?;
    Ok(entry)
}

fn admit_signed_nft_transaction_to_mempool(
    data_dir: &Path,
    signed: SignedNftTransaction,
) -> io::Result<MempoolNftTransactionEntry> {
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    if signed.unsigned.chain_id != genesis.chain_id
        || signed.unsigned.genesis_hash != genesis_hash(&genesis)
        || signed.unsigned.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "signed nft transaction chain domain does not match local node",
        ));
    }
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    enforce_mempool_admission_limits(&mempool, &signed.unsigned.source)?;
    let tx_id = nft_transaction_tx_id(&signed);
    if mempool_has_tx_id(&mempool, &tx_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("transaction `{tx_id}` is already pending"),
        ));
    }
    if mempool.has_sender_sequence(&signed.unsigned.source, signed.unsigned.sequence) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "sender `{}` already has pending sequence {}",
                signed.unsigned.source, signed.unsigned.sequence
            ),
        ));
    }
    let mut prefix = mempool.clone();
    prefix.pending_offer_transactions.clear();
    let mut dry_run_ledger = ledger_after_executable_mempool(
        &genesis,
        ledger,
        &prefix,
        block_height,
        asset_execution_compatibility,
    )?;
    let dry_run_receipt = execute_nft_transaction(&genesis, &mut dry_run_ledger, &signed);
    if !dry_run_receipt.accepted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mempool admission rejected `{}`: {}",
                dry_run_receipt.code, dry_run_receipt.message
            ),
        ));
    }
    let entry = MempoolNftTransactionEntry::new(tx_id, signed);
    store.append_mempool_nft_transaction_entry(entry.clone())?;
    Ok(entry)
}

fn admit_signed_offer_transaction_to_mempool(
    data_dir: &Path,
    signed: SignedOfferTransaction,
) -> io::Result<MempoolOfferTransactionEntry> {
    signed
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    if signed.unsigned.chain_id != genesis.chain_id
        || signed.unsigned.genesis_hash != genesis_hash(&genesis)
        || signed.unsigned.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "signed offer transaction chain domain does not match local node",
        ));
    }
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    enforce_mempool_admission_limits(&mempool, &signed.unsigned.source)?;
    let tx_id = offer_transaction_tx_id(&signed);
    if mempool_has_tx_id(&mempool, &tx_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("transaction `{tx_id}` is already pending"),
        ));
    }
    if mempool.has_sender_sequence(&signed.unsigned.source, signed.unsigned.sequence) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "sender `{}` already has pending sequence {}",
                signed.unsigned.source, signed.unsigned.sequence
            ),
        ));
    }
    let mut dry_run_ledger = ledger_after_executable_mempool(
        &genesis,
        ledger.clone(),
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    let dry_run_receipt =
        execute_offer_transaction(&genesis, &mut dry_run_ledger, &signed, block_height);
    if !dry_run_receipt.accepted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mempool admission rejected `{}`: {}",
                dry_run_receipt.code, dry_run_receipt.message
            ),
        ));
    }

    let entry = MempoolOfferTransactionEntry::new(tx_id, signed);
    store.append_mempool_offer_transaction_entry(entry.clone())?;
    Ok(entry)
}

fn mempool_has_tx_id(mempool: &MempoolState, tx_id: &str) -> bool {
    mempool.pending.iter().any(|entry| entry.tx_id == tx_id)
        || mempool
            .pending_payment_v2
            .iter()
            .any(|entry| entry.tx_id == tx_id)
        || mempool
            .pending_asset_transactions
            .iter()
            .any(|entry| entry.tx_id == tx_id)
        || mempool
            .pending_atomic_swaps
            .iter()
            .any(|entry| entry.tx_id == tx_id)
        || mempool
            .pending_fastlane_primary
            .iter()
            .any(|entry| entry.tx_id == tx_id)
        || mempool
            .pending_escrow_transactions
            .iter()
            .any(|entry| entry.tx_id == tx_id)
        || mempool
            .pending_nft_transactions
            .iter()
            .any(|entry| entry.tx_id == tx_id)
        || mempool
            .pending_offer_transactions
            .iter()
            .any(|entry| entry.tx_id == tx_id)
}

pub(super) fn mempool_pending_count_for_sender(mempool: &MempoolState, sender: &str) -> u64 {
    let count = mempool
        .pending
        .iter()
        .filter(|entry| entry.transfer.unsigned.from == sender)
        .count()
        + mempool
            .pending_payment_v2
            .iter()
            .filter(|entry| entry.payment.unsigned.from == sender)
            .count()
        + mempool
            .pending_asset_transactions
            .iter()
            .filter(|entry| entry.transaction.unsigned.source == sender)
            .count()
        + mempool
            .pending_atomic_swaps
            .iter()
            .filter(|entry| {
                entry.transaction.unsigned.leg_0.owner == sender
                    || entry.transaction.unsigned.leg_1.owner == sender
            })
            .count()
        + mempool
            .pending_fastlane_primary
            .iter()
            .filter(|entry| match &entry.transaction.operation {
                postfiat_types::FastLanePrimaryOperationV1::Deposit { signed } => {
                    signed.deposit.source_address == sender
                }
                postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit { signed } => {
                    signed.deposit.source_address == sender
                }
                _ => false,
            })
            .count()
        + mempool
            .pending_escrow_transactions
            .iter()
            .filter(|entry| entry.transaction.unsigned.source == sender)
            .count()
        + mempool
            .pending_nft_transactions
            .iter()
            .filter(|entry| entry.transaction.unsigned.source == sender)
            .count()
        + mempool
            .pending_offer_transactions
            .iter()
            .filter(|entry| entry.transaction.unsigned.source == sender)
            .count();
    u64::try_from(count).unwrap_or(u64::MAX)
}

pub(super) fn ledger_after_executable_mempool(
    genesis: &Genesis,
    mut ledger: LedgerState,
    mempool: &MempoolState,
    block_height: u64,
    asset_execution_compatibility: AssetExecutionCompatibility,
) -> io::Result<LedgerState> {
    for pending in &mempool.pending {
        let receipt = execute_transfer(genesis, &mut ledger, &pending.transfer);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool dry-run blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_payment_v2 {
        let receipt = execute_payment_v2(genesis, &mut ledger, &pending.payment);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool dry-run blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_asset_transactions {
        let receipt = execute_asset_transaction_with_compatibility(
            genesis,
            &mut ledger,
            &pending.transaction,
            block_height,
            asset_execution_compatibility,
        );
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool dry-run blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    if asset_execution_compatibility.atomic_swap_active(block_height) {
        for pending in &mempool.pending_atomic_swaps {
            let receipt = execute_atomic_swap_transaction_with_compatibility(
                genesis,
                &mut ledger,
                &pending.transaction,
                block_height,
                asset_execution_compatibility,
            );
            if !receipt.accepted {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "mempool dry-run blocked by stale pending `{}`: {}: {}",
                        pending.tx_id, receipt.code, receipt.message
                    ),
                ));
            }
        }
    }
    for pending in &mempool.pending_fastlane_primary {
        let receipt = execute_fastlane_primary_for_chain(
            genesis,
            &mut ledger,
            &pending.transaction,
            block_height,
        );
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool dry-run blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_escrow_transactions {
        let receipt =
            execute_escrow_transaction(genesis, &mut ledger, &pending.transaction, block_height);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool dry-run blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_nft_transactions {
        let receipt = execute_nft_transaction(genesis, &mut ledger, &pending.transaction);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool dry-run blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_offer_transactions {
        let receipt =
            execute_offer_transaction(genesis, &mut ledger, &pending.transaction, block_height);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool dry-run blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    Ok(ledger)
}

fn apply_pending_atomic_and_fastlane(
    genesis: &Genesis,
    base_ledger: &LedgerState,
    ledger: &mut LedgerState,
    mempool: &MempoolState,
    block_height: u64,
    asset_execution_compatibility: AssetExecutionCompatibility,
) -> io::Result<()> {
    // A transaction that is already stale against the committed ledger must
    // not wedge an unrelated admission. At the same time, a pending atomic
    // transaction that is valid without the candidate must still reject a
    // candidate that would invalidate it in canonical family order. Build the
    // no-candidate prefix independently so those two cases are not conflated.
    let mut without_candidate = base_ledger.clone();
    for pending in &mempool.pending {
        let receipt = execute_transfer(genesis, &mut without_candidate, &pending.transfer);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_payment_v2 {
        let receipt = execute_payment_v2(genesis, &mut without_candidate, &pending.payment);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    for pending in &mempool.pending_asset_transactions {
        let receipt = execute_asset_transaction_with_compatibility(
            genesis,
            &mut without_candidate,
            &pending.transaction,
            block_height,
            asset_execution_compatibility,
        );
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    if asset_execution_compatibility.atomic_swap_active(block_height) {
        for pending in &mempool.pending_atomic_swaps {
            let existing_receipt = execute_atomic_swap_transaction_with_compatibility(
                genesis,
                &mut without_candidate,
                &pending.transaction,
                block_height,
                asset_execution_compatibility,
            );
            if !existing_receipt.accepted {
                continue;
            }
            let receipt = execute_atomic_swap_transaction_with_compatibility(
                genesis,
                ledger,
                &pending.transaction,
                block_height,
                asset_execution_compatibility,
            );
            if !receipt.accepted {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "mempool admission blocked by stale pending `{}`: {}: {}",
                        pending.tx_id, receipt.code, receipt.message
                    ),
                ));
            }
        }
    }
    for pending in &mempool.pending_fastlane_primary {
        let existing_receipt = execute_fastlane_primary_for_chain(
            genesis,
            &mut without_candidate,
            &pending.transaction,
            block_height,
        );
        if !existing_receipt.accepted {
            continue;
        }
        let receipt =
            execute_fastlane_primary_for_chain(genesis, ledger, &pending.transaction, block_height);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "mempool admission blocked by stale pending `{}`: {}: {}",
                    pending.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    Ok(())
}

pub fn mempool_state(options: NodeOptions) -> io::Result<MempoolState> {
    let store = NodeStore::new(options.data_dir);
    store.read_mempool()
}

/// Remove only mempool entries that already have a terminal on-chain receipt.
///
/// A non-proposer may have admitted a transaction before another proposer
/// commits the same transaction. Consensus commit is authoritative; retaining
/// that local entry would permanently block the sender sequence after restart.
pub fn reconcile_terminal_mempool_entries(data_dir: &Path) -> io::Result<usize> {
    let store = NodeStore::new(data_dir);
    let terminal_tx_ids: BTreeSet<String> = store
        .read_receipts()?
        .into_iter()
        .map(|receipt| receipt.tx_id)
        .filter(|tx_id| !tx_id.is_empty())
        .collect();
    if terminal_tx_ids.is_empty() {
        return Ok(0);
    }

    let mut mempool = store.read_mempool()?;
    let before = mempool.len();
    mempool
        .pending
        .retain(|entry| !terminal_tx_ids.contains(&entry.tx_id));
    mempool
        .pending_payment_v2
        .retain(|entry| !terminal_tx_ids.contains(&entry.tx_id));
    mempool
        .pending_asset_transactions
        .retain(|entry| !terminal_tx_ids.contains(&entry.tx_id));
    mempool
        .pending_atomic_swaps
        .retain(|entry| !terminal_tx_ids.contains(&entry.tx_id));
    mempool
        .pending_fastlane_primary
        .retain(|entry| !terminal_tx_ids.contains(&entry.tx_id));
    mempool
        .pending_escrow_transactions
        .retain(|entry| !terminal_tx_ids.contains(&entry.tx_id));
    mempool
        .pending_nft_transactions
        .retain(|entry| !terminal_tx_ids.contains(&entry.tx_id));
    mempool
        .pending_offer_transactions
        .retain(|entry| !terminal_tx_ids.contains(&entry.tx_id));
    let removed = before.saturating_sub(mempool.len());
    if removed > 0 {
        store.write_mempool(&mempool)?;
    }
    Ok(removed)
}

pub fn verify_mempool(options: NodeOptions) -> io::Result<MempoolVerificationReport> {
    let store = NodeStore::new(options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let mempool = store.read_mempool()?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    verify_mempool_state(
        &genesis,
        &ledger,
        &shielded,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )
}

pub(super) fn verify_mempool_state(
    genesis: &Genesis,
    ledger: &LedgerState,
    shielded: &ShieldedState,
    mempool: &MempoolState,
    block_height: u64,
    asset_execution_compatibility: AssetExecutionCompatibility,
) -> io::Result<MempoolVerificationReport> {
    enforce_mempool_state_limits(mempool)?;
    let mut dry_run_ledger = ledger.clone();
    let mut tx_ids = HashSet::<String>::new();
    let mut sender_sequences = HashSet::<(String, u64)>::new();
    let mut senders = HashSet::<String>::new();
    let mut total_amount = 0u64;
    let mut total_fee = 0u64;

    for (index, entry) in mempool.pending.iter().enumerate() {
        if entry.tx_id.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool entry {index} has empty tx id"),
            ));
        }
        let expected_tx_id = transfer_tx_id(&entry.transfer);
        if entry.tx_id != expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool entry `{}` tx id mismatch", entry.tx_id),
            ));
        }
        if !tx_ids.insert(entry.tx_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate mempool tx id `{}`", entry.tx_id),
            ));
        }
        let sender_sequence = (
            entry.transfer.unsigned.from.clone(),
            entry.transfer.unsigned.sequence,
        );
        if !sender_sequences.insert(sender_sequence.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate mempool sender sequence `{}`:{}",
                    sender_sequence.0, sender_sequence.1
                ),
            ));
        }
        let receipt = execute_transfer(genesis, &mut dry_run_ledger, &entry.transfer);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool entry `{}` is not executable: {}: {}",
                    entry.tx_id, receipt.code, receipt.message
                ),
            ));
        }
        senders.insert(entry.transfer.unsigned.from.clone());
        total_amount = total_amount
            .checked_add(entry.transfer.unsigned.amount)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool amount total overflow")
            })?;
        total_fee = total_fee
            .checked_add(entry.transfer.unsigned.fee)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool fee total overflow")
            })?;
    }
    for (index, entry) in mempool.pending_payment_v2.iter().enumerate() {
        if entry.tx_id.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool payment_v2 entry {index} has empty tx id"),
            ));
        }
        let expected_tx_id = payment_v2_tx_id(&entry.payment);
        if entry.tx_id != expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool payment_v2 entry `{}` tx id mismatch", entry.tx_id),
            ));
        }
        if !tx_ids.insert(entry.tx_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate mempool tx id `{}`", entry.tx_id),
            ));
        }
        let sender_sequence = (
            entry.payment.unsigned.from.clone(),
            entry.payment.unsigned.sequence,
        );
        if !sender_sequences.insert(sender_sequence.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate mempool sender sequence `{}`:{}",
                    sender_sequence.0, sender_sequence.1
                ),
            ));
        }
        let receipt = execute_payment_v2(genesis, &mut dry_run_ledger, &entry.payment);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool payment_v2 entry `{}` is not executable: {}: {}",
                    entry.tx_id, receipt.code, receipt.message
                ),
            ));
        }
        senders.insert(entry.payment.unsigned.from.clone());
        total_amount = total_amount
            .checked_add(entry.payment.unsigned.amount)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool amount total overflow")
            })?;
        total_fee = total_fee
            .checked_add(entry.payment.unsigned.fee)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool fee total overflow")
            })?;
    }
    for (index, entry) in mempool.pending_asset_transactions.iter().enumerate() {
        if entry.tx_id.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool asset transaction entry {index} has empty tx id"),
            ));
        }
        let expected_tx_id = asset_transaction_tx_id(&entry.transaction);
        if entry.tx_id != expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool asset transaction entry `{}` tx id mismatch",
                    entry.tx_id
                ),
            ));
        }
        if !tx_ids.insert(entry.tx_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate mempool tx id `{}`", entry.tx_id),
            ));
        }
        let sender_sequence = (
            entry.transaction.unsigned.source.clone(),
            entry.transaction.unsigned.sequence,
        );
        if !sender_sequences.insert(sender_sequence.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate mempool sender sequence `{}`:{}",
                    sender_sequence.0, sender_sequence.1
                ),
            ));
        }
        let receipt = execute_asset_transaction_with_compatibility(
            genesis,
            &mut dry_run_ledger,
            &entry.transaction,
            block_height,
            asset_execution_compatibility,
        );
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool asset transaction entry `{}` is not executable: {}: {}",
                    entry.tx_id, receipt.code, receipt.message
                ),
            ));
        }
        senders.insert(entry.transaction.unsigned.source.clone());
        total_amount = total_amount
            .checked_add(asset_transaction_amount(&entry.transaction))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool amount total overflow")
            })?;
        total_fee = total_fee
            .checked_add(entry.transaction.unsigned.fee)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool fee total overflow")
            })?;
    }
    for (index, entry) in mempool.pending_atomic_swaps.iter().enumerate() {
        if entry.tx_id.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool atomic swap entry {index} has empty tx id"),
            ));
        }
        entry
            .transaction
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        let expected_tx_id = atomic_swap_transaction_tx_id(&entry.transaction);
        if entry.tx_id != expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool atomic swap entry `{}` tx id mismatch", entry.tx_id),
            ));
        }
        if !tx_ids.insert(entry.tx_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate mempool tx id `{}`", entry.tx_id),
            ));
        }
        for leg in [
            &entry.transaction.unsigned.leg_0,
            &entry.transaction.unsigned.leg_1,
        ] {
            let sender_sequence = (leg.owner.clone(), leg.sequence);
            if !sender_sequences.insert(sender_sequence.clone()) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "duplicate mempool sender sequence `{}`:{}",
                        sender_sequence.0, sender_sequence.1
                    ),
                ));
            }
            senders.insert(leg.owner.clone());
            total_amount = total_amount.checked_add(leg.amount).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool amount total overflow")
            })?;
            total_fee = total_fee.checked_add(leg.fee).ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool fee total overflow")
            })?;
        }
        let receipt = execute_atomic_swap_transaction_with_compatibility(
            genesis,
            &mut dry_run_ledger,
            &entry.transaction,
            block_height,
            asset_execution_compatibility,
        );
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool atomic swap entry `{}` is not executable: {}: {}",
                    entry.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    for (index, entry) in mempool.pending_fastlane_primary.iter().enumerate() {
        if entry.tx_id.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool FastLane primary entry {index} has empty tx id"),
            ));
        }
        entry.transaction.canonical_bytes().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("FastLane primary encoding failed: {error:?}"),
            )
        })?;
        let expected_tx_id = bytes_to_hex(
            &entry
                .transaction
                .tx_id()
                .map_err(|error| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("FastLane tx id failed: {error:?}"),
                    )
                })?
                .0,
        );
        if entry.tx_id != expected_tx_id || !tx_ids.insert(entry.tx_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool FastLane primary entry `{}` id mismatch or duplicate",
                    entry.tx_id
                ),
            ));
        }
        match &entry.transaction.operation {
            postfiat_types::FastLanePrimaryOperationV1::Deposit { signed } => {
                let sender_sequence = (
                    signed.deposit.source_address.clone(),
                    signed.deposit.sequence,
                );
                if !sender_sequences.insert(sender_sequence.clone()) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "duplicate mempool sender sequence `{}`:{}",
                            sender_sequence.0, sender_sequence.1
                        ),
                    ));
                }
                senders.insert(signed.deposit.source_address.clone());
                total_amount = total_amount
                    .checked_add(signed.deposit.amount_atoms)
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "mempool amount total overflow")
                    })?;
                total_fee = total_fee
                    .checked_add(signed.deposit.fee_pft)
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "mempool fee total overflow")
                    })?;
            }
            postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit { signed } => {
                let sender_sequence = (
                    signed.deposit.source_address.clone(),
                    signed.deposit.sequence,
                );
                if !sender_sequences.insert(sender_sequence.clone()) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!(
                            "duplicate mempool sender sequence `{}`:{}",
                            sender_sequence.0, sender_sequence.1
                        ),
                    ));
                }
                senders.insert(signed.deposit.source_address.clone());
                total_amount = total_amount
                    .checked_add(signed.deposit.amount_atoms)
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "mempool amount total overflow")
                    })?;
                total_fee = total_fee
                    .checked_add(signed.deposit.fee_pft)
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "mempool fee total overflow")
                    })?;
            }
            postfiat_types::FastLanePrimaryOperationV1::Redeem { signed } => {
                total_amount = total_amount
                    .checked_add(signed.claim.amount_atoms)
                    .ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "mempool amount total overflow")
                    })?;
            }
            postfiat_types::FastLanePrimaryOperationV1::AnchorCheckpoint { .. } => {}
            postfiat_types::FastLanePrimaryOperationV1::Control { .. } => {}
            postfiat_types::FastLanePrimaryOperationV1::FastPayRecoveryReveal { .. }
            | postfiat_types::FastLanePrimaryOperationV1::FastPayRecoveryDecision { .. } => {}
        }
        let receipt = execute_fastlane_primary_for_chain(
            genesis,
            &mut dry_run_ledger,
            &entry.transaction,
            block_height,
        );
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool FastLane primary entry `{}` is not executable: {}: {}",
                    entry.tx_id, receipt.code, receipt.message
                ),
            ));
        }
    }
    for (index, entry) in mempool.pending_escrow_transactions.iter().enumerate() {
        if entry.tx_id.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool escrow transaction entry {index} has empty tx id"),
            ));
        }
        let expected_tx_id = escrow_transaction_tx_id(&entry.transaction);
        if entry.tx_id != expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool escrow transaction entry `{}` tx id mismatch",
                    entry.tx_id
                ),
            ));
        }
        if !tx_ids.insert(entry.tx_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate mempool tx id `{}`", entry.tx_id),
            ));
        }
        let sender_sequence = (
            entry.transaction.unsigned.source.clone(),
            entry.transaction.unsigned.sequence,
        );
        if !sender_sequences.insert(sender_sequence.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate mempool sender sequence `{}`:{}",
                    sender_sequence.0, sender_sequence.1
                ),
            ));
        }
        let receipt = execute_escrow_transaction(
            genesis,
            &mut dry_run_ledger,
            &entry.transaction,
            block_height,
        );
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool escrow transaction entry `{}` is not executable: {}: {}",
                    entry.tx_id, receipt.code, receipt.message
                ),
            ));
        }
        senders.insert(entry.transaction.unsigned.source.clone());
        total_amount = total_amount
            .checked_add(escrow_transaction_amount(&entry.transaction))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool amount total overflow")
            })?;
        total_fee = total_fee
            .checked_add(entry.transaction.unsigned.fee)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool fee total overflow")
            })?;
    }
    for (index, entry) in mempool.pending_nft_transactions.iter().enumerate() {
        if entry.tx_id.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool nft transaction entry {index} has empty tx id"),
            ));
        }
        let expected_tx_id = nft_transaction_tx_id(&entry.transaction);
        if entry.tx_id != expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool nft transaction entry `{}` tx id mismatch",
                    entry.tx_id
                ),
            ));
        }
        if !tx_ids.insert(entry.tx_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate mempool tx id `{}`", entry.tx_id),
            ));
        }
        let sender_sequence = (
            entry.transaction.unsigned.source.clone(),
            entry.transaction.unsigned.sequence,
        );
        if !sender_sequences.insert(sender_sequence.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate mempool sender sequence `{}`:{}",
                    sender_sequence.0, sender_sequence.1
                ),
            ));
        }
        let receipt = execute_nft_transaction(genesis, &mut dry_run_ledger, &entry.transaction);
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool nft transaction entry `{}` is not executable: {}: {}",
                    entry.tx_id, receipt.code, receipt.message
                ),
            ));
        }
        senders.insert(entry.transaction.unsigned.source.clone());
        total_fee = total_fee
            .checked_add(entry.transaction.unsigned.fee)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool fee total overflow")
            })?;
    }
    for (index, entry) in mempool.pending_offer_transactions.iter().enumerate() {
        if entry.tx_id.trim().is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("mempool offer transaction entry {index} has empty tx id"),
            ));
        }
        let expected_tx_id = offer_transaction_tx_id(&entry.transaction);
        if entry.tx_id != expected_tx_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool offer transaction entry `{}` tx id mismatch",
                    entry.tx_id
                ),
            ));
        }
        if !tx_ids.insert(entry.tx_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate mempool tx id `{}`", entry.tx_id),
            ));
        }
        let sender_sequence = (
            entry.transaction.unsigned.source.clone(),
            entry.transaction.unsigned.sequence,
        );
        if !sender_sequences.insert(sender_sequence.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "duplicate mempool sender sequence `{}`:{}",
                    sender_sequence.0, sender_sequence.1
                ),
            ));
        }
        let receipt = execute_offer_transaction(
            genesis,
            &mut dry_run_ledger,
            &entry.transaction,
            block_height,
        );
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool offer transaction entry `{}` is not executable: {}: {}",
                    entry.tx_id, receipt.code, receipt.message
                ),
            ));
        }
        senders.insert(entry.transaction.unsigned.source.clone());
        total_amount = total_amount
            .checked_add(offer_transaction_amount(&entry.transaction))
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool amount total overflow")
            })?;
        total_fee = total_fee
            .checked_add(entry.transaction.unsigned.fee)
            .ok_or_else(|| {
                io::Error::new(io::ErrorKind::InvalidData, "mempool fee total overflow")
            })?;
    }

    verify_global_issued_asset_supply_caps(&dry_run_ledger, shielded)?;

    Ok(MempoolVerificationReport {
        verified: true,
        pending_count: mempool.len(),
        sender_count: senders.len(),
        total_amount,
        total_fee,
        latest_tx_id: mempool_latest_tx_id(mempool),
    })
}

pub(super) fn enforce_mempool_admission_limits(
    mempool: &MempoolState,
    sender: &str,
) -> io::Result<()> {
    if mempool.len() >= MAX_MEMPOOL_PENDING_TRANSACTIONS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mempool pending limit reached: {}",
                MAX_MEMPOOL_PENDING_TRANSACTIONS
            ),
        ));
    }
    if mempool.pending_atomic_swaps.iter().any(|entry| {
        entry.transaction.unsigned.leg_0.owner == sender
            || entry.transaction.unsigned.leg_1.owner == sender
    }) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("sender `{sender}` already participates in a pending atomic swap"),
        ));
    }
    let sender_pending = mempool
        .pending
        .iter()
        .filter(|entry| entry.transfer.unsigned.from == sender)
        .count()
        + mempool
            .pending_payment_v2
            .iter()
            .filter(|entry| entry.payment.unsigned.from == sender)
            .count()
        + mempool
            .pending_asset_transactions
            .iter()
            .filter(|entry| entry.transaction.unsigned.source == sender)
            .count()
        + mempool
            .pending_atomic_swaps
            .iter()
            .filter(|entry| {
                entry.transaction.unsigned.leg_0.owner == sender
                    || entry.transaction.unsigned.leg_1.owner == sender
            })
            .count()
        + mempool
            .pending_fastlane_primary
            .iter()
            .filter(|entry| match &entry.transaction.operation {
                postfiat_types::FastLanePrimaryOperationV1::Deposit { signed } => {
                    signed.deposit.source_address == sender
                }
                postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit { signed } => {
                    signed.deposit.source_address == sender
                }
                _ => false,
            })
            .count()
        + mempool
            .pending_escrow_transactions
            .iter()
            .filter(|entry| entry.transaction.unsigned.source == sender)
            .count()
        + mempool
            .pending_nft_transactions
            .iter()
            .filter(|entry| entry.transaction.unsigned.source == sender)
            .count();
    if sender_pending >= MAX_MEMPOOL_PENDING_PER_SENDER {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "mempool sender pending limit reached for `{sender}`: {}",
                MAX_MEMPOOL_PENDING_PER_SENDER
            ),
        ));
    }
    Ok(())
}

pub(super) fn enforce_mempool_state_limits(mempool: &MempoolState) -> io::Result<()> {
    if mempool.len() > MAX_MEMPOOL_PENDING_TRANSACTIONS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "mempool pending count {} exceeds limit {}",
                mempool.len(),
                MAX_MEMPOOL_PENDING_TRANSACTIONS
            ),
        ));
    }
    let mut by_sender = HashMap::<String, usize>::new();
    for entry in &mempool.pending {
        let count = by_sender
            .entry(entry.transfer.unsigned.from.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        if *count > MAX_MEMPOOL_PENDING_PER_SENDER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool sender `{}` pending count {} exceeds limit {}",
                    entry.transfer.unsigned.from, *count, MAX_MEMPOOL_PENDING_PER_SENDER
                ),
            ));
        }
    }
    for entry in &mempool.pending_payment_v2 {
        let count = by_sender
            .entry(entry.payment.unsigned.from.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        if *count > MAX_MEMPOOL_PENDING_PER_SENDER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool sender `{}` pending count {} exceeds limit {}",
                    entry.payment.unsigned.from, *count, MAX_MEMPOOL_PENDING_PER_SENDER
                ),
            ));
        }
    }
    for entry in &mempool.pending_asset_transactions {
        let count = by_sender
            .entry(entry.transaction.unsigned.source.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        if *count > MAX_MEMPOOL_PENDING_PER_SENDER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool sender `{}` pending count {} exceeds limit {}",
                    entry.transaction.unsigned.source, *count, MAX_MEMPOOL_PENDING_PER_SENDER
                ),
            ));
        }
    }
    for entry in &mempool.pending_atomic_swaps {
        for owner in [
            &entry.transaction.unsigned.leg_0.owner,
            &entry.transaction.unsigned.leg_1.owner,
        ] {
            let count = by_sender
                .entry(owner.clone())
                .and_modify(|count| *count += 1)
                .or_insert(1);
            if *count > MAX_MEMPOOL_PENDING_PER_SENDER {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "mempool sender `{owner}` pending count {} exceeds limit {}",
                        *count, MAX_MEMPOOL_PENDING_PER_SENDER
                    ),
                ));
            }
        }
    }
    for entry in &mempool.pending_fastlane_primary {
        let source_address = match &entry.transaction.operation {
            postfiat_types::FastLanePrimaryOperationV1::Deposit { signed } => {
                Some(&signed.deposit.source_address)
            }
            postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit { signed } => {
                Some(&signed.deposit.source_address)
            }
            _ => None,
        };
        if let Some(source_address) = source_address {
            let count = by_sender
                .entry(source_address.clone())
                .and_modify(|count| *count += 1)
                .or_insert(1);
            if *count > MAX_MEMPOOL_PENDING_PER_SENDER {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "mempool sender `{}` pending count {} exceeds limit {}",
                        source_address, *count, MAX_MEMPOOL_PENDING_PER_SENDER
                    ),
                ));
            }
        }
    }
    for entry in &mempool.pending_escrow_transactions {
        let count = by_sender
            .entry(entry.transaction.unsigned.source.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        if *count > MAX_MEMPOOL_PENDING_PER_SENDER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool sender `{}` pending count {} exceeds limit {}",
                    entry.transaction.unsigned.source, *count, MAX_MEMPOOL_PENDING_PER_SENDER
                ),
            ));
        }
    }
    for entry in &mempool.pending_nft_transactions {
        let count = by_sender
            .entry(entry.transaction.unsigned.source.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        if *count > MAX_MEMPOOL_PENDING_PER_SENDER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool sender `{}` pending count {} exceeds limit {}",
                    entry.transaction.unsigned.source, *count, MAX_MEMPOOL_PENDING_PER_SENDER
                ),
            ));
        }
    }
    for entry in &mempool.pending_offer_transactions {
        let count = by_sender
            .entry(entry.transaction.unsigned.source.clone())
            .and_modify(|count| *count += 1)
            .or_insert(1);
        if *count > MAX_MEMPOOL_PENDING_PER_SENDER {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "mempool sender `{}` pending count {} exceeds limit {}",
                    entry.transaction.unsigned.source, *count, MAX_MEMPOOL_PENDING_PER_SENDER
                ),
            ));
        }
    }
    Ok(())
}

fn mempool_latest_tx_id(mempool: &MempoolState) -> String {
    mempool
        .pending_offer_transactions
        .last()
        .map(|entry| entry.tx_id.clone())
        .or_else(|| {
            mempool
                .pending_nft_transactions
                .last()
                .map(|entry| entry.tx_id.clone())
        })
        .or_else(|| {
            mempool
                .pending_escrow_transactions
                .last()
                .map(|entry| entry.tx_id.clone())
        })
        .or_else(|| {
            mempool
                .pending_atomic_swaps
                .last()
                .map(|entry| entry.tx_id.clone())
        })
        .or_else(|| {
            mempool
                .pending_fastlane_primary
                .last()
                .map(|entry| entry.tx_id.clone())
        })
        .or_else(|| {
            mempool
                .pending_asset_transactions
                .last()
                .map(|entry| entry.tx_id.clone())
        })
        .or_else(|| {
            mempool
                .pending_payment_v2
                .last()
                .map(|entry| entry.tx_id.clone())
        })
        .or_else(|| mempool.pending.last().map(|entry| entry.tx_id.clone()))
        .unwrap_or_default()
}

fn asset_transaction_amount(transaction: &SignedAssetTransaction) -> u64 {
    match &transaction.unsigned.operation {
        AssetTransactionOperation::IssuedPayment(operation) => operation.amount,
        AssetTransactionOperation::AssetBurn(operation) => operation.amount,
        AssetTransactionOperation::AssetClawback(operation) => operation.amount,
        _ => 0,
    }
}

fn escrow_transaction_amount(transaction: &SignedEscrowTransaction) -> u64 {
    match &transaction.unsigned.operation {
        EscrowTransactionOperation::EscrowCreate(operation) => operation.amount,
        _ => 0,
    }
}

fn offer_transaction_amount(transaction: &SignedOfferTransaction) -> u64 {
    match &transaction.unsigned.operation {
        OfferTransactionOperation::OfferCreate(operation) => operation.taker_gets_amount,
        _ => 0,
    }
}

pub fn create_mempool_batch(options: MempoolBatchOptions) -> io::Result<TransactionBatch> {
    if options.max_transactions == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--max-transactions must be greater than zero",
        ));
    }
    if options.max_transactions > MAX_BATCH_TRANSACTIONS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "--max-transactions must not exceed {}",
                MAX_BATCH_TRANSACTIONS
            ),
        ));
    }

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mut mempool = store.read_mempool()?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    enforce_mempool_state_limits(&mempool)?;
    if mempool.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "mempool has no pending transactions",
        ));
    }
    let mut dry_run_ledger = ledger;
    let mut transactions = Vec::new();
    let mut payments_v2 = Vec::new();
    let mut asset_transactions = Vec::new();
    let mut atomic_swap_transactions = Vec::new();
    let mut fastlane_primary_transactions = Vec::new();
    let mut escrow_transactions = Vec::new();
    let mut nft_transactions = Vec::new();
    let mut offer_transactions = Vec::new();
    let mut remaining = Vec::new();
    let mut remaining_payments_v2 = Vec::new();
    let mut remaining_asset_transactions = Vec::new();
    let mut remaining_atomic_swaps = Vec::new();
    let mut remaining_fastlane_primary = Vec::new();
    let mut remaining_escrow_transactions = Vec::new();
    let mut remaining_nft_transactions = Vec::new();
    let mut remaining_offer_transactions = Vec::new();
    let pending = std::mem::take(&mut mempool.pending);
    for entry in pending {
        if transactions.len() >= options.max_transactions {
            remaining.push(entry);
            continue;
        }
        let receipt = execute_transfer(&genesis, &mut dry_run_ledger, &entry.transfer);
        if receipt.accepted {
            transactions.push(entry.transfer);
        }
    }
    let pending_payment_v2 = std::mem::take(&mut mempool.pending_payment_v2);
    for entry in pending_payment_v2 {
        if transactions.len().saturating_add(payments_v2.len()) >= options.max_transactions {
            remaining_payments_v2.push(entry);
            continue;
        }
        let receipt = execute_payment_v2(&genesis, &mut dry_run_ledger, &entry.payment);
        if receipt.accepted {
            payments_v2.push(entry.payment);
        }
    }
    let pending_asset_transactions = std::mem::take(&mut mempool.pending_asset_transactions);
    for entry in pending_asset_transactions {
        if transactions
            .len()
            .saturating_add(payments_v2.len())
            .saturating_add(asset_transactions.len())
            >= options.max_transactions
        {
            remaining_asset_transactions.push(entry);
            continue;
        }
        let receipt = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut dry_run_ledger,
            &entry.transaction,
            block_height,
            asset_execution_compatibility,
        );
        if receipt.accepted {
            asset_transactions.push(entry.transaction);
        }
    }
    let pending_atomic_swaps = std::mem::take(&mut mempool.pending_atomic_swaps);
    for entry in pending_atomic_swaps {
        if !asset_execution_compatibility.atomic_swap_active(block_height) {
            continue;
        }
        if transactions
            .len()
            .saturating_add(payments_v2.len())
            .saturating_add(asset_transactions.len())
            .saturating_add(atomic_swap_transactions.len())
            >= options.max_transactions
        {
            remaining_atomic_swaps.push(entry);
            continue;
        }
        let receipt = execute_atomic_swap_transaction_with_compatibility(
            &genesis,
            &mut dry_run_ledger,
            &entry.transaction,
            block_height,
            asset_execution_compatibility,
        );
        if receipt.accepted {
            atomic_swap_transactions.push(entry.transaction);
        }
    }
    let pending_fastlane_primary = std::mem::take(&mut mempool.pending_fastlane_primary);
    for entry in pending_fastlane_primary {
        if transactions
            .len()
            .saturating_add(payments_v2.len())
            .saturating_add(asset_transactions.len())
            .saturating_add(atomic_swap_transactions.len())
            .saturating_add(fastlane_primary_transactions.len())
            >= options.max_transactions
        {
            remaining_fastlane_primary.push(entry);
            continue;
        }
        let receipt = execute_fastlane_primary_for_chain(
            &genesis,
            &mut dry_run_ledger,
            &entry.transaction,
            block_height,
        );
        if receipt.accepted {
            fastlane_primary_transactions.push(entry.transaction);
        }
    }
    let pending_escrow_transactions = std::mem::take(&mut mempool.pending_escrow_transactions);
    for entry in pending_escrow_transactions {
        if transactions
            .len()
            .saturating_add(payments_v2.len())
            .saturating_add(asset_transactions.len())
            .saturating_add(atomic_swap_transactions.len())
            .saturating_add(fastlane_primary_transactions.len())
            .saturating_add(escrow_transactions.len())
            >= options.max_transactions
        {
            remaining_escrow_transactions.push(entry);
            continue;
        }
        let receipt = execute_escrow_transaction(
            &genesis,
            &mut dry_run_ledger,
            &entry.transaction,
            block_height,
        );
        if receipt.accepted {
            escrow_transactions.push(entry.transaction);
        }
    }
    let pending_nft_transactions = std::mem::take(&mut mempool.pending_nft_transactions);
    for entry in pending_nft_transactions {
        if transactions
            .len()
            .saturating_add(payments_v2.len())
            .saturating_add(asset_transactions.len())
            .saturating_add(atomic_swap_transactions.len())
            .saturating_add(fastlane_primary_transactions.len())
            .saturating_add(escrow_transactions.len())
            .saturating_add(nft_transactions.len())
            >= options.max_transactions
        {
            remaining_nft_transactions.push(entry);
            continue;
        }
        let receipt = execute_nft_transaction(&genesis, &mut dry_run_ledger, &entry.transaction);
        if receipt.accepted {
            nft_transactions.push(entry.transaction);
        }
    }
    let pending_offer_transactions = std::mem::take(&mut mempool.pending_offer_transactions);
    for entry in pending_offer_transactions {
        if transactions
            .len()
            .saturating_add(payments_v2.len())
            .saturating_add(asset_transactions.len())
            .saturating_add(atomic_swap_transactions.len())
            .saturating_add(fastlane_primary_transactions.len())
            .saturating_add(escrow_transactions.len())
            .saturating_add(nft_transactions.len())
            .saturating_add(offer_transactions.len())
            >= options.max_transactions
        {
            remaining_offer_transactions.push(entry);
            continue;
        }
        let receipt = execute_offer_transaction(
            &genesis,
            &mut dry_run_ledger,
            &entry.transaction,
            block_height,
        );
        if receipt.accepted {
            offer_transactions.push(entry.transaction);
        }
    }

    if transactions.is_empty()
        && payments_v2.is_empty()
        && asset_transactions.is_empty()
        && atomic_swap_transactions.is_empty()
        && fastlane_primary_transactions.is_empty()
        && escrow_transactions.is_empty()
        && nft_transactions.is_empty()
        && offer_transactions.is_empty()
    {
        mempool.pending = remaining;
        mempool.pending_payment_v2 = remaining_payments_v2;
        mempool.pending_asset_transactions = remaining_asset_transactions;
        mempool.pending_atomic_swaps = remaining_atomic_swaps;
        mempool.pending_fastlane_primary = remaining_fastlane_primary;
        mempool.pending_escrow_transactions = remaining_escrow_transactions;
        mempool.pending_nft_transactions = remaining_nft_transactions;
        mempool.pending_offer_transactions = remaining_offer_transactions;
        store.write_mempool(&mempool)?;
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            "mempool has no valid pending transactions",
        ));
    }

    let batch_domain = mempool_batch_domain(&genesis);
    let base_batch = build_mixed_transaction_batch_with_atomic_swaps(
        &batch_domain,
        transactions,
        payments_v2,
        asset_transactions,
        atomic_swap_transactions,
        escrow_transactions,
        nft_transactions,
        offer_transactions,
    )
    .map_err(invalid_data)?
    .batch;
    let batch = attach_fastlane_primary_transactions(
        &batch_domain,
        base_batch,
        fastlane_primary_transactions,
    )
    .map_err(invalid_data)?
    .batch;
    write_batch_file(&options.batch_file, &batch)?;
    mempool.pending = remaining;
    mempool.pending_payment_v2 = remaining_payments_v2;
    mempool.pending_asset_transactions = remaining_asset_transactions;
    mempool.pending_atomic_swaps = remaining_atomic_swaps;
    mempool.pending_fastlane_primary = remaining_fastlane_primary;
    mempool.pending_escrow_transactions = remaining_escrow_transactions;
    mempool.pending_nft_transactions = remaining_nft_transactions;
    mempool.pending_offer_transactions = remaining_offer_transactions;
    store.write_mempool(&mempool)?;
    Ok(batch)
}

pub fn create_signed_asset_transaction_batch(
    options: SignedAssetTransactionBatchOptions,
) -> io::Result<TransactionBatch> {
    if options.signed_asset_transaction_files.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "at least one signed asset transaction file is required",
        ));
    }
    if options.signed_asset_transaction_files.len() > MAX_BATCH_TRANSACTIONS {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "signed asset transaction count must not exceed {}",
                MAX_BATCH_TRANSACTIONS
            ),
        ));
    }

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    let mut dry_run_ledger = ledger;
    let mut asset_transactions = Vec::new();
    for signed_file in &options.signed_asset_transaction_files {
        let signed: SignedAssetTransaction =
            read_json_file(signed_file, "signed asset transaction")?;
        let receipt = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut dry_run_ledger,
            &signed,
            block_height,
            asset_execution_compatibility,
        );
        if !receipt.accepted {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "signed asset transaction `{}` rejected in batch dry-run at height {}: {} ({})",
                    signed_file.display(),
                    block_height,
                    receipt.code,
                    receipt.message
                ),
            ));
        }
        asset_transactions.push(signed);
    }

    let batch_domain = mempool_batch_domain(&genesis);
    let batch = build_mixed_transaction_batch_with_offers(
        &batch_domain,
        Vec::new(),
        Vec::new(),
        asset_transactions,
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .map_err(invalid_data)?
    .batch;
    write_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn propose_batch(options: BatchProposalOptions) -> io::Result<BlockProposalFile> {
    Ok(propose_batch_with_timings(options)?.proposal)
}

pub fn propose_batch_with_timings(
    options: BatchProposalOptions,
) -> io::Result<BatchProposalWithTimingsReport> {
    propose_batch_with_optional_required_parent(options, None)
}

pub fn propose_batch_with_required_parent_with_timings(
    options: BatchProposalOptions,
    required_parent: &RequiredBlockParent,
) -> io::Result<BatchProposalWithTimingsReport> {
    propose_batch_with_optional_required_parent(options, Some(required_parent))
}

fn propose_batch_with_optional_required_parent(
    options: BatchProposalOptions,
    required_parent: Option<&RequiredBlockParent>,
) -> io::Result<BatchProposalWithTimingsReport> {
    let total_start = std::time::Instant::now();
    let mut timings = BatchProposalTimingReport {
        schema: "postfiat.batch_proposal_timing.v1".to_string(),
        ..BatchProposalTimingReport::default()
    };
    if options.verify_block_log {
        let stage_start = std::time::Instant::now();
        verify_blocks(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?;
        timings.verify_block_log_ms = node_timing_elapsed_ms(stage_start);
    }
    let stage_start = std::time::Instant::now();
    let store = NodeStore::new(&options.data_dir);
    timings.store_init_ms = node_timing_elapsed_ms(stage_start);

    if let Some(required_parent) = required_parent {
        let observed = status(NodeOptions {
            data_dir: options.data_dir.clone(),
        })?;
        if observed.block_height != required_parent.height
            || observed.block_tip_hash != required_parent.block_hash
            || observed.state_root != required_parent.state_root
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "required proposal parent mismatch: expected height {} hash {} root {}, observed height {} hash {} root {}",
                    required_parent.height,
                    required_parent.block_hash,
                    required_parent.state_root,
                    observed.block_height,
                    observed.block_tip_hash,
                    observed.state_root,
                ),
            ));
        }
    }

    let stage_start = std::time::Instant::now();
    let batch_kind = normalize_block_proposal_batch_kind(options.batch_kind.as_deref())?;
    timings.batch_kind_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let (mut proposal, shielded_breakdown) = build_ordered_batch_proposal_with_timings(
        &store,
        batch_kind,
        &options.batch_file,
        options.view,
        None,
    )?;
    timings.ordered_proposal_ms = node_timing_elapsed_ms(stage_start);
    if let Some(required_parent) = required_parent {
        let expected_child_height = required_parent.height.checked_add(1).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "required parent height overflow",
            )
        })?;
        if proposal.block_height != expected_child_height
            || proposal.parent_hash != required_parent.block_hash
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "proposal escaped required parent: expected child height {} parent {}, built height {} parent {}",
                    expected_child_height,
                    required_parent.block_hash,
                    proposal.block_height,
                    proposal.parent_hash,
                ),
            ));
        }
    }
    if let Some(shielded_breakdown) = shielded_breakdown {
        timings.verifier_setup_ms = shielded_breakdown
            .private_egress_verifier_breakdown
            .vk_cached_calls
            .iter()
            .map(|timing| timing.total_ms)
            .sum();
        timings.state_exec_ms = shielded_breakdown.state_exec_ms;
        timings.shielded_breakdown = Some(shielded_breakdown);
    }

    let stage_start = std::time::Instant::now();
    validate_block_proposal_timeout_evidence(
        &store,
        &proposal,
        options.timeout_certificate_file.as_deref(),
        options.verify_block_log,
    )?;
    timings.timeout_evidence_ms = node_timing_elapsed_ms(stage_start);

    if let Some(key_file) = options.key_file.as_ref() {
        let stage_start = std::time::Instant::now();
        let signer_id = options
            .validator_id
            .clone()
            .unwrap_or_else(|| proposal.proposer.clone());
        sign_block_proposal_file(&store, &mut proposal, key_file, Some(signer_id.as_str()))?;
        timings.signing_ms = node_timing_elapsed_ms(stage_start);
    } else if options.validator_id.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--validator requires --key-file for signed proposals",
        ));
    }

    let stage_start = std::time::Instant::now();
    verify_block_proposal_signature_if_present(&store, &proposal)?;
    timings.signature_verify_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    write_block_proposal_file(&options.proposal_file, &proposal)?;
    timings.serialization_ms = node_timing_elapsed_ms(stage_start);
    timings.total_ms = node_timing_elapsed_ms(total_start);
    Ok(BatchProposalWithTimingsReport { proposal, timings })
}

pub(super) fn normalize_block_proposal_batch_kind(
    batch_kind: Option<&str>,
) -> io::Result<&'static str> {
    match batch_kind.unwrap_or(BATCH_KIND_TRANSPARENT) {
        BATCH_KIND_TRANSPARENT => Ok(BATCH_KIND_TRANSPARENT),
        BATCH_KIND_GOVERNANCE => Ok(BATCH_KIND_GOVERNANCE),
        BATCH_KIND_SHIELDED => Ok(BATCH_KIND_SHIELDED),
        BATCH_KIND_BRIDGE => Ok(BATCH_KIND_BRIDGE),
        other => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported block proposal batch kind `{other}`"),
        )),
    }
}

#[allow(dead_code)]
fn build_ordered_batch_proposal(
    store: &NodeStore,
    batch_kind: &str,
    batch_file: &Path,
    view: Option<u64>,
) -> io::Result<BlockProposalFile> {
    Ok(build_ordered_batch_proposal_with_timings(store, batch_kind, batch_file, view, None)?.0)
}

pub(super) fn build_ordered_batch_proposal_with_timings(
    store: &NodeStore,
    batch_kind: &str,
    batch_file: &Path,
    view: Option<u64>,
    supplied_fastpay_effects: Option<&[postfiat_types::FastPayVersionFenceV1]>,
) -> io::Result<(BlockProposalFile, Option<ShieldedBatchProposalTimingReport>)> {
    match normalize_block_proposal_batch_kind(Some(batch_kind))? {
        BATCH_KIND_TRANSPARENT => {
            build_transparent_batch_proposal(store, batch_file, view, supplied_fastpay_effects)
                .map(|proposal| (proposal, None))
        }
        BATCH_KIND_GOVERNANCE => {
            build_governance_batch_proposal(store, batch_file, view, supplied_fastpay_effects)
                .map(|proposal| (proposal, None))
        }
        BATCH_KIND_SHIELDED => build_shielded_batch_proposal_with_timings(
            store,
            batch_file,
            view,
            supplied_fastpay_effects,
        )
        .map(|(proposal, timings)| (proposal, Some(timings))),
        BATCH_KIND_BRIDGE => {
            build_bridge_batch_proposal(store, batch_file, view, supplied_fastpay_effects)
                .map(|proposal| (proposal, None))
        }
        _ => unreachable!("normalize_block_proposal_batch_kind only returns supported kinds"),
    }
}

fn build_transparent_batch_proposal(
    store: &NodeStore,
    batch_file: &Path,
    view: Option<u64>,
    supplied_fastpay_effects: Option<&[postfiat_types::FastPayVersionFenceV1]>,
) -> io::Result<BlockProposalFile> {
    let genesis = store.read_genesis()?;
    let mut ledger = store.read_ledger()?;
    let mut governance = store.read_governance()?;
    let batch = read_batch_file(batch_file)?;
    let batch_domain = mempool_batch_domain(&genesis);
    let reference = reference_for_batch(&batch_domain, &batch).map_err(invalid_data)?;
    verify_batch_payload(&batch_domain, &batch, &reference).map_err(invalid_data)?;
    let ordered_reference = next_reference(vec![reference]).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "ordering produced no batch reference",
        )
    })?;

    let ordered_batches = store.read_ordered_batches()?;
    if ordered_batches.contains(&ordered_reference.batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("batch `{}` already applied", ordered_reference.batch_id),
        ));
    }
    let shielded = store.read_shielded()?;
    let bridge = store.read_bridge()?;
    let fastpay_pre_state_effects = match supplied_fastpay_effects {
        Some(effects) => {
            reconcile_fastpay_pre_state_effects(store, &mut ledger, &shielded, effects)?
        }
        None => fastpay_pre_state_effects_for_next_block(store, &ledger)?,
    };
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(store, &genesis)?;
    let block_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let parent_hash = chain_tip.block_hash.clone();
    let _ = activate_due_validator_registry_updates_for_commit(
        store,
        &genesis,
        &mut governance,
        block_height,
    )?;

    let compatibility =
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);
    ensure_atomic_swap_batch_allowed(&batch, block_height, compatibility)?;
    let receipts = execute_transparent_batch(
        &genesis,
        &governance,
        &mut ledger,
        &batch,
        block_height,
        compatibility,
    );
    let batch_id = ordered_reference.batch_id;
    let mut proposed_ordered_batches = ordered_batches;
    proposed_ordered_batches.push(batch_id.clone());
    build_block_proposal_from_state(BlockProposalPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &proposed_ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash,
        view: view.unwrap_or(0),
        batch_kind: BATCH_KIND_TRANSPARENT,
        batch_id: &batch_id,
        payload: &batch,
        receipts: &receipts,
        fastpay_pre_state_effects,
    })
}

fn build_governance_batch_proposal(
    store: &NodeStore,
    batch_file: &Path,
    view: Option<u64>,
    supplied_fastpay_effects: Option<&[postfiat_types::FastPayVersionFenceV1]>,
) -> io::Result<BlockProposalFile> {
    let genesis = store.read_genesis()?;
    let mut ledger = store.read_ledger()?;
    let mut governance = store.read_governance()?;
    let batch = read_governance_action_batch_file(batch_file)?;
    verify_governance_action_batch_id(&genesis, &batch)?;

    let ordered_batches = store.read_ordered_batches()?;
    if ordered_batches.contains(&batch.batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("governance batch `{}` already applied", batch.batch_id),
        ));
    }
    let shielded = store.read_shielded()?;
    let bridge = store.read_bridge()?;
    let fastpay_pre_state_effects = match supplied_fastpay_effects {
        Some(effects) => {
            reconcile_fastpay_pre_state_effects(store, &mut ledger, &shielded, effects)?
        }
        None => fastpay_pre_state_effects_for_next_block(store, &ledger)?,
    };
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(store, &genesis)?;
    let block_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let parent_hash = chain_tip.block_hash.clone();
    let due_activations = activate_due_validator_registry_updates_for_commit(
        store,
        &genesis,
        &mut governance,
        block_height,
    )?;
    let registry = due_activations
        .registry
        .unwrap_or(read_validator_registry_file(
            &store.data_dir().join(VALIDATOR_REGISTRY_FILE),
        )?);
    verify_live_signed_governance_batch(&genesis, &governance, &registry, &batch, block_height)?;

    ensure_governance_batch_lifecycle_ready(&batch, block_height)?;
    let receipts =
        execute_governance_batch(&mut governance, Some(&mut ledger), &batch, block_height);
    let mut proposed_ordered_batches = ordered_batches;
    proposed_ordered_batches.push(batch.batch_id.clone());
    build_block_proposal_from_state(BlockProposalPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &proposed_ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash,
        view: view.unwrap_or(0),
        batch_kind: BATCH_KIND_GOVERNANCE,
        batch_id: &batch.batch_id,
        payload: &batch,
        receipts: &receipts,
        fastpay_pre_state_effects,
    })
}

#[allow(dead_code)]
fn build_shielded_batch_proposal(
    store: &NodeStore,
    batch_file: &Path,
    view: Option<u64>,
) -> io::Result<BlockProposalFile> {
    Ok(build_shielded_batch_proposal_with_timings(store, batch_file, view, None)?.0)
}

fn build_shielded_batch_proposal_with_timings(
    store: &NodeStore,
    batch_file: &Path,
    view: Option<u64>,
    supplied_fastpay_effects: Option<&[postfiat_types::FastPayVersionFenceV1]>,
) -> io::Result<(BlockProposalFile, ShieldedBatchProposalTimingReport)> {
    let total_start = std::time::Instant::now();
    let mut timings = ShieldedBatchProposalTimingReport {
        schema: "postfiat.shielded_batch_proposal_timing.v1".to_string(),
        ..ShieldedBatchProposalTimingReport::default()
    };

    let stage_start = std::time::Instant::now();
    let genesis = store.read_genesis()?;
    timings.read_genesis_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let mut ledger = store.read_ledger()?;
    timings.read_ledger_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let mut governance = store.read_governance()?;
    timings.read_governance_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let mut shielded = store.read_shielded()?;
    timings.read_shielded_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let batch = read_shielded_action_batch_file(batch_file)?;
    timings.read_batch_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    verify_shielded_action_batch_id(&genesis, &batch)?;
    reject_live_legacy_cleartext_shielded_actions(&batch)?;
    timings.verify_batch_id_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let ordered_batches = store.read_ordered_batches()?;
    timings.read_ordered_batches_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    if ordered_batches.contains(&batch.batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("shielded batch `{}` already applied", batch.batch_id),
        ));
    }
    timings.duplicate_check_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let bridge = store.read_bridge()?;
    timings.read_bridge_ms = node_timing_elapsed_ms(stage_start);

    let fastpay_pre_state_effects = match supplied_fastpay_effects {
        Some(effects) => {
            reconcile_fastpay_pre_state_effects(store, &mut ledger, &shielded, effects)?
        }
        None => fastpay_pre_state_effects_for_next_block(store, &ledger)?,
    };

    let stage_start = std::time::Instant::now();
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(store, &genesis)?;
    let block_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let parent_hash = chain_tip.block_hash.clone();
    timings.chain_tip_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let _ = activate_due_validator_registry_updates_for_commit(
        store,
        &genesis,
        &mut governance,
        block_height,
    )?;
    timings.activation_ms = node_timing_elapsed_ms(stage_start);

    reset_asset_orchard_private_egress_timings();
    reset_asset_orchard_private_egress_node_timings();
    let stage_start = std::time::Instant::now();
    let receipts = execute_shielded_batch(
        &genesis,
        &mut ledger,
        &mut shielded,
        &batch,
        block_height,
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance),
        governance.orchard_pool_paused,
        false,
    );
    timings.state_exec_ms = node_timing_elapsed_ms(stage_start);
    let private_egress_verifier_breakdown = take_asset_orchard_private_egress_timings();
    let private_egress_state_breakdown = take_asset_orchard_private_egress_node_timings();
    let verifier_setup_ms = private_egress_verifier_breakdown
        .vk_cached_calls
        .iter()
        .map(|timing| timing.total_ms)
        .sum::<f64>()
        + private_egress_state_breakdown
            .state_applications
            .iter()
            .flat_map(|timing| timing.verifier_breakdown.vk_cached_calls.iter())
            .map(|timing| timing.total_ms)
            .sum::<f64>();
    timings.verifier_setup_ms = verifier_setup_ms;
    timings.private_egress_verifier_breakdown = private_egress_verifier_breakdown;
    timings.private_egress_state_breakdown = private_egress_state_breakdown;

    let mut proposed_ordered_batches = ordered_batches;
    proposed_ordered_batches.push(batch.batch_id.clone());
    let stage_start = std::time::Instant::now();
    let proposal = build_block_proposal_from_state(BlockProposalPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &proposed_ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash,
        view: view.unwrap_or(0),
        batch_kind: BATCH_KIND_SHIELDED,
        batch_id: &batch.batch_id,
        payload: &batch,
        receipts: &receipts,
        fastpay_pre_state_effects,
    })?;
    timings.build_proposal_from_state_ms = node_timing_elapsed_ms(stage_start);
    timings.total_ms = node_timing_elapsed_ms(total_start);
    Ok((proposal, timings))
}

fn build_bridge_batch_proposal(
    store: &NodeStore,
    batch_file: &Path,
    view: Option<u64>,
    supplied_fastpay_effects: Option<&[postfiat_types::FastPayVersionFenceV1]>,
) -> io::Result<BlockProposalFile> {
    let genesis = store.read_genesis()?;
    let mut ledger = store.read_ledger()?;
    let mut governance = store.read_governance()?;
    let shielded = store.read_shielded()?;
    let mut bridge = store.read_bridge()?;
    let fastpay_pre_state_effects = match supplied_fastpay_effects {
        Some(effects) => {
            reconcile_fastpay_pre_state_effects(store, &mut ledger, &shielded, effects)?
        }
        None => fastpay_pre_state_effects_for_next_block(store, &ledger)?,
    };
    let validator_registry =
        read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let batch = read_bridge_action_batch_file(batch_file)?;
    verify_bridge_action_batch_id(&genesis, &batch)?;

    let ordered_batches = store.read_ordered_batches()?;
    if ordered_batches.contains(&batch.batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("bridge batch `{}` already applied", batch.batch_id),
        ));
    }
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(store, &genesis)?;
    let block_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let parent_hash = chain_tip.block_hash.clone();
    let due_activations = activate_due_validator_registry_updates_for_commit(
        store,
        &genesis,
        &mut governance,
        block_height,
    )?;
    let execution_validator_registry = due_activations
        .registry
        .as_ref()
        .unwrap_or(&validator_registry);

    let receipts = execute_bridge_batch(
        &genesis,
        &mut bridge,
        &batch,
        governance.bridge_witness_epoch,
        execution_validator_registry,
    );
    let mut proposed_ordered_batches = ordered_batches;
    proposed_ordered_batches.push(batch.batch_id.clone());
    build_block_proposal_from_state(BlockProposalPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &proposed_ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash,
        view: view.unwrap_or(0),
        batch_kind: BATCH_KIND_BRIDGE,
        batch_id: &batch.batch_id,
        payload: &batch,
        receipts: &receipts,
        fastpay_pre_state_effects,
    })
}
