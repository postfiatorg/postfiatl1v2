include!("atomic_swap_response_validation.rs");

pub fn validate_health_response(
    response: &RpcResponse,
    kind: RpcResponseKind,
) -> Result<(), RpcResponseValidationError> {
    validate_response_kind(response, kind)
}

fn validate_status_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    nonzero_u64_field(result, "validator_count")?;
    clean_string_field(result, "node_id")?;
    expect_string_eq(result, "status", "running")?;
    u64_field(result, "last_run_unix")?;
    let state_root = string_field(result, "state_root")?;
    if !is_lower_hex_len(state_root, 96) {
        return Err(invalid_result(
            "state_root",
            "expected 96 lowercase hex characters",
        ));
    }
    let block_height = u64_field(result, "block_height")?;
    validate_block_tip_hash(
        block_height,
        string_field(result, "block_tip_hash")?,
        "block_tip_hash",
    )?;
    u64_field(result, "mempool_pending")?;
    Ok(())
}

fn validate_server_info_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", SERVER_INFO_SCHEMA)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    clean_string_field(result, "node_id")?;
    let node_status = clean_string_field(result, "status")?;
    if !matches!(node_status, "initialized" | "running") {
        return Err(invalid_result(
            "status",
            "expected initialized or running status",
        ));
    }
    let ledger_height = u64_field(result, "ledger.height")?;
    validate_block_tip_hash(
        ledger_height,
        string_field(result, "ledger.hash")?,
        "ledger.hash",
    )?;
    lower_hex_field(result, "ledger.state_root", 96)?;
    nonzero_u64_field(result, "validators.active_count")?;
    nonzero_u64_field(result, "fees.minimum_transfer_fee")?;
    u64_field(result, "mempool.pending")?;
    expect_string_eq(result, "rpc.version", RPC_VERSION)?;
    let aliases = array_field(result, "rpc.read_aliases")?;
    if aliases.is_empty() {
        return Err(invalid_result(
            "rpc.read_aliases",
            "expected at least one read alias",
        ));
    }
    for alias in aliases {
        clean_string_entry(alias, "rpc.read_aliases")?;
    }
    Ok(())
}

fn validate_metrics_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", METRICS_SCHEMA)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    clean_string_field(result, "node_id")?;
    nonzero_u64_field(result, "consensus.active_validator_count")?;
    nonzero_u64_field(result, "consensus.crypto_policy_version")?;
    nonzero_u64_field(result, "consensus.bridge_witness_epoch")?;
    u64_field(result, "consensus.amendment_count")?;
    let block_height = u64_field(result, "ordering.block_height")?;
    validate_block_tip_hash(
        block_height,
        string_field(result, "ordering.block_tip_hash")?,
        "ordering.block_tip_hash",
    )?;
    u64_field(result, "ordering.ordered_batch_count")?;
    u64_field(result, "ordering.archived_batch_count")?;
    u64_field(result, "execution.account_count")?;
    u64_field(result, "execution.receipt_count")?;
    u64_field(result, "execution.burned_fee_total")?;
    nonzero_u64_field(result, "execution.account_reserve")?;
    nonzero_u64_field(result, "execution.minimum_transfer_fee")?;
    nonzero_u64_field(result, "execution.transfer_account_creation_fee")?;
    nonzero_u64_field(result, "execution.transfer_fee_byte_quantum")?;
    nonzero_u64_field(result, "execution.transfer_fee_per_quantum")?;
    lower_hex_field(result, "execution.state_root", 96)?;
    let asset_count = u64_field(result, "assets.asset_count")?;
    let trustline_count = u64_field(result, "assets.trustline_count")?;
    let holder_count = u64_field(result, "assets.holder_count")?;
    u64_field(result, "assets.total_outstanding_supply")?;
    u64_field(result, "assets.open_issued_escrow_count")?;
    u64_field(result, "assets.open_issued_escrow_amount")?;
    u64_field(result, "assets.open_issued_offer_count")?;
    u64_field(result, "assets.open_issued_offer_amount")?;
    let authorization_required_asset_count =
        u64_field(result, "assets.authorization_required_asset_count")?;
    let freeze_enabled_asset_count = u64_field(result, "assets.freeze_enabled_asset_count")?;
    let clawback_enabled_asset_count = u64_field(result, "assets.clawback_enabled_asset_count")?;
    let unauthorized_trustline_count = u64_field(result, "assets.unauthorized_trustline_count")?;
    let frozen_trustline_count = u64_field(result, "assets.frozen_trustline_count")?;
    if holder_count > trustline_count {
        return Err(invalid_result(
            "assets.holder_count",
            "expected holder_count to be less than or equal to trustline_count",
        ));
    }
    if authorization_required_asset_count > asset_count {
        return Err(invalid_result(
            "assets.authorization_required_asset_count",
            "expected authorization_required_asset_count to be less than or equal to asset_count",
        ));
    }
    if freeze_enabled_asset_count > asset_count {
        return Err(invalid_result(
            "assets.freeze_enabled_asset_count",
            "expected freeze_enabled_asset_count to be less than or equal to asset_count",
        ));
    }
    if clawback_enabled_asset_count > asset_count {
        return Err(invalid_result(
            "assets.clawback_enabled_asset_count",
            "expected clawback_enabled_asset_count to be less than or equal to asset_count",
        ));
    }
    if unauthorized_trustline_count > trustline_count {
        return Err(invalid_result(
            "assets.unauthorized_trustline_count",
            "expected unauthorized_trustline_count to be less than or equal to trustline_count",
        ));
    }
    if frozen_trustline_count > trustline_count {
        return Err(invalid_result(
            "assets.frozen_trustline_count",
            "expected frozen_trustline_count to be less than or equal to trustline_count",
        ));
    }
    u64_field(result, "mempool.pending")?;
    nonzero_u64_field(result, "storage.replicated_state_file_count")?;
    u64_field(result, "shielded.note_count")?;
    u64_field(result, "shielded.nullifier_count")?;
    u64_field(result, "shielded.turnstile_event_count")?;
    u64_field(result, "bridge.domain_count")?;
    u64_field(result, "bridge.transfer_count")?;
    u64_field(result, "bridge.replay_cache_count")?;
    Ok(())
}

fn validate_ledger_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", LEDGER_SCHEMA)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    let ledger_index = u64_field(result, "ledger_index")?;
    validate_block_tip_hash(
        ledger_index,
        string_field(result, "ledger_hash")?,
        "ledger_hash",
    )?;
    lower_hex_field(result, "state_root", 96)?;
    u64_field(result, "account_count")?;
    u64_field(result, "receipt_count")?;
    u64_field(result, "burned_fee_total")?;
    let returned_block_count = u64_field(result, "returned_block_count")?;
    let blocks = array_field(result, "blocks")?;
    if returned_block_count != blocks.len() as u64 {
        return Err(invalid_result(
            "returned_block_count",
            "expected returned_block_count to match blocks length",
        ));
    }
    validate_blocks_result(field(result, "blocks")?)?;
    Ok(())
}

fn validate_block_tip_hash(
    block_height: u64,
    block_tip_hash: &str,
    field: &str,
) -> Result<(), RpcResponseValidationError> {
    if block_height == 0 {
        if block_tip_hash != "genesis" {
            return Err(invalid_result(field, "expected `genesis` for height zero"));
        }
    } else if !is_lower_hex_len(block_tip_hash, 96) {
        return Err(invalid_result(
            field,
            "expected 96 lowercase hex characters",
        ));
    }
    Ok(())
}

fn validate_state_verification_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", STATE_VERIFICATION_SCHEMA)?;
    expect_bool_eq(result, "verified", true)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;

    expect_bool_eq(result, "block_log.verified", true)?;
    let block_count = u64_field(result, "block_log.block_count")?;
    validate_block_tip_hash(
        block_count,
        string_field(result, "block_log.tip_hash")?,
        "block_log.tip_hash",
    )?;
    lower_hex_field(result, "block_log.state_root", 96)?;

    expect_bool_eq(result, "governance.verified", true)?;
    nonzero_u64_field(result, "governance.active_validator_count")?;
    nonzero_u64_field(result, "governance.crypto_policy_version")?;
    nonzero_u64_field(result, "governance.bridge_witness_epoch")?;
    let amendment_count = u64_field(result, "governance.amendment_count")?;
    validate_optional_hash_for_count(
        amendment_count,
        string_field(result, "governance.latest_amendment_id")?,
        "governance.latest_amendment_id",
    )?;

    expect_bool_eq(result, "bridge.verified", true)?;
    u64_field(result, "bridge.domain_count")?;
    let bridge_transfer_count = u64_field(result, "bridge.transfer_count")?;
    let bridge_attestation_count = u64_field(result, "bridge.attestation_count")?;
    if bridge_attestation_count > bridge_transfer_count {
        return Err(invalid_result(
            "bridge.attestation_count",
            "expected no more attestations than transfers",
        ));
    }
    let bridge_replay_cache_count = u64_field(result, "bridge.replay_cache_count")?;
    if bridge_replay_cache_count > bridge_transfer_count {
        return Err(invalid_result(
            "bridge.replay_cache_count",
            "expected no more replay cache entries than transfers",
        ));
    }
    u64_field(result, "bridge.inbound_used")?;
    u64_field(result, "bridge.outbound_used")?;
    validate_optional_hash_for_count(
        bridge_transfer_count,
        string_field(result, "bridge.latest_transfer_id")?,
        "bridge.latest_transfer_id",
    )?;

    expect_bool_eq(result, "shielded.verified", true)?;
    let note_count = u64_field(result, "shielded.note_count")?;
    u64_field(result, "shielded.nullifier_count")?;
    let turnstile_event_count = u64_field(result, "shielded.turnstile_event_count")?;
    lower_hex_field(result, "shielded.tree_root", 96)?;
    u64_field(result, "shielded.bootstrap_deposit_total")?;
    u64_field(result, "shielded.migration_total")?;
    u64_field(result, "shielded.orchard_deposit_total")?;
    let spent_note_count = u64_field(result, "shielded.spent_note_count")?;
    let live_note_count = u64_field(result, "shielded.live_note_count")?;
    match spent_note_count.checked_add(live_note_count) {
        Some(total) if total == note_count => {}
        _ => {
            return Err(invalid_result(
                "shielded.live_note_count",
                "expected spent_note_count plus live_note_count to equal note_count",
            ));
        }
    }
    validate_optional_hash_for_count(
        turnstile_event_count,
        string_field(result, "shielded.latest_turnstile_event_id")?,
        "shielded.latest_turnstile_event_id",
    )?;

    expect_bool_eq(result, "mempool.verified", true)?;
    let pending_count = u64_field(result, "mempool.pending_count")?;
    let sender_count = u64_field(result, "mempool.sender_count")?;
    if pending_count
        .checked_mul(2)
        .is_some_and(|sender_count_limit| sender_count > sender_count_limit)
    {
        return Err(invalid_result(
            "mempool.sender_count",
            "expected no more than two senders per pending transaction",
        ));
    }
    u64_field(result, "mempool.total_amount")?;
    u64_field(result, "mempool.total_fee")?;
    validate_optional_hash_for_count(
        pending_count,
        string_field(result, "mempool.latest_tx_id")?,
        "mempool.latest_tx_id",
    )?;
    Ok(())
}

fn validate_optional_hash_for_count(
    count: u64,
    value: &str,
    field: &str,
) -> Result<(), RpcResponseValidationError> {
    if count == 0 {
        if !value.is_empty() {
            return Err(invalid_result(
                field,
                "expected empty value when count is zero",
            ));
        }
    } else if !is_lower_hex_len(value, 96) {
        return Err(invalid_result(
            field,
            "expected 96 lowercase hex characters",
        ));
    }
    Ok(())
}

fn validate_local_key_result(
    result: &Value,
    validators: Option<u32>,
) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", LOCAL_KEY_VALIDATION_SCHEMA)?;
    clean_string_field(result, "node_id")?;
    if let Some(validators) = validators {
        let found = u64_field(result, "required_validator_count")?;
        if found != u64::from(validators) {
            return Err(invalid_result(
                "required_validator_count",
                format!("expected {validators}, found {found}"),
            ));
        }
    }
    expect_bool_eq(result, "faucet_key_valid", true)?;
    expect_bool_eq(result, "faucet_key_permissions_valid", true)?;
    clean_string_field(result, "faucet_address")?;
    expect_bool_eq(result, "validator_keys_valid", true)?;
    expect_bool_eq(result, "validator_key_permissions_valid", true)?;
    let validator_key_count = u64_field(result, "validator_key_count")?;
    let required_validator_count = nonzero_u64_field(result, "required_validator_count")?;
    if validator_key_count < required_validator_count {
        return Err(invalid_result(
            "validator_key_count",
            "expected at least required_validator_count keys",
        ));
    }
    if contains_key_material_field(result) {
        return Err(invalid_result(
            "result",
            "local-key validation response contains key material fields",
        ));
    }
    Ok(())
}

fn validate_account_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    clean_string_field(result, "address")?;
    u64_field(result, "balance")?;
    u64_field(result, "sequence")?;
    if result.get("public_key_hex").is_some() {
        optional_lower_hex_string_field_value(result, "public_key_hex")?;
    }
    Ok(())
}

fn validate_account_tx_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", "postfiat-account-tx-v1")?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    let address = clean_string_field(result, "address")?;
    optional_u64_result_field(result, "from_height")?;
    optional_u64_result_field(result, "to_height")?;
    let scan_limit = nonzero_u64_field(result, "scan_limit")?;
    if scan_limit > MAX_RPC_READ_QUERY_LIMIT as u64 {
        return Err(invalid_result(
            "scan_limit",
            format!("expected scan_limit <= {MAX_RPC_READ_QUERY_LIMIT}"),
        ));
    }
    let scanned_block_count = u64_field(result, "scanned_block_count")?;
    if scanned_block_count > scan_limit {
        return Err(invalid_result(
            "scanned_block_count",
            "expected scanned_block_count to be <= scan_limit",
        ));
    }
    let archive_lookup_count = u64_field(result, "archive_lookup_count")?;
    if archive_lookup_count > scanned_block_count {
        return Err(invalid_result(
            "archive_lookup_count",
            "expected archive_lookup_count to be <= scanned_block_count",
        ));
    }
    bool_field(result, "truncated")?;
    let row_count = u64_field(result, "row_count")?;
    if row_count > scan_limit {
        return Err(invalid_result(
            "row_count",
            "expected row_count to be <= scan_limit",
        ));
    }
    let rows = array_field(result, "rows")?;
    if row_count != rows.len() as u64 {
        return Err(invalid_result(
            "row_count",
            "expected row_count to match rows length",
        ));
    }
    for row in rows {
        lower_hex_field(row, "tx_id", 96)?;
        nonzero_u64_field(row, "block_height")?;
        expect_string_eq(row, "batch_kind", "transparent")?;
        lower_hex_field(row, "batch_id", 96)?;
        u64_field(row, "transaction_index")?;
        if let Some(transaction_kind) = row.get("transaction_kind") {
            let transaction_kind = transaction_kind
                .as_str()
                .ok_or_else(|| invalid_result("transaction_kind", "expected string value"))?;
            if !matches!(
                transaction_kind,
                TRANSPARENT_TRANSFER_KIND | PAYMENT_V2_TRANSACTION_KIND
            ) && !is_supported_asset_transaction_kind(transaction_kind)
                && transaction_kind != ATOMIC_SWAP_TRANSACTION_KIND
                && !is_supported_escrow_transaction_kind(transaction_kind)
                && !is_supported_nft_transaction_kind(transaction_kind)
                && !is_supported_offer_transaction_kind(transaction_kind)
            {
                return Err(invalid_result(
                    "transaction_kind",
                    "expected transparent_transfer, payment_v2, supported asset transaction kind, supported escrow transaction kind, supported nft transaction kind, or supported offer transaction kind",
                ));
            }
        }
        let from = clean_string_field(row, "from")?;
        let to = clean_string_field(row, "to")?;
        if from != address && to != address {
            return Err(invalid_result(
                "rows",
                "expected every account_tx row to involve the requested address",
            ));
        }
        u64_field(row, "amount")?;
        u64_field(row, "fee")?;
        u64_field(row, "sequence")?;
        optional_lower_hex_string_field_value(row, "memo_hash")?;
        if row.get("memo_count").is_some() {
            optional_u64_result_field(row, "memo_count")?;
        }
        if row.get("memo_bytes").is_some() {
            optional_u64_result_field(row, "memo_bytes")?;
        }
        if let Some(asset_id) =
            optional_lower_hex_len_field_value(row, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?
        {
            let transaction_kind = row
                .get("transaction_kind")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !is_supported_asset_transaction_kind(transaction_kind)
                && transaction_kind != ATOMIC_SWAP_TRANSACTION_KIND
                && !is_supported_escrow_transaction_kind(transaction_kind)
                && !is_supported_offer_transaction_kind(transaction_kind)
            {
                return Err(invalid_result(
                    "asset_id",
                    format!("unexpected asset_id `{asset_id}` on account_tx row"),
                ));
            }
        }
        if row.get("issuer").is_some() {
            optional_clean_string_result_field(row, "issuer")?;
            let transaction_kind = row
                .get("transaction_kind")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if !is_supported_asset_transaction_kind(transaction_kind)
                && transaction_kind != ATOMIC_SWAP_TRANSACTION_KIND
                && transaction_kind != NFT_MINT_TRANSACTION_KIND
                && transaction_kind != NFT_TRANSFER_TRANSACTION_KIND
            {
                return Err(invalid_result(
                    "issuer",
                    "unexpected issuer on non-asset or non-nft account_tx row",
                ));
            }
        }
        if row.get("trustline_authorized").is_some() {
            optional_bool_result_field(row, "trustline_authorized")?;
            if row.get("transaction_kind").and_then(Value::as_str)
                != Some(TRUST_SET_TRANSACTION_KIND)
            {
                return Err(invalid_result(
                    "trustline_authorized",
                    "unexpected trustline_authorized on non-trust_set account_tx row",
                ));
            }
        }
        if row.get("trustline_frozen").is_some() {
            optional_bool_result_field(row, "trustline_frozen")?;
            if row.get("transaction_kind").and_then(Value::as_str)
                != Some(TRUST_SET_TRANSACTION_KIND)
            {
                return Err(invalid_result(
                    "trustline_frozen",
                    "unexpected trustline_frozen on non-trust_set account_tx row",
                ));
            }
        }
        if row.get("nft_id").is_some() {
            optional_lower_hex_len_field_value(row, "nft_id", NFT_ID_HEX_LEN)?;
            if !is_supported_nft_transaction_kind(
                row.get("transaction_kind")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ) {
                return Err(invalid_result(
                    "nft_id",
                    "unexpected nft_id on non-nft account_tx row",
                ));
            }
        }
        if row.get("nft_issuer_transfer_fee").is_some() {
            optional_u64_result_field(row, "nft_issuer_transfer_fee")?;
            if row.get("transaction_kind").and_then(Value::as_str)
                != Some(NFT_TRANSFER_TRANSACTION_KIND)
            {
                return Err(invalid_result(
                    "nft_issuer_transfer_fee",
                    "unexpected nft_issuer_transfer_fee on non-nft-transfer account_tx row",
                ));
            }
        }
        if row.get("nft_collection_flags").is_some() {
            let Some(collection_flags) = optional_u64_field_value(row, "nft_collection_flags")?
            else {
                return Err(invalid_result(
                    "nft_collection_flags",
                    "expected non-null collection flags when present",
                ));
            };
            if collection_flags > u32::MAX as u64 {
                return Err(invalid_result(
                    "nft_collection_flags",
                    "expected u32-compatible collection flags",
                ));
            }
            if row.get("transaction_kind").and_then(Value::as_str)
                != Some(NFT_MINT_TRANSACTION_KIND)
            {
                return Err(invalid_result(
                    "nft_collection_flags",
                    "unexpected nft_collection_flags on non-nft-mint account_tx row",
                ));
            }
        }
        if row.get("escrow_id").is_some() {
            optional_lower_hex_len_field_value(row, "escrow_id", ESCROW_ID_HEX_LEN)?;
            if !is_supported_escrow_transaction_kind(
                row.get("transaction_kind")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ) {
                return Err(invalid_result(
                    "escrow_id",
                    "unexpected escrow_id on non-escrow account_tx row",
                ));
            }
        }
        if row.get("offer_id").is_some() {
            optional_lower_hex_len_field_value(row, "offer_id", OFFER_ID_HEX_LEN)?;
            if !is_supported_offer_transaction_kind(
                row.get("transaction_kind")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
            ) {
                return Err(invalid_result(
                    "offer_id",
                    "unexpected offer_id on non-offer account_tx row",
                ));
            }
        }
        if let Some(tx_role) = row.get("tx_role") {
            let tx_role = tx_role
                .as_str()
                .ok_or_else(|| invalid_result("tx_role", "expected string value"))?;
            let transaction_kind = row
                .get("transaction_kind")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if transaction_kind == ATOMIC_SWAP_TRANSACTION_KIND {
                if !matches!(tx_role, "leg_0" | "leg_1") {
                    return Err(invalid_result(
                        "tx_role",
                        "expected atomic swap tx_role leg_0 or leg_1",
                    ));
                }
            } else if !is_supported_offer_transaction_kind(transaction_kind) {
                return Err(invalid_result(
                    "tx_role",
                    "unexpected tx_role on non-offer/non-atomic account_tx row",
                ));
            } else if !matches!(
                tx_role,
                OFFER_TX_ROLE_TAKER | OFFER_TX_ROLE_MAKER | OFFER_TX_ROLE_CANCEL
            ) {
                return Err(invalid_result(
                    "tx_role",
                    "expected supported offer tx role",
                ));
            }
        }
        if row.get("counterparty_offer_id").is_some() {
            optional_lower_hex_len_field_value(row, "counterparty_offer_id", OFFER_ID_HEX_LEN)?;
            if row.get("tx_role").and_then(Value::as_str) != Some(OFFER_TX_ROLE_MAKER) {
                return Err(invalid_result(
                    "counterparty_offer_id",
                    "expected counterparty_offer_id only on maker offer rows",
                ));
            }
        }
        if row.get("fill_index").is_some() {
            optional_u64_result_field(row, "fill_index")?;
            if row.get("tx_role").and_then(Value::as_str) != Some(OFFER_TX_ROLE_MAKER) {
                return Err(invalid_result(
                    "fill_index",
                    "expected fill_index only on maker offer rows",
                ));
            }
        }
        if row.get("condition_hash").is_some() {
            optional_lower_hex_len_field_value(
                row,
                "condition_hash",
                ESCROW_CONDITION_HASH_HEX_LEN,
            )?;
            if row.get("transaction_kind").and_then(Value::as_str)
                != Some(ESCROW_CREATE_TRANSACTION_KIND)
            {
                return Err(invalid_result(
                    "condition_hash",
                    "unexpected condition_hash on non-escrow-create account_tx row",
                ));
            }
        }
        optional_bool_result_field(row, "accepted")?;
        optional_clean_string_result_field(row, "receipt_code")?;
    }
    Ok(())
}

