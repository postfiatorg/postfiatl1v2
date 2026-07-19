use super::*;

pub(super) fn bridge_witness_chain_domain_error(
    action: &BridgeTransferAction,
    genesis: &Genesis,
) -> Option<(&'static str, String)> {
    let attestation = action.witness_attestation.as_ref()?;
    let expected_genesis_hash = genesis_hash(genesis);
    if attestation.chain_id != genesis.chain_id
        || attestation.genesis_hash != expected_genesis_hash
        || attestation.protocol_version != genesis.protocol_version
    {
        return Some((
            "bad_witness_chain_domain",
            format!(
                "bridge witness attestation domain {}:{}:{} does not match chain {}:{}:{}",
                attestation.chain_id,
                attestation.genesis_hash,
                attestation.protocol_version,
                genesis.chain_id,
                expected_genesis_hash,
                genesis.protocol_version
            ),
        ));
    }
    None
}

pub(super) fn create_dev_key_file() -> io::Result<DevKeyFile> {
    let key_pair = ml_dsa_65_keygen().map_err(invalid_data)?;
    let public_key_hex = bytes_to_hex(&key_pair.public_key);
    Ok(DevKeyFile {
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        address: address_from_public_key(&key_pair.public_key),
        public_key_hex,
        private_key_hex: bytes_to_hex(&key_pair.private_key),
    })
}

pub(super) fn derive_wallet_dev_key_file(backup: &WalletBackupFile) -> io::Result<DevKeyFile> {
    validate_wallet_backup_file(backup)?;
    let seed = derive_wallet_seed(backup)?;
    let key_pair = ml_dsa_65_keygen_from_seed(&seed);
    let public_key_hex = bytes_to_hex(&key_pair.public_key);
    let key_file = DevKeyFile {
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        address: address_from_public_key(&key_pair.public_key),
        public_key_hex,
        private_key_hex: bytes_to_hex(&key_pair.private_key),
    };
    validate_dev_key_file(&key_file)?;
    Ok(key_file)
}

pub(super) fn derive_wallet_seed(backup: &WalletBackupFile) -> io::Result<[u8; 32]> {
    let master_seed = Zeroizing::new(wallet_master_seed_bytes(&backup.master_seed_hex)?);
    let derivation_payload = serde_json::to_vec(&(
        WALLET_DERIVATION_DOMAIN,
        backup.algorithm_id.as_str(),
        backup.chain_id.as_str(),
        backup.account_index,
        backup.key_role.as_str(),
        bytes_to_hex(&master_seed[..]),
    ))
    .map_err(invalid_data)?;
    let digest = hash_bytes(WALLET_DERIVATION_DOMAIN, &derivation_payload);
    let mut seed = [0u8; 32];
    seed.copy_from_slice(&digest[..32]);
    Ok(seed)
}

pub(super) fn normalized_wallet_master_seed_hex(seed_hex: &str) -> io::Result<String> {
    Ok(bytes_to_hex(&wallet_master_seed_bytes(seed_hex)?))
}

pub(super) fn wallet_master_seed_bytes(seed_hex: &str) -> io::Result<[u8; 32]> {
    let bytes = hex_to_bytes(seed_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("wallet master seed hex is invalid: {error}"),
        )
    })?;
    let seed: [u8; 32] = bytes.try_into().map_err(|bytes: Vec<u8>| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "wallet master seed hex must decode to 32 bytes, got {}",
                bytes.len()
            ),
        )
    })?;
    Ok(seed)
}

pub(super) fn wallet_test_vector_signature_seed_bytes(seed_hex: &str) -> io::Result<[u8; 32]> {
    let bytes = hex_to_bytes(seed_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("wallet test-vector signature seed hex is invalid: {error}"),
        )
    })?;
    let seed: [u8; 32] = bytes.try_into().map_err(|bytes: Vec<u8>| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "wallet test-vector signature seed hex must decode to 32 bytes, got {}",
                bytes.len()
            ),
        )
    })?;
    Ok(seed)
}

pub(super) fn fixed_32_byte_hex(label: &str, value: &str) -> io::Result<[u8; 32]> {
    let bytes = hex_to_bytes(value).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} hex is invalid: {error}"),
        )
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{label} hex must decode to 32 bytes, got {}", bytes.len()),
        )
    })
}

pub(super) fn mutate_first_hex_nibble(value: &str) -> String {
    let mut mutated = value.to_string();
    if let Some(first) = mutated.get(0..1) {
        let replacement = if first == "0" { "1" } else { "0" };
        mutated.replace_range(0..1, replacement);
    }
    mutated
}

pub(super) fn create_validator_key_record(node_id: String) -> io::Result<ValidatorKeyRecord> {
    let key_pair = ml_dsa_65_keygen().map_err(invalid_data)?;
    Ok(ValidatorKeyRecord {
        node_id,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&key_pair.public_key),
        private_key_hex: bytes_to_hex(&key_pair.private_key),
    })
}

pub(super) fn ensure_validator_keys(
    store: &NodeStore,
    validator_count: u32,
) -> io::Result<ValidatorKeyFile> {
    let validators = local_validator_ids(validator_count)?;
    ensure_validator_keys_for_validators(store, &validators)
}

pub(super) fn ensure_validator_keys_for_validators(
    store: &NodeStore,
    validators: &[String],
) -> io::Result<ValidatorKeyFile> {
    validate_active_validator_ids(validators, "required validator keys")?;
    let path = store.data_dir().join(VALIDATOR_KEYS_FILE);
    let registry_path = store.data_dir().join(VALIDATOR_REGISTRY_FILE);
    let registry_exists = registry_path.exists();
    let existing_registry = if registry_exists {
        Some(read_validator_registry_file(&registry_path)?)
    } else {
        None
    };
    let mut key_file = match read_validator_key_file(&path) {
        Ok(key_file) => key_file,
        Err(error) if error.kind() == io::ErrorKind::NotFound && registry_exists => {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!(
                    "validator private keys are missing for existing registry `{}`",
                    registry_path.display()
                ),
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => ValidatorKeyFile {
            validators: Vec::new(),
        },
        Err(error) => return Err(error),
    };
    validate_validator_key_file(&key_file)?;

    let mut changed = false;
    for node_id in validators {
        if key_file
            .validators
            .iter()
            .any(|record| record.node_id == *node_id)
        {
            continue;
        }
        if existing_registry
            .as_ref()
            .is_some_and(|registry| validator_registry_record(registry, node_id).is_ok())
        {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("validator private key `{node_id}` is missing for existing registry"),
            ));
        }
        key_file
            .validators
            .push(create_validator_key_record(node_id.clone())?);
        changed = true;
    }
    if changed {
        sort_validator_key_records(&mut key_file.validators);
        write_validator_key_file(&path, &key_file)?;
    } else {
        set_private_file_permissions(&path)?;
    }
    ensure_validator_registry_for_validators(store, &key_file, validators, changed)?;
    Ok(key_file)
}

pub(super) fn validate_validator_key_file(key_file: &ValidatorKeyFile) -> io::Result<()> {
    let mut seen = HashSet::new();
    for record in &key_file.validators {
        if !seen.insert(record.node_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate validator key `{}`", record.node_id),
            ));
        }
        if record.algorithm_id != ML_DSA_65_ALGORITHM {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "validator key `{}` uses unsupported algorithm `{}`",
                    record.node_id, record.algorithm_id
                ),
            ));
        }
        if record.public_key_hex.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("validator key `{}` has empty public key", record.node_id),
            ));
        }
        let public_key = hex_to_bytes(&record.public_key_hex).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "validator key `{}` has invalid public key hex: {error}",
                    record.node_id
                ),
            )
        })?;
        ml_dsa_65_validate_public_key(&public_key).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "validator key `{}` has invalid ML-DSA-65 public key: {error}",
                    record.node_id
                ),
            )
        })?;
        if record.private_key_hex.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("validator key `{}` has empty private key", record.node_id),
            ));
        }
        let private_key =
            Zeroizing::new(hex_to_bytes(&record.private_key_hex).map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "validator key `{}` has invalid private key hex: {error}",
                        record.node_id
                    ),
                )
            })?);
        let self_check_message = validator_key_self_check_message(record)?;
        let self_check_signature = ml_dsa_65_sign_with_context_seed(
            &private_key,
            &self_check_message,
            VALIDATOR_KEY_SELF_CHECK_CONTEXT,
            &VALIDATOR_KEY_SELF_CHECK_SEED,
        )
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "validator key `{}` has invalid ML-DSA-65 private key: {error}",
                    record.node_id
                ),
            )
        })?;
        if !ml_dsa_65_verify_with_context(
            &public_key,
            &self_check_message,
            &self_check_signature,
            VALIDATOR_KEY_SELF_CHECK_CONTEXT,
        ) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "validator key `{}` public/private key mismatch",
                    record.node_id
                ),
            ));
        }
    }
    Ok(())
}

pub(super) fn validator_key_self_check_message(record: &ValidatorKeyRecord) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&(
        "postfiat.validator_key.self_check.v1",
        record.node_id.as_str(),
        record.algorithm_id.as_str(),
        record.public_key_hex.as_str(),
    ))
    .map_err(invalid_data)
}

pub(super) fn validator_key_record<'a>(
    key_file: &'a ValidatorKeyFile,
    node_id: &str,
) -> io::Result<&'a ValidatorKeyRecord> {
    key_file
        .validators
        .iter()
        .find(|record| record.node_id == node_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing validator key `{node_id}`"),
            )
        })
}

pub(super) fn ensure_validator_registry_for_validators(
    store: &NodeStore,
    key_file: &ValidatorKeyFile,
    validators: &[String],
    allow_update: bool,
) -> io::Result<ValidatorRegistry> {
    let path = store.data_dir().join(VALIDATOR_REGISTRY_FILE);
    let registry = validator_registry_from_keys_for_validators(key_file, validators)?;
    validate_validator_registry(&registry)?;
    if path.exists() && !allow_update {
        let existing = read_validator_registry_file(&path)?;
        validate_validator_registry(&existing)?;
        let existing_subset = validator_registry_subset_for_validators(&existing, validators)?;
        if existing_subset != registry {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "validator registry does not match local validator private keys",
            ));
        }
    } else {
        write_validator_registry_file(&path, &registry)?;
    }
    Ok(registry)
}

