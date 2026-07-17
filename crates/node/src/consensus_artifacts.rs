use super::*;

impl OwnedBlockEvidence {
    pub(super) fn from_block(block: &BlockRecord) -> Self {
        Self {
            height: block.header.height,
            view: block.header.view,
            parent_hash: block.header.parent_hash.clone(),
            proposer: block.header.proposer.clone(),
            batch_kind: block.header.batch_kind.clone(),
            batch_id: block.header.batch_id.clone(),
            state_root: block.header.state_root.clone(),
            receipt_ids: block.receipt_ids.clone(),
            fastpay_pre_state_effects: block.fastpay_pre_state_effects.clone(),
        }
    }

    pub(super) fn from_proposal(proposal: &BlockProposalFile) -> Self {
        Self {
            height: proposal.block_height,
            view: proposal.view,
            parent_hash: proposal.parent_hash.clone(),
            proposer: proposal.proposer.clone(),
            batch_kind: proposal.batch_kind.clone(),
            batch_id: proposal.batch_id.clone(),
            state_root: proposal.state_root.clone(),
            receipt_ids: proposal.receipt_ids.clone(),
            fastpay_pre_state_effects: proposal.fastpay_pre_state_effects.clone(),
        }
    }

    pub(super) fn as_evidence(&self) -> BlockEvidence<'_> {
        BlockEvidence {
            height: self.height,
            view: self.view,
            parent_hash: &self.parent_hash,
            proposer: &self.proposer,
            batch_kind: &self.batch_kind,
            batch_id: &self.batch_id,
            state_root: &self.state_root,
            receipt_ids: &self.receipt_ids,
            fastpay_pre_state_effects: &self.fastpay_pre_state_effects,
        }
    }
}

pub(super) struct BlockVoteTarget {
    pub(super) evidence: OwnedBlockEvidence,
    pub(super) validators: Vec<String>,
    pub(super) block_hash: Option<String>,
    pub(super) proposal_hash: Option<String>,
}

pub(super) fn block_hash(
    genesis: &Genesis,
    evidence: &BlockEvidence<'_>,
    certificate_id: &str,
) -> io::Result<String> {
    if evidence.fastpay_pre_state_effects.is_empty() {
        let encoded = serde_json::to_vec(&(
            genesis.chain_id.as_str(),
            genesis_hash(genesis),
            genesis.protocol_version,
            evidence.height,
            evidence.view,
            evidence.parent_hash,
            evidence.proposer,
            evidence.batch_kind,
            evidence.batch_id,
            evidence.state_root,
            evidence.receipt_ids,
            certificate_id,
        ))
        .map_err(invalid_data)?;
        return Ok(hash_hex("postfiat.block.v1", &encoded));
    }
    let encoded = serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash(genesis),
        genesis.protocol_version,
        evidence.height,
        evidence.view,
        evidence.parent_hash,
        evidence.proposer,
        evidence.batch_kind,
        evidence.batch_id,
        evidence.state_root,
        evidence.receipt_ids,
        certificate_id,
        evidence.fastpay_pre_state_effects,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex("postfiat.block.v2", &encoded))
}

fn expected_certificate_file_block_hash(
    genesis: &Genesis,
    evidence: &BlockEvidence<'_>,
    certificate_id: &str,
    certificate_file: &BlockCertificateFile,
) -> io::Result<String> {
    if consensus_v2_active_at(genesis, evidence.height) {
        let commit = certificate_file
            .consensus_v2_commit
            .as_ref()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "activated consensus v2 certificate is missing its commit artifact",
                )
            })?;
        let proposal = &commit.proposal;
        let parent_matches = proposal.block.parent_block_id == evidence.parent_hash
            || (evidence.height == 1 && evidence.parent_hash == "genesis");
        if proposal.round.height != evidence.height
            || proposal.round.view != evidence.view
            || proposal.proposer != evidence.proposer
            || proposal.block.height != evidence.height
            || !parent_matches
            || proposal.block.state_root != evidence.state_root
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "consensus v2 certificate commit does not match block evidence",
            ));
        }
        Ok(proposal.block.block_id.clone())
    } else {
        if certificate_file.consensus_v2_commit.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "consensus v2 certificate commit appears before activation",
            ));
        }
        block_hash(genesis, evidence, certificate_id)
    }
}

pub(super) fn tx_finality_proof_id(
    genesis: &Genesis,
    receipt: &Receipt,
    receipt_index: u64,
    block: &BlockRecord,
) -> io::Result<String> {
    if block.fastpay_pre_state_effects.is_empty() {
        let encoded = serde_json::to_vec(&(
            genesis.chain_id.as_str(),
            genesis_hash(genesis),
            genesis.protocol_version,
            receipt,
            receipt_index,
            &block.header,
            &block.receipt_ids,
        ))
        .map_err(invalid_data)?;
        return Ok(hash_hex("postfiat.tx_finality.v1", &encoded));
    }
    let encoded = serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash(genesis),
        genesis.protocol_version,
        receipt,
        receipt_index,
        &block.header,
        &block.receipt_ids,
        &block.fastpay_pre_state_effects,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex("postfiat.tx_finality.v2", &encoded))
}

pub(super) fn validate_finality_tx_id(tx_id: &str) -> io::Result<()> {
    if tx_id.len() != 96
        || !tx_id
            .bytes()
            .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "tx_id must be 96 lowercase hex characters",
        ));
    }
    Ok(())
}

pub(super) fn block_certificate(
    genesis: &Genesis,
    evidence: &BlockEvidence<'_>,
    validator_keys: &ValidatorKeyFile,
    validators: &[String],
) -> io::Result<(String, BlockCertificate)> {
    let quorum = block_certificate_quorum(validators)?;
    let registry = validator_registry_from_keys_for_validators(validator_keys, validators)?;
    let registry_root = validator_registry_root(&registry, validators)?;
    let votes = validators
        .iter()
        .map(|validator| {
            let key_record = validator_key_record(validator_keys, validator)?;
            let message =
                block_certificate_vote_message(genesis, evidence, validator, true, &registry_root)?;
            let private_key =
                Zeroizing::new(hex_to_bytes(&key_record.private_key_hex).map_err(invalid_data)?);
            let signature_seed = block_certificate_signature_seed(&message)?;
            let signature = ml_dsa_65_sign_with_context_seed(
                &private_key,
                &message,
                BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
                &signature_seed,
            )
            .map_err(invalid_data)?;
            Ok(BlockCertificateVote {
                vote_id: block_certificate_vote_id(&message),
                validator: validator.clone(),
                accept: true,
                algorithm_id: key_record.algorithm_id.clone(),
                registry_root: registry_root.clone(),
                public_key_hex: String::new(),
                signature_hex: bytes_to_hex(&signature),
            })
        })
        .collect::<io::Result<Vec<_>>>()?;
    let certificate = BlockCertificate {
        validators: validators.to_vec(),
        quorum,
        registry_root,
        votes,
    };
    let certificate_id = block_certificate_id(genesis, evidence, &certificate)?;
    Ok((certificate_id, certificate))
}

pub(super) fn verify_external_block_certificate(
    genesis: &Genesis,
    evidence: &BlockEvidence<'_>,
    certificate_file: &BlockCertificateFile,
    expected_proposal_hash: Option<&str>,
    validator_registry: &ValidatorRegistry,
    expected_validators: &[String],
) -> io::Result<(String, BlockCertificate)> {
    verify_external_block_certificate_timed(
        genesis,
        evidence,
        certificate_file,
        expected_proposal_hash,
        validator_registry,
        expected_validators,
        None,
    )
}

pub(super) fn verify_external_block_certificate_timed(
    genesis: &Genesis,
    evidence: &BlockEvidence<'_>,
    certificate_file: &BlockCertificateFile,
    expected_proposal_hash: Option<&str>,
    validator_registry: &ValidatorRegistry,
    expected_validators: &[String],
    mut timings: Option<&mut ApplyBatchPrepareTimingReport>,
) -> io::Result<(String, BlockCertificate)> {
    let stage_start = std::time::Instant::now();
    if certificate_file.schema != BLOCK_CERTIFICATE_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported block certificate schema `{}`",
                certificate_file.schema
            ),
        ));
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if certificate_file.chain_id != genesis.chain_id
        || certificate_file.genesis_hash != expected_genesis_hash
        || certificate_file.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "external block certificate chain domain mismatch",
        ));
    }
    if certificate_file.block_height != evidence.height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "external block certificate height {} does not match block {}",
                certificate_file.block_height, evidence.height
            ),
        ));
    }
    if certificate_file.view != evidence.view || certificate_file.proposer != evidence.proposer {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "external block certificate ordering metadata mismatch",
        ));
    }
    if certificate_file.fastpay_pre_state_effects != evidence.fastpay_pre_state_effects {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "external block certificate FastPay pre-state evidence mismatch",
        ));
    }
    if certificate_file.proposal_hash.as_deref() != expected_proposal_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "external block certificate proposal hash mismatch",
        ));
    }
    if certificate_file.certificate.validators != expected_validators {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "external block certificate validator set mismatch",
        ));
    }
    let expected_quorum = block_certificate_quorum(expected_validators)?;
    if certificate_file.certificate.quorum != expected_quorum
        || certificate_file.certificate.votes.len() < expected_quorum
        || certificate_file.certificate.votes.len() > expected_validators.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "external block certificate quorum mismatch",
        ));
    }
    if let Some(timings) = &mut timings {
        timings.certificate_structural_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    validate_block_certificate_vote_set(
        &certificate_file.certificate.votes,
        expected_validators,
        "external block certificate",
    )?;
    if let Some(timings) = &mut timings {
        timings.certificate_vote_set_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    let registry_root = certificate_registry_root(
        &certificate_file.certificate.registry_root,
        validator_registry,
        expected_validators,
        "external block certificate",
    )?;
    if let Some(timings) = &mut timings {
        timings.certificate_registry_root_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    for vote in &certificate_file.certificate.votes {
        verify_block_certificate_vote_for_evidence(
            genesis,
            evidence,
            validator_registry,
            vote,
            &vote.validator,
            &registry_root,
        )?;
    }
    if let Some(timings) = &mut timings {
        timings.certificate_vote_signature_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    let certificate_id = block_certificate_id(genesis, evidence, &certificate_file.certificate)?;
    if certificate_file.certificate_id != certificate_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "external block certificate id mismatch",
        ));
    }
    if let Some(timings) = &mut timings {
        timings.certificate_id_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    if let Some(block_hash_value) = certificate_file.block_hash.as_ref() {
        let expected_block_hash = expected_certificate_file_block_hash(
            genesis,
            evidence,
            &certificate_id,
            certificate_file,
        )?;
        if block_hash_value != &expected_block_hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "external block certificate block hash mismatch",
            ));
        }
    }
    if let Some(timings) = &mut timings {
        timings.certificate_block_hash_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    let certificate = certificate_file.certificate.clone();
    if let Some(timings) = &mut timings {
        timings.certificate_clone_ms = apply_batch_elapsed_ms(stage_start);
    }
    Ok((certificate_id, certificate))
}

pub(super) fn verify_preverified_external_block_certificate_timed(
    genesis: &Genesis,
    evidence: &BlockEvidence<'_>,
    certificate_file: &BlockCertificateFile,
    expected_proposal_hash: Option<&str>,
    validator_registry: &ValidatorRegistry,
    expected_validators: &[String],
    mut timings: Option<&mut ApplyBatchPrepareTimingReport>,
) -> io::Result<(String, BlockCertificate)> {
    let stage_start = std::time::Instant::now();
    if certificate_file.schema != BLOCK_CERTIFICATE_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported block certificate schema `{}`",
                certificate_file.schema
            ),
        ));
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if certificate_file.chain_id != genesis.chain_id
        || certificate_file.genesis_hash != expected_genesis_hash
        || certificate_file.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preverified block certificate chain domain mismatch",
        ));
    }
    if certificate_file.block_height != evidence.height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "preverified block certificate height {} does not match block {}",
                certificate_file.block_height, evidence.height
            ),
        ));
    }
    if certificate_file.view != evidence.view || certificate_file.proposer != evidence.proposer {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preverified block certificate ordering metadata mismatch",
        ));
    }
    if certificate_file.fastpay_pre_state_effects != evidence.fastpay_pre_state_effects {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preverified block certificate FastPay pre-state evidence mismatch",
        ));
    }
    if certificate_file.proposal_hash.as_deref() != expected_proposal_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preverified block certificate proposal hash mismatch",
        ));
    }
    if certificate_file.certificate.validators != expected_validators {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preverified block certificate validator set mismatch",
        ));
    }
    let expected_quorum = block_certificate_quorum(expected_validators)?;
    if certificate_file.certificate.quorum != expected_quorum
        || certificate_file.certificate.votes.len() < expected_quorum
        || certificate_file.certificate.votes.len() > expected_validators.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "preverified block certificate quorum mismatch",
        ));
    }
    if let Some(timings) = &mut timings {
        timings.certificate_structural_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    validate_block_certificate_vote_set(
        &certificate_file.certificate.votes,
        expected_validators,
        "preverified block certificate",
    )?;
    if let Some(timings) = &mut timings {
        timings.certificate_vote_set_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    let registry_root = certificate_registry_root(
        &certificate_file.certificate.registry_root,
        validator_registry,
        expected_validators,
        "preverified block certificate",
    )?;
    for vote in &certificate_file.certificate.votes {
        if vote.registry_root != registry_root {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "preverified block certificate vote registry root mismatch",
            ));
        }
    }
    if let Some(timings) = &mut timings {
        timings.certificate_registry_root_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    let certificate_id = certificate_file.certificate_id.clone();
    if let Some(timings) = &mut timings {
        timings.certificate_id_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    if let Some(block_hash_value) = certificate_file.block_hash.as_ref() {
        let expected_block_hash = expected_certificate_file_block_hash(
            genesis,
            evidence,
            &certificate_id,
            certificate_file,
        )?;
        if block_hash_value != &expected_block_hash {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "preverified block certificate block hash mismatch",
            ));
        }
    }
    if let Some(timings) = &mut timings {
        timings.certificate_block_hash_ms = apply_batch_elapsed_ms(stage_start);
    }

    let stage_start = std::time::Instant::now();
    let certificate = certificate_file.certificate.clone();
    if let Some(timings) = &mut timings {
        timings.certificate_clone_ms = apply_batch_elapsed_ms(stage_start);
    }
    Ok((certificate_id, certificate))
}

