pub fn shield_mint(options: ShieldMintOptions) -> io::Result<ShieldedNote> {
    let _ = options;
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "legacy cleartext shield_mint is historical-replay-only; use an Asset-Orchard ingress action",
    ))
}

pub fn verify_or_apply_orchard_action(
    options: OrchardActionOptions,
) -> io::Result<OrchardActionReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let action = read_orchard_action_file(&options.action_file)?;
    let domain = orchard_authorizing_domain(&genesis, &action.pool_id)?;
    let verified =
        verify_serialized_orchard_action_with_built_key(&action, &domain).map_err(invalid_data)?;
    let mut shielded = store.read_shielded()?;
    let (receipt, applied) = if options.apply {
        let receipt = apply_verified_orchard_action_to_shielded_state(
            &genesis,
            &mut shielded,
            &action,
            &verified,
        )?;
        let applied = receipt.accepted;
        if applied {
            store.write_shielded(&shielded)?;
        }
        store.append_receipt(receipt.clone())?;
        (receipt, applied)
    } else {
        (
            Receipt::accepted(
                orchard_action_receipt_id(&genesis, &action, &verified, "verified")?,
                "Orchard action verified; state not mutated",
            ),
            false,
        )
    };

    Ok(OrchardActionReport {
        verified: true,
        applied,
        pool_id: action.pool_id,
        action_count: verified.action_count,
        nullifier_count: verified.nullifiers.len(),
        output_count: verified.output_commitments.len(),
        value_balance: verified.value_balance,
        fee: action.fee,
        receipt,
    })
}

pub fn orchard_operator_policy(
    options: OrchardOperatorPolicyOptions,
) -> io::Result<OrchardOperatorPolicyReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let genesis_hash_hex = genesis_hash(&genesis);
    validate_orchard_operator_policy_options(&options)?;

    let mut warnings = Vec::new();
    if options.privacy_enabled {
        warnings.push(
            "privacy alpha is enabled in this policy report; do not expose public write RPC until verifier worker isolation is implemented"
                .to_string(),
        );
    } else {
        warnings.push(
            "privacy alpha is disabled in this policy report; shielded write admission should fail closed"
                .to_string(),
        );
    }
    if options.max_concurrent_verifiers > DEFAULT_ORCHARD_VERIFIER_MAX_CONCURRENCY {
        warnings.push(format!(
            "configured verifier concurrency {} is above the controlled-testnet default {}; isolate verifier workers before using this on a public write edge",
            options.max_concurrent_verifiers, DEFAULT_ORCHARD_VERIFIER_MAX_CONCURRENCY
        ));
    }
    if options.verifier_timeout_ms < DEFAULT_ORCHARD_VERIFIER_TIMEOUT_MS {
        warnings.push(format!(
            "configured verifier timeout {}ms is below the controlled-testnet default {}ms; latest exact-size malformed local evidence is about 12s on this host",
            options.verifier_timeout_ms, DEFAULT_ORCHARD_VERIFIER_TIMEOUT_MS
        ));
    }

    Ok(OrchardOperatorPolicyReport {
        schema: ORCHARD_OPERATOR_POLICY_REPORT_SCHEMA.to_string(),
        chain_id: genesis.chain_id,
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        privacy_enabled: options.privacy_enabled,
        indexing_role: options.indexing_role,
        max_concurrent_verifiers: options.max_concurrent_verifiers,
        verifier_timeout_ms: options.verifier_timeout_ms,
        root_retention_roots: options.root_retention_roots,
        protocol_limits: OrchardProtocolLimitsReport {
            max_action_json_bytes: MAX_LOCAL_JSON_FILE_BYTES,
            max_actions_per_orchard_bundle: DEFAULT_MAX_ORCHARD_ACTIONS,
            max_proof_bytes: ORCHARD_PROOF_MAX_BYTES,
            max_ciphertext_blob_bytes: ORCHARD_CIPHERTEXT_MAX_BYTES,
            enc_ciphertext_bytes: ORCHARD_ENC_CIPHERTEXT_BYTES,
            out_ciphertext_bytes: ORCHARD_OUT_CIPHERTEXT_BYTES,
            compact_ciphertext_bytes: ORCHARD_COMPACT_CIPHERTEXT_BYTES,
            epk_bytes: ORCHARD_EPK_BYTES,
        },
        enforcement: OrchardOperatorEnforcementReport {
            protocol_size_bounds_enforced: true,
            action_count_bound_enforced: true,
            verifier_runs_in_process: true,
            verifier_timeout_enforced_in_process: false,
            verifier_concurrency_enforced_in_process: false,
            rpc_child_timeout_available_for_remote_batch_create: true,
            remote_batch_create_requires_action_json: true,
            remote_batch_create_uses_server_controlled_spool: true,
            remote_batch_create_rate_limited: true,
            remote_batch_create_concurrency_limited: true,
            public_write_edge_allowed: false,
            requires_worker_isolation_for_public_write_edge: true,
        },
        warnings,
    })
}

pub fn orchard_fee_resource_policy(
    options: OrchardFeeResourcePolicyOptions,
) -> io::Result<OrchardFeeResourcePolicyReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let genesis_hash_hex = genesis_hash(&genesis);

    let transparent_fee_schedule = OrchardTransparentFeeScheduleReport {
        min_transfer_fee: MIN_TRANSFER_FEE,
        transfer_fee_byte_quantum: TRANSFER_FEE_BYTE_QUANTUM as u64,
        transfer_fee_per_quantum: TRANSFER_FEE_PER_QUANTUM,
        transfer_account_creation_fee: TRANSFER_ACCOUNT_CREATION_FEE,
        account_reserve: ACCOUNT_RESERVE,
    };
    let orchard_resource_fee_schedule = OrchardResourceFeeScheduleReport {
        minimum_orchard_resource_fee: ORCHARD_FEE_BURN_MIN_FEE,
        orchard_fee_byte_quantum: ORCHARD_FEE_BURN_BYTE_QUANTUM as u64,
        orchard_fee_per_quantum: ORCHARD_FEE_BURN_FEE_PER_QUANTUM,
        resource_weight_formula:
            "canonical serialized Orchard action fields plus proof, signatures, nullifiers, commitments, encrypted outputs, and external binding hash bytes"
                .to_string(),
        fee_formula:
            "max(minimum_orchard_resource_fee, ceil(action_weight_bytes / orchard_fee_byte_quantum) * orchard_fee_per_quantum)"
                .to_string(),
    };
    let flow_fee_schedule = vec![
        OrchardFlowFeeScheduleReport {
            operation: "transparent_to_orchard_deposit".to_string(),
            fee_payer: "transparent funding account".to_string(),
            minimum_fee_components: vec![
                "transparent funding transfer minimum fee".to_string(),
                "Orchard resource fee charged by deposit envelope".to_string(),
            ],
            burn_accounting: vec![
                "transparent funding transfer fee is burned by transparent transfer apply"
                    .to_string(),
                "Orchard deposit resource fee is burned by shielded deposit apply"
                    .to_string(),
                "principal enters orchard_deposit and turnstile accounting".to_string(),
            ],
            receipt_fields: vec![
                "fee_charged".to_string(),
                "fee_burned".to_string(),
                "minimum_fee".to_string(),
                "orchard_deposit_total".to_string(),
            ],
        },
        OrchardFlowFeeScheduleReport {
            operation: "orchard_spend".to_string(),
            fee_payer: "shielded input value".to_string(),
            minimum_fee_components: vec![
                "zero when verified value_balance is nonpositive".to_string(),
                "Orchard resource fee when verified value_balance is positive".to_string(),
            ],
            burn_accounting: vec![
                "positive value balance is treated as fee burn".to_string(),
                "fee burn increases orchard_fee_burn_total".to_string(),
            ],
            receipt_fields: vec![
                "fee_charged".to_string(),
                "fee_burned".to_string(),
                "minimum_fee".to_string(),
            ],
        },
        OrchardFlowFeeScheduleReport {
            operation: "orchard_to_transparent_withdraw".to_string(),
            fee_payer: "shielded input value".to_string(),
            minimum_fee_components: vec![
                "Orchard resource fee".to_string(),
                "transparent account-creation state-expansion fee when the recipient account does not exist"
                    .to_string(),
            ],
            burn_accounting: vec![
                "withdraw fee is burned by shielded withdraw apply".to_string(),
                "withdraw principal credits the transparent recipient only inside ordered shielded batch apply"
                    .to_string(),
            ],
            receipt_fields: vec![
                "fee_charged".to_string(),
                "fee_burned".to_string(),
                "minimum_fee".to_string(),
                "state_expansion_fee".to_string(),
                "orchard_withdraw_total".to_string(),
            ],
        },
    ];
    let resource_bounds = OrchardResourceBoundsReport {
        max_action_json_bytes: MAX_LOCAL_JSON_FILE_BYTES,
        max_actions_per_orchard_bundle: DEFAULT_MAX_ORCHARD_ACTIONS,
        max_proof_bytes: ORCHARD_PROOF_MAX_BYTES,
        max_ciphertext_blob_bytes: ORCHARD_CIPHERTEXT_MAX_BYTES,
        enc_ciphertext_bytes: ORCHARD_ENC_CIPHERTEXT_BYTES,
        out_ciphertext_bytes: ORCHARD_OUT_CIPHERTEXT_BYTES,
        compact_ciphertext_bytes: ORCHARD_COMPACT_CIPHERTEXT_BYTES,
        epk_bytes: ORCHARD_EPK_BYTES,
        default_root_retention_roots: DEFAULT_ORCHARD_ROOT_RETENTION,
        default_verifier_timeout_ms: DEFAULT_ORCHARD_VERIFIER_TIMEOUT_MS,
        default_verifier_max_concurrency: DEFAULT_ORCHARD_VERIFIER_MAX_CONCURRENCY,
    };
    let anti_spam_policy = OrchardAntiSpamPolicyReport {
        protocol_size_bounds_enforced: true,
        action_count_bound_enforced: true,
        minimum_fee_enforced_for_positive_value_balance: true,
        direct_deposit_resource_fee_enforced: true,
        withdraw_state_expansion_fee_enforced: true,
        public_write_edge_allowed: false,
        remote_batch_create_rate_limited: true,
        remote_batch_create_concurrency_limited: true,
        requires_worker_isolation_for_public_write_edge: true,
    };
    let checks = OrchardFeeResourcePolicyChecks {
        schema: ORCHARD_FEE_RESOURCE_POLICY_REPORT_SCHEMA
            == "postfiat-orchard-fee-resource-policy-v1",
        nonzero_minimum_orchard_fee: orchard_resource_fee_schedule.minimum_orchard_resource_fee > 0,
        nonzero_orchard_fee_quantum: orchard_resource_fee_schedule.orchard_fee_byte_quantum > 0,
        nonzero_orchard_fee_per_quantum: orchard_resource_fee_schedule.orchard_fee_per_quantum > 0,
        transparent_fee_schedule_visible: transparent_fee_schedule.min_transfer_fee > 0
            && transparent_fee_schedule.transfer_fee_byte_quantum > 0
            && transparent_fee_schedule.transfer_fee_per_quantum > 0
            && transparent_fee_schedule.account_reserve > 0,
        protocol_bounds_visible: resource_bounds.max_action_json_bytes > 0
            && resource_bounds.max_actions_per_orchard_bundle > 0
            && resource_bounds.max_proof_bytes > 0
            && resource_bounds.max_ciphertext_blob_bytes > 0,
        public_write_edge_closed: !anti_spam_policy.public_write_edge_allowed,
        positive_value_balance_fee_required: anti_spam_policy
            .minimum_fee_enforced_for_positive_value_balance,
        direct_deposit_resource_fee_required: anti_spam_policy.direct_deposit_resource_fee_enforced,
        withdraw_state_expansion_fee_required: anti_spam_policy
            .withdraw_state_expansion_fee_enforced,
    };
    let passed = checks.schema
        && checks.nonzero_minimum_orchard_fee
        && checks.nonzero_orchard_fee_quantum
        && checks.nonzero_orchard_fee_per_quantum
        && checks.transparent_fee_schedule_visible
        && checks.protocol_bounds_visible
        && checks.public_write_edge_closed
        && checks.positive_value_balance_fee_required
        && checks.direct_deposit_resource_fee_required
        && checks.withdraw_state_expansion_fee_required;

    Ok(OrchardFeeResourcePolicyReport {
        schema: ORCHARD_FEE_RESOURCE_POLICY_REPORT_SCHEMA.to_string(),
        chain_id: genesis.chain_id,
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        pool_id: ORCHARD_DEFAULT_POOL_ID.to_string(),
        transparent_fee_schedule,
        orchard_resource_fee_schedule,
        flow_fee_schedule,
        resource_bounds,
        anti_spam_policy,
        checks,
        passed,
    })
}