pub(super) fn ensure_validator_registry_genesis(
    store: &NodeStore,
) -> io::Result<ValidatorRegistry> {
    let path = store.data_dir().join(VALIDATOR_REGISTRY_GENESIS_FILE);
    if path.exists() {
        let registry = read_validator_registry_file(&path)?;
        validate_validator_registry(&registry)?;
        return Ok(registry);
    }
    let registry = read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    write_validator_registry_file(&path, &registry)?;
    Ok(registry)
}

pub(super) fn read_validator_registry_replay_base(
    store: &NodeStore,
) -> io::Result<ValidatorRegistry> {
    match read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_GENESIS_FILE)) {
        Ok(registry) => Ok(registry),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))
        }
        Err(error) => Err(error),
    }
}

pub(super) fn validator_registry_from_keys_for_validators(
    key_file: &ValidatorKeyFile,
    validators: &[String],
) -> io::Result<ValidatorRegistry> {
    let mut records = Vec::with_capacity(validators.len());
    for validator in validators {
        let record = validator_key_record(key_file, validator)?;
        records.push(ValidatorRegistryRecord {
            node_id: record.node_id.clone(),
            algorithm_id: record.algorithm_id.clone(),
            public_key_hex: record.public_key_hex.clone(),
        });
    }
    Ok(ValidatorRegistry {
        validators: records,
    })
}

pub(super) fn validator_registry_subset_for_validators(
    registry: &ValidatorRegistry,
    validators: &[String],
) -> io::Result<ValidatorRegistry> {
    let mut records = Vec::with_capacity(validators.len());
    for validator in validators {
        records.push(validator_registry_record(registry, validator)?.clone());
    }
    Ok(ValidatorRegistry {
        validators: records,
    })
}

pub(super) fn validate_validator_registry(registry: &ValidatorRegistry) -> io::Result<()> {
    let mut seen = HashSet::new();
    for record in &registry.validators {
        if !seen.insert(record.node_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate validator registry key `{}`", record.node_id),
            ));
        }
        if record.algorithm_id != ML_DSA_65_ALGORITHM {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "validator registry key `{}` uses unsupported algorithm `{}`",
                    record.node_id, record.algorithm_id
                ),
            ));
        }
        if record.public_key_hex.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "validator registry key `{}` has empty public key",
                    record.node_id
                ),
            ));
        }
        let public_key = hex_to_bytes(&record.public_key_hex).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "validator registry key `{}` has invalid public key hex: {error}",
                    record.node_id
                ),
            )
        })?;
        ml_dsa_65_validate_public_key(&public_key).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "validator registry key `{}` has invalid ML-DSA-65 public key: {error}",
                    record.node_id
                ),
            )
        })?;
    }
    Ok(())
}

pub(super) fn validate_validator_registry_for_count(
    registry: &ValidatorRegistry,
    validator_count: u32,
) -> io::Result<()> {
    validate_validator_registry(registry)?;
    if registry.validators.len() < validator_count as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "validator registry has {} keys, expected at least {}",
                registry.validators.len(),
                validator_count
            ),
        ));
    }
    for node_id in local_validator_ids(validator_count)? {
        validator_registry_record(registry, &node_id)?;
    }
    Ok(())
}

pub(super) fn validator_registry_record<'a>(
    registry: &'a ValidatorRegistry,
    node_id: &str,
) -> io::Result<&'a ValidatorRegistryRecord> {
    registry
        .validators
        .iter()
        .find(|record| record.node_id == node_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing validator registry key `{node_id}`"),
            )
        })
}

pub(super) fn validator_registry_root(
    registry: &ValidatorRegistry,
    expected_validators: &[String],
) -> io::Result<String> {
    validate_validator_registry(registry)?;
    let records = expected_validators
        .iter()
        .map(|node_id| {
            let record = validator_registry_record(registry, node_id)?;
            Ok((
                record.node_id.as_str(),
                record.algorithm_id.as_str(),
                record.public_key_hex.as_str(),
            ))
        })
        .collect::<io::Result<Vec<_>>>()?;
    let encoded = serde_json::to_vec(&records).map_err(invalid_data)?;
    Ok(hash_hex("postfiat.validator_registry.root.v1", &encoded))
}

pub(super) fn validator_registry_update_previous_validators(
    update: &ValidatorRegistryUpdateRecord,
) -> Vec<String> {
    if update.previous_validators.is_empty() {
        update.validators.clone()
    } else {
        update.previous_validators.clone()
    }
}

pub(super) fn validator_registry_update_new_validators(
    update: &ValidatorRegistryUpdateRecord,
) -> Vec<String> {
    if update.new_validators.is_empty() {
        update.validators.clone()
    } else {
        update.new_validators.clone()
    }
}

pub(super) fn apply_verified_validator_registry_update_to_registry(
    genesis: &Genesis,
    registry: &mut ValidatorRegistry,
    update: &ValidatorRegistryUpdateRecord,
    current_height: u64,
    context: &str,
) -> io::Result<Vec<String>> {
    let domain = cobalt_domain(genesis);
    apply_verified_validator_registry_update_to_registry_for_domain(
        &domain,
        registry,
        update,
        current_height,
        context,
    )
}

pub(super) fn apply_historical_validator_registry_update_to_registry(
    genesis: &Genesis,
    registry: &mut ValidatorRegistry,
    update: &ValidatorRegistryUpdateRecord,
    current_height: u64,
    context: &str,
) -> io::Result<Vec<String>> {
    verify_historical_cobalt_validator_registry_update(genesis, update)?;
    apply_verified_validator_registry_update_to_registry_inner(
        registry,
        update,
        current_height,
        context,
    )
}

pub(super) fn apply_verified_validator_registry_update_to_registry_for_domain(
    domain: &CobaltDomain,
    registry: &mut ValidatorRegistry,
    update: &ValidatorRegistryUpdateRecord,
    current_height: u64,
    context: &str,
) -> io::Result<Vec<String>> {
    verify_cobalt_validator_registry_update(domain, update)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    apply_verified_validator_registry_update_to_registry_inner(
        registry,
        update,
        current_height,
        context,
    )
}

pub(super) fn apply_verified_validator_registry_update_to_registry_inner(
    registry: &mut ValidatorRegistry,
    update: &ValidatorRegistryUpdateRecord,
    current_height: u64,
    context: &str,
) -> io::Result<Vec<String>> {
    if current_height < update.activation_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{context} validator registry update activation height not reached"),
        ));
    }
    let previous_validators = validator_registry_update_previous_validators(update);
    let new_validators = validator_registry_update_new_validators(update);
    let previous_root = validator_registry_root(registry, &previous_validators)?;
    if previous_root != update.previous_registry_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{context} previous validator registry root mismatch"),
        ));
    }
    apply_validator_registry_update_to_registry(registry, update)?;
    sort_validator_registry_records(&mut registry.validators);
    validate_validator_registry(registry)?;
    let new_root = validator_registry_root(registry, &new_validators)?;
    if new_root != update.new_registry_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{context} new validator registry root mismatch"),
        ));
    }
    Ok(new_validators)
}

pub(super) fn verify_historical_cobalt_validator_registry_update(
    genesis: &Genesis,
    update: &ValidatorRegistryUpdateRecord,
) -> io::Result<()> {
    let canonical_domain = cobalt_domain(genesis);
    match verify_cobalt_validator_registry_update(&canonical_domain, update) {
        Ok(()) => return Ok(()),
        Err(error) if error == "validator registry update domain mismatch" => {}
        Err(error) => return Err(io::Error::new(io::ErrorKind::InvalidData, error)),
    }
    let embedded_domain = cobalt_domain_from_validator_registry_update(update);
    if embedded_domain.chain_id != genesis.chain_id
        || embedded_domain.protocol_version != genesis.protocol_version
        || embedded_domain.genesis_hash == canonical_domain.genesis_hash
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "historical validator registry update domain mismatch",
        ));
    }
    verify_cobalt_validator_registry_update(&embedded_domain, update)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) fn cobalt_domain_from_validator_registry_update(
    update: &ValidatorRegistryUpdateRecord,
) -> CobaltDomain {
    CobaltDomain {
        chain_id: update.chain_id.clone(),
        genesis_hash: update.genesis_hash.clone(),
        protocol_version: update.protocol_version,
    }
}

pub(super) fn apply_validator_registry_update_to_registry(
    registry: &mut ValidatorRegistry,
    update: &ValidatorRegistryUpdateRecord,
) -> io::Result<()> {
    match update.operation.as_str() {
        VALIDATOR_REGISTRY_OP_ADMIT | VALIDATOR_REGISTRY_OP_REACTIVATE => {
            if registry
                .validators
                .iter()
                .any(|record| record.node_id == update.subject_node_id)
            {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "validator registry update subject already active",
                ));
            }
            let new_record = update.new_record.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "validator registry update missing new record",
                )
            })?;
            if !new_record.active {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "validator registry update new record is not active",
                ));
            }
            registry
                .validators
                .push(validator_registry_record_from_entry(new_record));
        }
        VALIDATOR_REGISTRY_OP_REMOVE | VALIDATOR_REGISTRY_OP_SUSPEND => {
            let previous_record = update.previous_record.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "validator registry update missing previous record",
                )
            })?;
            let index = registry_validator_index(registry, &update.subject_node_id)?;
            ensure_registry_record_matches_entry(
                &registry.validators[index],
                previous_record,
                "previous",
            )?;
            registry.validators.remove(index);
        }
        VALIDATOR_REGISTRY_OP_ROTATE_KEY => {
            let previous_record = update.previous_record.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "validator registry update missing previous record",
                )
            })?;
            let new_record = update.new_record.as_ref().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "validator registry update missing new record",
                )
            })?;
            let index = registry_validator_index(registry, &update.subject_node_id)?;
            ensure_registry_record_matches_entry(
                &registry.validators[index],
                previous_record,
                "previous",
            )?;
            if !new_record.active {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "validator registry key rotation new record is not active",
                ));
            }
            registry.validators[index] = validator_registry_record_from_entry(new_record);
        }
        other => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported validator registry update operation `{other}`"),
            ));
        }
    }
    Ok(())
}