fn validate_fee_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", FEE_SCHEMA)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    nonzero_u64_field(result, "minimum_transfer_fee")?;
    nonzero_u64_field(result, "account_reserve")?;
    nonzero_u64_field(result, "transfer_account_creation_fee")?;
    nonzero_u64_field(result, "transfer_fee_byte_quantum")?;
    nonzero_u64_field(result, "transfer_fee_per_quantum")?;
    u64_field(result, "burned_fee_total")?;
    Ok(())
}

fn validate_transfer_fee_quote_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", TRANSFER_FEE_QUOTE_SCHEMA)?;
    if let Some(transaction_kind) = result.get("transaction_kind") {
        let transaction_kind = transaction_kind
            .as_str()
            .ok_or_else(|| invalid_result("transaction_kind", "expected string value"))?;
        if !matches!(
            transaction_kind,
            TRANSPARENT_TRANSFER_KIND | PAYMENT_V2_TRANSACTION_KIND
        ) {
            return Err(invalid_result(
                "transaction_kind",
                "expected transparent_transfer or payment_v2",
            ));
        }
    }
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    clean_string_field(result, "from")?;
    clean_string_field(result, "to")?;
    nonzero_u64_field(result, "amount")?;
    nonzero_u64_field(result, "sequence")?;
    let sequence_source = clean_string_field(result, "sequence_source")?;
    if !matches!(sequence_source, "explicit" | "ledger_mempool") {
        return Err(invalid_result(
            "sequence_source",
            "expected explicit or ledger_mempool",
        ));
    }
    u64_field(result, "sender_balance")?;
    u64_field(result, "sender_sequence")?;
    u64_field(result, "mempool_pending_for_sender")?;
    bool_field(result, "recipient_exists")?;
    bool_field(result, "will_create_recipient_account")?;
    nonzero_u64_field(result, "base_transfer_fee")?;
    u64_field(result, "state_expansion_fee")?;
    nonzero_u64_field(result, "minimum_fee")?;
    nonzero_u64_field(result, "account_reserve")?;
    nonzero_u64_field(result, "transfer_account_creation_fee")?;
    nonzero_u64_field(result, "transfer_fee_byte_quantum")?;
    nonzero_u64_field(result, "transfer_fee_per_quantum")?;
    nonzero_u64_field(result, "transfer_weight_bytes")?;
    if result.get("memo_count").is_some() {
        optional_u64_result_field(result, "memo_count")?;
    }
    if result.get("memo_bytes").is_some() {
        optional_u64_result_field(result, "memo_bytes")?;
    }
    optional_u64_result_field(result, "sender_balance_after_amount_and_fee")?;
    bool_field(result, "sender_meets_reserve_after_transfer")?;
    optional_u64_result_field(result, "recipient_balance_after_amount")?;
    bool_field(result, "recipient_meets_reserve_after_transfer")?;
    Ok(())
}

fn validate_asset_fee_quote_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ASSET_FEE_QUOTE_SCHEMA)?;
    let transaction_kind = clean_string_field(result, "transaction_kind")?;
    if !is_supported_asset_transaction_kind(transaction_kind) {
        return Err(invalid_result(
            "transaction_kind",
            "expected supported asset transaction kind",
        ));
    }
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    clean_string_field(result, "source")?;
    nonzero_u64_field(result, "sequence")?;
    let sequence_source = clean_string_field(result, "sequence_source")?;
    if !matches!(sequence_source, "explicit" | "ledger_mempool") {
        return Err(invalid_result(
            "sequence_source",
            "expected explicit or ledger_mempool",
        ));
    }
    u64_field(result, "sender_balance")?;
    u64_field(result, "sender_sequence")?;
    u64_field(result, "mempool_pending_for_sender")?;
    nonzero_u64_field(result, "base_asset_fee")?;
    u64_field(result, "state_expansion_fee")?;
    nonzero_u64_field(result, "minimum_fee")?;
    nonzero_u64_field(result, "account_reserve")?;
    nonzero_u64_field(result, "transfer_fee_byte_quantum")?;
    nonzero_u64_field(result, "transfer_fee_per_quantum")?;
    nonzero_u64_field(result, "asset_weight_bytes")?;
    optional_u64_result_field(result, "sender_balance_after_fee")?;
    bool_field(result, "sender_meets_reserve_after_fee")?;
    validate_asset_operation_fields(field(result, "operation")?, transaction_kind)?;
    Ok(())
}

fn validate_escrow_fee_quote_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ESCROW_FEE_QUOTE_SCHEMA)?;
    let transaction_kind = clean_string_field(result, "transaction_kind")?;
    if !is_supported_escrow_transaction_kind(transaction_kind) {
        return Err(invalid_result(
            "transaction_kind",
            "expected supported escrow transaction kind",
        ));
    }
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    clean_string_field(result, "source")?;
    nonzero_u64_field(result, "sequence")?;
    let sequence_source = clean_string_field(result, "sequence_source")?;
    if !matches!(sequence_source, "explicit" | "ledger_mempool") {
        return Err(invalid_result(
            "sequence_source",
            "expected explicit or ledger_mempool",
        ));
    }
    u64_field(result, "sender_balance")?;
    u64_field(result, "sender_sequence")?;
    u64_field(result, "mempool_pending_for_sender")?;
    nonzero_u64_field(result, "base_escrow_fee")?;
    u64_field(result, "state_expansion_fee")?;
    nonzero_u64_field(result, "minimum_fee")?;
    nonzero_u64_field(result, "account_reserve")?;
    nonzero_u64_field(result, "transfer_fee_byte_quantum")?;
    nonzero_u64_field(result, "transfer_fee_per_quantum")?;
    nonzero_u64_field(result, "escrow_weight_bytes")?;
    optional_u64_result_field(result, "sender_balance_after_fee")?;
    bool_field(result, "sender_meets_reserve_after_fee")?;
    validate_escrow_operation_fields(field(result, "operation")?, transaction_kind)?;
    Ok(())
}

fn validate_nft_fee_quote_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", NFT_FEE_QUOTE_SCHEMA)?;
    let transaction_kind = clean_string_field(result, "transaction_kind")?;
    if !is_supported_nft_transaction_kind(transaction_kind) {
        return Err(invalid_result(
            "transaction_kind",
            "expected supported nft transaction kind",
        ));
    }
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    clean_string_field(result, "source")?;
    nonzero_u64_field(result, "sequence")?;
    let sequence_source = clean_string_field(result, "sequence_source")?;
    if !matches!(sequence_source, "explicit" | "ledger_mempool") {
        return Err(invalid_result(
            "sequence_source",
            "expected explicit or ledger_mempool",
        ));
    }
    u64_field(result, "sender_balance")?;
    u64_field(result, "sender_sequence")?;
    u64_field(result, "mempool_pending_for_sender")?;
    nonzero_u64_field(result, "base_nft_fee")?;
    u64_field(result, "state_expansion_fee")?;
    nonzero_u64_field(result, "minimum_fee")?;
    nonzero_u64_field(result, "account_reserve")?;
    nonzero_u64_field(result, "transfer_fee_byte_quantum")?;
    nonzero_u64_field(result, "transfer_fee_per_quantum")?;
    nonzero_u64_field(result, "nft_weight_bytes")?;
    optional_u64_result_field(result, "sender_balance_after_fee")?;
    bool_field(result, "sender_meets_reserve_after_fee")?;
    let issuer_transfer_fee = u64_field(result, "issuer_transfer_fee")?;
    let issuer_transfer_fee_recipient_present = match result.get("issuer_transfer_fee_recipient") {
        Some(value) if !value.is_null() => {
            clean_string_field(result, "issuer_transfer_fee_recipient")?;
            true
        }
        _ => false,
    };
    optional_u64_result_field(result, "sender_balance_after_fee_and_issuer_transfer_fee")?;
    bool_field(
        result,
        "sender_meets_reserve_after_fee_and_issuer_transfer_fee",
    )?;
    if issuer_transfer_fee == 0 && issuer_transfer_fee_recipient_present {
        return Err(invalid_result(
            "issuer_transfer_fee_recipient",
            "expected recipient only when issuer_transfer_fee is nonzero",
        ));
    }
    if issuer_transfer_fee != 0 && !issuer_transfer_fee_recipient_present {
        return Err(invalid_result(
            "issuer_transfer_fee_recipient",
            "expected recipient when issuer_transfer_fee is nonzero",
        ));
    }
    validate_nft_operation_fields(field(result, "operation")?, transaction_kind)?;
    Ok(())
}

fn validate_offer_fee_quote_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", OFFER_FEE_QUOTE_SCHEMA)?;
    let transaction_kind = clean_string_field(result, "transaction_kind")?;
    if !is_supported_offer_transaction_kind(transaction_kind) {
        return Err(invalid_result(
            "transaction_kind",
            "expected supported offer transaction kind",
        ));
    }
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    clean_string_field(result, "source")?;
    nonzero_u64_field(result, "sequence")?;
    let sequence_source = clean_string_field(result, "sequence_source")?;
    if !matches!(sequence_source, "explicit" | "ledger_mempool") {
        return Err(invalid_result(
            "sequence_source",
            "expected explicit or ledger_mempool",
        ));
    }
    u64_field(result, "sender_balance")?;
    u64_field(result, "sender_sequence")?;
    u64_field(result, "mempool_pending_for_sender")?;
    nonzero_u64_field(result, "base_offer_fee")?;
    u64_field(result, "match_fee")?;
    u64_field(result, "state_expansion_fee")?;
    u64_field(result, "estimated_cross_count")?;
    nonzero_u64_field(result, "max_dex_crosses_per_transaction")?;
    bool_field(result, "will_create_residual_offer")?;
    u64_field(result, "offer_object_reserve")?;
    nonzero_u64_field(result, "minimum_fee")?;
    nonzero_u64_field(result, "account_reserve")?;
    nonzero_u64_field(result, "transfer_fee_byte_quantum")?;
    nonzero_u64_field(result, "transfer_fee_per_quantum")?;
    nonzero_u64_field(result, "offer_weight_bytes")?;
    optional_u64_result_field(result, "sender_balance_after_fee")?;
    optional_u64_result_field(result, "sender_balance_after_fee_and_reserve")?;
    bool_field(result, "sender_meets_reserve_after_fee")?;
    bool_field(result, "sender_meets_reserve_after_fee_and_reserve")?;
    validate_offer_operation_fields(field(result, "operation")?, transaction_kind)?;
    Ok(())
}

fn validate_atomic_settlement_template_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ATOMIC_SETTLEMENT_TEMPLATE_SCHEMA)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    lower_hex_field(
        result,
        "settlement_id",
        ATOMIC_SETTLEMENT_TEMPLATE_ID_HEX_LEN,
    )?;
    lower_hex_field(result, "condition_hash", ESCROW_CONDITION_HASH_HEX_LEN)?;
    let condition = string_field(result, "condition")?;
    if condition.is_empty() {
        return Err(invalid_result(
            "condition",
            "expected nonempty atomic settlement condition",
        ));
    }
    let finish_after = u64_field(result, "finish_after")?;
    let cancel_after = nonzero_u64_field(result, "cancel_after")?;
    if finish_after != 0 && cancel_after <= finish_after {
        return Err(invalid_result(
            "cancel_after",
            "expected cancel_after greater than finish_after",
        ));
    }

    let left = field(result, "left")?;
    let right = field(result, "right")?;
    validate_atomic_settlement_template_leg_result(left, condition, finish_after, cancel_after)?;
    validate_atomic_settlement_template_leg_result(right, condition, finish_after, cancel_after)?;

    let left_owner = string_field(left, "owner")?;
    let left_recipient = string_field(left, "recipient")?;
    let left_asset_id = string_field(left, "asset_id")?;
    let right_owner = string_field(right, "owner")?;
    let right_recipient = string_field(right, "recipient")?;
    let right_asset_id = string_field(right, "asset_id")?;
    if left_owner != right_recipient || right_owner != left_recipient {
        return Err(invalid_result(
            "left/right",
            "expected reciprocal atomic settlement legs",
        ));
    }
    validate_atomic_settlement_asset_pair(left_asset_id, right_asset_id)?;
    Ok(())
}

fn validate_atomic_settlement_template_leg_result(
    leg: &Value,
    condition: &str,
    finish_after: u64,
    cancel_after: u64,
) -> Result<(), RpcResponseValidationError> {
    clean_string_field(leg, "owner")?;
    clean_string_field(leg, "recipient")?;
    clean_string_field(leg, "asset_id")?;
    nonzero_u64_field(leg, "amount")?;
    nonzero_u64_field(leg, "sequence")?;
    let sequence_source = clean_string_field(leg, "sequence_source")?;
    if !matches!(sequence_source, "explicit" | "ledger_mempool") {
        return Err(invalid_result(
            "sequence_source",
            "expected explicit or ledger_mempool",
        ));
    }
    lower_hex_field(leg, "escrow_id", ESCROW_ID_HEX_LEN)?;
    expect_string_eq(leg, "transaction_kind", ESCROW_CREATE_TRANSACTION_KIND)?;
    nonzero_u64_field(leg, "base_escrow_fee")?;
    u64_field(leg, "state_expansion_fee")?;
    nonzero_u64_field(leg, "minimum_fee")?;
    nonzero_u64_field(leg, "escrow_weight_bytes")?;
    u64_field(leg, "sender_balance")?;
    u64_field(leg, "sender_sequence")?;
    u64_field(leg, "mempool_pending_for_sender")?;
    optional_u64_result_field(leg, "sender_balance_after_fee")?;
    bool_field(leg, "sender_meets_reserve_after_fee")?;

    let operation = field(leg, "operation")?;
    validate_escrow_operation_fields(operation, ESCROW_CREATE_TRANSACTION_KIND)?;
    for key in ["owner", "recipient", "asset_id"] {
        if string_field(operation, key)? != string_field(leg, key)? {
            return Err(invalid_result(
                format!("operation.{key}"),
                "expected operation field to match leg",
            ));
        }
    }
    if nonzero_u64_field(operation, "amount")? != nonzero_u64_field(leg, "amount")? {
        return Err(invalid_result(
            "operation.amount",
            "expected operation amount to match leg",
        ));
    }
    if string_field(operation, "condition")? != condition {
        return Err(invalid_result(
            "operation.condition",
            "expected operation condition to match template",
        ));
    }
    if u64_field_default_zero(operation, "finish_after")? != finish_after {
        return Err(invalid_result(
            "operation.finish_after",
            "expected operation finish_after to match template",
        ));
    }
    if u64_field_default_zero(operation, "cancel_after")? != cancel_after {
        return Err(invalid_result(
            "operation.cancel_after",
            "expected operation cancel_after to match template",
        ));
    }
    Ok(())
}

fn validate_atomic_settlement_asset_pair(
    left_asset_id: &str,
    right_asset_id: &str,
) -> Result<(), RpcResponseValidationError> {
    let left_is_pft = left_asset_id == "PFT";
    let right_is_pft = right_asset_id == "PFT";
    if left_is_pft == right_is_pft {
        return Err(invalid_result(
            "asset_id",
            "expected exactly one PFT leg and one issued-asset leg",
        ));
    }
    let issued_asset_id = if left_is_pft {
        right_asset_id
    } else {
        left_asset_id
    };
    if !is_lower_hex_len(issued_asset_id, ISSUED_ASSET_ID_HEX_LEN) {
        return Err(invalid_result(
            "asset_id",
            format!(
                "expected issued asset id to be {ISSUED_ASSET_ID_HEX_LEN} lowercase hex characters"
            ),
        ));
    }
    Ok(())
}

fn validate_offer_info_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", OFFER_INFO_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    lower_hex_field(result, "offer_id", OFFER_ID_HEX_LEN)?;
    let found = bool_field(result, "found")?;
    let offer = field(result, "offer")?;
    match (found, offer.is_null()) {
        (true, false) => {
            validate_offer_result(offer)?;
            if string_field(offer, "offer_id")? != string_field(result, "offer_id")? {
                return Err(invalid_result(
                    "offer",
                    "expected offer object to match requested offer_id",
                ));
            }
            Ok(())
        }
        (false, true) => Ok(()),
        (true, true) => Err(invalid_result(
            "offer",
            "expected offer object when found is true",
        )),
        (false, false) => Err(invalid_result(
            "offer",
            "expected null offer when found is false",
        )),
    }
}

fn validate_account_offers_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ACCOUNT_OFFERS_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    clean_string_field(result, "account")?;
    optional_clean_string_result_field(result, "state")?;
    let requested_state = result.get("state").and_then(Value::as_str);
    if let Some(state) = requested_state {
        validate_offer_state_result_value(state)?;
    }
    validate_offer_collection_result(result, "offer_count", "offers", |offer| {
        if string_field(offer, "owner")? != string_field(result, "account")? {
            return Ok(false);
        }
        if let Some(state) = requested_state {
            if string_field(offer, "state")? != state {
                return Ok(false);
            }
        }
        Ok(true)
    })
}

fn validate_book_offers_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", BOOK_OFFERS_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    let taker_gets_asset_id = dex_asset_id_result_field(result, "taker_gets_asset_id")?;
    let taker_pays_asset_id = dex_asset_id_result_field(result, "taker_pays_asset_id")?;
    if taker_gets_asset_id == taker_pays_asset_id {
        return Err(invalid_result(
            "taker_pays_asset_id",
            "expected distinct DEX asset ids",
        ));
    }
    validate_offer_collection_result(result, "offer_count", "offers", |offer| {
        Ok(
            dex_asset_id_result_field(offer, "taker_gets_asset_id")? == taker_gets_asset_id
                && dex_asset_id_result_field(offer, "taker_pays_asset_id")? == taker_pays_asset_id
                && string_field(offer, "state")? == OFFER_STATE_OPEN,
        )
    })
}

fn validate_offer_collection_result(
    result: &Value,
    count_field: &str,
    array_field_name: &str,
    mut include_offer: impl FnMut(&Value) -> Result<bool, RpcResponseValidationError>,
) -> Result<(), RpcResponseValidationError> {
    validate_bounded_result_limit(result)?;
    bool_field(result, "truncated")?;
    let count = u64_field(result, count_field)?;
    let offers = array_field(result, array_field_name)?;
    if count != offers.len() as u64 {
        return Err(invalid_result(
            count_field,
            format!("expected {count_field} to match {array_field_name} length"),
        ));
    }
    for offer in offers {
        validate_offer_result(offer)?;
        if !include_offer(offer)? {
            return Err(invalid_result(
                array_field_name,
                "expected every offer to match requested query",
            ));
        }
    }
    Ok(())
}