pub fn orchard_frontier_cache_warm(
    options: NodeOptions,
) -> io::Result<OrchardFrontierCacheWarmReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let genesis_hash_hex = genesis_hash(&genesis);
    let state_root_before = current_replicated_state_root(&store, &genesis)?;
    let mut shielded = store.read_shielded()?;

    let mut pool_initialized = false;
    let mut pool_id = ORCHARD_DEFAULT_POOL_ID.to_string();
    let mut output_count = 0;
    let mut retained_root_count = 0;
    let mut root = orchard_empty_root_hex();
    let mut cache_present_before = false;
    let mut cache_present_after = false;
    let mut cache_written = false;

    if let Some(pool) = shielded.orchard.as_mut() {
        pool_initialized = true;
        pool_id = pool.pool_id.clone();
        output_count = u64::try_from(pool.output_commitments.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Orchard output commitment count does not fit in u64",
            )
        })?;
        retained_root_count = u64::try_from(pool.root_history.len()).map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "Orchard root history count does not fit in u64",
            )
        })?;
        cache_present_before = pool.frontier_cache.is_some();
        let cache_before = pool.frontier_cache.clone();
        let cache = update_orchard_frontier_cache_for_current_outputs(pool)?;
        root = cache.root;
        cache_present_after = pool.frontier_cache.is_some();
        cache_written = pool.frontier_cache != cache_before;
    }

    if cache_written {
        store.write_shielded(&shielded)?;
    }

    let state_root_after = current_replicated_state_root(&store, &genesis)?;
    let state_root_unchanged = state_root_before == state_root_after;
    if !state_root_unchanged {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Orchard frontier cache warm changed replicated state root: before {state_root_before}, after {state_root_after}"
            ),
        ));
    }

    Ok(OrchardFrontierCacheWarmReport {
        schema: ORCHARD_FRONTIER_CACHE_WARM_REPORT_SCHEMA.to_string(),
        chain_id: genesis.chain_id,
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        data_dir: options.data_dir.display().to_string(),
        pool_initialized,
        pool_id,
        output_count,
        retained_root_count,
        root,
        cache_present_before,
        cache_present_after,
        cache_written,
        state_root_before,
        state_root_after,
        state_root_unchanged,
    })
}

pub fn orchard_pool_report(options: OrchardPoolReportOptions) -> io::Result<OrchardPoolReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let genesis_hash_hex = genesis_hash(&genesis);
    let verification = verify_shielded(NodeOptions {
        data_dir: options.data_dir,
    })?;
    let pool_initialized = !verification.orchard_pool_id.is_empty();
    let pool_id = if pool_initialized {
        verification.orchard_pool_id.clone()
    } else {
        ORCHARD_DEFAULT_POOL_ID.to_string()
    };
    let output_count = usize_to_u64_saturating(verification.orchard_output_count);
    let nullifier_count = usize_to_u64_saturating(verification.orchard_nullifier_count);
    let counters = OrchardPoolCountersReport {
        pool_initialized,
        output_count,
        nullifier_count,
        retained_root_count: usize_to_u64_saturating(verification.orchard_root_count),
        accepted_anchor_count: usize_to_u64_saturating(verification.orchard_anchor_count),
        latest_retained_root: verification.orchard_latest_root,
        turnstile_event_count: usize_to_u64_saturating(verification.turnstile_event_count),
        legacy_migration_total: verification.migration_total,
        direct_deposit_total: verification.orchard_deposit_total,
        accounted_pool_deposit_total: verification.orchard_turnstile_deposit_total,
        fee_burn_total: verification.orchard_fee_burn_total,
        withdraw_total: verification.orchard_withdraw_total,
        value_balance_total: verification.orchard_value_balance_total,
    };
    let active_note_bounds = OrchardPoolActiveNoteBoundsReport {
        exact_active_note_count_publicly_available: false,
        conservative_public_floor: output_count.saturating_sub(nullifier_count),
        public_upper_bound: output_count,
        method:
            "derived from public output and nullifier counters; exact wallet-owned note liveness is intentionally not public"
                .to_string(),
        limitation:
            "dummy actions, wallet scanning, and unlinkable nullifiers mean this is a public bound, not a user count or an audited anonymity guarantee"
                .to_string(),
    };
    let privacy_claim = OrchardPoolPrivacyClaimReport {
        safe_claim:
            "public Orchard pool telemetry for privacy-alpha accounting and conservative pool-size reporting"
                .to_string(),
        not_claimed: vec![
            "exact active user count".to_string(),
            "Zcash-equivalent anonymity set".to_string(),
            "audited production privacy".to_string(),
            "end-to-end post-quantum private value".to_string(),
        ],
        public_fields: vec![
            "pool id".to_string(),
            "output count".to_string(),
            "nullifier count".to_string(),
            "retained root count".to_string(),
            "latest retained root".to_string(),
            "turnstile deposit, fee-burn, and withdraw totals".to_string(),
        ],
        omitted_private_material: vec![
            "spending keys".to_string(),
            "viewing keys".to_string(),
            "note randomness".to_string(),
            "wallet witness paths".to_string(),
            "encrypted note payload bodies".to_string(),
        ],
    };
    let checks = OrchardPoolReportChecks {
        schema: ORCHARD_POOL_REPORT_SCHEMA == "postfiat-orchard-pool-report-v1",
        state_verified: verification.verified,
        turnstile_accounting_verified: verification.verified,
        no_private_material_fields: true,
        exact_active_note_count_not_claimed: !active_note_bounds
            .exact_active_note_count_publicly_available,
        pool_id_visible: !pool_id.is_empty(),
    };
    let passed = checks.schema
        && checks.state_verified
        && checks.turnstile_accounting_verified
        && checks.no_private_material_fields
        && checks.exact_active_note_count_not_claimed
        && checks.pool_id_visible;

    Ok(OrchardPoolReport {
        schema: ORCHARD_POOL_REPORT_SCHEMA.to_string(),
        chain_id: genesis.chain_id,
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        pool_id,
        counters,
        active_note_bounds,
        privacy_claim,
        checks,
        passed,
    })
}

fn usize_to_u64_saturating(value: usize) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

fn validate_orchard_operator_policy_options(
    options: &OrchardOperatorPolicyOptions,
) -> io::Result<()> {
    if options.max_concurrent_verifiers == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "max concurrent Orchard verifiers must be at least 1",
        ));
    }
    if options.verifier_timeout_ms == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard verifier timeout must be nonzero",
        ));
    }
    if options.root_retention_roots == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard root retention must be at least 1",
        ));
    }
    match options.indexing_role.as_str() {
        "disabled" | "local" | "public" => Ok(()),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard indexing role must be disabled, local, or public",
        )),
    }
}

pub fn orchard_wallet_keygen(
    options: OrchardWalletKeygenOptions,
) -> io::Result<OrchardWalletKeyReport> {
    ensure_output_can_be_written(
        &options.key_file,
        options.overwrite,
        "Orchard wallet key file",
    )?;
    let spending_key =
        derive_orchard_spending_key(&options.master_seed_hex, options.account_index)?;
    let address_raw_hex =
        orchard_default_address_from_spending_key(*spending_key).map_err(invalid_data)?;
    let key_file = OrchardWalletKeyFile {
        schema: ORCHARD_WALLET_FILE_SCHEMA.to_string(),
        kdf: ORCHARD_WALLET_DERIVATION_KDF.to_string(),
        derivation_domain: ORCHARD_WALLET_DERIVATION_DOMAIN.to_string(),
        account_index: options.account_index,
        spending_key_hex: bytes_to_hex(&*spending_key),
        address_raw_hex,
    };
    write_orchard_wallet_key_file(&options.key_file, &key_file)?;
    Ok(orchard_wallet_key_report(&options.key_file, &key_file))
}

pub fn orchard_view_key_export(
    options: OrchardViewKeyExportOptions,
) -> io::Result<OrchardViewKeyReport> {
    ensure_output_can_be_written(
        &options.view_key_file,
        options.overwrite,
        "Orchard view key file",
    )?;
    let key_file = read_orchard_wallet_key_file(&options.key_file)?;
    let spending_key = orchard_spending_key_bytes(&key_file.spending_key_hex)?;
    let full_viewing_key =
        orchard_full_viewing_key_from_spending_key(spending_key).map_err(invalid_data)?;
    let view_key = OrchardViewKeyFile {
        schema: ORCHARD_VIEW_KEY_FILE_SCHEMA.to_string(),
        source_schema: key_file.schema,
        account_index: key_file.account_index,
        full_viewing_key_hex: bytes_to_hex(&full_viewing_key),
        address_raw_hex: key_file.address_raw_hex,
    };
    write_orchard_view_key_file(&options.view_key_file, &view_key)?;
    Ok(orchard_view_key_report(&options.view_key_file, &view_key))
}

pub fn create_orchard_output_action(
    options: OrchardOutputActionOptions,
) -> io::Result<OrchardOutputActionReport> {
    ensure_output_can_be_written(
        &options.action_file,
        options.overwrite,
        "Orchard output action file",
    )?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let shielded = store.read_shielded()?;
    let anchor = orchard_latest_retained_anchor(&shielded, ORCHARD_DEFAULT_POOL_ID)?;
    let recipient_address_raw_hex = orchard_output_recipient_address(&options)?;
    let memo = orchard_memo_bytes(options.memo_hex.as_deref())?;
    let domain = orchard_authorizing_domain(&genesis, ORCHARD_DEFAULT_POOL_ID)?;
    let action = orchard_build_output_action(
        &domain,
        ORCHARD_DEFAULT_POOL_ID,
        options.fee,
        OrchardAnchor::parse_hex(anchor.clone()).map_err(invalid_data)?,
        &recipient_address_raw_hex,
        options.value,
        memo,
    )
    .map_err(invalid_data)?;
    let verified =
        verify_serialized_orchard_action_with_built_key(&action, &domain).map_err(invalid_data)?;
    let action_json = serde_json::to_string_pretty(&action).map_err(invalid_data)?;
    atomic_write(&options.action_file, format!("{action_json}\n"))?;

    Ok(OrchardOutputActionReport {
        schema: ORCHARD_OUTPUT_ACTION_REPORT_SCHEMA.to_string(),
        action_file: options.action_file.display().to_string(),
        pool_id: action.pool_id,
        anchor,
        recipient_address_raw_hex,
        value: options.value,
        fee: action.fee,
        action_count: verified.action_count,
        nullifier_count: verified.nullifiers.len(),
        output_count: verified.output_commitments.len(),
        verified: true,
    })
}

pub fn orchard_test_vector(
    options: OrchardTestVectorOptions,
) -> io::Result<OrchardTestVectorReport> {
    let genesis =
        Genesis::try_new_with_validator_count(options.chain_id.clone(), options.validator_count)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let genesis_hash_hex = genesis_hash(&genesis);
    let domain = orchard_authorizing_domain(&genesis, ORCHARD_DEFAULT_POOL_ID)?;
    let spending_key = fixed_32_byte_hex(
        "Orchard test-vector spending-key seed",
        &options.spending_key_seed_hex,
    )?;
    let build_seed = fixed_32_byte_hex("Orchard test-vector build seed", &options.build_seed_hex)?;
    let proof_seed = fixed_32_byte_hex("Orchard test-vector proof seed", &options.proof_seed_hex)?;
    let signature_seed = fixed_32_byte_hex(
        "Orchard test-vector signature seed",
        &options.signature_seed_hex,
    )?;
    let recipient_address_raw_hex =
        orchard_default_address_from_spending_key(spending_key).map_err(invalid_data)?;
    let anchor = orchard_empty_anchor();
    let action = orchard_build_output_action_test_vector(
        &domain,
        ORCHARD_DEFAULT_POOL_ID,
        options.fee,
        anchor.clone(),
        &recipient_address_raw_hex,
        options.value,
        [0u8; ORCHARD_MEMO_BYTES],
        options.external_binding_hash.as_deref(),
        build_seed,
        proof_seed,
        signature_seed,
    )
    .map_err(invalid_data)?;
    let rebuild = orchard_build_output_action_test_vector(
        &domain,
        ORCHARD_DEFAULT_POOL_ID,
        options.fee,
        anchor.clone(),
        &recipient_address_raw_hex,
        options.value,
        [0u8; ORCHARD_MEMO_BYTES],
        options.external_binding_hash.as_deref(),
        build_seed,
        proof_seed,
        signature_seed,
    )
    .map_err(invalid_data)?;
    let action_json = serde_json::to_vec(&action).map_err(invalid_data)?;
    let rebuild_json = serde_json::to_vec(&rebuild).map_err(invalid_data)?;
    let deterministic_rebuild_matches = action == rebuild && action_json == rebuild_json;
    let verified =
        verify_serialized_orchard_action_with_built_key(&action, &domain).map_err(invalid_data)?;
    let bundle = orchard_bundle_from_action(&action).map_err(invalid_data)?;
    let authorizing_sighash = orchard_authorizing_sighash_with_external_binding(
        &domain,
        action.fee,
        action.external_binding_hash.as_deref(),
        &bundle,
    )
    .map_err(invalid_data)?;
    let root_after_outputs =
        orchard_anchor_from_commitments(&action.output_commitments).map_err(invalid_data)?;

    let mut proof_tampered = action.clone();
    proof_tampered.proof =
        OrchardProofBytes::parse_hex(mutate_first_hex_nibble(action.proof.as_hex()))
            .map_err(invalid_data)?;
    let proof_error = verify_serialized_orchard_action_with_built_key(&proof_tampered, &domain)
        .expect_err("mutated Orchard proof must fail");

    if action.fee == u64::MAX {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard test-vector fee must be lower than u64::MAX",
        ));
    }
    let mut fee_tampered = action.clone();
    fee_tampered.fee += 1;
    let fee_error = verify_serialized_orchard_action_with_built_key(&fee_tampered, &domain)
        .expect_err("mutated Orchard fee must fail");

    Ok(OrchardTestVectorReport {
        schema: "postfiat-orchard-test-vector-v1".to_string(),
        fixture_warning: "public deterministic fixture only; do not fund or reuse these seeds"
            .to_string(),
        chain_id: genesis.chain_id,
        validator_count: genesis.validator_count,
        genesis_hash: genesis_hash_hex,
        protocol_version: genesis.protocol_version,
        pool_id: ORCHARD_DEFAULT_POOL_ID.to_string(),
        proof_system_id: action.proof_system_id.as_str().to_string(),
        circuit_id: action.circuit_id.as_str().to_string(),
        recipient_address_raw_hex,
        empty_anchor: anchor.as_hex().to_string(),
        external_binding_hash: action.external_binding_hash.clone(),
        value: options.value,
        fee: action.fee,
        action_hash: hash_hex(
            "postfiat.privacy.orchard-test-vector.action.v1",
            &action_json,
        ),
        action_json_bytes: action_json.len(),
        action_count: verified.action_count,
        nullifier_count: verified.nullifiers.len(),
        output_count: verified.output_commitments.len(),
        proof_bytes: action.proof.byte_len(),
        value_balance: verified.value_balance,
        authorizing_sighash_hex: bytes_to_hex(&authorizing_sighash),
        root_after_outputs: root_after_outputs.as_hex().to_string(),
        nullifiers: action
            .nullifiers
            .iter()
            .map(|nullifier| nullifier.as_hex().to_string())
            .collect(),
        output_commitments: action
            .output_commitments
            .iter()
            .map(|commitment| commitment.as_hex().to_string())
            .collect(),
        encrypted_outputs: action
            .encrypted_outputs
            .iter()
            .map(|output| OrchardTestVectorEncryptedOutputReport {
                epk_bytes: output.epk.byte_len(),
                enc_ciphertext_bytes: output.enc_ciphertext.byte_len(),
                out_ciphertext_bytes: output.out_ciphertext.byte_len(),
                compact_ciphertext_bytes: output
                    .compact_ciphertext
                    .as_ref()
                    .map(BoundedHexBlob::byte_len),
            })
            .collect(),
        tamper: OrchardTestVectorTamperReport {
            proof_error_code: proof_error.code().to_string(),
            fee_error_code: fee_error.code().to_string(),
        },
        deterministic_rebuild_matches,
        private_key_material_redacted: true,
    })
}

