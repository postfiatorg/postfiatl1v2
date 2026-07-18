pub fn ratify_validator_set(options: RatifyValidatorSetOptions) -> io::Result<GovernanceAmendment> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let domain = cobalt_domain(&genesis);
    let config = EssentialSubsetConfig::all_of(options.validators);
    let lifecycle = GovernanceAmendmentLifecycle {
        activation_height: options.activation_height,
        veto_until_height: options.veto_until_height,
        paused: options.paused,
    };
    let amendment = ratify_validator_set_amendment_with_lifecycle(
        &domain,
        &config,
        options.validator_count,
        options.support,
        lifecycle,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    write_amendment_file(&options.amendment_file, &amendment)?;
    Ok(amendment)
}

pub fn ratify_governance(options: RatifyGovernanceOptions) -> io::Result<GovernanceAmendment> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let domain = cobalt_domain(&genesis);
    let config = EssentialSubsetConfig::all_of(options.validators);
    let lifecycle = GovernanceAmendmentLifecycle {
        activation_height: options.activation_height,
        veto_until_height: options.veto_until_height,
        paused: options.paused,
    };
    let amendment = ratify_governance_amendment_with_lifecycle(
        &domain,
        &config,
        &options.kind,
        options.value,
        options.support,
        lifecycle,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    write_amendment_file(&options.amendment_file, &amendment)?;
    Ok(amendment)
}

pub fn sign_governance_amendment_authorization(
    options: GovernanceAuthorizationSignOptions,
) -> io::Result<SignedGovernanceAuthorizationV2> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let amendment = read_amendment_file(&options.amendment_file)?;
    verify_governance_amendment_evidence(&genesis, &amendment)?;
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let expected_slot = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    if options.proposal_slot != expected_slot {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "governance proposal slot {} does not match next block height {expected_slot}",
                options.proposal_slot
            ),
        ));
    }
    if options.expires_at_height < options.proposal_slot {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance authorization expiry precedes proposal slot",
        ));
    }
    let validators = active_validator_ids(&governance)?;
    if amendment.validators != validators {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "governance amendment validator set does not match the old active registry",
        ));
    }
    let vote = amendment
        .votes
        .iter()
        .find(|vote| vote.validator == options.validator && vote.accept)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "validator has no accepting vote in governance amendment",
            )
        })?;
    let registry = read_validator_registry_file(&options.data_dir.join(VALIDATOR_REGISTRY_FILE))?;
    let registry_root = validator_registry_root(&registry, &validators)?;
    let registry_record = validator_registry_record(&registry, &options.validator)?;
    let key_file = read_validator_key_file(&options.validator_key_file)?;
    validate_validator_key_file(&key_file)?;
    let key_record = validator_key_record(&key_file, &options.validator)?;
    if key_record.algorithm_id != registry_record.algorithm_id
        || key_record.public_key_hex != registry_record.public_key_hex
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "governance signing key does not match the old active registry",
        ));
    }
    let committee_epoch = governance
        .validator_registry_updates
        .iter()
        .filter(|update| update.activation_height < options.proposal_slot)
        .count() as u64;
    let mut authorization = SignedGovernanceAuthorizationV2 {
        schema: SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2.to_string(),
        validator: options.validator,
        vote_id: vote.vote_id.clone(),
        old_registry_root: registry_root,
        committee_epoch,
        proposal_slot: options.proposal_slot,
        expires_at_height: options.expires_at_height,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        signature_hex: String::new(),
    };
    let signing_bytes =
        governance_amendment_authorization_signing_bytes(&amendment, &authorization)?;
    let private_key = Zeroizing::new(hex_to_bytes(&key_record.private_key_hex).map_err(invalid_data)?);
    authorization.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &private_key,
            &signing_bytes,
            GOVERNANCE_AUTHORIZATION_SIGNATURE_CONTEXT_V2,
        )
        .map_err(invalid_data)?,
    );
    let json = serde_json::to_string_pretty(&authorization).map_err(invalid_data)?;
    atomic_write(&options.authorization_file, format!("{json}\n"))?;
    Ok(authorization)
}

pub fn assemble_signed_governance_amendment(
    options: GovernanceAmendmentAssembleOptions,
) -> io::Result<GovernanceAmendment> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let registry = read_validator_registry_file(&options.data_dir.join(VALIDATOR_REGISTRY_FILE))?;
    let mut amendment = read_amendment_file(&options.amendment_file)?;
    let mut authorizations = options
        .authorization_files
        .iter()
        .map(|path| read_json_file(path, "signed governance authorization"))
        .collect::<io::Result<Vec<SignedGovernanceAuthorizationV2>>>()?;
    authorizations.sort_by(|left, right| left.validator.cmp(&right.validator));
    amendment.signed_authorizations = authorizations;
    let batch = GovernanceActionBatch::new("authorization-check", vec![amendment.clone()]);
    verify_live_signed_governance_batch(
        &genesis,
        &governance,
        &registry,
        &batch,
        options.proposal_slot,
    )?;
    write_amendment_file(&options.output_file, &amendment)?;
    Ok(amendment)
}

pub fn apply_amendment(options: ApplyAmendmentOptions) -> io::Result<GovernanceAmendment> {
    let _ = options;
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "direct governance state mutation is disabled; live governance requires signed authorization and consensus admission",
    ))
}

