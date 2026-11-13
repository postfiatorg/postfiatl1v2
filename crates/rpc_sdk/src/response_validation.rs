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

    validate_block_log_verification_section(result, "block_log")?;

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

    validate_bridge_verification_section(result, "bridge")?;
    validate_shielded_verification_section(result, "shielded")?;
    validate_mempool_verification_section(result, "mempool")?;
    Ok(())
}

/// `verify_blocks` returns the same report `verify_state` nests as `block_log`.
fn validate_block_log_verification_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    validate_block_log_verification_section(result, "")
}

/// `verify_bridge` returns the same report `verify_state` nests as `bridge`.
fn validate_bridge_verification_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    validate_bridge_verification_section(result, "")
}

/// `verify_mempool` returns the same report `verify_state` nests as `mempool`.
fn validate_mempool_verification_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    validate_mempool_verification_section(result, "")
}

/// `verify_shielded` returns the same report `verify_state` nests as `shielded`.
fn validate_shielded_verification_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    validate_shielded_verification_section(result, "")
}

/// Field path of `field` inside the `prefix` section, or `field` itself for a
/// standalone report (`prefix == ""`).
fn section_path(prefix: &str, field: &str) -> String {
    if prefix.is_empty() {
        field.to_string()
    } else {
        format!("{prefix}.{field}")
    }
}

fn validate_block_log_verification_section(
    result: &Value,
    prefix: &str,
) -> Result<(), RpcResponseValidationError> {
    let path = |field: &str| section_path(prefix, field);
    expect_bool_eq(result, &path("verified"), true)?;
    let block_count = u64_field(result, &path("block_count"))?;
    validate_block_tip_hash(
        block_count,
        string_field(result, &path("tip_hash"))?,
        &path("tip_hash"),
    )?;
    lower_hex_field(result, &path("state_root"), 96)?;
    Ok(())
}

fn validate_bridge_verification_section(
    result: &Value,
    prefix: &str,
) -> Result<(), RpcResponseValidationError> {
    let path = |field: &str| section_path(prefix, field);
    expect_bool_eq(result, &path("verified"), true)?;
    u64_field(result, &path("domain_count"))?;
    let bridge_transfer_count = u64_field(result, &path("transfer_count"))?;
    let bridge_attestation_count = u64_field(result, &path("attestation_count"))?;
    if bridge_attestation_count > bridge_transfer_count {
        return Err(invalid_result(
            path("attestation_count"),
            "expected no more attestations than transfers",
        ));
    }
    let bridge_replay_cache_count = u64_field(result, &path("replay_cache_count"))?;
    if bridge_replay_cache_count > bridge_transfer_count {
        return Err(invalid_result(
            path("replay_cache_count"),
            "expected no more replay cache entries than transfers",
        ));
    }
    u64_field(result, &path("inbound_used"))?;
    u64_field(result, &path("outbound_used"))?;
    validate_optional_hash_for_count(
        bridge_transfer_count,
        string_field(result, &path("latest_transfer_id"))?,
        &path("latest_transfer_id"),
    )?;
    Ok(())
}

fn validate_shielded_verification_section(
    result: &Value,
    prefix: &str,
) -> Result<(), RpcResponseValidationError> {
    let path = |field: &str| section_path(prefix, field);
    expect_bool_eq(result, &path("verified"), true)?;
    let note_count = u64_field(result, &path("note_count"))?;
    u64_field(result, &path("nullifier_count"))?;
    let turnstile_event_count = u64_field(result, &path("turnstile_event_count"))?;
    lower_hex_field(result, &path("tree_root"), 96)?;
    u64_field(result, &path("bootstrap_deposit_total"))?;
    u64_field(result, &path("migration_total"))?;
    u64_field(result, &path("orchard_deposit_total"))?;
    let spent_note_count = u64_field(result, &path("spent_note_count"))?;
    let live_note_count = u64_field(result, &path("live_note_count"))?;
    match spent_note_count.checked_add(live_note_count) {
        Some(total) if total == note_count => {}
        _ => {
            return Err(invalid_result(
                path("live_note_count"),
                "expected spent_note_count plus live_note_count to equal note_count",
            ));
        }
    }
    validate_optional_hash_for_count(
        turnstile_event_count,
        string_field(result, &path("latest_turnstile_event_id"))?,
        &path("latest_turnstile_event_id"),
    )?;
    Ok(())
}

fn validate_mempool_verification_section(
    result: &Value,
    prefix: &str,
) -> Result<(), RpcResponseValidationError> {
    let path = |field: &str| section_path(prefix, field);
    expect_bool_eq(result, &path("verified"), true)?;
    let pending_count = u64_field(result, &path("pending_count"))?;
    let sender_count = u64_field(result, &path("sender_count"))?;
    if pending_count
        .checked_mul(2)
        .is_some_and(|sender_count_limit| sender_count > sender_count_limit)
    {
        return Err(invalid_result(
            path("sender_count"),
            "expected no more than two senders per pending transaction",
        ));
    }
    u64_field(result, &path("total_amount"))?;
    u64_field(result, &path("total_fee"))?;
    validate_optional_hash_for_count(
        pending_count,
        string_field(result, &path("latest_tx_id"))?,
        &path("latest_tx_id"),
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
    // Present only when nonzero (`account_set`), e.g. 1 = require memo.
    if result.get("flags").is_some() {
        let flags = u64_field(result, "flags")?;
        if flags == 0 || flags & !u64::from(postfiat_types::ACCOUNT_ALLOWED_FLAGS) != 0 {
            return Err(invalid_result(
                "flags",
                "expected nonzero flags within the supported account flag mask",
            ));
        }
    }
    Ok(())
}

/// `account_tx_index_status` is the operator's index-freshness read. The checks
/// mirror how the node builds `AccountTxIndexStatusReport` (an absent index file
/// yields `index_present: false`, zero counts, no heights and an empty
/// `tip_hash`; `index_usable` is exactly "no reason") and the Python client's
/// refusal to accept a leaked filesystem path.
fn validate_account_tx_index_status_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ACCOUNT_TX_INDEX_STATUS_SCHEMA)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;

    validate_index_file_name(clean_string_field(result, "index_path")?, "index_path")?;
    validate_index_file_name(
        clean_string_field_allow_empty(result, "disk_index_path")?,
        "disk_index_path",
    )?;

    let index_present = bool_field(result, "index_present")?;
    let index_usable = bool_field(result, "index_usable")?;
    let reason = optional_reason_field(result, "reason")?;
    if index_usable && !index_present {
        return Err(invalid_result(
            "index_usable",
            "expected an absent index to be unusable",
        ));
    }
    if index_usable == reason.is_some() {
        return Err(invalid_result(
            "reason",
            "expected a reason exactly when the index is unusable",
        ));
    }

    let disk_index_present = bool_field(result, "disk_index_present")?;
    let disk_index_usable = bool_field(result, "disk_index_usable")?;
    let disk_index_reason = optional_reason_field(result, "disk_index_reason")?;
    if disk_index_usable && !disk_index_present {
        return Err(invalid_result(
            "disk_index_usable",
            "expected an absent disk index to be unusable",
        ));
    }
    if disk_index_usable == disk_index_reason.is_some() {
        return Err(invalid_result(
            "disk_index_reason",
            "expected a reason exactly when the disk index is unusable",
        ));
    }

    let indexed_from_height = optional_u64_field_value(result, "indexed_from_height")?;
    let indexed_to_height = optional_u64_field_value(result, "indexed_to_height")?;
    let indexed_block_count = u64_field(result, "indexed_block_count")?;
    let indexed_row_count = u64_field(result, "indexed_row_count")?;
    let account_count = u64_field(result, "account_count")?;
    u64_field(result, "disk_account_shard_count")?;
    let tip_hash = string_field(result, "tip_hash")?;
    if index_present {
        // The node sets both heights per indexed block (`get_or_insert` for the
        // first, `Some` for the latest) or leaves both unset for an empty index.
        match (indexed_from_height, indexed_to_height) {
            (Some(from_height), Some(to_height)) if from_height > to_height => {
                return Err(invalid_result(
                    "indexed_to_height",
                    "expected indexed_to_height to be at least indexed_from_height",
                ));
            }
            (Some(_), None) | (None, Some(_)) => {
                return Err(invalid_result(
                    "indexed_to_height",
                    "expected indexed_from_height and indexed_to_height to be set together",
                ));
            }
            _ => {}
        }
        if tip_hash != "genesis" && !is_lower_hex_len(tip_hash, 96) {
            return Err(invalid_result(
                "tip_hash",
                "expected `genesis` or 96 lowercase hex characters",
            ));
        }
    } else {
        if indexed_from_height.is_some() || indexed_to_height.is_some() {
            return Err(invalid_result(
                "indexed_from_height",
                "expected no indexed heights when the index is absent",
            ));
        }
        if indexed_block_count != 0 || indexed_row_count != 0 || account_count != 0 {
            return Err(invalid_result(
                "indexed_block_count",
                "expected zero index counts when the index is absent",
            ));
        }
        if !tip_hash.is_empty() {
            return Err(invalid_result(
                "tip_hash",
                "expected an empty tip_hash when the index is absent",
            ));
        }
    }
    let current_tip_hash = string_field(result, "current_tip_hash")?;
    if current_tip_hash != "genesis" && !is_lower_hex_len(current_tip_hash, 96) {
        return Err(invalid_result(
            "current_tip_hash",
            "expected `genesis` or 96 lowercase hex characters",
        ));
    }
    Ok(())
}

/// The node reports index files by bare file name; a separator means a
/// filesystem path leaked into the public read.
fn validate_index_file_name(value: &str, field: &str) -> Result<(), RpcResponseValidationError> {
    if value.contains('/') || value.contains('\\') {
        return Err(invalid_result(field, "expected a bare file name, not a path"));
    }
    Ok(())
}

fn optional_reason_field<'a>(
    value: &'a Value,
    path: &str,
) -> Result<Option<&'a str>, RpcResponseValidationError> {
    let found = field(value, path)?;
    if found.is_null() {
        return Ok(None);
    }
    match found.as_str() {
        Some(reason) if !reason.trim().is_empty() => Ok(Some(reason)),
        _ => Err(invalid_result(path, "expected null or a non-empty reason")),
    }
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
        validate_account_tx_row(row, Some(address))?;
    }
    Ok(())
}

/// `transaction_kind` values of `account_tx` rows built from shielded blocks:
/// shielded-pool exits that credit the row's transparent `to` account.
const ACCOUNT_TX_SHIELDED_WITHDRAWAL_KINDS: [&str; 3] = [
    "orchard_withdraw_v1",
    "asset_orchard_egress_v1",
    "asset_orchard_private_egress_v1",
];
const ACCOUNT_TX_ROLE_SHIELDED_WITHDRAWAL: &str = "shielded_withdrawal";

fn is_account_tx_shielded_withdrawal_kind(kind: &str) -> bool {
    ACCOUNT_TX_SHIELDED_WITHDRAWAL_KINDS.contains(&kind)
}