pub fn create_orchard_deposit_action(
    options: OrchardDepositActionOptions,
) -> io::Result<OrchardDepositActionReport> {
    ensure_output_can_be_written(
        &options.deposit_file,
        options.overwrite,
        "Orchard deposit action file",
    )?;
    if options.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard deposit amount must be nonzero",
        ));
    }

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let anchor = orchard_latest_retained_anchor(&shielded, ORCHARD_DEFAULT_POOL_ID)?;
    let recipient_address_raw_hex = orchard_deposit_recipient_address(&options)?;
    let memo = orchard_memo_bytes(options.memo_hex.as_deref())?;
    let (policy_id, disclosure_hash) =
        orchard_deposit_payload_values(options.policy_id.clone(), options.disclosure_hash.clone())?;

    let mut fee = options.fee;
    let mut last_minimum_fee = 0;
    for _ in 0..3 {
        let built = build_orchard_deposit_action_file(
            &genesis,
            &ledger,
            &options,
            anchor.clone(),
            recipient_address_raw_hex.clone(),
            memo,
            fee,
            &policy_id,
            &disclosure_hash,
        )?;
        last_minimum_fee = built.report.minimum_fee;
        if fee < built.report.minimum_fee {
            if options.fee != 0 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "minimum Orchard deposit fee is {}",
                        built.report.minimum_fee
                    ),
                ));
            }
            fee = built.report.minimum_fee;
            continue;
        }
        let json = serde_json::to_string_pretty(&built.file).map_err(invalid_data)?;
        atomic_write(&options.deposit_file, format!("{json}\n"))?;
        return Ok(built.report);
    }

    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        format!("minimum Orchard deposit fee did not converge; last quote was {last_minimum_fee}"),
    ))
}

struct BuiltOrchardDepositAction {
    file: OrchardDepositActionFile,
    report: OrchardDepositActionReport,
}

#[allow(clippy::too_many_arguments)]
fn build_orchard_deposit_action_file(
    genesis: &Genesis,
    ledger: &LedgerState,
    options: &OrchardDepositActionOptions,
    anchor: String,
    recipient_address_raw_hex: String,
    memo: [u8; ORCHARD_MEMO_BYTES],
    fee: u64,
    policy_id: &str,
    disclosure_hash: &str,
) -> io::Result<BuiltOrchardDepositAction> {
    let funding_amount = options.amount.checked_add(fee).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard deposit amount plus fee overflowed",
        )
    })?;
    let funding_transfer = build_signed_transfer(
        genesis,
        ledger,
        &options.data_dir,
        options.key_file.clone(),
        FEE_COLLECTOR_ADDRESS.to_string(),
        funding_amount,
    )?;
    let funding_transfer_id = transfer_tx_id(&funding_transfer);
    let external_binding_hash = orchard_deposit_external_binding_hash(
        OrchardDepositExternalBindingInput {
            genesis,
            pool_id: ORCHARD_DEFAULT_POOL_ID,
            funding_transfer_id: &funding_transfer_id,
            from_address: &funding_transfer.unsigned.from,
            amount: options.amount,
            fee,
            policy_id,
            disclosure_hash,
        },
    )?;
    let domain = orchard_authorizing_domain(genesis, ORCHARD_DEFAULT_POOL_ID)?;
    let action = orchard_build_output_action_with_external_binding(
        &domain,
        ORCHARD_DEFAULT_POOL_ID,
        0,
        OrchardAnchor::parse_hex(anchor.clone()).map_err(invalid_data)?,
        &recipient_address_raw_hex,
        options.amount,
        memo,
        Some(&external_binding_hash),
    )
    .map_err(invalid_data)?;
    let verified =
        verify_serialized_orchard_action_with_built_key(&action, &domain).map_err(invalid_data)?;
    let minimum_fee = orchard_minimum_resource_fee_for_action(&action);
    let file = OrchardDepositActionFile {
        schema: ORCHARD_DEPOSIT_ACTION_FILE_SCHEMA.to_string(),
        action,
        funding_transfer,
        amount: options.amount,
        fee,
        policy_id: policy_id.to_string(),
        disclosure_hash: disclosure_hash.to_string(),
        external_binding_hash: external_binding_hash.clone(),
    };
    let report = OrchardDepositActionReport {
        schema: ORCHARD_DEPOSIT_ACTION_REPORT_SCHEMA.to_string(),
        deposit_file: options.deposit_file.display().to_string(),
        pool_id: ORCHARD_DEFAULT_POOL_ID.to_string(),
        anchor,
        from: file.funding_transfer.unsigned.from.clone(),
        recipient_address_raw_hex,
        amount: options.amount,
        fee,
        minimum_fee,
        funding_transfer_fee: file.funding_transfer.unsigned.fee,
        funding_transfer_id,
        external_binding_hash,
        policy_id: policy_id.to_string(),
        disclosure_hash: disclosure_hash.to_string(),
        action_count: verified.action_count,
        nullifier_count: verified.nullifiers.len(),
        output_count: verified.output_commitments.len(),
        value_balance: verified.value_balance,
        verified: true,
    };
    Ok(BuiltOrchardDepositAction { file, report })
}

pub fn create_asset_orchard_ingress(
    options: AssetOrchardIngressCreateOptions,
) -> io::Result<AssetOrchardIngressReport> {
    ensure_output_can_be_written(
        &options.ingress_file,
        options.overwrite,
        "AssetOrchard ingress file",
    )?;
    ensure_output_can_be_written(
        &options.note_file,
        options.overwrite,
        "AssetOrchard wallet note file",
    )?;
    if options.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress amount must be nonzero",
        ));
    }
    if options.encrypted_output_hex.is_some() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "custom AssetOrchard encrypted outputs are disabled; recipient note encryption is mandatory",
        ));
    }

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let mempool = store.read_mempool()?;
    let key_file = read_key_file(&options.key_file)?;
    let source = key_file.address.clone();
    let source_account = ledger.account(&source).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("source account `{source}` not found"),
        )
    })?;
    let sequence = next_pending_sender_sequence(&mempool, &source, source_account.sequence)?;
    let block_height = next_block_height_from_chain_tip(&store, &genesis)?;
    let asset_execution_compatibility = asset_execution_compatibility_from_store(&store)?;
    let quote_ledger = ledger_after_executable_mempool(
        &genesis,
        ledger,
        &mempool,
        block_height,
        asset_execution_compatibility,
    )?;
    let asset = quote_ledger
        .asset_definition(&options.asset_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("asset `{}` not found", options.asset_id),
            )
        })?;
    let burn_operation = AssetTransactionOperation::AssetBurn(AssetBurnOperation {
        owner: source.clone(),
        issuer: asset.issuer.clone(),
        asset_id: options.asset_id.clone(),
        amount: options.amount,
    });
    let mut fee = options.fee;
    let (minimum_burn_fee, burn_fee) = loop {
        let quoted = quote_signed_asset_transaction(
            &genesis,
            source.clone(),
            fee,
            sequence,
            burn_operation.clone(),
        )?;
        let minimum_fee = minimum_asset_transaction_fee(&quoted)
            .saturating_add(asset_transaction_state_expansion_fee(&quote_ledger, &quoted));
        if fee >= minimum_fee {
            break (minimum_fee, fee);
        }
        if options.fee != 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("minimum AssetOrchard ingress burn fee is {minimum_fee}"),
            ));
        }
        fee = minimum_fee;
    };
    let burn_transaction = wallet_sign_asset_transaction(WalletSignAssetTransactionOptions {
        key_file: options.key_file,
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        fee: burn_fee,
        sequence,
        expected_source: Some(source.clone()),
        operation: burn_operation,
    })?;
    let burn_transaction_id = asset_transaction_tx_id(&burn_transaction);
    let genesis_hash_32 = asset_orchard_domain_genesis_hash(&genesis_hash(&genesis))
        .map_err(invalid_data)?;
    let wallet_note = build_asset_orchard_wallet_note(
        &genesis.chain_id,
        genesis_hash_32,
        genesis.protocol_version,
        &options.asset_id,
        options.amount,
        &options.note_seed_hex,
    )
    .map_err(invalid_data)?;
    let encrypted_output = bytes_to_hex(
        &encrypt_asset_orchard_wallet_note(
            &genesis.chain_id,
            genesis_hash_32,
            genesis.protocol_version,
            &wallet_note,
        )
        .map_err(invalid_data)?
        .to_bytes()
        .map_err(invalid_data)?,
    );
    let payload = AssetOrchardIngressV2ActionPayload {
        burn_transaction,
        pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
        asset_id: options.asset_id.clone(),
        amount: options.amount,
        output_commitment: wallet_note.output_commitment.as_hex().to_string(),
        encrypted_output,
    };
    validate_asset_orchard_ingress_v2_payload(&payload)?;
    let ingress_file = AssetOrchardIngressFile {
        schema: ASSET_ORCHARD_INGRESS_FILE_SCHEMA.to_string(),
        payload,
    };
    let ingress_json = serde_json::to_string_pretty(&ingress_file).map_err(invalid_data)?;
    atomic_write(&options.ingress_file, format!("{ingress_json}\n"))?;
    write_asset_orchard_wallet_note_file(&options.note_file, &wallet_note)?;
    Ok(AssetOrchardIngressReport {
        schema: ASSET_ORCHARD_INGRESS_REPORT_SCHEMA.to_string(),
        ingress_file: options.ingress_file.display().to_string(),
        note_file: options.note_file.display().to_string(),
        pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
        from: source,
        asset_id: options.asset_id,
        amount: options.amount,
        asset_tag_lo: wallet_note.note.asset_tag_lo,
        asset_tag_hi: wallet_note.note.asset_tag_hi,
        output_commitment: wallet_note.output_commitment.as_hex().to_string(),
        encrypted_output_bytes: ingress_file.payload.encrypted_output.len() / 2,
        burn_transaction_id,
        burn_fee,
        minimum_burn_fee,
        sequence,
        verified: true,
    })
}

pub fn create_asset_orchard_ingress_batch(
    options: AssetOrchardIngressBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let file = read_asset_orchard_ingress_file(&options.ingress_file)?;
    validate_asset_orchard_ingress_v2_payload(&file.payload)?;
    let batch = build_shielded_action_batch(
        &genesis,
        vec![ShieldedAction::AssetOrchardIngressV2(file.payload)],
    )?;
    write_shielded_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_asset_orchard_egress(
    options: AssetOrchardEgressCreateOptions,
) -> io::Result<AssetOrchardEgressReport> {
    ensure_output_can_be_written(
        &options.egress_file,
        options.overwrite,
        "AssetOrchard egress file",
    )?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let note: AssetOrchardWalletNote =
        read_json_file(&options.note_file, "AssetOrchard egress note")?;
    if let Some(amount) = options.amount {
        if amount != note.value {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "AssetOrchard disclosed egress is whole-note only; requested amount {amount}, note value {}",
                    note.value
                ),
            ));
        }
    }
    let genesis_hash_32 = asset_orchard_domain_genesis_hash(&genesis_hash(&genesis))
        .map_err(invalid_data)?;
    let authorization = build_asset_orchard_disclosed_egress_authorization(
        &genesis.chain_id,
        genesis_hash_32,
        genesis.protocol_version,
        &options.to,
        &note,
    )
    .map_err(invalid_data)?;
    let payload = AssetOrchardEgressActionPayload {
        pool_id: note.pool_id.clone(),
        to: options.to.clone(),
        asset_id: note.asset_id.clone(),
        amount: note.value,
        output_commitment: note.output_commitment.as_hex().to_string(),
        nullifier: authorization.nullifier.as_hex().to_string(),
        note: asset_orchard_ingress_note_from_public(&note.note),
        nk: note.nk.as_hex().to_string(),
        rivk: note.rivk.as_str().to_string(),
        spend_auth_verification_key: authorization
            .spend_auth_verification_key
            .as_hex()
            .to_string(),
        spend_auth_randomizer: authorization.spend_auth_randomizer.clone(),
        randomized_verification_key: authorization
            .randomized_verification_key
            .as_hex()
            .to_string(),
        spend_authorization_signature: authorization
            .spend_authorization_signature
            .as_hex()
            .to_string(),
    };
    validate_asset_orchard_egress_payload_for_genesis(&genesis, &payload)?;
    let file = AssetOrchardEgressFile {
        schema: ASSET_ORCHARD_EGRESS_FILE_SCHEMA.to_string(),
        payload,
    };
    let json = serde_json::to_string_pretty(&file).map_err(invalid_data)?;
    atomic_write(&options.egress_file, format!("{json}\n"))?;
    Ok(AssetOrchardEgressReport {
        schema: ASSET_ORCHARD_EGRESS_REPORT_SCHEMA.to_string(),
        egress_file: options.egress_file.display().to_string(),
        note_file: options.note_file.display().to_string(),
        pool_id: file.payload.pool_id,
        to: file.payload.to,
        asset_id: file.payload.asset_id,
        amount: file.payload.amount,
        output_commitment: file.payload.output_commitment,
        nullifier: file.payload.nullifier,
        spend_auth_verification_key: file.payload.spend_auth_verification_key,
        randomized_verification_key: file.payload.randomized_verification_key,
        sighash: authorization.sighash,
        verified: true,
        privacy: "disclosed_note_opening_whole_note_egress".to_string(),
    })
}