pub fn create_governance_batch(
    options: GovernanceBatchOptions,
) -> io::Result<GovernanceActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let amendments = options
        .amendment_file
        .as_deref()
        .map(read_amendment_file)
        .transpose()?
        .into_iter()
        .collect::<Vec<_>>();
    for amendment in &amendments {
        verify_governance_amendment_evidence(&genesis, amendment)?;
    }
    let registry_updates = options
        .registry_update_file
        .as_deref()
        .map(read_validator_registry_update_file)
        .transpose()?
        .into_iter()
        .collect::<Vec<_>>();
    let domain = cobalt_domain(&genesis);
    for update in &registry_updates {
        verify_cobalt_validator_registry_update(&domain, update)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    }
    if amendments.is_empty() && registry_updates.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance batch requires an amendment or validator registry update",
        ));
    }
    let batch = build_governance_action_batch(&genesis, amendments, registry_updates)?;
    write_governance_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_fastswap_governance_bootstrap(
    options: FastSwapGovernanceBootstrapOptions,
) -> io::Result<GovernanceActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let raw = read_bounded_json_text_file(&options.payload_file, "FastSwap bootstrap payload")?;
    let payload = serde_json::from_str::<postfiat_types::FastSwapGovernanceBootstrapPayloadV1>(
        &raw,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    payload
        .validate_payload()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid FastSwap bootstrap payload"))?;
    if payload.committee.domain.chain.chain_id != genesis.chain_id
        || bytes_to_hex(&payload.committee.domain.chain.genesis_hash.0) != genesis_hash(&genesis)
        || payload.committee.domain.chain.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastSwap bootstrap payload domain does not match genesis",
        ));
    }
    let bootstrap_id = payload
        .bootstrap_id()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid FastSwap bootstrap id"))?;
    let kind = format!(
        "{}{}",
        postfiat_types::FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1,
        bytes_to_hex(&bootstrap_id.0)
    );
    let domain = cobalt_domain(&genesis);
    let config = EssentialSubsetConfig::all_of(options.validators);
    let lifecycle = GovernanceAmendmentLifecycle {
        activation_height: options.activation_height,
        veto_until_height: options.veto_until_height,
        paused: options.paused,
    };
    let amendment = ratify_governance_amendment_with_lifecycle(
        &domain,
        &config,
        &kind,
        postfiat_types::FASTSWAP_SCHEMA_VERSION_V1,
        options.support,
        lifecycle,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    write_amendment_file(&options.amendment_file, &amendment)?;
    let bootstrap = postfiat_types::FastSwapGovernanceBootstrapV1 { amendment, payload };
    let batch = build_governance_action_batch_with_fastswap_bootstraps(&genesis, vec![bootstrap])?;
    verify_governance_action_batch_id(&genesis, &batch)?;
    write_governance_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn assemble_signed_fastswap_governance_bootstrap(
    options: SignedFastSwapGovernanceBootstrapOptions,
) -> io::Result<GovernanceActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let registry = read_validator_registry_file(&options.data_dir.join(VALIDATOR_REGISTRY_FILE))?;
    let raw = read_bounded_json_text_file(&options.payload_file, "FastSwap bootstrap payload")?;
    let payload = serde_json::from_str::<postfiat_types::FastSwapGovernanceBootstrapPayloadV1>(
        &raw,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let amendment = read_amendment_file(&options.signed_amendment_file)?;
    let bootstrap = postfiat_types::FastSwapGovernanceBootstrapV1 { amendment, payload };
    let batch = build_governance_action_batch_with_fastswap_bootstraps(&genesis, vec![bootstrap])?;
    verify_governance_action_batch_id(&genesis, &batch)?;
    verify_live_signed_governance_batch(
        &genesis,
        &governance,
        &registry,
        &batch,
        options.proposal_slot,
    )?;
    write_governance_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_fastpay_recovery_governance_bootstrap(
    options: FastPayRecoveryGovernanceBootstrapOptions,
) -> io::Result<GovernanceActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let raw = read_bounded_json_text_file(&options.payload_file, "FastPay recovery payload")?;
    let payload = serde_json::from_str::<postfiat_types::FastPayRecoveryGovernancePayloadV1>(
        &raw,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    payload
        .payload_bytes()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if payload.committee.chain_id != genesis.chain_id
        || payload.committee.genesis_hash != genesis_hash(&genesis)
        || payload.committee.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay recovery payload domain does not match genesis",
        ));
    }
    let kind = format!(
        "{}{}",
        postfiat_types::FASTPAY_RECOVERY_GOVERNANCE_KIND_PREFIX_V1,
        payload
            .payload_id()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
    );
    let domain = cobalt_domain(&genesis);
    let config = EssentialSubsetConfig::all_of(options.validators);
    let amendment = ratify_governance_amendment_with_lifecycle(
        &domain,
        &config,
        &kind,
        postfiat_types::FASTPAY_RECOVERY_GOVERNANCE_VERSION_V1,
        options.support,
        GovernanceAmendmentLifecycle {
            // The payload hash binds the future feature activation. The
            // governance action itself must be committable before that height.
            activation_height: 0,
            veto_until_height: options.veto_until_height,
            paused: false,
        },
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    write_amendment_file(&options.amendment_file, &amendment)?;
    let bootstrap = postfiat_types::FastPayRecoveryGovernanceBootstrapV1 { amendment, payload };
    bootstrap
        .validate_payload_binding()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let batch =
        build_governance_action_batch_with_fastpay_recovery_bootstrap(&genesis, bootstrap)?;
    verify_governance_action_batch_id(&genesis, &batch)?;
    write_governance_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn assemble_signed_fastpay_recovery_governance_bootstrap(
    options: SignedFastPayRecoveryGovernanceBootstrapOptions,
) -> io::Result<GovernanceActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let registry = read_validator_registry_file(&options.data_dir.join(VALIDATOR_REGISTRY_FILE))?;
    let raw = read_bounded_json_text_file(&options.payload_file, "FastPay recovery payload")?;
    let payload = serde_json::from_str::<postfiat_types::FastPayRecoveryGovernancePayloadV1>(
        &raw,
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let amendment = read_amendment_file(&options.signed_amendment_file)?;
    let bootstrap = postfiat_types::FastPayRecoveryGovernanceBootstrapV1 { amendment, payload };
    bootstrap
        .validate_payload_binding()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let batch =
        build_governance_action_batch_with_fastpay_recovery_bootstrap(&genesis, bootstrap)?;
    verify_governance_action_batch_id(&genesis, &batch)?;
    verify_live_signed_governance_batch(
        &genesis,
        &governance,
        &registry,
        &batch,
        options.proposal_slot,
    )?;
    write_governance_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn create_vault_bridge_route_profile_governance(
    options: VaultBridgeRouteProfileGovernanceOptions,
) -> io::Result<GovernanceActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let raw = read_bounded_json_text_file(&options.profile_file, "vault bridge route profile")?;
    let profile = serde_json::from_str::<postfiat_types::VaultBridgeRouteProfileV1>(&raw)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    profile
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let kind = postfiat_types::vault_bridge_route_amendment_kind(&profile)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let domain = cobalt_domain(&genesis);
    let config = EssentialSubsetConfig::all_of(options.validators);
    let amendment = ratify_governance_amendment_with_lifecycle(
        &domain,
        &config,
        &kind,
        profile.route_epoch,
        options.support,
        GovernanceAmendmentLifecycle {
            activation_height: profile.activation_height,
            veto_until_height: options.veto_until_height,
            paused: false,
        },
    )
    .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    write_amendment_file(&options.amendment_file, &amendment)?;
    let tier4_finality_bootstrap = options
        .tier4_finality_bootstrap_file
        .as_ref()
        .map(|path| {
            let raw = read_bounded_json_text_file(path, "Tier-4 finality bootstrap")?;
            serde_json::from_str::<postfiat_types::EthereumArbitrumFinalityStateV2>(&raw)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .transpose()?;
    let activation = postfiat_types::VaultBridgeRouteProfileActivationV1 {
        schema: postfiat_types::VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1.to_string(),
        profile,
        amendment,
        tier4_finality_bootstrap,
    };
    let batch = build_governance_action_batch_with_vault_bridge_route_profile_activation(
        &genesis,
        activation,
    )?;
    verify_governance_action_batch_id(&genesis, &batch)?;
    write_governance_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn assemble_signed_vault_bridge_route_profile_governance(
    options: SignedVaultBridgeRouteProfileGovernanceOptions,
) -> io::Result<GovernanceActionBatch> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let registry = read_validator_registry_file(&options.data_dir.join(VALIDATOR_REGISTRY_FILE))?;
    let raw = read_bounded_json_text_file(&options.profile_file, "vault bridge route profile")?;
    let profile = serde_json::from_str::<postfiat_types::VaultBridgeRouteProfileV1>(&raw)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let amendment = read_amendment_file(&options.signed_amendment_file)?;
    let tier4_finality_bootstrap = options
        .tier4_finality_bootstrap_file
        .as_ref()
        .map(|path| {
            let raw = read_bounded_json_text_file(path, "Tier-4 finality bootstrap")?;
            serde_json::from_str::<postfiat_types::EthereumArbitrumFinalityStateV2>(&raw)
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
        })
        .transpose()?;
    let activation = postfiat_types::VaultBridgeRouteProfileActivationV1 {
        schema: postfiat_types::VAULT_BRIDGE_ROUTE_PROFILE_ACTIVATION_SCHEMA_V1.to_string(),
        profile,
        amendment,
        tier4_finality_bootstrap,
    };
    let batch = build_governance_action_batch_with_vault_bridge_route_profile_activation(
        &genesis,
        activation,
    )?;
    verify_governance_action_batch_id(&genesis, &batch)?;
    verify_live_signed_governance_batch(
        &genesis,
        &governance,
        &registry,
        &batch,
        options.proposal_slot,
    )?;
    write_governance_action_batch_file(&options.batch_file, &batch)?;
    Ok(batch)
}

pub fn validator_registry_root_report(
    options: ValidatorRegistryRootOptions,
) -> io::Result<ValidatorRegistryRootReport> {
    let store = NodeStore::new(&options.data_dir);
    if options.validators.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "validators must be nonempty",
        ));
    }
    let registry_path = options
        .registry_file
        .unwrap_or_else(|| store.data_dir().join(VALIDATOR_REGISTRY_FILE));
    let registry = read_validator_registry_file(&registry_path)?;
    let registry_root = validator_registry_root(&registry, &options.validators)?;
    Ok(ValidatorRegistryRootReport {
        schema: "postfiat-validator-registry-root-v1".to_string(),
        validator_count: options.validators.len(),
        validators: options.validators,
        registry_root,
    })
}

pub fn create_validator_registry_update(
    options: ValidatorRegistryUpdateOptions,
) -> io::Result<ValidatorRegistryUpdateRecord> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let domain = cobalt_domain(&genesis);
    let config = EssentialSubsetConfig::all_of(options.validators);
    let previous_record = options
        .previous_record_file
        .as_deref()
        .map(read_validator_registry_entry_file)
        .transpose()?;
    let new_record = options
        .new_record_file
        .as_deref()
        .map(read_validator_registry_entry_file)
        .transpose()?;
    let request = ValidatorRegistryUpdateRequest {
        activation_height: options.activation_height,
        previous_registry_root: options.previous_registry_root,
        new_registry_root: options.new_registry_root,
        previous_trust_graph_root: None,
        new_trust_graph_root: None,
        trust_graph_transition_id: None,
        previous_validators: options.previous_validators,
        new_validators: options.new_validators,
        operation: options.operation,
        subject_node_id: options.subject_node_id,
        previous_record,
        new_record,
    };
    let update = certify_validator_registry_update(&domain, &config, request, options.support)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    write_validator_registry_update_file(&options.update_file, &update)?;
    Ok(update)
}