/// One `account_tx` row, as returned by `account_tx` and in `tx`
/// `transaction_rows`. With `address`, the row must involve that account.
fn validate_account_tx_row(
    row: &Value,
    address: Option<&str>,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(row, "tx_id", 96)?;
    nonzero_u64_field(row, "block_height")?;
    let batch_kind = clean_string_field(row, "batch_kind")?;
    lower_hex_field(row, "batch_id", 96)?;
    u64_field(row, "transaction_index")?;
    let row_kind = row
        .get("transaction_kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match batch_kind {
        "transparent" => {
            if is_account_tx_shielded_withdrawal_kind(row_kind) {
                return Err(invalid_result(
                    "transaction_kind",
                    "unexpected shielded withdrawal on a transparent account_tx row",
                ));
            }
        }
        "shielded" => {
            if !is_account_tx_shielded_withdrawal_kind(row_kind) {
                return Err(invalid_result(
                    "transaction_kind",
                    "expected a shielded withdrawal kind on a shielded account_tx row",
                ));
            }
            if row.get("tx_role").and_then(Value::as_str)
                != Some(ACCOUNT_TX_ROLE_SHIELDED_WITHDRAWAL)
            {
                return Err(invalid_result(
                    "tx_role",
                    "expected shielded_withdrawal tx_role on a shielded account_tx row",
                ));
            }
        }
        _ => {
            return Err(invalid_result(
                "batch_kind",
                "expected transparent or shielded account_tx row",
            ));
        }
    }
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
            && !is_account_tx_shielded_withdrawal_kind(transaction_kind)
        {
            return Err(invalid_result(
                "transaction_kind",
                "expected transparent_transfer, payment_v2, supported asset transaction kind, supported escrow transaction kind, supported nft transaction kind, supported offer transaction kind, or shielded withdrawal kind",
            ));
        }
    }
    let from = clean_string_field(row, "from")?;
    let to = clean_string_field(row, "to")?;
    if let Some(address) = address {
        if from != address && to != address {
            return Err(invalid_result(
                "rows",
                "expected every account_tx row to involve the requested address",
            ));
        }
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
    validate_account_tx_row_memos(row, row_kind)?;
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
            && transaction_kind != "asset_orchard_egress_v1"
            && transaction_kind != "asset_orchard_private_egress_v1"
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
        } else if is_account_tx_shielded_withdrawal_kind(transaction_kind) {
            if tx_role != ACCOUNT_TX_ROLE_SHIELDED_WITHDRAWAL {
                return Err(invalid_result(
                    "tx_role",
                    "expected shielded_withdrawal tx_role on a shielded withdrawal row",
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
    Ok(())
}

/// `memos` is present on native and issued `payment_v2` rows: signed lowercase-hex
/// memo fields plus optional UTF-8 `*_text` readings, one entry per memo.
fn validate_account_tx_row_memos(
    row: &Value,
    transaction_kind: &str,
) -> Result<(), RpcResponseValidationError> {
    let Some(memos) = row.get("memos") else {
        return Ok(());
    };
    if !matches!(transaction_kind, PAYMENT_V2_TRANSACTION_KIND | postfiat_types::ISSUED_PAYMENT_V2_TRANSACTION_KIND) {
        return Err(invalid_result(
            "memos",
            "unexpected memos on a payment without memo support",
        ));
    }
    let memos = memos
        .as_array()
        .ok_or_else(|| invalid_result("memos", "expected array value"))?;
    if let Some(memo_count) = row.get("memo_count").and_then(Value::as_u64) {
        if memo_count != memos.len() as u64 {
            return Err(invalid_result(
                "memos",
                "expected memos length to match memo_count",
            ));
        }
    }
    for memo in memos {
        for key in ["memo_type", "memo_format", "memo_data"] {
            let value = string_field(memo, key)?;
            if !value.is_empty() && !is_lower_hex_string(value) {
                return Err(invalid_result(
                    key,
                    "expected empty or lowercase hex memo field",
                ));
            }
        }
        for key in ["memo_type_text", "memo_format_text", "memo_data_text"] {
            if let Some(text) = memo.get(key) {
                if !text.is_string() {
                    return Err(invalid_result(key, "expected string value"));
                }
            }
        }
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

/// `nav_reserve_proof_status` mirrors the node's construction: the asset's
/// registered NAV proof profile (when the asset exists and its profile id is
/// registered) plus that asset's reserve packets, newest first by
/// (epoch, submitted_at_height, packet_id) and bounded to the latest sixteen.
fn validate_nav_reserve_proof_status_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", NAV_RESERVE_PROOF_STATUS_SCHEMA)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    nonzero_u32_field(result, "protocol_version")?;
    let current_height = u64_field(result, "current_height")?;
    lower_hex_field(result, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;

    let found = bool_field(result, "found")?;
    let active_profile = field(result, "active_profile")?;
    if !found && !active_profile.is_null() {
        return Err(invalid_result(
            "active_profile",
            "expected no active profile when the asset is not found",
        ));
    }
    // `found` with a null profile is legitimate: the asset's profile id may
    // not (or no longer) resolve to a registered profile.
    if !active_profile.is_null() {
        validate_nav_proof_profile(active_profile)?;
    }

    let packets = array_field(result, "packets")?;
    if packets.len() > NAV_RESERVE_PROOF_STATUS_MAX_PACKETS {
        return Err(invalid_result(
            "packets",
            format!("expected at most {NAV_RESERVE_PROOF_STATUS_MAX_PACKETS} packets"),
        ));
    }
    if !found && !packets.is_empty() {
        return Err(invalid_result(
            "packets",
            "expected no packets when the asset is not found",
        ));
    }
    let mut previous: Option<(u64, u64, String)> = None;
    for packet in packets {
        let key = validate_nav_reserve_packet_status(packet, current_height)?;
        if let Some(previous) = &previous {
            if key >= *previous {
                return Err(invalid_result(
                    "packets",
                    "expected packets newest first by epoch, submitted_at_height and packet_id",
                ));
            }
        }
        previous = Some(key);
    }
    Ok(())
}

fn validate_nav_proof_profile(profile: &Value) -> Result<(), RpcResponseValidationError> {
    if !profile.is_object() {
        return Err(invalid_result("active_profile", "expected object value"));
    }
    clean_string_field(profile, "profile_id")?;
    clean_string_field(profile, "registered_by")?;
    clean_string_field(profile, "verifier_kind")?;
    clean_string_field_allow_empty(profile, "source_class")?;
    u64_field(profile, "max_snapshot_age_blocks")?;
    u64_field(profile, "challenge_window_blocks")?;
    u64_field(profile, "max_epoch_gap_blocks")?;
    u64_field(profile, "settle_deadline_blocks")?;
    u64_field(profile, "min_challenge_bond")?;
    Ok(())
}

/// Validates one packet row and returns its ordering key
/// (epoch, submitted_at_height, packet_id).
fn validate_nav_reserve_packet_status(
    packet: &Value,
    current_height: u64,
) -> Result<(u64, u64, String), RpcResponseValidationError> {
    if !packet.is_object() {
        return Err(invalid_result("packets", "expected packet object"));
    }
    let packet_id = clean_string_field(packet, "packet_id")?.to_string();
    let epoch = u64_field(packet, "epoch")?;
    clean_string_field(packet, "state")?;
    optional_hex96_field(packet, "reserve_packet_hash")?;
    u64_field(packet, "nav_per_unit")?;
    u64_field(packet, "circulating_supply")?;
    u64_field(packet, "verified_net_assets")?;
    let submitted_at_height = u64_field(packet, "submitted_at_height")?;
    if submitted_at_height > current_height {
        return Err(invalid_result(
            "submitted_at_height",
            "expected submitted_at_height at most current_height",
        ));
    }
    let not_before = u64_field(packet, "observation_not_before")?;
    let not_after = u64_field(packet, "observation_not_after")?;
    if not_before > not_after {
        return Err(invalid_result(
            "observation_not_after",
            "expected observation_not_after at least observation_not_before",
        ));
    }
    u64_field(packet, "proof_verified_net_assets")?;
    u64_field(packet, "consensus_overlay_value")?;
    u64_field(packet, "gross_assets")?;
    u64_field(packet, "total_liabilities")?;
    u64_field(packet, "cryptographically_verified_value")?;
    u64_field(packet, "attested_value")?;
    u64_field(packet, "controlled_value")?;
    u32_range_field(packet, "source_count")?;
    validate_nav_reserve_trust_counts(packet, "quantity_trust_counts")?;
    validate_nav_reserve_trust_counts(packet, "valuation_trust_counts")?;
    optional_hex96_field(packet, "quantity_trust_root")?;
    optional_hex96_field(packet, "valuation_trust_root")?;
    optional_hex96_field(packet, "source_disclosure_root")?;
    Ok((epoch, submitted_at_height, packet_id))
}

fn validate_nav_reserve_trust_counts(
    packet: &Value,
    path: &str,
) -> Result<(), RpcResponseValidationError> {
    let counts = field(packet, path)?;
    if !counts.is_object() {
        return Err(invalid_result(path, "expected object value"));
    }
    let cryptographic = u32_range_field(counts, "cryptographic")?;
    let attested = u32_range_field(counts, "attested")?;
    let controlled = u32_range_field(counts, "controlled")?;
    if cryptographic
        .checked_add(attested)
        .and_then(|total| total.checked_add(controlled))
        .is_none()
    {
        return Err(invalid_result(path, "expected trust counts to fit a u32 total"));
    }
    Ok(())
}

fn u32_range_field(value: &Value, path: &str) -> Result<u32, RpcResponseValidationError> {
    u32::try_from(u64_field(value, path)?).map_err(|_| invalid_result(path, "expected u32 value"))
}

/// A hash field the node may leave empty: empty, or 96 lowercase hex characters.
fn optional_hex96_field(value: &Value, path: &str) -> Result<(), RpcResponseValidationError> {
    let found = clean_string_field_allow_empty(value, path)?;
    if !found.is_empty() && !is_lower_hex_len(found, 96) {
        return Err(invalid_result(path, "expected empty or 96 lowercase hex characters"));
    }
    Ok(())
}

/// `vault_bridge_route` mirrors the node's construction in market_bridge.rs:
/// the governed route profile active at the tip (so it passes the profile's
/// own `validate()` rules and `activation_height <= current_height <
/// expires_at_height`), its authorizing amendment (whose value is the route
/// epoch and whose activation height is the profile's), the route binding
/// derived from (profile_hash, route_epoch), the NAV proof profile summary,
/// and the trust class implied by the evidence tier.
fn validate_vault_bridge_route_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", VAULT_BRIDGE_ROUTE_REPORT_SCHEMA)?;
    clean_string_field(result, "chain_id")?;
    lower_hex_field(result, "genesis_hash", 96)?;
    let current_height = u64_field(result, "current_height")?;

    let profile = field(result, "profile")?;
    if !profile.is_object() {
        return Err(invalid_result("profile", "expected object value"));
    }
    let (route_epoch, activation_height, evidence_tier) = validate_vault_bridge_route_profile(profile)?;
    let expires_at_height = u64_field(profile, "expires_at_height")?;
    if activation_height > current_height {
        return Err(invalid_result(
            "profile.activation_height",
            "expected the active route to be activated at or before current_height",
        ));
    }
    if expires_at_height <= current_height {
        return Err(invalid_result(
            "profile.expires_at_height",
            "expected the active route to expire after current_height",
        ));
    }

    lower_hex_field(result, "profile_hash", postfiat_types::VAULT_BRIDGE_HEX_HASH_LEN)?;
    let profile_hash = string_field(result, "profile_hash")?;
    let expected_binding = postfiat_types::vault_bridge_route_binding(profile_hash, route_epoch)
        .map_err(|error| invalid_result("route_binding", error))?;
    expect_string_eq(result, "route_binding", &expected_binding)?;

    lower_hex_field(result, "governance_amendment_id", 96)?;
    let governance_activation_height = u64_field(result, "governance_activation_height")?;
    if governance_activation_height != activation_height {
        return Err(invalid_result(
            "governance_activation_height",
            "expected the authorizing amendment to activate with the route profile",
        ));
    }
    let governance_route_epoch = u64_field(result, "governance_route_epoch")?;
    if governance_route_epoch != u64::from(route_epoch) {
        return Err(invalid_result(
            "governance_route_epoch",
            "expected the authorizing amendment value to be the route epoch",
        ));
    }

    clean_string_field(result, "nav_profile_id")?;
    clean_string_field_allow_empty(result, "nav_profile_source_class")?;
    clean_string_field(result, "nav_profile_verifier_kind")?;
    let policy_hash = clean_string_field_allow_empty(result, "nav_profile_policy_hash")?;
    if !policy_hash.is_empty() && !is_lower_hex(policy_hash) {
        return Err(invalid_result(
            "nav_profile_policy_hash",
            "expected empty or lowercase hex characters",
        ));
    }

    let expected_trust_class = if evidence_tier == postfiat_types::VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN {
        postfiat_types::VAULT_BRIDGE_ROUTE_TRUST_CLASS_TRUSTLESS_FINALITY
    } else {
        postfiat_types::VAULT_BRIDGE_ROUTE_TRUST_CLASS_CONTROLLED
    };
    expect_string_eq(result, "route_trust_class", expected_trust_class)?;
    bool_field(result, "live_value_enabled")?;
    expect_bool_eq(result, "active", true)?;
    Ok(())
}

/// The profile's own `validate()` rules (crates/types VaultBridgeRouteProfileV1),
/// re-stated over JSON. Returns (route_epoch, activation_height, evidence_tier).
fn validate_vault_bridge_route_profile(
    profile: &Value,
) -> Result<(u32, u64, String), RpcResponseValidationError> {
    expect_string_eq(profile, "schema", postfiat_types::VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1)?;
    let route_id = clean_string_field(profile, "route_id")?;
    if route_id.len() > 64
        || !route_id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(invalid_result(
            "profile.route_id",
            "expected 1..64 lowercase identifier bytes",
        ));
    }
    lower_hex_field(profile, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    nonzero_u64_field(profile, "source_chain_id")?;
    validate_nonzero_evm_address_field(profile, "vault_address")?;
    validate_nonzero_evm_address_field(profile, "token_address")?;
    if string_field(profile, "vault_address")? == postfiat_types::RETIRED_VAULT_BRIDGE_ADDRESS {
        return Err(invalid_result("profile.vault_address", "expected a vault that is not retired"));
    }
    validate_prefixed_hash_field(profile, "vault_runtime_code_hash", 64)?;
    validate_prefixed_hash_field(profile, "token_runtime_code_hash", 64)?;
    let route_epoch = nonzero_u32_field(profile, "route_epoch")?;
    let verifier_kind = clean_string_field(profile, "verifier_kind")?;
    let evidence_tier = clean_string_field(profile, "evidence_tier")?.to_string();
    let min_attestations = u64_field(profile, "min_attestations")?;
    let minimum_confirmations = u64_field(profile, "minimum_confirmations")?;
    let verifier_policy_hash = clean_string_field_allow_empty(profile, "verifier_policy_hash")?;
    let verifier_program_vkey = clean_string_field_allow_empty(profile, "verifier_program_vkey")?;
    let verifier_proof_encoding = clean_string_field_allow_empty(profile, "verifier_proof_encoding")?;
    let max_proof_bytes = u64_field(profile, "max_proof_bytes")?;
    let max_public_values_bytes = u64_field(profile, "max_public_values_bytes")?;
    let expected_tier = match verifier_kind {
        postfiat_types::NAV_PROFILE_VERIFIER_MULTI_FETCH => {
            if min_attestations == 0 || minimum_confirmations == 0 {
                return Err(invalid_result(
                    "profile.min_attestations",
                    "expected nonzero attestation and confirmation thresholds for an independently observed route",
                ));
            }
            if !verifier_policy_hash.is_empty()
                || !verifier_program_vkey.is_empty()
                || !verifier_proof_encoding.is_empty()
                || max_proof_bytes != 0
                || max_public_values_bytes != 0
            {
                return Err(invalid_result(
                    "profile.verifier_policy_hash",
                    "expected no proof-verifier fields on an independently observed route",
                ));
            }
            postfiat_types::VAULT_BRIDGE_EVIDENCE_TIER_INDEPENDENTLY_OBSERVED
        }
        postfiat_types::NAV_PROFILE_VERIFIER_SP1_GROTH16
        | postfiat_types::NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
        | postfiat_types::NAV_PROFILE_VERIFIER_SP1_ARBITRUM_BONDED_V1
        | postfiat_types::NAV_PROFILE_VERIFIER_SP1_ARC_FINALITY_V1 => {
            if min_attestations != 0 || minimum_confirmations != 0 {
                return Err(invalid_result(
                    "profile.min_attestations",
                    "expected no observer thresholds on a receipt-proven route",
                ));
            }
            if !is_lower_hex_len(verifier_policy_hash, postfiat_types::NAV_SP1_POLICY_HASH_HEX_LEN) {
                return Err(invalid_result(
                    "profile.verifier_policy_hash",
                    "expected a lowercase 32-byte hex policy hash",
                ));
            }
            if verifier_program_vkey.len() != postfiat_types::NAV_SP1_PROGRAM_VKEY_HEX_LEN
                || !verifier_program_vkey.starts_with("0x")
                || !verifier_program_vkey[2..].bytes().all(|byte| byte.is_ascii_hexdigit())
            {
                return Err(invalid_result(
                    "profile.verifier_program_vkey",
                    "expected a 0x-prefixed 32-byte hex program vkey",
                ));
            }
            if verifier_proof_encoding != postfiat_types::NAV_SP1_PROOF_ENCODING_GROTH16 {
                return Err(invalid_result(
                    "profile.verifier_proof_encoding",
                    "expected the groth16 proof encoding",
                ));
            }
            postfiat_types::VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN
        }
        _ => {
            return Err(invalid_result("profile.verifier_kind", "expected a supported verifier kind"));
        }
    };
    if evidence_tier != expected_tier {
        return Err(invalid_result(
            "profile.evidence_tier",
            "expected the evidence tier implied by verifier_kind",
        ));
    }
    nonzero_u64_field(profile, "max_snapshot_age_blocks")?;
    nonzero_u64_field(profile, "challenge_window_blocks")?;
    nonzero_u64_field(profile, "max_epoch_gap_blocks")?;
    nonzero_u64_field(profile, "settle_deadline_blocks")?;
    u64_field(profile, "min_challenge_bond")?;
    let activation_height = nonzero_u64_field(profile, "activation_height")?;
    let expires_at_height = u64_field(profile, "expires_at_height")?;
    if expires_at_height <= activation_height {
        return Err(invalid_result(
            "profile.expires_at_height",
            "expected expiry to follow the activation height",
        ));
    }
    Ok((route_epoch, activation_height, evidence_tier))
}

/// The route profile's rule: a lowercase, non-zero EVM address (the generic
/// `validate_evm_address_field` accepts mixed case and the zero address).
fn validate_nonzero_evm_address_field(
    value: &Value,
    path: &str,
) -> Result<(), RpcResponseValidationError> {
    validate_evm_address_field(value, path)?;
    let address = string_field(value, path)?;
    let hex = &address[2..];
    if hex.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(invalid_result(format!("profile.{path}"), "expected a lowercase EVM address"));
    }
    if hex.bytes().all(|byte| byte == b'0') {
        return Err(invalid_result(format!("profile.{path}"), "expected a non-zero EVM address"));
    }
    Ok(())
}

/// A 0x-prefixed lowercase hash of `hex_len` hex characters that is not all zero.
fn validate_prefixed_hash_field(
    value: &Value,
    path: &str,
    hex_len: usize,
) -> Result<(), RpcResponseValidationError> {
    let hash = string_field(value, path)?;
    let Some(hex) = hash.strip_prefix("0x") else {
        return Err(invalid_result(path, "expected a 0x-prefixed lowercase hash"));
    };
    if !is_lower_hex_len(hex, hex_len) {
        return Err(invalid_result(path, "expected a 0x-prefixed lowercase hash"));
    }
    if hex.bytes().all(|byte| byte == b'0') {
        return Err(invalid_result(path, "expected a non-zero hash"));
    }
    Ok(())
}

fn is_lower_hex(value: &str) -> bool {
    !value.is_empty()
        && value.len() % 2 == 0
        && value
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_asset_info_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ASSET_INFO_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    lower_hex_field(result, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    let found = bool_field(result, "found")?;
    let asset = field(result, "asset")?;
    match (found, asset.is_null()) {
        (true, false) => {
            validate_issued_asset_result(asset)?;
            if string_field(asset, "asset_id")? != string_field(result, "asset_id")? {
                return Err(invalid_result(
                    "asset",
                    "expected asset object to match requested asset_id",
                ));
            }
            Ok(())
        }
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
        (true, false) => {
            validate_escrow_result(escrow)?;
            if string_field(escrow, "escrow_id")? != string_field(result, "escrow_id")? {
                return Err(invalid_result(
                    "escrow",
                    "expected escrow object to match requested escrow_id",
                ));
            }
            Ok(())
        }
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
    if let Some(rows) = result.get("transaction_rows") {
        let rows = rows
            .as_array()
            .ok_or_else(|| invalid_result("transaction_rows", "expected array value"))?;
        for row in rows {
            validate_account_tx_row(row, None)?;
            if string_field(row, "tx_id")? != tx_id
                || nonzero_u64_field(row, "block_height")? != block_height
            {
                return Err(invalid_result(
                    "transaction_rows",
                    "expected every transaction row to belong to the finality tx_id and block",
                ));
            }
        }
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
    let typed_operation: AssetTransactionOperation = serde_json::from_value(value.clone())
        .map_err(|_| invalid_result("operation", "expected valid asset operation"))?;
    if typed_operation.transaction_kind() != expected_kind {
        return Err(invalid_result(
            "operation",
            "expected operation to match transaction kind",
        ));
    }
    typed_operation
        .validate()
        .map_err(|_| invalid_result("operation", "expected valid asset operation"))?;
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
        postfiat_types::ISSUED_PAYMENT_V2_TRANSACTION_KIND => {
            clean_string_field(value, "from")?;
            clean_string_field(value, "to")?;
            clean_string_field(value, "issuer")?;
            lower_hex_field(value, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
            nonzero_u64_field(value, "amount")?;
        }
        postfiat_types::ACCOUNT_SET_TRANSACTION_KIND => {
            clean_string_field(value, "account")?;
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
        MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND
        | MARKET_OPS_FINALIZE_TRANSACTION_KIND
        | FX_FIX_REGISTER_TRANSACTION_KIND_V1
        | FX_FIX_PAUSE_TRANSACTION_KIND_V1
        | FX_FIX_RESERVATION_CREATE_TRANSACTION_KIND_V1
        | FX_FIX_RESERVATION_RELEASE_TRANSACTION_KIND_V1
        | VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND
        | VAULT_BRIDGE_DEPOSIT_CHALLENGE_TRANSACTION_KIND
        | VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND
        | VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND
        | VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND
        | VAULT_BRIDGE_FAST_INGRESS_LIFECYCLE_TRANSACTION_KIND
        | VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND
        | VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND
        | VAULT_BRIDGE_MINT_FROM_RECEIPTS_TRANSACTION_KIND
        | VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND
        | VAULT_BRIDGE_REDEEM_SETTLE_TRANSACTION_KIND
        | VAULT_BRIDGE_BUCKET_IMPAIR_TRANSACTION_KIND
        | VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND
        | PFTL_UNISWAP_ROUTE_INIT_V2_TRANSACTION_KIND
        | PFTL_UNISWAP_ORDER_RESERVE_TRANSACTION_KIND
        | PFTL_UNISWAP_ORDER_RELEASE_TRANSACTION_KIND
        | PFTL_UNISWAP_PRIMARY_SUBSCRIBE_V2_TRANSACTION_KIND
        | PFTL_UNISWAP_REDEMPTION_FUND_TRANSACTION_KIND
        | PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND
        | PFTL_UNISWAP_ROUTE_EPOCH_ADVANCE_TRANSACTION_KIND
        | PFTL_UNISWAP_ROUTE_PAUSE_TRANSACTION_KIND
        | postfiat_types::EXTERNAL_LANE_RELEASE_TRANSACTION_KIND_V1
        | postfiat_types::EXTERNAL_LANE_EXPORT_TRANSACTION_KIND_V1 => {}
        PFTL_UNISWAP_ROUTE_BRIDGE_POLICY_UPDATE_TRANSACTION_KIND => {
            clean_string_field(value, "issuer")?;
            clean_string_field(value, "route_id")?;
            nonzero_u64_field(value, "authority_epoch")?;
            lower_hex_field(value, "committee_root", 96)?;
            nonzero_u64_field(value, "minimum_confirmations")?;
            lower_hex_field(value, "handoff_controller_code_hash", 64)?;
            lower_hex_field(value, "wrapped_navcoin_code_hash", 64)?;
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
            | postfiat_types::ISSUED_PAYMENT_V2_TRANSACTION_KIND
            | postfiat_types::ACCOUNT_SET_TRANSACTION_KIND
            | ASSET_BURN_TRANSACTION_KIND
            | ASSET_CLAWBACK_TRANSACTION_KIND
            | NAV_ASSET_REGISTER_TRANSACTION_KIND
            | NAV_RESERVE_SUBMIT_TRANSACTION_KIND
            | NAV_RESERVE_CHALLENGE_TRANSACTION_KIND
            | NAV_EPOCH_FINALIZE_TRANSACTION_KIND
            | MARKET_OPS_POLICY_REGISTER_TRANSACTION_KIND
            | MARKET_OPS_FINALIZE_TRANSACTION_KIND
            | FX_FIX_REGISTER_TRANSACTION_KIND_V1
            | FX_FIX_PAUSE_TRANSACTION_KIND_V1
            | FX_FIX_RESERVATION_CREATE_TRANSACTION_KIND_V1
            | FX_FIX_RESERVATION_RELEASE_TRANSACTION_KIND_V1
            | NAV_MINT_AT_NAV_TRANSACTION_KIND
            | NAV_REDEEM_AT_NAV_TRANSACTION_KIND
            | NAV_HALT_TRANSACTION_KIND
            | NAV_PROFILE_REGISTER_TRANSACTION_KIND
            | NAV_REDEEM_SETTLE_TRANSACTION_KIND
            | NAV_RESERVE_ATTEST_TRANSACTION_KIND
            | NAV_ATTESTOR_REGISTER_TRANSACTION_KIND
            | VAULT_BRIDGE_DEPOSIT_PROPOSE_TRANSACTION_KIND
            | VAULT_BRIDGE_DEPOSIT_CHALLENGE_TRANSACTION_KIND
            | VAULT_BRIDGE_DEPOSIT_ATTEST_TRANSACTION_KIND
            | VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND
            | VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND
            | VAULT_BRIDGE_FAST_INGRESS_LIFECYCLE_TRANSACTION_KIND
            | VAULT_BRIDGE_RECEIPT_SUBMIT_TRANSACTION_KIND
            | VAULT_BRIDGE_RECEIPT_COUNT_TRANSACTION_KIND
            | VAULT_BRIDGE_MINT_FROM_RECEIPTS_TRANSACTION_KIND
            | VAULT_BRIDGE_BURN_TO_REDEEM_TRANSACTION_KIND
            | VAULT_BRIDGE_REDEEM_SETTLE_TRANSACTION_KIND
            | VAULT_BRIDGE_BUCKET_IMPAIR_TRANSACTION_KIND
            | VAULT_BRIDGE_NAV_SUBSCRIPTION_ALLOCATE_TRANSACTION_KIND
            | PFTL_UNISWAP_ROUTE_INIT_TRANSACTION_KIND
            | PFTL_UNISWAP_PRIMARY_SUBSCRIBE_TRANSACTION_KIND
            | PFTL_UNISWAP_ROUTE_INIT_V2_TRANSACTION_KIND
            | PFTL_UNISWAP_ORDER_RESERVE_TRANSACTION_KIND
            | PFTL_UNISWAP_ORDER_RELEASE_TRANSACTION_KIND
            | PFTL_UNISWAP_PRIMARY_SUBSCRIBE_V2_TRANSACTION_KIND
            | PFTL_UNISWAP_REDEMPTION_FUND_TRANSACTION_KIND
            | PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND
            | PFTL_UNISWAP_ROUTE_EPOCH_ADVANCE_TRANSACTION_KIND
            | PFTL_UNISWAP_ROUTE_PAUSE_TRANSACTION_KIND
            | PFTL_UNISWAP_EXPORT_DEBIT_TRANSACTION_KIND
            | PFTL_UNISWAP_DESTINATION_CONSUME_TRANSACTION_KIND
            | PFTL_UNISWAP_REFUND_SOURCE_TRANSACTION_KIND
            | PFTL_UNISWAP_RETURN_IMPORT_TRANSACTION_KIND
            | PFTL_UNISWAP_ROUTE_BRIDGE_POLICY_UPDATE_TRANSACTION_KIND
            | postfiat_types::EXTERNAL_LANE_RELEASE_TRANSACTION_KIND_V1
            | postfiat_types::EXTERNAL_LANE_EXPORT_TRANSACTION_KIND_V1
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
    // Both producers emit v2; v1 is still accepted from older nodes. The same
    // row checks, including the optional authority fields, apply to either.
    let schema = string_field(result, "schema")?;
    if !matches!(
        schema,
        "postfiat-pftl-uniswap-routes-status-v1" | "postfiat-pftl-uniswap-routes-status-v2"
    ) {
        return Err(invalid_result(
            "schema",
            format!(
                "expected `postfiat-pftl-uniswap-routes-status-v1` or `postfiat-pftl-uniswap-routes-status-v2`, found `{schema}`"
            ),
        ));
    }
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
    let schema = string_field(result, "schema")?;
    if schema != "postfiat-pftl-uniswap-supply-status-v1"
        && schema != "postfiat-pftl-uniswap-supply-status-v2"
    {
        return Err(invalid_result(
            "schema",
            "expected a supported PFTL-Uniswap supply-status schema",
        ));
    }
    if schema == "postfiat-pftl-uniswap-supply-status-v2" {
        if nonzero_u32_field(result, "route_schema_version")? != 2 {
            return Err(invalid_result(
                "route_schema_version",
                "v2 supply status requires route schema version 2",
            ));
        }
        nonzero_u64_field(result, "route_epoch")?;
        lower_hex_field(result, "policy_hash", 96)?;
        nonzero_u64_field(result, "policy_epoch")?;
        if nonzero_u32_field(result, "issue_multiplier_bps")? != 10_050
            || nonzero_u32_field(result, "redeem_multiplier_bps")? != 9_995
        {
            return Err(invalid_result(
                "primary_market_multiplier_bps",
                "v2 supply status has unexpected a666 issue/redemption multipliers",
            ));
        }
        let outbound = string_field(result, "outbound_verification_class")?;
        let return_class = string_field(result, "return_verification_class")?;
        if outbound != "TRUSTLESS_FINALITY" || return_class != "BFT_CHECKPOINT" {
            return Err(invalid_result(
                "directional_verification_class",
                "v2 a666 route requires TRUSTLESS_FINALITY outbound and BFT_CHECKPOINT return",
            ));
        }
        u64_field(result, "issue_capacity_remaining_atoms")?;
        u64_field(result, "redeem_capacity_remaining_atoms")?;
        nonzero_u64_field(result, "max_order_atoms")?;
        nonzero_u64_field(result, "min_order_atoms")?;
        nonzero_u64_field(result, "policy_valid_from_height")?;
        nonzero_u64_field(result, "policy_expires_at_height")?;
        nonzero_u64_field(result, "max_nav_age_blocks")?;
        nonzero_u64_field(result, "pricing_nav_epoch")?;
        lower_hex_field(result, "pricing_reserve_packet_hash", 96)?;
        u64_field(result, "non_nav_spread_atoms")?;
        u64_field(result, "active_reservation_count")?;
        u64_field(result, "active_reservation_atoms")?;
        u64_field(result, "export_entitlement_count")?;
        u64_field(result, "export_entitlement_atoms")?;
        u64_field(result, "wrapped_exposure_atoms")?;
        u64_field(result, "committed_wrapped_exposure_atoms")?;
        u64_field(result, "available_export_capacity_atoms")?;
        u64_field(result, "available_issue_atoms")?;
        u64_field(result, "available_redeem_atoms")?;
        u64_field(result, "reserve_backed_redeem_atoms")?;
    }
    clean_string_field(result, "route_id")?;
    lower_hex_field(result, "route_config_digest", 96)?;
    clean_string_field(result, "route_trust_class")?;
    lower_hex_field(result, "native_nav_asset_id", 96)?;
    lower_hex_field(result, "settlement_asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    validate_evm_address_field(result, "handoff_controller")?;
    validate_evm_address_field(result, "wrapped_navcoin_token")?;
    nonzero_u64_field(result, "ethereum_chain_id")?;
    let live_value_enabled = bool_field(result, "live_value_enabled")?;
    let paused = bool_field(result, "paused")?;
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
    u64_field(result, "source_debited_export_packet_count")?;
    let source_debited_atoms = u64_field(result, "source_debited_export_packet_atoms")?;
    if source_debited_atoms != outstanding {
        return Err(invalid_result(
            "source_debited_export_packet_atoms",
            "expected source-debited packet atoms to equal outstanding bridge claims",
        ));
    }
    let available_return_import_atoms = u64_field(result, "available_return_import_atoms")?;
    let expected_return_import_atoms = if paused || !live_value_enabled {
        0
    } else {
        ethereum
    };
    if available_return_import_atoms != expected_return_import_atoms {
        return Err(invalid_result(
            "available_return_import_atoms",
            "expected return-import capacity to equal active Ethereum spendable supply",
        ));
    }
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
    let trust_class = string_field(route, "route_trust_class")?;
    expect_string_eq(route, "movement_model", "burn_mint")?;
    clean_string_field(route, "refund_model")?;
    let live_value_enabled = bool_field(route, "live_value_enabled")?;
    let route_live = bool_field(route, "route_live")?;
    let paused = bool_field(route, "paused")?;
    if trust_class == "CONTROLLED" && live_value_enabled {
        return Err(invalid_result(
            "live_value_enabled",
            "CONTROLLED routes cannot enable public live-value routing",
        ));
    }
    if route_live != (live_value_enabled && !paused && trust_class != "DISABLED") {
        return Err(invalid_result(
            "route_live",
            "route_live does not match live-value, pause, and trust-class gates",
        ));
    }
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
    validate_navcoin_bridge_route_authority_fields(route)
}

/// Route authority fields are optional because older nodes omit them.
fn validate_navcoin_bridge_route_authority_fields(
    route: &Value,
) -> Result<(), RpcResponseValidationError> {
    let present = |name: &str| route.get(name).is_some();
    if !present("authority_epoch")
        && !present("authority_committee_current")
        && !present("authority_committee_unknown_reason")
    {
        return Ok(());
    }
    let authority_epoch = optional_u64_field_value(route, "authority_epoch")?;
    if authority_epoch == Some(0) {
        return Err(invalid_result(
            "authority_epoch",
            "expected null or a nonzero authority epoch",
        ));
    }
    optional_bool_result_field(route, "authority_committee_current")?;
    let current = field(route, "authority_committee_current")?.as_bool();
    let reason = optional_reason_field(route, "authority_committee_unknown_reason")?;
    match (current, reason) {
        (Some(_), None) if authority_epoch.is_some() => Ok(()),
        (None, Some(_)) => Ok(()),
        _ => Err(invalid_result(
            "authority_committee_current",
            "expected a boolean with an authority epoch and no reason, or null with a reason",
        )),
    }
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
        "CONTROLLED" | "OPTIMISTIC" | "TRUSTLESS_FINALITY" | "BFT_CHECKPOINT" | "DISABLED"
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

/// Reads a field the node declares as `u128`.
///
/// The node builds every RPC response through `serde_json::to_value`, which
/// (without the `arbitrary_precision` feature, not enabled in this
/// workspace) only accepts a `u128` that fits in a `u64`; a larger value
/// makes the node's own serialization fail instead of reaching the wire.
/// On the SDK side the same serde_json parses an integer literal above
/// `u64::MAX` into a lossy `f64`, so an exact comparison is impossible.
/// This helper is therefore exact for every value the node can emit and
/// fails closed, with a distinct message, for an integer literal beyond
/// `u64::MAX` rather than accepting a rounded value.
fn u128_field(value: &Value, path: &str) -> Result<u128, RpcResponseValidationError> {
    let number = field(value, path)?;
    if let Some(exact) = number.as_u64() {
        return Ok(u128::from(exact));
    }
    if number
        .as_f64()
        .is_some_and(|wide| wide.is_finite() && wide >= 0.0 && wide.fract() == 0.0)
    {
        return Err(invalid_result(
            path,
            "integer exceeds u64::MAX; the node cannot serialize such a value and the SDK cannot represent it exactly",
        ));
    }
    Err(invalid_result(path, "expected unsigned integer value"))
}

fn nonzero_u128_field(value: &Value, path: &str) -> Result<u128, RpcResponseValidationError> {
    let parsed = u128_field(value, path)?;
    if parsed == 0 {
        return Err(invalid_result(path, "expected nonzero unsigned integer value"));
    }
    Ok(parsed)
}

const MARKET_OPS_STATUS_RESULT_SCHEMA: &str = "postfiat-market-ops-public-status-v1";
const MARKET_OPS_STATUS_RESULT_DISCLOSURE: &str = "The protocol proves NAV and may execute bounded market operations under public caps. Holders do not have a standing right to redeem at NAV, and market operations can pause.";
const MARKET_OPS_STATUS_ACTIVE_VALUE: &str = "active";
const MARKET_OPS_STATUS_VALUES: [&str; 6] = [
    MARKET_OPS_STATUS_ACTIVE_VALUE,
    "expired",
    "missing_source_packet",
    "paused",
    "stale",
    "underfunded",
];

/// Validates a `market_ops_status` report (`MarketOpsPublicStatus` in the
/// node), mirroring `MarketOpsPublicStatus::validate` and the cap rule in
/// `build_market_ops_public_status`: schema and disclosure pinned, hex
/// identifiers sized, the NAV floor / verified net assets / valid supply and
/// envelope epoch nonzero, the status drawn from the six published values,
/// and both current caps zero unless the status is `active`.
///
/// The status must also be one the builder could reach from the report's own
/// freshness flags and alignment-reserve figures (the builder decides in the
/// order paused, missing_source_packet, stale, expired, underfunded, active):
/// `active` requires both packets fresh and funded >= required;
/// `underfunded` requires both fresh and funded < required; `expired`
/// requires both fresh; `stale` requires at least one stale packet; and
/// `missing_source_packet` requires a stale reserve packet. `paused` takes
/// precedence over every input. None of this needs the current time or
/// block height.
///
/// The node declares the USD-e8 and atoms figures as `u128`; see
/// `u128_field` for how values beyond `u64::MAX` are handled.
///
/// This checks the report in isolation and is private. It is reachable only
/// through `validate_response_kind(.., RpcResponseKind::MarketOpsStatus)`,
/// which cannot tell a valid report for another asset or epoch from the one
/// asked for. Callers must use `validate_market_ops_status_response_for_request`,
/// the primary API, which binds the report to its request (the CLI requires
/// `--request-file` for this kind).
fn validate_market_ops_status_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", MARKET_OPS_STATUS_RESULT_SCHEMA)?;
    lower_hex_field(result, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    nonzero_u128_field(result, "nav_floor_usd_e8")?;
    nonzero_u128_field(result, "verified_net_assets_usd_e8")?;
    nonzero_u128_field(result, "valid_global_supply_atoms")?;
    let reserve_packet_fresh = bool_field(result, "reserve_packet_fresh")?;
    let supply_packet_fresh = bool_field(result, "supply_packet_fresh")?;
    u64_field(result, "reserve_packet_age_blocks")?;
    u64_field(result, "supply_packet_age_blocks")?;
    let funded_alignment_reserve_usd_e8 = u128_field(result, "funded_alignment_reserve_usd_e8")?;
    let required_alignment_reserve_usd_e8 =
        u128_field(result, "required_alignment_reserve_usd_e8")?;
    let current_reserve_deploy_cap_usd_e8 = u128_field(result, "current_reserve_deploy_cap_usd_e8")?;
    let current_mint_cap_atoms = u128_field(result, "current_mint_cap_atoms")?;
    let market_operations_status = clean_string_field(result, "market_operations_status")?;
    if !MARKET_OPS_STATUS_VALUES.contains(&market_operations_status) {
        return Err(invalid_result(
            "market_operations_status",
            "expected one of active, expired, missing_source_packet, paused, stale, underfunded",
        ));
    }
    if market_operations_status != MARKET_OPS_STATUS_ACTIVE_VALUE {
        if current_reserve_deploy_cap_usd_e8 != 0 {
            return Err(invalid_result(
                "current_reserve_deploy_cap_usd_e8",
                "expected a zero reserve deploy cap unless market operations are active",
            ));
        }
        if current_mint_cap_atoms != 0 {
            return Err(invalid_result(
                "current_mint_cap_atoms",
                "expected a zero mint cap unless market operations are active",
            ));
        }
    }
    validate_market_ops_status_consistency(
        market_operations_status,
        reserve_packet_fresh,
        supply_packet_fresh,
        funded_alignment_reserve_usd_e8 >= required_alignment_reserve_usd_e8,
    )?;
    lower_hex_field(result, "accepted_policy_hash", 64)?;
    lower_hex_field(result, "envelope_hash", 96)?;
    nonzero_u64_field(result, "envelope_epoch")?;
    u64_field(result, "packet_expires_at")?;
    expect_string_eq(result, "disclosure", MARKET_OPS_STATUS_RESULT_DISCLOSURE)?;
    Ok(())
}

/// Rejects a status the node's `build_market_ops_public_status` cannot emit
/// for the freshness flags and funding the report itself carries.
fn validate_market_ops_status_consistency(
    status: &str,
    reserve_packet_fresh: bool,
    supply_packet_fresh: bool,
    funded: bool,
) -> Result<(), RpcResponseValidationError> {
    let both_fresh = reserve_packet_fresh && supply_packet_fresh;
    match status {
        MARKET_OPS_STATUS_ACTIVE_VALUE => {
            if !reserve_packet_fresh {
                return Err(invalid_result(
                    "reserve_packet_fresh",
                    "active market operations require a fresh reserve packet",
                ));
            }
            if !supply_packet_fresh {
                return Err(invalid_result(
                    "supply_packet_fresh",
                    "active market operations require a fresh supply packet",
                ));
            }
            if !funded {
                return Err(invalid_result(
                    "funded_alignment_reserve_usd_e8",
                    "active market operations require funded >= required alignment reserve",
                ));
            }
        }
        "underfunded" if !both_fresh || funded => {
            return Err(invalid_result(
                "market_operations_status",
                "underfunded requires both packets fresh and funded < required alignment reserve",
            ));
        }
        "expired" if !both_fresh => {
            return Err(invalid_result(
                "market_operations_status",
                "expired requires both packets fresh; a stale packet reports stale",
            ));
        }
        "stale" if both_fresh => {
            return Err(invalid_result(
                "market_operations_status",
                "stale requires a stale reserve or supply packet",
            ));
        }
        "missing_source_packet" if reserve_packet_fresh => {
            return Err(invalid_result(
                "market_operations_status",
                "missing_source_packet cannot carry a fresh reserve packet",
            ));
        }
        _ => {}
    }
    Ok(())
}

/// Validates a `market_ops_status` response against the request that
/// produced it. This is the API to use for this kind: the request must itself be a valid `market_ops_status`
/// request, the response id must equal the request id, the report must pass
/// `validate_market_ops_status_result`, its `asset_id` must equal the
/// requested asset, and, when the request names an `epoch`, its
/// `envelope_epoch` must equal it. With no requested epoch the node serves
/// the latest finalized envelope, so any envelope epoch binds.
pub fn validate_market_ops_status_response_for_request(
    response: &RpcResponse,
    request: &RpcRequest,
) -> Result<(), RpcResponseValidationError> {
    validate_request(request, None, Some(RpcRequestKind::MarketOpsStatus))
        .map_err(|error| invalid_result("request", error.to_string()))?;
    validate_response(response, Some(&request.id), true)?;
    validate_response_kind(response, RpcResponseKind::MarketOpsStatus)?;
    let result = response
        .result
        .as_ref()
        .ok_or_else(|| invalid_result("result", "missing successful response result"))?;
    let requested_asset_id = request
        .params
        .get("asset_id")
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_result("request.params.asset_id", "expected string"))?;
    if string_field(result, "asset_id")? != requested_asset_id {
        return Err(invalid_result(
            "asset_id",
            format!("expected the requested asset `{requested_asset_id}`"),
        ));
    }
    if let Some(epoch) = request.params.get("epoch") {
        let requested_epoch = epoch.as_u64().ok_or_else(|| {
            invalid_result("request.params.epoch", "expected unsigned integer")
        })?;
        if u64_field(result, "envelope_epoch")? != requested_epoch {
            return Err(invalid_result(
                "envelope_epoch",
                format!("expected the requested epoch {requested_epoch}"),
            ));
        }
    }
    Ok(())
}


const OWNED_OBJECTS_RESULT_SCHEMA: &str = "postfiat-owned-objects-v1";
/// Hex length of the 32-byte owned object id (`OwnedObject::id`).
const OWNED_OBJECT_ID_HEX_LEN: usize = 64;

/// Reads a hex string of a fixed length, accepting either case. The node
/// decodes these identifiers with a case-insensitive hex decoder and echoes
/// them as stored, so a lowercase-only rule would reject valid reports.
fn hex_field_any_case<'a>(
    value: &'a Value,
    path: &str,
    expected_len: usize,
) -> Result<&'a str, RpcResponseValidationError> {
    let text = string_field(value, path)?;
    if text.len() != expected_len || !text.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(invalid_result(
            path,
            format!("expected {expected_len} hex characters"),
        ));
    }
    Ok(text)
}

/// Validates an `owned_objects` report (`OwnedObjectsReport` in the node),
/// mirroring the node builder in crates/node lifecycle_queries.rs: schema
/// pinned, common chain fields, the owner key echoed, the limit bounded by
/// `OWNED_OBJECTS_MAX_LIMIT`, `object_count` equal to the row count and at
/// most the limit, `truncated` only when the page is full, every row owned
/// by the requested key and (when an asset filter was given) carrying that
/// asset, rows sorted by (asset, id, version), and `total_value` equal to
/// the sum of the returned values unless the page was truncated, in which
/// case the node summed before truncating so it must be at least that sum.
fn validate_owned_objects_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", OWNED_OBJECTS_RESULT_SCHEMA)?;
    validate_asset_read_common_fields(result)?;
    let owner_public_key_hex = hex_field_any_case(
        result,
        "owner_public_key_hex",
        OWNED_OWNER_PUBLIC_KEY_HEX_LEN,
    )?;
    let requested_asset = match result.get("asset") {
        None | Some(Value::Null) => None,
        Some(_) => Some(clean_string_field(result, "asset")?),
    };
    let limit = nonzero_u64_field(result, "limit")?;
    if limit > OWNED_OBJECTS_MAX_LIMIT as u64 {
        return Err(invalid_result(
            "limit",
            format!("expected limit <= {OWNED_OBJECTS_MAX_LIMIT}"),
        ));
    }
    let truncated = bool_field(result, "truncated")?;
    let object_count = u64_field(result, "object_count")?;
    let total_value = u64_field(result, "total_value")?;
    let objects = array_field(result, "objects")?;
    if object_count != u64::try_from(objects.len()).unwrap_or(u64::MAX) {
        return Err(invalid_result(
            "object_count",
            "expected object_count to match objects length",
        ));
    }
    if object_count > limit {
        return Err(invalid_result(
            "object_count",
            "expected at most limit objects",
        ));
    }
    // Cursor-era nodes add `cursor`, `next_cursor`, `height` and
    // `state_root`; a page may then end early at the node's byte bound, and
    // a page after the first holds only part of the summed objects.
    let paged = result.get("next_cursor").is_some();
    let next_cursor = match result.get("next_cursor") {
        None | Some(Value::Null) => None,
        Some(_) => Some(clean_string_field(result, "next_cursor")?),
    };
    let continued = matches!(result.get("cursor"), Some(value) if !value.is_null());
    if paged {
        u64_field(result, "height")?;
        clean_string_field(result, "state_root")?;
        // The object view the page was read from (height/root do not
        // identify it); a cursor-era page always carries it.
        lower_hex_field(result, "view_digest", 64)?;
        if truncated != next_cursor.is_some() {
            return Err(invalid_result(
                "next_cursor",
                "expected next_cursor exactly when the page is truncated",
            ));
        }
    }
    // Revision-era nodes add `inventory_revision` (read with the page, at
    // the page's finalized `height`, with the data directory's
    // `incarnation`) or the reason it is unavailable, and the answering
    // `node_id`.
    let revision_present = !matches!(
        result.get("inventory_revision"),
        None | Some(Value::Null)
    );
    if !matches!(
        result.get("inventory_revision_unavailable"),
        None | Some(Value::Null)
    ) {
        clean_string_field(result, "inventory_revision_unavailable")?;
        if revision_present {
            return Err(invalid_result(
                "inventory_revision_unavailable",
                "expected no reason beside an inventory revision",
            ));
        }
    }
    match result.get("inventory_revision") {
        None | Some(Value::Null) => {}
        Some(Value::Object(revision)) => {
            let finalized_height = u64_field(result, "inventory_revision.finalized_height")?;
            u64_field(result, "inventory_revision.fastpay_sequence")?;
            if result.get("height").and_then(Value::as_u64) != Some(finalized_height) {
                return Err(invalid_result(
                    "inventory_revision.finalized_height",
                    "expected the revision height to equal the page height",
                ));
            }
            if revision.contains_key("incarnation") {
                lower_hex_field(result, "inventory_revision.incarnation", 32)?;
            }
        }
        Some(_) => {
            return Err(invalid_result(
                "inventory_revision",
                "expected an object with finalized_height and fastpay_sequence",
            ));
        }
    }
    if result.get("node_id").is_some() {
        optional_clean_string_result_field(result, "node_id")?;
    }
    if truncated && object_count != limit && !paged {
        return Err(invalid_result(
            "truncated",
            "expected a truncated report to hold exactly limit objects",
        ));
    }
    let keys_omitted = matches!(result.get("owner_keys_omitted"), Some(Value::Bool(true)));

    let mut returned_value_sum = 0_u64;
    let mut previous: Option<(&str, &str, u64)> = None;
    for (index, object) in objects.iter().enumerate() {
        let id = hex_field_any_case(object, "id", OWNED_OBJECT_ID_HEX_LEN)?;
        let version = u64_field(object, "version")?;
        let object_owner = if keys_omitted {
            if object.get("owner_pubkey_hex").is_some() {
                return Err(invalid_result(
                    format!("objects[{index}].owner_pubkey_hex"),
                    "expected no per-object owner key when owner_keys_omitted",
                ));
            }
            owner_public_key_hex
        } else {
            string_field(object, "owner_pubkey_hex")?
        };
        let value = u64_field(object, "value")?;
        let asset = clean_string_field(object, "asset")?;
        if object_owner != owner_public_key_hex {
            return Err(invalid_result(
                format!("objects[{index}].owner_pubkey_hex"),
                "expected every object to belong to the requested owner",
            ));
        }
        if let Some(requested_asset) = requested_asset {
            if asset != requested_asset {
                return Err(invalid_result(
                    format!("objects[{index}].asset"),
                    "expected every object to match the requested asset",
                ));
            }
        }
        let key = (asset, id, version);
        if previous.is_some_and(|previous| previous > key) {
            return Err(invalid_result(
                format!("objects[{index}]"),
                "expected objects sorted by (asset, id, version)",
            ));
        }
        previous = Some(key);
        returned_value_sum = returned_value_sum.checked_add(value).ok_or_else(|| {
            invalid_result("total_value", "returned object values overflowed")
        })?;
    }
    if truncated || continued {
        if total_value < returned_value_sum {
            return Err(invalid_result(
                "total_value",
                "expected total_value to be at least the sum of the returned object values when truncated",
            ));
        }
    } else if total_value != returned_value_sum {
        return Err(invalid_result(
            "total_value",
            "expected total_value to equal the sum of the returned object values",
        ));
    }
    Ok(())
}

const OWNED_RECOVERY_STATUS_RESULT_SCHEMA: &str = "postfiat-fastpay-recovery-status-v1";
const OWNED_RECOVERY_STATUS_VALUES: [&str; 4] = [
    "confirmed",
    "cancelled",
    "certificate_revealed",
    "open_or_unknown",
];

/// Validates an `owned_recovery_status` report, mirroring the node's
/// `owned_recovery_status_v3` (crates/node fastpay_recovery_node.rs): the
/// schema is pinned, `lock_id` is echoed as 96 lowercase hex, `fence` is
/// either null or a `FastPayVersionFenceV1` record for that lock that
/// passes the type's own `validate_shape`, and `status` must be exactly what
/// the node derives: `confirmed` / `cancelled` from the fence decision when
/// a fence exists, otherwise `certificate_revealed` when `reveal_count > 0`
/// and `open_or_unknown` when it is zero.
fn validate_owned_recovery_status_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", OWNED_RECOVERY_STATUS_RESULT_SCHEMA)?;
    lower_hex_field(result, "lock_id", OWNED_RECOVERY_LOCK_ID_HEX_LEN)?;
    let lock_id = string_field(result, "lock_id")?;
    let status = clean_string_field(result, "status")?;
    if !OWNED_RECOVERY_STATUS_VALUES.contains(&status) {
        return Err(invalid_result(
            "status",
            "expected one of confirmed, cancelled, certificate_revealed, open_or_unknown",
        ));
    }
    let reveal_count = u64_field(result, "reveal_count")?;
    let fence_value = field(result, "fence")?;
    let fence = if fence_value.is_null() {
        None
    } else {
        let fence: postfiat_types::FastPayVersionFenceV1 =
            serde_json::from_value(fence_value.clone()).map_err(|error| {
                invalid_result(
                    "fence",
                    format!("expected a FastPay version fence record: {error}"),
                )
            })?;
        fence
            .validate_shape()
            .map_err(|error| invalid_result("fence", error))?;
        if fence.lock_id != lock_id {
            return Err(invalid_result(
                "fence.lock_id",
                "expected the fence to belong to the requested lock",
            ));
        }
        Some(fence)
    };
    let expected_status = match fence.as_ref().map(|fence| &fence.decision) {
        Some(postfiat_types::FastPayRecoveryDecisionV1::Confirmed { .. }) => "confirmed",
        Some(postfiat_types::FastPayRecoveryDecisionV1::Cancelled) => "cancelled",
        None if reveal_count > 0 => "certificate_revealed",
        None => "open_or_unknown",
    };
    if status != expected_status {
        return Err(invalid_result(
            "status",
            format!("expected `{expected_status}` from the fence decision and reveal count"),
        ));
    }
    Ok(())
}

fn nested_u64_field(
    value: &Value,
    prefix: &str,
    name: &str,
) -> Result<u64, RpcResponseValidationError> {
    field(value, name)?
        .as_u64()
        .ok_or_else(|| invalid_result(format!("{prefix}.{name}"), "expected unsigned integer value"))
}

fn nested_string_field<'a>(
    value: &'a Value,
    prefix: &str,
    name: &str,
) -> Result<&'a str, RpcResponseValidationError> {
    field(value, name)?
        .as_str()
        .ok_or_else(|| invalid_result(format!("{prefix}.{name}"), "expected string value"))
}

/// Validates an `owned_recovery_capabilities` report, mirroring the node's
/// `owned_recovery_capabilities_v3` (crates/node fastpay_recovery_node.rs),
/// which builds a `postfiat_types::FastPayRecoveryCapabilitiesV1` and calls
/// its `validate()` before serializing. Field-level checks run first so a
/// rejection names the exact field (`domain.genesis_hash`,
/// `policy.max_validity_blocks`, ...); the result is then deserialized into
/// the real type (which is `deny_unknown_fields`) and the type's own
/// `validate()` is applied, so the SDK enforces exactly what the node
/// enforces and nothing weaker.
fn validate_owned_recovery_capabilities_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(
        result,
        "schema",
        postfiat_types::FASTPAY_RECOVERY_CAPABILITIES_SCHEMA_V1,
    )?;

    let domain = field(result, "domain")?;
    if nested_string_field(domain, "domain", "schema")?
        != postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3
    {
        return Err(invalid_result(
            "domain.schema",
            format!(
                "expected {}",
                postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3
            ),
        ));
    }
    if nested_string_field(domain, "domain", "chain_id")?
        .trim()
        .is_empty()
    {
        return Err(invalid_result("domain.chain_id", "expected nonempty string value"));
    }
    if !is_lower_hex_len(nested_string_field(domain, "domain", "genesis_hash")?, 96) {
        return Err(invalid_result(
            "domain.genesis_hash",
            "expected 96 lowercase hex characters",
        ));
    }
    let protocol_version = nested_u64_field(domain, "domain", "protocol_version")?;
    if protocol_version == 0 || protocol_version > u64::from(u32::MAX) {
        return Err(invalid_result(
            "domain.protocol_version",
            "expected nonzero u32 value",
        ));
    }
    if nested_string_field(domain, "domain", "registry_id")?
        .trim()
        .is_empty()
    {
        return Err(invalid_result("domain.registry_id", "expected nonempty string value"));
    }

    nonzero_u64_field(result, "committee_epoch")?;
    u64_field(result, "current_height")?;
    let validator_count = nonzero_u64_field(result, "validator_count")?;
    let quorum = nonzero_u64_field(result, "quorum")?;
    // The node reports its active committee, and committee validation pins
    // quorum to expected_quorum(validator count) = n - (n - 1) / 3.
    let validator_count_usize = usize::try_from(validator_count).map_err(|_| {
        invalid_result("validator_count", "validator_count does not fit in usize")
    })?;
    let expected_quorum =
        postfiat_types::FastPayRecoveryCommitteeV1::expected_quorum(validator_count_usize)
            .ok_or_else(|| invalid_result("validator_count", "expected nonzero validator_count"))?;
    if usize::try_from(quorum).ok() != Some(expected_quorum) {
        return Err(invalid_result(
            "quorum",
            format!(
                "expected the committee quorum {expected_quorum} for {validator_count} validators"
            ),
        ));
    }

    let policy = field(result, "policy")?;
    if nested_string_field(policy, "policy", "schema")?
        != postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1
    {
        return Err(invalid_result(
            "policy.schema",
            format!("expected {}", postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1),
        ));
    }
    if nested_u64_field(policy, "policy", "activation_height")? == 0 {
        return Err(invalid_result(
            "policy.activation_height",
            "expected nonzero unsigned integer value",
        ));
    }
    let max_validity_blocks = nested_u64_field(policy, "policy", "max_validity_blocks")?;
    if max_validity_blocks == 0
        || max_validity_blocks > postfiat_types::MAX_FASTPAY_VALIDITY_BLOCKS
    {
        return Err(invalid_result(
            "policy.max_validity_blocks",
            format!(
                "expected 1..={}",
                postfiat_types::MAX_FASTPAY_VALIDITY_BLOCKS
            ),
        ));
    }
    let max_recovery_blocks = nested_u64_field(policy, "policy", "max_recovery_blocks")?;
    if max_recovery_blocks == 0
        || max_recovery_blocks > postfiat_types::MAX_FASTPAY_RECOVERY_BLOCKS
    {
        return Err(invalid_result(
            "policy.max_recovery_blocks",
            format!(
                "expected 1..={}",
                postfiat_types::MAX_FASTPAY_RECOVERY_BLOCKS
            ),
        ));
    }

    let capabilities: postfiat_types::FastPayRecoveryCapabilitiesV1 =
        serde_json::from_value(result.clone()).map_err(|error| {
            invalid_result(
                "result",
                format!("expected a FastPay recovery capabilities record: {error}"),
            )
        })?;
    capabilities
        .validate()
        .map_err(|error| invalid_result("result", error))?;
    Ok(())
}

const OWNED_CERTIFICATE_OPERATIONS: [&str; 2] = ["transfer", "unwrap"];

/// Validates the shape of an `owned_certificate` result (the node returns a
/// `postfiat_types::FastPayCertificateV1`, externally tagged as
/// `{"operation": "transfer" | "unwrap", "certificate": {..}}`). Field-level
/// checks run first so a rejection names the exact field; the result is then
/// deserialized into the real type (both certificate structs are
/// `deny_unknown_fields`) and the type's own `validate_commitment_shape` is
/// applied. Binding the certificate to the request's selector needs the
/// request and lives in `validate_owned_certificate_response`.
fn validate_owned_certificate_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    let operation = clean_string_field(result, "operation")?;
    if !OWNED_CERTIFICATE_OPERATIONS.contains(&operation) {
        return Err(invalid_result("operation", "expected transfer or unwrap"));
    }
    let certificate_value = field(result, "certificate")?;
    let order = field(certificate_value, "order")
        .map_err(|_| invalid_result("certificate.order", "missing certificate order"))?;
    let domain = field(order, "domain")
        .map_err(|_| invalid_result("certificate.order.domain", "missing order domain"))?;
    if nested_string_field(domain, "certificate.order.domain", "schema")?
        != postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3
    {
        return Err(invalid_result(
            "certificate.order.domain.schema",
            format!("expected {}", postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3),
        ));
    }
    if nested_string_field(domain, "certificate.order.domain", "chain_id")?
        .trim()
        .is_empty()
    {
        return Err(invalid_result(
            "certificate.order.domain.chain_id",
            "expected nonempty string value",
        ));
    }
    if !is_lower_hex_len(
        nested_string_field(domain, "certificate.order.domain", "genesis_hash")?,
        96,
    ) {
        return Err(invalid_result(
            "certificate.order.domain.genesis_hash",
            "expected 96 lowercase hex characters",
        ));
    }
    let protocol_version = nested_u64_field(domain, "certificate.order.domain", "protocol_version")?;
    if protocol_version == 0 || protocol_version > u64::from(u32::MAX) {
        return Err(invalid_result(
            "certificate.order.domain.protocol_version",
            "expected nonzero u32 value",
        ));
    }
    if nested_string_field(domain, "certificate.order.domain", "registry_id")?
        .trim()
        .is_empty()
    {
        return Err(invalid_result(
            "certificate.order.domain.registry_id",
            "expected nonempty string value",
        ));
    }

    let recovery = field(order, "recovery")
        .map_err(|_| invalid_result("certificate.order.recovery", "missing order recovery"))?;
    if nested_string_field(recovery, "certificate.order.recovery", "schema")?
        != postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1
    {
        return Err(invalid_result(
            "certificate.order.recovery.schema",
            format!("expected {}", postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1),
        ));
    }
    if nested_u64_field(recovery, "certificate.order.recovery", "committee_epoch")? == 0 {
        return Err(invalid_result(
            "certificate.order.recovery.committee_epoch",
            "expected nonzero unsigned integer value",
        ));
    }
    if !is_lower_hex_len(
        nested_string_field(recovery, "certificate.order.recovery", "lock_id")?,
        OWNED_CERTIFICATE_SELECTOR_HEX_LEN,
    ) {
        return Err(invalid_result(
            "certificate.order.recovery.lock_id",
            format!("expected {OWNED_CERTIFICATE_SELECTOR_HEX_LEN} lowercase hex characters"),
        ));
    }
    let valid_from = nested_u64_field(recovery, "certificate.order.recovery", "valid_from_height")?;
    let expires_at = nested_u64_field(recovery, "certificate.order.recovery", "expires_at_height")?;
    let closes_at = nested_u64_field(
        recovery,
        "certificate.order.recovery",
        "recovery_closes_at_height",
    )?;
    if valid_from == 0 || expires_at < valid_from || closes_at <= expires_at {
        return Err(invalid_result(
            "certificate.order.recovery.recovery_closes_at_height",
            "expected 0 < valid_from_height <= expires_at_height < recovery_closes_at_height",
        ));
    }
    let inputs = field(order, "inputs")
        .map_err(|_| invalid_result("certificate.order.inputs", "missing order inputs"))?;
    if inputs.as_array().is_none_or(Vec::is_empty) {
        return Err(invalid_result(
            "certificate.order.inputs",
            "expected at least one input object",
        ));
    }

    let certificate: postfiat_types::FastPayCertificateV1 = serde_json::from_value(result.clone())
        .map_err(|error| {
            invalid_result("result", format!("expected a FastPay certificate: {error}"))
        })?;
    certificate
        .validate_commitment_shape()
        .map_err(|error| invalid_result("certificate.votes", error))?;
    // The embedded lock id is a commitment to the order: recompute the
    // operation-specific lock id from the committed order fields (domain,
    // recovery window, inputs, outputs or unwrap target, fee, nonce, memos)
    // and require it to equal `certificate.order.recovery.lock_id`, as the
    // execution layer does before accepting a certificate
    // (owned_transfer_recovery.rs). Without this a certificate whose order was
    // changed after the lock was derived would still claim the original lock.
    let (embedded_lock_id, expected_lock_id) = match &certificate {
        postfiat_types::FastPayCertificateV1::Transfer(certificate) => (
            &certificate.order.recovery.lock_id,
            postfiat_types::fastpay_transfer_lock_id_v1(&certificate.order),
        ),
        postfiat_types::FastPayCertificateV1::Unwrap(certificate) => (
            &certificate.order.recovery.lock_id,
            postfiat_types::fastpay_unwrap_lock_id_v1(&certificate.order),
        ),
    };
    if *embedded_lock_id != expected_lock_id {
        return Err(invalid_result(
            "certificate.order.recovery.lock_id",
            "expected the lock id recomputed from the committed order",
        ));
    }
    Ok(())
}

const VAULT_BRIDGE_STATUS_RESULT_SCHEMA: &str = "postfiat-vault-bridge-status-v1";
const VAULT_BRIDGE_STATUS_RESULT_DISCLOSURE: &str = "vault bridge asset is source-bound and bridge-backed on PFTL. Counted vault bridge asset receipts must reference a ERC20BridgeVault deposit event evidence root; withdrawals are PFTL burn packets claimed from the source vault after challenge/finality. No pooled or automatic par redemption is implied.";
const VAULT_BRIDGE_STATUS_BUCKET_ACTIVE: &str = "active";
const VAULT_BRIDGE_STATUS_BUCKET_IMPAIRED: &str = "impaired";
const VAULT_BRIDGE_STATUS_PAR_BPS: u64 = 10_000;

struct VaultBridgeStatusBucketSummary<'a> {
    bucket_id: &'a str,
    active: bool,
    counts_toward_counted_value: bool,
    counted_value_atoms: u64,
    redeemable_at_par: bool,
    allocated_atoms: u64,
}