pub fn create_asset_orchard_egress_batch(
    options: AssetOrchardEgressBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let file = read_asset_orchard_egress_file(&options.egress_file)?;
    validate_asset_orchard_egress_payload_for_genesis(&genesis, &file.payload)?;
    let batch = build_shielded_action_batch(
        &genesis,
        vec![ShieldedAction::AssetOrchardEgressV1(file.payload)],
    )?;
    write_shielded_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

fn private_egress_timing_report_path(output_file: &Path) -> PathBuf {
    let name = output_file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("asset-orchard-private-egress");
    output_file.with_file_name(format!("{name}.timing.json"))
}

fn write_private_egress_timing_report<T: serde::Serialize>(
    path: &Path,
    timing: &T,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(timing).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub fn create_asset_orchard_private_egress(
    options: AssetOrchardPrivateEgressCreateOptions,
) -> io::Result<AssetOrchardPrivateEgressReport> {
    let total_start = std::time::Instant::now();
    let mut timing = AssetOrchardPrivateEgressCreateTimingReport::default();
    let timing_file = private_egress_timing_report_path(&options.egress_file);
    ensure_output_can_be_written(
        &options.egress_file,
        options.overwrite,
        "AssetOrchard private egress file",
    )?;

    let stage_start = std::time::Instant::now();
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let shielded = store.read_shielded()?;
    let pool = shielded
        .orchard
        .as_ref()
        .filter(|pool| pool.pool_id == ASSET_ORCHARD_POOL_ID_V1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "AssetOrchard pool is empty; ingress asset-typed notes before private egress",
            )
        })?;
    timing.store_state_load_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let note: AssetOrchardWalletNote =
        read_json_file(&options.note_file, "AssetOrchard private egress note")?;
    let asset_id = options.asset_id.unwrap_or_else(|| note.asset_id.clone());
    let amount = options.amount.unwrap_or(note.value);
    let genesis_hash_32 = asset_orchard_domain_genesis_hash(&genesis_hash(&genesis))
        .map_err(invalid_data)?;
    timing.note_read_prep_ms = node_timing_elapsed_ms(stage_start);

    reset_asset_orchard_private_egress_timings();
    let stage_start = std::time::Instant::now();
    let built = build_asset_orchard_private_egress_action(
        &genesis.chain_id,
        genesis_hash_32,
        genesis.protocol_version,
        note,
        &options.to,
        &asset_id,
        amount,
        options.fee,
        &options.policy_id,
        &options.disclosure_hash,
        &pool.output_commitments,
    )
    .map_err(invalid_data)?;
    timing.action_build_ms = node_timing_elapsed_ms(stage_start);
    timing.action_build_breakdown = take_asset_orchard_private_egress_timings();

    reset_asset_orchard_private_egress_timings();
    let stage_start = std::time::Instant::now();
    let domain = orchard_authorizing_domain(&genesis, &built.action.pool_id)?;
    verify_serialized_asset_orchard_private_egress_action(
        &built.action,
        &domain,
        &options.to,
        &asset_id,
        &options.policy_id,
        &options.disclosure_hash,
    )
    .map_err(invalid_data)?;
    timing.serialized_action_verification_ms = node_timing_elapsed_ms(stage_start);
    timing.serialized_action_verification_breakdown =
        take_asset_orchard_private_egress_timings();

    reset_asset_orchard_private_egress_timings();
    let stage_start = std::time::Instant::now();
    let payload = asset_orchard_private_egress_payload_from_action(
        &built.action,
        options.to.clone(),
        asset_id.clone(),
        options.policy_id.clone(),
        options.disclosure_hash.clone(),
    );
    validate_asset_orchard_private_egress_payload_for_genesis(&genesis, &payload)?;
    timing.payload_genesis_validation_ms = node_timing_elapsed_ms(stage_start);
    timing.payload_genesis_validation_breakdown =
        take_asset_orchard_private_egress_timings();

    let stage_start = std::time::Instant::now();
    let file = AssetOrchardPrivateEgressFile {
        schema: ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA.to_string(),
        payload,
    };
    let json = serde_json::to_string_pretty(&file).map_err(invalid_data)?;
    atomic_write(&options.egress_file, format!("{json}\n"))?;
    let report = AssetOrchardPrivateEgressReport {
        schema: ASSET_ORCHARD_PRIVATE_EGRESS_REPORT_SCHEMA.to_string(),
        egress_file: options.egress_file.display().to_string(),
        note_file: options.note_file.display().to_string(),
        pool_id: file.payload.pool_id,
        to: file.payload.to,
        asset_id: file.payload.asset_id,
        amount: file.payload.amount,
        fee: file.payload.fee,
        policy_id: file.payload.policy_id,
        disclosure_hash: file.payload.disclosure_hash,
        anchor: file.payload.anchor,
        nullifier: file.payload.nullifier,
        randomized_verification_key: file.payload.randomized_verification_key,
        exit_binding_hash: file.payload.exit_binding_hash,
        proof_bytes: built.action.proof.byte_len(),
        verified: true,
        privacy: "private_note_opening_egress_proof_public_exit".to_string(),
    };
    timing.file_write_report_ms = node_timing_elapsed_ms(stage_start);
    timing.total_ms = node_timing_elapsed_ms(total_start);
    write_private_egress_timing_report(&timing_file, &timing)?;
    Ok(report)
}

pub fn create_asset_orchard_private_egress_batch(
    options: AssetOrchardPrivateEgressBatchOptions,
) -> io::Result<ShieldedActionBatch> {
    let total_start = std::time::Instant::now();
    let mut timing = AssetOrchardPrivateEgressBatchTimingReport::default();
    let timing_file = private_egress_timing_report_path(&options.batch_file);

    let stage_start = std::time::Instant::now();
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    timing.store_genesis_load_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let file = read_asset_orchard_private_egress_file(&options.egress_file)?;
    timing.egress_file_read_ms = node_timing_elapsed_ms(stage_start);

    reset_asset_orchard_private_egress_timings();
    let stage_start = std::time::Instant::now();
    validate_asset_orchard_private_egress_payload_for_genesis(&genesis, &file.payload)?;
    timing.payload_genesis_validation_ms = node_timing_elapsed_ms(stage_start);
    timing.payload_genesis_validation_breakdown =
        take_asset_orchard_private_egress_timings();

    let stage_start = std::time::Instant::now();
    let batch = build_shielded_action_batch(
        &genesis,
        vec![ShieldedAction::AssetOrchardPrivateEgressV1(file.payload)],
    )?;
    timing.batch_build_ms = node_timing_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    write_shielded_action_batch_file(&options.batch_file, &batch)?;
    timing.batch_write_ms = node_timing_elapsed_ms(stage_start);
    timing.total_ms = node_timing_elapsed_ms(total_start);
    write_private_egress_timing_report(&timing_file, &timing)?;
    Ok(batch)
}

pub fn asset_orchard_note_status(
    options: AssetOrchardNoteStatusOptions,
) -> io::Result<AssetOrchardNoteStatusReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let shielded = store.read_shielded()?;
    let pool = shielded
        .orchard
        .as_ref()
        .filter(|pool| pool.pool_id == ASSET_ORCHARD_POOL_ID_V1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "AssetOrchard pool is empty; ingress asset-typed notes before checking note status",
            )
        })?;
    let note: AssetOrchardWalletNote =
        read_json_file(&options.note_file, "AssetOrchard wallet note")?;
    let genesis_hash_32 = asset_orchard_domain_genesis_hash(&genesis_hash(&genesis))
        .map_err(invalid_data)?;
    let nullifier = asset_orchard_wallet_note_nullifier(
        &genesis.chain_id,
        genesis_hash_32,
        genesis.protocol_version,
        &note,
    )
    .map_err(invalid_data)?;
    let output_commitment = note.output_commitment.as_hex().to_string();
    let nullifier_hex = nullifier.as_hex().to_string();
    let pool_output = pool
        .output_commitments
        .iter()
        .any(|commitment| commitment == &output_commitment);
    let spent = pool.is_nullified(&nullifier_hex);

    Ok(AssetOrchardNoteStatusReport {
        schema: ASSET_ORCHARD_NOTE_STATUS_REPORT_SCHEMA.to_string(),
        note_file: options.note_file.display().to_string(),
        pool_id: note.pool_id,
        asset_id: note.asset_id,
        amount: note.value,
        output_commitment,
        nullifier: nullifier_hex,
        pool_output,
        spent,
        spendable: pool_output && !spent,
    })
}

pub fn asset_orchard_scan(
    options: AssetOrchardScanOptions,
) -> io::Result<AssetOrchardScanReport> {
    ensure_output_can_be_written(
        &options.note_file,
        options.overwrite,
        "AssetOrchard recovered note file",
    )?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let shielded = store.read_shielded()?;
    let pool = shielded
        .orchard
        .as_ref()
        .filter(|pool| pool.pool_id == ASSET_ORCHARD_POOL_ID_V1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "AssetOrchard pool is empty; no encrypted outputs are available to scan",
            )
        })?;
    let genesis_hash_32 = asset_orchard_domain_genesis_hash(&genesis_hash(&genesis))
        .map_err(invalid_data)?;
    let mut encrypted_output_v1_count = 0usize;
    let mut legacy_output_count = 0usize;
    let mut nonmatching_output_count = 0usize;
    let mut recovered = None;
    for record in &pool.asset_orchard_outputs {
        let encrypted_output = hex_to_bytes(&record.encrypted_output).map_err(invalid_data)?;
        if !encrypted_output.starts_with(ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC) {
            legacy_output_count += 1;
            continue;
        }
        encrypted_output_v1_count += 1;
        match decrypt_asset_orchard_wallet_note(
            &genesis.chain_id,
            genesis_hash_32,
            genesis.protocol_version,
            &options.note_seed_hex,
            &record.output_commitment,
            &encrypted_output,
        )
        .map_err(invalid_data)?
        {
            Some(note) if recovered.is_none() => recovered = Some(note),
            Some(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "AssetOrchard scan key matched multiple chain outputs; use a distinct per-note seed",
                ));
            }
            None => nonmatching_output_count += 1,
        }
    }
    let note = recovered.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            "AssetOrchard scan found no chain output for this note seed",
        )
    })?;
    write_asset_orchard_wallet_note_file(&options.note_file, &note)?;
    Ok(AssetOrchardScanReport {
        schema: ASSET_ORCHARD_SCAN_REPORT_SCHEMA.to_string(),
        note_file: options.note_file.display().to_string(),
        pool_id: pool.pool_id.clone(),
        chain_output_count: pool.asset_orchard_outputs.len(),
        encrypted_output_v1_count,
        legacy_output_count,
        nonmatching_output_count,
        output_commitment: note.output_commitment.as_hex().to_string(),
        recovered: true,
    })
}

pub fn create_asset_orchard_swap_action(
    options: AssetOrchardSwapCreateOptions,
) -> io::Result<AssetOrchardSwapCreateReport> {
    let (report, _) = create_asset_orchard_swap_action_verified(options)?;
    Ok(report)
}