fn validate_offer_result(offer: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(offer, "offer_id", OFFER_ID_HEX_LEN)?;
    clean_string_field(offer, "owner")?;
    nonzero_u64_field(offer, "owner_sequence")?;
    let taker_gets_asset_id = dex_asset_id_result_field(offer, "taker_gets_asset_id")?;
    let taker_pays_asset_id = dex_asset_id_result_field(offer, "taker_pays_asset_id")?;
    if taker_gets_asset_id == taker_pays_asset_id {
        return Err(invalid_result(
            "taker_pays_asset_id",
            "expected distinct DEX asset ids",
        ));
    }
    let taker_gets_amount_remaining = u64_field(offer, "taker_gets_amount_remaining")?;
    let taker_pays_amount_remaining = u64_field(offer, "taker_pays_amount_remaining")?;
    let original_taker_gets_amount = nonzero_u64_field(offer, "original_taker_gets_amount")?;
    let original_taker_pays_amount = nonzero_u64_field(offer, "original_taker_pays_amount")?;
    if taker_gets_amount_remaining > original_taker_gets_amount {
        return Err(invalid_result(
            "taker_gets_amount_remaining",
            "expected remaining amount to be <= original amount",
        ));
    }
    if taker_pays_amount_remaining > original_taker_pays_amount {
        return Err(invalid_result(
            "taker_pays_amount_remaining",
            "expected remaining amount to be <= original amount",
        ));
    }
    let created_height = nonzero_u64_field(offer, "created_height")?;
    let expiration_height = u64_field(offer, "expiration_height")?;
    if expiration_height != 0 && expiration_height <= created_height {
        return Err(invalid_result(
            "expiration_height",
            "expected zero or greater than created_height",
        ));
    }
    let reserve_paid = u64_field(offer, "reserve_paid")?;
    let state = clean_string_field(offer, "state")?;
    validate_offer_state_result_value(state)?;
    match state {
        OFFER_STATE_OPEN => {
            if taker_gets_amount_remaining == 0 || taker_pays_amount_remaining == 0 {
                return Err(invalid_result(
                    "state",
                    "expected open offers to have nonzero remaining amounts",
                ));
            }
            if reserve_paid == 0 {
                return Err(invalid_result(
                    "reserve_paid",
                    "expected open offers to retain reserve",
                ));
            }
        }
        OFFER_STATE_FILLED
            if taker_gets_amount_remaining != 0 || taker_pays_amount_remaining != 0 =>
        {
            return Err(invalid_result(
                "state",
                "expected filled offers to have zero remaining amounts",
            ));
        }
        _ => {}
    }
    Ok(())
}

fn validate_offer_state_result_value(state: &str) -> Result<(), RpcResponseValidationError> {
    if is_supported_offer_state(state) {
        Ok(())
    } else {
        Err(invalid_result(
            "state",
            "expected open, filled, canceled, or unfunded",
        ))
    }
}

fn dex_asset_id_result_field<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a str, RpcResponseValidationError> {
    let asset_id = clean_string_field(value, path)?;
    if asset_id == "PFT" || is_lower_hex_len(asset_id, ISSUED_ASSET_ID_HEX_LEN) {
        Ok(asset_id)
    } else {
        Err(invalid_result(
            path,
            format!("expected PFT or {ISSUED_ASSET_ID_HEX_LEN} lowercase hex characters"),
        ))
    }
}

fn validate_asset_info_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ASSET_INFO_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    lower_hex_field(result, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    let found = bool_field(result, "found")?;
    let asset = field(result, "asset")?;
    match (found, asset.is_null()) {
        (true, false) => validate_issued_asset_result(asset),
        (false, true) => Ok(()),
        (true, true) => Err(invalid_result(
            "asset",
            "expected asset object when found is true",
        )),
        (false, false) => Err(invalid_result(
            "asset",
            "expected null asset when found is false",
        )),
    }
}

fn validate_account_lines_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ACCOUNT_LINES_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    clean_string_field(result, "account")?;
    optional_clean_string_result_field(result, "issuer")?;
    let requested_issuer = result.get("issuer").and_then(Value::as_str);
    let requested_asset_id =
        optional_lower_hex_len_field_value(result, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    validate_asset_line_collection_result(
        result,
        "line_count",
        "lines",
        requested_issuer,
        requested_asset_id.as_deref(),
        false,
    )
}

fn validate_account_assets_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ACCOUNT_ASSETS_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    clean_string_field(result, "account")?;
    let requested_asset_id =
        optional_lower_hex_len_field_value(result, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    validate_asset_line_collection_result(
        result,
        "asset_count",
        "assets",
        None,
        requested_asset_id.as_deref(),
        true,
    )
}

fn validate_issuer_assets_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ISSUER_ASSETS_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    clean_string_field(result, "issuer")?;
    validate_bounded_result_limit(result)?;
    bool_field(result, "truncated")?;
    let asset_count = u64_field(result, "asset_count")?;
    let assets = array_field(result, "assets")?;
    if asset_count != assets.len() as u64 {
        return Err(invalid_result(
            "asset_count",
            "expected asset_count to match assets length",
        ));
    }
    for asset in assets {
        validate_issued_asset_result(asset)?;
        if string_field(asset, "issuer")? != string_field(result, "issuer")? {
            return Err(invalid_result(
                "assets",
                "expected every issuer_assets entry to match requested issuer",
            ));
        }
    }
    Ok(())
}

fn validate_escrow_info_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ESCROW_INFO_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    lower_hex_field(result, "escrow_id", ESCROW_ID_HEX_LEN)?;
    let found = bool_field(result, "found")?;
    let escrow = field(result, "escrow")?;
    match (found, escrow.is_null()) {
        (true, false) => validate_escrow_result(escrow),
        (false, true) => Ok(()),
        (true, true) => Err(invalid_result(
            "escrow",
            "expected escrow object when found is true",
        )),
        (false, false) => Err(invalid_result(
            "escrow",
            "expected null escrow when found is false",
        )),
    }
}

fn validate_account_escrows_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ACCOUNT_ESCROWS_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    clean_string_field(result, "account")?;
    optional_clean_string_result_field(result, "role")?;
    let requested_role = result.get("role").and_then(Value::as_str);
    if let Some(role) = requested_role {
        if !matches!(role, "owner" | "recipient") {
            return Err(invalid_result("role", "expected owner or recipient"));
        }
    }
    optional_clean_string_result_field(result, "state")?;
    let requested_state = result.get("state").and_then(Value::as_str);
    if let Some(state) = requested_state {
        validate_escrow_state_result_value(state)?;
    }
    validate_bounded_result_limit(result)?;
    bool_field(result, "truncated")?;
    let escrow_count = u64_field(result, "escrow_count")?;
    let escrows = array_field(result, "escrows")?;
    if escrow_count != escrows.len() as u64 {
        return Err(invalid_result(
            "escrow_count",
            "expected escrow_count to match escrows length",
        ));
    }
    let account = string_field(result, "account")?;
    for escrow in escrows {
        validate_escrow_result(escrow)?;
        let owner = string_field(escrow, "owner")?;
        let recipient = string_field(escrow, "recipient")?;
        let involves_account = match requested_role {
            Some("owner") => owner == account,
            Some("recipient") => recipient == account,
            None => owner == account || recipient == account,
            Some(_) => false,
        };
        if !involves_account {
            return Err(invalid_result(
                "escrows",
                "expected every escrow to involve requested account and role",
            ));
        }
        if let Some(state) = requested_state {
            if string_field(escrow, "state")? != state {
                return Err(invalid_result(
                    "escrows",
                    "expected every escrow to match requested state",
                ));
            }
        }
    }
    Ok(())
}

fn validate_nft_info_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", NFT_INFO_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    lower_hex_field(result, "nft_id", NFT_ID_HEX_LEN)?;
    let found = bool_field(result, "found")?;
    let nft = field(result, "nft")?;
    match (found, nft.is_null()) {
        (true, false) => {
            validate_nft_result(nft)?;
            if string_field(nft, "nft_id")? != string_field(result, "nft_id")? {
                return Err(invalid_result(
                    "nft",
                    "expected nft object to match requested nft_id",
                ));
            }
            Ok(())
        }
        (false, true) => Ok(()),
        (true, true) => Err(invalid_result(
            "nft",
            "expected nft object when found is true",
        )),
        (false, false) => Err(invalid_result(
            "nft",
            "expected null nft when found is false",
        )),
    }
}

fn validate_account_nfts_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ACCOUNT_NFTS_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    clean_string_field(result, "account")?;
    let include_burned = bool_field(result, "include_burned")?;
    validate_nft_collection_result(result, "nft_count", "nfts", include_burned, |nft| {
        Ok(string_field(nft, "owner")? == string_field(result, "account")?)
    })
}

fn validate_issuer_nfts_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ISSUER_NFTS_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    clean_string_field(result, "issuer")?;
    optional_clean_string_result_field(result, "collection_id")?;
    let requested_collection_id = result.get("collection_id").and_then(Value::as_str);
    let include_burned = bool_field(result, "include_burned")?;
    validate_nft_collection_result(result, "nft_count", "nfts", include_burned, |nft| {
        if string_field(nft, "issuer")? != string_field(result, "issuer")? {
            return Ok(false);
        }
        if let Some(collection_id) = requested_collection_id {
            if string_field(nft, "collection_id")? != collection_id {
                return Ok(false);
            }
        }
        Ok(true)
    })
}

fn validate_nft_collection_result(
    result: &Value,
    count_field: &str,
    array_field_name: &str,
    include_burned: bool,
    mut include_nft: impl FnMut(&Value) -> Result<bool, RpcResponseValidationError>,
) -> Result<(), RpcResponseValidationError> {
    validate_bounded_result_limit(result)?;
    bool_field(result, "truncated")?;
    let count = u64_field(result, count_field)?;
    let nfts = array_field(result, array_field_name)?;
    if count != nfts.len() as u64 {
        return Err(invalid_result(
            count_field,
            format!("expected {count_field} to match {array_field_name} length"),
        ));
    }
    for nft in nfts {
        validate_nft_result(nft)?;
        if !include_nft(nft)? {
            return Err(invalid_result(
                array_field_name,
                "expected every nft to match requested query",
            ));
        }
        if !include_burned && bool_field(nft, "burned")? {
            return Err(invalid_result(
                array_field_name,
                "expected burned nfts to be omitted unless include_burned is true",
            ));
        }
    }
    Ok(())
}

fn validate_nft_result(nft: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(nft, "nft_id", NFT_ID_HEX_LEN)?;
    clean_string_field(nft, "issuer")?;
    clean_string_field(nft, "collection_id")?;
    nonzero_u64_field(nft, "serial")?;
    clean_string_field(nft, "owner")?;
    lower_hex_string_field(nft, "metadata_hash")?;
    clean_string_field_allow_empty(nft, "metadata_uri")?;
    let flags = u64_field(nft, "flags")?;
    if flags > u32::MAX as u64 {
        return Err(invalid_result("flags", "expected u32-compatible flags"));
    }
    let collection_flags = u64_field(nft, "collection_flags")?;
    if collection_flags > u32::MAX as u64 {
        return Err(invalid_result(
            "collection_flags",
            "expected u32-compatible collection flags",
        ));
    }
    u64_field(nft, "issuer_transfer_fee")?;
    let transferable = bool_field(nft, "transferable")?;
    let issuer_burnable = bool_field(nft, "issuer_burnable")?;
    let collection_transfer_locked = bool_field(nft, "collection_transfer_locked")?;
    let collection_burn_locked = bool_field(nft, "collection_burn_locked")?;
    bool_field(nft, "burned")?;
    if transferable != (flags & NFT_FLAG_TRANSFERABLE as u64 != 0) {
        return Err(invalid_result(
            "transferable",
            "expected transferable to match flags",
        ));
    }
    if issuer_burnable != (flags & NFT_FLAG_ISSUER_BURNABLE as u64 != 0) {
        return Err(invalid_result(
            "issuer_burnable",
            "expected issuer_burnable to match flags",
        ));
    }
    if collection_transfer_locked
        != (collection_flags & NFT_COLLECTION_FLAG_TRANSFER_LOCKED as u64 != 0)
    {
        return Err(invalid_result(
            "collection_transfer_locked",
            "expected collection_transfer_locked to match collection_flags",
        ));
    }
    if collection_burn_locked != (collection_flags & NFT_COLLECTION_FLAG_BURN_LOCKED as u64 != 0) {
        return Err(invalid_result(
            "collection_burn_locked",
            "expected collection_burn_locked to match collection_flags",
        ));
    }
    Ok(())
}

fn validate_escrow_result(escrow: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(escrow, "escrow_id", ESCROW_ID_HEX_LEN)?;
    clean_string_field(escrow, "owner")?;
    nonzero_u64_field(escrow, "owner_sequence")?;
    clean_string_field(escrow, "recipient")?;
    clean_string_field(escrow, "asset_id")?;
    nonzero_u64_field(escrow, "amount")?;
    u64_field(escrow, "fee")?;
    optional_lower_hex_len_field_value(escrow, "condition_hash", ESCROW_CONDITION_HASH_HEX_LEN)?;
    u64_field(escrow, "finish_after")?;
    u64_field(escrow, "cancel_after")?;
    let state = clean_string_field(escrow, "state")?;
    validate_escrow_state_result_value(state)?;
    nonzero_u64_field(escrow, "created_height")?;
    Ok(())
}

fn validate_escrow_state_result_value(state: &str) -> Result<(), RpcResponseValidationError> {
    if matches!(
        state,
        ESCROW_STATE_OPEN | ESCROW_STATE_FINISHED | ESCROW_STATE_CANCELED
    ) {
        Ok(())
    } else {
        Err(invalid_result(
            "state",
            "expected open, finished, or canceled",
        ))
    }
}

fn validate_asset_read_common_fields(result: &Value) -> Result<(), RpcResponseValidationError> {
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    Ok(())
}

fn validate_asset_line_collection_result(
    result: &Value,
    count_field: &str,
    array_field_name: &str,
    requested_issuer: Option<&str>,
    requested_asset_id: Option<&str>,
    require_positive_balance: bool,
) -> Result<(), RpcResponseValidationError> {
    validate_bounded_result_limit(result)?;
    bool_field(result, "truncated")?;
    let count = u64_field(result, count_field)?;
    let lines = array_field(result, array_field_name)?;
    if count != lines.len() as u64 {
        return Err(invalid_result(
            count_field,
            format!("expected {count_field} to match {array_field_name} length"),
        ));
    }
    for line in lines {
        validate_asset_line_result(line)?;
        if string_field(line, "account")? != string_field(result, "account")? {
            return Err(invalid_result(
                array_field_name,
                "expected every line to match requested account",
            ));
        }
        if let Some(requested_issuer) = requested_issuer {
            if string_field(line, "issuer")? != requested_issuer {
                return Err(invalid_result(
                    array_field_name,
                    "expected every line to match requested issuer",
                ));
            }
        }
        if let Some(requested_asset_id) = requested_asset_id {
            if string_field(line, "asset_id")? != requested_asset_id {
                return Err(invalid_result(
                    array_field_name,
                    "expected every line to match requested asset_id",
                ));
            }
        }
        if require_positive_balance && u64_field(line, "balance")? == 0 {
            return Err(invalid_result(
                array_field_name,
                "expected every account asset line to have positive balance",
            ));
        }
    }
    Ok(())
}

fn validate_bounded_result_limit(result: &Value) -> Result<(), RpcResponseValidationError> {
    let limit = nonzero_u64_field(result, "limit")?;
    if limit > MAX_RPC_READ_QUERY_LIMIT as u64 {
        return Err(invalid_result(
            "limit",
            format!("expected limit <= {MAX_RPC_READ_QUERY_LIMIT}"),
        ));
    }
    Ok(())
}

fn validate_issued_asset_result(asset: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(asset, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    clean_string_field(asset, "issuer")?;
    clean_string_field(asset, "code")?;
    nonzero_u32_field(asset, "version")?;
    u64_field(asset, "precision")?;
    clean_string_field_allow_empty(asset, "display_name")?;
    let max_supply = optional_u64_field_value(asset, "max_supply")?;
    bool_field(asset, "requires_authorization")?;
    bool_field(asset, "freeze_enabled")?;
    bool_field(asset, "clawback_enabled")?;
    let outstanding_supply = u64_field(asset, "outstanding_supply")?;
    let trustline_count = u64_field(asset, "trustline_count")?;
    let holder_count = u64_field(asset, "holder_count")?;
    if holder_count > trustline_count {
        return Err(invalid_result(
            "holder_count",
            "expected holder_count to be <= trustline_count",
        ));
    }
    if max_supply.is_some_and(|max_supply| outstanding_supply > max_supply) {
        return Err(invalid_result(
            "outstanding_supply",
            "expected outstanding_supply to be <= max_supply",
        ));
    }
    Ok(())
}

fn validate_asset_line_result(line: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(line, "trustline_id", TRUSTLINE_ID_HEX_LEN)?;
    clean_string_field(line, "account")?;
    clean_string_field(line, "issuer")?;
    lower_hex_field(line, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    clean_string_field(line, "code")?;
    nonzero_u32_field(line, "version")?;
    u64_field(line, "precision")?;
    u64_field(line, "balance")?;
    nonzero_u64_field(line, "limit")?;
    bool_field(line, "authorized")?;
    bool_field(line, "frozen")?;
    u64_field(line, "reserve_paid")?;
    Ok(())
}

fn validate_validators_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", VALIDATORS_SCHEMA)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    let validator_count = nonzero_u64_field(result, "validator_count")?;
    lower_hex_field(result, "registry_root", 96)?;
    clean_string_field(result, "source_file")?;
    let validators = array_field(result, "validators")?;
    if validator_count != validators.len() as u64 {
        return Err(invalid_result(
            "validator_count",
            "expected validator_count to match validators length",
        ));
    }
    for validator in validators {
        clean_string_field(validator, "node_id")?;
        expect_string_eq(validator, "algorithm_id", ML_DSA_65_ALGORITHM)?;
        lower_hex_string_field(validator, "public_key_hex")?;
    }
    Ok(())
}

fn validate_manifests_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", MANIFESTS_SCHEMA)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    let available = bool_field(result, "available")?;
    clean_string_field(result, "source")?;
    let manifest_count = u64_field(result, "manifest_count")?;
    let manifests = array_field(result, "manifests")?;
    if manifest_count != manifests.len() as u64 {
        return Err(invalid_result(
            "manifest_count",
            "expected manifest_count to match manifests length",
        ));
    }
    let bundle_hash = string_field(result, "bundle_hash")?;
    if available {
        lower_hex_field(result, "bundle_hash", 96)?;
        clean_string_field(result, "network")?;
        nonzero_u64_field(result, "quorum")?;
    } else {
        if !bundle_hash.is_empty() {
            return Err(invalid_result(
                "bundle_hash",
                "expected empty bundle_hash when manifests are unavailable",
            ));
        }
        if manifest_count != 0 {
            return Err(invalid_result(
                "manifest_count",
                "expected zero manifests when manifests are unavailable",
            ));
        }
    }
    for manifest in manifests {
        clean_string_field(manifest, "validator_id")?;
        clean_string_field(manifest, "manifest_file")?;
        lower_hex_field(manifest, "manifest_hash", 96)?;
        lower_hex_string_field(manifest, "hot_public_key_hex")?;
        clean_string_field(manifest, "provider_group")?;
        clean_string_field(manifest, "region_group")?;
        clean_string_field(manifest, "jurisdiction_group")?;
        clean_string_field(manifest, "legal_domain_group")?;
        clean_string_field(manifest, "funding_domain_group")?;
    }
    Ok(())
}

fn validate_receipts_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    let receipts = result_array(result, "result")?;
    for receipt in receipts {
        validate_receipt_fields(receipt)?;
    }
    Ok(())
}

fn validate_receipt_fields(receipt: &Value) -> Result<(), RpcResponseValidationError> {
    clean_string_field(receipt, "tx_id")?;
    let accepted = bool_field(receipt, "accepted")?;
    let code = clean_string_field(receipt, "code")?;
    if accepted {
        if !matches!(code, "accepted" | "filled" | "partially_filled") {
            return Err(invalid_result(
                "code",
                "expected accepted receipts to use code `accepted`, `filled`, or `partially_filled`",
            ));
        }
    } else if matches!(code, "accepted" | "filled" | "partially_filled") {
        return Err(invalid_result(
            "code",
            "expected rejected receipts to use a non-accepted code",
        ));
    }
    clean_string_field(receipt, "message")?;
    if receipt.get("nft_issuer_transfer_fee").is_some() {
        let nft_issuer_transfer_fee = optional_u64_field_value(receipt, "nft_issuer_transfer_fee")?;
        let recipient_present = match receipt.get("nft_issuer_transfer_fee_recipient") {
            Some(value) if !value.is_null() => {
                clean_string_field(receipt, "nft_issuer_transfer_fee_recipient")?;
                true
            }
            _ => false,
        };
        if nft_issuer_transfer_fee.is_some_and(|fee| fee != 0) && !recipient_present {
            return Err(invalid_result(
                "nft_issuer_transfer_fee_recipient",
                "expected recipient when nft_issuer_transfer_fee is nonzero",
            ));
        }
        if nft_issuer_transfer_fee.is_some_and(|fee| fee == 0) && recipient_present {
            return Err(invalid_result(
                "nft_issuer_transfer_fee_recipient",
                "expected recipient only when nft_issuer_transfer_fee is nonzero",
            ));
        }
    } else if receipt.get("nft_issuer_transfer_fee_recipient").is_some() {
        return Err(invalid_result(
            "nft_issuer_transfer_fee_recipient",
            "expected nft_issuer_transfer_fee when recipient is present",
        ));
    }
    if receipt.get("nft_collection_flags").is_some() {
        let Some(collection_flags) = optional_u64_field_value(receipt, "nft_collection_flags")?
        else {
            return Err(invalid_result(
                "nft_collection_flags",
                "expected non-null collection flags when present",
            ));
        };
        if collection_flags > u32::MAX as u64 {
            return Err(invalid_result(
                "nft_collection_flags",
                "expected u32-compatible collection flags",
            ));
        }
    }
    optional_lower_hex_len_field_value(receipt, "offer_id", OFFER_ID_HEX_LEN)?;
    if let Some(fills) = receipt.get("offer_fills") {
        let fills = fills
            .as_array()
            .ok_or_else(|| invalid_result("offer_fills", "expected array value"))?;
        for fill in fills {
            u64_field(fill, "fill_index")?;
            lower_hex_field(fill, "maker_offer_id", OFFER_ID_HEX_LEN)?;
            clean_string_field(fill, "maker_owner")?;
            clean_string_field(fill, "taker")?;
            clean_string_field(fill, "maker_sends_asset_id")?;
            nonzero_u64_field(fill, "maker_sends_amount")?;
            clean_string_field(fill, "taker_sends_asset_id")?;
            nonzero_u64_field(fill, "taker_sends_amount")?;
            u64_field(fill, "maker_taker_gets_remaining")?;
            u64_field(fill, "maker_taker_pays_remaining")?;
            optional_clean_string_result_field(fill, "terminal_maker_state")?;
        }
    }
    validate_atomic_swap_leg_receipts(receipt)?;
    Ok(())
}

