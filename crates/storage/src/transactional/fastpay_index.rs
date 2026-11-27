use super::*;

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