pub fn create_asset_orchard_swap_action_verified(
    options: AssetOrchardSwapCreateOptions,
) -> io::Result<(AssetOrchardSwapCreateReport, AssetOrchardSwapAction)> {
    ensure_output_can_be_written(
        &options.action_file,
        options.overwrite,
        "AssetOrchard swap action file",
    )?;
    for output_note_file in &options.output_note_files {
        ensure_output_can_be_written(
            output_note_file,
            options.overwrite,
            "AssetOrchard swap output note file",
        )?;
    }

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let shielded = store.read_shielded()?;
    let pool = shielded
        .orchard
        .as_ref()
        .filter(|pool| pool.pool_id == ASSET_ORCHARD_POOL_ID_V1)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "AssetOrchard pool is empty; ingress asset-typed notes before swapping",
            )
        })?;
    let input_notes = [
        read_json_file::<AssetOrchardWalletNote>(
            &options.input_note_files[0],
            "AssetOrchard input note 0",
        )?,
        read_json_file::<AssetOrchardWalletNote>(
            &options.input_note_files[1],
            "AssetOrchard input note 1",
        )?,
    ];
    let genesis_hash_32 = asset_orchard_domain_genesis_hash(&genesis_hash(&genesis))
        .map_err(invalid_data)?;
    let built = build_asset_orchard_swap_action(
        &genesis.chain_id,
        genesis_hash_32,
        genesis.protocol_version,
        input_notes,
        options.output_note_seed_hexes,
        &pool.output_commitments,
        read_json_file::<postfiat_privacy_orchard::AssetOrchardPricingClaim>(
            &options.pricing_claim_file,
            "AssetOrchard pricing claim",
        )?,
    )
    .map_err(invalid_data)?;
    let domain = orchard_authorizing_domain(&genesis, &built.action.pool_id)?;
    verify_serialized_asset_orchard_swap_action(&built.action, &domain).map_err(invalid_data)?;

    let action_json = serde_json::to_string_pretty(&built.action).map_err(invalid_data)?;
    atomic_write(&options.action_file, format!("{action_json}\n"))?;
    for (path, note) in options
        .output_note_files
        .iter()
        .zip(built.output_notes.iter())
    {
        write_asset_orchard_wallet_note_file(path, note)?;
    }
    let report = AssetOrchardSwapCreateReport {
        schema: ASSET_ORCHARD_SWAP_CREATE_REPORT_SCHEMA.to_string(),
        action_file: options.action_file.display().to_string(),
        output_note_files: options
            .output_note_files
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .try_into()
            .expect("fixed output note file count"),
        pool_id: built.action.pool_id.clone(),
        anchor: built.anchor.as_hex().to_string(),
        nullifiers: built
            .action
            .nullifiers
            .iter()
            .map(|nullifier| nullifier.as_hex().to_string())
            .collect(),
        output_commitments: built
            .action
            .output_commitments
            .iter()
            .map(|commitment| commitment.as_hex().to_string())
            .collect(),
        proof_bytes: built.action.proof.byte_len(),
        verified: true,
    };
    Ok((report, built.action))
}

fn write_asset_orchard_wallet_note_file(
    path: &Path,
    note: &AssetOrchardWalletNote,
) -> io::Result<()> {
    let mut note_json = Zeroizing::new(serde_json::to_string_pretty(note).map_err(invalid_data)?);
    note_json.push('\n');
    atomic_write_private_asset_orchard_note(path, note_json.as_bytes())
}

fn atomic_write_private_asset_orchard_note(
    path: &Path,
    contents: impl AsRef<[u8]>,
) -> io::Result<()> {
    ensure_private_asset_orchard_note_parent(path)?;
    atomic_write(path, contents)?;
    set_private_file_permissions(path)
}

#[cfg(unix)]
fn ensure_private_asset_orchard_note_parent(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "AssetOrchard wallet note file must be inside an explicit private directory",
            )
        })?;
    if parent == Path::new("/") || parent == Path::new("/tmp") || parent == Path::new("/var/tmp") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "AssetOrchard wallet note parent `{}` is a shared directory; choose a private note directory",
                parent.display()
            ),
        ));
    }
    std::fs::create_dir_all(parent)?;
    let mut permissions = std::fs::metadata(parent)?.permissions();
    permissions.set_mode(0o700);
    std::fs::set_permissions(parent, permissions)
}

#[cfg(not(unix))]
fn ensure_private_asset_orchard_note_parent(path: &Path) -> io::Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "AssetOrchard wallet note file must be inside an explicit private directory",
            )
        })?;
    std::fs::create_dir_all(parent)
}

fn read_asset_orchard_ingress_file(path: &Path) -> io::Result<AssetOrchardIngressFile> {
    let file: AssetOrchardIngressFile = read_json_file(path, "AssetOrchard ingress")?;
    if file.schema != ASSET_ORCHARD_INGRESS_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "AssetOrchard ingress file schema mismatch",
        ));
    }
    Ok(file)
}

fn read_asset_orchard_egress_file(path: &Path) -> io::Result<AssetOrchardEgressFile> {
    let file: AssetOrchardEgressFile = read_json_file(path, "AssetOrchard egress")?;
    if file.schema != ASSET_ORCHARD_EGRESS_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "AssetOrchard egress file schema mismatch",
        ));
    }
    Ok(file)
}

fn read_asset_orchard_private_egress_file(
    path: &Path,
) -> io::Result<AssetOrchardPrivateEgressFile> {
    let file: AssetOrchardPrivateEgressFile =
        read_json_file(path, "AssetOrchard private egress")?;
    if file.schema != ASSET_ORCHARD_PRIVATE_EGRESS_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "AssetOrchard private egress file schema mismatch",
        ));
    }
    Ok(file)
}

fn asset_orchard_ingress_note_from_public(
    note: &AssetOrchardPublicNoteOpening,
) -> AssetOrchardIngressNote {
    AssetOrchardIngressNote {
        diversifier: note.diversifier.clone(),
        g_d: note.g_d.as_hex().to_string(),
        pk_d: note.pk_d.as_hex().to_string(),
        asset_tag_lo: note.asset_tag_lo,
        asset_tag_hi: note.asset_tag_hi,
        value: note.value,
        rho: note.rho.as_hex().to_string(),
        psi: note.psi.as_hex().to_string(),
        rcm: note.rcm.clone(),
    }
}

fn asset_orchard_private_egress_payload_from_action(
    action: &AssetOrchardPrivateEgressAction,
    to: String,
    asset_id: String,
    policy_id: String,
    disclosure_hash: String,
) -> AssetOrchardPrivateEgressActionPayload {
    AssetOrchardPrivateEgressActionPayload {
        version: action.version,
        schema: action.schema.clone(),
        pool_id: action.pool_id.clone(),
        to,
        asset_id,
        amount: action.amount,
        fee: action.fee,
        policy_id,
        disclosure_hash,
        proof_system_id: action.proof_system_id.clone(),
        circuit_id: action.circuit_id.clone(),
        pool_domain: action.pool_domain.as_hex().to_string(),
        anchor: action.anchor.as_hex().to_string(),
        nullifier: action.nullifier.as_hex().to_string(),
        randomized_verification_key: action.randomized_verification_key.as_hex().to_string(),
        asset_tag_lo: action.asset_tag_lo,
        asset_tag_hi: action.asset_tag_hi,
        exit_binding_hash: action.exit_binding_hash.as_hex().to_string(),
        proof: action.proof.as_hex().to_string(),
        spend_authorization_signature: action.spend_authorization_signature.as_hex().to_string(),
    }
}

fn asset_orchard_private_egress_action_from_payload(
    payload: &AssetOrchardPrivateEgressActionPayload,
) -> io::Result<AssetOrchardPrivateEgressAction> {
    Ok(AssetOrchardPrivateEgressAction {
        version: payload.version,
        schema: payload.schema.clone(),
        pool_id: payload.pool_id.clone(),
        proof_system_id: payload.proof_system_id.clone(),
        circuit_id: payload.circuit_id.clone(),
        pool_domain: AssetOrchardFieldElement::parse_hex(payload.pool_domain.clone())
            .map_err(invalid_data)?,
        anchor: AssetOrchardFieldElement::parse_hex(payload.anchor.clone())
            .map_err(invalid_data)?,
        nullifier: AssetOrchardFieldElement::parse_hex(payload.nullifier.clone())
            .map_err(invalid_data)?,
        randomized_verification_key: AssetOrchardPoint::parse_hex(
            payload.randomized_verification_key.clone(),
        )
        .map_err(invalid_data)?,
        asset_tag_lo: payload.asset_tag_lo,
        asset_tag_hi: payload.asset_tag_hi,
        amount: payload.amount,
        fee: payload.fee,
        exit_binding_hash: AssetOrchardSwapBindingHash::parse_hex(
            payload.exit_binding_hash.clone(),
        )
        .map_err(invalid_data)?,
        proof: AssetOrchardProofBytes::parse_hex(payload.proof.clone()).map_err(invalid_data)?,
        spend_authorization_signature: AssetOrchardSpendAuthSignature::parse_hex(
            payload.spend_authorization_signature.clone(),
        )
        .map_err(invalid_data)?,
    })
}

pub fn create_orchard_spend_action(
    options: OrchardSpendActionOptions,
) -> io::Result<OrchardSpendActionReport> {
    ensure_output_can_be_written(
        &options.action_file,
        options.overwrite,
        "Orchard spend action file",
    )?;

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let shielded = store.read_shielded()?;
    let Some(pool) = shielded.orchard.as_ref() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard pool is empty; no note can be spent",
        ));
    };
    if pool.pool_id != ORCHARD_DEFAULT_POOL_ID {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Orchard pool id `{}` does not match expected pool `{}`",
                pool.pool_id, ORCHARD_DEFAULT_POOL_ID
            ),
        ));
    }
    verify_orchard_pool_state(pool)?;

    let spending_key = orchard_spend_action_spending_key(&options)?;
    let nullifiers = pool
        .nullifiers
        .iter()
        .map(|nullifier| OrchardNullifier::parse_hex(nullifier).map_err(invalid_data))
        .collect::<io::Result<Vec<_>>>()?;
    let encrypted_outputs = pool
        .encrypted_outputs
        .iter()
        .map(orchard_encrypted_output_from_record)
        .collect::<io::Result<Vec<_>>>()?;
    let decrypted = orchard_scan_encrypted_outputs_with_spending_key(
        spending_key,
        &nullifiers,
        &encrypted_outputs,
    )
    .map_err(invalid_data)?;
    let input = decrypted
        .into_iter()
        .find(|output| output.output_index == options.input_output_index)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Orchard spending key cannot spend output index {}",
                    options.input_output_index
                ),
            )
        })?;
    if pool.is_nullified(&input.nullifier) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Orchard output index {} is already spent by nullifier {}",
                input.output_index, input.nullifier
            ),
        ));
    }
    let input_nullifier = input.nullifier.clone();

    let commitments = orchard_pool_commitments(pool)?;
    let witness = orchard_merkle_witness_from_commitments(&commitments, input.output_index)
        .map_err(invalid_data)?;
    let recipient_address_raw_hex = orchard_spend_recipient_address(&options)?;
    let spendable_value = input.value.checked_sub(options.fee).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Orchard spend fee {} exceeds input value {}",
                options.fee, input.value
            ),
        )
    })?;
    if matches!(options.amount, Some(0)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--amount must be nonzero when provided",
        ));
    }
    let recipient_value = options.amount.unwrap_or(spendable_value);
    if recipient_value > spendable_value {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Orchard spend amount {recipient_value} exceeds input value {} minus fee {}",
                input.value, options.fee
            ),
        ));
    }
    let change_value = spendable_value - recipient_value;
    let change_address_raw_hex = if change_value > 0 {
        orchard_spend_change_address(&options, spending_key)?
    } else {
        ensure_orchard_change_address_not_requested(&options)?;
        String::new()
    };
    let memo = orchard_memo_bytes(options.memo_hex.as_deref())?;
    let domain = orchard_authorizing_domain(&genesis, ORCHARD_DEFAULT_POOL_ID)?;
    let spend_note = OrchardSpendNote {
        output_index: input.output_index,
        commitment: input.commitment.clone(),
        address_raw_hex: input.address_raw_hex,
        value: input.value,
        rho: input.rho,
        rseed: input.rseed,
        merkle_position: witness.position,
        witness_anchor: witness.anchor.clone(),
        witness_auth_path: witness.auth_path,
    };
    let action = orchard_build_spend_action(
        &domain,
        ORCHARD_DEFAULT_POOL_ID,
        options.fee,
        OrchardAnchor::parse_hex(witness.anchor).map_err(invalid_data)?,
        spending_key,
        &spend_note,
        &recipient_address_raw_hex,
        recipient_value,
        (change_value > 0).then_some(change_address_raw_hex.as_str()),
        memo,
    )
    .map_err(invalid_data)?;
    let verified =
        verify_serialized_orchard_action_with_built_key(&action, &domain).map_err(invalid_data)?;
    let minimum_fee = orchard_minimum_fee_for_action(&action, &verified);
    if !verified
        .nullifiers
        .iter()
        .any(|nullifier| nullifier.as_hex() == input_nullifier)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard spend action did not reveal the selected input nullifier",
        ));
    }
    let action_json = serde_json::to_string_pretty(&action).map_err(invalid_data)?;
    atomic_write(&options.action_file, format!("{action_json}\n"))?;

    Ok(OrchardSpendActionReport {
        schema: ORCHARD_SPEND_ACTION_REPORT_SCHEMA.to_string(),
        action_file: options.action_file.display().to_string(),
        pool_id: action.pool_id,
        anchor: verified.anchor.as_hex().to_string(),
        input_output_index: spend_note.output_index,
        input_nullifier,
        recipient_address_raw_hex,
        input_value: spend_note.value,
        output_value: spendable_value,
        recipient_value,
        change_value,
        change_address_raw_hex,
        fee: action.fee,
        minimum_fee,
        action_count: verified.action_count,
        nullifier_count: verified.nullifiers.len(),
        output_count: verified.output_commitments.len(),
        value_balance: verified.value_balance,
        verified: true,
    })
}