pub(super) fn registry_validator_index(
    registry: &ValidatorRegistry,
    node_id: &str,
) -> io::Result<usize> {
    registry
        .validators
        .iter()
        .position(|record| record.node_id == node_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("validator registry update subject `{node_id}` is not active"),
            )
        })
}

pub(super) fn validator_registry_record_from_entry(
    entry: &ValidatorRegistryEntry,
) -> ValidatorRegistryRecord {
    ValidatorRegistryRecord {
        node_id: entry.node_id.clone(),
        algorithm_id: entry.algorithm_id.clone(),
        public_key_hex: entry.public_key_hex.clone(),
    }
}

pub(super) fn ensure_registry_record_matches_entry(
    record: &ValidatorRegistryRecord,
    entry: &ValidatorRegistryEntry,
    label: &str,
) -> io::Result<()> {
    if !entry.active
        || record.node_id != entry.node_id
        || record.algorithm_id != entry.algorithm_id
        || record.public_key_hex != entry.public_key_hex
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("validator registry update {label} record mismatch"),
        ));
    }
    Ok(())
}

pub(super) fn sort_validator_registry_records(records: &mut [ValidatorRegistryRecord]) {
    records.sort_by(|left, right| {
        validator_index(&left.node_id)
            .cmp(&validator_index(&right.node_id))
            .then_with(|| left.node_id.cmp(&right.node_id))
    });
}

pub(super) fn sort_validator_key_records(records: &mut [ValidatorKeyRecord]) {
    records.sort_by(|left, right| {
        validator_index(&left.node_id)
            .cmp(&validator_index(&right.node_id))
            .then_with(|| left.node_id.cmp(&right.node_id))
    });
}

pub(super) fn validator_index(node_id: &str) -> Option<u32> {
    node_id.strip_prefix("validator-")?.parse().ok()
}

pub(super) fn build_signed_transfer(
    genesis: &Genesis,
    ledger: &LedgerState,
    data_dir: &Path,
    key_file: Option<PathBuf>,
    to: String,
    amount: u64,
) -> io::Result<SignedTransfer> {
    let key_file = read_transfer_key_file(data_dir, key_file)?;
    let sender = ledger.account(&key_file.address).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("sender account `{}` not found", key_file.address),
        )
    })?;
    let sequence = sender.sequence.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "sender account `{}` sequence is exhausted",
                key_file.address
            ),
        )
    })?;
    build_signed_transfer_for_key(genesis, ledger, &key_file, to, amount, sequence)
}

pub(super) fn build_signed_transfer_for_key(
    genesis: &Genesis,
    ledger: &LedgerState,
    key_file: &DevKeyFile,
    to: String,
    amount: u64,
    sequence: u64,
) -> io::Result<SignedTransfer> {
    let _sender = ledger.account(&key_file.address).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("sender account `{}` not found", key_file.address),
        )
    })?;

    let private_key =
        Zeroizing::new(hex_to_bytes(&key_file.private_key_hex).map_err(invalid_data)?);
    let mut fee = postfiat_execution::MIN_TRANSFER_FEE;
    for _ in 0..8 {
        let unsigned = UnsignedTransfer {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
            transaction_kind: postfiat_types::TRANSFER_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            from: key_file.address.clone(),
            to: to.clone(),
            amount,
            fee,
            sequence,
        };
        unsigned
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        let signature =
            ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).map_err(invalid_data)?;
        let signed = SignedTransfer {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: key_file.public_key_hex.clone(),
            signature_hex: bytes_to_hex(&signature),
        };
        let minimum_fee = minimum_transfer_fee_for_ledger(ledger, &signed);
        if fee >= minimum_fee {
            return Ok(signed);
        }
        fee = minimum_fee;
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "minimum transfer fee did not converge",
    ))
}

pub(super) fn quote_signed_transfer(
    genesis: &Genesis,
    from: String,
    to: String,
    amount: u64,
    fee: u64,
    sequence: u64,
) -> io::Result<SignedTransfer> {
    let unsigned = UnsignedTransfer {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: postfiat_types::TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from,
        to,
        amount,
        fee,
        sequence,
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(SignedTransfer {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: "0".repeat(ML_DSA_65_PUBLIC_KEY_BYTES.saturating_mul(2)),
        signature_hex: "0".repeat(ML_DSA_65_SIGNATURE_BYTES.saturating_mul(2)),
    })
}

pub(super) fn payment_v2_quote_memos(
    options: &TransferFeeQuoteOptions,
) -> io::Result<Vec<PaymentMemo>> {
    if options.memo_type.is_none() && options.memo_format.is_none() && options.memo_data.is_none() {
        return Ok(Vec::new());
    }
    let memo = PaymentMemo {
        memo_type: options.memo_type.clone().unwrap_or_default(),
        memo_format: options.memo_format.clone().unwrap_or_default(),
        memo_data: options.memo_data.clone().unwrap_or_default(),
    };
    memo.validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(vec![memo])
}

pub(super) fn quote_signed_payment_v2(
    genesis: &Genesis,
    from: String,
    to: String,
    amount: u64,
    fee: u64,
    sequence: u64,
    memos: Vec<PaymentMemo>,
) -> io::Result<SignedPaymentV2> {
    let unsigned = UnsignedPaymentV2 {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: PAYMENT_V2_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from,
        to,
        amount,
        fee,
        sequence,
        memos,
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(SignedPaymentV2 {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: "0".repeat(ML_DSA_65_PUBLIC_KEY_BYTES.saturating_mul(2)),
        signature_hex: "0".repeat(ML_DSA_65_SIGNATURE_BYTES.saturating_mul(2)),
    })
}

pub(super) fn quote_signed_asset_transaction(
    genesis: &Genesis,
    source: String,
    fee: u64,
    sequence: u64,
    operation: AssetTransactionOperation,
) -> io::Result<SignedAssetTransaction> {
    let transaction_kind = operation.transaction_kind().to_string();
    let unsigned = UnsignedAssetTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind,
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source,
        fee,
        sequence,
        operation,
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(SignedAssetTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: "0".repeat(ML_DSA_65_PUBLIC_KEY_BYTES.saturating_mul(2)),
        signature_hex: "0".repeat(ML_DSA_65_SIGNATURE_BYTES.saturating_mul(2)),
    })
}

pub(super) fn quote_signed_escrow_transaction(
    genesis: &Genesis,
    source: String,
    fee: u64,
    sequence: u64,
    operation: EscrowTransactionOperation,
) -> io::Result<SignedEscrowTransaction> {
    let transaction_kind = operation.transaction_kind().to_string();
    let unsigned = UnsignedEscrowTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind,
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source,
        fee,
        sequence,
        operation,
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(SignedEscrowTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: "0".repeat(ML_DSA_65_PUBLIC_KEY_BYTES.saturating_mul(2)),
        signature_hex: "0".repeat(ML_DSA_65_SIGNATURE_BYTES.saturating_mul(2)),
    })
}

pub(super) fn quote_signed_nft_transaction(
    genesis: &Genesis,
    source: String,
    fee: u64,
    sequence: u64,
    operation: NftTransactionOperation,
) -> io::Result<SignedNftTransaction> {
    let transaction_kind = operation.transaction_kind().to_string();
    let unsigned = UnsignedNftTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind,
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source,
        fee,
        sequence,
        operation,
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(SignedNftTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: "0".repeat(ML_DSA_65_PUBLIC_KEY_BYTES.saturating_mul(2)),
        signature_hex: "0".repeat(ML_DSA_65_SIGNATURE_BYTES.saturating_mul(2)),
    })
}

pub(super) fn quote_signed_offer_transaction(
    genesis: &Genesis,
    source: String,
    fee: u64,
    sequence: u64,
    operation: OfferTransactionOperation,
) -> io::Result<SignedOfferTransaction> {
    let transaction_kind = operation.transaction_kind().to_string();
    let unsigned = UnsignedOfferTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind,
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source,
        fee,
        sequence,
        operation,
    };
    unsigned
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    Ok(SignedOfferTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: "0".repeat(ML_DSA_65_PUBLIC_KEY_BYTES.saturating_mul(2)),
        signature_hex: "0".repeat(ML_DSA_65_SIGNATURE_BYTES.saturating_mul(2)),
    })
}

pub(super) fn read_transfer_key_file(
    data_dir: &Path,
    key_file: Option<PathBuf>,
) -> io::Result<DevKeyFile> {
    read_key_file(&key_file.unwrap_or_else(|| data_dir.join(FAUCET_KEY_FILE)))
}

pub(super) fn next_pending_sender_sequence(
    mempool: &MempoolState,
    sender: &str,
    ledger_sequence: u64,
) -> io::Result<u64> {
    let pending_sequence = mempool
        .pending
        .iter()
        .filter_map(|entry| {
            (entry.transfer.unsigned.from == sender).then_some(entry.transfer.unsigned.sequence)
        })
        .chain(mempool.pending_payment_v2.iter().filter_map(|entry| {
            (entry.payment.unsigned.from == sender).then_some(entry.payment.unsigned.sequence)
        }))
        .chain(
            mempool
                .pending_asset_transactions
                .iter()
                .filter_map(|entry| {
                    (entry.transaction.unsigned.source == sender)
                        .then_some(entry.transaction.unsigned.sequence)
                }),
        )
        .chain(mempool.pending_atomic_swaps.iter().flat_map(|entry| {
            [
                &entry.transaction.unsigned.leg_0,
                &entry.transaction.unsigned.leg_1,
            ]
            .into_iter()
            .filter_map(|leg| (leg.owner == sender).then_some(leg.sequence))
        }))
        .chain(mempool.pending_fastlane_primary.iter().filter_map(|entry| {
            match &entry.transaction.operation {
                postfiat_types::FastLanePrimaryOperationV1::Deposit { signed }
                    if signed.deposit.source_address == sender =>
                {
                    Some(signed.deposit.sequence)
                }
                postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit { signed }
                    if signed.deposit.source_address == sender =>
                {
                    Some(signed.deposit.sequence)
                }
                _ => None,
            }
        }))
        .chain(
            mempool
                .pending_escrow_transactions
                .iter()
                .filter_map(|entry| {
                    (entry.transaction.unsigned.source == sender)
                        .then_some(entry.transaction.unsigned.sequence)
                }),
        )
        .chain(mempool.pending_nft_transactions.iter().filter_map(|entry| {
            (entry.transaction.unsigned.source == sender)
                .then_some(entry.transaction.unsigned.sequence)
        }))
        .chain(
            mempool
                .pending_offer_transactions
                .iter()
                .filter_map(|entry| {
                    (entry.transaction.unsigned.source == sender)
                        .then_some(entry.transaction.unsigned.sequence)
                }),
        )
        .max()
        .unwrap_or(ledger_sequence);
    pending_sequence.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("sender `{sender}` sequence is exhausted"),
        )
    })
}