pub(super) fn verify_block_certificate_evidence(
    genesis: &Genesis,
    block: &BlockRecord,
    validator_registry: &ValidatorRegistry,
    expected_validators: &[String],
) -> io::Result<()> {
    let expected_proposer =
        leader_for_view(expected_validators, block.header.height, block.header.view)
            .map_err(invalid_data)?;
    if block.header.proposer != expected_proposer {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block {} proposer mismatch", block.header.height),
        ));
    }
    if block.header.certificate.validators != expected_validators {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} certificate validator set mismatch",
                block.header.height
            ),
        ));
    }
    let expected_quorum = block_certificate_quorum(expected_validators)?;
    if block.header.certificate.quorum != expected_quorum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block {} certificate quorum mismatch", block.header.height),
        ));
    }
    if block.header.certificate.votes.len() < expected_quorum
        || block.header.certificate.votes.len() > expected_validators.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} certificate vote count mismatch",
                block.header.height
            ),
        ));
    }
    let vote_set_context = format!("block {} certificate", block.header.height);
    validate_block_certificate_vote_set(
        &block.header.certificate.votes,
        expected_validators,
        &vote_set_context,
    )?;
    let registry_root = certificate_registry_root_or_legacy(
        &block.header.certificate.registry_root,
        validator_registry,
        expected_validators,
        &vote_set_context,
    )?;
    for vote in &block.header.certificate.votes {
        verify_block_certificate_vote(
            genesis,
            block,
            validator_registry,
            vote,
            &vote.validator,
            &registry_root,
        )?;
    }
    let block_evidence = BlockEvidence::from_block(block);
    let expected_certificate_id =
        block_certificate_id(genesis, &block_evidence, &block.header.certificate)?;
    if block.header.certificate_id != expected_certificate_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block {} certificate id mismatch", block.header.height),
        ));
    }
    Ok(())
}

pub(super) fn verify_block_certificate_vote(
    genesis: &Genesis,
    block: &BlockRecord,
    validator_registry: &ValidatorRegistry,
    vote: &BlockCertificateVote,
    expected_validator: &str,
    expected_registry_root: &str,
) -> io::Result<()> {
    let block_evidence = BlockEvidence::from_block(block);
    verify_block_certificate_vote_for_evidence(
        genesis,
        &block_evidence,
        validator_registry,
        vote,
        expected_validator,
        expected_registry_root,
    )
}

pub(super) fn verify_block_certificate_vote_for_evidence(
    genesis: &Genesis,
    evidence: &BlockEvidence<'_>,
    validator_registry: &ValidatorRegistry,
    vote: &BlockCertificateVote,
    expected_validator: &str,
    expected_registry_root: &str,
) -> io::Result<()> {
    if vote.validator != expected_validator || !vote.accept {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block {} certificate vote mismatch", evidence.height),
        ));
    }
    if vote.registry_root != expected_registry_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} certificate vote registry root mismatch",
                evidence.height
            ),
        ));
    }
    let registry_record = validator_registry_record(validator_registry, expected_validator)?;
    if vote.algorithm_id != registry_record.algorithm_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} certificate vote algorithm mismatch",
                evidence.height
            ),
        ));
    }
    if !vote.public_key_hex.is_empty() && vote.public_key_hex != registry_record.public_key_hex {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} certificate vote public key mismatch",
                evidence.height
            ),
        ));
    }
    let message = block_certificate_vote_message(
        genesis,
        evidence,
        expected_validator,
        true,
        expected_registry_root,
    )?;
    let expected_vote_id = block_certificate_vote_id(&message);
    if vote.vote_id != expected_vote_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block {} certificate vote mismatch", evidence.height),
        ));
    }
    let public_key = hex_to_bytes(&registry_record.public_key_hex).map_err(invalid_data)?;
    let signature = hex_to_bytes(&vote.signature_hex).map_err(invalid_data)?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        &message,
        &signature,
        BLOCK_CERTIFICATE_SIGNATURE_CONTEXT,
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "block {} certificate vote signature mismatch",
                evidence.height
            ),
        ));
    }
    Ok(())
}

pub(super) fn verify_block_timeout_vote_for_target(
    genesis: &Genesis,
    block_height: u64,
    view: u64,
    validator_registry: &ValidatorRegistry,
    vote: &BlockTimeoutVote,
    expected_validator: &str,
    expected_registry_root: &str,
) -> io::Result<()> {
    if vote.validator != expected_validator {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block timeout vote validator mismatch for `{expected_validator}`"),
        ));
    }
    if vote.high_qc_id.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout vote high_qc_id must be nonempty",
        ));
    }
    if vote.registry_root != expected_registry_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block timeout vote `{expected_validator}` registry root mismatch"),
        ));
    }
    let registry_record = validator_registry_record(validator_registry, expected_validator)?;
    if vote.algorithm_id != registry_record.algorithm_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block timeout vote `{expected_validator}` algorithm mismatch"),
        ));
    }
    if !vote.public_key_hex.is_empty() && vote.public_key_hex != registry_record.public_key_hex {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block timeout vote `{expected_validator}` public key mismatch"),
        ));
    }
    let message = block_timeout_vote_message(
        genesis,
        block_height,
        view,
        &vote.high_qc_id,
        expected_validator,
        expected_registry_root,
    )?;
    let expected_vote_id = block_timeout_vote_id(&message);
    if vote.vote_id != expected_vote_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block timeout vote `{expected_validator}` id mismatch"),
        ));
    }
    let public_key = hex_to_bytes(&registry_record.public_key_hex).map_err(invalid_data)?;
    let signature = hex_to_bytes(&vote.signature_hex).map_err(invalid_data)?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        &message,
        &signature,
        BLOCK_TIMEOUT_SIGNATURE_CONTEXT,
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("block timeout vote `{expected_validator}` signature mismatch"),
        ));
    }
    Ok(())
}

pub(super) fn verify_block_timeout_certificate_material(
    genesis: &Genesis,
    validator_registry: &ValidatorRegistry,
    expected_validators: &[String],
    certificate_file: &BlockTimeoutCertificateFile,
) -> io::Result<()> {
    if certificate_file.schema != BLOCK_TIMEOUT_CERTIFICATE_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "unsupported block timeout certificate schema `{}`",
                certificate_file.schema
            ),
        ));
    }
    let expected_genesis_hash = genesis_hash(genesis);
    if certificate_file.chain_id != genesis.chain_id
        || certificate_file.genesis_hash != expected_genesis_hash
        || certificate_file.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout certificate chain domain mismatch",
        ));
    }
    if certificate_file.block_height == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout certificate height must be positive",
        ));
    }
    if certificate_file.certificate.validators != expected_validators {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout certificate validator set mismatch",
        ));
    }
    let expected_quorum = block_certificate_quorum(expected_validators)?;
    if certificate_file.certificate.quorum != expected_quorum
        || certificate_file.certificate.votes.len() < expected_quorum
        || certificate_file.certificate.votes.len() > expected_validators.len()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout certificate quorum mismatch",
        ));
    }
    validate_block_timeout_vote_set(
        &certificate_file.certificate.votes,
        expected_validators,
        "block timeout certificate",
    )?;
    let expected_high_qc_id = certificate_file
        .certificate
        .votes
        .iter()
        .map(|vote| vote.high_qc_id.clone())
        .max()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "block timeout certificate has no votes",
            )
        })?;
    if certificate_file.certificate.high_qc_id != expected_high_qc_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout certificate high_qc mismatch",
        ));
    }
    let registry_root = certificate_registry_root(
        &certificate_file.certificate.registry_root,
        validator_registry,
        expected_validators,
        "block timeout certificate",
    )?;
    for vote in &certificate_file.certificate.votes {
        verify_block_timeout_vote_for_target(
            genesis,
            certificate_file.block_height,
            certificate_file.view,
            validator_registry,
            vote,
            &vote.validator,
            &registry_root,
        )?;
    }
    let expected_hotstuff_certificate_id = verify_hotstuff_timeout_certificate(
        genesis,
        expected_validators,
        certificate_file.block_height,
        certificate_file.view,
        &certificate_file.certificate.votes,
    )?;
    if certificate_file.hotstuff_certificate_id != expected_hotstuff_certificate_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout certificate hotstuff id mismatch",
        ));
    }
    let expected_certificate_id = block_timeout_certificate_id(
        genesis,
        certificate_file.block_height,
        certificate_file.view,
        &certificate_file.hotstuff_certificate_id,
        &certificate_file.certificate,
    )?;
    if certificate_file.certificate_id != expected_certificate_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout certificate id mismatch",
        ));
    }
    Ok(())
}

pub(super) fn local_validator_ids(validator_count: u32) -> io::Result<Vec<String>> {
    if validator_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "block certificate validator count must be positive",
        ));
    }
    Ok((0..validator_count)
        .map(|index| format!("validator-{index}"))
        .collect())
}

pub fn active_validator_ids_for_node(options: NodeOptions) -> io::Result<Vec<String>> {
    let store = NodeStore::new(&options.data_dir);
    let governance = store.read_governance()?;
    active_validator_ids(&governance)
}

pub(super) fn active_validator_ids(governance: &GovernanceState) -> io::Result<Vec<String>> {
    if governance.active_validators.is_empty() {
        return local_validator_ids(governance.active_validator_count);
    }
    validate_active_validator_ids(
        &governance.active_validators,
        "governance active validators",
    )?;
    if governance.active_validators.len() != governance.active_validator_count as usize {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance active validator count does not match active validator list",
        ));
    }
    Ok(governance.active_validators.clone())
}

pub(super) fn set_active_validator_ids(
    governance: &mut GovernanceState,
    validators: Vec<String>,
) -> io::Result<()> {
    validate_active_validator_ids(&validators, "governance active validators")?;
    let validator_count = u32::try_from(validators.len()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "governance active validator list exceeds u32",
        )
    })?;
    governance.active_validator_count = validator_count;
    if validators == local_validator_ids(validator_count)? {
        governance.active_validators.clear();
    } else {
        governance.active_validators = validators;
    }
    Ok(())
}

pub(super) fn validate_active_validator_ids(
    validators: &[String],
    context: &str,
) -> io::Result<()> {
    if validators.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{context} must be nonempty"),
        ));
    }
    let mut sorted = validators.to_vec();
    sorted.sort();
    sorted.dedup();
    if sorted != validators {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{context} must be sorted unique"),
        ));
    }
    Ok(())
}

pub(super) fn block_certificate_quorum(validators: &[String]) -> io::Result<usize> {
    bft_quorum_threshold(validators.len()).map_err(invalid_data)
}

pub(super) fn certificate_registry_root_or_legacy(
    certificate_root: &str,
    registry: &ValidatorRegistry,
    expected_validators: &[String],
    context: &str,
) -> io::Result<String> {
    if certificate_root.is_empty() {
        return Ok(String::new());
    }
    let expected_root = validator_registry_root(registry, expected_validators)?;
    if certificate_root != expected_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{context} registry root mismatch"),
        ));
    }
    Ok(expected_root)
}

pub(super) fn certificate_registry_root(
    certificate_root: &str,
    registry: &ValidatorRegistry,
    expected_validators: &[String],
    context: &str,
) -> io::Result<String> {
    if certificate_root.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{context} registry root is required"),
        ));
    }
    certificate_registry_root_or_legacy(certificate_root, registry, expected_validators, context)
}

pub(super) fn validate_block_certificate_vote_set(
    votes: &[BlockCertificateVote],
    expected_validators: &[String],
    context: &str,
) -> io::Result<()> {
    let mut seen_validators = HashSet::new();
    let mut last_validator_index = None;
    for vote in votes {
        let validator_index = expected_validators
            .iter()
            .position(|validator| validator == &vote.validator)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{context} unexpected vote validator"),
                )
            })?;
        if !seen_validators.insert(vote.validator.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{context} duplicate vote validator"),
            ));
        }
        if last_validator_index.is_some_and(|last_index| validator_index <= last_index) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{context} vote validator order mismatch"),
            ));
        }
        last_validator_index = Some(validator_index);
    }
    Ok(())
}

pub(super) fn validate_block_timeout_vote_set(
    votes: &[BlockTimeoutVote],
    expected_validators: &[String],
    context: &str,
) -> io::Result<()> {
    let mut seen_validators = HashSet::new();
    let mut last_validator_index = None;
    for vote in votes {
        let validator_index = expected_validators
            .iter()
            .position(|validator| validator == &vote.validator)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("{context} unexpected timeout vote validator"),
                )
            })?;
        if !seen_validators.insert(vote.validator.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{context} duplicate timeout vote validator"),
            ));
        }
        if last_validator_index.is_some_and(|last_index| validator_index <= last_index) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("{context} timeout vote validator order mismatch"),
            ));
        }
        last_validator_index = Some(validator_index);
    }
    Ok(())
}

pub(super) fn block_proposal_signature_message(
    proposal: &BlockProposalFile,
) -> io::Result<Vec<u8>> {
    if !proposal.fastpay_pre_state_effects.is_empty() {
        return serde_json::to_vec(&(
            "postfiat.block_proposal.signature-message.v2",
            proposal.chain_id.as_str(),
            proposal.genesis_hash.as_str(),
            proposal.protocol_version,
            proposal.block_height,
            proposal.view,
            proposal.parent_hash.as_str(),
            proposal.proposer.as_str(),
            proposal.batch_kind.as_str(),
            proposal.batch_id.as_str(),
            proposal.payload_hash.as_str(),
            proposal.state_root.as_str(),
            proposal.receipt_ids.as_slice(),
            proposal.fastpay_pre_state_effects.as_slice(),
        ))
        .map_err(invalid_data);
    }
    serde_json::to_vec(&(
        proposal.chain_id.as_str(),
        proposal.genesis_hash.as_str(),
        proposal.protocol_version,
        proposal.block_height,
        proposal.view,
        proposal.parent_hash.as_str(),
        proposal.proposer.as_str(),
        proposal.batch_kind.as_str(),
        proposal.batch_id.as_str(),
        proposal.payload_hash.as_str(),
        proposal.state_root.as_str(),
        proposal.receipt_ids.as_slice(),
    ))
    .map_err(invalid_data)
}