pub fn sign_validator_registry_update_authorization(
    options: ValidatorRegistryAuthorizationSignOptions,
) -> io::Result<SignedGovernanceAuthorizationV2> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let update = read_validator_registry_update_file(&options.update_file)?;
    let domain = cobalt_domain(&genesis);
    verify_cobalt_validator_registry_update(&domain, &update)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let expected_slot = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    if options.proposal_slot != expected_slot {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "validator registry proposal slot {} does not match next block height {expected_slot}",
                options.proposal_slot
            ),
        ));
    }
    if options.expires_at_height < options.proposal_slot {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "validator registry authorization expiry precedes proposal slot",
        ));
    }
    let validators = active_validator_ids(&governance)?;
    let registry = read_validator_registry_file(&options.data_dir.join(VALIDATOR_REGISTRY_FILE))?;
    let registry_root = validator_registry_root(&registry, &validators)?;
    if update.validators != validators || update.previous_registry_root != registry_root {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "validator registry update does not bind the old active registry",
        ));
    }
    let vote = update
        .votes
        .iter()
        .find(|vote| vote.validator == options.validator && vote.accept)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::PermissionDenied,
                "validator has no accepting vote in validator registry update",
            )
        })?;
    let registry_record = validator_registry_record(&registry, &options.validator)?;
    let key_file = read_validator_key_file(&options.validator_key_file)?;
    validate_validator_key_file(&key_file)?;
    let key_record = validator_key_record(&key_file, &options.validator)?;
    if key_record.algorithm_id != registry_record.algorithm_id
        || key_record.public_key_hex != registry_record.public_key_hex
    {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "validator registry signing key does not match the old active registry",
        ));
    }
    let committee_epoch = governance
        .validator_registry_updates
        .iter()
        .filter(|existing| existing.activation_height < options.proposal_slot)
        .count() as u64;
    let mut authorization = SignedGovernanceAuthorizationV2 {
        schema: SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2.to_string(),
        validator: options.validator,
        vote_id: vote.vote_id.clone(),
        old_registry_root: registry_root,
        committee_epoch,
        proposal_slot: options.proposal_slot,
        expires_at_height: options.expires_at_height,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        signature_hex: String::new(),
    };
    let signing_bytes =
        validator_registry_update_authorization_signing_bytes(&update, &authorization)?;
    let private_key = Zeroizing::new(hex_to_bytes(&key_record.private_key_hex).map_err(invalid_data)?);
    authorization.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &private_key,
            &signing_bytes,
            GOVERNANCE_AUTHORIZATION_SIGNATURE_CONTEXT_V2,
        )
        .map_err(invalid_data)?,
    );
    let json = serde_json::to_string_pretty(&authorization).map_err(invalid_data)?;
    atomic_write(&options.authorization_file, format!("{json}\n"))?;
    Ok(authorization)
}

pub fn assemble_signed_validator_registry_update(
    options: ValidatorRegistryUpdateAssembleOptions,
) -> io::Result<ValidatorRegistryUpdateRecord> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let registry = read_validator_registry_file(&options.data_dir.join(VALIDATOR_REGISTRY_FILE))?;
    let mut update = read_validator_registry_update_file(&options.update_file)?;
    let mut authorizations = options
        .authorization_files
        .iter()
        .map(|path| read_json_file(path, "signed validator registry authorization"))
        .collect::<io::Result<Vec<SignedGovernanceAuthorizationV2>>>()?;
    authorizations.sort_by(|left, right| left.validator.cmp(&right.validator));
    update.signed_authorizations = authorizations;
    let batch = GovernanceActionBatch::with_registry_updates(
        "authorization-check",
        Vec::new(),
        vec![update.clone()],
    );
    verify_live_signed_governance_batch(
        &genesis,
        &governance,
        &registry,
        &batch,
        options.proposal_slot,
    )?;
    write_validator_registry_update_file(&options.output_file, &update)?;
    Ok(update)
}

pub fn verify_validator_registry_update_file(
    options: ValidatorRegistryUpdateVerifyOptions,
) -> io::Result<ValidatorRegistryUpdateVerificationReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let domain = cobalt_domain(&genesis);
    let update = read_validator_registry_update_file(&options.update_file)?;
    verify_cobalt_validator_registry_update(&domain, &update)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    let previous_validators = validator_registry_update_previous_validators(&update);
    let new_validators = validator_registry_update_new_validators(&update);

    let previous_registry_root_verified = if let Some(path) = options.previous_registry_file {
        let registry = read_validator_registry_file(&path)?;
        let root = validator_registry_root(&registry, &previous_validators)?;
        if root != update.previous_registry_root {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "previous validator registry root mismatch",
            ));
        }
        true
    } else {
        false
    };
    let new_registry_root_verified = if let Some(path) = options.new_registry_file {
        let registry = read_validator_registry_file(&path)?;
        let root = validator_registry_root(&registry, &new_validators)?;
        if root != update.new_registry_root {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "new validator registry root mismatch",
            ));
        }
        true
    } else {
        false
    };

    Ok(ValidatorRegistryUpdateVerificationReport {
        schema: "postfiat-validator-registry-update-verification-v1".to_string(),
        verified: true,
        update_id: update.update_id,
        operation: update.operation,
        subject_node_id: update.subject_node_id,
        activation_height: update.activation_height,
        previous_registry_root: update.previous_registry_root,
        new_registry_root: update.new_registry_root,
        previous_registry_root_verified,
        new_registry_root_verified,
        previous_validator_count: previous_validators.len(),
        new_validator_count: new_validators.len(),
        support_count: update.support.len(),
        vote_count: update.votes.len(),
    })
}

pub fn apply_validator_registry_update(
    options: ValidatorRegistryUpdateApplyOptions,
) -> io::Result<ValidatorRegistryUpdateApplyReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let domain = cobalt_domain(&genesis);
    let update = read_validator_registry_update_file(&options.update_file)?;
    verify_cobalt_validator_registry_update(&domain, &update)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    if options.current_height < update.activation_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "validator registry update activation height not reached",
        ));
    }

    let previous_validators = validator_registry_update_previous_validators(&update);
    let new_validators = validator_registry_update_new_validators(&update);
    let previous_registry_path = options
        .previous_registry_file
        .unwrap_or_else(|| store.data_dir().join(VALIDATOR_REGISTRY_FILE));
    let live_registry_path = store.data_dir().join(VALIDATOR_REGISTRY_FILE);
    let output_registry_path = options
        .output_registry_file
        .unwrap_or_else(|| live_registry_path.clone());
    let mut registry = read_validator_registry_file(&previous_registry_path)?;
    apply_verified_validator_registry_update_to_registry(
        &genesis,
        &mut registry,
        &update,
        options.current_height,
        "validator registry update apply",
    )?;
    if output_registry_path == live_registry_path {
        if let Ok(expected) = local_validator_ids(new_validators.len() as u32) {
            if expected == new_validators {
                let mut governance = store.read_governance()?;
                governance.active_validator_count = new_validators.len() as u32;
                store.write_governance(&governance)?;
            }
        }
    }
    write_validator_registry_file(&output_registry_path, &registry)?;

    Ok(ValidatorRegistryUpdateApplyReport {
        schema: "postfiat-validator-registry-update-apply-v1".to_string(),
        applied: true,
        update_id: update.update_id,
        operation: update.operation,
        subject_node_id: update.subject_node_id,
        activation_height: update.activation_height,
        current_height: options.current_height,
        previous_registry_root: update.previous_registry_root,
        new_registry_root: update.new_registry_root,
        previous_validator_count: previous_validators.len(),
        new_validator_count: new_validators.len(),
        output_registry_file: output_registry_path.display().to_string(),
    })
}