fn validate_tx_finality_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    validate_tx_finality_result_with_schema(result, "postfiat-tx-finality-v1")
}

fn validate_tx_finality_result_with_schema(
    result: &Value,
    schema: &str,
) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", schema)?;
    lower_hex_field(result, "proof_id", 96)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    lower_hex_field(result, "tx_id", 96)?;
    let tx_id = string_field(result, "tx_id")?;
    expect_bool_eq(result, "confirmed", true)?;
    let block_log_verified = bool_field(result, "block_log_verified")?;
    let verification_mode = result
        .get("verification_mode")
        .and_then(Value::as_str)
        .unwrap_or("full-block-replay");
    if block_log_verified {
        if verification_mode != "full-block-replay" {
            return Err(invalid_result(
                "verification_mode",
                "expected full-block-replay when block_log_verified is true",
            ));
        }
    } else if verification_mode != "selected-block-hot-path" {
        return Err(invalid_result(
            "verification_mode",
            "expected selected-block-hot-path when block_log_verified is false",
        ));
    }
    let block_count = nonzero_u64_field(result, "block_count")?;
    validate_block_tip_hash(block_count, string_field(result, "tip_hash")?, "tip_hash")?;
    lower_hex_field(result, "tip_state_root", 96)?;

    let receipt = field(result, "receipt")?;
    validate_receipt_fields(receipt)?;
    if string_field(receipt, "tx_id")? != tx_id {
        return Err(invalid_result(
            "receipt.tx_id",
            "expected receipt tx_id to match finality tx_id",
        ));
    }

    let receipt_index = u64_field(result, "receipt_index")?;
    let receipt_count = nonzero_u64_field(result, "receipt_count")?;
    let block = field(result, "block")?;
    validate_blocks_result(&Value::Array(vec![block.clone()]))?;
    let block_height = nonzero_u64_field(block, "header.height")?;
    if block_height > block_count {
        return Err(invalid_result(
            "block_count",
            "expected block_count to be at least finality block height",
        ));
    }
    let block_receipt_count = u64_field(block, "header.receipt_count")?;
    if block_receipt_count != receipt_count {
        return Err(invalid_result(
            "receipt_count",
            "expected receipt_count to match block header receipt_count",
        ));
    }
    let receipt_ids = array_field(block, "receipt_ids")?;
    let Some(receipt_id) = receipt_ids.get(receipt_index as usize) else {
        return Err(invalid_result(
            "receipt_index",
            "expected receipt_index to point into block receipt_ids",
        ));
    };
    if clean_string_entry(receipt_id, "receipt_ids")? != tx_id {
        return Err(invalid_result(
            "receipt_index",
            "expected indexed block receipt_id to match finality tx_id",
        ));
    }
    Ok(())
}

fn validate_blocks_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    let blocks = result_array(result, "result")?;
    let mut previous_height = None;
    let mut previous_block_hash = None;
    for block in blocks {
        let height = nonzero_u64_field(block, "header.height")?;
        validate_block_parent_hash(block, height, previous_height, previous_block_hash)?;
        let batch_kind = nonempty_string_field(block, "header.batch_kind")?;
        if !is_supported_batch_kind(batch_kind) {
            return Err(invalid_result(
                "header.batch_kind",
                format!("unsupported batch kind `{batch_kind}`"),
            ));
        }
        lower_hex_field(block, "header.batch_id", 96)?;
        lower_hex_field(block, "header.block_hash", 96)?;
        lower_hex_field(block, "header.state_root", 96)?;
        let receipt_count = u64_field(block, "header.receipt_count")?;
        let receipt_ids = field(block, "receipt_ids")?
            .as_array()
            .ok_or_else(|| invalid_result("receipt_ids", "expected array value"))?;
        if receipt_count != receipt_ids.len() as u64 {
            return Err(invalid_result(
                "header.receipt_count",
                format!(
                    "expected receipt_ids length {}, found {receipt_count}",
                    receipt_ids.len()
                ),
            ));
        }
        for receipt_id in receipt_ids {
            clean_string_entry(receipt_id, "receipt_ids")?;
        }
        lower_hex_field(block, "header.certificate_id", 96)?;
        validate_block_certificate_fields(field(block, "header.certificate")?)?;
        previous_height = Some(height);
        previous_block_hash = Some(string_field(block, "header.block_hash")?);
    }
    Ok(())
}

fn validate_block_parent_hash(
    block: &Value,
    height: u64,
    previous_height: Option<u64>,
    previous_block_hash: Option<&str>,
) -> Result<(), RpcResponseValidationError> {
    let parent_hash = string_field(block, "header.parent_hash")?;
    if height == 1 {
        if parent_hash != "genesis" {
            return Err(invalid_result(
                "header.parent_hash",
                format!("expected `genesis`, found `{parent_hash}`"),
            ));
        }
    } else if !is_lower_hex_len(parent_hash, 96) {
        return Err(invalid_result(
            "header.parent_hash",
            "expected `genesis` for height 1 or 96 lowercase hex characters",
        ));
    }

    if let Some(previous_height) = previous_height {
        let expected_height = previous_height.checked_add(1).ok_or_else(|| {
            invalid_result(
                "header.height",
                "previous returned block height is exhausted",
            )
        })?;
        if height != expected_height {
            return Err(invalid_result(
                "header.height",
                format!("expected adjacent block height {expected_height}, found {height}"),
            ));
        }
        let Some(previous_block_hash) = previous_block_hash else {
            return Err(invalid_result(
                "header.parent_hash",
                "missing previous block hash while validating adjacent block linkage",
            ));
        };
        if parent_hash != previous_block_hash {
            return Err(invalid_result(
                "header.parent_hash",
                "expected parent_hash to match previous returned block_hash",
            ));
        }
    }
    Ok(())
}

fn validate_block_certificate_fields(
    certificate: &Value,
) -> Result<(), RpcResponseValidationError> {
    let quorum = nonzero_u64_field(certificate, "quorum")?;
    lower_hex_field(certificate, "registry_root", 96)?;
    let registry_root = string_field(certificate, "registry_root")?;
    let validators = array_field(certificate, "validators")?;
    if (validators.len() as u64) < quorum {
        return Err(invalid_result(
            "validators",
            format!(
                "expected at least quorum {quorum} validators, found {}",
                validators.len()
            ),
        ));
    }
    let mut validator_ids = Vec::with_capacity(validators.len());
    for validator in validators {
        let validator = clean_string_entry(validator, "validators")?;
        if validator_ids.contains(&validator) {
            return Err(invalid_result(
                "validators",
                format!("duplicate validator `{validator}`"),
            ));
        }
        validator_ids.push(validator);
    }

    let votes = array_field(certificate, "votes")?;
    if (votes.len() as u64) < quorum || votes.len() > validators.len() {
        return Err(invalid_result(
            "votes",
            format!(
                "expected vote count between quorum {quorum} and validator count {}, found {}",
                validators.len(),
                votes.len()
            ),
        ));
    }
    let mut vote_ids = Vec::with_capacity(votes.len());
    let mut vote_validators = Vec::with_capacity(votes.len());
    for vote in votes {
        lower_hex_field(vote, "vote_id", 96)?;
        let vote_id = string_field(vote, "vote_id")?;
        if vote_ids.contains(&vote_id) {
            return Err(invalid_result(
                "vote_id",
                format!("duplicate vote id `{vote_id}`"),
            ));
        }
        vote_ids.push(vote_id);

        let validator = clean_string_field(vote, "validator")?;
        if !validator_ids.contains(&validator) {
            return Err(invalid_result(
                "validator",
                format!("vote validator `{validator}` is not in certificate validators"),
            ));
        }
        if vote_validators.contains(&validator) {
            return Err(invalid_result(
                "validator",
                format!("duplicate vote validator `{validator}`"),
            ));
        }
        vote_validators.push(validator);

        expect_bool_eq(vote, "accept", true)?;
        expect_string_eq(vote, "algorithm_id", ML_DSA_65_ALGORITHM)?;
        lower_hex_field(vote, "registry_root", 96)?;
        if string_field(vote, "registry_root")? != registry_root {
            return Err(invalid_result(
                "registry_root",
                "expected vote registry_root to match certificate registry_root",
            ));
        }
        optional_empty_string_field(vote, "public_key_hex")?;
        lower_hex_string_field(vote, "signature_hex")?;
    }
    Ok(())
}

fn validate_batch_archive_result(
    result: &Value,
    context: Option<&BatchArchiveValidationContext>,
) -> Result<(), RpcResponseValidationError> {
    if let Some(context) = context {
        validate_batch_archive_context(context)?;
    }
    let archive = result_array(result, "result")?;
    for entry in archive {
        let batch_kind = nonempty_string_field(entry, "batch_kind")?;
        lower_hex_field(entry, "batch_id", 96)?;
        let batch_id = string_field(entry, "batch_id")?;
        lower_hex_field(entry, "payload_hash", 96)?;
        let payload_hash = string_field(entry, "payload_hash")?;
        let payload_json = nonempty_string_field(entry, "payload_json")?;
        if payload_json.len() > MAX_RPC_BATCH_ARCHIVE_PAYLOAD_BYTES {
            return Err(invalid_result(
                "payload_json",
                format!("expected at most {MAX_RPC_BATCH_ARCHIVE_PAYLOAD_BYTES} bytes"),
            ));
        }
        let payload = serde_json::from_str::<Value>(payload_json)
            .map_err(|error| invalid_result("payload_json", format!("invalid JSON: {error}")))?;
        validate_batch_archive_payload(batch_kind, batch_id, &payload)?;
        if let Some(context) = context {
            let expected = batch_archive_payload_hash(context, batch_kind, batch_id, payload_json)?;
            if payload_hash != expected {
                return Err(invalid_result(
                    "payload_hash",
                    format!("expected `{expected}`, found `{payload_hash}`"),
                ));
            }
        }
    }
    Ok(())
}

fn validate_archive_window_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", HISTORY_ARCHIVE_WINDOW_SCHEMA)?;
    lower_hex_field(result, "bundle_hash", 96)?;
    let proof = field(result, "proof")?;
    validate_archive_handoff_proof_result(proof)?;

    let blocks = array_field(result, "blocks")?;
    let batches = array_field(result, "batches")?;
    let receipts = array_field(result, "receipts")?;
    let proof_block_count = u64_field(proof, "block_count")?;
    let proof_batch_count = u64_field(proof, "batch_count")?;
    let proof_receipt_count = u64_field(proof, "receipt_count")?;
    if proof_block_count != blocks.len() as u64 {
        return Err(invalid_result(
            "proof.block_count",
            format!(
                "expected blocks length {}, found {proof_block_count}",
                blocks.len()
            ),
        ));
    }
    if proof_batch_count != batches.len() as u64 {
        return Err(invalid_result(
            "proof.batch_count",
            format!(
                "expected batches length {}, found {proof_batch_count}",
                batches.len()
            ),
        ));
    }
    if proof_receipt_count != receipts.len() as u64 {
        return Err(invalid_result(
            "proof.receipt_count",
            format!(
                "expected receipts length {}, found {proof_receipt_count}",
                receipts.len()
            ),
        ));
    }

    validate_blocks_result(&Value::Array(blocks.clone()))?;
    validate_batch_archive_result(&Value::Array(batches.clone()), None)?;
    validate_receipts_result(&Value::Array(receipts.clone()))?;
    if let Some(first_block) = blocks.first() {
        let first_hash = string_field(first_block, "header.block_hash")?;
        if first_hash != string_field(proof, "first_block_hash")? {
            return Err(invalid_result(
                "proof.first_block_hash",
                "expected first block hash to match bundled block",
            ));
        }
    }
    if let Some(last_block) = blocks.last() {
        let last_hash = string_field(last_block, "header.block_hash")?;
        if last_hash != string_field(proof, "last_block_hash")? {
            return Err(invalid_result(
                "proof.last_block_hash",
                "expected last block hash to match bundled block",
            ));
        }
    }
    Ok(())
}

fn validate_archive_handoff_proof_result(proof: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(proof, "schema", HISTORY_ARCHIVE_HANDOFF_SCHEMA)?;
    clean_string_field(proof, "chain_id")?;
    lower_hex_field(proof, "genesis_hash", 96)?;
    nonzero_u32_field(proof, "protocol_version")?;
    let archive_uri = string_field(proof, "archive_uri")?;
    if archive_uri != archive_uri.trim() || archive_uri.chars().any(char::is_control) {
        return Err(invalid_result(
            "archive_uri",
            "expected archive URI without leading, trailing, or control whitespace",
        ));
    }
    let from_height = nonzero_u64_field(proof, "from_height")?;
    let to_height = nonzero_u64_field(proof, "to_height")?;
    if to_height < from_height {
        return Err(invalid_result(
            "to_height",
            "expected to_height to be greater than or equal to from_height",
        ));
    }
    let expected_block_count = to_height
        .checked_sub(from_height)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| invalid_result("to_height", "archive window overflow"))?;
    let block_count = nonzero_u64_field(proof, "block_count")?;
    if block_count != expected_block_count {
        return Err(invalid_result(
            "block_count",
            format!(
                "expected archive window block count {expected_block_count}, found {block_count}"
            ),
        ));
    }
    u64_field(proof, "batch_count")?;
    u64_field(proof, "receipt_count")?;
    validate_block_tip_hash(
        block_count,
        string_field(proof, "first_block_hash")?,
        "first_block_hash",
    )?;
    validate_block_tip_hash(
        block_count,
        string_field(proof, "last_block_hash")?,
        "last_block_hash",
    )?;
    lower_hex_field(proof, "block_range_root", 96)?;
    lower_hex_field(proof, "batch_payload_root", 96)?;
    lower_hex_field(proof, "receipt_root", 96)?;
    lower_hex_field(proof, "proof_hash", 96)?;
    Ok(())
}

fn validate_batch_archive_context(
    context: &BatchArchiveValidationContext,
) -> Result<(), RpcResponseValidationError> {
    if context.chain_id.trim().is_empty() {
        return Err(invalid_result(
            "chain_id",
            "expected nonempty archive validation chain id",
        ));
    }
    if !is_lower_hex_len(&context.genesis_hash, 96) {
        return Err(invalid_result(
            "genesis_hash",
            "expected 96 lowercase hex characters",
        ));
    }
    Ok(())
}

fn batch_archive_payload_hash(
    context: &BatchArchiveValidationContext,
    batch_kind: &str,
    batch_id: &str,
    payload_json: &str,
) -> Result<String, RpcResponseValidationError> {
    let encoded = serde_json::to_vec(&(
        context.chain_id.as_str(),
        context.genesis_hash.as_str(),
        context.protocol_version,
        batch_kind,
        batch_id,
        payload_json,
    ))
    .map_err(|error| invalid_result("payload_hash", format!("hash preimage failed: {error}")))?;
    Ok(hash_hex(BATCH_ARCHIVE_PAYLOAD_HASH_DOMAIN, &encoded))
}

fn hash_hex(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha3_384::new();
    hasher.update(domain.as_bytes());
    hasher.update([0u8]);
    hasher.update(bytes);
    bytes_to_hex(&hasher.finalize())
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn validate_batch_archive_payload(
    batch_kind: &str,
    batch_id: &str,
    payload: &Value,
) -> Result<(), RpcResponseValidationError> {
    if contains_private_key_material_field(payload) {
        return Err(invalid_result(
            "payload_json",
            "archive payload contains private key material fields",
        ));
    }
    let payload_batch_id = nonempty_string_field(payload, "batch_id")?;
    if payload_batch_id != batch_id {
        return Err(invalid_result(
            "payload_json.batch_id",
            format!("expected `{batch_id}`, found `{payload_batch_id}`"),
        ));
    }
    match batch_kind {
        "transparent" => validate_transparent_archive_payload(payload),
        "governance" => validate_governance_archive_payload(payload),
        "shielded" => validate_shielded_archive_payload(payload),
        "bridge" => validate_bridge_archive_payload(payload),
        other => Err(invalid_result(
            "batch_kind",
            format!("unsupported archived batch kind `{other}`"),
        )),
    }
}

fn validate_transparent_archive_payload(payload: &Value) -> Result<(), RpcResponseValidationError> {
    let transactions = array_field(payload, "transactions")?;
    let atomic_swap_transactions = payload
        .get("atomic_swap_transactions")
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| invalid_result("atomic_swap_transactions", "expected array value"))
        })
        .transpose()?;
    if transactions.is_empty() && atomic_swap_transactions.is_none_or(Vec::is_empty) {
        return Err(invalid_result(
            "transactions",
            "expected at least one transparent transfer or atomic swap transaction",
        ));
    }
    for transaction in transactions {
        validate_signed_transfer_fields(transaction)?;
    }
    if let Some(atomic_swap_transactions) = atomic_swap_transactions {
        for transaction in atomic_swap_transactions {
            validate_signed_atomic_swap_transaction_fields(transaction)?;
        }
    }
    Ok(())
}

fn validate_governance_archive_payload(payload: &Value) -> Result<(), RpcResponseValidationError> {
    let amendments = nonempty_array_field(payload, "amendments", "governance amendment")?;
    for amendment in amendments {
        validate_governance_amendment_fields(amendment)?;
    }
    Ok(())
}

fn validate_shielded_archive_payload(payload: &Value) -> Result<(), RpcResponseValidationError> {
    let actions = nonempty_array_field(payload, "actions", "shielded action")?;
    for action in actions {
        match string_field(action, "kind")? {
            "shield_mint" => validate_shield_mint_action(action)?,
            "shield_spend" => validate_shield_spend_action(action)?,
            "shield_migrate" => validate_shield_migrate_action(action)?,
            "orchard_action_v1" => validate_orchard_action_payload(action)?,
            "orchard_deposit_v1" => validate_orchard_deposit_action_payload(action)?,
            "orchard_withdraw_v1" => validate_orchard_withdraw_action_payload(action)?,
            "shielded_swap_v1" => validate_shielded_swap_action_payload(action)?,
            other => {
                return Err(invalid_result(
                    "kind",
                    format!("unsupported shielded action kind `{other}`"),
                ))
            }
        }
    }
    Ok(())
}

fn validate_bridge_archive_payload(payload: &Value) -> Result<(), RpcResponseValidationError> {
    let actions = nonempty_array_field(payload, "actions", "bridge action")?;
    for action in actions {
        match string_field(action, "kind")? {
            "bridge_domain" => validate_bridge_domain_action(action)?,
            "bridge_transfer" => validate_bridge_transfer_action(action)?,
            "bridge_pause" => validate_bridge_pause_action(action, None)?,
            other => {
                return Err(invalid_result(
                    "kind",
                    format!("unsupported bridge action kind `{other}`"),
                ))
            }
        }
    }
    Ok(())
}

fn validate_governance_amendment_fields(
    amendment: &Value,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(amendment, "amendment_id", 96)?;
    nonempty_string_field(amendment, "chain_id")?;
    lower_hex_field(amendment, "genesis_hash", 96)?;
    u64_field(amendment, "protocol_version")?;
    lower_hex_field(amendment, "instance_id", 96)?;
    lower_hex_field(amendment, "proposal_id", 96)?;
    lower_hex_field(amendment, "certificate_id", 96)?;
    nonempty_string_field(amendment, "proposer")?;
    nonempty_string_array_field(amendment, "validators")?;
    nonzero_u64_field(amendment, "quorum")?;
    validate_governance_kind(amendment)?;
    nonzero_u64_field(amendment, "value")?;
    nonempty_string_array_field(amendment, "support")?;
    let votes = nonempty_array_field(amendment, "votes", "governance vote")?;
    for vote in votes {
        lower_hex_field(vote, "vote_id", 96)?;
        nonempty_string_field(vote, "validator")?;
        bool_field(vote, "accept")?;
    }
    Ok(())
}