/// Validates a `vault_bridge_status` report (`VaultBridgeStatusReport` in the
/// node) and the invariants the node computes when assembling it: issued
/// supply = transparent + Orchard, healthy/impaired allocation sums over the
/// bucket rows, per-row unallocated/redeemable/remaining arithmetic,
/// attestation tallies, `*_count` fields equal to their row counts, and the
/// top-level `counted_value_atoms` (checked sum of active/impaired bucket
/// counted values) and `unallocated_counted_capacity_atoms` (checked sum of
/// the receipt rows' unallocated values) totals.
fn validate_vault_bridge_status_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", VAULT_BRIDGE_STATUS_RESULT_SCHEMA)?;
    lower_hex_field(result, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    clean_string_field(result, "issuer")?;
    clean_string_field(result, "proof_profile")?;
    clean_string_field(result, "valuation_unit")?;
    u64_field(result, "finalized_epoch")?;
    u64_field(result, "nav_per_unit")?;
    u64_field(result, "circulating_supply")?;
    string_field(result, "finalized_reserve_packet_hash")?;
    let issued_supply_atoms = u64_field(result, "issued_supply_atoms")?;
    let transparent_supply_atoms = u64_field(result, "transparent_supply_atoms")?;
    let orchard_supply_atoms = u64_field(result, "orchard_supply_atoms")?;
    if transparent_supply_atoms.checked_add(orchard_supply_atoms) != Some(issued_supply_atoms) {
        return Err(invalid_result(
            "issued_supply_atoms",
            "expected issued supply to equal transparent plus Orchard supply",
        ));
    }
    let counted_value_atoms = u64_field(result, "counted_value_atoms")?;
    let healthy_allocated_atoms = u64_field(result, "healthy_allocated_atoms")?;
    let impaired_allocated_atoms = u64_field(result, "impaired_allocated_atoms")?;
    let source_series_enforced = bool_field(result, "source_series_enforced")?;
    let expected_classification = if source_series_enforced {
        "mixed_legacy_pooled_and_source_series"
    } else {
        "legacy_pooled"
    };
    expect_string_eq(
        result,
        "display_family_classification",
        expected_classification,
    )?;
    let unallocated_counted_capacity_atoms =
        u64_field(result, "unallocated_counted_capacity_atoms")?;
    clean_string_field(result, "source_root")?;

    let buckets = array_field(result, "buckets")?;
    let mut active_bucket_ids: Vec<&str> = Vec::new();
    let mut expected_healthy_atoms = 0_u64;
    let mut expected_impaired_atoms = 0_u64;
    // The node's `vault_bridge_counted_value_for_asset`: a checked sum of
    // counted value over the asset's active and impaired buckets only.
    let mut expected_counted_value_atoms = Some(0_u64);
    for (index, bucket) in buckets.iter().enumerate() {
        let summary = validate_vault_bridge_status_bucket_row(bucket, index)?;
        if summary.active {
            active_bucket_ids.push(summary.bucket_id);
        }
        if summary.counts_toward_counted_value {
            expected_counted_value_atoms = expected_counted_value_atoms
                .and_then(|total| total.checked_add(summary.counted_value_atoms));
        }
        let total = if summary.redeemable_at_par {
            &mut expected_healthy_atoms
        } else {
            &mut expected_impaired_atoms
        };
        *total = total
            .checked_add(summary.allocated_atoms)
            .ok_or_else(|| {
                invalid_result(
                    format!("buckets[{index}]"),
                    "bucket allocation sum overflowed",
                )
            })?;
    }
    if healthy_allocated_atoms != expected_healthy_atoms {
        return Err(invalid_result(
            "healthy_allocated_atoms",
            "expected the allocation sum of buckets redeemable at par",
        ));
    }
    if impaired_allocated_atoms != expected_impaired_atoms {
        return Err(invalid_result(
            "impaired_allocated_atoms",
            "expected the allocation sum of buckets not redeemable at par",
        ));
    }

    match expected_counted_value_atoms {
        Some(expected) if expected == counted_value_atoms => {}
        Some(_) => {
            return Err(invalid_result(
                "counted_value_atoms",
                "expected the counted value sum of active and impaired buckets",
            ));
        }
        None => {
            return Err(invalid_result(
                "counted_value_atoms",
                "active and impaired bucket counted value sum overflowed",
            ));
        }
    }

    let receipts = array_field(result, "receipts")?;
    // The node sums each active-bucket receipt's available counted value with
    // the same predicate it uses for the per-row `unallocated_value_atoms`, so
    // the total is the checked sum of the validated row values.
    let mut expected_unallocated_capacity_atoms = Some(0_u64);
    for (index, receipt) in receipts.iter().enumerate() {
        let unallocated =
            validate_vault_bridge_status_receipt_row(receipt, index, &active_bucket_ids)?;
        expected_unallocated_capacity_atoms =
            expected_unallocated_capacity_atoms.and_then(|total| total.checked_add(unallocated));
    }
    match expected_unallocated_capacity_atoms {
        Some(expected) if expected == unallocated_counted_capacity_atoms => {}
        Some(_) => {
            return Err(invalid_result(
                "unallocated_counted_capacity_atoms",
                "expected the sum of receipt unallocated values in active buckets",
            ));
        }
        None => {
            return Err(invalid_result(
                "unallocated_counted_capacity_atoms",
                "receipt unallocated value sum overflowed",
            ));
        }
    }
    let bridge_deposits = array_field(result, "bridge_deposits")?;
    for (index, deposit) in bridge_deposits.iter().enumerate() {
        validate_vault_bridge_status_deposit_row(deposit, index)?;
    }
    let allocations = array_field(result, "allocations")?;
    for (index, allocation) in allocations.iter().enumerate() {
        validate_vault_bridge_status_allocation_row(allocation, index)?;
    }
    let redemptions = array_field(result, "redemptions")?;
    for redemption in redemptions {
        validate_vault_bridge_status_redemption_row(redemption)?;
    }

    vault_bridge_status_count_matches(result, "bucket_count", buckets.len())?;
    vault_bridge_status_count_matches(result, "receipt_count", receipts.len())?;
    vault_bridge_status_count_matches(result, "bridge_deposit_count", bridge_deposits.len())?;
    vault_bridge_status_count_matches(result, "allocation_count", allocations.len())?;
    vault_bridge_status_count_matches(result, "redemption_count", redemptions.len())?;
    expect_string_eq(result, "disclosure", VAULT_BRIDGE_STATUS_RESULT_DISCLOSURE)?;
    Ok(())
}