pub fn verify_validator_registry_lifecycle_replay_bundle(
    options: ValidatorRegistryLifecycleReplayVerifyOptions,
) -> io::Result<ValidatorRegistryLifecycleReplayVerifyReport> {
    let bundle: ValidatorRegistryLifecycleReplayBundle = read_json_file(
        &options.bundle_file,
        "validator registry lifecycle replay bundle",
    )?;
    if bundle.schema != "postfiat-validator-registry-lifecycle-replay-bundle-v0" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported validator registry lifecycle replay bundle schema `{}`",
                bundle.schema
            ),
        ));
    }
    if bundle.initial_validators.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "validator registry lifecycle replay initial validator set is empty",
        ));
    }
    if bundle.ordered_updates.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "validator registry lifecycle replay has no updates",
        ));
    }
    let domain = CobaltDomain {
        chain_id: bundle.chain_id.clone(),
        genesis_hash: bundle.genesis_hash.clone(),
        protocol_version: bundle.protocol_version,
    };
    let mut registry = bundle.initial_registry.clone();
    let mut current_validators = bundle.initial_validators.clone();
    let initial_registry_root = validator_registry_root(&registry, &current_validators)?;
    let first_previous_validators =
        validator_registry_update_previous_validators(&bundle.ordered_updates[0]);
    if current_validators != first_previous_validators {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "validator registry lifecycle replay initial validators do not match first update",
        ));
    }

    let mut seen_update_ids = HashSet::new();
    let mut operations = Vec::with_capacity(bundle.ordered_updates.len());
    let mut update_ids = Vec::with_capacity(bundle.ordered_updates.len());
    for (index, update) in bundle.ordered_updates.iter().enumerate() {
        if update.chain_id != domain.chain_id
            || update.genesis_hash != domain.genesis_hash
            || update.protocol_version != domain.protocol_version
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("validator registry lifecycle replay update {index} domain mismatch"),
            ));
        }
        if !seen_update_ids.insert(update.update_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate validator registry lifecycle replay update id",
            ));
        }
        let previous_validators = validator_registry_update_previous_validators(update);
        if previous_validators != current_validators {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "validator registry lifecycle replay update {index} validator order mismatch"
                ),
            ));
        }
        current_validators = apply_verified_validator_registry_update_to_registry_for_domain(
            &domain,
            &mut registry,
            update,
            update.activation_height,
            "validator registry lifecycle replay",
        )?;
        operations.push(update.operation.clone());
        update_ids.push(update.update_id.clone());
    }

    if current_validators != bundle.final_validators {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "validator registry lifecycle replay final validators mismatch",
        ));
    }
    let final_registry_root = validator_registry_root(&registry, &bundle.final_validators)?;
    if final_registry_root != bundle.final_registry_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "validator registry lifecycle replay final registry root mismatch",
        ));
    }

    Ok(ValidatorRegistryLifecycleReplayVerifyReport {
        schema: "postfiat-validator-registry-lifecycle-replay-verify-v0".to_string(),
        verified: true,
        bundle_file: options.bundle_file.display().to_string(),
        chain_id: domain.chain_id,
        genesis_hash: domain.genesis_hash,
        protocol_version: domain.protocol_version,
        initial_validator_count: bundle.initial_validators.len(),
        final_validator_count: bundle.final_validators.len(),
        initial_registry_root,
        final_registry_root,
        update_count: bundle.ordered_updates.len(),
        latest_update_id: update_ids.last().cloned().unwrap_or_default(),
        operations,
        update_ids,
    })
}

pub fn verify_governance_replay_package(
    options: GovernanceReplayVerifyOptions,
) -> io::Result<GovernanceReplayVerifyReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let genesis_hash = genesis_hash(&genesis);
    let package: GovernanceReplayPackage =
        read_json_file(&options.package_file, "governance replay package")?;
    if package.schema != "postfiat-governance-replay-package-v0" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported governance replay package schema `{}`",
                package.schema
            ),
        ));
    }
    if package
        .chain_id
        .as_ref()
        .is_some_and(|chain_id| chain_id != &genesis.chain_id)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance replay package chain id mismatch",
        ));
    }
    if package
        .genesis_hash
        .as_ref()
        .is_some_and(|hash| hash != &genesis_hash)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance replay package genesis hash mismatch",
        ));
    }
    if package
        .protocol_version
        .is_some_and(|version| version != genesis.protocol_version)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance replay package protocol version mismatch",
        ));
    }

    let previous_registry_file =
        resolve_governance_replay_path(&options.package_file, &package.previous_registry_file)?;
    let update_file = resolve_governance_replay_path(&options.package_file, &package.update_file)?;
    let new_registry_file =
        resolve_governance_replay_path(&options.package_file, &package.new_registry_file)?;

    let update = read_validator_registry_update_file(&update_file)?;
    if package
        .expected_update_id
        .as_ref()
        .is_some_and(|expected| expected != &update.update_id)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance replay package expected update id mismatch",
        ));
    }
    let genesis_bundle_report = if let Some(bundle_file) = package.genesis_bundle_file.as_deref() {
        let bundle_file = resolve_governance_replay_path(&options.package_file, bundle_file)?;
        let report = verify_governance_genesis_bundle(GovernanceGenesisVerifyOptions {
            data_dir: options.data_dir.clone(),
            bundle_file,
        })?;
        if report.registry_root != update.previous_registry_root {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance replay package genesis registry root does not match previous registry root",
            ));
        }
        Some(report)
    } else {
        None
    };

    let update_report =
        verify_validator_registry_update_file(ValidatorRegistryUpdateVerifyOptions {
            data_dir: options.data_dir.clone(),
            update_file: update_file.clone(),
            previous_registry_file: Some(previous_registry_file),
            new_registry_file: Some(new_registry_file.clone()),
        })?;

    let (governance_batch_verified, governance_batch_id, governance_batch_contains_update) =
        if let Some(batch_file) = package.governance_batch_file.as_deref() {
            let batch_file = resolve_governance_replay_path(&options.package_file, batch_file)?;
            let batch = read_governance_action_batch_file(&batch_file)?;
            verify_governance_action_batch_id(&genesis, &batch)?;
            if package
                .expected_batch_id
                .as_ref()
                .is_some_and(|expected| expected != &batch.batch_id)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance replay package expected batch id mismatch",
                ));
            }
            let contains_update = batch
                .validator_registry_updates
                .iter()
                .any(|candidate| candidate.update_id == update.update_id);
            if !contains_update {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance replay batch does not contain the registry update",
                ));
            }
            (true, batch.batch_id, true)
        } else {
            (false, String::new(), false)
        };

    let amendment_replay_report =
        if let Some(bundle_file) = package.amendment_replay_bundle_file.as_deref() {
            let bundle_file = resolve_governance_replay_path(&options.package_file, bundle_file)?;
            let report = verify_governance_amendment_replay_bundle(
                GovernanceAmendmentReplayVerifyOptions { bundle_file },
            )?;
            if report.chain_id.as_str() != genesis.chain_id.as_str()
                || report.genesis_hash.as_str() != genesis_hash.as_str()
                || report.protocol_version != genesis.protocol_version
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "governance replay package amendment replay domain mismatch",
                ));
            }
            Some(report)
        } else {
            None
        };

    let post_change =
        if let Some(certificate_file) = package.post_change_certificate_file.as_deref() {
            let block_file = package.post_change_block_file.as_deref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "post-change certificate requires post_change_block_file",
                )
            })?;
            let block_file = resolve_governance_replay_path(&options.package_file, block_file)?;
            let certificate_file =
                resolve_governance_replay_path(&options.package_file, certificate_file)?;
            let batch_file = package
                .post_change_batch_file
                .as_deref()
                .map(|path| resolve_governance_replay_path(&options.package_file, path))
                .transpose()?;
            let registry = read_validator_registry_file(&new_registry_file)?;
            let block: BlockRecord = read_json_file(&block_file, "post-change block record")?;
            if block.header.height < update.activation_height {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "post-change block height is before registry update activation",
                ));
            }
            if block.header.certificate_id.trim().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "post-change block has empty certificate id",
                ));
            }
            let certificate = read_block_certificate_file(&certificate_file)?;
            if certificate.certificate_id != block.header.certificate_id {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "post-change certificate id does not match block header",
                ));
            }
            if certificate
                .block_hash
                .as_ref()
                .is_some_and(|hash| hash != &block.header.block_hash)
            {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "post-change certificate block hash does not match block header",
                ));
            }
            if certificate.proposal_hash.is_some() && batch_file.is_none() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "post-change certificate with proposal hash requires post_change_batch_file",
                ));
            }
            let evidence = BlockEvidence::from_block(&block);
            let expected_proposal_hash = if certificate.proposal_hash.is_some() {
                let batch_file = batch_file.as_ref().ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        "post-change batch file is required for proposal hash verification",
                    )
                })?;
                let payload_json = canonical_governance_replay_batch_payload_json(
                    &block.header.batch_kind,
                    batch_file,
                )?;
                let payload_hash = batch_archive_payload_hash(
                    &genesis,
                    &block.header.batch_kind,
                    &block.header.batch_id,
                    &payload_json,
                )?;
                Some(block_proposal_hash_from_evidence(
                    &genesis,
                    &evidence,
                    &payload_hash,
                )?)
            } else {
                None
            };
            verify_external_block_certificate(
                &genesis,
                &evidence,
                &certificate,
                expected_proposal_hash.as_deref(),
                &registry,
                &validator_registry_update_new_validators(&update),
            )?;
            if certificate.certificate.registry_root != update.new_registry_root {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "post-change certificate registry root does not match registry update new root",
                ));
            }
            Some((
                block.header.height,
                certificate.certificate_id,
                certificate.certificate.registry_root,
            ))
        } else {
            None
        };

    Ok(GovernanceReplayVerifyReport {
        schema: "postfiat-governance-replay-verify-v0".to_string(),
        verified: true,
        package_file: options.package_file.display().to_string(),
        chain_id: genesis.chain_id,
        genesis_hash,
        protocol_version: genesis.protocol_version,
        update_id: update_report.update_id,
        operation: update_report.operation,
        subject_node_id: update_report.subject_node_id,
        activation_height: update_report.activation_height,
        previous_registry_root: update_report.previous_registry_root,
        new_registry_root: update_report.new_registry_root,
        previous_registry_root_verified: update_report.previous_registry_root_verified,
        new_registry_root_verified: update_report.new_registry_root_verified,
        previous_validator_count: update_report.previous_validator_count,
        new_validator_count: update_report.new_validator_count,
        support_count: update_report.support_count,
        vote_count: update_report.vote_count,
        governance_genesis_bundle_verified: genesis_bundle_report.is_some(),
        governance_genesis_bundle_hash: genesis_bundle_report
            .as_ref()
            .map(|report| report.bundle_hash.clone()),
        governance_genesis_registry_root: genesis_bundle_report
            .as_ref()
            .map(|report| report.registry_root.clone()),
        governance_genesis_operator_manifest_count: genesis_bundle_report
            .as_ref()
            .map(|report| report.operator_manifest_count),
        governance_batch_verified,
        governance_batch_id,
        governance_batch_contains_update,
        amendment_replay_verified: amendment_replay_report.is_some(),
        amendment_replay_bundle_file: amendment_replay_report
            .as_ref()
            .map(|report| report.bundle_file.clone()),
        amendment_replay_amendment_count: amendment_replay_report
            .as_ref()
            .map(|report| report.amendment_count),
        amendment_replay_activation_record_count: amendment_replay_report
            .as_ref()
            .map(|report| report.activation_record_count),
        amendment_replay_supersession_record_count: amendment_replay_report
            .as_ref()
            .map(|report| report.supersession_record_count),
        amendment_replay_rollback_record_count: amendment_replay_report
            .as_ref()
            .map(|report| report.rollback_record_count),
        post_change_certificate_verified: post_change.is_some(),
        post_change_block_height: post_change.as_ref().map(|(height, _, _)| *height),
        post_change_certificate_id: post_change
            .as_ref()
            .map(|(_, certificate_id, _)| certificate_id.clone()),
        post_change_certificate_registry_root: post_change.map(|(_, _, root)| root),
    })
}