fn validate_governance_kind(value: &Value) -> Result<(), RpcResponseValidationError> {
    let kind = string_field(value, "kind")?;
    if !matches!(
        kind,
        "validator_set" | "crypto_policy" | "bridge_witness_epoch" | "authority_mode"
    ) {
        return Err(invalid_result(
            "kind",
            format!("unsupported governance amendment kind `{kind}`"),
        ));
    }
    Ok(())
}

fn nonempty_string_array_field(
    value: &Value,
    path: &str,
) -> Result<(), RpcResponseValidationError> {
    let entries = nonempty_array_field(value, path, "string entry")?;
    for entry in entries {
        let entry = entry
            .as_str()
            .ok_or_else(|| invalid_result(path, "expected string entries"))?;
        if entry.trim().is_empty() {
            return Err(invalid_result(path, "expected nonempty string entries"));
        }
    }
    Ok(())
}

fn nonempty_array_field<'a>(
    value: &'a Value,
    path: &str,
    item_name: &str,
) -> Result<&'a Vec<Value>, RpcResponseValidationError> {
    let entries = array_field(value, path)?;
    if entries.is_empty() {
        return Err(invalid_result(
            path,
            format!("expected at least one archived payload {item_name}"),
        ));
    }
    Ok(entries)
}

fn is_supported_batch_kind(batch_kind: &str) -> bool {
    matches!(
        batch_kind,
        "transparent" | "governance" | "shielded" | "bridge"
    )
}

fn validate_mempool_entry_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(result, "tx_id", 96)?;
    validate_signed_transfer_fields(field(result, "transfer")?)?;
    Ok(())
}

fn validate_mempool_payment_v2_entry_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(result, "tx_id", 96)?;
    validate_signed_payment_v2_fields(field(result, "payment")?)?;
    Ok(())
}

fn validate_mempool_asset_transaction_entry_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(result, "tx_id", 96)?;
    validate_signed_asset_transaction_fields(field(result, "transaction")?)?;
    Ok(())
}

fn validate_mempool_escrow_transaction_entry_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(result, "tx_id", 96)?;
    validate_signed_escrow_transaction_fields(field(result, "transaction")?)?;
    Ok(())
}

fn validate_mempool_nft_transaction_entry_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(result, "tx_id", 96)?;
    validate_signed_nft_transaction_fields(field(result, "transaction")?)?;
    Ok(())
}

fn validate_mempool_offer_transaction_entry_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(result, "tx_id", 96)?;
    validate_signed_offer_transaction_fields(field(result, "transaction")?)?;
    Ok(())
}

fn validate_mempool_status_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    for entry in array_field(result, "pending")? {
        validate_mempool_entry_result(entry)?;
    }
    if let Some(payments_v2) = result.get("pending_payment_v2") {
        let payments_v2 = payments_v2
            .as_array()
            .ok_or_else(|| invalid_result("pending_payment_v2", "expected array value"))?;
        for entry in payments_v2 {
            validate_mempool_payment_v2_entry_result(entry)?;
        }
    }
    if let Some(asset_transactions) = result.get("pending_asset_transactions") {
        let asset_transactions = asset_transactions
            .as_array()
            .ok_or_else(|| invalid_result("pending_asset_transactions", "expected array value"))?;
        for entry in asset_transactions {
            validate_mempool_asset_transaction_entry_result(entry)?;
        }
    }
    if let Some(escrow_transactions) = result.get("pending_escrow_transactions") {
        let escrow_transactions = escrow_transactions
            .as_array()
            .ok_or_else(|| invalid_result("pending_escrow_transactions", "expected array value"))?;
        for entry in escrow_transactions {
            validate_mempool_escrow_transaction_entry_result(entry)?;
        }
    }
    if let Some(nft_transactions) = result.get("pending_nft_transactions") {
        let nft_transactions = nft_transactions
            .as_array()
            .ok_or_else(|| invalid_result("pending_nft_transactions", "expected array value"))?;
        for entry in nft_transactions {
            validate_mempool_nft_transaction_entry_result(entry)?;
        }
    }
    if let Some(offer_transactions) = result.get("pending_offer_transactions") {
        let offer_transactions = offer_transactions
            .as_array()
            .ok_or_else(|| invalid_result("pending_offer_transactions", "expected array value"))?;
        for entry in offer_transactions {
            validate_mempool_offer_transaction_entry_result(entry)?;
        }
    }
    if let Some(atomic_swaps) = result.get("pending_atomic_swaps") {
        let atomic_swaps = atomic_swaps
            .as_array()
            .ok_or_else(|| invalid_result("pending_atomic_swaps", "expected array value"))?;
        for entry in atomic_swaps {
            validate_mempool_atomic_swap_entry_result(entry)?;
        }
    }
    Ok(())
}

fn validate_mempool_batch_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(result, "batch_id", 96)?;
    let transactions = array_field(result, "transactions")?;
    let payments_v2 = result
        .get("payments_v2")
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| invalid_result("payments_v2", "expected array value"))
        })
        .transpose()?;
    let payment_v2_count = payments_v2.map_or(0, Vec::len);
    let asset_transactions = result
        .get("asset_transactions")
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| invalid_result("asset_transactions", "expected array value"))
        })
        .transpose()?;
    let asset_transaction_count = asset_transactions.map_or(0, Vec::len);
    let escrow_transactions = result
        .get("escrow_transactions")
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| invalid_result("escrow_transactions", "expected array value"))
        })
        .transpose()?;
    let escrow_transaction_count = escrow_transactions.map_or(0, Vec::len);
    let nft_transactions = result
        .get("nft_transactions")
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| invalid_result("nft_transactions", "expected array value"))
        })
        .transpose()?;
    let nft_transaction_count = nft_transactions.map_or(0, Vec::len);
    let offer_transactions = result
        .get("offer_transactions")
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| invalid_result("offer_transactions", "expected array value"))
        })
        .transpose()?;
    let offer_transaction_count = offer_transactions.map_or(0, Vec::len);
    let atomic_swap_transactions = result
        .get("atomic_swap_transactions")
        .map(|value| {
            value
                .as_array()
                .ok_or_else(|| invalid_result("atomic_swap_transactions", "expected array value"))
        })
        .transpose()?;
    let atomic_swap_transaction_count = atomic_swap_transactions.map_or(0, Vec::len);
    if transactions.is_empty()
        && payment_v2_count == 0
        && asset_transaction_count == 0
        && escrow_transaction_count == 0
        && nft_transaction_count == 0
        && offer_transaction_count == 0
        && atomic_swap_transaction_count == 0
    {
        return Err(invalid_result(
            "transactions",
            "expected at least one transparent transfer, payment v2, asset transaction, atomic swap, escrow transaction, nft transaction, or offer transaction",
        ));
    }
    for transaction in transactions {
        validate_signed_transfer_fields(transaction)?;
    }
    if let Some(payments_v2) = payments_v2 {
        for payment in payments_v2 {
            validate_signed_payment_v2_fields(payment)?;
        }
    }
    if let Some(asset_transactions) = asset_transactions {
        for transaction in asset_transactions {
            validate_signed_asset_transaction_fields(transaction)?;
        }
    }
    if let Some(escrow_transactions) = escrow_transactions {
        for transaction in escrow_transactions {
            validate_signed_escrow_transaction_fields(transaction)?;
        }
    }
    if let Some(nft_transactions) = nft_transactions {
        for transaction in nft_transactions {
            validate_signed_nft_transaction_fields(transaction)?;
        }
    }
    if let Some(offer_transactions) = offer_transactions {
        for transaction in offer_transactions {
            validate_signed_offer_transaction_fields(transaction)?;
        }
    }
    if let Some(atomic_swap_transactions) = atomic_swap_transactions {
        for transaction in atomic_swap_transactions {
            validate_signed_atomic_swap_transaction_fields(transaction)?;
        }
    }
    Ok(())
}

fn validate_signed_transfer_fields(transfer: &Value) -> Result<(), RpcResponseValidationError> {
    validate_unsigned_transfer_fields(field(transfer, "unsigned")?)?;
    expect_string_eq(transfer, "algorithm_id", ML_DSA_65_ALGORITHM)?;
    lower_hex_string_field(transfer, "public_key_hex")?;
    lower_hex_string_field(transfer, "signature_hex")?;
    Ok(())
}

fn validate_unsigned_transfer_fields(unsigned: &Value) -> Result<(), RpcResponseValidationError> {
    clean_string_field(unsigned, "chain_id")?;
    lower_hex_field(unsigned, "genesis_hash", 96)?;
    nonzero_u32_field(unsigned, "protocol_version")?;
    expect_string_eq(unsigned, "address_namespace", TRANSPARENT_ADDRESS_NAMESPACE)?;
    expect_string_eq(unsigned, "transaction_kind", TRANSPARENT_TRANSFER_KIND)?;
    expect_string_eq(unsigned, "signature_algorithm_id", ML_DSA_65_ALGORITHM)?;
    clean_string_field(unsigned, "from")?;
    clean_string_field(unsigned, "to")?;
    nonzero_u64_field(unsigned, "amount")?;
    nonzero_u64_field(unsigned, "fee")?;
    u64_field(unsigned, "sequence")?;
    Ok(())
}

fn validate_signed_payment_v2_fields(payment: &Value) -> Result<(), RpcResponseValidationError> {
    validate_unsigned_payment_v2_fields(field(payment, "unsigned")?)?;
    expect_string_eq(payment, "algorithm_id", ML_DSA_65_ALGORITHM)?;
    lower_hex_string_field(payment, "public_key_hex")?;
    lower_hex_string_field(payment, "signature_hex")?;
    Ok(())
}

fn validate_unsigned_payment_v2_fields(unsigned: &Value) -> Result<(), RpcResponseValidationError> {
    clean_string_field(unsigned, "chain_id")?;
    lower_hex_field(unsigned, "genesis_hash", 96)?;
    nonzero_u32_field(unsigned, "protocol_version")?;
    expect_string_eq(unsigned, "address_namespace", TRANSPARENT_ADDRESS_NAMESPACE)?;
    expect_string_eq(unsigned, "transaction_kind", PAYMENT_V2_TRANSACTION_KIND)?;
    expect_string_eq(unsigned, "signature_algorithm_id", ML_DSA_65_ALGORITHM)?;
    clean_string_field(unsigned, "from")?;
    clean_string_field(unsigned, "to")?;
    nonzero_u64_field(unsigned, "amount")?;
    nonzero_u64_field(unsigned, "fee")?;
    u64_field(unsigned, "sequence")?;
    for memo in array_field(unsigned, "memos")? {
        validate_payment_v2_memo_fields(memo)?;
    }
    Ok(())
}

fn validate_payment_v2_memo_fields(memo: &Value) -> Result<(), RpcResponseValidationError> {
    let memo_type = string_field(memo, "memo_type")?;
    let memo_format = string_field(memo, "memo_format")?;
    let memo_data = string_field(memo, "memo_data")?;
    if memo_type.is_empty() && memo_format.is_empty() && memo_data.is_empty() {
        return Err(invalid_result(
            "memo",
            "expected at least one payment memo field",
        ));
    }
    for (path, value) in [
        ("memo_type", memo_type),
        ("memo_format", memo_format),
        ("memo_data", memo_data),
    ] {
        if !value.is_empty() && !is_lower_hex_string(value) {
            return Err(invalid_result(
                path,
                "expected empty or lowercase hex string with even length",
            ));
        }
    }
    Ok(())
}

fn validate_signed_asset_transaction_fields(
    transaction: &Value,
) -> Result<(), RpcResponseValidationError> {
    validate_unsigned_asset_transaction_fields(field(transaction, "unsigned")?)?;
    expect_string_eq(transaction, "algorithm_id", ML_DSA_65_ALGORITHM)?;
    lower_hex_string_field(transaction, "public_key_hex")?;
    lower_hex_string_field(transaction, "signature_hex")?;
    Ok(())
}

fn validate_unsigned_asset_transaction_fields(
    unsigned: &Value,
) -> Result<(), RpcResponseValidationError> {
    clean_string_field(unsigned, "chain_id")?;
    lower_hex_field(unsigned, "genesis_hash", 96)?;
    nonzero_u32_field(unsigned, "protocol_version")?;
    expect_string_eq(unsigned, "address_namespace", TRANSPARENT_ADDRESS_NAMESPACE)?;
    let transaction_kind = clean_string_field(unsigned, "transaction_kind")?;
    if !is_supported_asset_transaction_kind(transaction_kind) {
        return Err(invalid_result(
            "transaction_kind",
            "expected supported asset transaction kind",
        ));
    }
    expect_string_eq(unsigned, "signature_algorithm_id", ML_DSA_65_ALGORITHM)?;
    clean_string_field(unsigned, "source")?;
    nonzero_u64_field(unsigned, "fee")?;
    u64_field(unsigned, "sequence")?;
    validate_asset_operation_fields(unsigned, transaction_kind)?;
    Ok(())
}

fn validate_signed_escrow_transaction_fields(
    transaction: &Value,
) -> Result<(), RpcResponseValidationError> {
    validate_unsigned_escrow_transaction_fields(field(transaction, "unsigned")?)?;
    expect_string_eq(transaction, "algorithm_id", ML_DSA_65_ALGORITHM)?;
    lower_hex_string_field(transaction, "public_key_hex")?;
    lower_hex_string_field(transaction, "signature_hex")?;
    Ok(())
}

fn validate_unsigned_escrow_transaction_fields(
    unsigned: &Value,
) -> Result<(), RpcResponseValidationError> {
    clean_string_field(unsigned, "chain_id")?;
    lower_hex_field(unsigned, "genesis_hash", 96)?;
    nonzero_u32_field(unsigned, "protocol_version")?;
    expect_string_eq(unsigned, "address_namespace", TRANSPARENT_ADDRESS_NAMESPACE)?;
    let transaction_kind = clean_string_field(unsigned, "transaction_kind")?;
    if !is_supported_escrow_transaction_kind(transaction_kind) {
        return Err(invalid_result(
            "transaction_kind",
            "expected supported escrow transaction kind",
        ));
    }
    expect_string_eq(unsigned, "signature_algorithm_id", ML_DSA_65_ALGORITHM)?;
    clean_string_field(unsigned, "source")?;
    nonzero_u64_field(unsigned, "fee")?;
    u64_field(unsigned, "sequence")?;
    validate_escrow_operation_fields(unsigned, transaction_kind)?;
    Ok(())
}

fn validate_signed_nft_transaction_fields(
    transaction: &Value,
) -> Result<(), RpcResponseValidationError> {
    validate_unsigned_nft_transaction_fields(field(transaction, "unsigned")?)?;
    expect_string_eq(transaction, "algorithm_id", ML_DSA_65_ALGORITHM)?;
    lower_hex_string_field(transaction, "public_key_hex")?;
    lower_hex_string_field(transaction, "signature_hex")?;
    Ok(())
}

fn validate_unsigned_nft_transaction_fields(
    unsigned: &Value,
) -> Result<(), RpcResponseValidationError> {
    clean_string_field(unsigned, "chain_id")?;
    lower_hex_field(unsigned, "genesis_hash", 96)?;
    nonzero_u32_field(unsigned, "protocol_version")?;
    expect_string_eq(unsigned, "address_namespace", TRANSPARENT_ADDRESS_NAMESPACE)?;
    let transaction_kind = clean_string_field(unsigned, "transaction_kind")?;
    if !is_supported_nft_transaction_kind(transaction_kind) {
        return Err(invalid_result(
            "transaction_kind",
            "expected supported nft transaction kind",
        ));
    }
    expect_string_eq(unsigned, "signature_algorithm_id", ML_DSA_65_ALGORITHM)?;
    clean_string_field(unsigned, "source")?;
    nonzero_u64_field(unsigned, "fee")?;
    u64_field(unsigned, "sequence")?;
    validate_nft_operation_fields(unsigned, transaction_kind)?;
    Ok(())
}

fn validate_signed_offer_transaction_fields(
    transaction: &Value,
) -> Result<(), RpcResponseValidationError> {
    validate_unsigned_offer_transaction_fields(field(transaction, "unsigned")?)?;
    expect_string_eq(transaction, "algorithm_id", ML_DSA_65_ALGORITHM)?;
    lower_hex_string_field(transaction, "public_key_hex")?;
    lower_hex_string_field(transaction, "signature_hex")?;
    Ok(())
}

fn validate_unsigned_offer_transaction_fields(
    unsigned: &Value,
) -> Result<(), RpcResponseValidationError> {
    clean_string_field(unsigned, "chain_id")?;
    lower_hex_field(unsigned, "genesis_hash", 96)?;
    nonzero_u32_field(unsigned, "protocol_version")?;
    expect_string_eq(unsigned, "address_namespace", TRANSPARENT_ADDRESS_NAMESPACE)?;
    let transaction_kind = clean_string_field(unsigned, "transaction_kind")?;
    if !is_supported_offer_transaction_kind(transaction_kind) {
        return Err(invalid_result(
            "transaction_kind",
            "expected supported offer transaction kind",
        ));
    }
    expect_string_eq(unsigned, "signature_algorithm_id", ML_DSA_65_ALGORITHM)?;
    clean_string_field(unsigned, "source")?;
    nonzero_u64_field(unsigned, "fee")?;
    u64_field(unsigned, "sequence")?;
    validate_offer_operation_fields(unsigned, transaction_kind)?;
    Ok(())
}

