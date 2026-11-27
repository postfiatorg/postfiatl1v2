use super::*;

/// Order rows that confirmed FastPay anchors lack (stores written before the
/// order index existed). An anchor from a retained block takes that block's
/// height and effect position; a checkpoint anchor takes its anchor height and
/// position 0, as in `verify_logical_integrity`.
pub(super) fn missing_order_entries<I, B>(
    indexes: &I,
    blocks: Option<&B>,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<Vec<FastPayOrderIndexEntryV1>>
where
    I: ReadableTable<&'static [u8], &'static [u8]>,
    B: ReadableTable<&'static [u8], &'static [u8]>,
{
    counters.full_history_scans.fetch_add(1, Ordering::Relaxed);
    let mut anchor_keys = Vec::new();
    for entry in indexes.iter().map_err(database_error)? {
        let (key, _) = entry.map_err(database_error)?;
        if key.value().starts_with(FASTPAY_ANCHOR_KEY_PREFIX) {
            anchor_keys.push(key.value().to_vec());
        }
    }
    let mut missing = BTreeMap::<String, FastPayOrderIndexEntryV1>::new();
    for key in anchor_keys {
        let Some(raw) = read_authenticated(
            indexes,
            HISTORY_INDEXES_TABLE,
            &key,
            MAX_RECORD_BYTES,
            integrity_key,
            counters,
        )?
        else {
            continue;
        };
        let anchor: StoredFastPayAnchorV1 = decode_json(&raw)?;
        let block = match blocks {
            Some(blocks) => read_authenticated(
                blocks,
                BLOCKS_BY_HEIGHT_TABLE,
                &ordered_u64_key(anchor.finalized_height),
                MAX_RECORD_BYTES,
                integrity_key,
                counters,
            )?
            .map(|raw| decode_json::<BlockRecord>(&raw))
            .transpose()?,
            None => None,
        };
        let effect_index = block
            .and_then(|block| {
                block
                    .fastpay_pre_state_effects
                    .iter()
                    .position(|effect| effect.lock_id == anchor.effect.lock_id)
            })
            .unwrap_or(0);
        let Some(entry) = fastpay_order_index_entry_from_effect(
            &anchor.effect,
            anchor.finalized_height,
            effect_index as u32,
        ) else {
            continue;
        };
        let order_key = fastpay_order_index_key(&entry.order_hash)?;
        if indexes
            .get(order_key.as_slice())
            .map_err(database_error)?
            .is_none()
        {
            missing.insert(entry.order_hash.clone(), entry);
        }
    }
    Ok(missing.into_values().collect())
}

/// Populate or verify the acceleration index for confirmed consensusless
/// FastPay effects already authenticated by a retained-history checkpoint.
pub(super) fn write_block_anchors(
    transaction: &redb::WriteTransaction,
    block: &BlockRecord,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    let mut indexes = transaction
        .open_table(HISTORY_INDEXES)
        .map_err(database_error)?;
    let mut lock_ids = BTreeSet::new();
    for (effect_index, effect) in block.fastpay_pre_state_effects.iter().enumerate() {
        effect.validate_shape().map_err(|error| {
            StorageError::new(
                StorageErrorCode::CorruptRecord,
                format!("retained FastPay block effect is invalid: {error}"),
            )
        })?;
        if !lock_ids.insert(effect.lock_id.as_str()) {
            return Err(StorageError::new(
                StorageErrorCode::CorruptRecord,
                "retained FastPay block contains a duplicate lock id",
            ));
        }
        let key = fastpay_anchor_key(&effect.lock_id)?;
        let record = StoredFastPayAnchorV1 {
            schema: STORED_FASTPAY_ANCHOR_SCHEMA.to_owned(),
            finalized_height: block.header.height,
            effect: effect.clone(),
        };
        if let Some(raw) = read_authenticated(
            &indexes,
            HISTORY_INDEXES_TABLE,
            &key,
            MAX_RECORD_BYTES,
            integrity_key,
            counters,
        )? {
            if decode_json::<StoredFastPayAnchorV1>(&raw)? != record {
                return Err(StorageError::new(
                    StorageErrorCode::InitializationConflict,
                    "retained FastPay block anchor conflicts with another canonical effect",
                ));
            }
        } else {
            insert_authenticated(
                &mut indexes,
                HISTORY_INDEXES_TABLE,
                &key,
                &encode_json(&record, MAX_RECORD_BYTES)?,
                MAX_RECORD_BYTES,
                integrity_key,
                counters,
            )?;
        }
        if let Some(order_entry) =
            fastpay_order_index_entry_from_effect(effect, block.header.height, effect_index as u32)
        {
            let order_key = fastpay_order_index_key(&order_entry.order_hash)?;
            if let Some(raw) = read_authenticated(
                &indexes,
                HISTORY_INDEXES_TABLE,
                &order_key,
                MAX_RECORD_BYTES,
                integrity_key,
                counters,
            )? {
                if decode_json::<FastPayOrderIndexEntryV1>(&raw)? != order_entry {
                    return Err(StorageError::new(
                        StorageErrorCode::InitializationConflict,
                        "retained FastPay order index conflicts with another canonical effect",
                    ));
                }
            } else {
                insert_authenticated(
                    &mut indexes,
                    HISTORY_INDEXES_TABLE,
                    &order_key,
                    &encode_json(&order_entry, MAX_RECORD_BYTES)?,
                    MAX_RECORD_BYTES,
                    integrity_key,
                    counters,
                )?;
            }
        }
    }
    Ok(())
}

pub(super) fn write_checkpoint_anchors(
    transaction: &redb::WriteTransaction,
    ledger: &LedgerState,
    finalized_height: u64,
    integrity_key: &IntegrityKey,
    counters: &WorkCounterState,
) -> StorageResult<()> {
    let mut indexes = transaction
        .open_table(HISTORY_INDEXES)
        .map_err(database_error)?;
    let mut lock_ids = BTreeSet::new();
    for effect in ledger.fastpay_version_fences.iter().filter(|effect| {
        effect.origin == postfiat_types::FastPayFenceOriginV1::Consensusless
            && matches!(
                effect.decision,
                postfiat_types::FastPayRecoveryDecisionV1::Confirmed { .. }
            )
    }) {
        effect.validate_shape().map_err(|error| {
            StorageError::new(
                StorageErrorCode::CorruptRecord,
                format!("retained FastPay effect is invalid: {error}"),
            )
        })?;
        if !lock_ids.insert(effect.lock_id.as_str()) {
            return Err(StorageError::new(
                StorageErrorCode::CorruptRecord,
                "retained FastPay effect lock id is duplicated",
            ));
        }
        let key = fastpay_anchor_key(&effect.lock_id)?;
        let record = StoredFastPayAnchorV1 {
            schema: STORED_FASTPAY_ANCHOR_SCHEMA.to_owned(),
            finalized_height,
            effect: effect.clone(),
        };
        if let Some(raw) = read_authenticated(
            &indexes,
            HISTORY_INDEXES_TABLE,
            &key,
            MAX_RECORD_BYTES,
            integrity_key,
            counters,
        )? {
            if decode_json::<StoredFastPayAnchorV1>(&raw)? != record {
                return Err(StorageError::new(
                    StorageErrorCode::InitializationConflict,
                    "retained FastPay anchor index conflicts with checkpoint state",
                ));
            }
            continue;
        }
        insert_authenticated(
            &mut indexes,
            HISTORY_INDEXES_TABLE,
            &key,
            &encode_json(&record, MAX_RECORD_BYTES)?,
            MAX_RECORD_BYTES,
            integrity_key,
            counters,
        )?;
        if let Some(order_entry) =
            fastpay_order_index_entry_from_effect(effect, finalized_height, 0)
        {
            let order_key = fastpay_order_index_key(&order_entry.order_hash)?;
            if let Some(raw) = read_authenticated(
                &indexes,
                HISTORY_INDEXES_TABLE,
                &order_key,
                MAX_RECORD_BYTES,
                integrity_key,
                counters,
            )? {
                if decode_json::<FastPayOrderIndexEntryV1>(&raw)? != order_entry {
                    return Err(StorageError::new(
                        StorageErrorCode::InitializationConflict,
                        "retained FastPay order index conflicts with checkpoint state",
                    ));
                }
            } else {
                insert_authenticated(
                    &mut indexes,
                    HISTORY_INDEXES_TABLE,
                    &order_key,
                    &encode_json(&order_entry, MAX_RECORD_BYTES)?,
                    MAX_RECORD_BYTES,
                    integrity_key,
                    counters,
                )?;
            }
        }
    }
    Ok(())
}