pub fn verify_governance_amendment_replay_bundle(
    options: GovernanceAmendmentReplayVerifyOptions,
) -> io::Result<GovernanceAmendmentReplayVerifyReport> {
    let bundle: GovernanceAmendmentReplayBundle =
        read_json_file(&options.bundle_file, "governance amendment replay bundle")?;
    if bundle.schema != "postfiat-cobalt-amendment-replay-bundle-v0" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported governance amendment replay bundle schema `{}`",
                bundle.schema
            ),
        ));
    }
    verify_governance_amendment_replay_counts(&bundle)?;

    let first = bundle.ordered_amendments.first().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment replay bundle has no amendments",
        )
    })?;
    let domain = CobaltDomain {
        chain_id: first.chain_id.clone(),
        genesis_hash: first.genesis_hash.clone(),
        protocol_version: first.protocol_version,
    };
    let mut seen_amendment_ids = HashSet::new();
    for amendment in &bundle.ordered_amendments {
        verify_cobalt_governance_amendment(&domain, amendment)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if !seen_amendment_ids.insert(amendment.amendment_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment replay amendment id",
            ));
        }
    }

    let mut replay = GovernanceState::new(initial_amendment_replay_validator_count(&bundle));
    verify_governance_amendment_replay_activation_records(&bundle, &mut replay)?;
    verify_governance_amendment_replay_supersession_records(&domain, &bundle)?;
    verify_governance_amendment_replay_rollback_records(&domain, &bundle)?;
    verify_governance_amendment_replay_final_state(&bundle, &replay)?;

    Ok(GovernanceAmendmentReplayVerifyReport {
        schema: "postfiat-governance-amendment-replay-verify-v0".to_string(),
        verified: true,
        bundle_file: options.bundle_file.display().to_string(),
        chain_id: domain.chain_id,
        genesis_hash: domain.genesis_hash,
        protocol_version: domain.protocol_version,
        active_validator_count: replay.active_validator_count,
        crypto_policy_version: replay.crypto_policy_version,
        bridge_witness_epoch: replay.bridge_witness_epoch,
        authority_mode: replay.authority_mode,
        amendment_count: bundle.ordered_amendments.len(),
        latest_amendment_id: bundle
            .ordered_amendments
            .last()
            .map(|amendment| amendment.amendment_id.clone())
            .unwrap_or_default(),
        activation_record_count: bundle.ordered_activation_records.len(),
        latest_activation_record_id: bundle
            .ordered_activation_records
            .last()
            .map(|record| record.activation_record_id.clone())
            .unwrap_or_default(),
        supersession_record_count: bundle.ordered_supersession_records.len(),
        latest_supersession_record_id: bundle
            .ordered_supersession_records
            .last()
            .map(|record| record.supersession_record_id.clone())
            .unwrap_or_default(),
        rollback_record_count: bundle.ordered_rollback_records.len(),
        latest_rollback_record_id: bundle
            .ordered_rollback_records
            .last()
            .map(|record| record.rollback_record_id.clone())
            .unwrap_or_default(),
    })
}

fn verify_governance_amendment_replay_counts(
    bundle: &GovernanceAmendmentReplayBundle,
) -> io::Result<()> {
    if bundle.ordered_amendment_count != bundle.ordered_amendments.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment replay amendment count mismatch",
        ));
    }
    if bundle.ordered_activation_record_count != bundle.ordered_activation_records.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment replay activation record count mismatch",
        ));
    }
    if bundle.ordered_supersession_record_count != bundle.ordered_supersession_records.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment replay supersession record count mismatch",
        ));
    }
    if bundle.ordered_rollback_record_count != bundle.ordered_rollback_records.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment replay rollback record count mismatch",
        ));
    }
    Ok(())
}

fn initial_amendment_replay_validator_count(bundle: &GovernanceAmendmentReplayBundle) -> u32 {
    bundle
        .ordered_amendments
        .iter()
        .zip(bundle.ordered_activation_records.iter())
        .find(|(amendment, _)| amendment.kind == GOVERNANCE_KIND_VALIDATOR_SET)
        .map(|(_, record)| record.previous_value)
        .unwrap_or(bundle.final_governance.active_validator_count)
}

fn verify_governance_amendment_replay_activation_records(
    bundle: &GovernanceAmendmentReplayBundle,
    replay: &mut GovernanceState,
) -> io::Result<()> {
    if bundle.ordered_amendments.len() != bundle.ordered_activation_records.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment replay activation record count mismatch",
        ));
    }

    let mut seen_record_ids = HashSet::new();
    let mut seen_amendment_ids = HashSet::new();
    for (index, amendment) in bundle.ordered_amendments.iter().enumerate() {
        let record = &bundle.ordered_activation_records[index];
        if record.amendment_id != amendment.amendment_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "governance amendment replay activation record order mismatch for {}",
                    amendment.amendment_id
                ),
            ));
        }
        verify_governance_amendment_activation_record(replay, amendment, record, true)?;
        if !seen_record_ids.insert(record.activation_record_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment replay activation record id",
            ));
        }
        if !seen_amendment_ids.insert(record.amendment_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment replay activation record amendment id",
            ));
        }
        replay.apply(amendment.clone());
    }
    Ok(())
}

fn verify_governance_amendment_replay_supersession_records(
    domain: &CobaltDomain,
    bundle: &GovernanceAmendmentReplayBundle,
) -> io::Result<()> {
    let expected = expected_governance_amendment_supersessions(&bundle.ordered_amendments);
    if expected.len() != bundle.ordered_supersession_records.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment replay supersession record count mismatch",
        ));
    }

    let mut seen_record_ids = HashSet::new();
    let mut seen_superseding_ids = HashSet::new();
    for (index, (superseded, superseding)) in expected.iter().enumerate() {
        let record = &bundle.ordered_supersession_records[index];
        verify_governance_amendment_supersession_record_for_domain(
            domain,
            superseded,
            superseding,
            record,
            true,
        )?;
        if !seen_record_ids.insert(record.supersession_record_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment replay supersession record id",
            ));
        }
        if !seen_superseding_ids.insert(record.superseding_amendment_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment replay supersession record superseding amendment id",
            ));
        }
    }
    Ok(())
}