pub(super) fn block_certificate_vote_message(
    genesis: &Genesis,
    evidence: &BlockEvidence<'_>,
    validator: &str,
    accept: bool,
    registry_root: &str,
) -> io::Result<Vec<u8>> {
    if !evidence.fastpay_pre_state_effects.is_empty() {
        return serde_json::to_vec(&(
            "postfiat.block_certificate_vote.message.v2",
            genesis.chain_id.as_str(),
            genesis_hash(genesis),
            genesis.protocol_version,
            registry_root,
            evidence.height,
            evidence.view,
            evidence.parent_hash,
            evidence.proposer,
            evidence.batch_kind,
            evidence.batch_id,
            evidence.state_root,
            evidence.receipt_ids,
            evidence.fastpay_pre_state_effects,
            validator,
            accept,
        ))
        .map_err(invalid_data);
    }
    if registry_root.is_empty() {
        return serde_json::to_vec(&(
            genesis.chain_id.as_str(),
            genesis_hash(genesis),
            genesis.protocol_version,
            evidence.height,
            evidence.view,
            evidence.parent_hash,
            evidence.proposer,
            evidence.batch_kind,
            evidence.batch_id,
            evidence.state_root,
            evidence.receipt_ids,
            validator,
            accept,
        ))
        .map_err(invalid_data);
    }
    serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash(genesis),
        genesis.protocol_version,
        registry_root,
        evidence.height,
        evidence.view,
        evidence.parent_hash,
        evidence.proposer,
        evidence.batch_kind,
        evidence.batch_id,
        evidence.state_root,
        evidence.receipt_ids,
        validator,
        accept,
    ))
    .map_err(invalid_data)
}

pub(super) fn block_timeout_vote_message(
    genesis: &Genesis,
    block_height: u64,
    view: u64,
    high_qc_id: &str,
    validator: &str,
    registry_root: &str,
) -> io::Result<Vec<u8>> {
    if registry_root.is_empty() {
        return serde_json::to_vec(&(
            genesis.chain_id.as_str(),
            genesis_hash(genesis),
            genesis.protocol_version,
            block_height,
            view,
            high_qc_id,
            validator,
        ))
        .map_err(invalid_data);
    }
    serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash(genesis),
        genesis.protocol_version,
        registry_root,
        block_height,
        view,
        high_qc_id,
        validator,
    ))
    .map_err(invalid_data)
}

pub(super) fn block_certificate_vote_id(message: &[u8]) -> String {
    hash_hex("postfiat.block_certificate_vote.local.v1", message)
}

pub(super) fn block_timeout_vote_id(message: &[u8]) -> String {
    hash_hex("postfiat.block_timeout_vote.local.v1", message)
}

pub(super) fn block_proposal_signature_seed(message: &[u8]) -> io::Result<[u8; 32]> {
    let digest = hash_bytes("postfiat.block_proposal.signature_seed.v1", message);
    digest[..32].try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "block proposal signature seed length mismatch",
        )
    })
}

pub(super) fn block_certificate_signature_seed(message: &[u8]) -> io::Result<[u8; 32]> {
    let digest = hash_bytes("postfiat.block_certificate.signature_seed.v1", message);
    digest[..32].try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "block certificate signature seed length mismatch",
        )
    })
}

pub(super) fn block_timeout_signature_seed(message: &[u8]) -> io::Result<[u8; 32]> {
    let digest = hash_bytes("postfiat.block_timeout.signature_seed.v1", message);
    digest[..32].try_into().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "block timeout signature seed length mismatch",
        )
    })
}

pub(super) fn block_certificate_id(
    genesis: &Genesis,
    evidence: &BlockEvidence<'_>,
    certificate: &BlockCertificate,
) -> io::Result<String> {
    if !evidence.fastpay_pre_state_effects.is_empty() {
        let encoded = serde_json::to_vec(&(
            "postfiat.block_certificate.v4",
            (
                genesis.chain_id.as_str(),
                genesis_hash(genesis),
                genesis.protocol_version,
                certificate.registry_root.as_str(),
            ),
            (
                evidence.height,
                evidence.view,
                evidence.parent_hash,
                evidence.proposer,
                evidence.batch_kind,
                evidence.batch_id,
                evidence.state_root,
                evidence.receipt_ids,
                evidence.fastpay_pre_state_effects,
            ),
            (
                &certificate.validators,
                certificate.quorum,
                &certificate.votes,
            ),
        ))
        .map_err(invalid_data)?;
        return Ok(hash_hex("postfiat.block_certificate.local.v4", &encoded));
    }
    if certificate.registry_root.is_empty() {
        let encoded = serde_json::to_vec(&(
            genesis.chain_id.as_str(),
            genesis_hash(genesis),
            genesis.protocol_version,
            evidence.height,
            evidence.view,
            evidence.parent_hash,
            evidence.proposer,
            evidence.batch_kind,
            evidence.batch_id,
            evidence.state_root,
            evidence.receipt_ids,
            &certificate.validators,
            certificate.quorum,
            &certificate.votes,
        ))
        .map_err(invalid_data)?;
        return Ok(hash_hex("postfiat.block_certificate.local.v2", &encoded));
    }
    let encoded = serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash(genesis),
        genesis.protocol_version,
        certificate.registry_root.as_str(),
        evidence.height,
        evidence.view,
        evidence.parent_hash,
        evidence.proposer,
        evidence.batch_kind,
        evidence.batch_id,
        evidence.state_root,
        evidence.receipt_ids,
        &certificate.validators,
        certificate.quorum,
        &certificate.votes,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex("postfiat.block_certificate.local.v3", &encoded))
}

pub(super) fn block_timeout_certificate_id(
    genesis: &Genesis,
    block_height: u64,
    view: u64,
    hotstuff_certificate_id: &str,
    certificate: &BlockTimeoutCertificate,
) -> io::Result<String> {
    if certificate.registry_root.is_empty() {
        let encoded = serde_json::to_vec(&(
            genesis.chain_id.as_str(),
            genesis_hash(genesis),
            genesis.protocol_version,
            block_height,
            view,
            hotstuff_certificate_id,
            &certificate.validators,
            certificate.quorum,
            certificate.high_qc_id.as_str(),
            &certificate.votes,
        ))
        .map_err(invalid_data)?;
        return Ok(hash_hex(
            "postfiat.block_timeout_certificate.local.v1",
            &encoded,
        ));
    }
    let encoded = serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash(genesis),
        genesis.protocol_version,
        certificate.registry_root.as_str(),
        block_height,
        view,
        hotstuff_certificate_id,
        &certificate.validators,
        certificate.quorum,
        certificate.high_qc_id.as_str(),
        &certificate.votes,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex(
        "postfiat.block_timeout_certificate.local.v2",
        &encoded,
    ))
}

pub(super) fn verify_hotstuff_timeout_certificate(
    genesis: &Genesis,
    validators: &[String],
    block_height: u64,
    view: u64,
    votes: &[BlockTimeoutVote],
) -> io::Result<String> {
    let domain = ConsensusDomain {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
    };
    let validator_set = ValidatorSet::try_new(validators.to_vec()).map_err(invalid_data)?;
    let timeout_votes = votes
        .iter()
        .map(|vote| {
            TimeoutVote::new(
                &domain,
                block_height,
                view,
                vote.high_qc_id.clone(),
                vote.validator.clone(),
            )
            .map_err(invalid_data)
        })
        .collect::<io::Result<Vec<_>>>()?;
    let certificate = certify_timeout(&domain, &validator_set, block_height, view, timeout_votes)
        .map_err(invalid_data)?;
    verify_timeout_certificate(&domain, &validator_set, &certificate).map_err(invalid_data)?;
    Ok(certificate.certificate_id)
}

pub(super) fn write_batch_file(path: &Path, batch: &TransactionBatch) -> io::Result<()> {
    let json = serde_json::to_string_pretty(batch).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_batch_file(path: &Path) -> io::Result<TransactionBatch> {
    read_json_file(path, "transaction batch")
}

#[cfg(test)]
pub(super) fn write_signed_transfer_file(path: &Path, transfer: &SignedTransfer) -> io::Result<()> {
    transfer
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let json = serde_json::to_string_pretty(transfer).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_signed_transfer_file(path: &Path) -> io::Result<SignedTransfer> {
    let transfer: SignedTransfer = read_json_file(path, "signed transfer")?;
    transfer
        .validate()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    Ok(transfer)
}

pub(super) fn build_shielded_action_batch(
    genesis: &Genesis,
    actions: Vec<ShieldedAction>,
) -> io::Result<ShieldedActionBatch> {
    let batch_id = chain_bound_action_batch_id(
        genesis,
        "postfiat.shielded_action_batch.v1",
        "shielded",
        &actions,
    )?;
    Ok(ShieldedActionBatch::new(batch_id, actions))
}

pub(super) fn verify_shielded_action_batch_id(
    genesis: &Genesis,
    batch: &ShieldedActionBatch,
) -> io::Result<()> {
    let expected = chain_bound_action_batch_id(
        genesis,
        "postfiat.shielded_action_batch.v1",
        "shielded",
        &batch.actions,
    )?;
    if batch.batch_id != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "shielded batch id mismatch",
        ));
    }
    Ok(())
}

pub(super) fn reject_live_legacy_cleartext_shielded_actions(
    batch: &ShieldedActionBatch,
) -> io::Result<()> {
    if batch.actions.iter().any(|action| {
        matches!(
            action,
            ShieldedAction::Mint(_)
                | ShieldedAction::Spend(_)
                | ShieldedAction::AssetOrchardIngressV1(_)
        )
    }) {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "legacy cleartext shield_mint/shield_spend and AssetOrchard ingress v1 actions are historical-replay-only; use Asset-Orchard ingress v2",
        ));
    }
    Ok(())
}

pub(super) fn write_shielded_action_batch_file(
    path: &Path,
    batch: &ShieldedActionBatch,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(batch).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_shielded_action_batch_file(path: &Path) -> io::Result<ShieldedActionBatch> {
    read_json_file(path, "shielded action batch")
}

pub(super) fn build_bridge_action_batch(
    genesis: &Genesis,
    actions: Vec<BridgeAction>,
) -> io::Result<BridgeActionBatch> {
    let batch_id = chain_bound_action_batch_id(
        genesis,
        "postfiat.bridge_action_batch.v1",
        "bridge",
        &actions,
    )?;
    Ok(BridgeActionBatch::new(batch_id, actions))
}

pub(super) fn verify_bridge_action_batch_id(
    genesis: &Genesis,
    batch: &BridgeActionBatch,
) -> io::Result<()> {
    let expected = chain_bound_action_batch_id(
        genesis,
        "postfiat.bridge_action_batch.v1",
        "bridge",
        &batch.actions,
    )?;
    if batch.batch_id != expected {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "bridge batch id mismatch",
        ));
    }
    Ok(())
}

pub(super) fn write_bridge_action_batch_file(
    path: &Path,
    batch: &BridgeActionBatch,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(batch).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_bridge_action_batch_file(path: &Path) -> io::Result<BridgeActionBatch> {
    read_json_file(path, "bridge action batch")
}

pub(super) fn build_governance_action_batch(
    genesis: &Genesis,
    amendments: Vec<GovernanceAmendment>,
    registry_updates: Vec<ValidatorRegistryUpdateRecord>,
) -> io::Result<GovernanceActionBatch> {
    build_governance_action_batch_with_agent_dry_runs(
        genesis,
        amendments,
        registry_updates,
        Vec::new(),
    )
}

pub(super) fn build_governance_action_batch_with_agent_dry_runs(
    genesis: &Genesis,
    amendments: Vec<GovernanceAmendment>,
    registry_updates: Vec<ValidatorRegistryUpdateRecord>,
    governance_agent_dry_runs: Vec<GovernanceAgentDryRunAmendment>,
) -> io::Result<GovernanceActionBatch> {
    let batch_id = if governance_agent_dry_runs.is_empty() {
        chain_bound_action_batch_id(
            genesis,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(&amendments, &registry_updates),
        )?
    } else {
        chain_bound_action_batch_id(
            genesis,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(&amendments, &registry_updates, &governance_agent_dry_runs),
        )?
    };
    Ok(GovernanceActionBatch::with_governance_agent_dry_runs(
        batch_id,
        amendments,
        registry_updates,
        governance_agent_dry_runs,
    ))
}

pub(super) fn build_governance_action_batch_with_fastswap_bootstraps(
    genesis: &Genesis,
    fastswap_bootstraps: Vec<postfiat_types::FastSwapGovernanceBootstrapV1>,
) -> io::Result<GovernanceActionBatch> {
    let amendments: Vec<GovernanceAmendment> = Vec::new();
    let registry_updates: Vec<ValidatorRegistryUpdateRecord> = Vec::new();
    let governance_agent_dry_runs: Vec<GovernanceAgentDryRunAmendment> = Vec::new();
    let batch_id = chain_bound_action_batch_id(
        genesis,
        "postfiat.governance_action_batch.v1",
        "governance",
        &(
            &amendments,
            &registry_updates,
            &governance_agent_dry_runs,
            &fastswap_bootstraps,
        ),
    )?;
    Ok(GovernanceActionBatch {
        batch_id,
        amendments,
        validator_registry_updates: registry_updates,
        governance_agent_dry_runs,
        fastswap_bootstraps,
        fastpay_recovery_bootstraps: Vec::new(),
        vault_bridge_route_profile_activations: Vec::new(),
    })
}

pub(super) fn build_governance_action_batch_with_fastpay_recovery_bootstrap(
    genesis: &Genesis,
    bootstrap: postfiat_types::FastPayRecoveryGovernanceBootstrapV1,
) -> io::Result<GovernanceActionBatch> {
    let bootstraps = vec![bootstrap.clone()];
    let batch_id = chain_bound_action_batch_id(
        genesis,
        "postfiat.governance_action_batch.v1",
        "governance",
        &(
            Vec::<GovernanceAmendment>::new(),
            Vec::<ValidatorRegistryUpdateRecord>::new(),
            Vec::<GovernanceAgentDryRunAmendment>::new(),
            Vec::<postfiat_types::FastSwapGovernanceBootstrapV1>::new(),
            &bootstraps,
            Vec::<postfiat_types::VaultBridgeRouteProfileActivationV1>::new(),
        ),
    )?;
    Ok(GovernanceActionBatch::with_fastpay_recovery_bootstrap(
        batch_id, bootstrap,
    ))
}

pub(super) fn build_governance_action_batch_with_vault_bridge_route_profile_activation(
    genesis: &Genesis,
    activation: postfiat_types::VaultBridgeRouteProfileActivationV1,
) -> io::Result<GovernanceActionBatch> {
    let activations = vec![activation.clone()];
    let batch_id = chain_bound_action_batch_id(
        genesis,
        "postfiat.governance_action_batch.v1",
        "governance",
        &(
            Vec::<GovernanceAmendment>::new(),
            Vec::<ValidatorRegistryUpdateRecord>::new(),
            Vec::<GovernanceAgentDryRunAmendment>::new(),
            Vec::<postfiat_types::FastSwapGovernanceBootstrapV1>::new(),
            &activations,
        ),
    )?;
    Ok(GovernanceActionBatch::with_vault_bridge_route_profile_activation(batch_id, activation))
}

fn verify_fastswap_governance_bootstrap_evidence(
    genesis: &Genesis,
    bootstrap: &postfiat_types::FastSwapGovernanceBootstrapV1,
) -> io::Result<()> {
    bootstrap.validate_payload().map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "invalid FastSwap bootstrap payload",
        )
    })?;
    verify_governance_amendment_evidence(genesis, &bootstrap.amendment)?;
    let bootstrap_id = bootstrap
        .bootstrap_id()
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid FastSwap bootstrap id"))?;
    let expected_kind = format!(
        "{}{}",
        postfiat_types::FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1,
        bytes_to_hex(&bootstrap_id.0)
    );
    let committee_validators = bootstrap
        .payload
        .committee
        .validators
        .iter()
        .map(|validator| validator.validator_id.as_str())
        .collect::<Vec<_>>();
    if bootstrap.amendment.kind != expected_kind
        || bootstrap.amendment.value != postfiat_types::FASTSWAP_SCHEMA_VERSION_V1
        || bootstrap.amendment.chain_id != genesis.chain_id
        || bootstrap.amendment.genesis_hash != genesis_hash(genesis)
        || bootstrap.amendment.protocol_version != genesis.protocol_version
        || bootstrap.payload.committee.domain.chain.chain_id != genesis.chain_id
        || bytes_to_hex(&bootstrap.payload.committee.domain.chain.genesis_hash.0)
            != genesis_hash(genesis)
        || bootstrap.payload.committee.domain.chain.protocol_version != genesis.protocol_version
        || committee_validators
            != bootstrap
                .amendment
                .validators
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastSwap bootstrap governance binding mismatch",
        ));
    }
    Ok(())
}