pub(super) fn write_key_file(path: &Path, key_file: &DevKeyFile) -> io::Result<()> {
    validate_dev_key_file(key_file)?;
    let json = serde_json::to_string_pretty(key_file).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))?;
    set_private_file_permissions(path)
}

pub(super) fn read_key_file(path: &Path) -> io::Result<DevKeyFile> {
    validate_private_file_permissions(path, "development key")?;
    let key_file = read_json_file(path, "development key")?;
    validate_dev_key_file(&key_file)?;
    Ok(key_file)
}

pub(super) fn write_wallet_backup_file(path: &Path, backup: &WalletBackupFile) -> io::Result<()> {
    validate_wallet_backup_file(backup)?;
    let json = serde_json::to_string_pretty(backup).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))?;
    set_private_file_permissions(path)
}

pub(super) fn read_wallet_backup_file(path: &Path) -> io::Result<WalletBackupFile> {
    validate_private_file_permissions(path, "wallet backup")?;
    let backup = read_json_file(path, "wallet backup")?;
    validate_wallet_backup_file(&backup)?;
    Ok(backup)
}

pub(super) fn write_orchard_wallet_key_file(
    path: &Path,
    key_file: &OrchardWalletKeyFile,
) -> io::Result<()> {
    validate_orchard_wallet_key_file(key_file)?;
    let json = serde_json::to_string_pretty(key_file).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))?;
    set_private_file_permissions(path)
}

pub(super) fn read_orchard_wallet_key_file(path: &Path) -> io::Result<OrchardWalletKeyFile> {
    validate_private_file_permissions(path, "Orchard wallet key file")?;
    let key_file = read_json_file(path, "Orchard wallet key")?;
    validate_orchard_wallet_key_file(&key_file)?;
    Ok(key_file)
}

pub(super) fn write_orchard_view_key_file(
    path: &Path,
    view_key: &OrchardViewKeyFile,
) -> io::Result<()> {
    validate_orchard_view_key_file(view_key)?;
    let json = serde_json::to_string_pretty(view_key).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))?;
    set_private_file_permissions(path)
}

pub(super) fn read_orchard_view_key_file(path: &Path) -> io::Result<OrchardViewKeyFile> {
    validate_private_file_permissions(path, "Orchard view key file")?;
    let view_key = read_json_file(path, "Orchard view key")?;
    validate_orchard_view_key_file(&view_key)?;
    Ok(view_key)
}

pub(super) fn validate_wallet_backup_file(backup: &WalletBackupFile) -> io::Result<()> {
    if backup.schema != WALLET_BACKUP_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("wallet backup uses unsupported schema `{}`", backup.schema),
        ));
    }
    if backup.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "wallet backup uses unsupported algorithm `{}`",
                backup.algorithm_id
            ),
        ));
    }
    if backup.kdf != WALLET_DERIVATION_KDF {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("wallet backup uses unsupported KDF `{}`", backup.kdf),
        ));
    }
    if backup.derivation_domain != WALLET_DERIVATION_DOMAIN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "wallet backup uses unsupported derivation domain `{}`",
                backup.derivation_domain
            ),
        ));
    }
    if backup.chain_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wallet backup chain id is empty",
        ));
    }
    if backup.key_role != WALLET_KEY_ROLE_TRANSPARENT_SPEND {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "wallet backup uses unsupported key role `{}`",
                backup.key_role
            ),
        ));
    }
    let normalized_seed = normalized_wallet_master_seed_hex(&backup.master_seed_hex)?;
    if backup.master_seed_hex != normalized_seed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "wallet backup master seed must be lowercase canonical hex",
        ));
    }
    Ok(())
}

pub(super) fn validate_orchard_wallet_key_file(key_file: &OrchardWalletKeyFile) -> io::Result<()> {
    if key_file.schema != ORCHARD_WALLET_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Orchard wallet key uses unsupported schema `{}`",
                key_file.schema
            ),
        ));
    }
    if key_file.kdf != ORCHARD_WALLET_DERIVATION_KDF {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("Orchard wallet key uses unsupported KDF `{}`", key_file.kdf),
        ));
    }
    if key_file.derivation_domain != ORCHARD_WALLET_DERIVATION_DOMAIN {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Orchard wallet key uses unsupported derivation domain `{}`",
                key_file.derivation_domain
            ),
        ));
    }
    let spending_key = orchard_spending_key_bytes(&key_file.spending_key_hex)?;
    let expected_address =
        orchard_default_address_from_spending_key(spending_key).map_err(invalid_data)?;
    if key_file.address_raw_hex != expected_address {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard wallet key address does not match spending key",
        ));
    }
    Ok(())
}

pub(super) fn validate_orchard_view_key_file(view_key: &OrchardViewKeyFile) -> io::Result<()> {
    if view_key.schema != ORCHARD_VIEW_KEY_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Orchard view key uses unsupported schema `{}`",
                view_key.schema
            ),
        ));
    }
    if view_key.source_schema != ORCHARD_WALLET_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "Orchard view key uses unsupported source schema `{}`",
                view_key.source_schema
            ),
        ));
    }
    let full_viewing_key = orchard_full_viewing_key_bytes(&view_key.full_viewing_key_hex)?;
    let expected_address =
        orchard_default_address_from_full_viewing_key(full_viewing_key).map_err(invalid_data)?;
    if view_key.address_raw_hex != expected_address {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "Orchard view key address does not match full viewing key",
        ));
    }
    Ok(())
}

pub(super) fn wallet_key_report(
    operation: &str,
    backup: &WalletBackupFile,
    key_file: &DevKeyFile,
    key_file_path: &Path,
    backup_file_path: Option<&Path>,
) -> WalletKeyReport {
    WalletKeyReport {
        schema: WALLET_KEY_REPORT_SCHEMA.to_string(),
        operation: operation.to_string(),
        algorithm_id: key_file.algorithm_id.clone(),
        kdf: backup.kdf.clone(),
        derivation_domain: backup.derivation_domain.clone(),
        chain_id: backup.chain_id.clone(),
        account_index: backup.account_index,
        key_role: backup.key_role.clone(),
        address: key_file.address.clone(),
        public_key_hex: key_file.public_key_hex.clone(),
        key_file: key_file_path.display().to_string(),
        backup_file: backup_file_path.map(|path| path.display().to_string()),
        private_key_material_redacted: true,
    }
}

pub(super) fn ensure_output_can_be_written(
    path: &Path,
    overwrite: bool,
    label: &str,
) -> io::Result<()> {
    if path.exists() && !overwrite {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!(
                "{label} `{}` already exists; pass --overwrite",
                path.display()
            ),
        ));
    }
    Ok(())
}

pub(super) fn validate_dev_key_file(key_file: &DevKeyFile) -> io::Result<()> {
    if key_file.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "development key uses unsupported algorithm `{}`",
                key_file.algorithm_id
            ),
        ));
    }
    if key_file.address.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "development key address is empty",
        ));
    }
    if key_file.public_key_hex.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "development key public key is empty",
        ));
    }
    let public_key = hex_to_bytes(&key_file.public_key_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("development key public key has invalid hex: {error}"),
        )
    })?;
    ml_dsa_65_validate_public_key(&public_key).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("development key public key is invalid ML-DSA-65: {error}"),
        )
    })?;
    let expected_address = address_from_public_key(&public_key);
    if key_file.address != expected_address {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "development key address does not match public key",
        ));
    }
    if key_file.private_key_hex.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "development key private key is empty",
        ));
    }
    let private_key = Zeroizing::new(hex_to_bytes(&key_file.private_key_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("development key private key has invalid hex: {error}"),
        )
    })?);
    let self_check_message = dev_key_self_check_message(key_file)?;
    let self_check_signature = ml_dsa_65_sign_with_context_seed(
        &private_key,
        &self_check_message,
        DEV_KEY_SELF_CHECK_CONTEXT,
        &DEV_KEY_SELF_CHECK_SEED,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("development key private key is invalid ML-DSA-65: {error}"),
        )
    })?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        &self_check_message,
        &self_check_signature,
        DEV_KEY_SELF_CHECK_CONTEXT,
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "development key public/private key mismatch",
        ));
    }
    Ok(())
}

pub(super) fn dev_key_self_check_message(key_file: &DevKeyFile) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&(
        "postfiat.dev_key.self_check.v1",
        key_file.algorithm_id.as_str(),
        key_file.address.as_str(),
        key_file.public_key_hex.as_str(),
    ))
    .map_err(invalid_data)
}

pub(super) fn write_faucet_account_file(path: &Path, account: &Account) -> io::Result<()> {
    validate_faucet_account(account)?;
    let json = serde_json::to_string_pretty(account).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_faucet_account_file(path: &Path) -> io::Result<Account> {
    let account = read_json_file(path, "faucet account")?;
    validate_faucet_account(&account)?;
    Ok(account)
}

pub(super) fn validate_faucet_account(account: &Account) -> io::Result<()> {
    if account.address.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "faucet account address is empty",
        ));
    }
    if account.balance != DEFAULT_FAUCET_BALANCE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("genesis native supply must equal {DEFAULT_FAUCET_BALANCE} atoms"),
        ));
    }
    if account.sequence != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "faucet account sequence must start at zero",
        ));
    }
    let Some(public_key_hex) = account
        .public_key_hex
        .as_ref()
        .filter(|public_key| !public_key.is_empty())
    else {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "faucet account public key is empty",
        ));
    };
    let public_key = hex_to_bytes(public_key_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("faucet account public key has invalid hex: {error}"),
        )
    })?;
    ml_dsa_65_validate_public_key(&public_key).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("faucet account public key is invalid ML-DSA-65: {error}"),
        )
    })?;
    let expected_address = address_from_public_key(&public_key);
    if account.address != expected_address {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "faucet account address does not match public key",
        ));
    }
    Ok(())
}

