pub fn market_ops_asset_id(asset_id: &str) -> Result<[u8; 32], String> {
    validate_lower_hex_len("market_ops.asset_id", asset_id, ISSUED_ASSET_ID_HEX_LEN)?;
    let preimage = format!("asset_id={asset_id}\n");
    Ok(hash32_domain(
        MARKET_OPS_ASSET_ID_DOMAIN,
        preimage.as_bytes(),
    ))
}

pub fn market_ops_reserve_packet_hash(reserve_packet_hash: &str) -> Result<[u8; 32], String> {
    validate_lower_hex_len(
        "market_ops.reserve_packet_hash",
        reserve_packet_hash,
        NAV_RESERVE_PACKET_ID_HEX_LEN,
    )?;
    let preimage = format!("reserve_packet_hash={reserve_packet_hash}\n");
    Ok(hash32_domain(
        MARKET_OPS_RESERVE_PACKET_HASH_DOMAIN,
        preimage.as_bytes(),
    ))
}

pub fn market_ops_supply_packet_hash(
    asset_id: &str,
    epoch: u64,
    valid_global_supply_atoms: u128,
) -> Result<[u8; 32], String> {
    validate_lower_hex_len("market_ops.asset_id", asset_id, ISSUED_ASSET_ID_HEX_LEN)?;
    if epoch == 0 {
        return Err("market_ops.epoch must be nonzero".to_string());
    }
    let preimage = format!(
        "asset_id={asset_id}\nepoch={epoch}\nvalid_global_supply_atoms={valid_global_supply_atoms}\n"
    );
    Ok(hash32_domain(
        MARKET_OPS_SUPPLY_PACKET_HASH_DOMAIN,
        preimage.as_bytes(),
    ))
}

pub fn market_ops_evidence_root(
    discount_observations: &[MarketOpsVenueObservation],
    premium_observations: &[MarketOpsVenueObservation],
) -> Result<[u8; 32], String> {
    if discount_observations.len() > MAX_MARKET_OPS_OBSERVATIONS {
        return Err(format!(
            "market_ops.discount_observations exceeds maximum of {MAX_MARKET_OPS_OBSERVATIONS}"
        ));
    }
    if premium_observations.len() > MAX_MARKET_OPS_OBSERVATIONS {
        return Err(format!(
            "market_ops.premium_observations exceeds maximum of {MAX_MARKET_OPS_OBSERVATIONS}"
        ));
    }
    let mut preimage = Vec::new();
    append_market_ops_observations_preimage(&mut preimage, b"discount", discount_observations)?;
    append_market_ops_observations_preimage(&mut preimage, b"premium", premium_observations)?;
    Ok(hash32_domain(MARKET_OPS_EVIDENCE_ROOT_DOMAIN, &preimage))
}

pub fn market_ops_evm_evidence_root(
    bundle: &MarketOpsEvmEvidenceBundle,
) -> Result<[u8; 32], String> {
    bundle.validate()?;
    let mut preimage = Vec::new();
    append_market_ops_evm_line(&mut preimage, "encoding_version", bundle.encoding_version);
    append_market_ops_evm_line(&mut preimage, "chain_id", bundle.chain_id);
    append_market_ops_evm_hex(&mut preimage, "venue_id", &bundle.venue_id);
    append_market_ops_evm_hex(&mut preimage, "pool_id", &bundle.pool_id);
    append_market_ops_evm_hex(&mut preimage, "pool_manager", &bundle.pool_manager);
    append_market_ops_evm_hex(&mut preimage, "hook_address", &bundle.hook_address);
    append_market_ops_evm_hex(&mut preimage, "pool_config_hash", &bundle.pool_config_hash);
    append_market_ops_evm_hex(&mut preimage, "hook_code_hash", &bundle.hook_code_hash);

    append_market_ops_evm_line(&mut preimage, "headers.count", bundle.headers.len());
    for (index, header) in bundle.headers.iter().enumerate() {
        let prefix = format!("headers.{index}");
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.block_number"),
            header.block_number,
        );
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.block_hash"),
            &header.block_hash,
        );
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.parent_hash"),
            &header.parent_hash,
        );
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.state_root"),
            &header.state_root,
        );
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.receipts_root"),
            &header.receipts_root,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.timestamp"),
            header.timestamp,
        );
    }

    append_market_ops_evm_line(&mut preimage, "receipts.count", bundle.receipts.len());
    for (index, receipt) in bundle.receipts.iter().enumerate() {
        let prefix = format!("receipts.{index}");
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.block_number"),
            receipt.block_number,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.transaction_index"),
            receipt.transaction_index,
        );
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.receipt_hash"),
            &receipt.receipt_hash,
        );
        append_market_ops_evm_line(&mut preimage, &format!("{prefix}.status"), receipt.status);
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.logs_root"),
            &receipt.logs_root,
        );
    }

    append_market_ops_evm_line(&mut preimage, "logs.count", bundle.logs.len());
    for (index, log) in bundle.logs.iter().enumerate() {
        let prefix = format!("logs.{index}");
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.block_number"),
            log.block_number,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.transaction_index"),
            log.transaction_index,
        );
        append_market_ops_evm_line(&mut preimage, &format!("{prefix}.log_index"), log.log_index);
        append_market_ops_evm_hex(&mut preimage, &format!("{prefix}.address"), &log.address);
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.topics.count"),
            log.topics.len(),
        );
        for (topic_index, topic) in log.topics.iter().enumerate() {
            append_market_ops_evm_hex(
                &mut preimage,
                &format!("{prefix}.topics.{topic_index}"),
                topic,
            );
        }
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.data_hash"),
            &log.data_hash,
        );
    }

    append_market_ops_evm_line(
        &mut preimage,
        "hook_checkpoints.count",
        bundle.hook_checkpoints.len(),
    );
    for (index, checkpoint) in bundle.hook_checkpoints.iter().enumerate() {
        let prefix = format!("hook_checkpoints.{index}");
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.block_number"),
            checkpoint.block_number,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.log_index"),
            checkpoint.log_index,
        );
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.pool_id"),
            &checkpoint.pool_id,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.checkpoint_count"),
            checkpoint.checkpoint_count,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.swap_count"),
            checkpoint.swap_count,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.depth_count"),
            checkpoint.depth_count,
        );
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.swap_root"),
            &checkpoint.swap_root,
        );
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.depth_root"),
            &checkpoint.depth_root,
        );
        append_market_ops_evm_hex(
            &mut preimage,
            &format!("{prefix}.pftl_state_hash"),
            &checkpoint.pftl_state_hash,
        );
    }

    append_market_ops_evm_line(&mut preimage, "pool_states.count", bundle.pool_states.len());
    for (index, pool_state) in bundle.pool_states.iter().enumerate() {
        let prefix = format!("pool_states.{index}");
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.block_number"),
            pool_state.block_number,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.observation_sequence"),
            pool_state.observation_sequence,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.timestamp"),
            pool_state.timestamp,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.dt_seconds"),
            pool_state.dt_seconds,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.checkpoint_count"),
            pool_state.checkpoint_count,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.price_usd_e8"),
            pool_state.price_usd_e8,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.volume_usd_e8"),
            pool_state.volume_usd_e8,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.zero_for_one"),
            pool_state.zero_for_one,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.fee_bps"),
            pool_state.fee_bps,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.liquidity"),
            pool_state.liquidity,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.base_reserve_atoms"),
            pool_state.base_reserve_atoms,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.quote_reserve_usd_e8"),
            pool_state.quote_reserve_usd_e8,
        );
        append_market_ops_evm_line(
            &mut preimage,
            &format!("{prefix}.replayable"),
            pool_state.replayable,
        );
    }

    Ok(hash32_domain(
        MARKET_OPS_EVM_EVIDENCE_ROOT_DOMAIN,
        &preimage,
    ))
}

fn hash32_domain(domain: &str, bytes: &[u8]) -> [u8; 32] {
    let mut hasher = Sha3_256::new();
    hasher.update(domain.as_bytes());
    hasher.update([0u8]);
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut output = [0u8; 32];
    output.copy_from_slice(&digest);
    output
}

fn append_market_ops_observations_preimage(
    out: &mut Vec<u8>,
    label: &[u8],
    observations: &[MarketOpsVenueObservation],
) -> Result<(), String> {
    out.extend_from_slice(label);
    out.push(b'\n');
    out.extend_from_slice(b"count=");
    out.extend_from_slice(observations.len().to_string().as_bytes());
    out.push(b'\n');
    for (index, observation) in observations.iter().enumerate() {
        observation.validate()?;
        out.extend_from_slice(b"index=");
        out.extend_from_slice(index.to_string().as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"dt_seconds=");
        out.extend_from_slice(observation.dt_seconds.to_string().as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"price_usd_e8=");
        out.extend_from_slice(observation.price_usd_e8.to_string().as_bytes());
        out.push(b'\n');
        out.extend_from_slice(b"volume_usd_e8=");
        out.extend_from_slice(observation.volume_usd_e8.to_string().as_bytes());
        out.push(b'\n');
    }
    Ok(())
}

fn append_market_ops_evm_line(out: &mut Vec<u8>, label: &str, value: impl std::fmt::Display) {
    out.extend_from_slice(label.as_bytes());
    out.push(b'=');
    out.extend_from_slice(value.to_string().as_bytes());
    out.push(b'\n');
}

fn append_market_ops_evm_hex(out: &mut Vec<u8>, label: &str, bytes: &[u8]) {
    append_market_ops_evm_line(out, label, bytes_to_lower_hex(bytes));
}

fn validate_nonempty_bounded(field: &str, len: usize, max: usize) -> Result<(), String> {
    if len == 0 {
        return Err(format!("{field} must be nonempty"));
    }
    if len > max {
        return Err(format!("{field} exceeds maximum of {max}"));
    }
    Ok(())
}