fn verify_fastpay_recovery_bootstrap_evidence(
    genesis: &Genesis,
    bootstrap: &postfiat_types::FastPayRecoveryGovernanceBootstrapV1,
) -> io::Result<()> {
    bootstrap
        .validate_payload_binding()
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    verify_governance_amendment_evidence(genesis, &bootstrap.amendment)?;
    if bootstrap.payload.committee.chain_id != genesis.chain_id
        || bootstrap.payload.committee.genesis_hash != genesis_hash(genesis)
        || bootstrap.payload.committee.protocol_version != genesis.protocol_version
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay recovery bootstrap chain domain mismatch",
        ));
    }
    Ok(())
}

pub(super) fn verify_governance_action_batch_id(
    genesis: &Genesis,
    batch: &GovernanceActionBatch,
) -> io::Result<()> {
    if !batch.fastswap_bootstraps.is_empty()
        && (batch.fastswap_bootstraps.len() != 1
            || !batch.amendments.is_empty()
            || !batch.validator_registry_updates.is_empty()
            || !batch.governance_agent_dry_runs.is_empty()
            || !batch.fastpay_recovery_bootstraps.is_empty()
            || !batch.vault_bridge_route_profile_activations.is_empty())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastSwap governance bootstrap must be the only action in its batch",
        ));
    }
    if !batch.vault_bridge_route_profile_activations.is_empty()
        && (batch.vault_bridge_route_profile_activations.len() != 1
            || !batch.amendments.is_empty()
            || !batch.validator_registry_updates.is_empty()
            || !batch.governance_agent_dry_runs.is_empty()
            || !batch.fastswap_bootstraps.is_empty()
            || !batch.fastpay_recovery_bootstraps.is_empty())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "vault bridge route profile activation must be the only action in its batch",
        ));
    }
    if !batch.fastpay_recovery_bootstraps.is_empty()
        && (batch.fastpay_recovery_bootstraps.len() != 1
            || !batch.amendments.is_empty()
            || !batch.validator_registry_updates.is_empty()
            || !batch.governance_agent_dry_runs.is_empty()
            || !batch.fastswap_bootstraps.is_empty()
            || !batch.vault_bridge_route_profile_activations.is_empty())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay recovery bootstrap must be the only action in its batch",
        ));
    }
    for amendment in &batch.amendments {
        verify_governance_amendment_evidence(genesis, amendment)?;
    }
    let domain = cobalt_domain(genesis);
    for update in &batch.validator_registry_updates {
        verify_cobalt_validator_registry_update(&domain, update)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    }
    for dry_run in &batch.governance_agent_dry_runs {
        if dry_run.chain_id != genesis.chain_id
            || dry_run.genesis_hash != genesis_hash(genesis)
            || dry_run.protocol_version != genesis.protocol_version
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance agent dry-run domain mismatch",
            ));
        }
        validate_governance_agent_dry_run_amendment(dry_run)?;
    }
    for bootstrap in &batch.fastswap_bootstraps {
        verify_fastswap_governance_bootstrap_evidence(genesis, bootstrap)?;
    }
    for bootstrap in &batch.fastpay_recovery_bootstraps {
        verify_fastpay_recovery_bootstrap_evidence(genesis, bootstrap)?;
    }
    for activation in &batch.vault_bridge_route_profile_activations {
        activation
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        verify_governance_amendment_evidence(genesis, &activation.amendment)?;
    }
    if batch.amendments.is_empty()
        && batch.validator_registry_updates.is_empty()
        && batch.governance_agent_dry_runs.is_empty()
        && batch.fastswap_bootstraps.is_empty()
        && batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance batch has no actions",
        ));
    }
    let expected = chain_bound_action_batch_id(
        genesis,
        "postfiat.governance_action_batch.v1",
        "governance",
        &(
            &batch.amendments,
            &batch.validator_registry_updates,
            &batch.governance_agent_dry_runs,
            &batch.fastswap_bootstraps,
            &batch.fastpay_recovery_bootstraps,
            &batch.vault_bridge_route_profile_activations,
        ),
    )?;
    let five_tuple_expected = if batch.fastpay_recovery_bootstraps.is_empty() {
        Some(chain_bound_action_batch_id(
            genesis,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(
                &batch.amendments,
                &batch.validator_registry_updates,
                &batch.governance_agent_dry_runs,
                &batch.fastswap_bootstraps,
                &batch.vault_bridge_route_profile_activations,
            ),
        )?)
    } else {
        None
    };
    let four_tuple_expected = if batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        Some(chain_bound_action_batch_id(
            genesis,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(
                &batch.amendments,
                &batch.validator_registry_updates,
                &batch.governance_agent_dry_runs,
                &batch.fastswap_bootstraps,
            ),
        )?)
    } else {
        None
    };
    let three_tuple_expected = if batch.fastswap_bootstraps.is_empty()
        && batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        Some(chain_bound_action_batch_id(
            genesis,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(
                &batch.amendments,
                &batch.validator_registry_updates,
                &batch.governance_agent_dry_runs,
            ),
        )?)
    } else {
        None
    };
    let two_tuple_expected = if batch.governance_agent_dry_runs.is_empty()
        && batch.fastswap_bootstraps.is_empty()
        && batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        Some(chain_bound_action_batch_id(
            genesis,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(&batch.amendments, &batch.validator_registry_updates),
        )?)
    } else {
        None
    };
    let legacy_expected = if batch.validator_registry_updates.is_empty()
        && batch.governance_agent_dry_runs.is_empty()
        && batch.fastswap_bootstraps.is_empty()
        && batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        Some(chain_bound_action_batch_id(
            genesis,
            "postfiat.governance_action_batch.v1",
            "governance",
            &batch.amendments,
        )?)
    } else {
        None
    };
    if batch.batch_id != expected
        && five_tuple_expected.as_ref() != Some(&batch.batch_id)
        && four_tuple_expected.as_ref() != Some(&batch.batch_id)
        && three_tuple_expected.as_ref() != Some(&batch.batch_id)
        && two_tuple_expected.as_ref() != Some(&batch.batch_id)
        && legacy_expected.as_ref() != Some(&batch.batch_id)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance batch id mismatch",
        ));
    }
    Ok(())
}

pub(super) fn verify_archived_governance_action_batch_id(
    genesis: &Genesis,
    batch: &GovernanceActionBatch,
) -> io::Result<()> {
    if !batch.fastswap_bootstraps.is_empty()
        && (batch.fastswap_bootstraps.len() != 1
            || !batch.amendments.is_empty()
            || !batch.validator_registry_updates.is_empty()
            || !batch.governance_agent_dry_runs.is_empty()
            || !batch.fastpay_recovery_bootstraps.is_empty()
            || !batch.vault_bridge_route_profile_activations.is_empty())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastSwap governance bootstrap must be the only action in its batch",
        ));
    }
    if !batch.vault_bridge_route_profile_activations.is_empty()
        && (batch.vault_bridge_route_profile_activations.len() != 1
            || !batch.amendments.is_empty()
            || !batch.validator_registry_updates.is_empty()
            || !batch.governance_agent_dry_runs.is_empty()
            || !batch.fastswap_bootstraps.is_empty()
            || !batch.fastpay_recovery_bootstraps.is_empty())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "vault bridge route profile activation must be the only action in its batch",
        ));
    }
    if !batch.fastpay_recovery_bootstraps.is_empty()
        && (batch.fastpay_recovery_bootstraps.len() != 1
            || !batch.amendments.is_empty()
            || !batch.validator_registry_updates.is_empty()
            || !batch.governance_agent_dry_runs.is_empty()
            || !batch.fastswap_bootstraps.is_empty()
            || !batch.vault_bridge_route_profile_activations.is_empty())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay recovery bootstrap must be the only action in its batch",
        ));
    }
    for amendment in &batch.amendments {
        verify_governance_amendment_evidence(genesis, amendment)?;
    }
    for update in &batch.validator_registry_updates {
        verify_historical_cobalt_validator_registry_update(genesis, update)?;
    }
    for dry_run in &batch.governance_agent_dry_runs {
        if dry_run.chain_id != genesis.chain_id
            || dry_run.genesis_hash != genesis_hash(genesis)
            || dry_run.protocol_version != genesis.protocol_version
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance agent dry-run domain mismatch",
            ));
        }
        validate_governance_agent_dry_run_amendment(dry_run)?;
    }
    for bootstrap in &batch.fastswap_bootstraps {
        verify_fastswap_governance_bootstrap_evidence(genesis, bootstrap)?;
    }
    for bootstrap in &batch.fastpay_recovery_bootstraps {
        verify_fastpay_recovery_bootstrap_evidence(genesis, bootstrap)?;
    }
    for activation in &batch.vault_bridge_route_profile_activations {
        activation
            .validate()
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        verify_governance_amendment_evidence(genesis, &activation.amendment)?;
    }
    if batch.amendments.is_empty()
        && batch.validator_registry_updates.is_empty()
        && batch.governance_agent_dry_runs.is_empty()
        && batch.fastswap_bootstraps.is_empty()
        && batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance batch has no actions",
        ));
    }
    if governance_action_batch_id_matches_genesis_hash(genesis, batch, &genesis_hash(genesis))? {
        return Ok(());
    }
    if batch.amendments.is_empty()
        && batch.governance_agent_dry_runs.is_empty()
        && batch.fastswap_bootstraps.is_empty()
        && batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        if let Some(embedded_genesis_hash) = common_embedded_registry_update_genesis_hash(batch) {
            if embedded_genesis_hash != genesis_hash(genesis)
                && governance_action_batch_id_matches_genesis_hash(
                    genesis,
                    batch,
                    &embedded_genesis_hash,
                )?
            {
                return Ok(());
            }
        }
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidData,
        "governance batch id mismatch",
    ))
}

pub(super) fn common_embedded_registry_update_genesis_hash(
    batch: &GovernanceActionBatch,
) -> Option<String> {
    let mut updates = batch.validator_registry_updates.iter();
    let first = updates.next()?.genesis_hash.clone();
    if updates.all(|update| update.genesis_hash == first) {
        Some(first)
    } else {
        None
    }
}

pub(super) fn governance_action_batch_id_matches_genesis_hash(
    genesis: &Genesis,
    batch: &GovernanceActionBatch,
    genesis_hash_hex: &str,
) -> io::Result<bool> {
    let expected = chain_bound_action_batch_id_for_genesis_hash(
        genesis,
        genesis_hash_hex,
        "postfiat.governance_action_batch.v1",
        "governance",
        &(
            &batch.amendments,
            &batch.validator_registry_updates,
            &batch.governance_agent_dry_runs,
            &batch.fastswap_bootstraps,
            &batch.fastpay_recovery_bootstraps,
            &batch.vault_bridge_route_profile_activations,
        ),
    )?;
    let five_tuple_expected = if batch.fastpay_recovery_bootstraps.is_empty() {
        Some(chain_bound_action_batch_id_for_genesis_hash(
            genesis,
            genesis_hash_hex,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(
                &batch.amendments,
                &batch.validator_registry_updates,
                &batch.governance_agent_dry_runs,
                &batch.fastswap_bootstraps,
                &batch.vault_bridge_route_profile_activations,
            ),
        )?)
    } else {
        None
    };
    let four_tuple_expected = if batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        Some(chain_bound_action_batch_id_for_genesis_hash(
            genesis,
            genesis_hash_hex,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(
                &batch.amendments,
                &batch.validator_registry_updates,
                &batch.governance_agent_dry_runs,
                &batch.fastswap_bootstraps,
            ),
        )?)
    } else {
        None
    };
    let three_tuple_expected = if batch.fastswap_bootstraps.is_empty()
        && batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        Some(chain_bound_action_batch_id_for_genesis_hash(
            genesis,
            genesis_hash_hex,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(
                &batch.amendments,
                &batch.validator_registry_updates,
                &batch.governance_agent_dry_runs,
            ),
        )?)
    } else {
        None
    };
    let two_tuple_expected = if batch.governance_agent_dry_runs.is_empty()
        && batch.fastswap_bootstraps.is_empty()
        && batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        Some(chain_bound_action_batch_id_for_genesis_hash(
            genesis,
            genesis_hash_hex,
            "postfiat.governance_action_batch.v1",
            "governance",
            &(&batch.amendments, &batch.validator_registry_updates),
        )?)
    } else {
        None
    };
    let legacy_expected = if batch.validator_registry_updates.is_empty()
        && batch.governance_agent_dry_runs.is_empty()
        && batch.fastswap_bootstraps.is_empty()
        && batch.fastpay_recovery_bootstraps.is_empty()
        && batch.vault_bridge_route_profile_activations.is_empty()
    {
        Some(chain_bound_action_batch_id_for_genesis_hash(
            genesis,
            genesis_hash_hex,
            "postfiat.governance_action_batch.v1",
            "governance",
            &batch.amendments,
        )?)
    } else {
        None
    };
    Ok(batch.batch_id == expected
        || five_tuple_expected.as_ref() == Some(&batch.batch_id)
        || four_tuple_expected.as_ref() == Some(&batch.batch_id)
        || three_tuple_expected.as_ref() == Some(&batch.batch_id)
        || two_tuple_expected.as_ref() == Some(&batch.batch_id)
        || legacy_expected.as_ref() == Some(&batch.batch_id))
}