pub(super) fn write_validator_key_file(path: &Path, key_file: &ValidatorKeyFile) -> io::Result<()> {
    validate_validator_key_file(key_file)?;
    let json = serde_json::to_string_pretty(key_file).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))?;
    set_private_file_permissions(path)
}

pub(super) fn read_validator_key_file(path: &Path) -> io::Result<ValidatorKeyFile> {
    validate_private_file_permissions(path, "validator key file")?;
    let key_file = read_json_file(path, "validator key file")?;
    validate_validator_key_file(&key_file)?;
    Ok(key_file)
}

pub(super) fn write_block_proposal_file(
    path: &Path,
    proposal: &BlockProposalFile,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(proposal).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_block_proposal_file(path: &Path) -> io::Result<BlockProposalFile> {
    read_json_file(path, "block proposal")
}

#[allow(dead_code)]
pub(super) fn write_block_vote_file(path: &Path, vote: &BlockVoteFile) -> io::Result<()> {
    let json = serde_json::to_string_pretty(vote).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_block_vote_file(path: &Path) -> io::Result<BlockVoteFile> {
    read_json_file(path, "block vote")
}

pub(super) fn write_block_certificate_file(
    path: &Path,
    certificate: &BlockCertificateFile,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(certificate).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_block_certificate_file(path: &Path) -> io::Result<BlockCertificateFile> {
    read_json_file(path, "block certificate")
}

pub(super) fn write_block_timeout_vote_file(
    path: &Path,
    vote: &BlockTimeoutVoteFile,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(vote).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_block_timeout_vote_file(path: &Path) -> io::Result<BlockTimeoutVoteFile> {
    read_json_file(path, "block timeout vote")
}

pub(super) fn write_block_timeout_certificate_file(
    path: &Path,
    certificate: &BlockTimeoutCertificateFile,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(certificate).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_block_timeout_certificate_file(
    path: &Path,
) -> io::Result<BlockTimeoutCertificateFile> {
    read_json_file(path, "block timeout certificate")
}

pub(super) fn write_block_equivocation_evidence_file(
    path: &Path,
    evidence: &BlockEquivocationEvidenceFile,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(evidence).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub fn read_block_equivocation_evidence_file(
    path: &Path,
) -> io::Result<BlockEquivocationEvidenceFile> {
    read_json_file(path, "block equivocation evidence")
}

pub(super) struct CommitCertificateMaterial {
    pub(super) validator_keys: Option<ValidatorKeyFile>,
    pub(super) external_certificate: Option<BlockCertificateFile>,
    pub(super) external_validator_registry: Option<ValidatorRegistry>,
    pub(super) external_certificate_preverified: bool,
}

pub(super) fn read_commit_certificate_material(
    store: &NodeStore,
    validators: &[String],
    certificate_file: Option<&Path>,
    verified_certificate: Option<&VerifiedBlockCertificateFile>,
) -> io::Result<CommitCertificateMaterial> {
    if let Some(verified_certificate) = verified_certificate {
        let external_certificate = verified_certificate.as_block_certificate_file().clone();
        if let Some(certificate_file) = certificate_file {
            let disk_certificate = read_block_certificate_file(certificate_file)?;
            if disk_certificate != external_certificate {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "verified block certificate does not match certificate file",
                ));
            }
        }
        let external_validator_registry =
            read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
        return Ok(CommitCertificateMaterial {
            validator_keys: None,
            external_certificate: Some(external_certificate),
            external_validator_registry: Some(external_validator_registry),
            external_certificate_preverified: true,
        });
    }
    let external_certificate = certificate_file
        .map(read_block_certificate_file)
        .transpose()?;
    if external_certificate.is_some() {
        let external_validator_registry =
            read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
        return Ok(CommitCertificateMaterial {
            validator_keys: None,
            external_certificate,
            external_validator_registry: Some(external_validator_registry),
            external_certificate_preverified: false,
        });
    }
    Ok(CommitCertificateMaterial {
        validator_keys: Some(ensure_validator_keys_for_validators(store, validators)?),
        external_certificate: None,
        external_validator_registry: None,
        external_certificate_preverified: false,
    })
}

pub(super) fn write_validator_registry_file(
    path: &Path,
    registry: &ValidatorRegistry,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(registry).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_validator_registry_file(path: &Path) -> io::Result<ValidatorRegistry> {
    read_json_file(path, "validator registry")
}

pub(super) struct HistoricalBlockReplay {
    pub(super) block: BlockRecord,
    pub(super) view: u64,
    pub(super) proposer: String,
    pub(super) parent_hash: String,
    pub(super) state_root: String,
    pub(super) bridge_exit_root: Option<String>,
    pub(super) receipt_ids: Vec<String>,
}

pub(crate) fn historical_block_replay_from_file(path: &Path) -> io::Result<HistoricalBlockReplay> {
    let block: BlockRecord = read_json_file(path, "historical replay block record")?;
    Ok(HistoricalBlockReplay {
        block: block.clone(),
        view: block.header.view,
        proposer: block.header.proposer,
        parent_hash: block.header.parent_hash,
        state_root: block.header.state_root,
        bridge_exit_root: block.header.bridge_exit_root,
        receipt_ids: block.receipt_ids,
    })
}

pub(super) fn historical_replay_commit_inputs(
    certificate_material: &CommitCertificateMaterial,
    replay_block_file: Option<&Path>,
    batch_file: &Path,
) -> io::Result<(Option<HistoricalBlockReplay>, Option<Box<str>>)> {
    let historical_replay = if certificate_material.external_certificate.is_some() {
        replay_block_file
            .map(historical_block_replay_from_file)
            .transpose()?
    } else {
        None
    };
    let archived_payload_json = if historical_replay.is_some() {
        Some(read_bounded_json_text_file(batch_file, "archived batch payload")?.into_boxed_str())
    } else {
        None
    };
    Ok((historical_replay, archived_payload_json))
}

pub(super) struct OrderedCommitPlan<'a, T> {
    pub(super) genesis: &'a Genesis,
    pub(super) governance: &'a GovernanceState,
    pub(super) ledger: &'a LedgerState,
    pub(super) ordered_batches: &'a [String],
    pub(super) shielded: &'a ShieldedState,
    pub(super) bridge: &'a BridgeState,
    pub(super) block_height: u64,
    pub(super) parent_hash: String,
    pub(super) batch_kind: &'a str,
    pub(super) batch_id: &'a str,
    pub(super) payload: &'a T,
    pub(super) batch_receipts: &'a [Receipt],
    pub(super) archived_payload_json: Option<&'a str>,
    pub(super) validator_keys: Option<&'a ValidatorKeyFile>,
    pub(super) external_certificate: Option<&'a BlockCertificateFile>,
    pub(super) external_validator_registry: Option<&'a ValidatorRegistry>,
    pub(super) external_certificate_preverified: bool,
    pub(super) historical_replay: Option<HistoricalBlockReplay>,
    pub(super) certificate_validators: &'a [String],
    pub(super) fastpay_pre_state_effects: &'a [postfiat_types::FastPayVersionFenceV1],
}

pub(super) struct OrderedCommitArtifacts {
    pub(super) height: u64,
    pub(super) receipt_delta: Vec<Receipt>,
    pub(super) ordered_batch_id: String,
    pub(super) archive_entry: BatchArchiveEntry,
    pub(super) block: BlockRecord,
}

pub(super) struct OrderedCommitWithTimings {
    pub(super) artifacts: OrderedCommitArtifacts,
    pub(super) timings: ApplyBatchPrepareTimingReport,
}

pub(super) struct OrderedCommitWrite {
    pub(super) ledger: Option<LedgerState>,
    pub(super) governance: Option<GovernanceState>,
    pub(super) shielded: Option<ShieldedState>,
    pub(super) bridge: Option<BridgeState>,
    pub(super) commit: OrderedCommitArtifacts,
    pub(super) validator_registry: Option<ValidatorRegistry>,
}

pub(super) const CHAIN_TIP_SCHEMA: &str = "postfiat-chain-tip-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct OrderedCommitJournal {
    pub(super) schema: String,
    pub(super) height: u64,
    pub(super) ledger: Option<LedgerState>,
    pub(super) governance: Option<GovernanceState>,
    pub(super) shielded: Option<ShieldedState>,
    pub(super) bridge: Option<BridgeState>,
    pub(super) receipts: Vec<Receipt>,
    pub(super) ordered_batches: Vec<String>,
    pub(super) archive: BatchArchive,
    pub(super) blocks: BlockLog,
    pub(super) validator_registry: Option<ValidatorRegistry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct OrderedCommitDeltaJournal {
    pub(super) schema: String,
    pub(super) height: u64,
    pub(super) ledger: Option<LedgerState>,
    pub(super) governance: Option<GovernanceState>,
    pub(super) shielded: Option<ShieldedState>,
    pub(super) bridge: Option<BridgeState>,
    pub(super) receipt_delta: Vec<Receipt>,
    pub(super) ordered_batch_id: String,
    pub(super) archive_entry: BatchArchiveEntry,
    pub(super) block: BlockRecord,
    pub(super) validator_registry: Option<ValidatorRegistry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum StoredOrderedCommitJournal {
    Delta(OrderedCommitDeltaJournal),
    Full(OrderedCommitJournal),
}

#[derive(Deserialize)]
struct StoredOrderedCommitJournalSchema {
    schema: String,
}

pub(super) fn empty_apply_batch_prepare_timing_report() -> ApplyBatchPrepareTimingReport {
    ApplyBatchPrepareTimingReport {
        schema: "postfiat-ordered-commit-prepare-timings-v1".to_string(),
        total_ms: 0.0,
        ordered_batches_ms: 0.0,
        state_root_ms: 0.0,
        receipts_ms: 0.0,
        archive_clone_ms: 0.0,
        payload_json_ms: 0.0,
        payload_hash_ms: 0.0,
        archive_update_ms: 0.0,
        blocks_clone_ms: 0.0,
        proposer_selection_ms: 0.0,
        receipt_ids_ms: 0.0,
        proposal_hash_ms: 0.0,
        certificate_ms: 0.0,
        certificate_structural_ms: 0.0,
        certificate_vote_set_ms: 0.0,
        certificate_registry_root_ms: 0.0,
        certificate_vote_signature_ms: 0.0,
        certificate_id_ms: 0.0,
        certificate_block_hash_ms: 0.0,
        certificate_clone_ms: 0.0,
        certificate_local_signing_ms: 0.0,
        block_hash_ms: 0.0,
        block_push_ms: 0.0,
    }
}

pub(super) fn prepare_ordered_commit<T: Serialize>(
    plan: OrderedCommitPlan<'_, T>,
) -> io::Result<OrderedCommitArtifacts> {
    prepare_ordered_commit_timed(plan).map(|report| report.artifacts)
}

fn historical_replay_state_root_matches(
    plan: &OrderedCommitPlan<'_, impl Serialize>,
    replay: &HistoricalBlockReplay,
    ordered_batches: &[String],
    current_root: &str,
) -> io::Result<bool> {
    if replay.state_root == current_root {
        return Ok(true);
    }
    let archived_root = replay.state_root.as_str();
    let block = &replay.block;
    if legacy_nav_incomplete_replicated_state_root(
        plan.genesis,
        plan.governance,
        plan.ledger,
        ordered_batches,
        plan.shielded,
        plan.bridge,
    )? == archived_root
    {
        return Ok(true);
    }
    if archived_wan_devnet_legacy_nav_profile_id_allowed(plan.genesis, block)
        && legacy_nav_profile_sp1_uncommitted_replicated_state_root(
            plan.genesis,
            plan.governance,
            plan.ledger,
            ordered_batches,
            plan.shielded,
            plan.bridge,
        )? == archived_root
    {
        return Ok(true);
    }
    if archived_wan_devnet_legacy_nav_asset_commitment_allowed(plan.genesis, block)
        && legacy_nav_asset_uncommitted_replicated_state_root(
            plan.genesis,
            plan.governance,
            plan.ledger,
            ordered_batches,
            plan.shielded,
            plan.bridge,
        )? == archived_root
    {
        return Ok(true);
    }
    if archived_wan_devnet_legacy_nav_profile_id_allowed(plan.genesis, block)
        && legacy_vault_bridge_domainless_withdrawal_replicated_state_root(
            plan.genesis,
            plan.governance,
            plan.ledger,
            ordered_batches,
            plan.shielded,
            plan.bridge,
        )? == archived_root
    {
        return Ok(true);
    }
    if bridge_verification_legacy_replay_allowed(plan.governance, block.header.height)
        && legacy_vault_bridge_deposit_attestation_replicated_state_root(
            plan.genesis,
            plan.governance,
            plan.ledger,
            ordered_batches,
            plan.shielded,
            plan.bridge,
        )? == archived_root
    {
        return Ok(true);
    }
    Ok(legacy_json_replicated_state_root(
        plan.genesis,
        plan.governance,
        plan.ledger,
        ordered_batches,
        plan.shielded,
        plan.bridge,
    )? == archived_root)
}

pub(super) fn prepare_ordered_commit_timed<T: Serialize>(
    plan: OrderedCommitPlan<'_, T>,
) -> io::Result<OrderedCommitWithTimings> {
    let total_start = std::time::Instant::now();
    let mut timings = empty_apply_batch_prepare_timing_report();

    let stage_start = std::time::Instant::now();
    let mut ordered_batches = plan.ordered_batches.to_vec();
    ordered_batches.push(plan.batch_id.to_string());
    timings.ordered_batches_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let simulated_state_root = replicated_state_root(
        plan.genesis,
        plan.governance,
        plan.ledger,
        &ordered_batches,
        plan.shielded,
        plan.bridge,
    )?;
    timings.state_root_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let receipt_delta = plan.batch_receipts.to_vec();
    timings.receipts_ms = apply_batch_elapsed_ms(stage_start);

    if let Some(replay) = plan.historical_replay.as_ref() {
        if replay.block.fastpay_pre_state_effects != plan.fastpay_pre_state_effects {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "historical replay FastPay pre-state evidence mismatch",
            ));
        }
        if replay.parent_hash != plan.parent_hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "historical replay parent mismatch: archived {}, current {}",
                    replay.parent_hash, plan.parent_hash
                ),
            ));
        }
        if !historical_replay_state_root_matches(
            &plan,
            replay,
            &ordered_batches,
            &simulated_state_root,
        )? {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "historical replay state root mismatch: archived {}, recomputed {}",
                    replay.state_root, simulated_state_root
                ),
            ));
        }
        let recomputed_receipt_ids = plan
            .batch_receipts
            .iter()
            .map(|receipt| receipt.tx_id.as_str())
            .collect::<Vec<_>>();
        let archived_receipt_ids = replay
            .receipt_ids
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        if archived_receipt_ids != recomputed_receipt_ids {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "historical replay receipt ids do not match recomputed receipts",
            ));
        }
    }

    timings.archive_clone_ms = 0.0;

    let stage_start = std::time::Instant::now();
    let payload_json = if let Some(raw) = plan.archived_payload_json {
        raw.to_string()
    } else {
        serde_json::to_string(plan.payload).map_err(invalid_data)?
    };
    timings.payload_json_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let payload_hash =
        batch_archive_payload_hash(plan.genesis, plan.batch_kind, plan.batch_id, &payload_json)?;
    timings.payload_hash_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let archive_entry = BatchArchiveEntry {
        batch_kind: plan.batch_kind.to_string(),
        batch_id: plan.batch_id.to_string(),
        payload_hash: payload_hash.clone(),
        payload_json,
    };
    timings.archive_update_ms = apply_batch_elapsed_ms(stage_start);

    timings.blocks_clone_ms = 0.0;

    let height = plan.block_height;
    let historical_replay = plan.historical_replay.as_ref();
    let view = historical_replay
        .map(|replay| replay.view)
        .or_else(|| {
            plan.external_certificate
                .map(|certificate| certificate.view)
        })
        .unwrap_or(0);
    let stage_start = std::time::Instant::now();
    let proposer = if let Some(replay) = historical_replay {
        replay.proposer.clone()
    } else {
        leader_for_view(plan.certificate_validators, height, view).map_err(invalid_data)?
    };
    let parent_hash = historical_replay
        .map(|replay| replay.parent_hash.clone())
        .unwrap_or_else(|| plan.parent_hash.clone());
    timings.proposer_selection_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let receipt_ids = if let Some(replay) = historical_replay {
        replay.receipt_ids.clone()
    } else {
        plan.batch_receipts
            .iter()
            .map(|receipt| receipt.tx_id.clone())
            .collect::<Vec<_>>()
    };
    timings.receipt_ids_ms = apply_batch_elapsed_ms(stage_start);

    let evidence_state_root = historical_replay
        .map(|replay| replay.state_root.clone())
        .unwrap_or(simulated_state_root);
    let recomputed_bridge_exit_root = bridge_exit_root_for_block(
        plan.genesis,
        plan.governance,
        plan.ledger,
        plan.batch_receipts,
        height,
    )?;
    let evidence_bridge_exit_root = historical_replay
        .map(|replay| replay.bridge_exit_root.clone())
        .unwrap_or_else(|| recomputed_bridge_exit_root.clone());
    if historical_replay.is_some() && evidence_bridge_exit_root != recomputed_bridge_exit_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "historical replay bridge exit root does not match recomputed exits",
        ));
    }

    let block_evidence = BlockEvidence {
        height,
        view,
        parent_hash: &parent_hash,
        proposer: &proposer,
        batch_kind: plan.batch_kind,
        batch_id: plan.batch_id,
        state_root: evidence_state_root.as_str(),
        bridge_exit_root: evidence_bridge_exit_root.as_deref(),
        receipt_ids: &receipt_ids,
        fastpay_pre_state_effects: plan.fastpay_pre_state_effects,
    };
    let stage_start = std::time::Instant::now();
    let expected_proposal_hash =
        block_proposal_hash_from_evidence(plan.genesis, &block_evidence, &payload_hash)?;
    timings.proposal_hash_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let (certificate_id, certificate) =
        if let Some(external_certificate) = plan.external_certificate {
            let registry = plan.external_validator_registry.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "external block certificate requires validator registry",
                )
            })?;
            if plan.external_certificate_preverified {
                verify_preverified_external_block_certificate_timed(
                    plan.genesis,
                    &block_evidence,
                    external_certificate,
                    Some(&expected_proposal_hash),
                    registry,
                    plan.certificate_validators,
                    Some(&mut timings),
                )?
            } else {
                verify_external_block_certificate_timed(
                    plan.genesis,
                    &block_evidence,
                    external_certificate,
                    Some(&expected_proposal_hash),
                    registry,
                    plan.certificate_validators,
                    Some(&mut timings),
                )?
            }
        } else {
            let validator_keys = plan.validator_keys.ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "local block certificate signing requires validator keys",
                )
            })?;
            let local_signing_start = std::time::Instant::now();
            let certificate = block_certificate(
                plan.genesis,
                &block_evidence,
                validator_keys,
                plan.certificate_validators,
            )?;
            timings.certificate_local_signing_ms = apply_batch_elapsed_ms(local_signing_start);
            certificate
        };
    timings.certificate_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let consensus_v2_commit = plan
        .external_certificate
        .and_then(|certificate| certificate.consensus_v2_commit.clone());
    let block_hash = if consensus_v2_active_at(plan.genesis, height) {
        let commit = consensus_v2_commit.as_ref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "consensus v2 activation requires committed precommit QC in block header",
            )
        })?;
        let expected_parent = if height == 1 && parent_hash == "genesis" {
            consensus_v2_genesis_parent_id(&commit.proposal.domain).map_err(invalid_data)?
        } else {
            parent_hash.clone()
        };
        if commit.proposal.round.height != height
            || commit.proposal.round.view != view
            || commit.proposal.proposer != proposer
            || commit.proposal.block.parent_block_id != expected_parent
            || commit.proposal.block.payload_hash != payload_hash
            || commit.proposal.block.state_root != evidence_state_root
            || commit.proposal.block.bridge_exit_root != evidence_bridge_exit_root
            || commit.precommit_qc.phase != postfiat_types::ConsensusV2Phase::Precommit
            || commit.precommit_qc.block.as_ref() != Some(&commit.proposal.block)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "consensus v2 commit does not match prepared block",
            ));
        }
        commit.proposal.block.block_id.clone()
    } else {
        if consensus_v2_commit.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "consensus v2 commit present before activation",
            ));
        }
        block_hash(plan.genesis, &block_evidence, &certificate_id)?
    };
    timings.block_hash_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    let block = BlockRecord {
        header: BlockHeader {
            height,
            view,
            parent_hash,
            proposer,
            batch_kind: plan.batch_kind.to_string(),
            batch_id: plan.batch_id.to_string(),
            state_root: evidence_state_root,
            bridge_exit_root: evidence_bridge_exit_root,
            receipt_count: receipt_ids.len() as u64,
            certificate_id,
            certificate,
            consensus_v2_commit,
            block_hash,
        },
        receipt_ids,
        fastpay_pre_state_effects: plan.fastpay_pre_state_effects.to_vec(),
    };
    timings.block_push_ms = apply_batch_elapsed_ms(stage_start);
    timings.total_ms = apply_batch_elapsed_ms(total_start);

    Ok(OrderedCommitWithTimings {
        artifacts: OrderedCommitArtifacts {
            height,
            receipt_delta,
            ordered_batch_id: plan.batch_id.to_string(),
            archive_entry,
            block,
        },
        timings,
    })
}