fn validate_asset_operation_fields(
    value: &Value,
    expected_kind: &str,
) -> Result<(), RpcResponseValidationError> {
    let operation = clean_string_field(value, "operation")?;
    if operation != expected_kind {
        return Err(invalid_result(
            "operation",
            "expected operation to match transaction kind",
        ));
    }
    match operation {
        ASSET_CREATE_TRANSACTION_KIND => {
            clean_string_field(value, "issuer")?;
            clean_string_field(value, "code")?;
            nonzero_u32_field(value, "version")?;
            u64_field(value, "precision")?;
            if let Some(display_name) = value.get("display_name") {
                display_name
                    .as_str()
                    .ok_or_else(|| invalid_result("display_name", "expected string value"))?;
            }
            if value.get("max_supply").is_some() {
                optional_u64_result_field(value, "max_supply")?;
            }
            if value.get("requires_authorization").is_some() {
                bool_field(value, "requires_authorization")?;
            }
            if value.get("freeze_enabled").is_some() {
                bool_field(value, "freeze_enabled")?;
            }
            if value.get("clawback_enabled").is_some() {
                bool_field(value, "clawback_enabled")?;
            }
        }
        TRUST_SET_TRANSACTION_KIND => {
            clean_string_field(value, "account")?;
            clean_string_field(value, "issuer")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "limit")?;
            if value.get("authorized").is_some() {
                bool_field(value, "authorized")?;
            }
            if value.get("frozen").is_some() {
                bool_field(value, "frozen")?;
            }
            u64_field(value, "reserve_paid")?;
        }
        ISSUED_PAYMENT_TRANSACTION_KIND => {
            clean_string_field(value, "from")?;
            clean_string_field(value, "to")?;
            clean_string_field(value, "issuer")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "amount")?;
        }
        ASSET_BURN_TRANSACTION_KIND => {
            clean_string_field(value, "owner")?;
            clean_string_field(value, "issuer")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "amount")?;
        }
        ASSET_CLAWBACK_TRANSACTION_KIND => {
            clean_string_field(value, "owner")?;
            clean_string_field(value, "issuer")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "amount")?;
        }
        NAV_ASSET_REGISTER_TRANSACTION_KIND => {
            clean_string_field(value, "issuer")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            clean_string_field(value, "reserve_operator")?;
            clean_string_field(value, "proof_profile")?;
            clean_string_field(value, "valuation_unit")?;
            clean_string_field(value, "redemption_account")?;
        }
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND => {
            clean_string_field(value, "issuer")?;
            clean_string_field(value, "submitter")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "epoch")?;
            nonzero_u64_field(value, "nav_per_unit")?;
            u64_field(value, "circulating_supply")?;
            u64_field(value, "verified_net_assets")?;
            clean_string_field(value, "proof_profile")?;
            lower_hex_field(value, "source_root", 96)?;
            lower_hex_field(value, "attestor_root", 96)?;
            lower_hex_field(value, "reserve_packet_hash", 96)?;
        }
        NAV_RESERVE_CHALLENGE_TRANSACTION_KIND => {
            clean_string_field(value, "challenger")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "epoch")?;
            lower_hex_field(value, "reserve_packet_hash", 96)?;
            lower_hex_field(value, "challenge_hash", 96)?;
        }
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND => {
            clean_string_field(value, "issuer")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "epoch")?;
            lower_hex_field(value, "reserve_packet_hash", 96)?;
        }
        NAV_MINT_AT_NAV_TRANSACTION_KIND => {
            clean_string_field(value, "issuer")?;
            clean_string_field(value, "to")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "amount")?;
            nonzero_u64_field(value, "epoch")?;
            lower_hex_field(value, "reserve_packet_hash", 96)?;
        }
        NAV_REDEEM_AT_NAV_TRANSACTION_KIND => {
            clean_string_field(value, "owner")?;
            clean_string_field(value, "issuer")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "amount")?;
            nonzero_u64_field(value, "epoch")?;
            lower_hex_field(value, "reserve_packet_hash", 96)?;
        }
        NAV_HALT_TRANSACTION_KIND => {
            clean_string_field(value, "issuer")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            bool_field(value, "halted")?;
            clean_string_field(value, "reason")?;
        }
        NAV_PROFILE_REGISTER_TRANSACTION_KIND => {
            clean_string_field(value, "registrant")?;
            clean_string_field(value, "verifier_kind")?;
        }
        NAV_REDEEM_SETTLE_TRANSACTION_KIND => {
            clean_string_field(value, "issuer")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            lower_hex_field(value, "redemption_id", 96)?;
            lower_hex_field(value, "settlement_receipt_hash", 96)?;
        }
        NAV_RESERVE_ATTEST_TRANSACTION_KIND => {
            clean_string_field(value, "attestor")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "epoch")?;
            lower_hex_field(value, "reserve_packet_hash", 96)?;
            lower_hex_field(value, "observation_root", 96)?;
        }
        NAV_ATTESTOR_REGISTER_TRANSACTION_KIND => {
            clean_string_field(value, "attestor")?;
            clean_string_field(value, "domain")?;
        }
        PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND => {
            clean_string_field(value, "operator")?;
            clean_string_field(value, "route_id")?;
            lower_hex_field(value, "route_config_digest", 96)?;
            clean_string_field(value, "route_trust_class")?;
            lower_hex_field(value, "native_nav_asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            lower_hex_field(value, "settlement_asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            validate_evm_address_field(value, "handoff_controller")?;
            validate_evm_address_field(value, "settlement_adapter")?;
            validate_evm_address_field(value, "wrapped_navcoin_token")?;
            nonzero_u64_field(value, "ethereum_chain_id")?;
            nonzero_u64_field(value, "route_supply_cap_atoms")?;
            nonzero_u64_field(value, "packet_notional_cap_atoms")?;
            u64_field(value, "latest_finalized_nav_epoch")?;
            nonzero_u64_field(value, "return_finality_blocks")?;
        }
        PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND => {
            clean_string_field(value, "subscriber")?;
            clean_string_field(value, "route_id")?;
            lower_hex_field(value, "settlement_asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            lower_hex_field(value, "subscription_nonce", 64)?;
            nonzero_u64_field(value, "settlement_value_atoms")?;
            nonzero_u64_field(value, "nav_price_settlement_atoms_per_nav_atom")?;
            nonzero_u64_field(value, "pricing_nav_epoch")?;
            lower_hex_field(value, "pricing_reserve_packet_hash", 96)?;
        }
        PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND => {
            clean_string_field(value, "owner")?;
            clean_string_field(value, "route_id")?;
            lower_hex_field(value, "packet_hash", 96)?;
            lower_hex_field(value, "export_nonce", 64)?;
            validate_evm_address_field(value, "ethereum_recipient")?;
            nonzero_u64_field(value, "amount_atoms")?;
            nonzero_u64_field(value, "destination_deadline_seconds")?;
            nonzero_u64_field(value, "refund_delay_blocks")?;
        }
        PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND => {
            clean_string_field(value, "operator")?;
            clean_string_field(value, "route_id")?;
            lower_hex_field(value, "packet_hash", 96)?;
            lower_hex_field(value, "ethereum_consume_tx_hash", 64)?;
            nonzero_u64_field(value, "consumed_height")?;
            nonzero_u64_field(value, "finalized_height")?;
        }
        PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND => {
            clean_string_field(value, "operator")?;
            clean_string_field(value, "route_id")?;
            lower_hex_field(value, "packet_hash", 96)?;
            lower_hex_field(value, "non_consumption_proof_hash", 96)?;
        }
        PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND => {
            clean_string_field(value, "operator")?;
            clean_string_field(value, "route_id")?;
            lower_hex_field(value, "burn_event_hash", 64)?;
            nonzero_u64_field(value, "ethereum_chain_id")?;
            validate_evm_address_field(value, "bridge_controller")?;
            validate_evm_address_field(value, "wrapped_navcoin_token")?;
            lower_hex_field(value, "native_nav_asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            validate_evm_address_field(value, "ethereum_sender")?;
            clean_string_field(value, "pftl_recipient")?;
            nonzero_u64_field(value, "amount_atoms")?;
            lower_hex_field(value, "return_nonce", 64)?;
            nonzero_u64_field(value, "burn_height")?;
            nonzero_u64_field(value, "finalized_height")?;
        }
        _ => {
            return Err(invalid_result(
                "operation",
                "expected supported asset transaction kind",
            ))
        }
    }
    Ok(())
}

fn validate_escrow_operation_fields(
    value: &Value,
    expected_kind: &str,
) -> Result<(), RpcResponseValidationError> {
    let operation = clean_string_field(value, "operation")?;
    if operation != expected_kind {
        return Err(invalid_result(
            "operation",
            "expected operation to match transaction kind",
        ));
    }
    match operation {
        ESCROW_CREATE_TRANSACTION_KIND => {
            clean_string_field(value, "owner")?;
            clean_string_field(value, "recipient")?;
            clean_string_field(value, "asset_id")?;
            nonzero_u64_field(value, "amount")?;
            string_field(value, "condition")?;
            u64_field_default_zero(value, "finish_after")?;
            u64_field_default_zero(value, "cancel_after")?;
        }
        ESCROW_FINISH_TRANSACTION_KIND => {
            lower_hex_field(value, "escrow_id", ESCROW_ID_HEX_LEN)?;
            clean_string_field(value, "owner")?;
            clean_string_field(value, "recipient")?;
            string_field(value, "fulfillment")?;
        }
        ESCROW_CANCEL_TRANSACTION_KIND => {
            lower_hex_field(value, "escrow_id", ESCROW_ID_HEX_LEN)?;
            clean_string_field(value, "owner")?;
        }
        _ => {
            return Err(invalid_result(
                "operation",
                "expected supported escrow transaction kind",
            ))
        }
    }
    Ok(())
}

fn validate_nft_operation_fields(
    value: &Value,
    expected_kind: &str,
) -> Result<(), RpcResponseValidationError> {
    let operation = clean_string_field(value, "operation")?;
    if operation != expected_kind {
        return Err(invalid_result(
            "operation",
            "expected operation to match transaction kind",
        ));
    }
    match operation {
        NFT_MINT_TRANSACTION_KIND => {
            clean_string_field(value, "issuer")?;
            clean_string_field(value, "collection_id")?;
            nonzero_u64_field(value, "serial")?;
            clean_string_field(value, "owner")?;
            lower_hex_string_field(value, "metadata_hash")?;
            if let Some(metadata_uri) = value.get("metadata_uri") {
                metadata_uri
                    .as_str()
                    .ok_or_else(|| invalid_result("metadata_uri", "expected string value"))?;
            }
            if value.get("flags").is_some() {
                u64_field(value, "flags")?;
            }
            if value.get("collection_flags").is_some() {
                u64_field(value, "collection_flags")?;
            }
            if value.get("issuer_transfer_fee").is_some() {
                u64_field(value, "issuer_transfer_fee")?;
            }
        }
        NFT_TRANSFER_TRANSACTION_KIND => {
            lower_hex_field(value, "nft_id", NFT_ID_HEX_LEN)?;
            clean_string_field(value, "from")?;
            clean_string_field(value, "to")?;
            if value.get("issuer").is_some() {
                optional_clean_string_result_field(value, "issuer")?;
            }
            if value.get("issuer_transfer_fee").is_some() {
                u64_field(value, "issuer_transfer_fee")?;
            }
        }
        NFT_BURN_TRANSACTION_KIND => {
            lower_hex_field(value, "nft_id", NFT_ID_HEX_LEN)?;
            clean_string_field(value, "owner")?;
        }
        _ => {
            return Err(invalid_result(
                "operation",
                "expected supported nft transaction kind",
            ))
        }
    }
    Ok(())
}

fn validate_offer_operation_fields(
    value: &Value,
    expected_kind: &str,
) -> Result<(), RpcResponseValidationError> {
    let operation = clean_string_field(value, "operation")?;
    if operation != expected_kind {
        return Err(invalid_result(
            "operation",
            "expected operation to match transaction kind",
        ));
    }
    match operation {
        OFFER_CREATE_TRANSACTION_KIND => {
            clean_string_field(value, "owner")?;
            clean_string_field(value, "taker_gets_asset_id")?;
            nonzero_u64_field(value, "taker_gets_amount")?;
            clean_string_field(value, "taker_pays_asset_id")?;
            nonzero_u64_field(value, "taker_pays_amount")?;
            u64_field(value, "expiration_height")?;
        }
        OFFER_CANCEL_TRANSACTION_KIND => {
            lower_hex_field(value, "offer_id", OFFER_ID_HEX_LEN)?;
            clean_string_field(value, "owner")?;
        }
        _ => {
            return Err(invalid_result(
                "operation",
                "expected supported offer transaction kind",
            ))
        }
    }
    Ok(())
}

fn is_supported_asset_transaction_kind(kind: &str) -> bool {
    matches!(
        kind,
        ASSET_CREATE_TRANSACTION_KIND
            | TRUST_SET_TRANSACTION_KIND
            | ISSUED_PAYMENT_TRANSACTION_KIND
            | ASSET_BURN_TRANSACTION_KIND
            | ASSET_CLAWBACK_TRANSACTION_KIND
            | NAV_ASSET_REGISTER_TRANSACTION_KIND
            | NAV_RESERVE_SUBMIT_TRANSACTION_KIND
            | NAV_RESERVE_CHALLENGE_TRANSACTION_KIND
            | NAV_EPOCH_FINALIZE_TRANSACTION_KIND
            | NAV_MINT_AT_NAV_TRANSACTION_KIND
            | NAV_REDEEM_AT_NAV_TRANSACTION_KIND
            | NAV_HALT_TRANSACTION_KIND
            | NAV_PROFILE_REGISTER_TRANSACTION_KIND
            | NAV_REDEEM_SETTLE_TRANSACTION_KIND
            | NAV_RESERVE_ATTEST_TRANSACTION_KIND
            | NAV_ATTESTOR_REGISTER_TRANSACTION_KIND
            | PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND
            | PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND
            | PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND
            | PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND
            | PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND
            | PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND
    )
}

fn is_supported_escrow_transaction_kind(kind: &str) -> bool {
    matches!(
        kind,
        ESCROW_CREATE_TRANSACTION_KIND
            | ESCROW_FINISH_TRANSACTION_KIND
            | ESCROW_CANCEL_TRANSACTION_KIND
    )
}

fn is_supported_nft_transaction_kind(kind: &str) -> bool {
    matches!(
        kind,
        NFT_MINT_TRANSACTION_KIND | NFT_TRANSFER_TRANSACTION_KIND | NFT_BURN_TRANSACTION_KIND
    )
}

fn is_supported_offer_transaction_kind(kind: &str) -> bool {
    matches!(
        kind,
        OFFER_CREATE_TRANSACTION_KIND | OFFER_CANCEL_TRANSACTION_KIND
    )
}

fn is_supported_offer_state(state: &str) -> bool {
    matches!(
        state,
        OFFER_STATE_OPEN | OFFER_STATE_FILLED | OFFER_STATE_CANCELED | OFFER_STATE_UNFUNDED
    )
}

fn validate_shielded_action_batch_result(
    result: &Value,
    expected_kind: &str,
) -> Result<(), RpcResponseValidationError> {
    reject_private_key_material_fields(
        result,
        "shielded response contains private key material fields",
    )?;
    lower_hex_field(result, "batch_id", 96)?;
    let actions = array_field(result, "actions")?;
    if actions.is_empty() {
        return Err(invalid_result(
            "actions",
            "expected at least one shielded action",
        ));
    }
    for action in actions {
        expect_string_eq(action, "kind", expected_kind)?;
        match expected_kind {
            "shield_mint" => validate_shield_mint_action(action)?,
            "shield_spend" => validate_shield_spend_action(action)?,
            "shield_migrate" => validate_shield_migrate_action(action)?,
            "orchard_action_v1" => validate_orchard_action_payload(action)?,
            "orchard_deposit_v1" => validate_orchard_deposit_action_payload(action)?,
            "orchard_withdraw_v1" => validate_orchard_withdraw_action_payload(action)?,
            "shielded_swap_v1" => validate_shielded_swap_action_payload(action)?,
            _ => {
                return Err(invalid_result(
                    "kind",
                    format!("unsupported shielded action kind `{expected_kind}`"),
                ))
            }
        }
    }
    Ok(())
}

fn validate_shield_mint_action(action: &Value) -> Result<(), RpcResponseValidationError> {
    nonempty_string_field(action, "owner")?;
    nonempty_string_field(action, "asset_id")?;
    nonzero_u64_field(action, "amount")?;
    string_field(action, "memo")?;
    Ok(())
}

fn validate_shield_spend_action(action: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(action, "note_id", 96)?;
    nonempty_string_field(action, "to")?;
    nonzero_u64_field(action, "amount")?;
    string_field(action, "memo")?;
    Ok(())
}

fn validate_shield_migrate_action(action: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(action, "note_id", 96)?;
    nonempty_string_field(action, "target_pool")?;
    string_field(action, "memo")?;
    Ok(())
}

fn validate_orchard_action_payload(action: &Value) -> Result<(), RpcResponseValidationError> {
    validate_orchard_action_json_payload(action)?;
    Ok(())
}

fn validate_orchard_deposit_action_payload(
    action: &Value,
) -> Result<(), RpcResponseValidationError> {
    let parsed = validate_orchard_action_json_payload(action)?;
    validate_signed_transfer_fields(field(action, "funding_transfer")?)?;
    nonzero_u64_field(action, "amount")?;
    u64_field(action, "fee")?;
    expect_string_eq(action, "policy_id", ORCHARD_DEPOSIT_POLICY_ID)?;
    let disclosure_hash = string_field(action, "disclosure_hash")?;
    if !disclosure_hash.is_empty() && !is_lower_hex_len(disclosure_hash, 96) {
        return Err(invalid_result(
            "disclosure_hash",
            "expected empty string or 96 lowercase hex characters",
        ));
    }
    lower_hex_field(&parsed, "external_binding_hash", 96)?;
    let action_fee = u64_field(&parsed, "fee")?;
    if action_fee != 0 {
        return Err(invalid_result(
            "action_json.fee",
            format!("expected deposit action fee 0, found {action_fee}"),
        ));
    }
    Ok(())
}

fn validate_orchard_withdraw_action_payload(
    action: &Value,
) -> Result<(), RpcResponseValidationError> {
    let parsed = validate_orchard_action_json_payload(action)?;
    nonempty_string_field(action, "to")?;
    nonzero_u64_field(action, "amount")?;
    let fee = u64_field(action, "fee")?;
    expect_string_eq(action, "policy_id", ORCHARD_WITHDRAW_POLICY_ID)?;
    let disclosure_hash = string_field(action, "disclosure_hash")?;
    if !disclosure_hash.is_empty() && !is_lower_hex_len(disclosure_hash, 96) {
        return Err(invalid_result(
            "disclosure_hash",
            "expected empty string or 96 lowercase hex characters",
        ));
    }
    lower_hex_field(&parsed, "external_binding_hash", 96)?;
    let action_fee = u64_field(&parsed, "fee")?;
    if action_fee != fee {
        return Err(invalid_result(
            "action_json.fee",
            format!("expected withdraw payload fee {fee}, found {action_fee}"),
        ));
    }
    Ok(())
}

fn validate_shielded_swap_action_payload(action: &Value) -> Result<(), RpcResponseValidationError> {
    let swap_json = nonempty_string_field(action, "swap_json")?;
    if swap_json.len() > MAX_RPC_BATCH_ARCHIVE_PAYLOAD_BYTES {
        return Err(invalid_result(
            "swap_json",
            format!("expected at most {MAX_RPC_BATCH_ARCHIVE_PAYLOAD_BYTES} bytes"),
        ));
    }
    let parsed = serde_json::from_str::<Value>(swap_json).map_err(|error| {
        invalid_result("swap_json", format!("invalid ShieldedSwap JSON: {error}"))
    })?;
    if !parsed.is_object() {
        return Err(invalid_result(
            "swap_json",
            "expected ShieldedSwap action JSON object",
        ));
    }
    if contains_private_key_material_field(&parsed) {
        return Err(invalid_result(
            "swap_json",
            "ShieldedSwap action payload contains private key material fields",
        ));
    }
    if parsed
        .as_object()
        .is_some_and(|object| object.contains_key("pool_domain")
            || object.contains_key("randomized_verification_keys"))
    {
        return validate_asset_orchard_swap_action_payload(&parsed);
    }
    validate_legacy_shielded_swap_action_payload(&parsed)
}

fn validate_asset_orchard_swap_action_payload(
    parsed: &Value,
) -> Result<(), RpcResponseValidationError> {
    for field_name in [
        "version",
        "schema",
        "pool_id",
        "proof_system_id",
        "circuit_id",
        "pool_domain",
        "anchor",
        "nullifiers",
        "randomized_verification_keys",
        "output_commitments",
        "encrypted_outputs",
        "pricing_claim",
        "swap_binding_hash",
        "fee",
        "proof",
        "spend_authorization_signatures",
    ] {
        field(parsed, field_name)?;
    }
    if u64_field(parsed, "version")? != 1 {
        return Err(invalid_result("version", "expected 1"));
    }
    expect_string_eq(parsed, "schema", ASSET_ORCHARD_SWAP_ACTION_SCHEMA_V1)?;
    expect_string_eq(parsed, "pool_id", ASSET_ORCHARD_POOL_ID_V1)?;
    expect_string_eq(
        parsed,
        "proof_system_id",
        ASSET_ORCHARD_PROOF_SYSTEM_ID_V1,
    )?;
    expect_string_eq(parsed, "circuit_id", ASSET_ORCHARD_CIRCUIT_ID_V1)?;
    lower_hex_field(parsed, "pool_domain", 64)?;
    lower_hex_field(parsed, "anchor", 64)?;
    validate_fixed_hex_array(parsed, "nullifiers", 2, 64)?;
    validate_fixed_hex_array(parsed, "randomized_verification_keys", 2, 64)?;
    validate_fixed_hex_array(parsed, "output_commitments", 2, 64)?;
    validate_bounded_lower_hex_string_array(
        parsed,
        "encrypted_outputs",
        2,
        MAX_RPC_ASSET_ORCHARD_ENCRYPTED_OUTPUT_BYTES,
    )?;
    let pricing = field(parsed, "pricing_claim")?;
    if u64_field(pricing, "nav_epoch")? == 0
        || u64_field(pricing, "ratio_numerator")? == 0
        || u64_field(pricing, "ratio_denominator")? == 0
    {
        return Err(invalid_result("pricing_claim", "epoch and ratio terms must be nonzero"));
    }
    lower_hex_field(pricing, "reserve_packet_hash", 96)?;
    for tag in ["base_asset_tag_lo", "base_asset_tag_hi", "quote_asset_tag_lo", "quote_asset_tag_hi"] {
        lower_hex_field(pricing, tag, 32)?;
    }
    let mode = nonempty_string_field(pricing, "mode")?;
    if !matches!(mode, "at_nav" | "at_nav_with_band" | "negotiated") {
        return Err(invalid_result("pricing_claim.mode", "unsupported pricing mode"));
    }
    if u64_field(pricing, "band_bps")? > 10_000 {
        return Err(invalid_result("pricing_claim.band_bps", "must not exceed 10000"));
    }
    lower_hex_field(parsed, "swap_binding_hash", 128)?;
    if u64_field(parsed, "fee")? != 0 {
        return Err(invalid_result("fee", "expected 0"));
    }
    validate_bounded_lower_hex_string_field(parsed, "proof", MAX_RPC_ASSET_ORCHARD_PROOF_BYTES)?;
    validate_fixed_hex_array(parsed, "spend_authorization_signatures", 2, 128)?;
    Ok(())
}

fn validate_legacy_shielded_swap_action_payload(
    parsed: &Value,
) -> Result<(), RpcResponseValidationError> {
    for field_name in [
        "schema",
        "pool_id",
        "proof_system_id",
        "circuit_id",
        "anchor",
        "nullifiers",
        "input_asset_commitments",
        "input_value_commitments",
        "input_authorization_commitments",
        "output_commitments",
        "output_asset_commitments",
        "output_value_commitments",
        "encrypted_outputs",
        "swap_binding_hash",
        "fee",
        "proof",
    ] {
        field(&parsed, field_name)?;
    }
    nonempty_string_field(&parsed, "schema")?;
    nonempty_string_field(&parsed, "pool_id")?;
    nonempty_string_field(&parsed, "proof_system_id")?;
    nonempty_string_field(&parsed, "circuit_id")?;
    lower_hex_field(&parsed, "anchor", 64)?;
    lower_hex_field(&parsed, "swap_binding_hash", 96)?;
    lower_hex_field(&parsed, "proof", 96)?;
    u64_field(&parsed, "fee")?;
    validate_fixed_hex_array(&parsed, "nullifiers", 2, 64)?;
    validate_fixed_hex_array(&parsed, "input_asset_commitments", 2, 96)?;
    validate_fixed_hex_array(&parsed, "input_value_commitments", 2, 96)?;
    validate_fixed_hex_array(&parsed, "input_authorization_commitments", 2, 96)?;
    validate_fixed_hex_array(&parsed, "output_commitments", 2, 64)?;
    validate_fixed_hex_array(&parsed, "output_asset_commitments", 2, 96)?;
    validate_fixed_hex_array(&parsed, "output_value_commitments", 2, 96)?;
    let encrypted_outputs = array_field(&parsed, "encrypted_outputs")?;
    if encrypted_outputs.len() != 2 {
        return Err(invalid_result(
            "encrypted_outputs",
            "expected exactly two encrypted outputs",
        ));
    }
    for (index, output) in encrypted_outputs.iter().enumerate() {
        let prefix = format!("encrypted_outputs[{index}]");
        lower_hex_field(output, "cmx", 64)?;
        lower_hex_field(output, "epk", 64)?;
        lower_hex_field(output, "enc_ciphertext", 1160)?;
        lower_hex_field(output, "out_ciphertext", 160)?;
        if output
            .as_object()
            .is_some_and(|object| object.contains_key("compact_ciphertext"))
        {
            lower_hex_field(output, "compact_ciphertext", 104)?;
        }
        if string_field(output, "cmx")? != string_array_entry(&parsed, "output_commitments", index)?
        {
            return Err(invalid_result(
                prefix,
                "expected encrypted output cmx to match output commitment",
            ));
        }
    }
    Ok(())
}

fn validate_orchard_action_json_payload(
    action: &Value,
) -> Result<Value, RpcResponseValidationError> {
    let action_json = nonempty_string_field(action, "action_json")?;
    if action_json.len() > MAX_RPC_BATCH_ARCHIVE_PAYLOAD_BYTES {
        return Err(invalid_result(
            "action_json",
            format!("expected at most {MAX_RPC_BATCH_ARCHIVE_PAYLOAD_BYTES} bytes"),
        ));
    }
    let parsed = serde_json::from_str::<Value>(action_json)
        .map_err(|error| invalid_result("action_json", format!("invalid Orchard JSON: {error}")))?;
    if !parsed.is_object() {
        return Err(invalid_result(
            "action_json",
            "expected Orchard action JSON object",
        ));
    }
    if contains_private_key_material_field(&parsed) {
        return Err(invalid_result(
            "action_json",
            "Orchard action payload contains private key material fields",
        ));
    }
    nonempty_string_field(&parsed, "pool_id")?;
    string_field(&parsed, "proof_system_id")?;
    string_field(&parsed, "circuit_id")?;
    string_field(&parsed, "anchor")?;
    array_field(&parsed, "nullifiers")?;
    array_field(&parsed, "output_commitments")?;
    array_field(&parsed, "encrypted_outputs")?;
    i64_field(&parsed, "value_balance")?;
    u64_field(&parsed, "fee")?;
    string_field(&parsed, "proof")?;
    string_field(&parsed, "binding_signature")?;
    Ok(parsed)
}

fn validate_fixed_hex_array(
    value: &Value,
    path: &str,
    expected_count: usize,
    expected_len: usize,
) -> Result<(), RpcResponseValidationError> {
    let entries = array_field(value, path)?;
    if entries.len() != expected_count {
        return Err(invalid_result(
            path,
            format!("expected exactly {expected_count} entries"),
        ));
    }
    for (index, entry) in entries.iter().enumerate() {
        let Some(found) = entry.as_str() else {
            return Err(invalid_result(
                format!("{path}[{index}]"),
                format!("expected {expected_len} lowercase hex characters"),
            ));
        };
        if !is_lower_hex_len(found, expected_len) {
            return Err(invalid_result(
                format!("{path}[{index}]"),
                format!("expected {expected_len} lowercase hex characters"),
            ));
        }
    }
    Ok(())
}

fn validate_bounded_lower_hex_string_field(
    value: &Value,
    path: &str,
    max_bytes: usize,
) -> Result<(), RpcResponseValidationError> {
    let found = string_field(value, path)?;
    validate_bounded_lower_hex_string(found, path.to_string(), max_bytes)
}

fn validate_bounded_lower_hex_string_array(
    value: &Value,
    path: &str,
    expected_count: usize,
    max_bytes: usize,
) -> Result<(), RpcResponseValidationError> {
    let entries = array_field(value, path)?;
    if entries.len() != expected_count {
        return Err(invalid_result(
            path,
            format!("expected exactly {expected_count} entries"),
        ));
    }
    for (index, entry) in entries.iter().enumerate() {
        let Some(found) = entry.as_str() else {
            return Err(invalid_result(
                format!("{path}[{index}]"),
                format!("expected nonempty lowercase hex blob up to {max_bytes} bytes"),
            ));
        };
        validate_bounded_lower_hex_string(found, format!("{path}[{index}]"), max_bytes)?;
    }
    Ok(())
}

fn validate_bounded_lower_hex_string(
    value: &str,
    path: String,
    max_bytes: usize,
) -> Result<(), RpcResponseValidationError> {
    if value.is_empty() || value.len() % 2 != 0 || value.len() > max_bytes * 2 {
        return Err(invalid_result(
            path,
            format!("expected nonempty lowercase hex blob up to {max_bytes} bytes"),
        ));
    }
    if !value
        .bytes()
        .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(invalid_result(
            path,
            format!("expected nonempty lowercase hex blob up to {max_bytes} bytes"),
        ));
    }
    Ok(())
}