fn vault_bridge_status_count_matches(
    result: &Value,
    path: &str,
    rows: usize,
) -> Result<(), RpcResponseValidationError> {
    let count = u64_field(result, path)?;
    if count != u64::try_from(rows).unwrap_or(u64::MAX) {
        return Err(invalid_result(
            path,
            format!("expected {path} to equal the {rows} rows returned"),
        ));
    }
    Ok(())
}

fn validate_vault_bridge_status_bucket_row<'a>(
    row: &'a Value,
    index: usize,
) -> Result<VaultBridgeStatusBucketSummary<'a>, RpcResponseValidationError> {
    let row_path = |field: &str| format!("buckets[{index}].{field}");
    let bucket_id = clean_string_field(row, "bucket_id")?;
    clean_string_field(row, "source_series_id")?;
    clean_string_field(row, "source_domain")?;
    clean_string_field(row, "policy_hash")?;
    u64_field(row, "gross_receipt_atoms")?;
    let counted_value_atoms = u64_field(row, "counted_value_atoms")?;
    let outstanding = u64_field(row, "outstanding_vault_bridge_atoms")?;
    let nav_subscription = u64_field(row, "nav_subscription_allocations_atoms")?;
    let redemption_queue = u64_field(row, "redemption_queue_atoms")?;
    let other = u64_field(row, "other_allocations_atoms")?;
    let unallocated = u64_field(row, "unallocated_counted_capacity_atoms")?;
    let impairment_factor_bps = u64_field(row, "impairment_factor_bps")?;
    let redeemable_claim_atoms = u64_field(row, "redeemable_claim_atoms")?;
    let redeemable_at_par = bool_field(row, "redeemable_at_par")?;
    let status = clean_string_field(row, "status")?;
    u64_field(row, "last_packet_epoch")?;
    u64_field(row, "last_updated_height")?;

    let allocated_atoms = outstanding
        .checked_add(nav_subscription)
        .and_then(|value| value.checked_add(redemption_queue))
        .and_then(|value| value.checked_add(other))
        .ok_or_else(|| {
            invalid_result(
                row_path("outstanding_vault_bridge_atoms"),
                "bucket allocation components overflowed",
            )
        })?;
    let active = status == VAULT_BRIDGE_STATUS_BUCKET_ACTIVE;
    let expected_unallocated = if active {
        counted_value_atoms.checked_sub(allocated_atoms).ok_or_else(|| {
            invalid_result(
                row_path("unallocated_counted_capacity_atoms"),
                "active bucket allocated atoms exceed counted value",
            )
        })?
    } else {
        0
    };
    if unallocated != expected_unallocated {
        return Err(invalid_result(
            row_path("unallocated_counted_capacity_atoms"),
            "expected counted value minus allocated atoms for active buckets and zero otherwise",
        ));
    }
    if redeemable_at_par != (active && impairment_factor_bps == VAULT_BRIDGE_STATUS_PAR_BPS) {
        return Err(invalid_result(
            row_path("redeemable_at_par"),
            "expected redeemable_at_par only for active buckets at a 10000 bps impairment factor",
        ));
    }
    let expected_claim = u64::try_from(
        u128::from(allocated_atoms) * u128::from(impairment_factor_bps)
            / u128::from(VAULT_BRIDGE_STATUS_PAR_BPS),
    )
    .map_err(|_| {
        invalid_result(
            row_path("redeemable_claim_atoms"),
            "redeemable claim overflowed",
        )
    })?;
    if redeemable_claim_atoms != expected_claim {
        return Err(invalid_result(
            row_path("redeemable_claim_atoms"),
            "expected allocated atoms scaled by the impairment factor",
        ));
    }
    Ok(VaultBridgeStatusBucketSummary {
        bucket_id,
        active,
        counts_toward_counted_value: active || status == VAULT_BRIDGE_STATUS_BUCKET_IMPAIRED,
        counted_value_atoms,
        redeemable_at_par,
        allocated_atoms,
    })
}