pub(super) fn write_ordered_commit_with_journal_locked(
    store: &NodeStore,
    commit_lock: &StorageMutationLock,
    write: OrderedCommitWrite,
) -> io::Result<()> {
    write_ordered_commit_with_journal_timed_locked(store, commit_lock, write).map(|_| ())
}

pub(super) fn empty_apply_batch_write_timing_report() -> ApplyBatchWriteTimingReport {
    ApplyBatchWriteTimingReport {
        schema: "postfiat-ordered-commit-write-timings-v1".to_string(),
        total_ms: 0.0,
        write_journal_ms: 0.0,
        write_ledger_ms: 0.0,
        write_governance_ms: 0.0,
        write_shielded_ms: 0.0,
        write_bridge_ms: 0.0,
        write_receipts_ms: 0.0,
        write_ordered_batches_ms: 0.0,
        write_batch_archive_ms: 0.0,
        write_blocks_ms: 0.0,
        write_validator_registry_ms: 0.0,
        refresh_account_tx_index_ms: 0.0,
        remove_journal_ms: 0.0,
    }
}

pub(super) fn write_ordered_commit_with_journal_timed_locked(
    store: &NodeStore,
    _commit_lock: &StorageMutationLock,
    write: OrderedCommitWrite,
) -> io::Result<ApplyBatchWriteTimingReport> {
    let total_start = std::time::Instant::now();
    let mut timings = empty_apply_batch_write_timing_report();
    let journal = ordered_commit_delta_journal(write)?;
    let stage_start = std::time::Instant::now();
    store.write_ordered_commit_journal(&journal)?;
    timings.write_journal_ms = apply_batch_elapsed_ms(stage_start);

    apply_ordered_commit_delta_journal_timed(store, &journal, &mut timings)?;

    let stage_start = std::time::Instant::now();
    store.remove_ordered_commit_journal()?;
    timings.remove_journal_ms = apply_batch_elapsed_ms(stage_start);
    timings.total_ms = apply_batch_elapsed_ms(total_start);
    Ok(timings)
}