pub(super) fn cobalt_domain(genesis: &Genesis) -> CobaltDomain {
    CobaltDomain {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
    }
}

pub(super) fn mempool_batch_domain(genesis: &Genesis) -> MempoolBatchDomain {
    MempoolBatchDomain {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
    }
}

pub(super) fn verify_governance_amendment_evidence(
    genesis: &Genesis,
    amendment: &GovernanceAmendment,
) -> io::Result<()> {
    let domain = cobalt_domain(genesis);
    verify_cobalt_governance_amendment(&domain, amendment)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub(super) const GOVERNANCE_AUTHORIZATION_SIGNATURE_CONTEXT_V2: &[u8] =
    b"postfiat-l1-v2/governance-authorization/v2";

pub(super) fn governance_amendment_authorization_signing_bytes(
    amendment: &GovernanceAmendment,
    authorization: &SignedGovernanceAuthorizationV2,
) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&(
        (
            SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2,
            "amendment",
            amendment.chain_id.as_str(),
            amendment.genesis_hash.as_str(),
            amendment.protocol_version,
            amendment.instance_id.as_str(),
            amendment.proposal_id.as_str(),
            amendment.proposer.as_str(),
            amendment.validators.as_slice(),
            amendment.quorum,
            amendment.kind.as_str(),
            amendment.value,
            amendment.activation_height,
            amendment.veto_until_height,
            amendment.paused,
        ),
        (
            authorization.old_registry_root.as_str(),
            authorization.committee_epoch,
            authorization.proposal_slot,
            authorization.expires_at_height,
            authorization.validator.as_str(),
            authorization.vote_id.as_str(),
            true,
            authorization.algorithm_id.as_str(),
        ),
    ))
    .map_err(invalid_data)
}

pub(super) fn validator_registry_update_authorization_signing_bytes(
    update: &ValidatorRegistryUpdateRecord,
    authorization: &SignedGovernanceAuthorizationV2,
) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&(
        (
            SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2,
            "validator_registry_update",
            update.chain_id.as_str(),
            update.genesis_hash.as_str(),
            update.protocol_version,
            update.instance_id.as_str(),
            update.proposal_id.as_str(),
            update.proposer.as_str(),
            update.validators.as_slice(),
            update.quorum,
            update.activation_height,
            update.previous_registry_root.as_str(),
            update.new_registry_root.as_str(),
            update.operation.as_str(),
            update.subject_node_id.as_str(),
        ),
        (
            update.previous_trust_graph_root.as_deref(),
            update.new_trust_graph_root.as_deref(),
            update.trust_graph_transition_id.as_deref(),
            update.previous_validators.as_slice(),
            update.new_validators.as_slice(),
            update.previous_record.as_ref(),
            update.new_record.as_ref(),
        ),
        (
            authorization.old_registry_root.as_str(),
            authorization.committee_epoch,
            authorization.proposal_slot,
            authorization.expires_at_height,
            authorization.validator.as_str(),
            authorization.vote_id.as_str(),
            true,
            authorization.algorithm_id.as_str(),
        ),
    ))
    .map_err(invalid_data)
}

fn verify_signed_governance_authorization_set<F>(
    label: &str,
    validators: &[String],
    quorum: usize,
    support: &[String],
    votes: &[GovernanceVote],
    authorizations: &[SignedGovernanceAuthorizationV2],
    registry: &ValidatorRegistry,
    expected_registry_root: &str,
    expected_committee_epoch: u64,
    expected_proposal_slot: u64,
    signing_bytes: F,
) -> io::Result<()>
where
    F: Fn(&SignedGovernanceAuthorizationV2) -> io::Result<Vec<u8>>,
{
    if authorizations.len() != votes.len() || authorizations.len() != support.len() {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} requires one signed governance authorization for every support vote"),
        ));
    }
    if authorizations.len() < quorum {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!("{label} signed governance authorization set is below quorum"),
        ));
    }
    let mut prior_validator: Option<&str> = None;
    for ((authorization, vote), support_validator) in authorizations.iter().zip(votes).zip(support)
    {
        if prior_validator.is_some_and(|prior| prior >= authorization.validator.as_str()) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{label} signed authorizations must be sorted unique"),
            ));
        }
        prior_validator = Some(authorization.validator.as_str());
        if authorization.schema != SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2
            || authorization.validator != *support_validator
            || authorization.validator != vote.validator
            || authorization.vote_id != vote.vote_id
            || !vote.accept
            || !validators.contains(&authorization.validator)
            || authorization.old_registry_root != expected_registry_root
            || authorization.committee_epoch != expected_committee_epoch
            || authorization.proposal_slot != expected_proposal_slot
            || authorization.expires_at_height < expected_proposal_slot
            || authorization.algorithm_id != ML_DSA_65_ALGORITHM
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{label} signed authorization binding mismatch"),
            ));
        }
        let record = validator_registry_record(registry, &authorization.validator)?;
        if record.algorithm_id != authorization.algorithm_id {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{label} signed authorization key algorithm mismatch"),
            ));
        }
        let public_key = decode_ml_dsa_65_public_key_hex(
            &format!("{label} validator public key"),
            &record.public_key_hex,
        )?;
        let signature = decode_ml_dsa_65_signature_hex(
            &format!("{label} authorization signature"),
            &authorization.signature_hex,
        )?;
        let message = signing_bytes(authorization)?;
        if !ml_dsa_65_verify_with_context(
            &public_key,
            &message,
            &signature,
            GOVERNANCE_AUTHORIZATION_SIGNATURE_CONTEXT_V2,
        ) {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{label} signed authorization signature verification failed"),
            ));
        }
    }
    Ok(())
}

pub(super) fn verify_live_signed_governance_batch(
    genesis: &Genesis,
    governance: &GovernanceState,
    registry: &ValidatorRegistry,
    batch: &GovernanceActionBatch,
    proposal_slot: u64,
) -> io::Result<()> {
    let validators = active_validator_ids(governance)?;
    let registry_root = validator_registry_root(registry, &validators)?;
    let committee_epoch = governance
        .validator_registry_updates
        .iter()
        .filter(|update| update.activation_height < proposal_slot)
        .count() as u64;
    for amendment in &batch.amendments {
        verify_governance_amendment_evidence(genesis, amendment)?;
        if amendment.validators != validators {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "governance amendment validator set does not match the old active registry",
            ));
        }
        verify_signed_governance_authorization_set(
            "governance amendment",
            &amendment.validators,
            amendment.quorum,
            &amendment.support,
            &amendment.votes,
            &amendment.signed_authorizations,
            registry,
            &registry_root,
            committee_epoch,
            proposal_slot,
            |authorization| {
                governance_amendment_authorization_signing_bytes(amendment, authorization)
            },
        )?;
    }
    for update in &batch.validator_registry_updates {
        let domain = cobalt_domain(genesis);
        verify_cobalt_validator_registry_update(&domain, update)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        if update.validators != validators || update.previous_registry_root != registry_root {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "validator registry update does not bind the old active registry",
            ));
        }
        verify_signed_governance_authorization_set(
            "validator registry update",
            &update.validators,
            update.quorum,
            &update.support,
            &update.votes,
            &update.signed_authorizations,
            registry,
            &registry_root,
            committee_epoch,
            proposal_slot,
            |authorization| {
                validator_registry_update_authorization_signing_bytes(update, authorization)
            },
        )?;
    }
    for bootstrap in &batch.fastswap_bootstraps {
        let amendment = &bootstrap.amendment;
        if amendment.validators != validators {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "FastSwap governance bootstrap validator set does not match the old active registry",
            ));
        }
        verify_signed_governance_authorization_set(
            "FastSwap governance bootstrap",
            &amendment.validators,
            amendment.quorum,
            &amendment.support,
            &amendment.votes,
            &amendment.signed_authorizations,
            registry,
            &registry_root,
            committee_epoch,
            proposal_slot,
            |authorization| {
                governance_amendment_authorization_signing_bytes(amendment, authorization)
            },
        )?;
    }
    for bootstrap in &batch.fastpay_recovery_bootstraps {
        let amendment = &bootstrap.amendment;
        if amendment.validators != validators {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "FastPay recovery bootstrap validator set does not match the active registry",
            ));
        }
        verify_signed_governance_authorization_set(
            "FastPay recovery governance bootstrap",
            &amendment.validators,
            amendment.quorum,
            &amendment.support,
            &amendment.votes,
            &amendment.signed_authorizations,
            registry,
            &registry_root,
            committee_epoch,
            proposal_slot,
            |authorization| {
                governance_amendment_authorization_signing_bytes(amendment, authorization)
            },
        )?;
    }
    for activation in &batch.vault_bridge_route_profile_activations {
        let amendment = &activation.amendment;
        if amendment.validators != validators {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "vault bridge route profile validator set does not match the old active registry",
            ));
        }
        verify_signed_governance_authorization_set(
            "vault bridge route profile activation",
            &amendment.validators,
            amendment.quorum,
            &amendment.support,
            &amendment.votes,
            &amendment.signed_authorizations,
            registry,
            &registry_root,
            committee_epoch,
            proposal_slot,
            |authorization| {
                governance_amendment_authorization_signing_bytes(amendment, authorization)
            },
        )?;
    }
    Ok(())
}

pub(super) fn chain_bound_action_batch_id<T: Serialize>(
    genesis: &Genesis,
    hash_domain: &str,
    batch_kind: &str,
    payload: &T,
) -> io::Result<String> {
    chain_bound_action_batch_id_for_genesis_hash(
        genesis,
        &genesis_hash(genesis),
        hash_domain,
        batch_kind,
        payload,
    )
}

pub(super) fn chain_bound_action_batch_id_for_genesis_hash<T: Serialize>(
    genesis: &Genesis,
    genesis_hash_hex: &str,
    hash_domain: &str,
    batch_kind: &str,
    payload: &T,
) -> io::Result<String> {
    let encoded = serde_json::to_vec(&(
        genesis.chain_id.as_str(),
        genesis_hash_hex,
        genesis.protocol_version,
        batch_kind,
        payload,
    ))
    .map_err(invalid_data)?;
    Ok(hash_hex(hash_domain, &encoded))
}

pub(super) fn write_governance_action_batch_file(
    path: &Path,
    batch: &GovernanceActionBatch,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(batch).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_governance_action_batch_file(path: &Path) -> io::Result<GovernanceActionBatch> {
    read_json_file(path, "governance action batch")
}

pub(super) fn read_operator_manifest_file(path: &Path) -> io::Result<OperatorManifest> {
    let raw = read_bounded_json_text_file(path, "operator manifest")?;
    reject_operator_manifest_private_material(&raw)?;
    serde_json::from_str(&raw).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "failed to parse operator manifest `{}`: {error}",
                path.display()
            ),
        )
    })
}

pub(super) fn write_operator_manifest_file(
    path: &Path,
    manifest: &OperatorManifest,
) -> io::Result<()> {
    reject_operator_manifest_private_material(
        &serde_json::to_string(manifest).map_err(invalid_data)?,
    )?;
    verify_operator_manifest_record(manifest, path)?;
    let json = serde_json::to_string_pretty(manifest).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn write_governance_genesis_bundle_file(
    path: &Path,
    bundle: &GovernanceGenesisBundle,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(bundle).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_governance_genesis_bundle_file(
    path: &Path,
) -> io::Result<GovernanceGenesisBundle> {
    read_json_file(path, "governance genesis bundle")
}

pub(super) fn write_validator_registry_update_file(
    path: &Path,
    update: &ValidatorRegistryUpdateRecord,
) -> io::Result<()> {
    let json = serde_json::to_string_pretty(update).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_validator_registry_update_file(
    path: &Path,
) -> io::Result<ValidatorRegistryUpdateRecord> {
    read_json_file(path, "validator registry update")
}

pub(super) fn canonical_governance_replay_batch_payload_json(
    batch_kind: &str,
    batch_file: &Path,
) -> io::Result<String> {
    match normalize_block_proposal_batch_kind(Some(batch_kind))? {
        BATCH_KIND_TRANSPARENT => {
            let batch = read_batch_file(batch_file)?;
            serde_json::to_string(&batch).map_err(invalid_data)
        }
        BATCH_KIND_GOVERNANCE => {
            let batch = read_governance_action_batch_file(batch_file)?;
            serde_json::to_string(&batch).map_err(invalid_data)
        }
        BATCH_KIND_SHIELDED => {
            let batch = read_shielded_action_batch_file(batch_file)?;
            serde_json::to_string(&batch).map_err(invalid_data)
        }
        BATCH_KIND_BRIDGE => {
            let batch = read_bridge_action_batch_file(batch_file)?;
            serde_json::to_string(&batch).map_err(invalid_data)
        }
        _ => unreachable!("normalize_block_proposal_batch_kind only returns supported kinds"),
    }
}

pub(super) fn resolve_governance_replay_path(
    package_file: &Path,
    path: &str,
) -> io::Result<PathBuf> {
    if path.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance replay package contains an empty path",
        ));
    }
    let path = PathBuf::from(path);
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(package_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(path))
}

pub(super) fn governance_replay_path_reference(package_file: &Path, path: &Path) -> String {
    let package_dir = package_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    if let (Ok(package_dir), Ok(path)) = (
        std::fs::canonicalize(package_dir),
        std::fs::canonicalize(path),
    ) {
        if let Ok(relative) = path.strip_prefix(&package_dir) {
            return relative.to_string_lossy().into_owned();
        }
        return path.to_string_lossy().into_owned();
    }
    if let Ok(relative) = path.strip_prefix(package_dir) {
        return relative.to_string_lossy().into_owned();
    }
    if path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .is_some_and(|parent| parent == package_dir)
    {
        if let Some(file_name) = path.file_name() {
            return file_name.to_string_lossy().into_owned();
        }
    }
    path.to_string_lossy().into_owned()
}