fn validate_vault_bridge_status_receipt_row(
    row: &Value,
    index: usize,
    active_bucket_ids: &[&str],
) -> Result<u64, RpcResponseValidationError> {
    let row_path = |field: &str| format!("receipts[{index}].{field}");
    clean_string_field(row, "receipt_id")?;
    let bucket_id = clean_string_field(row, "bucket_id")?;
    clean_string_field(row, "source_domain")?;
    clean_string_field(row, "source_asset")?;
    clean_string_field(row, "claim_type")?;
    u64_field(row, "amount_atoms")?;
    u64_field(row, "haircut_bps")?;
    let counted_value_atoms = u64_field(row, "counted_value_atoms")?;
    let allocated_value_atoms = u64_field(row, "allocated_value_atoms")?;
    let unallocated_value_atoms = u64_field(row, "unallocated_value_atoms")?;
    clean_string_field(row, "status")?;
    u64_field(row, "created_at_height")?;
    u64_field(row, "counted_at_height")?;
    u64_field(row, "expires_at_height")?;
    clean_string_field_allow_empty(row, "source_tx_or_attestation")?;
    clean_string_field_allow_empty(row, "finality_ref")?;
    clean_string_field_allow_empty(row, "vault_id")?;
    if row
        .get("bridge_deposit_evidence_root")
        .is_some_and(|value| !value.is_null())
    {
        clean_string_field(row, "bridge_deposit_evidence_root")?;
    }
    let available = counted_value_atoms
        .checked_sub(allocated_value_atoms)
        .ok_or_else(|| {
            invalid_result(
                row_path("allocated_value_atoms"),
                "receipt allocated value exceeds counted value",
            )
        })?;
    let expected_unallocated = if active_bucket_ids.contains(&bucket_id) {
        available
    } else {
        0
    };
    if unallocated_value_atoms != expected_unallocated {
        return Err(invalid_result(
            row_path("unallocated_value_atoms"),
            "expected counted minus allocated value for receipts in active buckets and zero otherwise",
        ));
    }
    Ok(unallocated_value_atoms)
}