pub(super) fn ordered_commit_delta_journal(
    write: OrderedCommitWrite,
) -> io::Result<OrderedCommitDeltaJournal> {
    let block = write.commit.block;
    if block.header.batch_id.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "committed block has empty batch id",
        ));
    }
    if write.commit.ordered_batch_id != block.header.batch_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "ordered batch tail does not match committed block batch `{}`",
                block.header.batch_id
            ),
        ));
    }
    if write.commit.archive_entry.batch_kind != block.header.batch_kind
        || write.commit.archive_entry.batch_id != block.header.batch_id
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "commit archive entry does not match committed block",
        ));
    }
    let receipt_delta = receipts_for_block(&write.commit.receipt_delta, &block.receipt_ids)?;
    Ok(OrderedCommitDeltaJournal {
        schema: "postfiat-ordered-commit-delta-journal-v1".to_string(),
        height: write.commit.height,
        ledger: write.ledger,
        governance: write.governance,
        shielded: write.shielded,
        bridge: write.bridge,
        receipt_delta,
        ordered_batch_id: write.commit.ordered_batch_id,
        archive_entry: write.commit.archive_entry,
        block,
        validator_registry: write.validator_registry,
    })
}

pub(super) fn receipts_for_block(
    receipts: &[Receipt],
    receipt_ids: &[String],
) -> io::Result<Vec<Receipt>> {
    let mut seen = BTreeSet::new();
    let mut receipt_delta = Vec::with_capacity(receipt_ids.len());
    for receipt_id in receipt_ids {
        if !seen.insert(receipt_id.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("committed block has duplicate receipt id `{receipt_id}`"),
            ));
        }
        let receipt = receipts
            .iter()
            .rev()
            .find(|receipt| &receipt.tx_id == receipt_id)
            .cloned()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("missing receipt `{receipt_id}` for committed block"),
                )
            })?;
        receipt_delta.push(receipt);
    }
    Ok(receipt_delta)
}

pub(super) fn recover_ordered_commit_journal(store: &NodeStore) -> io::Result<()> {
    let commit_lock = store.lock_ordered_commit()?;
    recover_ordered_commit_journal_locked(store, &commit_lock)
}

pub(super) fn recover_ordered_commit_journal_locked(
    store: &NodeStore,
    _commit_lock: &StorageMutationLock,
) -> io::Result<()> {
    let Some(raw) = store.read_ordered_commit_journal_raw()? else {
        return Ok(());
    };
    let schema: StoredOrderedCommitJournalSchema =
        serde_json::from_str(&raw).map_err(invalid_data)?;
    let journal = match schema.schema.as_str() {
        "postfiat-ordered-commit-delta-journal-v1" => {
            StoredOrderedCommitJournal::Delta(serde_json::from_str(&raw).map_err(invalid_data)?)
        }
        "postfiat-ordered-commit-journal-v1" => {
            StoredOrderedCommitJournal::Full(serde_json::from_str(&raw).map_err(invalid_data)?)
        }
        unsupported => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported ordered commit journal schema `{unsupported}`"),
            ));
        }
    };
    apply_stored_ordered_commit_journal(store, &journal)?;
    store.remove_ordered_commit_journal()
}

pub(super) fn apply_stored_ordered_commit_journal(
    store: &NodeStore,
    journal: &StoredOrderedCommitJournal,
) -> io::Result<()> {
    match journal {
        StoredOrderedCommitJournal::Delta(journal) => {
            apply_ordered_commit_delta_journal(store, journal)
        }
        StoredOrderedCommitJournal::Full(journal) => apply_ordered_commit_journal(store, journal),
    }
}

pub(super) fn apply_ordered_commit_journal(
    store: &NodeStore,
    journal: &OrderedCommitJournal,
) -> io::Result<()> {
    let mut timings = empty_apply_batch_write_timing_report();
    apply_ordered_commit_journal_timed(store, journal, &mut timings)
}

pub(super) fn apply_ordered_commit_journal_timed(
    store: &NodeStore,
    journal: &OrderedCommitJournal,
    timings: &mut ApplyBatchWriteTimingReport,
) -> io::Result<()> {
    if journal.schema != "postfiat-ordered-commit-journal-v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported ordered commit journal schema `{}`",
                journal.schema
            ),
        ));
    }
    if let Some(ledger) = journal.ledger.as_ref() {
        let stage_start = std::time::Instant::now();
        store.write_ledger(ledger)?;
        timings.write_ledger_ms = apply_batch_elapsed_ms(stage_start);
    }
    if let Some(governance) = journal.governance.as_ref() {
        let stage_start = std::time::Instant::now();
        store.write_governance(governance)?;
        timings.write_governance_ms = apply_batch_elapsed_ms(stage_start);
    }
    if let Some(shielded) = journal.shielded.as_ref() {
        let stage_start = std::time::Instant::now();
        store.write_shielded(shielded)?;
        timings.write_shielded_ms = apply_batch_elapsed_ms(stage_start);
    }
    if let Some(bridge) = journal.bridge.as_ref() {
        let stage_start = std::time::Instant::now();
        store.write_bridge(bridge)?;
        timings.write_bridge_ms = apply_batch_elapsed_ms(stage_start);
    }
    let stage_start = std::time::Instant::now();
    store.write_receipts(&journal.receipts)?;
    timings.write_receipts_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    store.write_ordered_batches(&journal.ordered_batches)?;
    timings.write_ordered_batches_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    store.write_batch_archive(&journal.archive)?;
    timings.write_batch_archive_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    store.write_blocks(&journal.blocks)?;
    timings.write_blocks_ms = apply_batch_elapsed_ms(stage_start);

    if let Some(registry) = journal.validator_registry.as_ref() {
        let stage_start = std::time::Instant::now();
        write_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE), registry)?;
        timings.write_validator_registry_ms = apply_batch_elapsed_ms(stage_start);
    }
    // The account history index is a rebuildable operator cache, not consensus
    // state. It is deliberately not refreshed in the synchronous finality path;
    // account_tx falls back to a bounded scan when the cache is stale, and
    // account-tx-index-build refreshes the cache as maintenance work.
    timings.refresh_account_tx_index_ms = 0.0;
    Ok(())
}

pub(super) fn apply_ordered_commit_delta_journal(
    store: &NodeStore,
    journal: &OrderedCommitDeltaJournal,
) -> io::Result<()> {
    let mut timings = empty_apply_batch_write_timing_report();
    apply_ordered_commit_delta_journal_timed(store, journal, &mut timings)
}