pub fn create_orchard_withdraw_action(
    options: OrchardWithdrawActionOptions,
) -> io::Result<OrchardWithdrawActionReport> {
    ensure_output_can_be_written(
        &options.action_file,
        options.overwrite,
        "Orchard withdraw action file",
    )?;
    validate_orchard_withdraw_recipient(&options.to)?;
    if options.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--amount must be nonzero",
        ));
    }

    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let Some(pool) = shielded.orchard.as_ref() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard pool is empty; no note can be withdrawn",
        ));
    };
    if pool.pool_id != ORCHARD_DEFAULT_POOL_ID {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Orchard pool id `{}` does not match expected pool `{}`",
                pool.pool_id, ORCHARD_DEFAULT_POOL_ID
            ),
        ));
    }
    verify_orchard_pool_state(pool)?;

    let spending_key = orchard_withdraw_action_spending_key(&options)?;
    let nullifiers = pool
        .nullifiers
        .iter()
        .map(|nullifier| OrchardNullifier::parse_hex(nullifier).map_err(invalid_data))
        .collect::<io::Result<Vec<_>>>()?;
    let encrypted_outputs = pool
        .encrypted_outputs
        .iter()
        .map(orchard_encrypted_output_from_record)
        .collect::<io::Result<Vec<_>>>()?;
    let decrypted = orchard_scan_encrypted_outputs_with_spending_key(
        spending_key,
        &nullifiers,
        &encrypted_outputs,
    )
    .map_err(invalid_data)?;
    let input = decrypted
        .into_iter()
        .find(|output| output.output_index == options.input_output_index)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "Orchard spending key cannot withdraw output index {}",
                    options.input_output_index
                ),
            )
        })?;
    if pool.is_nullified(&input.nullifier) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Orchard output index {} is already spent by nullifier {}",
                input.output_index, input.nullifier
            ),
        ));
    }
    let input_nullifier = input.nullifier.clone();
    let exit_value = options.amount.checked_add(options.fee).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard withdraw amount plus fee overflowed",
        )
    })?;
    let change_value = input.value.checked_sub(exit_value).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "Orchard withdraw amount {} plus fee {} exceeds input value {}",
                options.amount, options.fee, input.value
            ),
        )
    })?;

    let commitments = orchard_pool_commitments(pool)?;
    let witness = orchard_merkle_witness_from_commitments(&commitments, input.output_index)
        .map_err(invalid_data)?;
    let change_address_raw_hex = if change_value > 0 {
        orchard_withdraw_change_address(&options, spending_key)?
    } else {
        ensure_orchard_withdraw_change_address_not_requested(&options)?;
        String::new()
    };
    let memo = orchard_memo_bytes(options.memo_hex.as_deref())?;
    let (policy_id, disclosure_hash) =
        orchard_withdraw_payload_values(options.policy_id, options.disclosure_hash)?;
    let external_binding_hash = orchard_withdraw_external_binding_hash(
        &genesis,
        ORCHARD_DEFAULT_POOL_ID,
        &options.to,
        options.amount,
        options.fee,
        &policy_id,
        &disclosure_hash,
    )?;
    let domain = orchard_authorizing_domain(&genesis, ORCHARD_DEFAULT_POOL_ID)?;
    let spend_note = OrchardSpendNote {
        output_index: input.output_index,
        commitment: input.commitment.clone(),
        address_raw_hex: input.address_raw_hex,
        value: input.value,
        rho: input.rho,
        rseed: input.rseed,
        merkle_position: witness.position,
        witness_anchor: witness.anchor.clone(),
        witness_auth_path: witness.auth_path,
    };
    let action = orchard_build_withdraw_action(
        &domain,
        ORCHARD_DEFAULT_POOL_ID,
        &external_binding_hash,
        options.fee,
        OrchardAnchor::parse_hex(witness.anchor).map_err(invalid_data)?,
        spending_key,
        &spend_note,
        options.amount,
        (change_value > 0).then_some(change_address_raw_hex.as_str()),
        memo,
    )
    .map_err(invalid_data)?;
    let verified =
        verify_serialized_orchard_action_with_built_key(&action, &domain).map_err(invalid_data)?;
    let state_expansion_fee = orchard_withdraw_state_expansion_fee(&ledger, &options.to);
    let minimum_fee =
        orchard_minimum_fee_for_action(&action, &verified).saturating_add(state_expansion_fee);
    if options.fee < minimum_fee {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("minimum Orchard withdraw fee is {minimum_fee}"),
        ));
    }
    if !verified
        .nullifiers
        .iter()
        .any(|nullifier| nullifier.as_hex() == input_nullifier)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard withdraw action did not reveal the selected input nullifier",
        ));
    }
    let action_json = serde_json::to_string_pretty(&action).map_err(invalid_data)?;
    atomic_write(&options.action_file, format!("{action_json}\n"))?;

    Ok(OrchardWithdrawActionReport {
        schema: ORCHARD_WITHDRAW_ACTION_REPORT_SCHEMA.to_string(),
        action_file: options.action_file.display().to_string(),
        pool_id: action.pool_id,
        anchor: verified.anchor.as_hex().to_string(),
        input_output_index: spend_note.output_index,
        input_nullifier,
        input_value: spend_note.value,
        withdraw_amount: options.amount,
        change_value,
        change_address_raw_hex,
        to: options.to,
        fee: action.fee,
        minimum_fee,
        state_expansion_fee,
        external_binding_hash,
        policy_id,
        disclosure_hash,
        action_count: verified.action_count,
        nullifier_count: verified.nullifiers.len(),
        output_count: verified.output_commitments.len(),
        value_balance: verified.value_balance,
        verified: true,
    })
}

pub fn orchard_wallet_scan(
    options: OrchardWalletScanOptions,
) -> io::Result<OrchardWalletScanReport> {
    let store = NodeStore::new(&options.data_dir);
    let shielded = store.read_shielded()?;
    let scan_key = orchard_scan_key(&options)?;
    let address_raw_hex = scan_key.address_raw_hex()?;
    let Some(pool) = shielded.orchard.as_ref() else {
        return Ok(OrchardWalletScanReport {
            schema: "postfiat-orchard-wallet-scan-v1".to_string(),
            pool_id: String::new(),
            address_raw_hex,
            latest_retained_root: orchard_empty_root_hex(),
            latest_retained_output_count: 0,
            output_count: 0,
            decrypted_count: 0,
            spent_count: 0,
            outputs: Vec::new(),
        });
    };

    verify_orchard_pool_state(pool)?;
    if pool.encrypted_outputs.len() > pool.output_commitments.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard encrypted output count exceeds output commitment count",
        ));
    }
    let commitments = orchard_pool_commitments(pool)?;
    let (latest_retained_root, latest_retained_output_count) = pool
        .root_history
        .last()
        .map(|record| (record.root.clone(), record.output_count))
        .unwrap_or_else(|| (orchard_empty_root_hex(), 0));
    let nullifiers = pool
        .nullifiers
        .iter()
        .map(|nullifier| OrchardNullifier::parse_hex(nullifier).map_err(invalid_data))
        .collect::<io::Result<Vec<_>>>()?;
    let outputs = pool
        .encrypted_outputs
        .iter()
        .map(orchard_encrypted_output_from_record)
        .collect::<io::Result<Vec<_>>>()?;
    let decrypted = scan_key.scan(&nullifiers, &outputs)?;
    let outputs = decrypted
        .into_iter()
        .map(|output| orchard_wallet_decrypted_output(pool, &commitments, output))
        .collect::<io::Result<Vec<_>>>()?;
    let spent_count = outputs.iter().filter(|output| output.spent).count();

    Ok(OrchardWalletScanReport {
        schema: "postfiat-orchard-wallet-scan-v1".to_string(),
        pool_id: pool.pool_id.clone(),
        address_raw_hex,
        latest_retained_root,
        latest_retained_output_count,
        output_count: pool.encrypted_outputs.len(),
        decrypted_count: outputs.len(),
        spent_count,
        outputs,
    })
}

pub fn orchard_disclosure_packet(
    options: OrchardDisclosureOptions,
) -> io::Result<OrchardDisclosurePacket> {
    ensure_output_can_be_written(
        &options.packet_file,
        options.overwrite,
        "Orchard disclosure packet",
    )?;
    let scan = orchard_wallet_scan(OrchardWalletScanOptions {
        data_dir: options.data_dir.clone(),
        spending_key_hex: options.spending_key_hex,
        key_file: options.key_file,
        view_key_file: options.view_key_file,
    })?;
    let output = scan
        .outputs
        .iter()
        .find(|output| output.output_index == options.output_index)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "Orchard output index {} was not decrypted by the provided scan key",
                    options.output_index
                ),
            )
        })?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let finality = find_orchard_disclosure_finality(&store, &output.commitment)?;
    let mut packet = OrchardDisclosurePacket {
        schema: ORCHARD_DISCLOSURE_PACKET_SCHEMA.to_string(),
        disclosure_hash: String::new(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        pool_id: scan.pool_id.clone(),
        address_raw_hex: output.address_raw_hex.clone(),
        output_index: output.output_index,
        merkle_position: output.merkle_position,
        commitment: output.commitment.clone(),
        nullifier: output.nullifier.clone(),
        value: output.value,
        spent: output.spent,
        memo_hex: output.memo_hex.clone(),
        witness_anchor: output.witness_anchor.clone(),
        witness_output_count: output.witness_output_count,
        latest_retained_root: scan.latest_retained_root.clone(),
        latest_retained_output_count: scan.latest_retained_output_count,
        finality,
        private_witness_redacted: true,
        auditor_instructions: vec![
            "Verify chain_id, genesis_hash, and protocol_version against the target PostFiat network.".to_string(),
            "Verify finality.block_hash and receipt_ids against the node block log or an independently archived history window.".to_string(),
            "Verify commitment inclusion by matching the disclosed commitment to the archived shielded batch payload hash.".to_string(),
            "This packet intentionally omits private key material, note seed material, and Merkle auth paths.".to_string(),
        ],
    };
    packet.disclosure_hash = orchard_disclosure_packet_hash(&packet)?;
    let json = serde_json::to_string_pretty(&packet).map_err(invalid_data)?;
    atomic_write(&options.packet_file, format!("{json}\n"))?;
    Ok(packet)
}

pub fn orchard_disclosure_verify(
    options: OrchardDisclosureVerifyOptions,
) -> io::Result<OrchardDisclosureVerifyReport> {
    let packet: OrchardDisclosurePacket =
        read_json_file(&options.packet_file, "Orchard disclosure packet")?;
    validate_orchard_disclosure_packet(&packet)?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    if packet.chain_id != genesis.chain_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "disclosure packet chain id `{}` does not match local genesis `{}`",
                packet.chain_id, genesis.chain_id
            ),
        ));
    }
    let local_genesis_hash = genesis_hash(&genesis);
    if packet.genesis_hash != local_genesis_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "disclosure packet genesis hash `{}` does not match local genesis `{}`",
                packet.genesis_hash, local_genesis_hash
            ),
        ));
    }
    if packet.protocol_version != genesis.protocol_version {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "disclosure packet protocol version {} does not match local genesis {}",
                packet.protocol_version, genesis.protocol_version
            ),
        ));
    }
    let finality_verified = verify_orchard_disclosure_finality(&store, &packet)?;
    Ok(OrchardDisclosureVerifyReport {
        schema: ORCHARD_DISCLOSURE_VERIFY_REPORT_SCHEMA.to_string(),
        packet_file: options.packet_file.display().to_string(),
        disclosure_hash: packet.disclosure_hash,
        chain_id: genesis.chain_id,
        pool_id: packet.pool_id,
        output_index: packet.output_index,
        commitment: packet.commitment,
        nullifier: packet.nullifier,
        packet_hash_verified: true,
        local_context_verified: true,
        finality_verified,
        verified: true,
    })
}

fn validate_orchard_disclosure_packet(packet: &OrchardDisclosurePacket) -> io::Result<()> {
    if packet.schema != ORCHARD_DISCLOSURE_PACKET_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported Orchard disclosure packet schema `{}`",
                packet.schema
            ),
        ));
    }
    if packet.disclosure_hash != orchard_disclosure_packet_hash(packet)? {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard disclosure packet hash mismatch",
        ));
    }
    if !packet.private_witness_redacted {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard disclosure packet must redact private witness material",
        ));
    }
    OrchardOutputCommitment::parse_hex(&packet.commitment).map_err(invalid_data)?;
    OrchardNullifier::parse_hex(&packet.nullifier).map_err(invalid_data)?;
    OrchardAnchor::parse_hex(&packet.witness_anchor).map_err(invalid_data)?;
    OrchardAnchor::parse_hex(&packet.latest_retained_root).map_err(invalid_data)?;
    let address = hex_to_bytes(&packet.address_raw_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("disclosure packet address hex is invalid: {error}"),
        )
    })?;
    if address.len() != ORCHARD_RAW_ADDRESS_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "disclosure packet address has {} bytes, expected {ORCHARD_RAW_ADDRESS_BYTES}",
                address.len()
            ),
        ));
    }
    hex_to_bytes(&packet.memo_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("disclosure packet memo hex is invalid: {error}"),
        )
    })?;
    if packet.auditor_instructions.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard disclosure packet auditor instructions must be nonempty",
        ));
    }
    if let Some(finality) = &packet.finality {
        if finality.batch_kind != BATCH_KIND_SHIELDED {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Orchard disclosure finality batch kind `{}` is not shielded",
                    finality.batch_kind
                ),
            ));
        }
        if finality.batch_id.is_empty()
            || finality.batch_payload_hash.is_empty()
            || finality.block_hash.is_empty()
            || finality.state_root.is_empty()
            || finality.certificate_id.is_empty()
            || finality.receipt_ids.is_empty()
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Orchard disclosure finality has empty required fields",
            ));
        }
    }
    Ok(())
}