fn verify_governance_amendment_replay_rollback_records(
    domain: &CobaltDomain,
    bundle: &GovernanceAmendmentReplayBundle,
) -> io::Result<()> {
    let expected = expected_governance_amendment_rollbacks(&bundle.ordered_amendments);
    if expected.len() != bundle.ordered_rollback_records.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment replay rollback record count mismatch",
        ));
    }

    let mut seen_record_ids = HashSet::new();
    let mut seen_rollback_ids = HashSet::new();
    for (index, (rolled_back, restored, rollback)) in expected.iter().enumerate() {
        let record = &bundle.ordered_rollback_records[index];
        verify_governance_amendment_rollback_record_for_domain(
            domain,
            rolled_back,
            restored,
            rollback,
            record,
            true,
        )?;
        if !seen_record_ids.insert(record.rollback_record_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment replay rollback record id",
            ));
        }
        if !seen_rollback_ids.insert(record.rollback_amendment_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance amendment replay rollback record rollback amendment id",
            ));
        }
    }
    Ok(())
}

fn verify_governance_amendment_replay_final_state(
    bundle: &GovernanceAmendmentReplayBundle,
    replay: &GovernanceState,
) -> io::Result<()> {
    let final_governance = &bundle.final_governance;
    let latest_amendment_id = bundle
        .ordered_amendments
        .last()
        .map(|amendment| amendment.amendment_id.as_str())
        .unwrap_or_default();
    let latest_activation_record_id = bundle
        .ordered_activation_records
        .last()
        .map(|record| record.activation_record_id.as_str())
        .unwrap_or_default();
    let latest_supersession_record_id = bundle
        .ordered_supersession_records
        .last()
        .map(|record| record.supersession_record_id.as_str())
        .unwrap_or_default();
    let latest_rollback_record_id = bundle
        .ordered_rollback_records
        .last()
        .map(|record| record.rollback_record_id.as_str())
        .unwrap_or_default();

    if final_governance.active_validator_count != replay.active_validator_count
        || final_governance.crypto_policy_version != replay.crypto_policy_version
        || final_governance.bridge_witness_epoch != replay.bridge_witness_epoch
        || final_governance.authority_mode != replay.authority_mode
        || final_governance.amendment_count != bundle.ordered_amendments.len()
        || final_governance.latest_amendment_id != latest_amendment_id
        || final_governance.amendment_activation_record_count
            != bundle.ordered_activation_records.len()
        || final_governance.latest_amendment_activation_record_id != latest_activation_record_id
        || final_governance.amendment_supersession_record_count
            != bundle.ordered_supersession_records.len()
        || final_governance.latest_amendment_supersession_record_id != latest_supersession_record_id
        || final_governance.amendment_rollback_record_count != bundle.ordered_rollback_records.len()
        || final_governance.latest_amendment_rollback_record_id != latest_rollback_record_id
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance amendment replay final governance mismatch",
        ));
    }
    Ok(())
}

pub fn create_governance_replay_package(
    options: GovernanceReplayBuildOptions,
) -> io::Result<GovernanceReplayPackage> {
    ensure_output_can_be_written(
        &options.output_file,
        options.overwrite,
        "governance replay package",
    )?;
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let update = read_validator_registry_update_file(&options.update_file)?;
    if let Some(parent) = options
        .output_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent)?;
    }
    let expected_batch_id = options
        .governance_batch_file
        .as_deref()
        .map(read_governance_action_batch_file)
        .transpose()?
        .map(|batch| batch.batch_id);
    let package = GovernanceReplayPackage {
        schema: "postfiat-governance-replay-package-v0".to_string(),
        chain_id: Some(genesis.chain_id.clone()),
        genesis_hash: Some(genesis_hash(&genesis)),
        protocol_version: Some(genesis.protocol_version),
        genesis_bundle_file: options
            .genesis_bundle_file
            .as_deref()
            .map(|path| governance_replay_path_reference(&options.output_file, path)),
        previous_registry_file: governance_replay_path_reference(
            &options.output_file,
            &options.previous_registry_file,
        ),
        update_file: governance_replay_path_reference(&options.output_file, &options.update_file),
        new_registry_file: governance_replay_path_reference(
            &options.output_file,
            &options.new_registry_file,
        ),
        amendment_replay_bundle_file: options
            .amendment_replay_bundle_file
            .as_deref()
            .map(|path| governance_replay_path_reference(&options.output_file, path)),
        governance_batch_file: options
            .governance_batch_file
            .as_deref()
            .map(|path| governance_replay_path_reference(&options.output_file, path)),
        post_change_block_file: options
            .post_change_block_file
            .as_deref()
            .map(|path| governance_replay_path_reference(&options.output_file, path)),
        post_change_batch_file: options
            .post_change_batch_file
            .as_deref()
            .map(|path| governance_replay_path_reference(&options.output_file, path)),
        post_change_certificate_file: options
            .post_change_certificate_file
            .as_deref()
            .map(|path| governance_replay_path_reference(&options.output_file, path)),
        expected_update_id: Some(update.update_id),
        expected_batch_id,
    };

    let package_json = serde_json::to_string_pretty(&package).map_err(invalid_data)?;
    let data_dir = options.data_dir;
    atomic_write_checked(
        &options.output_file,
        format!("{package_json}\n"),
        |package_file| {
            verify_governance_replay_package(GovernanceReplayVerifyOptions {
                data_dir: data_dir.clone(),
                package_file: package_file.to_path_buf(),
            })
            .map(|_| ())
        },
    )?;
    Ok(package)
}

pub fn verify_operator_manifest(
    options: OperatorManifestVerifyOptions,
) -> io::Result<OperatorManifestVerifyReport> {
    let manifest = read_operator_manifest_file(&options.manifest_file)?;
    verify_operator_manifest_record(&manifest, &options.manifest_file)
}

pub fn create_operator_manifest(
    options: OperatorManifestCreateOptions,
) -> io::Result<OperatorManifest> {
    ensure_output_can_be_written(&options.output_file, options.overwrite, "operator manifest")?;
    validate_private_file_permissions(&options.master_key_file, "operator manifest master key")?;
    let master_key = read_key_file(&options.master_key_file)?;
    decode_ml_dsa_65_public_key_hex(
        "operator manifest hot public key",
        &options.hot_public_key_hex,
    )?;
    let cobalt_trust = operator_cobalt_trust_binding_from_options(
        options.trust_graph_root,
        options.trust_graph_version,
        options.trust_view_id,
        options.trust_view_version,
    )?;

    let mut manifest = OperatorManifest {
        schema: OPERATOR_MANIFEST_FILE_SCHEMA.to_string(),
        chain_id: options.chain_id,
        network: options.network,
        validator_id: options.validator_id,
        master_public_key_hex: master_key.public_key_hex.clone(),
        hot_public_key_hex: options.hot_public_key_hex,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        key_role: "validator-hot".to_string(),
        operator: options.operator,
        contact: options.contact,
        infrastructure: OperatorInfrastructureLabels {
            provider_group: options.provider_group,
            region_group: options.region_group,
            jurisdiction_group: options.jurisdiction_group,
            legal_domain_group: options.legal_domain_group,
            funding_domain_group: options.funding_domain_group,
        },
        rotation_state: options.rotation_state,
        effective_height: options.effective_height,
        cobalt_trust,
        manifest_signing_key_hex: master_key.public_key_hex,
        signature_hex: String::new(),
        manifest_hash: String::new(),
    };
    validate_operator_manifest_fields_for_signing(&manifest)?;
    let private_key = Zeroizing::new(hex_to_bytes(&master_key.private_key_hex).map_err(invalid_data)?);
    let signing_payload = operator_manifest_signing_payload_bytes(&manifest)?;
    let signature = ml_dsa_65_sign_with_context(
        &private_key,
        &signing_payload,
        OPERATOR_MANIFEST_SIGNATURE_CONTEXT,
    )
    .map_err(invalid_data)?;
    manifest.signature_hex = bytes_to_hex(&signature);
    manifest.manifest_hash = operator_manifest_hash(&manifest)?;
    verify_operator_manifest_record(&manifest, &options.output_file)?;
    write_operator_manifest_file(&options.output_file, &manifest)?;
    Ok(manifest)
}