fn string_array_entry<'a>(
    value: &'a Value,
    path: &str,
    index: usize,
) -> Result<&'a str, RpcResponseValidationError> {
    array_field(value, path)?
        .get(index)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_result(format!("{path}[{index}]"), "expected string value"))
}

fn validate_redacted_receipts_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    reject_key_material_fields(result, "shielded response contains key material fields")?;
    validate_receipts_result(result)
}

fn validate_shield_scan_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    reject_key_material_fields(result, "shielded response contains key material fields")?;
    for note in result_array(result, "result")? {
        validate_shielded_note_fields(note)?;
    }
    Ok(())
}

fn validate_shield_disclosure_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    reject_key_material_fields(result, "shielded response contains key material fields")?;
    validate_shielded_note_fields(field(result, "note")?)?;
    lower_hex_field(result, "nullifier", 96)?;
    bool_field(result, "spent")?;
    Ok(())
}

fn validate_shield_turnstile_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    reject_key_material_fields(result, "shielded response contains key material fields")?;
    u64_field(result, "event_count")?;
    u64_field(result, "bootstrap_deposit_total")?;
    u64_field(result, "migration_total")?;
    u64_field(result, "orchard_deposit_total")?;
    for event in array_field(result, "events")? {
        validate_turnstile_event_fields(event)?;
    }
    Ok(())
}

fn validate_shielded_note_fields(note: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(note, "note_id", 96)?;
    lower_hex_field(note, "commitment", 96)?;
    u64_field(note, "position")?;
    nonempty_string_field(note, "owner")?;
    nonempty_string_field(note, "asset_id")?;
    nonzero_u64_field(note, "value")?;
    lower_hex_field(note, "rho", 96)?;
    string_field(note, "memo")?;
    lower_hex_field(note, "created_by", 96)?;
    Ok(())
}

fn validate_turnstile_event_fields(event: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(event, "event_id", 96)?;
    let kind = string_field(event, "kind")?;
    if !matches!(
        kind,
        "bootstrap_deposit" | "pool_migration" | "orchard_deposit"
    ) {
        return Err(invalid_result(
            "kind",
            format!("expected shielded turnstile event kind, found `{kind}`"),
        ));
    }
    nonempty_string_field(event, "owner")?;
    nonempty_string_field(event, "asset_id")?;
    nonzero_u64_field(event, "amount")?;
    lower_hex_field(event, "note_id", 96)?;
    nonempty_string_field(event, "source_pool")?;
    nonempty_string_field(event, "target_pool")?;
    Ok(())
}

fn validate_bridge_status_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    for domain in array_field(result, "domains")? {
        validate_bridge_domain_fields(domain)?;
    }
    for transfer in array_field(result, "transfers")? {
        validate_bridge_transfer_fields(transfer)?;
    }
    for replay_entry in array_field(result, "replay_cache")? {
        clean_string_entry(replay_entry, "replay_cache")?;
    }
    Ok(())
}

fn validate_navcoin_bridge_routes_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", "postfiat-pftl-uniswap-routes-status-v1")?;
    let route_count = u64_field(result, "route_count")?;
    let routes = array_field(result, "routes")?;
    if route_count != routes.len() as u64 {
        return Err(invalid_result(
            "route_count",
            "expected route_count to match routes length",
        ));
    }
    if routes.len() > MAX_RPC_READ_QUERY_LIMIT {
        return Err(invalid_result(
            "routes",
            format!("expected at most {MAX_RPC_READ_QUERY_LIMIT} route rows"),
        ));
    }
    let mut previous_route_id: Option<&str> = None;
    for route in routes {
        validate_navcoin_bridge_route_status_row(route)?;
        let route_id = string_field(route, "route_id")?;
        if previous_route_id.is_some_and(|previous| previous > route_id) {
            return Err(invalid_result(
                "routes",
                "expected route rows sorted by route_id",
            ));
        }
        previous_route_id = Some(route_id);
    }
    Ok(())
}

fn validate_navcoin_bridge_packet_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", "postfiat-pftl-uniswap-packet-status-v1")?;
    clean_string_field(result, "route_id")?;
    lower_hex_field(result, "route_config_digest", 96)?;
    let packet_hash = string_field(result, "packet_hash")?;
    lower_hex_field(result, "packet_hash", 96)?;
    let packet = field(result, "packet")?;
    validate_navcoin_bridge_export_packet_row(packet)?;
    let packet_row_hash = string_field(packet, "packet_hash")?;
    if packet_hash != packet_row_hash {
        return Err(invalid_result(
            "packet.packet_hash",
            "expected packet row hash to match report packet_hash",
        ));
    }
    lower_hex_field(result, "ledger_hash", 96)?;
    Ok(())
}

fn validate_navcoin_bridge_claims_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", "postfiat-pftl-uniswap-claims-status-v1")?;
    clean_string_field(result, "route_id")?;
    lower_hex_field(result, "route_config_digest", 96)?;
    lower_hex_field(result, "ledger_hash", 96)?;
    let limit = nonzero_u64_field(result, "limit")?;
    if limit > MAX_RPC_READ_QUERY_LIMIT as u64 {
        return Err(invalid_result(
            "limit",
            format!("expected limit at most {MAX_RPC_READ_QUERY_LIMIT}"),
        ));
    }
    bool_field(result, "truncated")?;
    u64_field(result, "outstanding_bridge_claims_atoms")?;
    u64_field(result, "pending_return_import_claims_atoms")?;
    let export_claim_count = u64_field(result, "export_claim_count")?;
    let return_claim_count = u64_field(result, "return_claim_count")?;
    let exports = array_field(result, "exports")?;
    let returns = array_field(result, "returns")?;
    let returned_rows = exports.len().checked_add(returns.len()).ok_or_else(|| {
        invalid_result("claims", "returned claims row count overflowed")
    })?;
    if returned_rows as u64 > limit {
        return Err(invalid_result(
            "claims",
            "expected returned claim rows to be at most limit",
        ));
    }
    if !bool_field(result, "truncated")?
        && (export_claim_count != exports.len() as u64
            || return_claim_count != returns.len() as u64)
    {
        return Err(invalid_result(
            "claims",
            "expected non-truncated claim counts to match returned rows",
        ));
    }
    for export in exports {
        validate_navcoin_bridge_export_packet_row(export)?;
    }
    for burn in returns {
        validate_navcoin_bridge_return_burn_row(burn)?;
    }
    Ok(())
}

fn validate_navcoin_bridge_supply_status_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", "postfiat-pftl-uniswap-supply-status-v1")?;
    clean_string_field(result, "route_id")?;
    lower_hex_field(result, "route_config_digest", 96)?;
    lower_hex_field(result, "native_nav_asset_id", 96)?;
    lower_hex_field(result, "settlement_asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    validate_evm_address_field(result, "wrapped_navcoin_token")?;
    let authorized = u64_field(result, "authorized_valid_supply_atoms")?;
    let pftl = u64_field(result, "pftl_spendable_supply_atoms")?;
    let native_balance_count = u64_field(result, "native_spendable_balance_count")?;
    let native_balance_limit = nonzero_u64_field(result, "native_spendable_balance_limit")?;
    if native_balance_limit > MAX_RPC_READ_QUERY_LIMIT as u64 {
        return Err(invalid_result(
            "native_spendable_balance_limit",
            format!("expected native balance limit at most {MAX_RPC_READ_QUERY_LIMIT}"),
        ));
    }
    let native_balances_truncated = bool_field(result, "native_spendable_balances_truncated")?;
    let native_balance_total = u64_field(result, "native_spendable_balance_sum_atoms")?;
    if native_balance_total != pftl {
        return Err(invalid_result(
            "native_spendable_balance_sum_atoms",
            "expected native balance sum to equal PFTL spendable supply",
        ));
    }
    let mut native_balance_sum = 0_u64;
    let mut previous_wallet: Option<&str> = None;
    let native_balance_rows = array_field(result, "native_spendable_balances")?;
    if native_balance_rows.len() as u64 > native_balance_limit {
        return Err(invalid_result(
            "native_spendable_balances",
            "expected displayed native balance rows to be at most the declared limit",
        ));
    }
    for row in native_balance_rows {
        let wallet = clean_string_field(row, "wallet")?;
        if let Some(previous) = previous_wallet {
            if previous >= wallet {
                return Err(invalid_result(
                    "native_spendable_balances",
                    "expected native balance rows to be sorted by unique wallet",
                ));
            }
        }
        previous_wallet = Some(wallet);
        let amount = nonzero_u64_field(row, "amount_atoms")?;
        native_balance_sum = native_balance_sum.checked_add(amount).ok_or_else(|| {
            invalid_result("native_spendable_balances", "native balance sum overflowed")
        })?;
    }
    if native_balances_truncated {
        if native_balance_count <= native_balance_limit {
            return Err(invalid_result(
                "native_spendable_balances_truncated",
                "truncated native balance rows require count greater than limit",
            ));
        }
        if native_balance_rows.len() as u64 != native_balance_limit {
            return Err(invalid_result(
                "native_spendable_balances",
                "truncated native balance rows must fill the declared limit",
            ));
        }
        if native_balance_sum > native_balance_total {
            return Err(invalid_result(
                "native_spendable_balances",
                "displayed native balance rows exceed the declared full native balance sum",
            ));
        }
    } else {
        if native_balance_rows.len() as u64 != native_balance_count {
            return Err(invalid_result(
                "native_spendable_balance_count",
                "untruncated native balance count must equal displayed rows",
            ));
        }
        if native_balance_sum != native_balance_total {
            return Err(invalid_result(
                "native_spendable_balances",
                "untruncated native balance rows must sum to the declared full native balance sum",
            ));
        }
    }
    if native_balance_count < native_balance_rows.len() as u64 {
        return Err(invalid_result(
            "native_spendable_balance_count",
            "native balance count cannot be smaller than displayed rows",
        ));
    }
    let ethereum = u64_field(result, "ethereum_spendable_supply_atoms")?;
    let other = u64_field(result, "other_registered_venue_supply_atoms")?;
    let outstanding = u64_field(result, "outstanding_bridge_claims_atoms")?;
    let pending_return = u64_field(result, "pending_return_import_claims_atoms")?;
    let live_sum = u64_field(result, "live_supply_sum_atoms")?;
    let route_cap = nonzero_u64_field(result, "route_supply_cap_atoms")?;
    u64_field(result, "supply_cap_remaining_atoms")?;
    nonzero_u64_field(result, "packet_notional_cap_atoms")?;
    u64_field(result, "settlement_reserve_atoms")?;
    let invariant_holds = bool_field(result, "invariant_holds")?;
    let recomputed = pftl
        .checked_add(ethereum)
        .and_then(|value| value.checked_add(other))
        .and_then(|value| value.checked_add(outstanding))
        .and_then(|value| value.checked_add(pending_return))
        .ok_or_else(|| invalid_result("live_supply_sum_atoms", "supply sum overflowed"))?;
    if live_sum != recomputed {
        return Err(invalid_result(
            "live_supply_sum_atoms",
            "expected live supply sum to equal supply components",
        ));
    }
    if invariant_holds && live_sum != authorized {
        return Err(invalid_result(
            "invariant_holds",
            "invariant_holds cannot be true when live supply differs from authorized supply",
        ));
    }
    if authorized > route_cap {
        return Err(invalid_result(
            "authorized_valid_supply_atoms",
            "authorized supply exceeds route cap",
        ));
    }
    lower_hex_field(result, "ledger_hash", 96)?;
    Ok(())
}

fn validate_navcoin_bridge_receipt_replay_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(
        result,
        "schema",
        "postfiat-navcoin-bridge-receipt-replay-v1",
    )?;
    clean_string_field(result, "route_id")?;
    lower_hex_field(result, "route_config_digest", 96)?;
    let initial_ledger_hash = string_field(result, "initial_ledger_hash")?;
    lower_hex_field(result, "initial_ledger_hash", 96)?;
    let final_ledger_hash = string_field(result, "final_ledger_hash")?;
    lower_hex_field(result, "final_ledger_hash", 96)?;
    let receipt_count = u64_field(result, "receipt_count")?;
    clean_string_field(result, "ledger_file")?;
    clean_string_field(result, "receipt_file")?;
    let status = clean_string_field(result, "status")?;
    match status {
        "empty_clean" => {
            if receipt_count != 0 {
                return Err(invalid_result(
                    "receipt_count",
                    "empty_clean replay reports must have zero receipts",
                ));
            }
            if initial_ledger_hash != final_ledger_hash {
                return Err(invalid_result(
                    "final_ledger_hash",
                    "empty_clean replay reports must have matching initial and final ledger hashes",
                ));
            }
            if !field(result, "receipt_root")?.is_null() {
                return Err(invalid_result(
                    "receipt_root",
                    "empty_clean replay reports must not include a receipt root",
                ));
            }
            if !field(result, "replay")?.is_null() {
                return Err(invalid_result(
                    "replay",
                    "empty_clean replay reports must not include a nested replay report",
                ));
            }
        }
        "verified" => {
            if receipt_count == 0 {
                return Err(invalid_result(
                    "receipt_count",
                    "verified replay reports must have at least one receipt",
                ));
            }
            let receipt_root = string_field(result, "receipt_root")?;
            lower_hex_field(result, "receipt_root", 96)?;
            let replay = field(result, "replay")?;
            expect_string_eq(
                replay,
                "schema",
                "postfiat-pftl-uniswap-receipt-replay-report-v1",
            )?;
            let replay_route_id = clean_string_field(replay, "route_id")?;
            if replay_route_id != clean_string_field(result, "route_id")? {
                return Err(invalid_result(
                    "replay.route_id",
                    "nested replay route id must match report route id",
                ));
            }
            if string_field(replay, "initial_ledger_hash")? != initial_ledger_hash {
                return Err(invalid_result(
                    "replay.initial_ledger_hash",
                    "nested replay initial ledger hash must match report initial ledger hash",
                ));
            }
            lower_hex_field(replay, "initial_ledger_hash", 96)?;
            if string_field(replay, "final_ledger_hash")? != final_ledger_hash {
                return Err(invalid_result(
                    "replay.final_ledger_hash",
                    "nested replay final ledger hash must match report final ledger hash",
                ));
            }
            lower_hex_field(replay, "final_ledger_hash", 96)?;
            if string_field(replay, "receipt_root")? != receipt_root {
                return Err(invalid_result(
                    "replay.receipt_root",
                    "nested replay receipt root must match report receipt root",
                ));
            }
            lower_hex_field(replay, "receipt_root", 96)?;
            if u64_field(replay, "receipt_count")? != receipt_count {
                return Err(invalid_result(
                    "replay.receipt_count",
                    "nested replay receipt count must match report receipt count",
                ));
            }
        }
        _ => {
            return Err(invalid_result(
                "status",
                "expected receipt replay status to be `verified` or `empty_clean`",
            ));
        }
    }
    Ok(())
}

fn validate_navcoin_bridge_packet_preflight_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(
        result,
        "schema",
        "postfiat-navcoin-bridge-packet-preflight-v1",
    )?;
    clean_string_field(result, "route_id")?;
    lower_hex_field(result, "route_config_digest", 96)?;
    lower_hex_field(result, "launch_config_digest", 96)?;
    lower_hex_field(result, "packet_digest", 96)?;
    lower_hex_field(result, "ledger_hash", 96)?;
    clean_string_field(result, "packet_file")?;
    expect_string_eq(result, "status", "ready")?;
    Ok(())
}