fn validate_vault_bridge_status_deposit_row(
    row: &Value,
    index: usize,
) -> Result<(), RpcResponseValidationError> {
    let row_path = |field: &str| format!("bridge_deposits[{index}].{field}");
    clean_string_field(row, "evidence_root")?;
    clean_string_field(row, "policy_hash")?;
    clean_string_field(row, "source_proof_kind")?;
    clean_string_field(row, "source_proof_hash")?;
    clean_string_field(row, "source_public_values_hash")?;
    u64_field(row, "source_chain_id")?;
    clean_string_field(row, "vault_address")?;
    clean_string_field(row, "token_address")?;
    clean_string_field(row, "depositor")?;
    clean_string_field(row, "pftl_recipient")?;
    u64_field(row, "amount_atoms")?;
    clean_string_field(row, "deposit_id")?;
    clean_string_field(row, "block_hash")?;
    clean_string_field(row, "tx_hash")?;
    u64_field(row, "log_index")?;
    clean_string_field(row, "proposer")?;
    clean_string_field(row, "status")?;
    u64_field(row, "submitted_at_height")?;
    u64_field(row, "finalized_at_height")?;
    u64_field(row, "expires_at_height")?;
    clean_string_field_allow_empty(row, "challenger")?;
    clean_string_field_allow_empty(row, "challenge_hash")?;
    u64_field(row, "challenge_bond")?;
    let pass_attestation_count = u64_field(row, "pass_attestation_count")?;
    let fail_attestation_count = u64_field(row, "fail_attestation_count")?;
    let mut passes = 0_u64;
    let mut fails = 0_u64;
    for attestation in array_field(row, "attestations")? {
        clean_string_field(attestation, "attestor")?;
        let pass = bool_field(attestation, "pass")?;
        clean_string_field(attestation, "observation_root")?;
        u64_field(attestation, "attested_at_height")?;
        if pass {
            passes += 1;
        } else {
            fails += 1;
        }
    }
    if pass_attestation_count != passes {
        return Err(invalid_result(
            row_path("pass_attestation_count"),
            "expected the number of passing attestations",
        ));
    }
    if fail_attestation_count != fails {
        return Err(invalid_result(
            row_path("fail_attestation_count"),
            "expected the number of failing attestations",
        ));
    }
    Ok(())
}