#[derive(Serialize)]
pub(super) struct OperatorManifestSigningPayload<'a> {
    pub(super) schema: &'a str,
    pub(super) chain_id: &'a str,
    pub(super) network: &'a str,
    pub(super) validator_id: &'a str,
    pub(super) master_public_key_hex: &'a str,
    pub(super) hot_public_key_hex: &'a str,
    pub(super) algorithm_id: &'a str,
    pub(super) key_role: &'a str,
    pub(super) operator: &'a str,
    pub(super) contact: &'a str,
    pub(super) infrastructure: &'a OperatorInfrastructureLabels,
    pub(super) rotation_state: &'a str,
    pub(super) effective_height: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) cobalt_trust: Option<&'a OperatorCobaltTrustBinding>,
    pub(super) manifest_signing_key_hex: &'a str,
}

#[derive(Serialize)]
pub(super) struct OperatorManifestHashPayload<'a> {
    pub(super) signing_payload: OperatorManifestSigningPayload<'a>,
    pub(super) signature_hex: &'a str,
}

#[derive(Serialize)]
pub(super) struct GovernanceGenesisBundleHashPayload<'a> {
    pub(super) schema: &'a str,
    pub(super) chain_id: &'a str,
    pub(super) genesis_hash: &'a str,
    pub(super) protocol_version: u32,
    pub(super) network: &'a str,
    pub(super) validators: &'a [String],
    pub(super) validator_count: usize,
    pub(super) quorum: usize,
    pub(super) registry_root: &'a str,
    pub(super) registry_records: &'a [ValidatorRegistryRecord],
    pub(super) operator_manifests: &'a [GovernanceGenesisOperatorManifestRef],
}

pub(super) fn verify_operator_manifest_record(
    manifest: &OperatorManifest,
    manifest_file: &Path,
) -> io::Result<OperatorManifestVerifyReport> {
    validate_operator_manifest_fields(manifest)?;
    let signer_public_key = decode_ml_dsa_65_public_key_hex(
        "operator manifest signing key",
        &manifest.manifest_signing_key_hex,
    )?;
    let signature =
        decode_ml_dsa_65_signature_hex("operator manifest signature", &manifest.signature_hex)?;
    let signing_payload = operator_manifest_signing_payload_bytes(manifest)?;
    let signature_verified = ml_dsa_65_verify_with_context(
        &signer_public_key,
        &signing_payload,
        &signature,
        OPERATOR_MANIFEST_SIGNATURE_CONTEXT,
    );
    if !signature_verified {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest signature verification failed",
        ));
    }
    let expected_manifest_hash = operator_manifest_hash(manifest)?;
    if manifest.manifest_hash != expected_manifest_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest hash mismatch",
        ));
    }
    Ok(OperatorManifestVerifyReport {
        schema: OPERATOR_MANIFEST_VERIFY_REPORT_SCHEMA.to_string(),
        verified: true,
        manifest_file: manifest_file.display().to_string(),
        manifest_hash: manifest.manifest_hash.clone(),
        chain_id: manifest.chain_id.clone(),
        network: manifest.network.clone(),
        validator_id: manifest.validator_id.clone(),
        algorithm_id: manifest.algorithm_id.clone(),
        key_role: manifest.key_role.clone(),
        rotation_state: manifest.rotation_state.clone(),
        effective_height: manifest.effective_height,
        hot_public_key_hex: manifest.hot_public_key_hex.clone(),
        cobalt_trust: manifest.cobalt_trust.clone(),
        manifest_signer_matches_master: manifest.manifest_signing_key_hex
            == manifest.master_public_key_hex,
        signature_verified: true,
        redaction_checked: true,
    })
}

pub(super) fn validate_operator_manifest_fields(manifest: &OperatorManifest) -> io::Result<()> {
    validate_operator_manifest_fields_for_signing(manifest)?;
    decode_ml_dsa_65_signature_hex("operator manifest signature", &manifest.signature_hex)?;
    validate_hex_string("operator manifest hash", &manifest.manifest_hash, Some(96))?;
    Ok(())
}

pub(super) fn validate_operator_manifest_fields_for_signing(
    manifest: &OperatorManifest,
) -> io::Result<()> {
    if manifest.schema != OPERATOR_MANIFEST_FILE_SCHEMA {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported operator manifest schema `{}`", manifest.schema),
        ));
    }
    validate_manifest_text_field("operator manifest chain id", &manifest.chain_id)?;
    validate_manifest_text_field("operator manifest network", &manifest.network)?;
    validate_manifest_text_field("operator manifest validator id", &manifest.validator_id)?;
    validate_manifest_text_field("operator manifest key role", &manifest.key_role)?;
    validate_manifest_text_field("operator manifest operator", &manifest.operator)?;
    validate_manifest_text_field("operator manifest contact", &manifest.contact)?;
    validate_manifest_text_field("operator manifest rotation state", &manifest.rotation_state)?;
    if let Some(cobalt_trust) = &manifest.cobalt_trust {
        validate_operator_cobalt_trust_binding(cobalt_trust)?;
    }
    validate_manifest_text_field(
        "operator manifest provider group",
        &manifest.infrastructure.provider_group,
    )?;
    validate_manifest_text_field(
        "operator manifest region group",
        &manifest.infrastructure.region_group,
    )?;
    validate_manifest_text_field(
        "operator manifest jurisdiction group",
        &manifest.infrastructure.jurisdiction_group,
    )?;
    validate_manifest_text_field(
        "operator manifest legal domain group",
        &manifest.infrastructure.legal_domain_group,
    )?;
    validate_manifest_text_field(
        "operator manifest funding domain group",
        &manifest.infrastructure.funding_domain_group,
    )?;
    if manifest.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "operator manifest uses unsupported algorithm `{}`",
                manifest.algorithm_id
            ),
        ));
    }
    if manifest.key_role != "validator-hot" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest key role must be validator-hot",
        ));
    }
    if !matches!(
        manifest.rotation_state.as_str(),
        "active" | "staged" | "retiring" | "suspended" | "removed"
    ) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest rotation state is unsupported",
        ));
    }
    decode_ml_dsa_65_public_key_hex(
        "operator manifest master public key",
        &manifest.master_public_key_hex,
    )?;
    decode_ml_dsa_65_public_key_hex(
        "operator manifest hot public key",
        &manifest.hot_public_key_hex,
    )?;
    decode_ml_dsa_65_public_key_hex(
        "operator manifest signing key",
        &manifest.manifest_signing_key_hex,
    )?;
    if manifest.manifest_signing_key_hex != manifest.master_public_key_hex {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest signer must match master public key in v1",
        ));
    }
    Ok(())
}

pub(super) fn operator_cobalt_trust_binding_from_options(
    trust_graph_root: Option<String>,
    trust_graph_version: Option<u64>,
    trust_view_id: Option<String>,
    trust_view_version: Option<u64>,
) -> io::Result<Option<OperatorCobaltTrustBinding>> {
    let any_set = trust_graph_root.is_some()
        || trust_graph_version.is_some()
        || trust_view_id.is_some()
        || trust_view_version.is_some();
    if !any_set {
        return Ok(None);
    }
    let cobalt_trust = OperatorCobaltTrustBinding {
        trust_graph_root: trust_graph_root.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "operator manifest Cobalt metadata requires --trust-graph-root",
            )
        })?,
        trust_graph_version: trust_graph_version.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "operator manifest Cobalt metadata requires --trust-graph-version",
            )
        })?,
        trust_view_id: trust_view_id.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "operator manifest Cobalt metadata requires --trust-view-id",
            )
        })?,
        trust_view_version: trust_view_version.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "operator manifest Cobalt metadata requires --trust-view-version",
            )
        })?,
    };
    validate_operator_cobalt_trust_binding(&cobalt_trust)?;
    Ok(Some(cobalt_trust))
}

pub(super) fn validate_operator_cobalt_trust_binding(
    cobalt_trust: &OperatorCobaltTrustBinding,
) -> io::Result<()> {
    validate_hex_string(
        "operator manifest Cobalt trust graph root",
        &cobalt_trust.trust_graph_root,
        Some(96),
    )?;
    if cobalt_trust.trust_graph_version == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest Cobalt trust graph version must be nonzero",
        ));
    }
    validate_hex_string(
        "operator manifest Cobalt trust view id",
        &cobalt_trust.trust_view_id,
        Some(96),
    )?;
    if cobalt_trust.trust_view_version == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest Cobalt trust view version must be nonzero",
        ));
    }
    Ok(())
}

pub(super) fn validate_operator_manifest_for_genesis(
    manifest: &OperatorManifest,
    genesis: &Genesis,
    network: &str,
    validator_id: &str,
    registry_record: &ValidatorRegistryRecord,
) -> io::Result<()> {
    if manifest.chain_id != genesis.chain_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest chain id mismatch",
        ));
    }
    if manifest.network != network {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest network mismatch",
        ));
    }
    if manifest.validator_id != validator_id {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest validator id mismatch",
        ));
    }
    if manifest.rotation_state != "active" {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "genesis operator manifest rotation state must be active",
        ));
    }
    if manifest.effective_height != 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "genesis operator manifest effective height must be zero",
        ));
    }
    if manifest.hot_public_key_hex != registry_record.public_key_hex {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest hot public key does not match validator registry",
        ));
    }
    Ok(())
}

pub(super) fn validate_governance_genesis_manifest_ref(
    manifest: &OperatorManifest,
    manifest_ref: &GovernanceGenesisOperatorManifestRef,
) -> io::Result<()> {
    if manifest.manifest_hash != manifest_ref.manifest_hash {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis operator manifest hash mismatch",
        ));
    }
    if manifest.hot_public_key_hex != manifest_ref.hot_public_key_hex {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis operator manifest hot key mismatch",
        ));
    }
    if manifest.cobalt_trust != manifest_ref.cobalt_trust {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis operator manifest Cobalt trust metadata mismatch",
        ));
    }
    if manifest.infrastructure.provider_group != manifest_ref.provider_group
        || manifest.infrastructure.region_group != manifest_ref.region_group
        || manifest.infrastructure.jurisdiction_group != manifest_ref.jurisdiction_group
        || manifest.infrastructure.legal_domain_group != manifest_ref.legal_domain_group
        || manifest.infrastructure.funding_domain_group != manifest_ref.funding_domain_group
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis operator manifest infrastructure labels mismatch",
        ));
    }
    Ok(())
}

pub(super) fn validate_governance_genesis_cobalt_trust(
    operator_manifests: &[GovernanceGenesisOperatorManifestRef],
) -> io::Result<()> {
    let with_cobalt_trust = operator_manifests
        .iter()
        .filter(|manifest| manifest.cobalt_trust.is_some())
        .count();
    if with_cobalt_trust == 0 {
        return Ok(());
    }
    if with_cobalt_trust != operator_manifests.len() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis manifests must either all omit or all include Cobalt trust metadata",
        ));
    }

    let first = operator_manifests[0]
        .cobalt_trust
        .as_ref()
        .expect("count checked");
    let mut trust_view_ids = BTreeSet::new();
    for manifest in operator_manifests {
        let cobalt_trust = manifest.cobalt_trust.as_ref().expect("count checked");
        validate_operator_cobalt_trust_binding(cobalt_trust)?;
        if cobalt_trust.trust_graph_root != first.trust_graph_root
            || cobalt_trust.trust_graph_version != first.trust_graph_version
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance genesis manifests contain stale or mixed Cobalt trust graph metadata",
            ));
        }
        if !trust_view_ids.insert(cobalt_trust.trust_view_id.clone()) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "governance genesis manifests contain duplicate Cobalt trust view id",
            ));
        }
    }
    Ok(())
}

pub(super) fn validate_governance_genesis_quorum(
    quorum: usize,
    validator_count: usize,
) -> io::Result<()> {
    if quorum == 0 || quorum > validator_count {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance genesis quorum must be between 1 and validator count",
        ));
    }
    let minimum = bft_quorum_threshold(validator_count).map_err(invalid_data)?;
    if quorum < minimum {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("governance genesis quorum must be at least BFT threshold {minimum}"),
        ));
    }
    Ok(())
}

pub(super) fn validate_manifest_text_field(label: &str, value: &str) -> io::Result<()> {
    if value.is_empty() || value.trim() != value {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must be nonempty and trimmed"),
        ));
    }
    if value.len() > MAX_OPERATOR_MANIFEST_TEXT_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} exceeds {MAX_OPERATOR_MANIFEST_TEXT_BYTES} bytes"),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} contains a control character"),
        ));
    }
    Ok(())
}

pub(super) fn reject_operator_manifest_private_material(raw: &str) -> io::Result<()> {
    let lower = raw.to_ascii_lowercase();
    let sensitive_markers = [
        "private_key_hex",
        "seed_hex",
        "mnemonic",
        "password",
        "secret_access_key",
        "ssh_cred",
        "machine_1_private_key",
        "machine_2_private_key",
        "machine_3_private_key",
        "machine_4_private_key",
        "machine_5_private_key",
    ];
    if sensitive_markers
        .iter()
        .any(|marker| lower.contains(marker))
        || (raw.contains("BEGIN ") && raw.contains("PRIVATE KEY"))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "operator manifest contains private material marker",
        ));
    }
    Ok(())
}

pub(super) fn operator_manifest_signing_payload_bytes(
    manifest: &OperatorManifest,
) -> io::Result<Vec<u8>> {
    serde_json::to_vec(&operator_manifest_signing_payload(manifest)).map_err(invalid_data)
}

pub(super) fn operator_manifest_signing_payload(
    manifest: &OperatorManifest,
) -> OperatorManifestSigningPayload<'_> {
    OperatorManifestSigningPayload {
        schema: manifest.schema.as_str(),
        chain_id: manifest.chain_id.as_str(),
        network: manifest.network.as_str(),
        validator_id: manifest.validator_id.as_str(),
        master_public_key_hex: manifest.master_public_key_hex.as_str(),
        hot_public_key_hex: manifest.hot_public_key_hex.as_str(),
        algorithm_id: manifest.algorithm_id.as_str(),
        key_role: manifest.key_role.as_str(),
        operator: manifest.operator.as_str(),
        contact: manifest.contact.as_str(),
        infrastructure: &manifest.infrastructure,
        rotation_state: manifest.rotation_state.as_str(),
        effective_height: manifest.effective_height,
        cobalt_trust: manifest.cobalt_trust.as_ref(),
        manifest_signing_key_hex: manifest.manifest_signing_key_hex.as_str(),
    }
}