fn orchard_disclosure_packet_hash(packet: &OrchardDisclosurePacket) -> io::Result<String> {
    let mut hashable = packet.clone();
    hashable.disclosure_hash.clear();
    let encoded = serde_json::to_vec(&hashable).map_err(invalid_data)?;
    Ok(hash_hex("postfiat.orchard.disclosure.packet.v1", &encoded))
}

fn verify_orchard_disclosure_finality(
    store: &NodeStore,
    packet: &OrchardDisclosurePacket,
) -> io::Result<bool> {
    let Some(finality) = packet.finality.as_ref() else {
        return Ok(false);
    };
    let archive = store.read_batch_archive()?;
    let entry = archive
        .find(&finality.batch_kind, &finality.batch_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "archived batch `{}`/`{}` not found",
                    finality.batch_kind, finality.batch_id
                ),
            )
        })?;
    if entry.payload_hash != finality.batch_payload_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard disclosure batch payload hash mismatch",
        ));
    }
    let matching_action_indexes =
        shielded_archive_payload_orchard_commitment_action_indexes(
            &entry.payload_json,
            &packet.commitment,
        )?;
    if matching_action_indexes.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard disclosure commitment not found in archived batch payload",
        ));
    }
    let blocks = store.read_blocks()?;
    let block = blocks
        .blocks
        .iter()
        .find(|block| {
            block.header.batch_kind == finality.batch_kind
                && block.header.batch_id == finality.batch_id
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("block for archived batch `{}` not found", finality.batch_id),
            )
        })?;
    if block.header.height != finality.block_height
        || block.header.block_hash != finality.block_hash
        || block.header.state_root != finality.state_root
        || block.header.certificate_id != finality.certificate_id
        || block.receipt_ids != finality.receipt_ids
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard disclosure finality does not match local block log",
        ));
    }
    let receipts = store.read_receipts()?;
    if !matching_action_indexes.iter().any(|index| {
        shielded_action_index_has_accepted_receipt(&receipts, &block.receipt_ids, *index)
    }) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard disclosure commitment was not produced by an accepted shielded action",
        ));
    }
    Ok(true)
}

fn find_orchard_disclosure_finality(
    store: &NodeStore,
    commitment: &str,
) -> io::Result<Option<OrchardDisclosureFinality>> {
    let archive = store.read_batch_archive()?;
    let blocks = store.read_blocks()?;
    let receipts = store.read_receipts()?;
    for entry in archive
        .batches
        .iter()
        .filter(|entry| entry.batch_kind == BATCH_KIND_SHIELDED)
    {
        let matching_action_indexes =
            shielded_archive_payload_orchard_commitment_action_indexes(
                &entry.payload_json,
                commitment,
            )?;
        if matching_action_indexes.is_empty() {
            continue;
        }
        let Some(block) = blocks.blocks.iter().find(|block| {
            block.header.batch_kind == entry.batch_kind && block.header.batch_id == entry.batch_id
        }) else {
            continue;
        };
        if !matching_action_indexes.iter().any(|index| {
            shielded_action_index_has_accepted_receipt(&receipts, &block.receipt_ids, *index)
        }) {
            continue;
        }
        return Ok(Some(OrchardDisclosureFinality {
            batch_kind: entry.batch_kind.clone(),
            batch_id: entry.batch_id.clone(),
            batch_payload_hash: entry.payload_hash.clone(),
            block_height: block.header.height,
            block_hash: block.header.block_hash.clone(),
            state_root: block.header.state_root.clone(),
            certificate_id: block.header.certificate_id.clone(),
            receipt_ids: block.receipt_ids.clone(),
        }));
    }
    Ok(None)
}

fn shielded_archive_payload_orchard_commitment_action_indexes(
    payload_json: &str,
    commitment: &str,
) -> io::Result<Vec<usize>> {
    let batch: ShieldedActionBatch = serde_json::from_str(payload_json).map_err(invalid_data)?;
    let mut indexes = Vec::new();
    for (index, action) in batch.actions.iter().enumerate() {
        let action_json = match action {
            ShieldedAction::OrchardV1(payload) => Some(payload.action_json.as_str()),
            ShieldedAction::OrchardWithdrawV1(payload) => Some(payload.action_json.as_str()),
            ShieldedAction::OrchardDepositV1(payload) => Some(payload.action_json.as_str()),
            ShieldedAction::ShieldedSwapV1(payload) => {
                if let Ok(asset_action) =
                    serde_json::from_str::<AssetOrchardSwapAction>(&payload.swap_json)
                {
                    asset_action.validate().map_err(invalid_data)?;
                    if asset_action
                        .output_commitments
                        .iter()
                        .any(|output_commitment| output_commitment.as_hex() == commitment)
                    {
                        indexes.push(index);
                    }
                } else {
                    let swap_action: ShieldedSwapAction =
                        serde_json::from_str(&payload.swap_json).map_err(invalid_data)?;
                    swap_action.validate().map_err(invalid_data)?;
                    if swap_action
                        .output_commitments
                        .iter()
                        .any(|output_commitment| output_commitment.as_hex() == commitment)
                    {
                        indexes.push(index);
                    }
                }
                None
            }
            ShieldedAction::AssetOrchardIngressV1(payload) => {
                if payload.output_commitment == commitment {
                    indexes.push(index);
                }
                None
            }
            ShieldedAction::AssetOrchardIngressV2(payload) => {
                if payload.output_commitment == commitment {
                    indexes.push(index);
                }
                None
            }
            ShieldedAction::AssetOrchardEgressV1(_) => None,
            ShieldedAction::AssetOrchardPrivateEgressV1(_) => None,
            ShieldedAction::Mint(_) | ShieldedAction::Spend(_) | ShieldedAction::Migrate(_) => None,
        };
        let Some(action_json) = action_json else {
            continue;
        };
        let orchard_action: OrchardShieldedAction =
            serde_json::from_str(action_json).map_err(invalid_data)?;
        orchard_action.validate().map_err(invalid_data)?;
        if orchard_action
            .output_commitments
            .iter()
            .any(|output_commitment| output_commitment.as_hex() == commitment)
        {
            indexes.push(index);
        }
    }
    Ok(indexes)
}

fn shielded_action_index_has_accepted_receipt(
    receipts: &[Receipt],
    receipt_ids: &[String],
    action_index: usize,
) -> bool {
    let Some(receipt_id) = receipt_ids.get(action_index) else {
        return false;
    };
    receipts
        .iter()
        .any(|receipt| receipt.tx_id == *receipt_id && receipt.accepted)
}

fn read_orchard_action_file(path: &Path) -> io::Result<OrchardShieldedAction> {
    let action = read_json_file(path, "Orchard shielded action")?;
    Ok(action)
}

fn read_orchard_deposit_action_file(path: &Path) -> io::Result<OrchardDepositActionFile> {
    let file: OrchardDepositActionFile = read_json_file(path, "Orchard deposit action")?;
    if file.schema != ORCHARD_DEPOSIT_ACTION_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported Orchard deposit action file schema `{}`",
                file.schema
            ),
        ));
    }
    Ok(file)
}

fn orchard_authorizing_domain(
    genesis: &Genesis,
    pool_id: &str,
) -> io::Result<OrchardAuthorizingDomain> {
    OrchardAuthorizingDomain::new(
        genesis.chain_id.clone(),
        genesis_hash(genesis),
        genesis.protocol_version,
        pool_id.to_string(),
    )
    .map_err(invalid_data)
}

fn orchard_withdraw_external_binding_hash(
    genesis: &Genesis,
    pool_id: &str,
    to: &str,
    amount: u64,
    fee: u64,
    policy_id: &str,
    disclosure_hash: &str,
) -> io::Result<String> {
    let payload = serde_json::to_vec(&(
        "postfiat.privacy.orchard.withdraw.external-binding.v1",
        genesis.chain_id.as_str(),
        genesis_hash(genesis),
        genesis.protocol_version,
        pool_id,
        to,
        amount,
        fee,
        policy_id,
        disclosure_hash,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex(
        "postfiat.privacy.orchard.withdraw.external-binding.v1",
        &payload,
    ))
}

struct OrchardDepositExternalBindingInput<'a> {
    genesis: &'a Genesis,
    pool_id: &'a str,
    funding_transfer_id: &'a str,
    from_address: &'a str,
    amount: u64,
    fee: u64,
    policy_id: &'a str,
    disclosure_hash: &'a str,
}

fn orchard_deposit_external_binding_hash(
    input: OrchardDepositExternalBindingInput<'_>,
) -> io::Result<String> {
    let payload = serde_json::to_vec(&(
        "postfiat.privacy.orchard.deposit.external-binding.v1",
        input.genesis.chain_id.as_str(),
        genesis_hash(input.genesis),
        input.genesis.protocol_version,
        input.pool_id,
        input.funding_transfer_id,
        input.from_address,
        input.amount,
        input.fee,
        input.policy_id,
        input.disclosure_hash,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex(
        "postfiat.privacy.orchard.deposit.external-binding.v1",
        &payload,
    ))
}

fn orchard_deposit_turnstile_event_id(event: &TurnstileEvent) -> io::Result<String> {
    validate_bounded_text("Orchard deposit event kind", &event.kind)?;
    validate_bounded_text("Orchard deposit event owner", &event.owner)?;
    validate_bounded_text("Orchard deposit event asset id", &event.asset_id)?;
    if event.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard deposit event amount must be nonzero",
        ));
    }
    validate_hex_string(
        "Orchard deposit funding transfer id",
        &event.note_id,
        Some(96),
    )?;
    validate_bounded_text("Orchard deposit event source pool", &event.source_pool)?;
    validate_bounded_text("Orchard deposit event target pool", &event.target_pool)?;
    let payload = serde_json::to_vec(&(
        "postfiat.privacy.orchard.deposit.turnstile.v1",
        event.kind.as_str(),
        event.owner.as_str(),
        event.asset_id.as_str(),
        event.amount,
        event.note_id.as_str(),
        event.source_pool.as_str(),
        event.target_pool.as_str(),
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex(
        "postfiat.privacy.orchard.deposit.turnstile.v1",
        &payload,
    ))
}

fn orchard_deposit_payload_values(
    policy_id: Option<String>,
    disclosure_hash: Option<String>,
) -> io::Result<(String, String)> {
    let policy_id = policy_id.unwrap_or_else(|| ORCHARD_DEPOSIT_POLICY_ID.to_string());
    let disclosure_hash = disclosure_hash.unwrap_or_default();
    validate_orchard_deposit_policy_id(&policy_id)?;
    validate_orchard_deposit_disclosure_hash(&disclosure_hash)?;
    Ok((policy_id, disclosure_hash))
}

fn orchard_withdraw_payload_values(
    policy_id: Option<String>,
    disclosure_hash: Option<String>,
) -> io::Result<(String, String)> {
    let policy_id = policy_id.unwrap_or_else(|| ORCHARD_WITHDRAW_POLICY_ID.to_string());
    let disclosure_hash = disclosure_hash.unwrap_or_default();
    validate_orchard_withdraw_policy_id(&policy_id)?;
    validate_orchard_withdraw_disclosure_hash(&disclosure_hash)?;
    Ok((policy_id, disclosure_hash))
}

fn validate_orchard_deposit_payload(payload: &OrchardDepositActionPayload) -> io::Result<()> {
    payload
        .funding_transfer
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if payload.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard deposit amount must be nonzero",
        ));
    }
    payload.amount.checked_add(payload.fee).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard deposit amount plus fee overflowed",
        )
    })?;
    validate_orchard_deposit_policy_id(&payload.policy_id)?;
    validate_orchard_deposit_disclosure_hash(&payload.disclosure_hash)
}

fn validate_asset_orchard_ingress_payload(
    payload: &AssetOrchardIngressActionPayload,
) -> io::Result<()> {
    payload
        .burn_transaction
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if payload.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "AssetOrchard ingress pool_id must be `{}`",
                ASSET_ORCHARD_POOL_ID_V1
            ),
        ));
    }
    validate_hex_string(
        "AssetOrchard ingress asset_id",
        &payload.asset_id,
        Some(ISSUED_ASSET_ID_HEX_LEN),
    )?;
    if payload.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress amount must be nonzero",
        ));
    }
    validate_hex_string(
        "AssetOrchard ingress output commitment",
        &payload.output_commitment,
        Some(ORCHARD_COMMITMENT_BYTES * 2),
    )?;
    AssetOrchardFieldElement::parse_hex(payload.output_commitment.clone()).map_err(invalid_data)?;
    OrchardOutputCommitment::parse_hex(payload.output_commitment.clone()).map_err(invalid_data)?;
    validate_bounded_lower_hex_field(
        "AssetOrchard ingress encrypted output",
        &payload.encrypted_output,
        ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES * 2,
    )?;
    let AssetTransactionOperation::AssetBurn(burn) =
        &payload.burn_transaction.unsigned.operation
    else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress requires an asset_burn transaction",
        ));
    };
    if payload.burn_transaction.unsigned.transaction_kind != ASSET_BURN_TRANSACTION_KIND {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress burn transaction kind mismatch",
        ));
    }
    if payload.burn_transaction.unsigned.source != burn.owner {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress burn must be signed by the burned owner",
        ));
    }
    if burn.asset_id != payload.asset_id || burn.amount != payload.amount {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress burn asset/amount does not match payload",
        ));
    }
    let note = asset_orchard_public_note_from_ingress(&payload.note)?;
    note.validate_for_asset(&payload.asset_id, payload.amount)
        .map_err(invalid_data)?;
    Ok(())
}