pub(super) fn apply_ordered_commit_delta_journal_timed(
    store: &NodeStore,
    journal: &OrderedCommitDeltaJournal,
    timings: &mut ApplyBatchWriteTimingReport,
) -> io::Result<()> {
    if journal.schema != "postfiat-ordered-commit-delta-journal-v1" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported ordered commit delta journal schema `{}`",
                journal.schema
            ),
        ));
    }
    if journal.height != journal.block.header.height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "ordered commit delta journal height {} does not match block height {}",
                journal.height, journal.block.header.height
            ),
        ));
    }
    if journal.ordered_batch_id != journal.block.header.batch_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "ordered commit delta journal batch `{}` does not match block batch `{}`",
                journal.ordered_batch_id, journal.block.header.batch_id
            ),
        ));
    }
    if journal.archive_entry.batch_kind != journal.block.header.batch_kind
        || journal.archive_entry.batch_id != journal.block.header.batch_id
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ordered commit delta journal archive entry does not match block batch",
        ));
    }
    if journal.receipt_delta.len() != journal.block.receipt_ids.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "ordered commit delta journal receipt count does not match block receipt ids",
        ));
    }
    for (receipt, receipt_id) in journal.receipt_delta.iter().zip(&journal.block.receipt_ids) {
        if &receipt.tx_id != receipt_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ordered commit delta journal receipt `{}` does not match block receipt `{receipt_id}`",
                    receipt.tx_id
                ),
            ));
        }
    }

    let genesis = store.read_genesis()?;
    let chain_tip = read_chain_tip_or_reconstruct_for_genesis(store, &genesis)?;
    let already_committed = journal.block.header.height == chain_tip.height
        && journal.block.header.block_hash == chain_tip.block_hash;
    if journal.block.header.height <= chain_tip.height && !already_committed {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "ordered commit delta block height {} conflicts with local tip height {}",
                journal.block.header.height, chain_tip.height
            ),
        ));
    }
    if !already_committed {
        let expected_height = chain_tip
            .height
            .checked_add(1)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
        if journal.block.header.height != expected_height {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ordered commit delta block height {} does not extend local tip {}",
                    journal.block.header.height, chain_tip.height
                ),
            ));
        }
        if journal.block.header.parent_hash != chain_tip.block_hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "ordered commit delta parent hash `{}` does not match local tip `{}`",
                    journal.block.header.parent_hash, chain_tip.block_hash
                ),
            ));
        }
    }

    if let Some(ledger) = journal.ledger.as_ref() {
        let stage_start = std::time::Instant::now();
        store.write_ledger(ledger)?;
        timings.write_ledger_ms = apply_batch_elapsed_ms(stage_start);
    }
    if let Some(governance) = journal.governance.as_ref() {
        let stage_start = std::time::Instant::now();
        store.write_governance(governance)?;
        timings.write_governance_ms = apply_batch_elapsed_ms(stage_start);
    }
    if let Some(shielded) = journal.shielded.as_ref() {
        let stage_start = std::time::Instant::now();
        store.write_shielded(shielded)?;
        timings.write_shielded_ms = apply_batch_elapsed_ms(stage_start);
    }
    if let Some(bridge) = journal.bridge.as_ref() {
        let stage_start = std::time::Instant::now();
        store.write_bridge(bridge)?;
        timings.write_bridge_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    if !already_committed {
        for receipt in &journal.receipt_delta {
            store.append_receipt_record(receipt)?;
        }
    }
    timings.write_receipts_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    if !already_committed {
        store.append_ordered_batch_record(&journal.ordered_batch_id)?;
    }
    timings.write_ordered_batches_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    if !already_committed {
        store.append_batch_archive_entry(journal.archive_entry.clone())?;
    }
    timings.write_batch_archive_ms = apply_batch_elapsed_ms(stage_start);

    let stage_start = std::time::Instant::now();
    if !already_committed {
        store.append_block_record(&journal.block)?;
        let next_tip = chain_tip_after_delta(&chain_tip, journal)?;
        store.write_chain_tip(&next_tip)?;
    }
    timings.write_blocks_ms = apply_batch_elapsed_ms(stage_start);

    if let Some(registry) = journal.validator_registry.as_ref() {
        let stage_start = std::time::Instant::now();
        write_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE), registry)?;
        timings.write_validator_registry_ms = apply_batch_elapsed_ms(stage_start);
    }
    // The account history index is a rebuildable operator cache, not consensus
    // state. Keep it out of synchronous delta commit; stale status remains
    // observable and account_tx falls back to block/archive scanning.
    timings.refresh_account_tx_index_ms = 0.0;
    Ok(())
}

pub(super) fn read_chain_tip_or_reconstruct_for_genesis(
    store: &NodeStore,
    genesis: &Genesis,
) -> io::Result<ChainTipState> {
    match store.read_chain_tip() {
        Ok(tip) => {
            validate_chain_tip_domain(&tip, genesis)?;
            Ok(tip)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            let tip = reconstruct_chain_tip_for_genesis(store, genesis)?;
            store.write_chain_tip(&tip)?;
            Ok(tip)
        }
        Err(error) => Err(error),
    }
}

pub(super) fn validate_chain_tip_domain(tip: &ChainTipState, genesis: &Genesis) -> io::Result<()> {
    if tip.schema != CHAIN_TIP_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported chain tip schema `{}`", tip.schema),
        ));
    }
    if tip.chain_id != genesis.chain_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "chain tip chain_id does not match local genesis",
        ));
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if tip.genesis_hash != expected_genesis_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "chain tip genesis_hash does not match local genesis",
        ));
    }
    if tip.protocol_version != genesis.protocol_version {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "chain tip protocol_version does not match local genesis",
        ));
    }
    if tip.height < tip.history_base_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "chain tip height is below history base height",
        ));
    }
    Ok(())
}

pub(super) fn reconstruct_chain_tip_for_genesis(
    store: &NodeStore,
    genesis: &Genesis,
) -> io::Result<ChainTipState> {
    let checkpoint = read_history_checkpoint_state_optional(store)?;
    let history_base_height = checkpoint
        .as_ref()
        .map(|checkpoint| checkpoint.pruned_up_to_height)
        .unwrap_or(0);
    let blocks = store.read_blocks()?;
    let ordered_batches = store.read_ordered_batches()?;
    let receipts = store.read_receipts()?;
    let height = history_base_height
        .checked_add(blocks.blocks.len() as u64)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "chain tip height overflow"))?;
    let block_hash = blocks
        .blocks
        .last()
        .map(|block| block.header.block_hash.clone())
        .or_else(|| {
            checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.checkpoint_block_hash.clone())
        })
        .unwrap_or_else(|| "genesis".to_string());
    let state_root = if let Some(block) = blocks.blocks.last() {
        block.header.state_root.clone()
    } else if let Some(checkpoint) = checkpoint.as_ref() {
        checkpoint.checkpoint_state_root.clone()
    } else {
        current_replicated_state_root(store, genesis)?
    };
    Ok(ChainTipState {
        schema: CHAIN_TIP_SCHEMA.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        height,
        block_hash,
        state_root,
        ordered_batch_count: ordered_batches.len() as u64,
        receipt_count: receipts.len() as u64,
        history_base_height,
    })
}

pub(super) fn chain_tip_after_delta(
    previous: &ChainTipState,
    journal: &OrderedCommitDeltaJournal,
) -> io::Result<ChainTipState> {
    let receipt_count = previous
        .receipt_count
        .checked_add(journal.receipt_delta.len() as u64)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "receipt count overflow"))?;
    let ordered_batch_count = previous.ordered_batch_count.checked_add(1).ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidData, "ordered batch count overflow")
    })?;
    Ok(ChainTipState {
        schema: previous.schema.clone(),
        chain_id: previous.chain_id.clone(),
        genesis_hash: previous.genesis_hash.clone(),
        protocol_version: previous.protocol_version,
        height: journal.block.header.height,
        block_hash: journal.block.header.block_hash.clone(),
        state_root: journal.block.header.state_root.clone(),
        ordered_batch_count,
        receipt_count,
        history_base_height: previous.history_base_height,
    })
}

pub(super) fn batch_archive_payload_hash(
    genesis: &Genesis,
    batch_kind: &str,
    batch_id: &str,
    payload_json: &str,
) -> io::Result<String> {
    let encoded = serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash(genesis),
        genesis.protocol_version,
        batch_kind,
        batch_id,
        payload_json,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex("postfiat.batch_archive_payload.v1", &encoded))
}

pub(super) struct BlockEvidence<'a> {
    pub(super) height: u64,
    pub(super) view: u64,
    pub(super) parent_hash: &'a str,
    pub(super) proposer: &'a str,
    pub(super) batch_kind: &'a str,
    pub(super) batch_id: &'a str,
    pub(super) state_root: &'a str,
    pub(super) bridge_exit_root: Option<&'a str>,
    pub(super) receipt_ids: &'a [String],
    pub(super) fastpay_pre_state_effects: &'a [postfiat_types::FastPayVersionFenceV1],
}

impl<'a> BlockEvidence<'a> {
    pub(super) fn from_block(block: &'a BlockRecord) -> Self {
        Self {
            height: block.header.height,
            view: block.header.view,
            parent_hash: &block.header.parent_hash,
            proposer: &block.header.proposer,
            batch_kind: &block.header.batch_kind,
            batch_id: &block.header.batch_id,
            state_root: &block.header.state_root,
            bridge_exit_root: block.header.bridge_exit_root.as_deref(),
            receipt_ids: &block.receipt_ids,
            fastpay_pre_state_effects: &block.fastpay_pre_state_effects,
        }
    }
}

pub(super) struct OwnedBlockEvidence {
    pub(super) height: u64,
    pub(super) view: u64,
    pub(super) parent_hash: String,
    pub(super) proposer: String,
    pub(super) batch_kind: String,
    pub(super) batch_id: String,
    pub(super) state_root: String,
    pub(super) bridge_exit_root: Option<String>,
    pub(super) receipt_ids: Vec<String>,
    pub(super) fastpay_pre_state_effects: Vec<postfiat_types::FastPayVersionFenceV1>,
}