fn is_zero_20(value: &[u8; 20]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn is_zero_32(value: &[u8; 32]) -> bool {
    value.iter().all(|byte| *byte == 0)
}

fn hash_u32(hasher: &mut Sha3_384, value: u32) {
    hasher.update(value.to_be_bytes());
}

fn hash_u64(hasher: &mut Sha3_384, value: u64) {
    hasher.update(value.to_be_bytes());
}

fn hash_uint256_from_u128(hasher: &mut Sha3_384, value: u128) {
    let mut word = [0u8; 32];
    word[16..].copy_from_slice(&value.to_be_bytes());
    hasher.update(word);
}

impl NavProofProfile {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        registered_by: impl Into<String>,
        verifier_kind: impl Into<String>,
        source_class: impl Into<String>,
        max_snapshot_age_blocks: u64,
        challenge_window_blocks: u64,
        max_epoch_gap_blocks: u64,
        settle_deadline_blocks: u64,
        min_challenge_bond: u64,
        min_attestations: u64,
        tolerance_bp: u64,
        valuation_policy_hash: impl Into<String>,
        sp1_program_vkey: impl Into<String>,
        sp1_proof_encoding: impl Into<String>,
        max_proof_bytes: u64,
        max_public_values_bytes: u64,
    ) -> Result<Self, String> {
        Self::new_with_bridge_observer_min_confirmations(
            registered_by,
            verifier_kind,
            source_class,
            max_snapshot_age_blocks,
            challenge_window_blocks,
            max_epoch_gap_blocks,
            settle_deadline_blocks,
            min_challenge_bond,
            min_attestations,
            tolerance_bp,
            0,
            valuation_policy_hash,
            sp1_program_vkey,
            sp1_proof_encoding,
            max_proof_bytes,
            max_public_values_bytes,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn new_with_bridge_observer_min_confirmations(
        registered_by: impl Into<String>,
        verifier_kind: impl Into<String>,
        source_class: impl Into<String>,
        max_snapshot_age_blocks: u64,
        challenge_window_blocks: u64,
        max_epoch_gap_blocks: u64,
        settle_deadline_blocks: u64,
        min_challenge_bond: u64,
        min_attestations: u64,
        tolerance_bp: u64,
        bridge_observer_min_confirmations: u64,
        valuation_policy_hash: impl Into<String>,
        sp1_program_vkey: impl Into<String>,
        sp1_proof_encoding: impl Into<String>,
        max_proof_bytes: u64,
        max_public_values_bytes: u64,
    ) -> Result<Self, String> {
        let verifier_kind = verifier_kind.into();
        let source_class = source_class.into();
        let valuation_policy_hash = valuation_policy_hash.into();
        let sp1_program_vkey = sp1_program_vkey.into();
        let sp1_proof_encoding = sp1_proof_encoding.into();
        let profile_id = nav_proof_profile_id_with_bridge_observer_min_confirmations(
            &verifier_kind,
            &source_class,
            max_snapshot_age_blocks,
            challenge_window_blocks,
            max_epoch_gap_blocks,
            settle_deadline_blocks,
            min_challenge_bond,
            min_attestations,
            tolerance_bp,
            bridge_observer_min_confirmations,
            &valuation_policy_hash,
            &sp1_program_vkey,
            &sp1_proof_encoding,
            max_proof_bytes,
            max_public_values_bytes,
        )?;
        let profile = Self {
            profile_id,
            registered_by: registered_by.into(),
            verifier_kind,
            source_class,
            max_snapshot_age_blocks,
            challenge_window_blocks,
            max_epoch_gap_blocks,
            settle_deadline_blocks,
            min_challenge_bond,
            min_attestations,
            tolerance_bp,
            bridge_observer_min_confirmations,
            valuation_policy_hash,
            vault_bridge_route_policy_hash: String::new(),
            sp1_program_vkey,
            sp1_proof_encoding,
            max_proof_bytes,
            max_public_values_bytes,
        };
        profile.validate()?;
        Ok(profile)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "nav_profile.profile_id",
            &self.profile_id,
            NAV_PROFILE_ID_HEX_LEN,
        )?;
        validate_text_field("nav_profile.registered_by", &self.registered_by)?;
        validate_nav_profile_verifier_kind(&self.verifier_kind)?;
        validate_text_field("nav_profile.source_class", &self.source_class)?;
        if self.verifier_kind == NAV_PROFILE_VERIFIER_MULTI_FETCH && self.min_attestations == 0 {
            return Err(
                "nav_profile.min_attestations must be nonzero for multi-fetch-quorum".to_string(),
            );
        }
        if self.bridge_observer_min_confirmations > 0
            && self.verifier_kind != NAV_PROFILE_VERIFIER_MULTI_FETCH
        {
            return Err(
                "nav_profile.bridge_observer_min_confirmations requires multi-fetch-quorum"
                    .to_string(),
            );
        }
        if self.bridge_observer_min_confirmations > 0
            && !self
                .source_class
                .starts_with(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
        {
            return Err(
                "nav_profile.bridge_observer_min_confirmations requires vault_bridge source_class"
                    .to_string(),
            );
        }
        if matches!(
            self.verifier_kind.as_str(),
            NAV_PROFILE_VERIFIER_SP1_GROTH16
                | NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
        )
            && self.valuation_policy_hash.is_empty()
        {
            return Err(
                "nav_profile.valuation_policy_hash is required for sp1-groth16".to_string(),
            );
        }
        validate_nav_profile_sp1_fields(
            &self.verifier_kind,
            &self.sp1_program_vkey,
            &self.sp1_proof_encoding,
        )?;
        if !self.valuation_policy_hash.is_empty() {
            let expected_len = if matches!(
                self.verifier_kind.as_str(),
                NAV_PROFILE_VERIFIER_SP1_GROTH16
                    | NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
            ) {
                NAV_SP1_POLICY_HASH_HEX_LEN
            } else {
                NAV_PROFILE_ID_HEX_LEN
            };
            validate_lower_hex_len(
                "nav_profile.valuation_policy_hash",
                &self.valuation_policy_hash,
                expected_len,
            )?;
        }
        if !self.vault_bridge_route_policy_hash.is_empty() {
            if !self
                .source_class
                .starts_with(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
            {
                return Err(
                    "nav_profile.vault_bridge_route_policy_hash requires vault_bridge source_class"
                        .to_string(),
                );
            }
            validate_lower_hex_len(
                "nav_profile.vault_bridge_route_policy_hash",
                &self.vault_bridge_route_policy_hash,
                NAV_PROFILE_ID_HEX_LEN,
            )?;
        }
        let expected = nav_proof_profile_id_with_route_policy(
            &self.verifier_kind,
            &self.source_class,
            self.max_snapshot_age_blocks,
            self.challenge_window_blocks,
            self.max_epoch_gap_blocks,
            self.settle_deadline_blocks,
            self.min_challenge_bond,
            self.min_attestations,
            self.tolerance_bp,
            self.bridge_observer_min_confirmations,
            &self.valuation_policy_hash,
            &self.sp1_program_vkey,
            &self.sp1_proof_encoding,
            self.max_proof_bytes,
            self.max_public_values_bytes,
            &self.vault_bridge_route_policy_hash,
        )?;
        if self.profile_id != expected {
            return Err("nav_profile.profile_id does not match profile parameters".to_string());
        }
        Ok(())
    }

    pub fn with_vault_bridge_route_policy_hash(
        mut self,
        route_policy_hash: impl Into<String>,
    ) -> Result<Self, String> {
        self.vault_bridge_route_policy_hash = route_policy_hash.into();
        self.profile_id = nav_proof_profile_id_with_route_policy(
            &self.verifier_kind,
            &self.source_class,
            self.max_snapshot_age_blocks,
            self.challenge_window_blocks,
            self.max_epoch_gap_blocks,
            self.settle_deadline_blocks,
            self.min_challenge_bond,
            self.min_attestations,
            self.tolerance_bp,
            self.bridge_observer_min_confirmations,
            &self.valuation_policy_hash,
            &self.sp1_program_vkey,
            &self.sp1_proof_encoding,
            self.max_proof_bytes,
            self.max_public_values_bytes,
            &self.vault_bridge_route_policy_hash,
        )?;
        self.validate()?;
        Ok(self)
    }
}

#[allow(clippy::too_many_arguments)]
pub fn nav_proof_profile_id(
    verifier_kind: &str,
    source_class: &str,
    max_snapshot_age_blocks: u64,
    challenge_window_blocks: u64,
    max_epoch_gap_blocks: u64,
    settle_deadline_blocks: u64,
    min_challenge_bond: u64,
    min_attestations: u64,
    tolerance_bp: u64,
    valuation_policy_hash: &str,
    sp1_program_vkey: &str,
    sp1_proof_encoding: &str,
    max_proof_bytes: u64,
    max_public_values_bytes: u64,
) -> Result<String, String> {
    nav_proof_profile_id_with_bridge_observer_min_confirmations(
        verifier_kind,
        source_class,
        max_snapshot_age_blocks,
        challenge_window_blocks,
        max_epoch_gap_blocks,
        settle_deadline_blocks,
        min_challenge_bond,
        min_attestations,
        tolerance_bp,
        0,
        valuation_policy_hash,
        sp1_program_vkey,
        sp1_proof_encoding,
        max_proof_bytes,
        max_public_values_bytes,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn nav_proof_profile_id_with_bridge_observer_min_confirmations(
    verifier_kind: &str,
    source_class: &str,
    max_snapshot_age_blocks: u64,
    challenge_window_blocks: u64,
    max_epoch_gap_blocks: u64,
    settle_deadline_blocks: u64,
    min_challenge_bond: u64,
    min_attestations: u64,
    tolerance_bp: u64,
    bridge_observer_min_confirmations: u64,
    valuation_policy_hash: &str,
    sp1_program_vkey: &str,
    sp1_proof_encoding: &str,
    max_proof_bytes: u64,
    max_public_values_bytes: u64,
) -> Result<String, String> {
    nav_proof_profile_id_with_route_policy(
        verifier_kind,
        source_class,
        max_snapshot_age_blocks,
        challenge_window_blocks,
        max_epoch_gap_blocks,
        settle_deadline_blocks,
        min_challenge_bond,
        min_attestations,
        tolerance_bp,
        bridge_observer_min_confirmations,
        valuation_policy_hash,
        sp1_program_vkey,
        sp1_proof_encoding,
        max_proof_bytes,
        max_public_values_bytes,
        "",
    )
}

#[allow(clippy::too_many_arguments)]
fn nav_proof_profile_id_with_route_policy(
    verifier_kind: &str,
    source_class: &str,
    max_snapshot_age_blocks: u64,
    challenge_window_blocks: u64,
    max_epoch_gap_blocks: u64,
    settle_deadline_blocks: u64,
    min_challenge_bond: u64,
    min_attestations: u64,
    tolerance_bp: u64,
    bridge_observer_min_confirmations: u64,
    valuation_policy_hash: &str,
    sp1_program_vkey: &str,
    sp1_proof_encoding: &str,
    max_proof_bytes: u64,
    max_public_values_bytes: u64,
    vault_bridge_route_policy_hash: &str,
) -> Result<String, String> {
    validate_nav_profile_verifier_kind(verifier_kind)?;
    validate_text_field("nav_profile.source_class", source_class)?;
    if bridge_observer_min_confirmations > 0
        && verifier_kind != NAV_PROFILE_VERIFIER_MULTI_FETCH
    {
        return Err(
            "nav_profile.bridge_observer_min_confirmations requires multi-fetch-quorum"
                .to_string(),
        );
    }
    if bridge_observer_min_confirmations > 0
        && !source_class.starts_with(VAULT_BRIDGE_PROFILE_SOURCE_CLASS_PREFIX)
    {
        return Err(
            "nav_profile.bridge_observer_min_confirmations requires vault_bridge source_class"
                .to_string(),
        );
    }
    if matches!(
        verifier_kind,
        NAV_PROFILE_VERIFIER_SP1_GROTH16 | NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
    ) && valuation_policy_hash.is_empty()
    {
        return Err("nav_profile.valuation_policy_hash is required for sp1-groth16".to_string());
    }
    validate_nav_profile_sp1_fields(verifier_kind, sp1_program_vkey, sp1_proof_encoding)?;
    let mut preimage = format!(
        "verifier_kind={verifier_kind}\nsource_class={source_class}\nmax_snapshot_age_blocks={max_snapshot_age_blocks}\nchallenge_window_blocks={challenge_window_blocks}\nmax_epoch_gap_blocks={max_epoch_gap_blocks}\nsettle_deadline_blocks={settle_deadline_blocks}\nmin_challenge_bond={min_challenge_bond}\nmin_attestations={min_attestations}\ntolerance_bp={tolerance_bp}\nvaluation_policy_hash={valuation_policy_hash}\nsp1_program_vkey={sp1_program_vkey}\nsp1_proof_encoding={sp1_proof_encoding}\nmax_proof_bytes={max_proof_bytes}\nmax_public_values_bytes={max_public_values_bytes}\n"
    );
    if bridge_observer_min_confirmations != 0 {
        preimage.push_str(&format!(
            "bridge_observer_min_confirmations={bridge_observer_min_confirmations}\n"
        ));
    }
    if !vault_bridge_route_policy_hash.is_empty() {
        preimage.push_str(&format!(
            "vault_bridge_route_policy_hash={vault_bridge_route_policy_hash}\n"
        ));
    }
    Ok(hash_hex_domain(NAV_PROFILE_ID_DOMAIN, preimage.as_bytes()))
}

fn validate_nav_profile_sp1_fields(
    verifier_kind: &str,
    sp1_program_vkey: &str,
    sp1_proof_encoding: &str,
) -> Result<(), String> {
    if matches!(
        verifier_kind,
        NAV_PROFILE_VERIFIER_SP1_GROTH16 | NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
    ) {
        if sp1_program_vkey.is_empty() {
            return Err("nav_profile.sp1_program_vkey is required for sp1-groth16".to_string());
        }
        if sp1_program_vkey.len() != NAV_SP1_PROGRAM_VKEY_HEX_LEN
            || !sp1_program_vkey.starts_with("0x")
            || !sp1_program_vkey[2..]
                .chars()
                .all(|ch| ch.is_ascii_hexdigit())
        {
            return Err(
                "nav_profile.sp1_program_vkey must be a 0x-prefixed 32-byte hex string".to_string(),
            );
        }
        if sp1_proof_encoding != NAV_SP1_PROOF_ENCODING_GROTH16 {
            return Err(format!(
                "nav_profile.sp1_proof_encoding must be {NAV_SP1_PROOF_ENCODING_GROTH16} for sp1-groth16"
            ));
        }
        return Ok(());
    }
    if !sp1_program_vkey.is_empty() || !sp1_proof_encoding.is_empty() {
        return Err("nav_profile sp1 fields are only valid for sp1-groth16 profiles".to_string());
    }
    Ok(())
}

fn validate_nav_profile_verifier_kind(verifier_kind: &str) -> Result<(), String> {
    match verifier_kind {
        NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT
        | NAV_PROFILE_VERIFIER_PLACEHOLDER
        | NAV_PROFILE_VERIFIER_MULTI_FETCH
        | NAV_PROFILE_VERIFIER_SP1_GROTH16
        | NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1 => Ok(()),
        _ => Err(format!(
            "nav_profile.verifier_kind must be one of [{NAV_PROFILE_VERIFIER_LEDGER_TRANSPARENT}, {NAV_PROFILE_VERIFIER_PLACEHOLDER}, {NAV_PROFILE_VERIFIER_MULTI_FETCH}, {NAV_PROFILE_VERIFIER_SP1_GROTH16}, {NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1}], got {verifier_kind}"
        )),
    }
}

pub fn nav_per_unit_floor(
    verified_net_assets: u64,
    circulating_supply: u64,
) -> Result<u64, String> {
    nav_per_unit_floor_with_unit_scale(verified_net_assets, circulating_supply, 1)
}

pub fn nav_per_unit_floor_with_unit_scale(
    verified_net_assets: u64,
    circulating_supply: u64,
    unit_scale: u128,
) -> Result<u64, String> {
    if circulating_supply == 0 {
        return Err("nav circulating_supply must be nonzero".to_string());
    }
    if unit_scale == 0 {
        return Err("nav unit_scale must be nonzero".to_string());
    }
    let numerator = u128::from(verified_net_assets)
        .checked_mul(unit_scale)
        .ok_or_else(|| "nav verified_net_assets times unit_scale would overflow".to_string())?;
    u64::try_from(numerator / u128::from(circulating_supply))
        .map_err(|_| "nav_per_unit exceeds u64".to_string())
}

pub fn nav_amount_claim_ceil(
    amount: u64,
    nav_per_unit: u64,
    unit_scale: u128,
) -> Result<u64, String> {
    if unit_scale == 0 {
        return Err("nav unit_scale must be nonzero".to_string());
    }
    let numerator = u128::from(amount)
        .checked_mul(u128::from(nav_per_unit))
        .ok_or_else(|| "nav amount times nav_per_unit would overflow".to_string())?;
    let claim = numerator
        .checked_add(unit_scale - 1)
        .ok_or_else(|| "nav amount claim rounding would overflow".to_string())?
        / unit_scale;
    u64::try_from(claim).map_err(|_| "nav amount claim exceeds u64".to_string())
}

/// Floating-NAV over-collateralization: reserves may exceed supply * nav_per_unit
/// when nav_per_unit is the floored per-unit price.
pub fn validate_nav_reserve_collateralization(
    verified_net_assets: u64,
    circulating_supply: u64,
    nav_per_unit: u64,
) -> Result<(), String> {
    let unit_scale = if nav_per_unit == VAULT_BRIDGE_UNIT {
        u128::from(VAULT_BRIDGE_UNIT)
    } else {
        1
    };
    validate_nav_reserve_collateralization_with_unit_scale(
        verified_net_assets,
        circulating_supply,
        nav_per_unit,
        unit_scale,
    )
}

pub fn validate_nav_reserve_collateralization_with_unit_scale(
    verified_net_assets: u64,
    circulating_supply: u64,
    nav_per_unit: u64,
    unit_scale: u128,
) -> Result<(), String> {
    if unit_scale == 0 {
        return Err("nav unit_scale must be nonzero".to_string());
    }
    let numerator = u128::from(circulating_supply)
        .checked_mul(u128::from(nav_per_unit))
        .ok_or_else(|| "nav circulating supply times nav_per_unit would overflow".to_string())?;
    let required = numerator
        .checked_add(unit_scale - 1)
        .ok_or_else(|| "nav collateral requirement would overflow".to_string())?
        / unit_scale;
    if u128::from(verified_net_assets) < required {
        return Err(if nav_per_unit == VAULT_BRIDGE_UNIT {
            "nav verified_net_assets must be >= atom-scaled circulating_supply at VAULT_BRIDGE_UNIT"
                .to_string()
        } else {
            "nav verified_net_assets must be >= circulating_supply * nav_per_unit".to_string()
        });
    }
    Ok(())
}

pub fn nav_reserve_packet_id(
    asset_id: &str,
    epoch: u64,
    reserve_packet_hash: &str,
) -> Result<String, String> {
    validate_lower_hex_len("nav_reserve.asset_id", asset_id, ISSUED_ASSET_ID_HEX_LEN)?;
    if epoch == 0 {
        return Err("nav_reserve.epoch must be nonzero".to_string());
    }
    validate_lower_hex_len(
        "nav_reserve.reserve_packet_hash",
        reserve_packet_hash,
        NAV_RESERVE_PACKET_ID_HEX_LEN,
    )?;
    let preimage =
        format!("asset_id={asset_id}\nepoch={epoch}\nreserve_packet_hash={reserve_packet_hash}\n");
    Ok(hash_hex_domain(
        NAV_RESERVE_PACKET_ID_DOMAIN,
        preimage.as_bytes(),
    ))
}

pub fn nav_redemption_id(
    chain_id: &str,
    owner: &str,
    asset_id: &str,
    owner_sequence: u64,
) -> Result<String, String> {
    validate_chain_id(chain_id)?;
    validate_text_field("nav_redemption.owner", owner)?;
    validate_lower_hex_len("nav_redemption.asset_id", asset_id, ISSUED_ASSET_ID_HEX_LEN)?;
    if owner_sequence == 0 {
        return Err("nav_redemption.owner_sequence must be nonzero".to_string());
    }
    let preimage =
        format!("chain_id={chain_id}\nowner={owner}\nasset_id={asset_id}\nowner_sequence={owner_sequence}\n");
    Ok(hash_hex_domain(
        NAV_REDEMPTION_ID_DOMAIN,
        preimage.as_bytes(),
    ))
}

pub fn vault_bridge_receipt_id(
    chain_id: &str,
    asset_id: &str,
    source_domain: &str,
    source_tx_or_attestation: &str,
    finality_ref: &str,
    amount_atoms: u64,
    policy_hash: &str,
) -> Result<String, String> {
    validate_chain_id(chain_id)?;
    validate_lower_hex_len("vault_bridge_receipt.asset_id", asset_id, ISSUED_ASSET_ID_HEX_LEN)?;
    validate_text_field("vault_bridge_receipt.source_domain", source_domain)?;
    validate_text_field(
        "vault_bridge_receipt.source_tx_or_attestation",
        source_tx_or_attestation,
    )?;
    validate_text_field("vault_bridge_receipt.finality_ref", finality_ref)?;
    if amount_atoms == 0 {
        return Err("vault_bridge_receipt.amount_atoms must be nonzero".to_string());
    }
    validate_vault_bridge_policy_hash("vault_bridge_receipt.policy_hash", policy_hash)?;
    let preimage = format!(
        "chain_id={chain_id}\nasset_id={asset_id}\nsource_domain_bytes={}\nsource_domain={source_domain}\nsource_tx_or_attestation_bytes={}\nsource_tx_or_attestation={source_tx_or_attestation}\nfinality_ref_bytes={}\nfinality_ref={finality_ref}\namount_atoms={amount_atoms}\npolicy_hash={policy_hash}\n",
        source_domain.len(),
        source_tx_or_attestation.len(),
        finality_ref.len(),
    );
    Ok(hash_hex_domain(
        VAULT_BRIDGE_RECEIPT_ID_DOMAIN,
        preimage.as_bytes(),
    ))
}

pub fn vault_bridge_bucket_id(
    asset_id: &str,
    source_domain: &str,
    policy_hash: &str,
) -> Result<String, String> {
    validate_lower_hex_len("vault_bridge_bucket.asset_id", asset_id, ISSUED_ASSET_ID_HEX_LEN)?;
    validate_text_field("vault_bridge_bucket.source_domain", source_domain)?;
    validate_vault_bridge_policy_hash("vault_bridge_bucket.policy_hash", policy_hash)?;
    let preimage = format!(
        "asset_id={asset_id}\nsource_domain_bytes={}\nsource_domain={source_domain}\npolicy_hash={policy_hash}\n",
        source_domain.len(),
    );
    Ok(hash_hex_domain(VAULT_BRIDGE_BUCKET_ID_DOMAIN, preimage.as_bytes()))
}

pub fn vault_bridge_allocation_id(
    chain_id: &str,
    receipt_id: &str,
    asset_id: &str,
    bucket_id: &str,
    amount_atoms: u64,
    purpose: &str,
    consumer_id: &str,
) -> Result<String, String> {
    validate_chain_id(chain_id)?;
    validate_lower_hex_len(
        "vault_bridge_allocation.receipt_id",
        receipt_id,
        VAULT_BRIDGE_RECEIPT_ID_HEX_LEN,
    )?;
    validate_lower_hex_len(
        "vault_bridge_allocation.asset_id",
        asset_id,
        ISSUED_ASSET_ID_HEX_LEN,
    )?;
    validate_lower_hex_len(
        "vault_bridge_allocation.bucket_id",
        bucket_id,
        VAULT_BRIDGE_BUCKET_ID_HEX_LEN,
    )?;
    if amount_atoms == 0 {
        return Err("vault_bridge_allocation.amount_atoms must be nonzero".to_string());
    }
    validate_vault_bridge_allocation_purpose(purpose)?;
    validate_text_field("vault_bridge_allocation.consumer_id", consumer_id)?;
    let preimage = format!(
        "chain_id={chain_id}\nreceipt_id={receipt_id}\nasset_id={asset_id}\nbucket_id={bucket_id}\namount_atoms={amount_atoms}\npurpose={purpose}\nconsumer_id_bytes={}\nconsumer_id={consumer_id}\n",
        consumer_id.len(),
    );
    Ok(hash_hex_domain(
        VAULT_BRIDGE_ALLOCATION_ID_DOMAIN,
        preimage.as_bytes(),
    ))
}

pub fn vault_bridge_redemption_id(
    chain_id: &str,
    owner: &str,
    asset_id: &str,
    owner_sequence: u64,
) -> Result<String, String> {
    validate_chain_id(chain_id)?;
    validate_text_field("vault_bridge_redemption.owner", owner)?;
    validate_lower_hex_len(
        "vault_bridge_redemption.asset_id",
        asset_id,
        ISSUED_ASSET_ID_HEX_LEN,
    )?;
    if owner_sequence == 0 {
        return Err("vault_bridge_redemption.owner_sequence must be nonzero".to_string());
    }
    let preimage = format!(
        "chain_id={chain_id}\nowner={owner}\nasset_id={asset_id}\nowner_sequence={owner_sequence}\n"
    );
    Ok(hash_hex_domain(
        VAULT_BRIDGE_REDEMPTION_ID_DOMAIN,
        preimage.as_bytes(),
    ))
}

pub fn vault_bridge_source_root_for_asset(
    buckets: &[VaultBridgeBucketState],
    asset_id: &str,
) -> Result<String, String> {
    validate_lower_hex_len("vault_bridge_source_root.asset_id", asset_id, ISSUED_ASSET_ID_HEX_LEN)?;
    let mut asset_buckets = buckets
        .iter()
        .filter(|bucket| bucket.asset_id == asset_id)
        .collect::<Vec<_>>();
    asset_buckets.sort_by(|left, right| {
        left.asset_id
            .cmp(&right.asset_id)
            .then(left.bucket_id.cmp(&right.bucket_id))
    });
    let mut preimage = format!("asset_id={asset_id}\nbucket_count={}\n", asset_buckets.len());
    for (index, bucket) in asset_buckets.iter().enumerate() {
        bucket.validate()?;
        preimage.push_str(&format!(
            "bucket[{index}].asset_id={}\nbucket[{index}].bucket_id={}\nbucket[{index}].source_domain_bytes={}\nbucket[{index}].source_domain={}\nbucket[{index}].policy_hash={}\nbucket[{index}].gross_receipt_atoms={}\nbucket[{index}].counted_value_atoms={}\nbucket[{index}].outstanding_vault_bridge_atoms={}\nbucket[{index}].nav_subscription_allocations_atoms={}\nbucket[{index}].redemption_queue_atoms={}\nbucket[{index}].other_allocations_atoms={}\nbucket[{index}].impairment_factor_bps={}\nbucket[{index}].status={}\nbucket[{index}].last_updated_height={}\n",
            bucket.asset_id,
            bucket.bucket_id,
            bucket.source_domain.len(),
            bucket.source_domain,
            bucket.policy_hash,
            bucket.gross_receipt_atoms,
            bucket.counted_value_atoms,
            bucket.outstanding_vault_bridge_atoms,
            bucket.nav_subscription_allocations_atoms,
            bucket.redemption_queue_atoms,
            bucket.other_allocations_atoms,
            bucket.impairment_factor_bps,
            bucket.status,
            bucket.last_updated_height,
        ));
    }
    Ok(hash_hex_domain(
        VAULT_BRIDGE_SOURCE_ROOT_DOMAIN,
        preimage.as_bytes(),
    ))
}

pub fn vault_bridge_counted_value_for_asset(
    buckets: &[VaultBridgeBucketState],
    asset_id: &str,
) -> Result<u64, String> {
    buckets
        .iter()
        .filter(|bucket| bucket.asset_id == asset_id)
        .filter(|bucket| {
            bucket.status == VAULT_BRIDGE_BUCKET_STATUS_ACTIVE
                || bucket.status == VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED
        })
        .try_fold(0_u64, |total, bucket| {
            bucket.validate()?;
            total
                .checked_add(bucket.counted_value_atoms)
                .ok_or_else(|| "vault_bridge counted value overflow".to_string())
        })
}

fn validate_nav_reserve_state(state: &str) -> Result<(), String> {
    validate_text_field("nav_reserve.state", state)?;
    match state {
        NAV_RESERVE_STATE_SUBMITTED
        | NAV_RESERVE_STATE_FINALIZED
        | NAV_RESERVE_STATE_CHALLENGED => Ok(()),
        _ => Err("nav_reserve.state must be submitted, finalized, or challenged".to_string()),
    }
}

fn validate_nav_redemption_state(state: &str) -> Result<(), String> {
    validate_text_field("nav_redemption.state", state)?;
    match state {
        NAV_REDEMPTION_STATE_PENDING | NAV_REDEMPTION_STATE_SETTLED => Ok(()),
        _ => Err("nav_redemption.state must be pending or settled".to_string()),
    }
}

fn validate_vault_bridge_receipt_status(status: &str) -> Result<(), String> {
    validate_text_field("vault_bridge_receipt.status", status)?;
    match status {
        VAULT_BRIDGE_RECEIPT_STATUS_PENDING
        | VAULT_BRIDGE_RECEIPT_STATUS_FINALIZED
        | VAULT_BRIDGE_RECEIPT_STATUS_COUNTED
        | VAULT_BRIDGE_RECEIPT_STATUS_PAUSED
        | VAULT_BRIDGE_RECEIPT_STATUS_IMPAIRED
        | VAULT_BRIDGE_RECEIPT_STATUS_REJECTED
        | VAULT_BRIDGE_RECEIPT_STATUS_RETIRED => Ok(()),
        _ => Err(
            "vault_bridge_receipt.status must be pending, finalized, counted, paused, impaired, rejected, or retired"
                .to_string(),
        ),
    }
}

fn validate_vault_bridge_deposit_status(status: &str) -> Result<(), String> {
    validate_text_field("vault_bridge_deposit.status", status)?;
    match status {
        VAULT_BRIDGE_DEPOSIT_STATUS_PENDING
        | VAULT_BRIDGE_DEPOSIT_STATUS_CHALLENGED
        | VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED => Ok(()),
        _ => Err(
            "vault_bridge_deposit.status must be pending, challenged, or finalized".to_string(),
        ),
    }
}

pub fn validate_vault_bridge_policy_hash(field: &str, value: &str) -> Result<(), String> {
    if value.len() != NAV_PROFILE_ID_HEX_LEN && value.len() != NAV_SP1_POLICY_HASH_HEX_LEN {
        return Err(format!(
            "{field} must be either {NAV_PROFILE_ID_HEX_LEN} hex chars or {NAV_SP1_POLICY_HASH_HEX_LEN} hex chars"
        ));
    }
    validate_lower_hex_len(field, value, value.len())
}

pub fn validate_vault_bridge_deposit_source_proof_fields(
    prefix: &str,
    source_proof_kind: &str,
    source_proof_hash: &str,
    source_public_values_hash: &str,
) -> Result<(), String> {
    if source_proof_kind.is_empty() {
        if !source_proof_hash.is_empty() || !source_public_values_hash.is_empty() {
            return Err(format!(
                "{prefix}.source proof hashes require source_proof_kind"
            ));
        }
        return Ok(());
    }
    validate_text_field(&format!("{prefix}.source_proof_kind"), source_proof_kind)?;
    if !matches!(
        source_proof_kind,
        NAV_PROFILE_VERIFIER_SP1_GROTH16 | NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1
    ) {
        return Err(format!(
            "{prefix}.source_proof_kind must be {NAV_PROFILE_VERIFIER_SP1_GROTH16} or {NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1}"
        ));
    }
    validate_lower_hex_len(
        &format!("{prefix}.source_proof_hash"),
        source_proof_hash,
        VAULT_BRIDGE_HEX_HASH_LEN,
    )?;
    validate_lower_hex_len(
        &format!("{prefix}.source_public_values_hash"),
        source_public_values_hash,
        VAULT_BRIDGE_HEX_HASH_LEN,
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn validate_vault_bridge_receipt_bridge_deposit_fields(
    claim_type: &str,
    source_domain: &str,
    source_asset: &str,
    amount_atoms: u64,
    source_tx_or_attestation: &str,
    finality_ref: &str,
    vault_id: &str,
    evidence: Option<&VaultBridgeDepositEvidence>,
) -> Result<(), String> {
    if claim_type != VAULT_BRIDGE_CLAIM_TYPE_BRIDGE_DEPOSIT {
        if evidence.is_some() {
            return Err(
                "vault_bridge_receipt.bridge_deposit_evidence is only valid for bridge_deposit claims"
                    .to_string(),
            );
        }
        return Ok(());
    }

    let evidence = evidence.ok_or_else(|| {
        "vault_bridge_receipt bridge_deposit claim requires bridge_deposit_evidence".to_string()
    })?;
    evidence.validate()?;
    if source_asset != evidence.source_asset_ref() {
        return Err(
            "vault_bridge_receipt bridge_deposit source_asset must match evidence token"
                .to_string(),
        );
    }
    if source_domain != evidence.source_domain() {
        return Err("vault_bridge_receipt bridge_deposit source_domain mismatch".to_string());
    }
    if amount_atoms != evidence.amount_atoms {
        return Err("vault_bridge_receipt bridge_deposit amount mismatch".to_string());
    }
    if source_tx_or_attestation != evidence.source_tx_or_attestation() {
        return Err("vault_bridge_receipt bridge_deposit source_tx_or_attestation mismatch".to_string());
    }
    if finality_ref != evidence.finality_ref() {
        return Err("vault_bridge_receipt bridge_deposit finality_ref mismatch".to_string());
    }
    if vault_id != evidence.vault_id() {
        return Err("vault_bridge_receipt bridge_deposit vault_id mismatch".to_string());
    }
    Ok(())
}

fn validate_evm_address_text(field: &str, value: &str) -> Result<(), String> {
    if value.len() != VAULT_BRIDGE_EVM_ADDRESS_TEXT_LEN || !value.starts_with("0x") {
        return Err(format!("{field} must be a 0x-prefixed lowercase EVM address"));
    }
    if !value[2..]
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(format!("{field} must be a 0x-prefixed lowercase EVM address"));
    }
    if value[2..].bytes().all(|byte| byte == b'0') {
        return Err(format!("{field} must not be the zero EVM address"));
    }
    Ok(())
}

fn decode_evm_address_20(field: &str, value: &str) -> Result<[u8; 20], String> {
    validate_evm_address_text(field, value)?;
    let bytes = decode_lower_hex_exact(field, &value[2..], 20)?;
    let mut output = [0u8; 20];
    output.copy_from_slice(&bytes);
    Ok(output)
}

fn decode_lower_hex_exact(field: &str, value: &str, bytes_len: usize) -> Result<Vec<u8>, String> {
    let expected_len = bytes_len
        .checked_mul(2)
        .ok_or_else(|| format!("{field} expected length overflow"))?;
    validate_lower_hex_len(field, value, expected_len)?;
    let mut out = Vec::with_capacity(bytes_len);
    let bytes = value.as_bytes();
    for index in (0..bytes.len()).step_by(2) {
        let high = hex_nibble(bytes[index]).ok_or_else(|| format!("{field} has invalid hex"))?;
        let low =
            hex_nibble(bytes[index + 1]).ok_or_else(|| format!("{field} has invalid hex"))?;
        out.push((high << 4) | low);
    }
    Ok(out)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}

fn append_abi_u256_u64(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&[0u8; 24]);
    out.extend_from_slice(&value.to_be_bytes());
}

fn append_abi_u256_usize(out: &mut Vec<u8>, value: usize) -> Result<(), String> {
    let value = u64::try_from(value)
        .map_err(|_| "abi usize value does not fit into uint64 helper".to_string())?;
    append_abi_u256_u64(out, value);
    Ok(())
}

fn append_abi_address(out: &mut Vec<u8>, value: &[u8; 20]) {
    out.extend_from_slice(&[0u8; 12]);
    out.extend_from_slice(value);
}

fn append_abi_bytes32(out: &mut Vec<u8>, value: &[u8]) -> Result<(), String> {
    if value.len() != 32 {
        return Err("abi bytes32 value must be exactly 32 bytes".to_string());
    }
    out.extend_from_slice(value);
    Ok(())
}

fn append_abi_dynamic_bytes(
    head: &mut Vec<u8>,
    tail: &mut Vec<u8>,
    head_len: usize,
    value: &[u8],
) -> Result<(), String> {
    let offset = head_len
        .checked_add(tail.len())
        .ok_or_else(|| "abi dynamic offset overflow".to_string())?;
    append_abi_u256_usize(head, offset)?;
    append_abi_u256_usize(tail, value.len())?;
    tail.extend_from_slice(value);
    let padding = (32 - (value.len() % 32)) % 32;
    tail.extend(std::iter::repeat_n(0u8, padding));
    Ok(())
}

fn append_abi_string_tail(out: &mut Vec<u8>, value: &str) {
    append_abi_u256_u64(out, value.len() as u64);
    out.extend_from_slice(value.as_bytes());
    let padding = (32 - (value.len() % 32)) % 32;
    out.extend(std::iter::repeat_n(0u8, padding));
}

pub fn pftl_uniswap_return_burn_id_from_fields(
    ethereum_chain_id: u64,
    bridge_controller: &str,
    wrapped_navcoin_token: &str,
    native_nav_asset_id: &str,
    ethereum_sender: &str,
    pftl_recipient: &str,
    amount_atoms: u64,
    return_nonce: &str,
    burn_height: u64,
) -> Result<String, String> {
    if ethereum_chain_id == 0 {
        return Err("pftl_uniswap_return_burn.ethereum_chain_id must be nonzero".to_string());
    }
    validate_evm_address_text("pftl_uniswap_return_burn.bridge_controller", bridge_controller)?;
    validate_evm_address_text(
        "pftl_uniswap_return_burn.wrapped_navcoin_token",
        wrapped_navcoin_token,
    )?;
    validate_lower_hex_len(
        "pftl_uniswap_return_burn.native_nav_asset_id",
        native_nav_asset_id,
        ISSUED_ASSET_ID_HEX_LEN,
    )?;
    validate_evm_address_text("pftl_uniswap_return_burn.ethereum_sender", ethereum_sender)?;
    validate_text_field("pftl_uniswap_return_burn.pftl_recipient", pftl_recipient)?;
    validate_lower_hex_len("pftl_uniswap_return_burn.return_nonce", return_nonce, 64)?;
    if amount_atoms == 0 || burn_height == 0 {
        return Err("pftl_uniswap_return_burn amount and height must be nonzero".to_string());
    }

    let bridge_controller =
        decode_evm_address_20("pftl_uniswap_return_burn.bridge_controller", bridge_controller)?;
    let wrapped_navcoin_token = decode_evm_address_20(
        "pftl_uniswap_return_burn.wrapped_navcoin_token",
        wrapped_navcoin_token,
    )?;
    let native_nav_asset_id = decode_lower_hex_exact(
        "pftl_uniswap_return_burn.native_nav_asset_id",
        native_nav_asset_id,
        ISSUED_ASSET_ID_HEX_LEN / 2,
    )?;
    let ethereum_sender =
        decode_evm_address_20("pftl_uniswap_return_burn.ethereum_sender", ethereum_sender)?;
    let return_nonce =
        decode_lower_hex_exact("pftl_uniswap_return_burn.return_nonce", return_nonce, 32)?;
    let head_len = 32usize
        .checked_mul(10)
        .ok_or_else(|| "return burn ABI head overflow".to_string())?;
    let mut head = Vec::with_capacity(head_len);
    let mut tail = Vec::new();
    append_abi_dynamic_bytes(
        &mut head,
        &mut tail,
        head_len,
        b"postfiat.pftl_uniswap.return_burn.v1",
    )?;
    append_abi_u256_u64(&mut head, ethereum_chain_id);
    append_abi_address(&mut head, &bridge_controller);
    append_abi_address(&mut head, &wrapped_navcoin_token);
    append_abi_dynamic_bytes(&mut head, &mut tail, head_len, &native_nav_asset_id)?;
    append_abi_address(&mut head, &ethereum_sender);
    append_abi_dynamic_bytes(&mut head, &mut tail, head_len, pftl_recipient.as_bytes())?;
    append_abi_u256_u64(&mut head, amount_atoms);
    append_abi_bytes32(&mut head, &return_nonce)?;
    append_abi_u256_u64(&mut head, burn_height);

    let mut abi = head;
    abi.extend_from_slice(&tail);
    let mut hasher = Keccak256::new();
    hasher.update(&abi);
    let digest = hasher.finalize();
    Ok(bytes_to_lower_hex(&digest))
}

pub fn pftl_uniswap_non_consumption_proof_hash(
    route_id: &str,
    packet_hash: &str,
    refund_not_before_height: u64,
) -> Result<String, String> {
    validate_text_field("pftl_uniswap_non_consumption.route_id", route_id)?;
    validate_lower_hex_len(
        "pftl_uniswap_non_consumption.packet_hash",
        packet_hash,
        VAULT_BRIDGE_HEX_HASH_LEN,
    )?;
    if refund_not_before_height == 0 {
        return Err("pftl_uniswap_non_consumption.refund_not_before_height must be nonzero".to_string());
    }
    let preimage = format!(
        "route_id={route_id}\npacket_hash={packet_hash}\nrefund_not_before_height={refund_not_before_height}\n"
    );
    Ok(hash_hex_domain(
        "postfiat.pftl_uniswap.non_consumption_commitment.v1",
        preimage.as_bytes(),
    ))
}

fn vault_bridge_evm_recipient_from_destination_ref(destination_ref: &str) -> Result<String, String> {
    validate_text_field("vault_bridge_redemption.destination_ref", destination_ref)?;
    let parts = destination_ref.split(':').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0] != VAULT_BRIDGE_EVM_DESTINATION_REF_PREFIX {
        return Err(format!(
            "vault_bridge_redemption.destination_ref must be `{VAULT_BRIDGE_EVM_DESTINATION_REF_PREFIX}:<evm_chain_id>:<0xrecipient>`"
        ));
    }
    let evm_chain_id = parts[1];
    if evm_chain_id.is_empty() || !evm_chain_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("vault_bridge_redemption.destination_ref EVM chain id must be decimal".to_string());
    }
    let recipient = parts[2].to_string();
    validate_evm_address_text("vault_bridge_redemption.destination_ref recipient", &recipient)?;
    Ok(recipient)
}

fn vault_bridge_evm_chain_id_from_destination_ref(destination_ref: &str) -> Result<u64, String> {
    validate_text_field("vault_bridge_redemption.destination_ref", destination_ref)?;
    let parts = destination_ref.split(':').collect::<Vec<_>>();
    if parts.len() != 3 || parts[0] != VAULT_BRIDGE_EVM_DESTINATION_REF_PREFIX {
        return Err(format!(
            "vault_bridge_redemption.destination_ref must be `{VAULT_BRIDGE_EVM_DESTINATION_REF_PREFIX}:<evm_chain_id>:<0xrecipient>`"
        ));
    }
    let evm_chain_id = parts[1];
    if evm_chain_id.is_empty() || !evm_chain_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("vault_bridge_redemption.destination_ref EVM chain id must be decimal".to_string());
    }
    let parsed = evm_chain_id
        .parse::<u64>()
        .map_err(|_| "vault_bridge_redemption.destination_ref EVM chain id must fit u64".to_string())?;
    if parsed == 0 {
        return Err("vault_bridge_redemption.destination_ref EVM chain id must be nonzero".to_string());
    }
    Ok(parsed)
}

fn vault_bridge_evm_source_domain_parts(source_domain: &str) -> Result<(u64, String, String), String> {
    validate_text_field("vault_bridge_redemption.source_domain", source_domain)?;
    let parts = source_domain.split(':').collect::<Vec<_>>();
    if parts.len() != 4 || parts[0] != VAULT_BRIDGE_DEPOSIT_SOURCE_DOMAIN_PREFIX {
        return Err(format!(
            "vault_bridge_redemption.source_domain must be `{VAULT_BRIDGE_DEPOSIT_SOURCE_DOMAIN_PREFIX}:<evm_chain_id>:<vault>:<token>`"
        ));
    }
    let source_chain_id = parts[1];
    if source_chain_id.is_empty() || !source_chain_id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err("vault_bridge_redemption.source_domain EVM chain id must be decimal".to_string());
    }
    let parsed = source_chain_id
        .parse::<u64>()
        .map_err(|_| "vault_bridge_redemption.source_domain EVM chain id must fit u64".to_string())?;
    if parsed == 0 {
        return Err("vault_bridge_redemption.source_domain EVM chain id must be nonzero".to_string());
    }
    let vault_address = parts[2].to_string();
    let token_address = parts[3].to_string();
    validate_evm_address_text("vault_bridge_redemption.source_domain vault", &vault_address)?;
    validate_evm_address_text("vault_bridge_redemption.source_domain token", &token_address)?;
    Ok((parsed, vault_address, token_address))
}

fn validate_vault_bridge_bucket_status(status: &str) -> Result<(), String> {
    validate_text_field("vault_bridge_bucket.status", status)?;
    match status {
        VAULT_BRIDGE_BUCKET_STATUS_ACTIVE
        | VAULT_BRIDGE_BUCKET_STATUS_PAUSED
        | VAULT_BRIDGE_BUCKET_STATUS_IMPAIRED
        | VAULT_BRIDGE_BUCKET_STATUS_RETIRED => Ok(()),
        _ => Err("vault_bridge_bucket.status must be active, paused, impaired, or retired".to_string()),
    }
}

fn validate_vault_bridge_allocation_purpose(purpose: &str) -> Result<(), String> {
    validate_text_field("vault_bridge_allocation.purpose", purpose)?;
    match purpose {
        VAULT_BRIDGE_ALLOCATION_PURPOSE_SUPPLY
        | VAULT_BRIDGE_ALLOCATION_PURPOSE_NAV_SUBSCRIPTION
        | VAULT_BRIDGE_ALLOCATION_PURPOSE_REDEMPTION
        | VAULT_BRIDGE_ALLOCATION_PURPOSE_OTHER => Ok(()),
        _ => Err(
            "vault_bridge_allocation.purpose must be vault_bridge_supply, nav_subscription, redemption, or other"
                .to_string(),
        ),
    }
}

fn validate_vault_bridge_redemption_state(state: &str) -> Result<(), String> {
    validate_text_field("vault_bridge_redemption.state", state)?;
    match state {
        VAULT_BRIDGE_REDEMPTION_STATE_PENDING | VAULT_BRIDGE_REDEMPTION_STATE_SETTLED => Ok(()),
        _ => Err("vault_bridge_redemption.state must be pending or settled".to_string()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NftDefinition {
    pub nft_id: String,
    pub issuer: String,
    pub collection_id: String,
    pub serial: u64,
    pub owner: String,
    pub metadata_hash: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub metadata_uri: String,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub flags: u32,
    #[serde(default, skip_serializing_if = "is_zero_u32")]
    pub collection_flags: u32,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub issuer_transfer_fee: u64,
    #[serde(default, skip_serializing_if = "is_false")]
    pub burned: bool,
}

impl NftDefinition {
    pub fn new(
        chain_id: &str,
        issuer: impl Into<String>,
        collection_id: impl Into<String>,
        serial: u64,
        owner: impl Into<String>,
        metadata_hash: impl Into<String>,
    ) -> Result<Self, String> {
        let issuer = issuer.into();
        let collection_id = collection_id.into();
        let nft_id = nft_id(chain_id, &issuer, &collection_id, serial)?;
        let nft = Self {
            nft_id,
            issuer,
            collection_id,
            serial,
            owner: owner.into(),
            metadata_hash: metadata_hash.into(),
            metadata_uri: String::new(),
            flags: 0,
            collection_flags: 0,
            issuer_transfer_fee: 0,
            burned: false,
        };
        nft.validate_for_chain(chain_id)?;
        Ok(nft)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("nft.nft_id", &self.nft_id, NFT_ID_HEX_LEN)?;
        validate_text_field("nft.issuer", &self.issuer)?;
        validate_nft_collection_id(&self.collection_id)?;
        if self.serial == 0 {
            return Err("nft.serial must be nonzero".to_string());
        }
        validate_text_field("nft.owner", &self.owner)?;
        validate_lower_hex_max(
            "nft.metadata_hash",
            &self.metadata_hash,
            MAX_NFT_METADATA_HASH_BYTES * 2,
        )?;
        validate_optional_text_field(
            "nft.metadata_uri",
            &self.metadata_uri,
            MAX_NFT_METADATA_URI_BYTES,
        )?;
        if self.flags & !NFT_ALLOWED_FLAGS != 0 {
            return Err("nft.flags contains unsupported bits".to_string());
        }
        if self.collection_flags & !NFT_COLLECTION_ALLOWED_FLAGS != 0 {
            return Err("nft.collection_flags contains unsupported bits".to_string());
        }
        Ok(())
    }

    pub fn validate_for_chain(&self, chain_id: &str) -> Result<(), String> {
        self.validate()?;
        let expected_nft_id = nft_id(chain_id, &self.issuer, &self.collection_id, self.serial)?;
        if self.nft_id != expected_nft_id {
            return Err(
                "nft.nft_id does not match chain, issuer, collection, and serial".to_string(),
            );
        }
        Ok(())
    }
}

pub fn nft_id(
    chain_id: &str,
    issuer: &str,
    collection_id: &str,
    serial: u64,
) -> Result<String, String> {
    validate_chain_id(chain_id)?;
    validate_text_field("nft.issuer", issuer)?;
    validate_nft_collection_id(collection_id)?;
    if serial == 0 {
        return Err("nft.serial must be nonzero".to_string());
    }
    let preimage = format!(
        "chain_id={chain_id}\nissuer={issuer}\ncollection_id_bytes={}\ncollection_id={collection_id}\nserial={serial}\n",
        collection_id.len()
    );
    Ok(hash_hex_domain(NFT_ID_DOMAIN, preimage.as_bytes()))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Escrow {
    pub escrow_id: String,
    pub owner: String,
    pub owner_sequence: u64,
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
    pub fee: u64,
    pub condition: String,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub finish_after: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub cancel_after: u64,
    pub state: String,
    pub created_height: u64,
}

impl Escrow {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        chain_id: &str,
        owner: impl Into<String>,
        owner_sequence: u64,
        recipient: impl Into<String>,
        asset_id: impl Into<String>,
        amount: u64,
        fee: u64,
        condition: impl Into<String>,
        finish_after: u64,
        cancel_after: u64,
        created_height: u64,
    ) -> Result<Self, String> {
        let owner = owner.into();
        let recipient = recipient.into();
        let escrow_id = escrow_id(chain_id, &owner, owner_sequence)?;
        let escrow = Self {
            escrow_id,
            owner,
            owner_sequence,
            recipient,
            asset_id: asset_id.into(),
            amount,
            fee,
            condition: condition.into(),
            finish_after,
            cancel_after,
            state: ESCROW_STATE_OPEN.to_string(),
            created_height,
        };
        escrow.validate_for_chain(chain_id)?;
        Ok(escrow)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("escrow.escrow_id", &self.escrow_id, ESCROW_ID_HEX_LEN)?;
        validate_text_field("escrow.owner", &self.owner)?;
        validate_text_field("escrow.recipient", &self.recipient)?;
        if self.owner == self.recipient {
            return Err("escrow.owner must differ from escrow.recipient".to_string());
        }
        if self.owner_sequence == 0 {
            return Err("escrow.owner_sequence must be nonzero".to_string());
        }
        validate_escrow_asset_id(&self.asset_id)?;
        if self.amount == 0 {
            return Err("escrow.amount must be nonzero".to_string());
        }
        validate_optional_text_field(
            "escrow.condition",
            &self.condition,
            MAX_ESCROW_CONDITION_BYTES,
        )?;
        if self.condition.is_empty() && self.finish_after == 0 && self.cancel_after == 0 {
            return Err("escrow must declare condition, finish_after, or cancel_after".to_string());
        }
        if self.finish_after != 0
            && self.cancel_after != 0
            && self.cancel_after <= self.finish_after
        {
            return Err("escrow.cancel_after must be greater than finish_after".to_string());
        }
        validate_escrow_state_value(&self.state)?;
        Ok(())
    }

    pub fn validate_for_chain(&self, chain_id: &str) -> Result<(), String> {
        self.validate()?;
        let expected_escrow_id = escrow_id(chain_id, &self.owner, self.owner_sequence)?;
        if self.escrow_id != expected_escrow_id {
            return Err("escrow.escrow_id does not match chain, owner, and sequence".to_string());
        }
        Ok(())
    }
}

pub fn escrow_id(chain_id: &str, owner: &str, owner_sequence: u64) -> Result<String, String> {
    validate_chain_id(chain_id)?;
    validate_text_field("escrow.owner", owner)?;
    if owner_sequence == 0 {
        return Err("escrow.owner_sequence must be nonzero".to_string());
    }
    let preimage = format!("chain_id={chain_id}\nowner={owner}\nowner_sequence={owner_sequence}\n");
    Ok(hash_hex_domain(ESCROW_ID_DOMAIN, preimage.as_bytes()))
}

pub fn escrow_condition_hash(condition: &str) -> Result<String, String> {
    validate_optional_text_field("escrow.condition", condition, MAX_ESCROW_CONDITION_BYTES)?;
    Ok(hash_hex_domain(
        ESCROW_CONDITION_HASH_DOMAIN,
        condition.as_bytes(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Offer {
    pub offer_id: String,
    pub owner: String,
    pub owner_sequence: u64,
    pub taker_gets_asset_id: String,
    pub taker_gets_amount_remaining: u64,
    pub taker_pays_asset_id: String,
    pub taker_pays_amount_remaining: u64,
    pub original_taker_gets_amount: u64,
    pub original_taker_pays_amount: u64,
    pub created_height: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub expiration_height: u64,
    #[serde(default, skip_serializing_if = "is_zero_u64")]
    pub reserve_paid: u64,
    pub state: String,
}

impl Offer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        chain_id: &str,
        owner: impl Into<String>,
        owner_sequence: u64,
        taker_gets_asset_id: impl Into<String>,
        taker_gets_amount: u64,
        taker_pays_asset_id: impl Into<String>,
        taker_pays_amount: u64,
        created_height: u64,
        expiration_height: u64,
    ) -> Result<Self, String> {
        let owner = owner.into();
        let offer_id = offer_id(chain_id, &owner, owner_sequence)?;
        let offer = Self {
            offer_id,
            owner,
            owner_sequence,
            taker_gets_asset_id: taker_gets_asset_id.into(),
            taker_gets_amount_remaining: taker_gets_amount,
            taker_pays_asset_id: taker_pays_asset_id.into(),
            taker_pays_amount_remaining: taker_pays_amount,
            original_taker_gets_amount: taker_gets_amount,
            original_taker_pays_amount: taker_pays_amount,
            created_height,
            expiration_height,
            reserve_paid: OFFER_OBJECT_RESERVE,
            state: OFFER_STATE_OPEN.to_string(),
        };
        offer.validate_for_chain(chain_id)?;
        Ok(offer)
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("offer.offer_id", &self.offer_id, OFFER_ID_HEX_LEN)?;
        validate_text_field("offer.owner", &self.owner)?;
        if self.owner_sequence == 0 {
            return Err("offer.owner_sequence must be nonzero".to_string());
        }
        validate_dex_asset_id("offer.taker_gets_asset_id", &self.taker_gets_asset_id)?;
        validate_dex_asset_id("offer.taker_pays_asset_id", &self.taker_pays_asset_id)?;
        if self.taker_gets_asset_id == self.taker_pays_asset_id {
            return Err("offer assets must differ".to_string());
        }
        if self.original_taker_gets_amount == 0 {
            return Err("offer.original_taker_gets_amount must be nonzero".to_string());
        }
        if self.original_taker_pays_amount == 0 {
            return Err("offer.original_taker_pays_amount must be nonzero".to_string());
        }
        if self.taker_gets_amount_remaining > self.original_taker_gets_amount {
            return Err(
                "offer.taker_gets_amount_remaining must not exceed original amount".to_string(),
            );
        }
        if self.taker_pays_amount_remaining > self.original_taker_pays_amount {
            return Err(
                "offer.taker_pays_amount_remaining must not exceed original amount".to_string(),
            );
        }
        if self.created_height == 0 {
            return Err("offer.created_height must be nonzero".to_string());
        }
        if self.expiration_height != 0 && self.expiration_height <= self.created_height {
            return Err("offer.expiration_height must be greater than created_height".to_string());
        }
        validate_offer_state_value(&self.state)?;
        match self.state.as_str() {
            OFFER_STATE_OPEN => {
                if self.taker_gets_amount_remaining == 0 || self.taker_pays_amount_remaining == 0 {
                    return Err("open offer remaining amounts must be nonzero".to_string());
                }
                if self.reserve_paid == 0 {
                    return Err("open offer reserve_paid must be nonzero".to_string());
                }
            }
            OFFER_STATE_FILLED
                if self.taker_gets_amount_remaining != 0
                    || self.taker_pays_amount_remaining != 0 =>
            {
                return Err("filled offer remaining amounts must be zero".to_string());
            }
            _ => {}
        }
        Ok(())
    }

    pub fn validate_for_chain(&self, chain_id: &str) -> Result<(), String> {
        self.validate()?;
        let expected_offer_id = offer_id(chain_id, &self.owner, self.owner_sequence)?;
        if self.offer_id != expected_offer_id {
            return Err("offer.offer_id does not match chain, owner, and sequence".to_string());
        }
        Ok(())
    }

    pub fn is_open(&self) -> bool {
        self.state == OFFER_STATE_OPEN
    }
}

pub fn offer_id(chain_id: &str, owner: &str, owner_sequence: u64) -> Result<String, String> {
    validate_chain_id(chain_id)?;
    validate_text_field("offer.owner", owner)?;
    if owner_sequence == 0 {
        return Err("offer.owner_sequence must be nonzero".to_string());
    }
    let preimage = format!("chain_id={chain_id}\nowner={owner}\nowner_sequence={owner_sequence}\n");
    Ok(hash_hex_domain(OFFER_ID_DOMAIN, preimage.as_bytes()))
}

pub fn offer_book_key(
    taker_gets_asset_id: &str,
    taker_pays_asset_id: &str,
) -> Result<String, String> {
    validate_dex_asset_id("dex.taker_gets_asset_id", taker_gets_asset_id)?;
    validate_dex_asset_id("dex.taker_pays_asset_id", taker_pays_asset_id)?;
    if taker_gets_asset_id == taker_pays_asset_id {
        return Err("dex offer book assets must differ".to_string());
    }
    Ok(format!("{taker_gets_asset_id}->{taker_pays_asset_id}"))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSettlementTemplateLeg {
    pub owner: String,
    pub recipient: String,
    pub asset_id: String,
    pub amount: u64,
    pub owner_sequence: u64,
}

impl AtomicSettlementTemplateLeg {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("atomic_settlement.leg.owner", &self.owner)?;
        validate_text_field("atomic_settlement.leg.recipient", &self.recipient)?;
        if self.owner == self.recipient {
            return Err("atomic_settlement leg owner must differ from recipient".to_string());
        }
        validate_escrow_asset_id(&self.asset_id)?;
        if self.amount == 0 {
            return Err("atomic_settlement leg amount must be nonzero".to_string());
        }
        if self.owner_sequence == 0 {
            return Err("atomic_settlement leg owner_sequence must be nonzero".to_string());
        }
        Ok(())
    }

    fn canonical_preimage(&self) -> String {
        format!(
            "owner={}\nrecipient={}\nasset_id={}\namount={}\nowner_sequence={}\n",
            self.owner, self.recipient, self.asset_id, self.amount, self.owner_sequence
        )
    }

    pub fn escrow_create_operation(
        &self,
        template: &AtomicSettlementTemplate,
    ) -> EscrowCreateOperation {
        EscrowCreateOperation {
            owner: self.owner.clone(),
            recipient: self.recipient.clone(),
            asset_id: self.asset_id.clone(),
            amount: self.amount,
            condition: template.condition.clone(),
            finish_after: template.finish_after,
            cancel_after: template.cancel_after,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSettlementTemplate {
    pub left: AtomicSettlementTemplateLeg,
    pub right: AtomicSettlementTemplateLeg,
    pub condition: String,
    pub finish_after: u64,
    pub cancel_after: u64,
}

impl AtomicSettlementTemplate {
    pub fn validate(&self) -> Result<(), String> {
        self.left.validate()?;
        self.right.validate()?;
        validate_optional_text_field(
            "atomic_settlement.condition",
            &self.condition,
            MAX_ESCROW_CONDITION_BYTES,
        )?;
        if self.condition.is_empty() {
            return Err("atomic_settlement.condition must be nonempty".to_string());
        }
        if self.cancel_after == 0 {
            return Err("atomic_settlement.cancel_after must be nonzero".to_string());
        }
        if self.finish_after != 0 && self.cancel_after <= self.finish_after {
            return Err(
                "atomic_settlement.cancel_after must be greater than finish_after".to_string(),
            );
        }
        if self.left.owner != self.right.recipient || self.right.owner != self.left.recipient {
            return Err("atomic_settlement legs must be reciprocal".to_string());
        }
        let left_is_pft = self.left.asset_id == "PFT";
        let right_is_pft = self.right.asset_id == "PFT";
        if left_is_pft == right_is_pft {
            return Err(
                "atomic_settlement template must contain exactly one PFT leg and one issued-asset leg"
                    .to_string(),
            );
        }
        self.left
            .escrow_create_operation(self)
            .validate()
            .map_err(|error| format!("left escrow create invalid: {error}"))?;
        self.right
            .escrow_create_operation(self)
            .validate()
            .map_err(|error| format!("right escrow create invalid: {error}"))?;
        Ok(())
    }

    pub fn escrow_create_operations(
        &self,
    ) -> Result<(EscrowCreateOperation, EscrowCreateOperation), String> {
        self.validate()?;
        Ok((
            self.left.escrow_create_operation(self),
            self.right.escrow_create_operation(self),
        ))
    }
}

pub fn atomic_settlement_template_id(
    chain_id: &str,
    template: &AtomicSettlementTemplate,
) -> Result<String, String> {
    validate_chain_id(chain_id)?;
    template.validate()?;
    let condition_hash = escrow_condition_hash(&template.condition)?;
    let mut leg_preimages = [
        template.left.canonical_preimage(),
        template.right.canonical_preimage(),
    ];
    leg_preimages.sort();
    let preimage = format!(
        "chain_id={chain_id}\ncondition_hash={condition_hash}\ncondition_bytes={}\ncondition={}\nfinish_after={}\ncancel_after={}\nleg_count=2\nleg[0]_bytes={}\n{}leg[1]_bytes={}\n{}",
        template.condition.len(),
        template.condition,
        template.finish_after,
        template.cancel_after,
        leg_preimages[0].len(),
        leg_preimages[0],
        leg_preimages[1].len(),
        leg_preimages[1]
    );
    Ok(hash_hex_domain(
        ATOMIC_SETTLEMENT_TEMPLATE_ID_DOMAIN,
        preimage.as_bytes(),
    ))
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct EscrowIndexes {
    pub by_owner: BTreeMap<String, Vec<String>>,
    pub by_recipient: BTreeMap<String, Vec<String>>,
    pub by_condition_hash: BTreeMap<String, Vec<String>>,
    pub by_expiry_height: BTreeMap<u64, Vec<String>>,
}

impl EscrowIndexes {
    fn sort_values(&mut self) {
        for escrow_ids in self.by_owner.values_mut() {
            escrow_ids.sort();
        }
        for escrow_ids in self.by_recipient.values_mut() {
            escrow_ids.sort();
        }
        for escrow_ids in self.by_condition_hash.values_mut() {
            escrow_ids.sort();
        }
        for escrow_ids in self.by_expiry_height.values_mut() {
            escrow_ids.sort();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct NftIndexes {
    pub by_owner: BTreeMap<String, Vec<String>>,
    pub by_issuer: BTreeMap<String, Vec<String>>,
    pub by_collection: BTreeMap<String, Vec<String>>,
}

impl NftIndexes {
    fn sort_values(&mut self) {
        for nft_ids in self.by_owner.values_mut() {
            nft_ids.sort();
        }
        for nft_ids in self.by_issuer.values_mut() {
            nft_ids.sort();
        }
        for nft_ids in self.by_collection.values_mut() {
            nft_ids.sort();
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct OfferIndexes {
    pub by_owner: BTreeMap<String, Vec<String>>,
    pub by_book: BTreeMap<String, Vec<String>>,
    pub by_state: BTreeMap<String, Vec<String>>,
    pub by_expiration_height: BTreeMap<u64, Vec<String>>,
}

impl OfferIndexes {
    fn sort_values(&mut self) {
        for offer_ids in self.by_owner.values_mut() {
            offer_ids.sort();
        }
        for offer_ids in self.by_book.values_mut() {
            offer_ids.sort();
        }
        for offer_ids in self.by_state.values_mut() {
            offer_ids.sort();
        }
        for offer_ids in self.by_expiration_height.values_mut() {
            offer_ids.sort();
        }
    }
}

pub const PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT: &str = "primary_pftl_mint";
pub const PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED: &str = "source_debited";
pub const PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V1: u32 = 1;
pub const PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED: &str = "destination_consumed";
pub const PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED: &str = "source_refunded";
pub const PFTL_UNISWAP_RETURN_STATUS_IMPORTED: &str = "imported";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapConsensusExportPacket {
    pub packet_hash: String,
    pub nonce: String,
    pub source_wallet: String,
    pub ethereum_recipient: String,
    pub amount_atoms: u64,
    pub source_height: u64,
    pub destination_deadline_seconds: u64,
    pub refund_not_before_height: u64,
    pub status: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethereum_packet_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethereum_packet_schema_version: Option<u32>,
}

impl PftlUniswapConsensusExportPacket {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "pftl_uniswap_export_packet.packet_hash",
            &self.packet_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len("pftl_uniswap_export_packet.nonce", &self.nonce, 64)?;
        if let Some(packet_digest) = &self.ethereum_packet_digest {
            validate_lower_hex_len(
                "pftl_uniswap_export_packet.ethereum_packet_digest",
                packet_digest,
                64,
            )?;
        }
        if self.ethereum_packet_schema_version.is_some_and(|version| version == 0) {
            return Err(
                "pftl_uniswap_export_packet.ethereum_packet_schema_version must be nonzero"
                    .to_string(),
            );
        }
        validate_text_field("pftl_uniswap_export_packet.source_wallet", &self.source_wallet)?;
        validate_evm_address_text(
            "pftl_uniswap_export_packet.ethereum_recipient",
            &self.ethereum_recipient,
        )?;
        if self.amount_atoms == 0
            || self.source_height == 0
            || self.destination_deadline_seconds == 0
            || self.refund_not_before_height == 0
        {
            return Err(
                "pftl_uniswap_export_packet height/deadline/amount fields must be nonzero"
                    .to_string(),
            );
        }
        if self.refund_not_before_height <= self.source_height {
            return Err(
                "pftl_uniswap_export_packet.refund_not_before_height must exceed source_height"
                    .to_string(),
            );
        }
        match self.status.as_str() {
            PFTL_UNISWAP_EXPORT_STATUS_SOURCE_DEBITED
            | PFTL_UNISWAP_EXPORT_STATUS_DESTINATION_CONSUMED
            | PFTL_UNISWAP_EXPORT_STATUS_SOURCE_REFUNDED => Ok(()),
            _ => Err("unsupported pftl_uniswap export packet status".to_string()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapConsensusReturnImport {
    pub burn_event_hash: String,
    pub ethereum_chain_id: u64,
    pub bridge_controller: String,
    pub wrapped_navcoin_token: String,
    pub native_nav_asset_id: String,
    pub ethereum_sender: String,
    pub pftl_recipient: String,
    pub amount_atoms: u64,
    pub return_nonce: String,
    pub burn_height: u64,
    pub finalized_height: u64,
    pub status: String,
}

impl PftlUniswapConsensusReturnImport {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len("pftl_uniswap_return.burn_event_hash", &self.burn_event_hash, 64)?;
        if self.ethereum_chain_id == 0 {
            return Err("pftl_uniswap_return.ethereum_chain_id must be nonzero".to_string());
        }
        validate_evm_address_text("pftl_uniswap_return.bridge_controller", &self.bridge_controller)?;
        validate_evm_address_text(
            "pftl_uniswap_return.wrapped_navcoin_token",
            &self.wrapped_navcoin_token,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_return.native_nav_asset_id",
            &self.native_nav_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_evm_address_text("pftl_uniswap_return.ethereum_sender", &self.ethereum_sender)?;
        validate_text_field("pftl_uniswap_return.pftl_recipient", &self.pftl_recipient)?;
        if self.amount_atoms == 0 || self.burn_height == 0 || self.finalized_height == 0 {
            return Err("pftl_uniswap_return amount and heights must be nonzero".to_string());
        }
        validate_lower_hex_len("pftl_uniswap_return.return_nonce", &self.return_nonce, 64)?;
        if self.status != PFTL_UNISWAP_RETURN_STATUS_IMPORTED {
            return Err("unsupported pftl_uniswap return import status".to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapConsensusRouteState {
    pub route_id: String,
    pub route_family: String,
    pub route_config_digest: String,
    pub route_trust_class: String,
    pub native_nav_asset_id: String,
    pub settlement_asset_id: String,
    pub handoff_controller: String,
    pub settlement_adapter: String,
    pub wrapped_navcoin_token: String,
    pub ethereum_chain_id: u64,
    pub route_supply_cap_atoms: u64,
    pub packet_notional_cap_atoms: u64,
    pub latest_finalized_nav_epoch: u64,
    pub return_finality_blocks: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ethereum_verification_policy: Option<EthereumRouteVerificationPolicyV1>,
    pub authorized_valid_supply_atoms: u64,
    pub pftl_spendable_supply_atoms: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub native_spendable_balances_atoms: BTreeMap<String, u64>,
    pub ethereum_spendable_supply_atoms: u64,
    pub other_registered_venue_supply_atoms: u64,
    pub outstanding_bridge_claims_atoms: u64,
    pub pending_return_import_claims_atoms: u64,
    pub settlement_reserve_atoms: u64,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub primary_subscription_nonces: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub export_packets: BTreeMap<String, PftlUniswapConsensusExportPacket>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub export_nonces: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub return_imports: BTreeMap<String, PftlUniswapConsensusReturnImport>,
    pub paused: bool,
}

impl PftlUniswapConsensusRouteState {
    pub fn validate(&self) -> Result<(), String> {
        validate_text_field("pftl_uniswap_route.route_id", &self.route_id)?;
        if self.route_family != PFTL_UNISWAP_ROUTE_FAMILY_PRIMARY_MINT {
            return Err("pftl_uniswap_route.route_family must be primary_pftl_mint".to_string());
        }
        validate_lower_hex_len(
            "pftl_uniswap_route.route_config_digest",
            &self.route_config_digest,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_text_field("pftl_uniswap_route.route_trust_class", &self.route_trust_class)?;
        validate_lower_hex_len(
            "pftl_uniswap_route.native_nav_asset_id",
            &self.native_nav_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_route.settlement_asset_id",
            &self.settlement_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.native_nav_asset_id == self.settlement_asset_id {
            return Err("pftl_uniswap_route native and settlement assets must differ".to_string());
        }
        validate_evm_address_text("pftl_uniswap_route.handoff_controller", &self.handoff_controller)?;
        validate_evm_address_text("pftl_uniswap_route.settlement_adapter", &self.settlement_adapter)?;
        validate_evm_address_text("pftl_uniswap_route.wrapped_navcoin_token", &self.wrapped_navcoin_token)?;
        if self.ethereum_chain_id == 0
            || self.route_supply_cap_atoms == 0
            || self.packet_notional_cap_atoms == 0
            || self.return_finality_blocks == 0
        {
            return Err(
                "pftl_uniswap_route chain, cap, and finality fields must be nonzero"
                    .to_string(),
            );
        }
        if let Some(policy) = &self.ethereum_verification_policy {
            policy.validate().map_err(|error| {
                format!("pftl_uniswap_route Ethereum verification policy: {error:?}")
            })?;
        }
        if self.primary_subscription_nonces.len() > MAX_PFTL_UNISWAP_ROUTE_ENTRIES
            || self.export_packets.len() > MAX_PFTL_UNISWAP_ROUTE_ENTRIES
            || self.export_nonces.len() > MAX_PFTL_UNISWAP_ROUTE_ENTRIES
            || self.return_imports.len() > MAX_PFTL_UNISWAP_ROUTE_ENTRIES
        {
            return Err("pftl_uniswap_route indexed state exceeds bounded route entry limit"
                .to_string());
        }
        for (nonce, wallet) in &self.primary_subscription_nonces {
            validate_lower_hex_len("pftl_uniswap_route.primary_subscription_nonce", nonce, 64)?;
            validate_text_field("pftl_uniswap_route.primary_subscription_wallet", wallet)?;
        }
        for (wallet, amount) in &self.native_spendable_balances_atoms {
            validate_text_field("pftl_uniswap_route.native_spendable_wallet", wallet)?;
            if *amount == 0 {
                return Err("pftl_uniswap_route native spendable map contains zero balance"
                    .to_string());
            }
        }
        for (packet_hash, packet) in &self.export_packets {
            packet.validate()?;
            if packet_hash != &packet.packet_hash {
                return Err("pftl_uniswap_route export packet key mismatch".to_string());
            }
        }
        for (nonce, packet_hash) in &self.export_nonces {
            validate_lower_hex_len("pftl_uniswap_route.export_nonce", nonce, 64)?;
            if !self.export_packets.contains_key(packet_hash) {
                return Err("pftl_uniswap_route export nonce references missing packet".to_string());
            }
        }
        for (burn_hash, burn) in &self.return_imports {
            burn.validate()?;
            if burn_hash != &burn.burn_event_hash {
                return Err("pftl_uniswap_route return import key mismatch".to_string());
            }
        }
        let pftl_side = self
            .pftl_spendable_supply_atoms
            .checked_add(self.outstanding_bridge_claims_atoms)
            .and_then(|value| value.checked_add(self.pending_return_import_claims_atoms))
            .ok_or_else(|| "pftl_uniswap_route pftl-side supply accounting overflow".to_string())?;
        let live_supply = pftl_side
            .checked_add(self.ethereum_spendable_supply_atoms)
            .and_then(|value| value.checked_add(self.other_registered_venue_supply_atoms))
            .ok_or_else(|| "pftl_uniswap_route live supply accounting overflow".to_string())?;
        if live_supply != self.authorized_valid_supply_atoms {
            return Err(
                "pftl_uniswap_route live supply must equal authorized valid supply".to_string(),
            );
        }
        let native_sum = self
            .native_spendable_balances_atoms
            .values()
            .try_fold(0_u64, |sum, amount| {
                sum.checked_add(*amount)
                    .ok_or_else(|| "pftl_uniswap_route native balance sum overflow".to_string())
            })?;
        if native_sum != self.pftl_spendable_supply_atoms {
            return Err(
                "pftl_uniswap_route native spendable balances must sum to pftl spendable supply"
                    .to_string(),
            );
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlUniswapConsensusReceipt {
    pub receipt_hash: String,
    pub transition: String,
    pub route_id: String,
    pub state_before_hash: String,
    pub state_after_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub packet_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub burn_event_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wallet: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount_atoms: Option<u64>,
    pub block_height: u64,
}

impl PftlUniswapConsensusReceipt {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "pftl_uniswap_receipt.receipt_hash",
            &self.receipt_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        match self.transition.as_str() {
            "route_init" | "primary_subscription" | "export_debit" | "destination_consume"
            | "source_refunded" | "return_imported" => {}
            _ => return Err("unsupported pftl_uniswap receipt transition".to_string()),
        }
        validate_text_field("pftl_uniswap_receipt.route_id", &self.route_id)?;
        validate_lower_hex_len(
            "pftl_uniswap_receipt.state_before_hash",
            &self.state_before_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        validate_lower_hex_len(
            "pftl_uniswap_receipt.state_after_hash",
            &self.state_after_hash,
            VAULT_BRIDGE_HEX_HASH_LEN,
        )?;
        if let Some(packet_hash) = &self.packet_hash {
            validate_lower_hex_len(
                "pftl_uniswap_receipt.packet_hash",
                packet_hash,
                VAULT_BRIDGE_HEX_HASH_LEN,
            )?;
        }
        if let Some(burn_hash) = &self.burn_event_hash {
            validate_lower_hex_len("pftl_uniswap_receipt.burn_event_hash", burn_hash, 64)?;
        }
        if let Some(wallet) = &self.wallet {
            validate_text_field("pftl_uniswap_receipt.wallet", wallet)?;
        }
        if self.amount_atoms == Some(0) || self.block_height == 0 {
            return Err("pftl_uniswap_receipt amount, when present, and height must be nonzero"
                .to_string());
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LedgerState {
    #[serde(default)]
    pub accounts: Vec<Account>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub asset_definitions: Vec<AssetDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub trustlines: Vec<TrustLine>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub escrows: Vec<Escrow>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nfts: Vec<NftDefinition>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub offers: Vec<Offer>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nav_assets: Vec<NavTrackedAsset>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nav_reserve_packets: Vec<NavReservePacket>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nav_redemptions: Vec<NavRedemption>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nav_proof_profiles: Vec<NavProofProfile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub nav_attestors: Vec<NavAttestor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub market_ops_policies: Vec<MarketOpsPolicyRegistration>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub market_ops_envelopes: Vec<FinalizedMarketOpsEnvelope>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vault_bridge_receipts: Vec<VaultBridgeReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vault_bridge_bucket_states: Vec<VaultBridgeBucketState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vault_bridge_allocations: Vec<VaultBridgeAllocation>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vault_bridge_redemptions: Vec<VaultBridgeRedemption>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vault_bridge_deposits: Vec<VaultBridgeDepositRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pftl_uniswap_routes: Vec<PftlUniswapConsensusRouteState>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pftl_uniswap_receipts: Vec<PftlUniswapConsensusReceipt>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub owned_objects: Vec<OwnedObject>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fastpay_recovery_policy: Option<FastPayRecoveryPolicyV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastpay_recovery_committees: Vec<FastPayRecoveryCommitteeV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastpay_recovery_reveals: Vec<FastPayRecoveryRevealV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastpay_version_fences: Vec<FastPayVersionFenceV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fast_lane_reserves: Vec<FastLaneReserveBalanceV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fast_lane_deposit_receipts: Vec<FastLaneDepositReceiptV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub redeemed_fast_lane_exit_claims: Vec<FastSwapExitClaimIdV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fast_lane_asset_rules: Vec<FastAssetRuleV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fast_lane_holder_permits: Vec<FastHolderPermitV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastswap_policy_snapshots: Vec<FastSwapPolicySnapshotV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fastswap_committees: Vec<FastSwapCommitteeV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fast_lane_prepare_fences: Vec<FastLanePrepareFenceV1>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub fast_lane_checkpoint_anchors: Vec<FastLaneCheckpointCertificateV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fastswap_activation_height: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub ethereum_arbitrum_finality_states: Vec<EthereumArbitrumFinalityStateV1>,
}

impl LedgerState {
    pub fn new(accounts: Vec<Account>) -> Self {
        Self {
            accounts,
            asset_definitions: Vec::new(),
            trustlines: Vec::new(),
            escrows: Vec::new(),
            nfts: Vec::new(),
            offers: Vec::new(),
            nav_assets: Vec::new(),
            nav_reserve_packets: Vec::new(),
            nav_redemptions: Vec::new(),
            nav_proof_profiles: Vec::new(),
            nav_attestors: Vec::new(),
            market_ops_policies: Vec::new(),
            market_ops_envelopes: Vec::new(),
            vault_bridge_receipts: Vec::new(),
            vault_bridge_bucket_states: Vec::new(),
            vault_bridge_allocations: Vec::new(),
            vault_bridge_redemptions: Vec::new(),
            vault_bridge_deposits: Vec::new(),
            pftl_uniswap_routes: Vec::new(),
            pftl_uniswap_receipts: Vec::new(),
            owned_objects: Vec::new(),
            fastpay_recovery_policy: None,
            fastpay_recovery_committees: Vec::new(),
            fastpay_recovery_reveals: Vec::new(),
            fastpay_version_fences: Vec::new(),
            fast_lane_reserves: Vec::new(),
            fast_lane_deposit_receipts: Vec::new(),
            redeemed_fast_lane_exit_claims: Vec::new(),
            fast_lane_asset_rules: Vec::new(),
            fast_lane_holder_permits: Vec::new(),
            fastswap_policy_snapshots: Vec::new(),
            fastswap_committees: Vec::new(),
            fast_lane_prepare_fences: Vec::new(),
            fast_lane_checkpoint_anchors: Vec::new(),
            fastswap_activation_height: None,
            ethereum_arbitrum_finality_states: Vec::new(),
        }
    }

    pub fn new_with_assets(
        accounts: Vec<Account>,
        asset_definitions: Vec<AssetDefinition>,
        trustlines: Vec<TrustLine>,
    ) -> Self {
        Self {
            accounts,
            asset_definitions,
            trustlines,
            escrows: Vec::new(),
            nfts: Vec::new(),
            offers: Vec::new(),
            nav_assets: Vec::new(),
            nav_reserve_packets: Vec::new(),
            nav_redemptions: Vec::new(),
            nav_proof_profiles: Vec::new(),
            nav_attestors: Vec::new(),
            market_ops_policies: Vec::new(),
            market_ops_envelopes: Vec::new(),
            vault_bridge_receipts: Vec::new(),
            vault_bridge_bucket_states: Vec::new(),
            vault_bridge_allocations: Vec::new(),
            vault_bridge_redemptions: Vec::new(),
            vault_bridge_deposits: Vec::new(),
            pftl_uniswap_routes: Vec::new(),
            pftl_uniswap_receipts: Vec::new(),
            owned_objects: Vec::new(),
            fastpay_recovery_policy: None,
            fastpay_recovery_committees: Vec::new(),
            fastpay_recovery_reveals: Vec::new(),
            fastpay_version_fences: Vec::new(),
            fast_lane_reserves: Vec::new(),
            fast_lane_deposit_receipts: Vec::new(),
            redeemed_fast_lane_exit_claims: Vec::new(),
            fast_lane_asset_rules: Vec::new(),
            fast_lane_holder_permits: Vec::new(),
            fastswap_policy_snapshots: Vec::new(),
            fastswap_committees: Vec::new(),
            fast_lane_prepare_fences: Vec::new(),
            fast_lane_checkpoint_anchors: Vec::new(),
            fastswap_activation_height: None,
            ethereum_arbitrum_finality_states: Vec::new(),
        }
    }

    pub fn new_with_ledger_objects(
        accounts: Vec<Account>,
        asset_definitions: Vec<AssetDefinition>,
        trustlines: Vec<TrustLine>,
        escrows: Vec<Escrow>,
    ) -> Self {
        Self {
            accounts,
            asset_definitions,
            trustlines,
            escrows,
            nfts: Vec::new(),
            offers: Vec::new(),
            nav_assets: Vec::new(),
            nav_reserve_packets: Vec::new(),
            nav_redemptions: Vec::new(),
            nav_proof_profiles: Vec::new(),
            nav_attestors: Vec::new(),
            market_ops_policies: Vec::new(),
            market_ops_envelopes: Vec::new(),
            vault_bridge_receipts: Vec::new(),
            vault_bridge_bucket_states: Vec::new(),
            vault_bridge_allocations: Vec::new(),
            vault_bridge_redemptions: Vec::new(),
            vault_bridge_deposits: Vec::new(),
            pftl_uniswap_routes: Vec::new(),
            pftl_uniswap_receipts: Vec::new(),
            owned_objects: Vec::new(),
            fastpay_recovery_policy: None,
            fastpay_recovery_committees: Vec::new(),
            fastpay_recovery_reveals: Vec::new(),
            fastpay_version_fences: Vec::new(),
            fast_lane_reserves: Vec::new(),
            fast_lane_deposit_receipts: Vec::new(),
            redeemed_fast_lane_exit_claims: Vec::new(),
            fast_lane_asset_rules: Vec::new(),
            fast_lane_holder_permits: Vec::new(),
            fastswap_policy_snapshots: Vec::new(),
            fastswap_committees: Vec::new(),
            fast_lane_prepare_fences: Vec::new(),
            fast_lane_checkpoint_anchors: Vec::new(),
            fastswap_activation_height: None,
            ethereum_arbitrum_finality_states: Vec::new(),
        }
    }

    pub fn new_with_nfts(accounts: Vec<Account>, nfts: Vec<NftDefinition>) -> Self {
        Self {
            accounts,
            asset_definitions: Vec::new(),
            trustlines: Vec::new(),
            escrows: Vec::new(),
            nfts,
            offers: Vec::new(),
            nav_assets: Vec::new(),
            nav_reserve_packets: Vec::new(),
            nav_redemptions: Vec::new(),
            nav_proof_profiles: Vec::new(),
            nav_attestors: Vec::new(),
            market_ops_policies: Vec::new(),
            market_ops_envelopes: Vec::new(),
            vault_bridge_receipts: Vec::new(),
            vault_bridge_bucket_states: Vec::new(),
            vault_bridge_allocations: Vec::new(),
            vault_bridge_redemptions: Vec::new(),
            vault_bridge_deposits: Vec::new(),
            pftl_uniswap_routes: Vec::new(),
            pftl_uniswap_receipts: Vec::new(),
            owned_objects: Vec::new(),
            fastpay_recovery_policy: None,
            fastpay_recovery_committees: Vec::new(),
            fastpay_recovery_reveals: Vec::new(),
            fastpay_version_fences: Vec::new(),
            fast_lane_reserves: Vec::new(),
            fast_lane_deposit_receipts: Vec::new(),
            redeemed_fast_lane_exit_claims: Vec::new(),
            fast_lane_asset_rules: Vec::new(),
            fast_lane_holder_permits: Vec::new(),
            fastswap_policy_snapshots: Vec::new(),
            fastswap_committees: Vec::new(),
            fast_lane_prepare_fences: Vec::new(),
            fast_lane_checkpoint_anchors: Vec::new(),
            fastswap_activation_height: None,
            ethereum_arbitrum_finality_states: Vec::new(),
        }
    }

    pub fn new_with_offers(
        accounts: Vec<Account>,
        asset_definitions: Vec<AssetDefinition>,
        trustlines: Vec<TrustLine>,
        offers: Vec<Offer>,
    ) -> Self {
        Self {
            accounts,
            asset_definitions,
            trustlines,
            escrows: Vec::new(),
            nfts: Vec::new(),
            offers,
            nav_assets: Vec::new(),
            nav_reserve_packets: Vec::new(),
            nav_redemptions: Vec::new(),
            nav_proof_profiles: Vec::new(),
            nav_attestors: Vec::new(),
            market_ops_policies: Vec::new(),
            market_ops_envelopes: Vec::new(),
            vault_bridge_receipts: Vec::new(),
            vault_bridge_bucket_states: Vec::new(),
            vault_bridge_allocations: Vec::new(),
            vault_bridge_redemptions: Vec::new(),
            vault_bridge_deposits: Vec::new(),
            pftl_uniswap_routes: Vec::new(),
            pftl_uniswap_receipts: Vec::new(),
            owned_objects: Vec::new(),
            fastpay_recovery_policy: None,
            fastpay_recovery_committees: Vec::new(),
            fastpay_recovery_reveals: Vec::new(),
            fastpay_version_fences: Vec::new(),
            fast_lane_reserves: Vec::new(),
            fast_lane_deposit_receipts: Vec::new(),
            redeemed_fast_lane_exit_claims: Vec::new(),
            fast_lane_asset_rules: Vec::new(),
            fast_lane_holder_permits: Vec::new(),
            fastswap_policy_snapshots: Vec::new(),
            fastswap_committees: Vec::new(),
            fast_lane_prepare_fences: Vec::new(),
            fast_lane_checkpoint_anchors: Vec::new(),
            fastswap_activation_height: None,
            ethereum_arbitrum_finality_states: Vec::new(),
        }
    }

    pub fn empty() -> Self {
        Self {
            accounts: Vec::new(),
            asset_definitions: Vec::new(),
            trustlines: Vec::new(),
            escrows: Vec::new(),
            nfts: Vec::new(),
            offers: Vec::new(),
            nav_assets: Vec::new(),
            nav_reserve_packets: Vec::new(),
            nav_redemptions: Vec::new(),
            nav_proof_profiles: Vec::new(),
            nav_attestors: Vec::new(),
            market_ops_policies: Vec::new(),
            market_ops_envelopes: Vec::new(),
            vault_bridge_receipts: Vec::new(),
            vault_bridge_bucket_states: Vec::new(),
            vault_bridge_allocations: Vec::new(),
            vault_bridge_redemptions: Vec::new(),
            vault_bridge_deposits: Vec::new(),
            pftl_uniswap_routes: Vec::new(),
            pftl_uniswap_receipts: Vec::new(),
            owned_objects: Vec::new(),
            fastpay_recovery_policy: None,
            fastpay_recovery_committees: Vec::new(),
            fastpay_recovery_reveals: Vec::new(),
            fastpay_version_fences: Vec::new(),
            fast_lane_reserves: Vec::new(),
            fast_lane_deposit_receipts: Vec::new(),
            redeemed_fast_lane_exit_claims: Vec::new(),
            fast_lane_asset_rules: Vec::new(),
            fast_lane_holder_permits: Vec::new(),
            fastswap_policy_snapshots: Vec::new(),
            fastswap_committees: Vec::new(),
            fast_lane_prepare_fences: Vec::new(),
            fast_lane_checkpoint_anchors: Vec::new(),
            fastswap_activation_height: None,
            ethereum_arbitrum_finality_states: Vec::new(),
        }
    }

    pub fn account(&self, address: &str) -> Option<&Account> {
        self.accounts
            .iter()
            .find(|account| account.address == address)
    }

    pub fn account_mut(&mut self, address: &str) -> Option<&mut Account> {
        self.accounts
            .iter_mut()
            .find(|account| account.address == address)
    }

    pub fn ensure_account(&mut self, address: &str) -> &mut Account {
        if let Some(index) = self
            .accounts
            .iter()
            .position(|account| account.address == address)
        {
            return &mut self.accounts[index];
        }
        self.accounts.push(Account::new(address, 0, None));
        let index = self.accounts.len() - 1;
        &mut self.accounts[index]
    }

    pub fn asset_definition(&self, asset_id: &str) -> Option<&AssetDefinition> {
        self.asset_definitions
            .iter()
            .find(|asset| asset.asset_id == asset_id)
    }

    pub fn trustline(&self, trustline_id: &str) -> Option<&TrustLine> {
        self.trustlines
            .iter()
            .find(|line| line.trustline_id == trustline_id)
    }

    pub fn trustline_for_account_asset(&self, account: &str, asset_id: &str) -> Option<&TrustLine> {
        self.trustlines
            .iter()
            .find(|line| line.account == account && line.asset_id == asset_id)
    }

    pub fn escrow(&self, escrow_id: &str) -> Option<&Escrow> {
        self.escrows
            .iter()
            .find(|escrow| escrow.escrow_id == escrow_id)
    }

    pub fn nft(&self, nft_id: &str) -> Option<&NftDefinition> {
        self.nfts.iter().find(|nft| nft.nft_id == nft_id)
    }

    pub fn offer(&self, offer_id: &str) -> Option<&Offer> {
        self.offers.iter().find(|offer| offer.offer_id == offer_id)
    }

    pub fn nav_asset(&self, asset_id: &str) -> Option<&NavTrackedAsset> {
        self.nav_assets
            .iter()
            .find(|nav_asset| nav_asset.asset_id == asset_id)
    }

    pub fn nav_asset_mut(&mut self, asset_id: &str) -> Option<&mut NavTrackedAsset> {
        self.nav_assets
            .iter_mut()
            .find(|nav_asset| nav_asset.asset_id == asset_id)
    }

    pub fn nav_reserve_packet(
        &self,
        asset_id: &str,
        epoch: u64,
        reserve_packet_hash: &str,
    ) -> Option<&NavReservePacket> {
        self.nav_reserve_packets.iter().find(|packet| {
            packet.asset_id == asset_id
                && packet.epoch == epoch
                && packet.reserve_packet_hash == reserve_packet_hash
        })
    }

    pub fn nav_reserve_packet_mut(
        &mut self,
        asset_id: &str,
        epoch: u64,
        reserve_packet_hash: &str,
    ) -> Option<&mut NavReservePacket> {
        self.nav_reserve_packets.iter_mut().find(|packet| {
            packet.asset_id == asset_id
                && packet.epoch == epoch
                && packet.reserve_packet_hash == reserve_packet_hash
        })
    }

    pub fn nav_proof_profile(&self, profile_id: &str) -> Option<&NavProofProfile> {
        self.nav_proof_profiles
            .iter()
            .find(|profile| profile.profile_id == profile_id)
    }

    pub fn ethereum_arbitrum_finality_state(
        &self,
        route_profile_hash: &str,
        route_epoch: u64,
    ) -> Option<&EthereumArbitrumFinalityStateV1> {
        self.ethereum_arbitrum_finality_states.iter().find(|state| {
            state.route_profile_hash == route_profile_hash && state.route_epoch == route_epoch
        })
    }

    pub fn ethereum_arbitrum_finality_state_mut(
        &mut self,
        route_profile_hash: &str,
        route_epoch: u64,
    ) -> Option<&mut EthereumArbitrumFinalityStateV1> {
        self.ethereum_arbitrum_finality_states
            .iter_mut()
            .find(|state| {
                state.route_profile_hash == route_profile_hash && state.route_epoch == route_epoch
            })
    }

    pub fn nav_attestor(&self, address: &str) -> Option<&NavAttestor> {
        self.nav_attestors
            .iter()
            .find(|attestor| attestor.address == address)
    }

    pub fn nav_redemption(&self, redemption_id: &str) -> Option<&NavRedemption> {
        self.nav_redemptions
            .iter()
            .find(|redemption| redemption.redemption_id == redemption_id)
    }

    pub fn nav_redemption_mut(&mut self, redemption_id: &str) -> Option<&mut NavRedemption> {
        self.nav_redemptions
            .iter_mut()
            .find(|redemption| redemption.redemption_id == redemption_id)
    }

    pub fn market_ops_policy_for_envelope(
        &self,
        envelope: &MarketOpsEnvelope,
    ) -> Option<&MarketOpsPolicyRegistration> {
        self.market_ops_policies
            .iter()
            .find(|policy| policy.accepts(envelope))
    }

    pub fn market_ops_envelope(
        &self,
        asset_id: &str,
        epoch: u64,
    ) -> Option<&FinalizedMarketOpsEnvelope> {
        self.market_ops_envelopes
            .iter()
            .find(|record| record.asset_id == asset_id && record.epoch == epoch)
    }

    pub fn vault_bridge_receipt(&self, receipt_id: &str) -> Option<&VaultBridgeReceipt> {
        self.vault_bridge_receipts
            .iter()
            .find(|receipt| receipt.receipt_id == receipt_id)
    }

    pub fn vault_bridge_receipt_mut(&mut self, receipt_id: &str) -> Option<&mut VaultBridgeReceipt> {
        self.vault_bridge_receipts
            .iter_mut()
            .find(|receipt| receipt.receipt_id == receipt_id)
    }

    pub fn vault_bridge_bucket(&self, bucket_id: &str) -> Option<&VaultBridgeBucketState> {
        self.vault_bridge_bucket_states
            .iter()
            .find(|bucket| bucket.bucket_id == bucket_id)
    }

    pub fn vault_bridge_bucket_mut(&mut self, bucket_id: &str) -> Option<&mut VaultBridgeBucketState> {
        self.vault_bridge_bucket_states
            .iter_mut()
            .find(|bucket| bucket.bucket_id == bucket_id)
    }

    pub fn vault_bridge_allocation(&self, allocation_id: &str) -> Option<&VaultBridgeAllocation> {
        self.vault_bridge_allocations
            .iter()
            .find(|allocation| allocation.allocation_id == allocation_id)
    }

    pub fn vault_bridge_redemption(&self, redemption_id: &str) -> Option<&VaultBridgeRedemption> {
        self.vault_bridge_redemptions
            .iter()
            .find(|redemption| redemption.redemption_id == redemption_id)
    }

    pub fn vault_bridge_redemption_mut(&mut self, redemption_id: &str) -> Option<&mut VaultBridgeRedemption> {
        self.vault_bridge_redemptions
            .iter_mut()
            .find(|redemption| redemption.redemption_id == redemption_id)
    }

    pub fn vault_bridge_deposit(
        &self,
        asset_id: &str,
        evidence_root: &str,
    ) -> Option<&VaultBridgeDepositRecord> {
        self.vault_bridge_deposits
            .iter()
            .find(|record| record.asset_id == asset_id && record.evidence_root == evidence_root)
    }

    pub fn vault_bridge_deposit_mut(
        &mut self,
        asset_id: &str,
        evidence_root: &str,
    ) -> Option<&mut VaultBridgeDepositRecord> {
        self.vault_bridge_deposits
            .iter_mut()
            .find(|record| record.asset_id == asset_id && record.evidence_root == evidence_root)
    }

    pub fn pftl_uniswap_route(&self, route_id: &str) -> Option<&PftlUniswapConsensusRouteState> {
        self.pftl_uniswap_routes
            .iter()
            .find(|route| route.route_id == route_id)
    }

    pub fn pftl_uniswap_route_mut(
        &mut self,
        route_id: &str,
    ) -> Option<&mut PftlUniswapConsensusRouteState> {
        self.pftl_uniswap_routes
            .iter_mut()
            .find(|route| route.route_id == route_id)
    }

    pub fn escrow_indexes(&self, chain_id: &str) -> Result<EscrowIndexes, String> {
        self.validate_escrow_state(chain_id)?;
        let mut indexes = EscrowIndexes::default();
        for escrow in &self.escrows {
            indexes
                .by_owner
                .entry(escrow.owner.clone())
                .or_default()
                .push(escrow.escrow_id.clone());
            indexes
                .by_recipient
                .entry(escrow.recipient.clone())
                .or_default()
                .push(escrow.escrow_id.clone());
            if !escrow.condition.is_empty() {
                indexes
                    .by_condition_hash
                    .entry(escrow_condition_hash(&escrow.condition)?)
                    .or_default()
                    .push(escrow.escrow_id.clone());
            }
            if escrow.cancel_after != 0 {
                indexes
                    .by_expiry_height
                    .entry(escrow.cancel_after)
                    .or_default()
                    .push(escrow.escrow_id.clone());
            }
        }
        indexes.sort_values();
        Ok(indexes)
    }

    pub fn nft_indexes(&self, chain_id: &str) -> Result<NftIndexes, String> {
        self.validate_nft_state(chain_id)?;
        let mut indexes = NftIndexes::default();
        for nft in &self.nfts {
            if !nft.burned {
                indexes
                    .by_owner
                    .entry(nft.owner.clone())
                    .or_default()
                    .push(nft.nft_id.clone());
            }
            indexes
                .by_issuer
                .entry(nft.issuer.clone())
                .or_default()
                .push(nft.nft_id.clone());
            indexes
                .by_collection
                .entry(nft.collection_id.clone())
                .or_default()
                .push(nft.nft_id.clone());
        }
        indexes.sort_values();
        Ok(indexes)
    }

    pub fn offer_indexes(&self, chain_id: &str) -> Result<OfferIndexes, String> {
        self.validate_offer_state(chain_id)?;
        let mut indexes = OfferIndexes::default();
        for offer in &self.offers {
            indexes
                .by_owner
                .entry(offer.owner.clone())
                .or_default()
                .push(offer.offer_id.clone());
            indexes
                .by_book
                .entry(offer_book_key(
                    &offer.taker_gets_asset_id,
                    &offer.taker_pays_asset_id,
                )?)
                .or_default()
                .push(offer.offer_id.clone());
            indexes
                .by_state
                .entry(offer.state.clone())
                .or_default()
                .push(offer.offer_id.clone());
            if offer.expiration_height != 0 {
                indexes
                    .by_expiration_height
                    .entry(offer.expiration_height)
                    .or_default()
                    .push(offer.offer_id.clone());
            }
        }
        indexes.sort_values();
        Ok(indexes)
    }

    pub fn validate_asset_state(&self, chain_id: &str) -> Result<(), String> {
        let mut asset_ids = BTreeSet::new();
        let mut assets_by_id = BTreeMap::new();
        for asset in &self.asset_definitions {
            asset.validate_for_chain(chain_id)?;
            if !asset_ids.insert(asset.asset_id.clone()) {
                return Err(format!("duplicate asset id `{}`", asset.asset_id));
            }
            assets_by_id.insert(asset.asset_id.clone(), asset);
        }

        let mut trustline_ids = BTreeSet::new();
        for line in &self.trustlines {
            line.validate()?;
            if !trustline_ids.insert(line.trustline_id.clone()) {
                return Err(format!("duplicate trustline id `{}`", line.trustline_id));
            }
            let Some(asset) = assets_by_id.get(&line.asset_id) else {
                return Err(format!(
                    "trustline `{}` references missing asset `{}`",
                    line.trustline_id, line.asset_id
                ));
            };
            if line.issuer != asset.issuer {
                return Err(format!(
                    "trustline `{}` issuer does not match asset issuer",
                    line.trustline_id
                ));
            }
        }

        let mut vault_bridge_receipt_ids = BTreeSet::new();
        for receipt in &self.vault_bridge_receipts {
            receipt.validate_for_chain(chain_id)?;
            if !vault_bridge_receipt_ids.insert(receipt.receipt_id.clone()) {
                return Err(format!("duplicate vault_bridge receipt id `{}`", receipt.receipt_id));
            }
            if !assets_by_id.contains_key(&receipt.asset_id) {
                return Err(format!(
                    "vault_bridge receipt `{}` references missing asset `{}`",
                    receipt.receipt_id, receipt.asset_id
                ));
            }
        }

        let mut vault_bridge_deposit_keys = BTreeSet::new();
        for record in &self.vault_bridge_deposits {
            record.validate()?;
            let key = (record.asset_id.clone(), record.evidence_root.clone());
            if !vault_bridge_deposit_keys.insert(key) {
                return Err(format!(
                    "duplicate vault_bridge bridge deposit `{}` for asset `{}`",
                    record.evidence_root, record.asset_id
                ));
            }
            if !assets_by_id.contains_key(&record.asset_id) {
                return Err(format!(
                    "vault_bridge bridge deposit `{}` references missing asset `{}`",
                    record.evidence_root, record.asset_id
                ));
            }
        }

        let mut vault_bridge_bucket_ids = BTreeSet::new();
        for bucket in &self.vault_bridge_bucket_states {
            bucket.validate()?;
            if !vault_bridge_bucket_ids.insert(bucket.bucket_id.clone()) {
                return Err(format!("duplicate vault_bridge bucket id `{}`", bucket.bucket_id));
            }
            if !assets_by_id.contains_key(&bucket.asset_id) {
                return Err(format!(
                    "vault_bridge bucket `{}` references missing asset `{}`",
                    bucket.bucket_id, bucket.asset_id
                ));
            }
        }

        let mut vault_bridge_allocation_ids = BTreeSet::new();
        for allocation in &self.vault_bridge_allocations {
            allocation.validate_for_chain(chain_id)?;
            if !vault_bridge_allocation_ids.insert(allocation.allocation_id.clone()) {
                return Err(format!(
                    "duplicate vault_bridge allocation id `{}`",
                    allocation.allocation_id
                ));
            }
            if !vault_bridge_receipt_ids.contains(&allocation.receipt_id) {
                return Err(format!(
                    "vault_bridge allocation `{}` references missing receipt `{}`",
                    allocation.allocation_id, allocation.receipt_id
                ));
            }
            if !vault_bridge_bucket_ids.contains(&allocation.bucket_id) {
                return Err(format!(
                    "vault_bridge allocation `{}` references missing bucket `{}`",
                    allocation.allocation_id, allocation.bucket_id
                ));
            }
        }

        let mut vault_bridge_redemption_ids = BTreeSet::new();
        for redemption in &self.vault_bridge_redemptions {
            redemption.validate_for_chain(chain_id)?;
            if !vault_bridge_redemption_ids.insert(redemption.redemption_id.clone()) {
                return Err(format!(
                    "duplicate vault_bridge redemption id `{}`",
                    redemption.redemption_id
                ));
            }
            if !assets_by_id.contains_key(&redemption.asset_id) {
                return Err(format!(
                    "vault_bridge redemption `{}` references missing asset `{}`",
                    redemption.redemption_id, redemption.asset_id
                ));
            }
            if !vault_bridge_bucket_ids.contains(&redemption.bucket_id) {
                return Err(format!(
                    "vault_bridge redemption `{}` references missing bucket `{}`",
                    redemption.redemption_id, redemption.bucket_id
                ));
            }
        }

        if self.pftl_uniswap_routes.len() > MAX_PFTL_UNISWAP_ROUTES {
            return Err("pftl_uniswap route count exceeds bounded consensus limit".to_string());
        }
        let nav_assets_by_id = self
            .nav_assets
            .iter()
            .map(|nav_asset| (nav_asset.asset_id.clone(), nav_asset))
            .collect::<BTreeMap<_, _>>();
        let mut pftl_uniswap_route_count_by_native_issuer = BTreeMap::new();
        let mut pftl_uniswap_route_ids = BTreeSet::new();
        for route in &self.pftl_uniswap_routes {
            route.validate()?;
            if !pftl_uniswap_route_ids.insert(route.route_id.clone()) {
                return Err(format!("duplicate pftl_uniswap route id `{}`", route.route_id));
            }
            let Some(native_asset) = assets_by_id.get(&route.native_nav_asset_id) else {
                return Err(format!(
                    "pftl_uniswap route `{}` references missing native NAV asset `{}`",
                    route.route_id, route.native_nav_asset_id
                ));
            };
            let Some(native_nav_asset) = nav_assets_by_id.get(&route.native_nav_asset_id) else {
                return Err(format!(
                    "pftl_uniswap route `{}` references unregistered native NAV asset `{}`",
                    route.route_id, route.native_nav_asset_id
                ));
            };
            if native_nav_asset.issuer != native_asset.issuer {
                return Err(format!(
                    "pftl_uniswap route `{}` native NAV issuer does not match issued asset issuer",
                    route.route_id
                ));
            }
            let route_count = pftl_uniswap_route_count_by_native_issuer
                .entry(native_nav_asset.issuer.clone())
                .or_insert(0_usize);
            *route_count += 1;
            if *route_count > MAX_PFTL_UNISWAP_ROUTES_PER_NATIVE_ISSUER {
                return Err(format!(
                    "pftl_uniswap route count for native NAV issuer `{}` exceeds bounded consensus limit",
                    native_nav_asset.issuer
                ));
            }
            if !assets_by_id.contains_key(&route.settlement_asset_id) {
                return Err(format!(
                    "pftl_uniswap route `{}` references missing settlement asset `{}`",
                    route.route_id, route.settlement_asset_id
                ));
            }
        }

        if self.pftl_uniswap_receipts.len() > MAX_PFTL_UNISWAP_RECEIPTS {
            return Err("pftl_uniswap receipt count exceeds bounded consensus limit".to_string());
        }
        let mut pftl_uniswap_receipt_hashes = BTreeSet::new();
        for receipt in &self.pftl_uniswap_receipts {
            receipt.validate()?;
            if !pftl_uniswap_receipt_hashes.insert(receipt.receipt_hash.clone()) {
                return Err(format!(
                    "duplicate pftl_uniswap receipt hash `{}`",
                    receipt.receipt_hash
                ));
            }
            if !pftl_uniswap_route_ids.contains(&receipt.route_id) {
                return Err(format!(
                    "pftl_uniswap receipt `{}` references missing route `{}`",
                    receipt.receipt_hash, receipt.route_id
                ));
            }
        }
        Ok(())
    }

    pub fn validate_escrow_state(&self, chain_id: &str) -> Result<(), String> {
        let mut escrow_ids = BTreeSet::new();
        for escrow in &self.escrows {
            escrow.validate_for_chain(chain_id)?;
            if !escrow_ids.insert(escrow.escrow_id.clone()) {
                return Err(format!("duplicate escrow id `{}`", escrow.escrow_id));
            }
        }
        Ok(())
    }

    pub fn validate_nft_state(&self, chain_id: &str) -> Result<(), String> {
        let mut nft_ids = BTreeSet::new();
        let mut collection_serials = BTreeSet::new();
        let mut collection_flags_by_key = BTreeMap::new();
        for nft in &self.nfts {
            nft.validate_for_chain(chain_id)?;
            if !nft_ids.insert(nft.nft_id.clone()) {
                return Err(format!("duplicate nft id `{}`", nft.nft_id));
            }
            let collection_key = (nft.issuer.clone(), nft.collection_id.clone(), nft.serial);
            if !collection_serials.insert(collection_key) {
                return Err(format!(
                    "duplicate nft serial `{}` for issuer `{}` collection `{}`",
                    nft.serial, nft.issuer, nft.collection_id
                ));
            }
            let collection_policy_key = (nft.issuer.clone(), nft.collection_id.clone());
            if let Some(collection_flags) =
                collection_flags_by_key.insert(collection_policy_key.clone(), nft.collection_flags)
            {
                if collection_flags != nft.collection_flags {
                    return Err(format!(
                        "nft collection flags mismatch for issuer `{}` collection `{}`",
                        nft.issuer, nft.collection_id
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn validate_offer_state(&self, chain_id: &str) -> Result<(), String> {
        let mut asset_ids = BTreeSet::new();
        for asset in &self.asset_definitions {
            asset.validate_for_chain(chain_id)?;
            if !asset_ids.insert(asset.asset_id.clone()) {
                return Err(format!("duplicate asset id `{}`", asset.asset_id));
            }
        }
        let mut offer_ids = BTreeSet::new();
        for offer in &self.offers {
            offer.validate_for_chain(chain_id)?;
            if !offer_ids.insert(offer.offer_id.clone()) {
                return Err(format!("duplicate offer id `{}`", offer.offer_id));
            }
            for asset_id in [&offer.taker_gets_asset_id, &offer.taker_pays_asset_id] {
                if asset_id != "PFT" && !asset_ids.contains(asset_id) {
                    return Err(format!(
                        "offer `{}` references missing asset `{asset_id}`",
                        offer.offer_id
                    ));
                }
            }
        }
        Ok(())
    }

    pub fn validate_nav_state(&self, chain_id: &str) -> Result<(), String> {
        let mut finality_route_epochs = BTreeSet::new();
        for state in &self.ethereum_arbitrum_finality_states {
            state.validate()?;
            if !finality_route_epochs.insert((state.route_profile_hash.clone(), state.route_epoch)) {
                return Err("duplicate pfUSDC Ethereum/Arbitrum finality-state route epoch"
                    .to_string());
            }
        }
        let mut asset_ids = BTreeSet::new();
        let assets_by_id = self
            .asset_definitions
            .iter()
            .map(|asset| (asset.asset_id.clone(), asset))
            .collect::<BTreeMap<_, _>>();
        for nav_asset in &self.nav_assets {
            nav_asset.validate()?;
            if !asset_ids.insert(nav_asset.asset_id.clone()) {
                return Err(format!("duplicate nav asset id `{}`", nav_asset.asset_id));
            }
            let Some(asset) = assets_by_id.get(&nav_asset.asset_id) else {
                return Err(format!(
                    "nav asset `{}` references missing issued asset",
                    nav_asset.asset_id
                ));
            };
            if nav_asset.issuer != asset.issuer {
                return Err(format!(
                    "nav asset `{}` issuer does not match issued asset issuer",
                    nav_asset.asset_id
                ));
            }
        }

        let mut packet_ids = BTreeSet::new();
        for packet in &self.nav_reserve_packets {
            packet.validate()?;
            if !packet_ids.insert(packet.packet_id.clone()) {
                return Err(format!(
                    "duplicate nav reserve packet id `{}`",
                    packet.packet_id
                ));
            }
            let Some(nav_asset) = self.nav_asset(&packet.asset_id) else {
                return Err(format!(
                    "nav reserve packet `{}` references missing nav asset `{}`",
                    packet.packet_id, packet.asset_id
                ));
            };
            if packet.issuer != nav_asset.issuer {
                return Err(format!(
                    "nav reserve packet `{}` issuer does not match nav asset",
                    packet.packet_id
                ));
            }
        }

        let mut redemption_ids = BTreeSet::new();
        for redemption in &self.nav_redemptions {
            redemption.validate_for_chain(chain_id)?;
            if !redemption_ids.insert(redemption.redemption_id.clone()) {
                return Err(format!(
                    "duplicate nav redemption id `{}`",
                    redemption.redemption_id
                ));
            }
            let Some(nav_asset) = self.nav_asset(&redemption.asset_id) else {
                return Err(format!(
                    "nav redemption `{}` references missing nav asset `{}`",
                    redemption.redemption_id, redemption.asset_id
                ));
            };
            if redemption.issuer != nav_asset.issuer {
                return Err(format!(
                    "nav redemption `{}` issuer does not match nav asset",
                    redemption.redemption_id
                ));
            }
        }

        let mut policy_keys = BTreeSet::new();
        for policy in &self.market_ops_policies {
            policy.validate()?;
            let key = (
                policy.program_id,
                policy.policy_hash,
                policy.parameter_hash,
                policy.venue_id,
                policy.pool_config_hash,
                policy.hook_code_hash,
                policy.activation_epoch,
            );
            if !policy_keys.insert(key) {
                return Err("duplicate market ops policy registration".to_string());
            }
        }

        let mut envelope_keys = BTreeSet::new();
        for record in &self.market_ops_envelopes {
            record.validate()?;
            let Some(nav_asset) = self.nav_asset(&record.asset_id) else {
                return Err(format!(
                    "market ops envelope `{}` epoch `{}` references missing nav asset `{}`",
                    record.envelope_hash, record.epoch, record.asset_id
                ));
            };
            if record.epoch > nav_asset.finalized_epoch {
                return Err(format!(
                    "market ops envelope `{}` epoch `{}` exceeds finalized nav epoch `{}`",
                    record.envelope_hash, record.epoch, nav_asset.finalized_epoch
                ));
            }
            if self
                .market_ops_policy_for_envelope(&record.envelope)
                .is_none()
            {
                return Err(format!(
                    "market ops envelope `{}` references unregistered policy",
                    record.envelope_hash
                ));
            }
            let key = (record.asset_id.clone(), record.epoch);
            if !envelope_keys.insert(key) {
                return Err(format!(
                    "duplicate market ops envelope for asset `{}` epoch `{}`",
                    record.asset_id, record.epoch
                ));
            }
        }
        Ok(())
    }
}