fn validate_asset_orchard_ingress_v2_payload(
    payload: &AssetOrchardIngressV2ActionPayload,
) -> io::Result<()> {
    payload
        .burn_transaction
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    if payload.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("AssetOrchard ingress pool_id must be `{ASSET_ORCHARD_POOL_ID_V1}`"),
        ));
    }
    validate_hex_string(
        "AssetOrchard ingress asset_id",
        &payload.asset_id,
        Some(ISSUED_ASSET_ID_HEX_LEN),
    )?;
    if payload.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress amount must be nonzero",
        ));
    }
    validate_hex_string(
        "AssetOrchard ingress output commitment",
        &payload.output_commitment,
        Some(ORCHARD_COMMITMENT_BYTES * 2),
    )?;
    AssetOrchardFieldElement::parse_hex(payload.output_commitment.clone()).map_err(invalid_data)?;
    OrchardOutputCommitment::parse_hex(payload.output_commitment.clone()).map_err(invalid_data)?;
    validate_bounded_lower_hex_field(
        "AssetOrchard ingress encrypted output",
        &payload.encrypted_output,
        ASSET_ORCHARD_ENCRYPTED_OUTPUT_MAX_BYTES * 2,
    )?;
    let encrypted_output = hex_to_bytes(&payload.encrypted_output).map_err(invalid_data)?;
    if !encrypted_output.starts_with(ASSET_ORCHARD_NOTE_CIPHERTEXT_MAGIC) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress v2 requires a PFAOENC1 authenticated note ciphertext",
        ));
    }
    let AssetTransactionOperation::AssetBurn(burn) =
        &payload.burn_transaction.unsigned.operation
    else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress requires an asset_burn transaction",
        ));
    };
    if payload.burn_transaction.unsigned.transaction_kind != ASSET_BURN_TRANSACTION_KIND
        || payload.burn_transaction.unsigned.source != burn.owner
        || burn.asset_id != payload.asset_id
        || burn.amount != payload.amount
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress burn transaction does not bind source/asset/amount",
        ));
    }
    Ok(())
}

fn validate_asset_orchard_ingress_payload_for_genesis(
    genesis: &Genesis,
    payload: &AssetOrchardIngressActionPayload,
) -> io::Result<()> {
    validate_asset_orchard_ingress_payload(payload)?;
    let genesis_hash_32 = asset_orchard_domain_genesis_hash(&genesis_hash(genesis))
        .map_err(invalid_data)?;
    let pool_domain = AssetOrchardSwapAction::expected_pool_domain(
        &genesis.chain_id,
        genesis_hash_32,
        genesis.protocol_version,
    )
    .map_err(invalid_data)?;
    let note = asset_orchard_public_note_from_ingress(&payload.note)?;
    let expected_cmx = note.cmx(pool_domain).map_err(invalid_data)?;
    if expected_cmx.as_hex() != payload.output_commitment {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard ingress output commitment does not match public note opening",
        ));
    }
    Ok(())
}

fn validate_asset_orchard_egress_payload(
    payload: &AssetOrchardEgressActionPayload,
) -> io::Result<()> {
    if payload.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "AssetOrchard egress pool_id must be `{}`",
                ASSET_ORCHARD_POOL_ID_V1
            ),
        ));
    }
    validate_bounded_text("AssetOrchard egress recipient", &payload.to)?;
    validate_hex_string(
        "AssetOrchard egress asset_id",
        &payload.asset_id,
        Some(ISSUED_ASSET_ID_HEX_LEN),
    )?;
    if payload.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard egress amount must be nonzero",
        ));
    }
    validate_hex_string(
        "AssetOrchard egress output commitment",
        &payload.output_commitment,
        Some(ORCHARD_COMMITMENT_BYTES * 2),
    )?;
    validate_hex_string(
        "AssetOrchard egress nullifier",
        &payload.nullifier,
        Some(ASSET_ORCHARD_FIELD_BYTES * 2),
    )?;
    validate_hex_string(
        "AssetOrchard egress nk",
        &payload.nk,
        Some(ASSET_ORCHARD_FIELD_BYTES * 2),
    )?;
    validate_hex_string(
        "AssetOrchard egress rivk",
        &payload.rivk,
        Some(ASSET_ORCHARD_FIELD_BYTES * 2),
    )?;
    validate_hex_string(
        "AssetOrchard egress spend_auth_randomizer",
        &payload.spend_auth_randomizer,
        Some(ASSET_ORCHARD_FIELD_BYTES * 2),
    )?;
    AssetOrchardFieldElement::parse_hex(payload.output_commitment.clone()).map_err(invalid_data)?;
    AssetOrchardFieldElement::parse_hex(payload.nullifier.clone()).map_err(invalid_data)?;
    AssetOrchardFieldElement::parse_hex(payload.nk.clone()).map_err(invalid_data)?;
    asset_orchard_scalar_from_hex(&payload.rivk).map_err(invalid_data)?;
    asset_orchard_scalar_from_hex(&payload.spend_auth_randomizer).map_err(invalid_data)?;
    AssetOrchardPoint::parse_hex(payload.spend_auth_verification_key.clone())
        .map_err(invalid_data)?;
    AssetOrchardPoint::parse_hex(payload.randomized_verification_key.clone())
        .map_err(invalid_data)?;
    AssetOrchardSpendAuthSignature::parse_hex(payload.spend_authorization_signature.clone())
        .map_err(invalid_data)?;
    let note = asset_orchard_public_note_from_ingress(&payload.note)?;
    note.validate_for_asset(&payload.asset_id, payload.amount)
        .map_err(invalid_data)
}

fn validate_asset_orchard_egress_payload_for_genesis(
    genesis: &Genesis,
    payload: &AssetOrchardEgressActionPayload,
) -> io::Result<()> {
    validate_asset_orchard_egress_payload(payload)?;
    let genesis_hash_32 = asset_orchard_domain_genesis_hash(&genesis_hash(genesis))
        .map_err(invalid_data)?;
    let pool_domain = AssetOrchardSwapAction::expected_pool_domain(
        &genesis.chain_id,
        genesis_hash_32,
        genesis.protocol_version,
    )
    .map_err(invalid_data)?;
    let note = asset_orchard_public_note_from_ingress(&payload.note)?;
    let output_commitment =
        AssetOrchardFieldElement::parse_hex(payload.output_commitment.clone())
            .map_err(invalid_data)?;
    let nullifier = AssetOrchardFieldElement::parse_hex(payload.nullifier.clone())
        .map_err(invalid_data)?;
    let nk = AssetOrchardFieldElement::parse_hex(payload.nk.clone()).map_err(invalid_data)?;
    let spend_auth_verification_key =
        AssetOrchardPoint::parse_hex(payload.spend_auth_verification_key.clone())
            .map_err(invalid_data)?;
    let randomized_verification_key =
        AssetOrchardPoint::parse_hex(payload.randomized_verification_key.clone())
            .map_err(invalid_data)?;
    let spend_authorization_signature =
        AssetOrchardSpendAuthSignature::parse_hex(payload.spend_authorization_signature.clone())
            .map_err(invalid_data)?;
    let preimage = AssetOrchardDisclosedEgressPreimage {
        chain_id: &genesis.chain_id,
        genesis_hash: genesis_hash_32,
        protocol_version: genesis.protocol_version,
        pool_id: &payload.pool_id,
        pool_domain,
        to: &payload.to,
        asset_id: &payload.asset_id,
        amount: payload.amount,
        output_commitment: output_commitment.to_field().map_err(invalid_data)?,
        nullifier: nullifier.to_field().map_err(invalid_data)?,
        spend_auth_verification_key: spend_auth_verification_key
            .to_affine()
            .map_err(invalid_data)?,
        spend_auth_randomizer: asset_orchard_scalar_from_hex(&payload.spend_auth_randomizer)
            .map_err(invalid_data)?,
        randomized_verification_key: randomized_verification_key
            .to_affine()
            .map_err(invalid_data)?,
    };
    verify_asset_orchard_disclosed_egress(&AssetOrchardDisclosedEgressCheck {
        preimage,
        note: &note,
        nk: &nk,
        rivk: &payload.rivk,
        spend_authorization_signature: &spend_authorization_signature,
    })
    .map_err(invalid_data)
}

fn validate_asset_orchard_private_egress_payload(
    payload: &AssetOrchardPrivateEgressActionPayload,
) -> io::Result<()> {
    if payload.pool_id != ASSET_ORCHARD_POOL_ID_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "AssetOrchard private egress pool_id must be `{}`",
                ASSET_ORCHARD_POOL_ID_V1
            ),
        ));
    }
    validate_bounded_text("AssetOrchard private egress recipient", &payload.to)?;
    validate_hex_string(
        "AssetOrchard private egress asset_id",
        &payload.asset_id,
        Some(ISSUED_ASSET_ID_HEX_LEN),
    )?;
    if payload.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard private egress amount must be nonzero",
        ));
    }
    if payload.fee != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard private egress v1 requires fee 0",
        ));
    }
    validate_bounded_text("AssetOrchard private egress policy_id", &payload.policy_id)?;
    validate_bounded_text(
        "AssetOrchard private egress disclosure_hash",
        &payload.disclosure_hash,
    )?;
    let expected_tag = AssetTag::derive(&payload.asset_id).map_err(invalid_data)?;
    if payload.asset_tag_lo != expected_tag.lo || payload.asset_tag_hi != expected_tag.hi {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "AssetOrchard private egress asset tag does not match asset_id",
        ));
    }
    let action = asset_orchard_private_egress_action_from_payload(payload)?;
    action.validate().map_err(invalid_data)
}

fn validate_asset_orchard_private_egress_payload_for_genesis(
    genesis: &Genesis,
    payload: &AssetOrchardPrivateEgressActionPayload,
) -> io::Result<()> {
    validate_asset_orchard_private_egress_payload(payload)?;
    let action = asset_orchard_private_egress_action_from_payload(payload)?;
    let domain = orchard_authorizing_domain(genesis, &payload.pool_id)?;
    verify_serialized_asset_orchard_private_egress_action(
        &action,
        &domain,
        &payload.to,
        &payload.asset_id,
        &payload.policy_id,
        &payload.disclosure_hash,
    )
    .map(|_| ())
    .map_err(invalid_data)
}

fn asset_orchard_public_note_from_ingress(
    note: &AssetOrchardIngressNote,
) -> io::Result<AssetOrchardPublicNoteOpening> {
    Ok(AssetOrchardPublicNoteOpening {
        diversifier: note.diversifier.clone(),
        g_d: AssetOrchardPoint::parse_hex(note.g_d.clone()).map_err(invalid_data)?,
        pk_d: AssetOrchardPoint::parse_hex(note.pk_d.clone()).map_err(invalid_data)?,
        asset_tag_lo: note.asset_tag_lo,
        asset_tag_hi: note.asset_tag_hi,
        value: note.value,
        rho: AssetOrchardFieldElement::parse_hex(note.rho.clone()).map_err(invalid_data)?,
        psi: AssetOrchardFieldElement::parse_hex(note.psi.clone()).map_err(invalid_data)?,
        rcm: note.rcm.clone(),
    })
}

fn validate_orchard_withdraw_payload(payload: &OrchardWithdrawActionPayload) -> io::Result<()> {
    validate_orchard_withdraw_recipient(&payload.to)?;
    if payload.amount == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard withdraw amount must be nonzero",
        ));
    }
    if payload.fee != 0 && payload.amount.checked_add(payload.fee).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "Orchard withdraw amount plus fee overflowed",
        ));
    }
    validate_orchard_withdraw_policy_id(&payload.policy_id)?;
    validate_orchard_withdraw_disclosure_hash(&payload.disclosure_hash)
}

fn validate_orchard_deposit_policy_id(policy_id: &str) -> io::Result<()> {
    validate_bounded_text("Orchard deposit policy id", policy_id)?;
    if policy_id != ORCHARD_DEPOSIT_POLICY_ID {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported Orchard deposit policy `{policy_id}`"),
        ));
    }
    Ok(())
}

fn validate_orchard_deposit_disclosure_hash(disclosure_hash: &str) -> io::Result<()> {
    if disclosure_hash.is_empty() {
        return Ok(());
    }
    validate_hex_string("Orchard deposit disclosure hash", disclosure_hash, Some(96))
}

fn validate_orchard_withdraw_recipient(to: &str) -> io::Result<()> {
    validate_bounded_text("Orchard withdraw recipient", to)
}

fn validate_orchard_withdraw_policy_id(policy_id: &str) -> io::Result<()> {
    validate_bounded_text("Orchard withdraw policy id", policy_id)?;
    if policy_id != ORCHARD_WITHDRAW_POLICY_ID {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("unsupported Orchard withdraw policy `{policy_id}`"),
        ));
    }
    Ok(())
}

fn validate_orchard_withdraw_disclosure_hash(disclosure_hash: &str) -> io::Result<()> {
    if disclosure_hash.is_empty() {
        return Ok(());
    }
    validate_hex_string(
        "Orchard withdraw disclosure hash",
        disclosure_hash,
        Some(96),
    )
}

fn validate_bounded_text(label: &str, value: &str) -> io::Result<()> {
    if value.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must be nonempty"),
        ));
    }
    if value != value.trim() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must not have leading or trailing whitespace"),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must not contain control characters"),
        ));
    }
    if value.len() > MAX_TEXT_FIELD_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} must not exceed {MAX_TEXT_FIELD_BYTES} bytes"),
        ));
    }
    Ok(())
}