pub(super) fn operator_manifest_hash(manifest: &OperatorManifest) -> io::Result<String> {
    let payload = OperatorManifestHashPayload {
        signing_payload: operator_manifest_signing_payload(manifest),
        signature_hex: manifest.signature_hex.as_str(),
    };
    let encoded = serde_json::to_vec(&payload).map_err(invalid_data)?;
    Ok(hash_hex("postfiat.operator_manifest.v1", &encoded))
}

pub(super) fn governance_genesis_bundle_hash(
    bundle: &GovernanceGenesisBundle,
) -> io::Result<String> {
    let payload = GovernanceGenesisBundleHashPayload {
        schema: bundle.schema.as_str(),
        chain_id: bundle.chain_id.as_str(),
        genesis_hash: bundle.genesis_hash.as_str(),
        protocol_version: bundle.protocol_version,
        network: bundle.network.as_str(),
        validators: &bundle.validators,
        validator_count: bundle.validator_count,
        quorum: bundle.quorum,
        registry_root: bundle.registry_root.as_str(),
        registry_records: &bundle.registry_records,
        operator_manifests: &bundle.operator_manifests,
    };
    let encoded = serde_json::to_vec(&payload).map_err(invalid_data)?;
    Ok(hash_hex("postfiat.governance_genesis_bundle.v1", &encoded))
}

pub(super) fn decode_ml_dsa_65_public_key_hex(label: &str, value: &str) -> io::Result<Vec<u8>> {
    validate_hex_string(label, value, Some(ML_DSA_65_PUBLIC_KEY_BYTES * 2))?;
    let bytes = hex_to_bytes(value).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} has invalid hex: {error}"),
        )
    })?;
    ml_dsa_65_validate_public_key(&bytes).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} is not a valid ML-DSA-65 public key: {error}"),
        )
    })?;
    Ok(bytes)
}

pub(super) fn decode_ml_dsa_65_signature_hex(label: &str, value: &str) -> io::Result<Vec<u8>> {
    validate_hex_string(label, value, Some(ML_DSA_65_SIGNATURE_BYTES * 2))?;
    hex_to_bytes(value).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} has invalid hex: {error}"),
        )
    })
}

pub(super) fn validate_hex_string(
    label: &str,
    value: &str,
    expected_len: Option<usize>,
) -> io::Result<()> {
    if value.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must be nonempty"),
        ));
    }
    if expected_len.is_some_and(|len| value.len() != len) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} has length {}, expected {}",
                value.len(),
                expected_len.unwrap()
            ),
        ));
    }
    if !value
        .as_bytes()
        .iter()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("{label} must be lowercase hex"),
        ));
    }
    Ok(())
}

pub(super) fn display_relative_artifact_path(base_dir: &Path, artifact_file: &Path) -> String {
    artifact_file
        .strip_prefix(base_dir)
        .unwrap_or(artifact_file)
        .display()
        .to_string()
}

pub(super) fn resolve_governance_genesis_path(
    bundle_file: &Path,
    path: &str,
) -> io::Result<PathBuf> {
    if path.trim().is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "governance genesis bundle contains an empty path",
        ));
    }
    let path = PathBuf::from(path);
    if path
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "governance genesis bundle path must not contain parent components",
        ));
    }
    if path.is_absolute() {
        return Ok(path);
    }
    Ok(bundle_file
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(path))
}

pub(super) fn read_validator_registry_entry_file(
    path: &Path,
) -> io::Result<ValidatorRegistryEntry> {
    read_json_file(path, "validator registry entry")
}

pub(super) fn write_amendment_file(path: &Path, amendment: &GovernanceAmendment) -> io::Result<()> {
    let json = serde_json::to_string_pretty(amendment).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_amendment_file(path: &Path) -> io::Result<GovernanceAmendment> {
    read_json_file(path, "governance amendment")
}

pub(super) fn write_snapshot_manifest(path: &Path, manifest: &SnapshotManifest) -> io::Result<()> {
    let json = serde_json::to_string_pretty(manifest).map_err(invalid_data)?;
    atomic_write(path, format!("{json}\n"))
}

pub(super) fn read_snapshot_manifest(path: &Path) -> io::Result<SnapshotManifest> {
    read_json_file(path, "snapshot manifest")
}

pub(super) fn read_json_file<T: DeserializeOwned>(path: &Path, label: &str) -> io::Result<T> {
    let raw = read_bounded_json_text_file(path, label)?;
    serde_json::from_str(&raw).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("failed to parse {label} `{}`: {error}", path.display()),
        )
    })
}

pub(super) fn read_bounded_json_text_file(path: &Path, label: &str) -> io::Result<String> {
    let metadata = std::fs::metadata(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to stat {label} `{}`: {error}", path.display()),
        )
    })?;
    if metadata.len() > MAX_LOCAL_JSON_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} `{}` exceeds {MAX_LOCAL_JSON_FILE_BYTES} bytes",
                path.display()
            ),
        ));
    }
    let raw = std::fs::read_to_string(path).map_err(|error| {
        io::Error::new(
            error.kind(),
            format!("failed to read {label} `{}`: {error}", path.display()),
        )
    })?;
    if raw.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!(
                "{label} `{}` exceeds {MAX_LOCAL_JSON_FILE_BYTES} bytes",
                path.display()
            ),
        ));
    }
    Ok(raw)
}

pub(super) fn validate_snapshot_manifest_files(manifest: &SnapshotManifest) -> io::Result<()> {
    let expected_files = match manifest.snapshot_version {
        SNAPSHOT_VERSION => SNAPSHOT_FILES,
        LEGACY_SNAPSHOT_VERSION => LEGACY_SNAPSHOT_FILES,
        version => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unsupported snapshot version {version}"),
            ));
        }
    };
    let expected = expected_files.iter().copied().collect::<HashSet<_>>();
    let mut seen = HashSet::new();
    for file in &manifest.files {
        let file_name = file.name.as_str();
        if !expected.contains(file_name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("unexpected snapshot file `{file_name}` in manifest"),
            ));
        }
        if !seen.insert(file_name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("duplicate snapshot file `{file_name}` in manifest"),
            ));
        }
    }
    for &file_name in expected_files {
        if !seen.contains(file_name) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("missing snapshot file `{file_name}` in manifest"),
            ));
        }
    }
    Ok(())
}

#[cfg(unix)]
pub(super) fn set_private_file_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_mode(0o600);
    std::fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
pub(super) fn set_private_file_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(unix)]
pub(super) fn validate_private_file_permissions(path: &Path, label: &str) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mode = std::fs::metadata(path)?.permissions().mode() & 0o777;
    if mode != 0o600 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            format!(
                "{label} `{}` has mode {:o}, expected 600",
                path.display(),
                mode
            ),
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
pub(super) fn validate_private_file_permissions(_path: &Path, _label: &str) -> io::Result<()> {
    Ok(())
}

pub(super) fn invalid_data(error: impl std::error::Error + Send + Sync + 'static) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

pub(super) fn shielded_error(error: ShieldedError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}

pub(super) fn shielded_state_error(error: ShieldedError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error)
}

pub(super) fn bridge_error(error: BridgeError) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, error)
}

pub(super) fn shielded_action_rejection_id(batch_id: &str, index: usize, code: &str) -> String {
    hash_hex(
        "postfiat.shielded_action_rejection.v1",
        format!("{batch_id}:{index}:{code}").as_bytes(),
    )
}

pub(super) fn ordered_shielded_mint_creator(batch_id: &str, index: usize) -> String {
    format!("ordered-shielded:{batch_id}:{index}")
}

pub(super) fn bridge_action_rejection_id(batch_id: &str, index: usize, code: &str) -> String {
    hash_hex(
        "postfiat.bridge_action_rejection.v1",
        format!("{batch_id}:{index}:{code}").as_bytes(),
    )
}

pub(super) fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or_default()
}

/// Wrap: debit an account balance and mint an owned object of equal value
/// (account lane -> owned lane). This is the bridge from Cobalt account
/// balance to FastPay owned objects.
pub fn wrap_owned(
    _options: NodeOptions,
    _from_address: &str,
    _owner_pubkey_hex: &str,
    _amount: u64,
    _asset: &str,
    _object_id_override: Option<&str>,
) -> io::Result<String> {
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unsigned wrap_owned is disabled; use a signed FastLane primary deposit admitted through consensus",
    ))
}

/// Unwrap: retire an owned object and credit its value to an account balance
/// (owned lane -> account lane). Only the object's owner may unwrap it.
pub fn unwrap_owned(
    _options: NodeOptions,
    _object_id: &str,
    _owner_pubkey_hex: &str,
    _to_address: &str,
) -> io::Result<String> {
    Err(io::Error::new(
        io::ErrorKind::PermissionDenied,
        "unsigned unwrap_owned is disabled; use owned_unwrap_sign + owned_unwrap_apply",
    ))
}

/// Apply a FastPay CERTIFIED owned-transfer to this node's ledger. Verifies the
/// certificate (owner auth + >= quorum validator votes, via the registry
/// pubkeys) then applies `apply_owned_certificate` (single-consumption +
/// conservation), writes the ledger back. A bare order is never trusted.
pub fn owned_certificate_domain(
    data_dir: &std::path::Path,
) -> io::Result<postfiat_types::OwnedCertificateDomain> {
    let genesis = NodeStore::new(data_dir).read_genesis()?;
    Ok(postfiat_types::OwnedCertificateDomain {
        schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: postfiat_execution::genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        registry_id: current_registry_id(data_dir)?,
    })
}

pub fn owned_apply_report(options: NodeOptions, cert_json: &str) -> io::Result<OwnedApplyReport> {
    let cert: postfiat_types::OwnedTransferCertificate =
        serde_json::from_str(cert_json).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("owned-transfer cert parse failed: {error}"),
            )
        })?;
    let validator_pks = load_validator_pubkeys(&options.data_dir)?;
    let n = validator_pks.len();
    let quorum = bft_quorum_threshold(n).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("owned-transfer validator quorum failed: {error}"),
        )
    })?;
    let store = NodeStore::new(&options.data_dir);
    let mut ledger = store.read_ledger()?;
    let domain = owned_certificate_domain(&options.data_dir)?;
    let outcome = postfiat_execution::apply_owned_certificate(
        &mut ledger,
        &cert,
        &validator_pks,
        &domain,
        quorum,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("owned-transfer apply failed: {error:?}"),
        )
    })?;
    store.write_ledger(&ledger)?;
    let cert_hash = postfiat_crypto_provider::bytes_to_hex(&postfiat_crypto_provider::hash_bytes(
        "postfiat.owned-cert-hash.v2",
        cert_json.as_bytes(),
    ));
    let registry_id = current_registry_id(&options.data_dir)?;
    let _ = record_pending_checkpoint(&options.data_dir, &cert_hash, cert_json, &registry_id);
    let mut summary = format!(
        "certified owned-transfer applied (quorum {quorum} of {n}): consumed {} input(s), created {} output(s)",
        outcome.consumed,
        outcome.created.len()
    );
    for object in &outcome.created {
        summary.push_str(&format!(
            "\n  id={} value={} asset={}",
            object.id, object.value, object.asset
        ));
    }
    Ok(OwnedApplyReport {
        schema: "postfiat-owned-apply-report-v1".to_string(),
        quorum,
        validator_count: n,
        consumed_count: outcome.consumed,
        created_count: outcome.created.len(),
        created_objects: outcome.created,
        summary,
    })
}

pub fn owned_apply(options: NodeOptions, cert_json: &str) -> io::Result<String> {
    owned_apply_report(options, cert_json).map(|report| report.summary)
}

/// Apply a FastPay CERTIFIED owned-unwrap to this node's ledger. Verifies owner
/// auth + quorum before crediting an account balance.
pub fn owned_unwrap_apply_report(
    options: NodeOptions,
    cert_json: &str,
) -> io::Result<OwnedUnwrapApplyReport> {
    let cert: postfiat_types::OwnedUnwrapCertificate =
        serde_json::from_str(cert_json).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("owned-unwrap cert parse failed: {error}"),
            )
        })?;
    let validator_pks = load_validator_pubkeys(&options.data_dir)?;
    let n = validator_pks.len();
    let quorum = bft_quorum_threshold(n).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("owned-unwrap validator quorum failed: {error}"),
        )
    })?;
    let store = NodeStore::new(&options.data_dir);
    let mut ledger = store.read_ledger()?;
    let domain = owned_certificate_domain(&options.data_dir)?;
    let outcome = postfiat_execution::apply_owned_unwrap_certificate(
        &mut ledger,
        &cert,
        &validator_pks,
        &domain,
        quorum,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("owned-unwrap apply failed: {error:?}"),
        )
    })?;
    store.write_ledger(&ledger)?;
    let cert_hash = postfiat_crypto_provider::bytes_to_hex(&postfiat_crypto_provider::hash_bytes(
        "postfiat.owned-unwrap-cert-hash.v2",
        cert_json.as_bytes(),
    ));
    let registry_id = current_registry_id(&options.data_dir)?;
    let _ = record_pending_checkpoint(&options.data_dir, &cert_hash, cert_json, &registry_id);
    let mut summary = format!(
        "certified owned-unwrap applied (quorum {quorum} of {n}): consumed {} input(s), credited {} {} to account {}",
        outcome.consumed,
        outcome.credited,
        cert.order.asset,
        outcome.credited_to,
    );
    if let Some(change) = &outcome.change_object {
        summary.push_str(&format!(
            "\n  change id={} value={} asset={}",
            change.id, change.value, change.asset
        ));
    }
    Ok(OwnedUnwrapApplyReport {
        schema: "postfiat-owned-unwrap-apply-report-v1".to_string(),
        quorum,
        validator_count: n,
        consumed_count: outcome.consumed,
        credited: outcome.credited,
        credited_to: outcome.credited_to,
        change_object: outcome.change_object,
        summary,
    })
}

pub fn owned_unwrap_apply(options: NodeOptions, cert_json: &str) -> io::Result<String> {
    owned_unwrap_apply_report(options, cert_json).map(|report| report.summary)
}