pub fn create_governance_genesis_bundle(
    options: GovernanceGenesisBundleOptions,
) -> io::Result<GovernanceGenesisBundle> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let genesis_hash = genesis_hash(&genesis);
    validate_active_validator_ids(&options.validators, "governance genesis validators")?;
    validate_governance_genesis_quorum(options.quorum, options.validators.len())?;
    validate_manifest_text_field("governance genesis network", &options.network)?;

    let registry = ensure_validator_registry_genesis(&store)?;
    let registry_records =
        validator_registry_subset_for_validators(&registry, &options.validators)?.validators;
    let registry_root = validator_registry_root(&registry, &options.validators)?;
    let output_parent = options
        .output_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));

    let mut operator_manifests = Vec::with_capacity(options.validators.len());
    for validator_id in &options.validators {
        let manifest_file = options
            .manifest_dir
            .join(format!("{validator_id}.operator-manifest.json"));
        let manifest = read_operator_manifest_file(&manifest_file)?;
        verify_operator_manifest_record(&manifest, &manifest_file)?;
        validate_operator_manifest_for_genesis(
            &manifest,
            &genesis,
            &options.network,
            validator_id,
            validator_registry_record(&registry, validator_id)?,
        )?;
        operator_manifests.push(GovernanceGenesisOperatorManifestRef {
            validator_id: validator_id.clone(),
            manifest_file: display_relative_artifact_path(output_parent, &manifest_file),
            manifest_hash: manifest.manifest_hash.clone(),
            hot_public_key_hex: manifest.hot_public_key_hex.clone(),
            provider_group: manifest.infrastructure.provider_group.clone(),
            region_group: manifest.infrastructure.region_group.clone(),
            jurisdiction_group: manifest.infrastructure.jurisdiction_group.clone(),
            legal_domain_group: manifest.infrastructure.legal_domain_group.clone(),
            funding_domain_group: manifest.infrastructure.funding_domain_group.clone(),
            cobalt_trust: manifest.cobalt_trust.clone(),
        });
    }
    validate_governance_genesis_cobalt_trust(&operator_manifests)?;

    let mut bundle = GovernanceGenesisBundle {
        schema: GOVERNANCE_GENESIS_BUNDLE_SCHEMA.to_string(),
        chain_id: genesis.chain_id,
        genesis_hash,
        protocol_version: genesis.protocol_version,
        network: options.network,
        validators: options.validators,
        validator_count: registry_records.len(),
        quorum: options.quorum,
        registry_root,
        registry_records,
        operator_manifests,
        bundle_hash: String::new(),
    };
    bundle.bundle_hash = governance_genesis_bundle_hash(&bundle)?;
    write_governance_genesis_bundle_file(&options.output_file, &bundle)?;
    Ok(bundle)
}

pub fn verify_governance_genesis_bundle(
    options: GovernanceGenesisVerifyOptions,
) -> io::Result<GovernanceGenesisVerifyReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let expected_genesis_hash = genesis_hash(&genesis);
    let bundle = read_governance_genesis_bundle_file(&options.bundle_file)?;
    if bundle.schema != GOVERNANCE_GENESIS_BUNDLE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported governance genesis bundle schema `{}`",
                bundle.schema
            ),
        ));
    }
    if bundle.chain_id != genesis.chain_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis bundle chain id mismatch",
        ));
    }
    if bundle.genesis_hash != expected_genesis_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis bundle genesis hash mismatch",
        ));
    }
    if bundle.protocol_version != genesis.protocol_version {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis bundle protocol version mismatch",
        ));
    }
    validate_manifest_text_field("governance genesis network", &bundle.network)?;
    validate_active_validator_ids(&bundle.validators, "governance genesis validators")?;
    validate_governance_genesis_quorum(bundle.quorum, bundle.validators.len())?;
    if bundle.validator_count != bundle.validators.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis bundle validator count mismatch",
        ));
    }

    let registry = ensure_validator_registry_genesis(&store)?;
    let expected_registry_records =
        validator_registry_subset_for_validators(&registry, &bundle.validators)?.validators;
    if bundle.registry_records != expected_registry_records {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis bundle registry records mismatch",
        ));
    }
    let expected_registry_root = validator_registry_root(&registry, &bundle.validators)?;
    if bundle.registry_root != expected_registry_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis bundle registry root mismatch",
        ));
    }
    if bundle.operator_manifests.len() != bundle.validators.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis bundle operator manifest count mismatch",
        ));
    }
    validate_governance_genesis_cobalt_trust(&bundle.operator_manifests)?;
    let expected_bundle_hash = governance_genesis_bundle_hash(&bundle)?;
    if bundle.bundle_hash != expected_bundle_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis bundle hash mismatch",
        ));
    }

    for (index, validator_id) in bundle.validators.iter().enumerate() {
        let manifest_ref = &bundle.operator_manifests[index];
        if manifest_ref.validator_id != *validator_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance genesis bundle operator manifest order mismatch",
            ));
        }
        let manifest_file =
            resolve_governance_genesis_path(&options.bundle_file, &manifest_ref.manifest_file)?;
        let manifest = read_operator_manifest_file(&manifest_file)?;
        verify_operator_manifest_record(&manifest, &manifest_file)?;
        validate_operator_manifest_for_genesis(
            &manifest,
            &genesis,
            &bundle.network,
            validator_id,
            validator_registry_record(&registry, validator_id)?,
        )?;
        validate_governance_genesis_manifest_ref(&manifest, manifest_ref)?;
    }

    Ok(GovernanceGenesisVerifyReport {
        schema: GOVERNANCE_GENESIS_VERIFY_REPORT_SCHEMA.to_string(),
        verified: true,
        bundle_file: options.bundle_file.display().to_string(),
        bundle_hash: bundle.bundle_hash,
        chain_id: bundle.chain_id,
        genesis_hash: bundle.genesis_hash,
        protocol_version: bundle.protocol_version,
        network: bundle.network,
        validators: bundle.validators,
        validator_count: bundle.validator_count,
        quorum: bundle.quorum,
        registry_root: bundle.registry_root,
        operator_manifest_count: bundle.operator_manifests.len(),
        operator_manifests_verified: true,
    })
}

pub fn apply_governance_batch(options: ApplyBatchOptions) -> io::Result<Vec<Receipt>> {
    apply_governance_batch_internal(options, None, false)
}

pub fn apply_governance_batch_with_replay(
    options: ApplyBatchOptions,
    replay_block_file: Option<PathBuf>,
) -> io::Result<Vec<Receipt>> {
    apply_governance_batch_internal(options, replay_block_file, false)
}

#[cfg(test)]
pub(crate) fn apply_unsigned_governance_fixture_for_test(
    options: ApplyBatchOptions,
) -> io::Result<Vec<Receipt>> {
    apply_governance_batch_internal(options, None, true)
}

fn apply_governance_batch_internal(
    options: ApplyBatchOptions,
    replay_block_file: Option<PathBuf>,
    allow_unsigned_test_fixture: bool,
) -> io::Result<Vec<Receipt>> {
    let store = NodeStore::new(&options.data_dir);
    let commit_lock = store.lock_ordered_commit()?;
    recover_ordered_commit_journal_locked(&store, &commit_lock)?;
    let genesis = store.read_genesis()?;
    let batch = read_governance_action_batch_file(&options.batch_file)?;
    verify_governance_action_batch_id(&genesis, &batch)?;

    let ordered_batches = store.read_ordered_batches()?;
    if ordered_batches.contains(&batch.batch_id) {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("governance batch `{}` already applied", batch.batch_id),
        ));
    }

    let mut governance = store.read_governance()?;
    let mut ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let bridge = store.read_bridge()?;
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let block_height = chain_tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let parent_hash = chain_tip.block_hash.clone();
    let due_activations = activate_due_validator_registry_updates_for_commit(
        &store,
        &genesis,
        &mut governance,
        block_height,
    )?;
    let certificate_validators = active_validator_ids(&governance)?;
    let certificate_material = read_commit_certificate_material(
        &store,
        &certificate_validators,
        options.certificate_file.as_deref(),
        None,
    )?;
    let (historical_replay, archived_payload_json) = historical_replay_commit_inputs(
        &certificate_material,
        replay_block_file.as_deref(),
        &options.batch_file,
    )?;
    let fastpay_pre_state_effects = match certificate_material.external_certificate.as_ref() {
        Some(certificate) => reconcile_certified_fastpay_pre_state_effects(
            &store,
            &mut ledger,
            &shielded,
            &certificate.fastpay_pre_state_effects,
        )?,
        None => fastpay_pre_state_effects_for_next_block(&store, &ledger)?,
    };
    if historical_replay.is_none() && !allow_unsigned_test_fixture {
        let registry = due_activations.registry.clone().unwrap_or(
            read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?,
        );
        verify_live_signed_governance_batch(
            &genesis,
            &governance,
            &registry,
            &batch,
            block_height,
        )?;
    }
    ensure_governance_batch_lifecycle_ready(&batch, block_height)?;
    let receipts = execute_governance_batch(
        &mut governance,
        Some(&mut ledger),
        &batch,
        block_height,
    );
    let mut proposed_ordered_batches = ordered_batches.clone();
    proposed_ordered_batches.push(batch.batch_id.clone());
    let consensus_proposal = build_block_proposal_from_state(BlockProposalPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &proposed_ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash: parent_hash.clone(),
        view: certificate_material
            .external_certificate
            .as_ref()
            .map_or(0, |certificate| certificate.view),
        batch_kind: BATCH_KIND_GOVERNANCE,
        batch_id: &batch.batch_id,
        payload: &batch,
        receipts: &receipts,
        fastpay_pre_state_effects: fastpay_pre_state_effects.clone(),
    })?;
    verify_consensus_v2_finality_requirement(
        store.data_dir(),
        &genesis,
        &consensus_proposal,
        certificate_material.external_certificate.as_ref(),
    )?;
    let commit = prepare_ordered_commit(OrderedCommitPlan {
        genesis: &genesis,
        governance: &governance,
        ledger: &ledger,
        ordered_batches: &ordered_batches,
        shielded: &shielded,
        bridge: &bridge,
        block_height,
        parent_hash,
        batch_kind: "governance",
        batch_id: &batch.batch_id,
        payload: &batch,
        batch_receipts: &receipts,
        archived_payload_json: archived_payload_json.as_deref(),
        validator_keys: certificate_material.validator_keys.as_ref(),
        external_certificate: certificate_material.external_certificate.as_ref(),
        external_validator_registry: certificate_material.external_validator_registry.as_ref(),
        external_certificate_preverified: false,
        historical_replay,
        certificate_validators: &certificate_validators,
        fastpay_pre_state_effects: &fastpay_pre_state_effects,
    })?;
    let live_registry_update =
        live_validator_registry_after_due_updates(&store, &genesis, &governance, commit.height)?;

    write_ordered_commit_with_journal_locked(
        &store,
        &commit_lock,
        OrderedCommitWrite {
            ledger: Some(ledger),
            governance: Some(governance),
            shielded: None,
            bridge: None,
            commit,
            validator_registry: live_registry_update.or(due_activations.registry),
        },
    )?;
    Ok(receipts)
}