fn validate_vault_bridge_status_allocation_row(
    row: &Value,
    index: usize,
) -> Result<(), RpcResponseValidationError> {
    clean_string_field(row, "allocation_id")?;
    clean_string_field(row, "receipt_id")?;
    clean_string_field(row, "bucket_id")?;
    let amount_atoms = u64_field(row, "amount_atoms")?;
    let released_atoms = u64_field(row, "released_atoms")?;
    let remaining_atoms = u64_field(row, "remaining_atoms")?;
    clean_string_field(row, "purpose")?;
    clean_string_field(row, "consumer_id")?;
    u64_field(row, "created_at_height")?;
    u64_field(row, "retired_at_height")?;
    if amount_atoms.checked_sub(released_atoms) != Some(remaining_atoms) {
        return Err(invalid_result(
            format!("allocations[{index}].remaining_atoms"),
            "expected amount minus released atoms",
        ));
    }
    Ok(())
}

fn validate_vault_bridge_status_redemption_row(
    row: &Value,
) -> Result<(), RpcResponseValidationError> {
    clean_string_field(row, "redemption_id")?;
    clean_string_field(row, "owner")?;
    u64_field(row, "owner_sequence")?;
    clean_string_field(row, "issuer")?;
    clean_string_field(row, "bucket_id")?;
    u64_field(row, "amount_atoms")?;
    u64_field(row, "epoch")?;
    clean_string_field_allow_empty(row, "reserve_packet_hash")?;
    clean_string_field_allow_empty(row, "destination_ref")?;
    u64_field(row, "settled_atoms")?;
    clean_string_field(row, "state")?;
    u64_field(row, "created_at_height")?;
    clean_string_field_allow_empty(row, "settlement_receipt_hash")?;
    clean_string_field_allow_empty(row, "burn_tx_id")?;
    clean_string_field_allow_empty(row, "withdrawal_recipient")?;
    clean_string_field_allow_empty(row, "withdrawal_evidence_root")?;
    clean_string_field_allow_empty(row, "withdrawal_packet_hash")?;
    clean_string_field_allow_empty(row, "withdrawal_packet_evm_digest")?;
    Ok(())
}
