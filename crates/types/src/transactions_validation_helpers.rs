fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

fn validate_chain_id(chain_id: &str) -> Result<(), String> {
    validate_text_field("genesis chain_id", chain_id)
}

fn validate_text_field(field: &str, value: &str) -> Result<(), String> {
    if value.trim().is_empty() {
        return Err(format!("{field} must be nonempty"));
    }
    if value != value.trim() {
        return Err(format!(
            "{field} must not have leading or trailing whitespace"
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} must not contain control characters"));
    }
    if value.len() > MAX_TEXT_FIELD_BYTES {
        return Err(format!(
            "{field} must not exceed {MAX_TEXT_FIELD_BYTES} bytes"
        ));
    }
    Ok(())
}

fn validate_optional_text_field(field: &str, value: &str, max_bytes: usize) -> Result<(), String> {
    if value.is_empty() {
        return Ok(());
    }
    if value != value.trim() {
        return Err(format!(
            "{field} must not have leading or trailing whitespace"
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(format!("{field} must not contain control characters"));
    }
    if value.len() > max_bytes {
        return Err(format!("{field} must not exceed {max_bytes} bytes"));
    }
    Ok(())
}

fn validate_issued_asset_code(code: &str) -> Result<(), String> {
    validate_text_field("asset.code", code)?;
    if code.len() > MAX_ISSUED_ASSET_CODE_BYTES {
        return Err(format!(
            "asset.code must not exceed {MAX_ISSUED_ASSET_CODE_BYTES} bytes"
        ));
    }
    Ok(())
}

fn validate_nft_collection_id(collection_id: &str) -> Result<(), String> {
    validate_text_field("nft.collection_id", collection_id)?;
    if collection_id.len() > MAX_NFT_COLLECTION_ID_BYTES {
        return Err(format!(
            "nft.collection_id must not exceed {MAX_NFT_COLLECTION_ID_BYTES} bytes"
        ));
    }
    Ok(())
}

fn validate_escrow_asset_id(asset_id: &str) -> Result<(), String> {
    validate_text_field("escrow.asset_id", asset_id)?;
    if asset_id == "PFT" {
        return Ok(());
    }
    validate_lower_hex_len("escrow.asset_id", asset_id, ISSUED_ASSET_ID_HEX_LEN)
}

fn validate_dex_asset_id(field: &str, asset_id: &str) -> Result<(), String> {
    validate_text_field(field, asset_id)?;
    if asset_id == "PFT" {
        return Ok(());
    }
    validate_lower_hex_len(field, asset_id, ISSUED_ASSET_ID_HEX_LEN)
}

fn validate_escrow_state_value(state: &str) -> Result<(), String> {
    validate_text_field("escrow.state", state)?;
    match state {
        ESCROW_STATE_OPEN | ESCROW_STATE_FINISHED | ESCROW_STATE_CANCELED => Ok(()),
        _ => Err("escrow.state must be open, finished, or canceled".to_string()),
    }
}

fn validate_offer_state_value(state: &str) -> Result<(), String> {
    validate_text_field("offer.state", state)?;
    match state {
        OFFER_STATE_OPEN | OFFER_STATE_FILLED | OFFER_STATE_CANCELED | OFFER_STATE_UNFUNDED => {
            Ok(())
        }
        _ => Err("offer.state must be open, filled, canceled, or unfunded".to_string()),
    }
}

fn append_market_ops_envelope_signing_bytes(bytes: &mut Vec<u8>, envelope: &MarketOpsEnvelope) {
    append_signing_line(
        bytes,
        "envelope.encoding_version",
        envelope.encoding_version,
    );
    append_signing_line(bytes, "envelope.chain_id", envelope.chain_id);
    append_signing_line(
        bytes,
        "envelope.adapter_address",
        bytes_to_lower_hex(&envelope.adapter_address),
    );
    append_signing_line(
        bytes,
        "envelope.vault_address",
        bytes_to_lower_hex(&envelope.vault_address),
    );
    append_signing_line(
        bytes,
        "envelope.mint_controller_address",
        bytes_to_lower_hex(&envelope.mint_controller_address),
    );
    append_signing_line(
        bytes,
        "envelope.asset_id",
        bytes_to_lower_hex(&envelope.asset_id),
    );
    append_signing_line(bytes, "envelope.epoch", envelope.epoch);
    append_signing_line(
        bytes,
        "envelope.program_id",
        bytes_to_lower_hex(&envelope.program_id),
    );
    append_signing_line(
        bytes,
        "envelope.policy_hash",
        bytes_to_lower_hex(&envelope.policy_hash),
    );
    append_signing_line(
        bytes,
        "envelope.parameter_hash",
        bytes_to_lower_hex(&envelope.parameter_hash),
    );
    append_signing_line(
        bytes,
        "envelope.reserve_packet_hash",
        bytes_to_lower_hex(&envelope.reserve_packet_hash),
    );
    append_signing_line(
        bytes,
        "envelope.supply_packet_hash",
        bytes_to_lower_hex(&envelope.supply_packet_hash),
    );
    append_signing_line(
        bytes,
        "envelope.evidence_root",
        bytes_to_lower_hex(&envelope.evidence_root),
    );
    append_signing_line(
        bytes,
        "envelope.previous_market_state_hash",
        bytes_to_lower_hex(&envelope.previous_market_state_hash),
    );
    append_signing_line(
        bytes,
        "envelope.venue_id",
        bytes_to_lower_hex(&envelope.venue_id),
    );
    append_signing_line(
        bytes,
        "envelope.pool_config_hash",
        bytes_to_lower_hex(&envelope.pool_config_hash),
    );
    append_signing_line(
        bytes,
        "envelope.hook_code_hash",
        bytes_to_lower_hex(&envelope.hook_code_hash),
    );
    append_signing_line(
        bytes,
        "envelope.nav_floor_usd_e8",
        envelope.nav_floor_usd_e8,
    );
    append_signing_line(
        bytes,
        "envelope.valid_global_supply_atoms",
        envelope.valid_global_supply_atoms,
    );
    append_signing_line(
        bytes,
        "envelope.verified_net_assets_usd_e8",
        envelope.verified_net_assets_usd_e8,
    );
    append_signing_line(
        bytes,
        "envelope.funded_alignment_reserve_usd_e8",
        envelope.funded_alignment_reserve_usd_e8,
    );
    append_signing_line(
        bytes,
        "envelope.required_alignment_reserve_usd_e8",
        envelope.required_alignment_reserve_usd_e8,
    );
    append_signing_line(
        bytes,
        "envelope.max_reserve_deploy_usd_e8",
        envelope.max_reserve_deploy_usd_e8,
    );
    append_signing_line(bytes, "envelope.max_mint_atoms", envelope.max_mint_atoms);
    append_signing_line(
        bytes,
        "envelope.discount_trigger_bps",
        envelope.discount_trigger_bps,
    );
    append_signing_line(
        bytes,
        "envelope.premium_trigger_bps",
        envelope.premium_trigger_bps,
    );
    append_signing_line(
        bytes,
        "envelope.data_window_start",
        envelope.data_window_start,
    );
    append_signing_line(bytes, "envelope.data_window_end", envelope.data_window_end);
    append_signing_line(bytes, "envelope.valid_after", envelope.valid_after);
    append_signing_line(bytes, "envelope.expires_at", envelope.expires_at);
    append_signing_line(
        bytes,
        "envelope.cooldown_seconds",
        envelope.cooldown_seconds,
    );
    append_signing_line(bytes, "envelope.nonce", bytes_to_lower_hex(&envelope.nonce));
}

fn append_market_ops_policy_registration_signing_bytes(
    bytes: &mut Vec<u8>,
    policy: &MarketOpsPolicyRegistration,
) {
    append_signing_line(
        bytes,
        "policy.program_id",
        bytes_to_lower_hex(&policy.program_id),
    );
    append_signing_line(
        bytes,
        "policy.policy_hash",
        bytes_to_lower_hex(&policy.policy_hash),
    );
    append_signing_line(
        bytes,
        "policy.parameter_hash",
        bytes_to_lower_hex(&policy.parameter_hash),
    );
    append_signing_line(
        bytes,
        "policy.venue_id",
        bytes_to_lower_hex(&policy.venue_id),
    );
    append_signing_line(
        bytes,
        "policy.pool_config_hash",
        bytes_to_lower_hex(&policy.pool_config_hash),
    );
    append_signing_line(
        bytes,
        "policy.hook_code_hash",
        bytes_to_lower_hex(&policy.hook_code_hash),
    );
    append_signing_line(bytes, "policy.activation_epoch", policy.activation_epoch);
    append_signing_line(
        bytes,
        "policy.deactivation_epoch",
        policy.deactivation_epoch,
    );
}

fn append_market_ops_policy_inputs_signing_bytes(
    bytes: &mut Vec<u8>,
    inputs: &MarketOpsPolicyInputs,
) {
    append_signing_line(bytes, "policy_inputs.unit_scale", inputs.unit_scale);
    append_signing_line(
        bytes,
        "policy_inputs.floor_factor_bps",
        inputs.floor_factor_bps,
    );
    append_signing_line(
        bytes,
        "policy_inputs.alignment.policy_min_usd_e8",
        inputs.alignment_params.policy_min_usd_e8,
    );
    append_signing_line(
        bytes,
        "policy_inputs.alignment.min_alignment_bps",
        inputs.alignment_params.min_alignment_bps,
    );
    append_signing_line(
        bytes,
        "policy_inputs.alignment.stress_repeat_factor_14d",
        inputs.alignment_params.stress_repeat_factor_14d,
    );
    append_signing_line(
        bytes,
        "policy_inputs.alignment.stress_repeat_factor_90d",
        inputs.alignment_params.stress_repeat_factor_90d,
    );
    append_signing_line(
        bytes,
        "policy_inputs.alignment.stale_epochs_allowed",
        inputs.alignment_params.stale_epochs_allowed,
    );
    append_signing_line(
        bytes,
        "policy_inputs.alignment.max_decay_per_epoch_bps",
        inputs.alignment_params.max_decay_per_epoch_bps,
    );
    append_signing_line(
        bytes,
        "policy_inputs.previous_required_alignment_reserve_usd_e8",
        inputs.previous_required_alignment_reserve_usd_e8,
    );
    append_u128_list_signing_bytes(
        bytes,
        "policy_inputs.cost_to_restore_14d_usd_e8",
        &inputs.cost_to_restore_14d_usd_e8,
    );
    append_u128_list_signing_bytes(
        bytes,
        "policy_inputs.cost_to_restore_90d_usd_e8",
        &inputs.cost_to_restore_90d_usd_e8,
    );
    append_signing_line(
        bytes,
        "policy_inputs.reserve_limits.available_alignment_reserve_usd_e8",
        inputs.reserve_limits.available_alignment_reserve_usd_e8,
    );
    append_signing_line(
        bytes,
        "policy_inputs.reserve_limits.venue_policy_cap_usd_e8",
        inputs.reserve_limits.venue_policy_cap_usd_e8,
    );
    append_signing_line(
        bytes,
        "policy_inputs.reserve_limits.depth_limited_cap_usd_e8",
        inputs.reserve_limits.depth_limited_cap_usd_e8,
    );
    append_signing_line(
        bytes,
        "policy_inputs.reserve_limits.cooldown_limited_cap_usd_e8",
        inputs.reserve_limits.cooldown_limited_cap_usd_e8,
    );
    append_signing_line(
        bytes,
        "policy_inputs.mint_limits.policy_max_mint_atoms",
        inputs.mint_limits.policy_max_mint_atoms,
    );
    append_signing_line(
        bytes,
        "policy_inputs.mint_limits.venue_bid_depth_atoms",
        inputs.mint_limits.venue_bid_depth_atoms,
    );
    append_signing_line(
        bytes,
        "policy_inputs.mint_limits.cooldown_mint_atoms",
        inputs.mint_limits.cooldown_mint_atoms,
    );
    append_market_ops_observations_signing_bytes(
        bytes,
        "policy_inputs.discount_observation",
        &inputs.discount_observations,
    );
    append_market_ops_observations_signing_bytes(
        bytes,
        "policy_inputs.premium_observation",
        &inputs.premium_observations,
    );
}

fn append_u128_list_signing_bytes(bytes: &mut Vec<u8>, label: &str, values: &[u128]) {
    append_signing_line(bytes, &format!("{label}.count"), values.len());
    for (index, value) in values.iter().enumerate() {
        append_signing_line(bytes, &format!("{label}.{index}"), *value);
    }
}

fn append_market_ops_observations_signing_bytes(
    bytes: &mut Vec<u8>,
    label: &str,
    observations: &[MarketOpsVenueObservation],
) {
    append_signing_line(bytes, &format!("{label}.count"), observations.len());
    for (index, observation) in observations.iter().enumerate() {
        append_signing_line(
            bytes,
            &format!("{label}.{index}.dt_seconds"),
            observation.dt_seconds,
        );
        append_signing_line(
            bytes,
            &format!("{label}.{index}.price_usd_e8"),
            observation.price_usd_e8,
        );
        append_signing_line(
            bytes,
            &format!("{label}.{index}.volume_usd_e8"),
            observation.volume_usd_e8,
        );
    }
}

fn append_signing_line(bytes: &mut Vec<u8>, label: &str, value: impl std::fmt::Display) {
    bytes.extend_from_slice(label.as_bytes());
    bytes.push(b'=');
    bytes.extend_from_slice(value.to_string().as_bytes());
    bytes.push(b'\n');
}

fn validate_lower_hex_len(field: &str, value: &str, len: usize) -> Result<(), String> {
    if value.len() != len
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{field} must be {len} lowercase hex characters"));
    }
    Ok(())
}

fn validate_postfiat_address(field: &str, value: &str) -> Result<(), String> {
    let Some(payload) = value.strip_prefix("pf") else {
        return Err(format!("{field} must start with `pf`"));
    };
    validate_lower_hex_len(field, payload, 40)
}

fn validate_lower_hex_max(field: &str, value: &str, max_len: usize) -> Result<(), String> {
    if value.is_empty() {
        return Err(format!("{field} must be nonempty lowercase hex"));
    }
    if value.len() > max_len {
        return Err(format!(
            "{field} must not exceed {max_len} lowercase hex characters"
        ));
    }
    if !value.len().is_multiple_of(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{field} must be even-length lowercase hex"));
    }
    Ok(())
}

fn validate_optional_lower_hex_max(
    field: &str,
    value: &str,
    max_bytes: usize,
) -> Result<(), String> {
    if value.len() > max_bytes.saturating_mul(2) {
        return Err(format!(
            "{field} must not exceed {max_bytes} bytes encoded as lowercase hex"
        ));
    }
    if !value.len().is_multiple_of(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{field} must be even-length lowercase hex"));
    }
    Ok(())
}

fn hex_encoded_byte_len(value: &str) -> usize {
    value.len() / 2
}

fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn hash_hex_domain(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha3_384::new();
    hasher.update(domain.as_bytes());
    hasher.update([0u8]);
    hasher.update(bytes);
    bytes_to_hex(&hasher.finalize())
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}