const COBALT_MODE_CANONICAL: &str = "canonical";
const COBALT_MODE_NON_UNIFORM: &str = "non_uniform";

pub fn verify_governance(options: NodeOptions) -> io::Result<GovernanceVerificationReport> {
    verify_governance_with_options(GovernanceVerifyOptions {
        data_dir: options.data_dir,
        cobalt_mode: COBALT_MODE_CANONICAL.to_string(),
        trust_graph_root: None,
    })
}

pub fn verify_governance_with_options(
    options: GovernanceVerifyOptions,
) -> io::Result<GovernanceVerificationReport> {
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let mode = parse_cobalt_governance_mode(&options.cobalt_mode)?;
    let cobalt_mode = cobalt_governance_mode_name(mode).to_string();
    validate_cobalt_cutover_options(mode, options.trust_graph_root.as_deref())?;
    for amendment in &governance.amendments {
        verify_governance_amendment_evidence_for_mode(&genesis, amendment, mode)?;
    }
    verify_governance_amendment_activation_records(&genesis, &governance)?;
    verify_governance_amendment_supersession_records(&genesis, &governance)?;
    verify_governance_amendment_rollback_records(&genesis, &governance)?;
    let mut seen_registry_updates = HashSet::new();
    for update in &governance.validator_registry_updates {
        verify_historical_cobalt_validator_registry_update(&genesis, update)?;
        if !seen_registry_updates.insert(update.update_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate validator registry update in governance state",
            ));
        }
        if mode == CobaltGovernanceMode::NonUniform {
            validate_nonuniform_registry_update(update, options.trust_graph_root.as_deref())?;
        }
    }
    let mut seen_agent_dry_runs = HashSet::new();
    let mut expected_previous_dry_run_id = String::new();
    for record in &governance.governance_agent_dry_run_records {
        validate_governance_agent_dry_run_record(&expected_previous_dry_run_id, record)?;
        if record.chain_id != genesis.chain_id
            || record.genesis_hash != genesis_hash(&genesis)
            || record.protocol_version != genesis.protocol_version
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance agent dry-run record domain mismatch",
            ));
        }
        if !seen_agent_dry_runs.insert(record.dry_run_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate governance agent dry run in governance state",
            ));
        }
        expected_previous_dry_run_id = record.dry_run_id.clone();
    }
    Ok(GovernanceVerificationReport {
        verified: true,
        cobalt_mode,
        trust_graph_root: options.trust_graph_root,
        active_validator_count: governance.active_validator_count,
        active_validators: active_validator_ids(&governance)?,
        crypto_policy_version: governance.crypto_policy_version,
        bridge_witness_epoch: governance.bridge_witness_epoch,
        authority_mode: governance.authority_mode,
        amendment_count: governance.amendments.len(),
        latest_amendment_id: governance
            .amendments
            .last()
            .map(|amendment| amendment.amendment_id.clone())
            .unwrap_or_default(),
        amendment_activation_record_count: governance.amendment_activation_records.len(),
        latest_amendment_activation_record_id: governance
            .amendment_activation_records
            .last()
            .map(|record| record.activation_record_id.clone())
            .unwrap_or_default(),
        amendment_supersession_record_count: governance.amendment_supersession_records.len(),
        latest_amendment_supersession_record_id: governance
            .amendment_supersession_records
            .last()
            .map(|record| record.supersession_record_id.clone())
            .unwrap_or_default(),
        amendment_rollback_record_count: governance.amendment_rollback_records.len(),
        latest_amendment_rollback_record_id: governance
            .amendment_rollback_records
            .last()
            .map(|record| record.rollback_record_id.clone())
            .unwrap_or_default(),
        validator_registry_update_count: governance.validator_registry_updates.len(),
        latest_validator_registry_update_id: governance
            .validator_registry_updates
            .last()
            .map(|update| update.update_id.clone())
            .unwrap_or_default(),
        governance_agent_dry_run_count: governance.governance_agent_dry_run_records.len(),
        latest_governance_agent_dry_run_id: governance
            .governance_agent_dry_run_records
            .last()
            .map(|record| record.dry_run_id.clone())
            .unwrap_or_default(),
    })
}

fn parse_cobalt_governance_mode(raw: &str) -> io::Result<CobaltGovernanceMode> {
    match raw {
        COBALT_MODE_CANONICAL => Ok(CobaltGovernanceMode::Canonical),
        COBALT_MODE_NON_UNIFORM | "non-uniform" => Ok(CobaltGovernanceMode::NonUniform),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "--cobalt-mode must be canonical or non-uniform",
        )),
    }
}

fn cobalt_governance_mode_name(mode: CobaltGovernanceMode) -> &'static str {
    match mode {
        CobaltGovernanceMode::Canonical => COBALT_MODE_CANONICAL,
        CobaltGovernanceMode::NonUniform => COBALT_MODE_NON_UNIFORM,
    }
}

fn validate_cobalt_cutover_options(
    mode: CobaltGovernanceMode,
    trust_graph_root: Option<&str>,
) -> io::Result<()> {
    if mode == CobaltGovernanceMode::Canonical {
        if trust_graph_root.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "--trust-graph-root is only valid with --cobalt-mode non-uniform",
            ));
        }
        return Ok(());
    }
    let trust_graph_root = trust_graph_root.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "--cobalt-mode non-uniform requires --trust-graph-root",
        )
    })?;
    validate_lower_hex_96("trust graph root", trust_graph_root)
}

fn validate_lower_hex_96(label: &str, value: &str) -> io::Result<()> {
    let valid = value.len() == 96
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("{label} must be a 96-character lowercase hex hash"),
    ))
}

fn validate_chain_bound_id(label: &str, value: &str) -> io::Result<()> {
    validate_lower_hex_96(label, value)
}

fn validate_chain_id_text(label: &str, value: &str) -> io::Result<()> {
    if value.is_empty() || value.len() > MAX_TEXT_FIELD_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must be 1..={MAX_TEXT_FIELD_BYTES} bytes"),
        ));
    }
    if value != value.trim() || value.bytes().any(|byte| byte < 0x20 || byte == 0x7f) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must not contain whitespace edges or control bytes"),
        ));
    }
    Ok(())
}

fn verify_governance_amendment_evidence_for_mode(
    genesis: &Genesis,
    amendment: &GovernanceAmendment,
    mode: CobaltGovernanceMode,
) -> io::Result<()> {
    let domain = cobalt_domain(genesis);
    verify_governance_amendment_for_mode(&domain, amendment, mode)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn validate_nonuniform_registry_update(
    update: &ValidatorRegistryUpdateRecord,
    current_trust_graph_root: Option<&str>,
) -> io::Result<()> {
    let Some(previous_trust_graph_root) = update.previous_trust_graph_root.as_deref() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "non-uniform Cobalt registry update missing previous trust graph root",
        ));
    };
    let Some(new_trust_graph_root) = update.new_trust_graph_root.as_deref() else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "non-uniform Cobalt registry update missing new trust graph root",
        ));
    };
    if update.trust_graph_transition_id.as_deref().is_none() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "non-uniform Cobalt registry update missing trust graph transition id",
        ));
    }
    if let Some(current) = current_trust_graph_root {
        if previous_trust_graph_root != current && new_trust_graph_root != current {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "non-uniform Cobalt registry update is not bound to current trust graph root",
            ));
        }
    }
    Ok(())
}