fn validate_navcoin_bridge_route_status_row(
    route: &Value,
) -> Result<(), RpcResponseValidationError> {
    clean_string_field(route, "route_id")?;
    validate_navcoin_bridge_route_family_field(route, "route_family")?;
    lower_hex_field(route, "route_config_digest", 96)?;
    validate_route_trust_class_field(route, "route_trust_class")?;
    bool_field(route, "route_live")?;
    bool_field(route, "paused")?;
    lower_hex_field(route, "native_nav_asset_id", 96)?;
    lower_hex_field(route, "settlement_asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    validate_evm_address_field(route, "wrapped_navcoin_token")?;
    validate_evm_address_field(route, "handoff_controller")?;
    validate_evm_address_field(route, "settlement_adapter")?;
    nonzero_u64_field(route, "ethereum_chain_id")?;
    nonzero_u64_field(route, "latest_finalized_nav_epoch")?;
    let route_cap = nonzero_u64_field(route, "route_supply_cap_atoms")?;
    nonzero_u64_field(route, "packet_notional_cap_atoms")?;
    let authorized = u64_field(route, "authorized_valid_supply_atoms")?;
    u64_field(route, "supply_cap_remaining_atoms")?;
    if authorized > route_cap {
        return Err(invalid_result(
            "authorized_valid_supply_atoms",
            "authorized supply exceeds route cap",
        ));
    }
    u64_field(route, "outstanding_bridge_claims_atoms")?;
    u64_field(route, "pending_return_import_claims_atoms")?;
    u64_field(route, "primary_subscription_count")?;
    let export_packet_count = u64_field(route, "export_packet_count")?;
    let outstanding_count = u64_field(route, "outstanding_export_packet_count")?;
    let consumed_count = u64_field(route, "consumed_export_packet_count")?;
    let refunded_count = u64_field(route, "refunded_export_packet_count")?;
    if outstanding_count
        .checked_add(consumed_count)
        .and_then(|value| value.checked_add(refunded_count))
        != Some(export_packet_count)
    {
        return Err(invalid_result(
            "export_packet_count",
            "expected export packet status counts to sum to export_packet_count",
        ));
    }
    let return_burn_count = u64_field(route, "return_burn_count")?;
    let pending_count = u64_field(route, "pending_return_burn_count")?;
    let imported_count = u64_field(route, "imported_return_burn_count")?;
    if pending_count.checked_add(imported_count) != Some(return_burn_count) {
        return Err(invalid_result(
            "return_burn_count",
            "expected return burn status counts to sum to return_burn_count",
        ));
    }
    lower_hex_field(route, "ledger_hash", 96)?;
    Ok(())
}

fn validate_navcoin_bridge_route_family_field(
    value: &Value,
    field_name: &'static str,
) -> Result<(), RpcResponseValidationError> {
    let route_family = string_field(value, field_name)?;
    match route_family {
        "primary_pftl_mint" | "secondary_inventory" => Ok(()),
        _ => Err(invalid_result(
            field_name,
            "expected NAVCoin bridge route_family to be primary_pftl_mint or secondary_inventory",
        )),
    }
}

fn validate_navcoin_bridge_export_packet_row(
    packet: &Value,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(packet, "packet_hash", 96)?;
    lower_hex_field(packet, "nonce", 64)?;
    clean_string_field(packet, "source_wallet")?;
    validate_evm_address_field(packet, "ethereum_recipient")?;
    nonzero_u64_field(packet, "amount_atoms")?;
    nonzero_u64_field(packet, "source_height")?;
    nonzero_u64_field(packet, "destination_deadline_seconds")?;
    nonzero_u64_field(packet, "refund_not_before_height")?;
    let status = string_field(packet, "status")?;
    let expected_claim_class = match status {
        "SourceDebited" => "outstanding_bridge_claim",
        "DestinationConsumed" => "destination_consumed",
        "SourceRefunded" => "source_refunded",
        _ => {
            return Err(invalid_result(
                "status",
                format!("unsupported export packet status `{status}`"),
            ))
        }
    };
    expect_string_eq(packet, "claim_class", expected_claim_class)?;
    Ok(())
}

fn validate_navcoin_bridge_return_burn_row(
    burn: &Value,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(burn, "burn_event_hash", 64)?;
    nonzero_u64_field(burn, "ethereum_chain_id")?;
    validate_evm_address_field(burn, "bridge_controller")?;
    validate_evm_address_field(burn, "wrapped_navcoin_token")?;
    lower_hex_field(burn, "native_nav_asset_id", 96)?;
    validate_evm_address_field(burn, "ethereum_sender")?;
    clean_string_field(burn, "pftl_recipient")?;
    nonzero_u64_field(burn, "amount_atoms")?;
    lower_hex_field(burn, "return_nonce", 64)?;
    nonzero_u64_field(burn, "burn_height")?;
    nonzero_u64_field(burn, "finalized_height")?;
    let status = string_field(burn, "status")?;
    let expected_claim_class = match status {
        "BurnObserved" => "pending_return_import_claim",
        "Imported" => "return_imported",
        _ => {
            return Err(invalid_result(
                "status",
                format!("unsupported return burn status `{status}`"),
            ))
        }
    };
    expect_string_eq(burn, "claim_class", expected_claim_class)?;
    Ok(())
}

fn validate_route_trust_class_field(
    value: &Value,
    path: &str,
) -> Result<(), RpcResponseValidationError> {
    let found = string_field(value, path)?;
    if matches!(
        found,
        "CONTROLLED" | "OPTIMISTIC" | "TRUSTLESS_FINALITY" | "DISABLED"
    ) {
        Ok(())
    } else {
        Err(invalid_result(
            path,
            format!("unsupported route trust class `{found}`"),
        ))
    }
}

fn validate_evm_address_field(value: &Value, path: &str) -> Result<(), RpcResponseValidationError> {
    let found = string_field(value, path)?;
    if found.len() == 42
        && found.starts_with("0x")
        && found[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        Ok(())
    } else {
        Err(invalid_result(
            path,
            "expected 20-byte 0x-prefixed EVM address",
        ))
    }
}

fn validate_bridge_action_batch_result(
    result: &Value,
    expected_kind: &str,
    expected_paused: Option<bool>,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(result, "batch_id", 96)?;
    let actions = array_field(result, "actions")?;
    if actions.is_empty() {
        return Err(invalid_result(
            "actions",
            "expected at least one bridge action",
        ));
    }
    for action in actions {
        expect_string_eq(action, "kind", expected_kind)?;
        match expected_kind {
            "bridge_domain" => validate_bridge_domain_action(action)?,
            "bridge_transfer" => validate_bridge_transfer_action(action)?,
            "bridge_pause" => validate_bridge_pause_action(action, expected_paused)?,
            _ => {
                return Err(invalid_result(
                    "kind",
                    format!("unsupported bridge action kind `{expected_kind}`"),
                ))
            }
        }
    }
    Ok(())
}

fn validate_bridge_domain_action(action: &Value) -> Result<(), RpcResponseValidationError> {
    clean_string_field(action, "domain_id")?;
    clean_string_field(action, "name")?;
    clean_string_field(action, "source_chain")?;
    clean_string_field(action, "target_chain")?;
    clean_string_field(action, "bridge_id")?;
    clean_string_field(action, "door_account")?;
    nonzero_u64_field(action, "inbound_cap")?;
    nonzero_u64_field(action, "outbound_cap")?;
    Ok(())
}

fn validate_bridge_transfer_action(action: &Value) -> Result<(), RpcResponseValidationError> {
    clean_string_field(action, "domain_id")?;
    validate_bridge_direction(action, "direction")?;
    clean_string_field(action, "from")?;
    clean_string_field(action, "to")?;
    clean_string_field(action, "asset_id")?;
    nonzero_u64_field(action, "amount")?;
    clean_string_field(action, "witness_id")?;
    u64_field(action, "witness_epoch")?;
    validate_bridge_witness_attestation(field(action, "witness_attestation")?)?;
    Ok(())
}

fn validate_bridge_pause_action(
    action: &Value,
    expected_paused: Option<bool>,
) -> Result<(), RpcResponseValidationError> {
    clean_string_field(action, "domain_id")?;
    let paused = bool_field(action, "paused")?;
    if let Some(expected) = expected_paused {
        if paused != expected {
            return Err(invalid_result(
                "paused",
                format!("expected {expected}, found {paused}"),
            ));
        }
    }
    Ok(())
}

fn validate_bridge_domain_fields(domain: &Value) -> Result<(), RpcResponseValidationError> {
    clean_string_field(domain, "domain_id")?;
    clean_string_field(domain, "name")?;
    clean_string_field(domain, "source_chain")?;
    clean_string_field(domain, "target_chain")?;
    clean_string_field(domain, "bridge_id")?;
    clean_string_field(domain, "door_account")?;
    nonzero_u64_field(domain, "inbound_cap")?;
    nonzero_u64_field(domain, "outbound_cap")?;
    u64_field(domain, "inbound_used")?;
    u64_field(domain, "outbound_used")?;
    bool_field(domain, "paused")?;
    Ok(())
}

fn validate_bridge_transfer_fields(transfer: &Value) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(transfer, "transfer_id", 96)?;
    clean_string_field(transfer, "domain_id")?;
    clean_string_field(transfer, "source_chain")?;
    clean_string_field(transfer, "target_chain")?;
    clean_string_field(transfer, "bridge_id")?;
    clean_string_field(transfer, "door_account")?;
    validate_bridge_direction(transfer, "direction")?;
    clean_string_field(transfer, "from")?;
    clean_string_field(transfer, "to")?;
    clean_string_field(transfer, "asset_id")?;
    nonzero_u64_field(transfer, "amount")?;
    clean_string_field(transfer, "witness_id")?;
    u64_field(transfer, "witness_epoch")?;
    u64_field(transfer, "sequence")?;
    validate_bridge_witness_attestation(field(transfer, "witness_attestation")?)?;
    Ok(())
}

fn validate_bridge_witness_attestation(
    attestation: &Value,
) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(attestation, "algorithm_id", "ML-DSA-65")?;
    lower_hex_field(attestation, "attestation_id", 96)?;
    clean_string_field(attestation, "chain_id")?;
    lower_hex_field(attestation, "genesis_hash", 96)?;
    nonzero_u32_field(attestation, "protocol_version")?;
    clean_string_field(attestation, "signer")?;
    lower_hex_string_field(attestation, "public_key_hex")?;
    lower_hex_string_field(attestation, "signature_hex")?;
    Ok(())
}

fn validate_bridge_direction(value: &Value, path: &str) -> Result<(), RpcResponseValidationError> {
    let direction = string_field(value, path)?;
    if !matches!(direction, "inbound" | "outbound") {
        return Err(invalid_result(
            path,
            format!("expected `inbound` or `outbound`, found `{direction}`"),
        ));
    }
    Ok(())
}

fn field<'a>(value: &'a Value, path: &str) -> Result<&'a Value, RpcResponseValidationError> {
    let mut current = value;
    for segment in path.split('.') {
        current = current
            .get(segment)
            .ok_or_else(|| invalid_result(path, format!("missing field segment `{segment}`")))?;
    }
    Ok(current)
}

fn string_field<'a>(value: &'a Value, path: &str) -> Result<&'a str, RpcResponseValidationError> {
    field(value, path)?
        .as_str()
        .ok_or_else(|| invalid_result(path, "expected string value"))
}

fn nonempty_string_field<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a str, RpcResponseValidationError> {
    let found = string_field(value, path)?;
    if found.trim().is_empty() {
        return Err(invalid_result(path, "expected nonempty string value"));
    }
    Ok(found)
}

fn clean_string_field<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a str, RpcResponseValidationError> {
    let found = nonempty_string_field(value, path)?;
    if found != found.trim() {
        return Err(invalid_result(
            path,
            "expected string without leading or trailing whitespace",
        ));
    }
    if found.chars().any(char::is_control) {
        return Err(invalid_result(
            path,
            "expected string without control characters",
        ));
    }
    Ok(found)
}

fn clean_string_field_allow_empty<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a str, RpcResponseValidationError> {
    let found = string_field(value, path)?;
    if found != found.trim() {
        return Err(invalid_result(
            path,
            "expected string without leading or trailing whitespace",
        ));
    }
    if found.chars().any(char::is_control) {
        return Err(invalid_result(
            path,
            "expected string without control characters",
        ));
    }
    Ok(found)
}

fn clean_string_entry<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a str, RpcResponseValidationError> {
    let found = value
        .as_str()
        .ok_or_else(|| invalid_result(path, "expected string entries"))?;
    if found.trim().is_empty() {
        return Err(invalid_result(path, "expected nonempty string entries"));
    }
    if found != found.trim() || found.chars().any(char::is_control) {
        return Err(invalid_result(
            path,
            "expected string entries without leading, trailing, or control whitespace",
        ));
    }
    Ok(found)
}

fn u64_field(value: &Value, path: &str) -> Result<u64, RpcResponseValidationError> {
    field(value, path)?
        .as_u64()
        .ok_or_else(|| invalid_result(path, "expected unsigned integer value"))
}

fn u64_field_default_zero(value: &Value, path: &str) -> Result<u64, RpcResponseValidationError> {
    let mut current = value;
    for segment in path.split('.') {
        match current.get(segment) {
            Some(next) => current = next,
            None => return Ok(0),
        }
    }
    current
        .as_u64()
        .ok_or_else(|| invalid_result(path, "expected unsigned integer value"))
}

fn i64_field(value: &Value, path: &str) -> Result<i64, RpcResponseValidationError> {
    field(value, path)?
        .as_i64()
        .ok_or_else(|| invalid_result(path, "expected signed integer value"))
}

fn optional_u64_result_field(value: &Value, path: &str) -> Result<(), RpcResponseValidationError> {
    optional_u64_field_value(value, path).map(|_| ())
}

fn optional_bool_result_field(value: &Value, path: &str) -> Result<(), RpcResponseValidationError> {
    let found = field(value, path)?;
    if found.is_null() || found.as_bool().is_some() {
        Ok(())
    } else {
        Err(invalid_result(path, "expected null or boolean value"))
    }
}

fn optional_clean_string_result_field(
    value: &Value,
    path: &str,
) -> Result<(), RpcResponseValidationError> {
    let found = field(value, path)?;
    if found.is_null() {
        Ok(())
    } else {
        clean_string_field(value, path).map(|_| ())
    }
}

fn optional_u64_field_value(
    value: &Value,
    path: &str,
) -> Result<Option<u64>, RpcResponseValidationError> {
    let found = field(value, path)?;
    if found.is_null() {
        Ok(None)
    } else if let Some(found) = found.as_u64() {
        Ok(Some(found))
    } else {
        Err(invalid_result(
            path,
            "expected null or unsigned integer value",
        ))
    }
}

fn optional_lower_hex_string_field_value(
    value: &Value,
    path: &str,
) -> Result<Option<String>, RpcResponseValidationError> {
    let Some(found) = value.get(path) else {
        return Ok(None);
    };
    if found.is_null() {
        return Ok(None);
    }
    let Some(found) = found.as_str() else {
        return Err(invalid_result(
            path,
            "expected null or lowercase hex string value",
        ));
    };
    if !is_lower_hex_string(found) {
        return Err(invalid_result(
            path,
            "expected nonempty lowercase hex string with even length",
        ));
    }
    Ok(Some(found.to_string()))
}

fn optional_lower_hex_len_field_value(
    value: &Value,
    path: &str,
    expected_len: usize,
) -> Result<Option<String>, RpcResponseValidationError> {
    let Some(found) = value.get(path) else {
        return Ok(None);
    };
    if found.is_null() {
        return Ok(None);
    }
    let Some(found) = found.as_str() else {
        return Err(invalid_result(
            path,
            "expected null or lowercase hex string value",
        ));
    };
    if !is_lower_hex_len(found, expected_len) {
        return Err(invalid_result(
            path,
            format!("expected {expected_len} lowercase hex characters"),
        ));
    }
    Ok(Some(found.to_string()))
}

fn nonzero_u64_field(value: &Value, path: &str) -> Result<u64, RpcResponseValidationError> {
    let found = u64_field(value, path)?;
    if found == 0 {
        return Err(invalid_result(
            path,
            "expected nonzero unsigned integer value",
        ));
    }
    Ok(found)
}

fn nonzero_u32_field(value: &Value, path: &str) -> Result<u32, RpcResponseValidationError> {
    let found = u64_field(value, path)?;
    if found == 0 || found > u64::from(u32::MAX) {
        return Err(invalid_result(
            path,
            "expected nonzero u32 protocol version",
        ));
    }
    Ok(found as u32)
}

fn bool_field(value: &Value, path: &str) -> Result<bool, RpcResponseValidationError> {
    field(value, path)?
        .as_bool()
        .ok_or_else(|| invalid_result(path, "expected boolean value"))
}

fn result_array<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a Vec<Value>, RpcResponseValidationError> {
    value
        .as_array()
        .ok_or_else(|| invalid_result(path, "expected array value"))
}

fn array_field<'a>(
    value: &'a Value,
    path: &str,
) -> Result<&'a Vec<Value>, RpcResponseValidationError> {
    field(value, path)?
        .as_array()
        .ok_or_else(|| invalid_result(path, "expected array value"))
}

fn lower_hex_field(
    value: &Value,
    path: &str,
    expected_len: usize,
) -> Result<(), RpcResponseValidationError> {
    let found = string_field(value, path)?;
    if !is_lower_hex_len(found, expected_len) {
        return Err(invalid_result(
            path,
            format!("expected {expected_len} lowercase hex characters"),
        ));
    }
    Ok(())
}

fn lower_hex_string_field(value: &Value, path: &str) -> Result<(), RpcResponseValidationError> {
    let found = string_field(value, path)?;
    if !is_lower_hex_string(found) {
        return Err(invalid_result(
            path,
            "expected nonempty lowercase hex string with even length",
        ));
    }
    Ok(())
}

fn optional_empty_string_field(
    value: &Value,
    path: &str,
) -> Result<(), RpcResponseValidationError> {
    let Some(found) = value.get(path) else {
        return Ok(());
    };
    let found = found
        .as_str()
        .ok_or_else(|| invalid_result(path, "expected string value"))?;
    if found.is_empty() {
        Ok(())
    } else {
        Err(invalid_result(
            path,
            "expected field to be omitted or an empty compact-certificate placeholder",
        ))
    }
}

fn expect_string_eq(
    value: &Value,
    path: &str,
    expected: &str,
) -> Result<(), RpcResponseValidationError> {
    let found = string_field(value, path)?;
    if found != expected {
        return Err(invalid_result(
            path,
            format!("expected `{expected}`, found `{found}`"),
        ));
    }
    Ok(())
}

fn expect_bool_eq(
    value: &Value,
    path: &str,
    expected: bool,
) -> Result<(), RpcResponseValidationError> {
    let found = field(value, path)?
        .as_bool()
        .ok_or_else(|| invalid_result(path, "expected boolean value"))?;
    if found != expected {
        return Err(invalid_result(
            path,
            format!("expected {expected}, found {found}"),
        ));
    }
    Ok(())
}

fn invalid_result(
    field: impl Into<String>,
    message: impl Into<String>,
) -> RpcResponseValidationError {
    RpcResponseValidationError::InvalidResult {
        field: field.into(),
        message: message.into(),
    }
}

fn reject_key_material_fields(
    value: &Value,
    message: &str,
) -> Result<(), RpcResponseValidationError> {
    if contains_key_material_field(value) {
        return Err(invalid_result("result", message));
    }
    Ok(())
}

fn reject_private_key_material_fields(
    value: &Value,
    message: &str,
) -> Result<(), RpcResponseValidationError> {
    if contains_private_key_material_field(value) {
        return Err(invalid_result("result", message));
    }
    Ok(())
}

fn is_lower_hex_len(value: &str, expected_len: usize) -> bool {
    value.len() == expected_len
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn is_lower_hex_string(value: &str) -> bool {
    !value.is_empty()
        && value.len().is_multiple_of(2)
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn contains_key_material_field(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields
            .iter()
            .any(|(key, value)| is_key_material_field(key) || contains_key_material_field(value)),
        Value::Array(values) => values.iter().any(contains_key_material_field),
        _ => false,
    }
}

fn contains_private_key_material_field(value: &Value) -> bool {
    match value {
        Value::Object(fields) => fields.iter().any(|(key, value)| {
            is_private_key_material_field(key) || contains_private_key_material_field(value)
        }),
        Value::Array(values) => values.iter().any(contains_private_key_material_field),
        _ => false,
    }
}

fn is_key_material_field(key: &str) -> bool {
    matches!(
        key,
        "private_key_hex"
            | "public_key_hex"
            | "master_seed_hex"
            | "spending_key_hex"
            | "full_viewing_key_hex"
            | "rseed"
    )
}

fn is_private_key_material_field(key: &str) -> bool {
    matches!(
        key,
        "private_key_hex"
            | "master_seed_hex"
            | "spending_key_hex"
            | "full_viewing_key_hex"
            | "rseed"
    )
}

fn invalid_protocol_data(error: RpcProtocolError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn invalid_protocol_input(error: RpcProtocolError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}

fn invalid_response_validation(error: RpcResponseValidationError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

fn invalid_request_validation(error: RpcRequestValidationError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}