/// Validator-side consensusless "validate + durable lock + sign". The request
/// must include the owner's authorization. Live ownership/version/conservation
/// checks run before a lock is acquired, and the lock is durably persisted
/// before this validator emits a signature.
pub fn owned_sign(
    options: NodeOptions,
    signed_order_json: &str,
    validator_id: &str,
) -> io::Result<String> {
    let signed: postfiat_types::SignedOwnedTransferOrder = serde_json::from_str(signed_order_json)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("signed owned-transfer order parse failed: {error}"),
            )
        })?;
    let ledger = NodeStore::new(&options.data_dir).read_ledger()?;
    let domain = owned_certificate_domain(&options.data_dir)?;
    postfiat_execution::validate_owned_transfer_admission(&ledger, &signed, &domain).map_err(
        |error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("owned-transfer admission failed: {error:?}"),
            )
        },
    )?;

    let signing_bytes = postfiat_execution::owned_transfer_signing_bytes(&signed.order);
    let order_hash = postfiat_crypto_provider::bytes_to_hex(&postfiat_crypto_provider::hash_bytes(
        "postfiat.owned-order-hash.v2",
        &signing_bytes,
    ));
    let registry_id = current_registry_id(&options.data_dir)?;
    let secret_key = load_owned_validator_secret_key(&options.data_dir, validator_id)?;

    // This compare-and-set is serialized across RPC threads/processes by an OS
    // file lock and uses a synced atomic rename. It must complete before sign.
    reserve_owned_input_locks(
        &options.data_dir,
        &signed.order.inputs,
        &registry_id,
        &order_hash,
        "owned-sign",
    )?;
    let signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
        &secret_key,
        &signing_bytes,
        postfiat_execution::OWNED_TRANSFER_CONTEXT,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("owned-transfer sign failed: {error}"),
        )
    })?;

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "validator_id": validator_id,
        "signature_hex": postfiat_crypto_provider::bytes_to_hex(&signature),
    }))
    .unwrap_or_else(|_| "{}".to_string()))
}

/// Validator-side lock + sign for certified owned unwrap. Shares the same
/// object-version lock table as `owned_sign`, so a transfer and unwrap cannot
/// both receive votes for the same owned object version.
pub fn owned_unwrap_sign(
    options: NodeOptions,
    signed_order_json: &str,
    validator_id: &str,
) -> io::Result<String> {
    let signed: postfiat_types::SignedOwnedUnwrapOrder = serde_json::from_str(signed_order_json)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("signed owned-unwrap order parse failed: {error}"),
            )
        })?;
    let ledger = NodeStore::new(&options.data_dir).read_ledger()?;
    let domain = owned_certificate_domain(&options.data_dir)?;
    postfiat_execution::validate_owned_unwrap_admission(&ledger, &signed, &domain).map_err(
        |error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("owned-unwrap admission failed: {error:?}"),
            )
        },
    )?;

    let signing_bytes = postfiat_execution::owned_unwrap_signing_bytes(&signed.order);
    let order_hash = postfiat_crypto_provider::bytes_to_hex(&postfiat_crypto_provider::hash_bytes(
        "postfiat.owned-unwrap-order-hash.v2",
        &signing_bytes,
    ));
    let registry_id = current_registry_id(&options.data_dir)?;
    let secret_key = load_owned_validator_secret_key(&options.data_dir, validator_id)?;
    reserve_owned_input_locks(
        &options.data_dir,
        &signed.order.inputs,
        &registry_id,
        &order_hash,
        "owned-unwrap-sign",
    )?;
    let signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
        &secret_key,
        &signing_bytes,
        postfiat_execution::OWNED_UNWRAP_CONTEXT,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::Other,
            format!("owned-unwrap sign failed: {error}"),
        )
    })?;

    Ok(serde_json::to_string_pretty(&serde_json::json!({
        "validator_id": validator_id,
        "signature_hex": postfiat_crypto_provider::bytes_to_hex(&signature),
    }))
    .unwrap_or_else(|_| "{}".to_string()))
}

pub(super) fn load_owned_validator_secret_key(
    data_dir: &std::path::Path,
    validator_id: &str,
) -> io::Result<Vec<u8>> {
    let key_path = data_dir.join("validator_keys.json");
    let key_text = std::fs::read_to_string(&key_path).map_err(|error| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("validator_keys.json: {error}"),
        )
    })?;
    let key_file: serde_json::Value = serde_json::from_str(&key_text).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("validator_keys parse: {error}"),
        )
    })?;
    let private_key_hex = key_file
        .get("validators")
        .and_then(serde_json::Value::as_array)
        .and_then(|validators| {
            validators.iter().find_map(|entry| {
                let id = entry.get("node_id")?.as_str()?;
                let private_key = entry.get("private_key_hex")?.as_str()?;
                (id == validator_id).then(|| private_key.to_string())
            })
        })
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("validator {validator_id} not in key file"),
            )
        })?;
    postfiat_crypto_provider::hex_to_bytes(&private_key_hex).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("validator private key hex: {error}"),
        )
    })
}

pub(super) fn reserve_owned_input_locks(
    data_dir: &std::path::Path,
    inputs: &[postfiat_types::OwnedObjectRef],
    registry_id: &str,
    order_hash: &str,
    operation: &str,
) -> io::Result<()> {
    std::fs::create_dir_all(data_dir)?;
    let guard_path = data_dir.join("owned_locks.guard");
    let guard = std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&guard_path)?;
    guard.lock()?;

    let locks_map = load_owned_input_locks(data_dir)?;

    for input in inputs {
        let key = format!("{}:{}:{}", input.id, input.version, registry_id);
        if let Some(existing) = locks_map.get(&key) {
            let existing_hash = existing.as_str().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("owned lock `{key}` is not a string"),
                )
            })?;
            if existing_hash != order_hash {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    format!(
                        "{operation} refused: input {} v{} is locked by a different order (equivocation refused)",
                        input.id, input.version
                    ),
                ));
            }
        }
    }
    let mut entries = serde_json::Map::new();
    for input in inputs {
        let key = format!("{}:{}:{}", input.id, input.version, registry_id);
        if !locks_map.contains_key(&key) {
            entries.insert(key, serde_json::Value::String(order_hash.to_string()));
        }
    }
    if entries.is_empty() {
        return Ok(());
    }

    let record = serde_json::json!({
        "schema": "postfiat-owned-lock-wal-v1",
        "operation": operation,
        "registry_id": registry_id,
        "order_hash": order_hash,
        "entries": entries,
    });
    let mut encoded = serde_json::to_vec(&record).map_err(invalid_data)?;
    encoded.push(b'\n');
    append_owned_lock_wal(data_dir, &encoded)?;
    Ok(())
}

/// Load the legacy snapshot plus the append-only lock WAL. The WAL is the
/// persist-before-sign path; the snapshot remains readable for rolling upgrade
/// compatibility. Any conflicting or corrupt durable record fails closed.
fn load_owned_input_locks(
    data_dir: &std::path::Path,
) -> io::Result<serde_json::Map<String, serde_json::Value>> {
    let lock_path = data_dir.join("owned_locks.json");
    let mut locks = match std::fs::read_to_string(&lock_path) {
        Ok(text) => serde_json::from_str::<serde_json::Value>(&text)
            .map_err(|error| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("owned_locks.json parse failed: {error}"),
                )
            })?
            .as_object()
            .cloned()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "owned_locks.json is not an object",
                )
            })?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => serde_json::Map::new(),
        Err(error) => return Err(error),
    };

    let wal_path = data_dir.join("owned_locks.wal");
    let wal = match std::fs::read(&wal_path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(locks),
        Err(error) => return Err(error),
    };
    let ends_with_newline = wal.last().copied() == Some(b'\n');
    let lines: Vec<&[u8]> = wal.split(|byte| *byte == b'\n').collect();
    for (index, line) in lines.iter().enumerate() {
        if line.is_empty() {
            continue;
        }
        if index + 1 == lines.len() && !ends_with_newline {
            // A torn final append cannot have preceded an emitted signature:
            // signing occurs only after sync_data succeeds. Ignore that tail.
            break;
        }
        let record: serde_json::Value = serde_json::from_slice(line).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("owned_locks.wal line {} parse failed: {error}", index + 1),
            )
        })?;
        if record.get("schema").and_then(serde_json::Value::as_str)
            != Some("postfiat-owned-lock-wal-v1")
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("owned_locks.wal line {} has unsupported schema", index + 1),
            ));
        }
        let entries = record
            .get("entries")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!(
                        "owned_locks.wal line {} entries is not an object",
                        index + 1
                    ),
                )
            })?;
        for (key, value) in entries {
            let order_hash = value.as_str().ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("owned_locks.wal lock `{key}` is not a string"),
                )
            })?;
            if let Some(existing) = locks.get(key).and_then(serde_json::Value::as_str) {
                if existing != order_hash {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("owned_locks.wal contains conflicting durable lock `{key}`"),
                    ));
                }
            } else {
                locks.insert(
                    key.clone(),
                    serde_json::Value::String(order_hash.to_string()),
                );
            }
        }
    }
    Ok(locks)
}

fn append_owned_lock_wal(data_dir: &std::path::Path, record: &[u8]) -> io::Result<()> {
    let wal_path = data_dir.join("owned_locks.wal");
    let existed = wal_path.exists();
    let mut wal = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&wal_path)?;
    wal.write_all(record)?;
    wal.sync_data()?;
    if !existed {
        // The first append must also make the directory entry durable. Every
        // later reservation needs only one fdatasync, avoiding the old temp
        // file fsync + rename + parent-directory fsync sequence.
        std::fs::File::open(data_dir)?.sync_all()?;
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn reserve_owned_transfer_lock_for_test(
    options: NodeOptions,
    order: &postfiat_types::OwnedTransferOrder,
) -> io::Result<()> {
    let signing_bytes = postfiat_execution::owned_transfer_signing_bytes(order);
    let order_hash = postfiat_crypto_provider::bytes_to_hex(&postfiat_crypto_provider::hash_bytes(
        "postfiat.owned-order-hash.v2",
        &signing_bytes,
    ));
    let registry_id = current_registry_id(&options.data_dir)?;
    reserve_owned_input_locks(
        &options.data_dir,
        &order.inputs,
        &registry_id,
        &order_hash,
        "owned-sign-test",
    )
}

#[cfg(test)]
pub(super) fn load_owned_input_locks_for_test(
    data_dir: &std::path::Path,
) -> io::Result<serde_json::Map<String, serde_json::Value>> {
    load_owned_input_locks(data_dir)
}

pub(super) fn load_owned_input_locks_for_snapshot(
    data_dir: &std::path::Path,
) -> io::Result<serde_json::Map<String, serde_json::Value>> {
    load_owned_input_locks(data_dir)
}

pub(super) fn load_validator_pubkeys(
    data_dir: &std::path::Path,
) -> io::Result<Vec<(String, String)>> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let expected_validators = local_validator_ids(genesis.validator_count)?;
    let registry = read_validator_registry_file(&data_dir.join("validator_registry.json"))?;
    validate_validator_registry_for_count(&registry, genesis.validator_count)?;
    let active_registry =
        validator_registry_subset_for_validators(&registry, &expected_validators)?;
    Ok(active_registry
        .validators
        .into_iter()
        .map(|record| (record.node_id, record.public_key_hex))
        .collect())
}

/// Content-based fingerprint of the active validator registry. Changes across a
/// governed registry transition — which is what scopes owned-transfer locks and
/// enables safe-unlock at the boundary (#4).
pub(super) fn current_registry_id(data_dir: &std::path::Path) -> io::Result<String> {
    let registry_path = data_dir.join("validator_registry.json");
    let bytes = std::fs::read(&registry_path).map_err(|error| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("validator_registry.json: {error}"),
        )
    })?;
    Ok(postfiat_crypto_provider::bytes_to_hex(
        &postfiat_crypto_provider::hash_bytes("postfiat.registry-id.v1", &bytes),
    ))
}

/// #3 checkpoint lane — record a finalized cert as pending checkpoint (called
/// after a successful owned-apply). `checkpoint_pending` later moves pending
/// certs into the durable checkpoint log (committed state that survives registry
/// transitions + is part of history).
pub(super) fn record_pending_checkpoint(
    data_dir: &std::path::Path,
    cert_hash: &str,
    cert_json: &str,
    registry_id: &str,
) -> io::Result<()> {
    let pending_path = data_dir.join("pending_checkpoint_certs.json");
    let mut pending: serde_json::Value = std::fs::read_to_string(&pending_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| serde_json::json!([]));
    let arr = pending.as_array_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "pending_checkpoint_certs.json is not an array",
        )
    })?;
    arr.push(serde_json::json!({
        "cert_hash": cert_hash,
        "cert": serde_json::from_str::<serde_json::Value>(cert_json).unwrap_or(serde_json::Value::Null),
        "registry_id": registry_id,
    }));
    std::fs::write(
        &pending_path,
        serde_json::to_string_pretty(&pending).unwrap_or_default(),
    )?;
    Ok(())
}

/// #3 checkpoint lane — move all pending finalized certs into the durable
/// checkpoint log (committed state — survives registry transitions + is part of
/// history), then clear the pending queue. This is the FastPay "after-the-fact
/// checkpointing" landing: finalized certs become durable committed state.
pub fn checkpoint_pending(options: NodeOptions) -> io::Result<String> {
    let pending_path = options.data_dir.join("pending_checkpoint_certs.json");
    let log_path = options.data_dir.join("checkpoint_log.json");
    let pending: serde_json::Value = std::fs::read_to_string(&pending_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| serde_json::json!([]));
    let arr = pending.as_array().cloned().unwrap_or_default();
    if arr.is_empty() {
        return Ok("no pending certs to checkpoint".into());
    }
    let mut log: serde_json::Value = std::fs::read_to_string(&log_path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| serde_json::json!([]));
    let log_arr = log.as_array_mut().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "checkpoint_log.json is not an array",
        )
    })?;
    for entry in &arr {
        log_arr.push(entry.clone());
    }
    std::fs::write(
        &log_path,
        serde_json::to_string_pretty(&log).unwrap_or_default(),
    )?;
    std::fs::write(&pending_path, "[]")?;
    Ok(format!(
        "checkpointed {} finalized cert(s) into the durable checkpoint log",
        arr.len()
    ))
}

/// Fail-closed placeholder for owned-lock reconfiguration recovery.
///
/// Deleting an old-registry lock is unsafe while a quorum certificate produced
/// under that registry can still arrive and execute. Safe release requires a
/// canonical epoch drain/checkpoint (or a quorum-certified cancel/tombstone),
/// neither of which exists in the payment lane today. Preserve every lock until
/// that structural protocol is implemented.
pub fn owned_safe_unlock(options: NodeOptions) -> io::Result<String> {
    let retained = load_owned_input_locks(&options.data_dir)?.len();
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        format!(
            "owned-safe-unlock disabled: retained {retained} lock(s); safe release requires canonical registry drain/checkpoint proof or quorum-certified cancellation"
        ),
    ))
}
