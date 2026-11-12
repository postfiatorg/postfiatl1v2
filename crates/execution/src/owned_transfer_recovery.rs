pub const OWNED_TRANSFER_CONTEXT_V3: &[u8] = b"postfiat-l1-v2/owned-transfer/v3";
pub const OWNED_UNWRAP_CONTEXT_V3: &[u8] = b"postfiat-l1-v2/owned-unwrap/v3";
pub const FASTPAY_APPLY_ACK_CONTEXT_V1: &[u8] = b"postfiat-l1-v2/fastpay-apply-ack/v1";

#[derive(Debug, Clone, Copy)]
pub struct FastPayRecoveryVerificationContext<'a> {
    pub validator_public_keys: &'a [(String, String)],
    pub expected_domain: &'a postfiat_types::OwnedCertificateDomain,
    pub committee_epoch: u64,
    pub policy: &'a postfiat_types::FastPayRecoveryPolicyV1,
    pub quorum: usize,
}

fn append_fastpay_v3_text(bytes: &mut Vec<u8>, value: &str) {
    bytes.extend_from_slice(&(value.len() as u64).to_le_bytes());
    bytes.extend_from_slice(value.as_bytes());
}

pub fn owned_transfer_v3_signing_bytes(order: &postfiat_types::OwnedTransferOrderV3) -> Vec<u8> {
    let mut bytes = b"postfiat.owned-transfer.v3\0".to_vec();
    let preimage = postfiat_types::fastpay_transfer_lock_preimage_v1(order);
    bytes.extend_from_slice(&(preimage.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&preimage);
    append_fastpay_v3_text(&mut bytes, &order.recovery.lock_id);
    bytes
}

pub fn owned_unwrap_v3_signing_bytes(order: &postfiat_types::OwnedUnwrapOrderV3) -> Vec<u8> {
    let mut bytes = b"postfiat.owned-unwrap.v3\0".to_vec();
    let preimage = postfiat_types::fastpay_unwrap_lock_preimage_v1(order);
    bytes.extend_from_slice(&(preimage.len() as u64).to_le_bytes());
    bytes.extend_from_slice(&preimage);
    append_fastpay_v3_text(&mut bytes, &order.recovery.lock_id);
    bytes
}

pub fn fastpay_transfer_order_digest_v3(order: &postfiat_types::OwnedTransferOrderV3) -> String {
    postfiat_crypto_provider::bytes_to_hex(&postfiat_crypto_provider::hash_bytes(
        "postfiat.fastpay.transfer-order.v3",
        &owned_transfer_v3_signing_bytes(order),
    ))
}

pub fn fastpay_unwrap_order_digest_v3(order: &postfiat_types::OwnedUnwrapOrderV3) -> String {
    postfiat_crypto_provider::bytes_to_hex(&postfiat_crypto_provider::hash_bytes(
        "postfiat.fastpay.unwrap-order.v3",
        &owned_unwrap_v3_signing_bytes(order),
    ))
}

pub fn fastpay_apply_ack_signing_bytes_v1(
    acknowledgement: &postfiat_types::FastPayApplyAckV1,
) -> Result<Vec<u8>, OwnedTransferError> {
    acknowledgement
        .validate_signing_shape()
        .map_err(|_| OwnedTransferError::InvalidRecovery)?;
    let mut bytes = b"postfiat.fastpay.apply-ack.v1\0".to_vec();
    for value in [
        acknowledgement.schema.as_str(),
        acknowledgement.domain.schema.as_str(),
        acknowledgement.domain.chain_id.as_str(),
        acknowledgement.domain.genesis_hash.as_str(),
        acknowledgement.domain.registry_id.as_str(),
        acknowledgement.lock_id.as_str(),
        acknowledgement.order_digest.as_str(),
        acknowledgement.certificate_digest.as_str(),
        acknowledgement.terminal_state_digest.as_str(),
        acknowledgement.validator_id.as_str(),
    ] {
        append_fastpay_v3_text(&mut bytes, value);
    }
    bytes.extend_from_slice(&acknowledgement.domain.protocol_version.to_le_bytes());
    bytes.extend_from_slice(&acknowledgement.committee_epoch.to_le_bytes());
    Ok(bytes)
}

pub fn verify_fastpay_apply_ack_v1(
    acknowledgement: &postfiat_types::FastPayApplyAckV1,
    validator_public_key_hex: &str,
) -> bool {
    let Ok(bytes) = fastpay_apply_ack_signing_bytes_v1(acknowledgement) else {
        return false;
    };
    let (Ok(public_key), Ok(signature)) = (
        postfiat_crypto_provider::hex_to_bytes(validator_public_key_hex),
        postfiat_crypto_provider::hex_to_bytes(&acknowledgement.signature_hex),
    ) else {
        return false;
    };
    postfiat_crypto_provider::ml_dsa_65_verify_with_context(
        &public_key,
        &bytes,
        &signature,
        FASTPAY_APPLY_ACK_CONTEXT_V1,
    )
}

/// One signed fence status against one validator public key.
pub fn verify_fastpay_fence_status_v2(
    status: &postfiat_types::FastPayFenceStatusV2,
    validator_public_key_hex: &str,
) -> bool {
    let Ok(bytes) = status.signing_bytes() else {
        return false;
    };
    verify_fastpay_fence_status_signature_v2(validator_public_key_hex, &bytes, &status.signature_hex)
}

fn verify_fastpay_fence_status_signature_v2(
    validator_public_key_hex: &str,
    bytes: &[u8],
    signature_hex: &str,
) -> bool {
    let (Ok(public_key), Ok(signature)) = (
        postfiat_crypto_provider::hex_to_bytes(validator_public_key_hex),
        postfiat_crypto_provider::hex_to_bytes(signature_hex),
    ) else {
        return false;
    };
    postfiat_crypto_provider::ml_dsa_65_verify_with_context(
        &public_key,
        bytes,
        &signature,
        postfiat_types::FASTPAY_FENCE_STATUS_CONTEXT_V2,
    )
}

/// The client check: a quorum of distinct committee members signed the same
/// answer for `lock_id` (`postfiat_types::verify_fastpay_fence_status_quorum_v2`
/// with ML-DSA-65 under the fence status context).
pub fn verify_fastpay_fence_status_quorum_ml_dsa_v2(
    statuses: &[postfiat_types::FastPayFenceStatusV2],
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
    pinned_registry_root: &str,
    lock_id: &str,
) -> Result<postfiat_types::FastPayFenceStatusQuorumV2, postfiat_types::FastPayFenceStatusRefusalV2>
{
    postfiat_types::verify_fastpay_fence_status_quorum_v2(
        statuses,
        committee,
        pinned_registry_root,
        lock_id,
        verify_fastpay_fence_status_signature_v2,
    )
}

fn validate_fastpay_v3_recovery(
    domain: &postfiat_types::OwnedCertificateDomain,
    recovery: &postfiat_types::FastPayOrderRecoveryV1,
    expected_domain: &postfiat_types::OwnedCertificateDomain,
    expected_committee_epoch: u64,
    policy: &postfiat_types::FastPayRecoveryPolicyV1,
    current_height: u64,
    expected_lock_id: &str,
) -> Result<(), OwnedTransferError> {
    if domain != expected_domain
        || domain.schema != postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3
        || expected_domain.schema != postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V3
        || current_height < policy.activation_height
        || recovery.committee_epoch != expected_committee_epoch
        || recovery.lock_id != expected_lock_id
        || recovery.validate(policy).is_err()
    {
        return Err(OwnedTransferError::InvalidRecovery);
    }
    if current_height < recovery.valid_from_height {
        return Err(OwnedTransferError::NotYetValid);
    }
    if current_height > recovery.expires_at_height {
        return Err(OwnedTransferError::Expired);
    }
    Ok(())
}

fn transfer_v3_semantic_order(
    order: &postfiat_types::OwnedTransferOrderV3,
) -> postfiat_types::OwnedTransferOrder {
    postfiat_types::OwnedTransferOrder {
        domain: order.domain.clone(),
        inputs: order.inputs.clone(),
        outputs: order.outputs.clone(),
        fee: order.fee,
        nonce: order.nonce,
        memos: order.memos.clone(),
    }
}

fn unwrap_v3_semantic_order(
    order: &postfiat_types::OwnedUnwrapOrderV3,
) -> postfiat_types::OwnedUnwrapOrder {
    postfiat_types::OwnedUnwrapOrder {
        domain: order.domain.clone(),
        inputs: order.inputs.clone(),
        to_address: order.to_address.clone(),
        amount: order.amount,
        asset: order.asset.clone(),
        fee: order.fee,
        nonce: order.nonce,
        memos: order.memos.clone(),
    }
}

fn verify_fastpay_owner_v3(
    owner_pubkey_hex: &str,
    owner_signature_hex: &str,
    signing_bytes: &[u8],
    context: &[u8],
) -> bool {
    let (Ok(public_key), Ok(signature)) = (
        postfiat_crypto_provider::hex_to_bytes(owner_pubkey_hex),
        postfiat_crypto_provider::hex_to_bytes(owner_signature_hex),
    ) else {
        return false;
    };
    postfiat_crypto_provider::ml_dsa_65_verify_with_context(
        &public_key,
        signing_bytes,
        &signature,
        context,
    )
}

fn verify_fastpay_votes_v3<VoteId, VoteSignature>(
    signing_bytes: &[u8],
    context: &[u8],
    validator_pks: &[(String, String)],
    votes: &[(VoteId, VoteSignature)],
) -> Result<usize, OwnedTransferError>
where
    VoteId: AsRef<str>,
    VoteSignature: AsRef<str>,
{
    let mut seen = std::collections::BTreeSet::new();
    let mut valid = 0usize;
    for (validator_id, signature_hex) in votes {
        let validator_id = validator_id.as_ref();
        if !seen.insert(validator_id) {
            return Err(OwnedTransferError::InvalidRecovery);
        }
        let Some((_, public_key_hex)) = validator_pks
            .iter()
            .find(|(candidate, _)| candidate == validator_id)
        else {
            return Err(OwnedTransferError::InvalidRecovery);
        };
        let (Ok(public_key), Ok(signature)) = (
            postfiat_crypto_provider::hex_to_bytes(public_key_hex),
            postfiat_crypto_provider::hex_to_bytes(signature_hex.as_ref()),
        ) else {
            continue;
        };
        if postfiat_crypto_provider::ml_dsa_65_verify_with_context(
            &public_key,
            signing_bytes,
            &signature,
            context,
        ) {
            valid += 1;
        }
    }
    Ok(valid)
}

pub fn validate_owned_transfer_v3_admission(
    ledger: &LedgerState,
    signed: &postfiat_types::SignedOwnedTransferOrderV3,
    expected_domain: &postfiat_types::OwnedCertificateDomain,
    expected_committee_epoch: u64,
    policy: &postfiat_types::FastPayRecoveryPolicyV1,
    current_height: u64,
) -> Result<(), OwnedTransferError> {
    let expected_lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&signed.order);
    validate_fastpay_v3_recovery(
        &signed.order.domain,
        &signed.order.recovery,
        expected_domain,
        expected_committee_epoch,
        policy,
        current_height,
        &expected_lock_id,
    )?;
    let signing_bytes = owned_transfer_v3_signing_bytes(&signed.order);
    if !verify_fastpay_owner_v3(
        &signed.owner_pubkey_hex,
        &signed.owner_signature_hex,
        &signing_bytes,
        OWNED_TRANSFER_CONTEXT_V3,
    ) {
        return Err(OwnedTransferError::OwnerAuthFailed);
    }
    prepare_owned_transfer(
        ledger,
        &transfer_v3_semantic_order(&signed.order),
        &signed.owner_pubkey_hex,
    )
    .map(|_| ())
}

pub fn validate_owned_unwrap_v3_admission(
    ledger: &LedgerState,
    signed: &postfiat_types::SignedOwnedUnwrapOrderV3,
    expected_domain: &postfiat_types::OwnedCertificateDomain,
    expected_committee_epoch: u64,
    policy: &postfiat_types::FastPayRecoveryPolicyV1,
    current_height: u64,
) -> Result<(), OwnedTransferError> {
    let expected_lock_id = postfiat_types::fastpay_unwrap_lock_id_v1(&signed.order);
    validate_fastpay_v3_recovery(
        &signed.order.domain,
        &signed.order.recovery,
        expected_domain,
        expected_committee_epoch,
        policy,
        current_height,
        &expected_lock_id,
    )?;
    let signing_bytes = owned_unwrap_v3_signing_bytes(&signed.order);
    if !verify_fastpay_owner_v3(
        &signed.owner_pubkey_hex,
        &signed.owner_signature_hex,
        &signing_bytes,
        OWNED_UNWRAP_CONTEXT_V3,
    ) {
        return Err(OwnedTransferError::OwnerAuthFailed);
    }
    prepare_owned_unwrap(
        ledger,
        &unwrap_v3_semantic_order(&signed.order),
        &signed.owner_pubkey_hex,
    )
    .map(|_| ())
}

pub fn verify_owned_transfer_certificate_v3(
    certificate: &postfiat_types::OwnedTransferCertificateV3,
    validator_pks: &[(String, String)],
    expected_domain: &postfiat_types::OwnedCertificateDomain,
    expected_committee_epoch: u64,
    policy: &postfiat_types::FastPayRecoveryPolicyV1,
    current_height: u64,
    quorum: usize,
) -> Result<usize, OwnedTransferError> {
    certificate
        .validate_commitment_shape()
        .map_err(|_| OwnedTransferError::InvalidRecovery)?;
    let expected_lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&certificate.order);
    validate_fastpay_v3_recovery(
        &certificate.order.domain,
        &certificate.order.recovery,
        expected_domain,
        expected_committee_epoch,
        policy,
        current_height,
        &expected_lock_id,
    )?;
    let signing_bytes = owned_transfer_v3_signing_bytes(&certificate.order);
    if !verify_fastpay_owner_v3(
        &certificate.owner_pubkey_hex,
        &certificate.owner_signature_hex,
        &signing_bytes,
        OWNED_TRANSFER_CONTEXT_V3,
    ) {
        return Err(OwnedTransferError::OwnerAuthFailed);
    }
    let votes = certificate
        .votes
        .iter()
        .map(|vote| (vote.validator_id.as_str(), vote.signature_hex.as_str()))
        .collect::<Vec<_>>();
    let valid = verify_fastpay_votes_v3(
        &signing_bytes,
        OWNED_TRANSFER_CONTEXT_V3,
        validator_pks,
        &votes,
    )?;
    if valid < quorum {
        return Err(OwnedTransferError::InsufficientQuorum {
            have: valid,
            need: quorum,
        });
    }
    Ok(valid)
}

pub fn verify_owned_unwrap_certificate_v3(
    certificate: &postfiat_types::OwnedUnwrapCertificateV3,
    validator_pks: &[(String, String)],
    expected_domain: &postfiat_types::OwnedCertificateDomain,
    expected_committee_epoch: u64,
    policy: &postfiat_types::FastPayRecoveryPolicyV1,
    current_height: u64,
    quorum: usize,
) -> Result<usize, OwnedTransferError> {
    certificate
        .validate_commitment_shape()
        .map_err(|_| OwnedTransferError::InvalidRecovery)?;
    let expected_lock_id = postfiat_types::fastpay_unwrap_lock_id_v1(&certificate.order);
    validate_fastpay_v3_recovery(
        &certificate.order.domain,
        &certificate.order.recovery,
        expected_domain,
        expected_committee_epoch,
        policy,
        current_height,
        &expected_lock_id,
    )?;
    let signing_bytes = owned_unwrap_v3_signing_bytes(&certificate.order);
    if !verify_fastpay_owner_v3(
        &certificate.owner_pubkey_hex,
        &certificate.owner_signature_hex,
        &signing_bytes,
        OWNED_UNWRAP_CONTEXT_V3,
    ) {
        return Err(OwnedTransferError::OwnerAuthFailed);
    }
    let votes = certificate
        .votes
        .iter()
        .map(|vote| (vote.validator_id.as_str(), vote.signature_hex.as_str()))
        .collect::<Vec<_>>();
    let valid = verify_fastpay_votes_v3(
        &signing_bytes,
        OWNED_UNWRAP_CONTEXT_V3,
        validator_pks,
        &votes,
    )?;
    if valid < quorum {
        return Err(OwnedTransferError::InsufficientQuorum {
            have: valid,
            need: quorum,
        });
    }
    Ok(valid)
}

pub fn apply_owned_transfer_certificate_v3(
    ledger: &mut LedgerState,
    certificate: &postfiat_types::OwnedTransferCertificateV3,
    context: FastPayRecoveryVerificationContext<'_>,
    verification_height: u64,
) -> Result<OwnedTransferOutcome, OwnedTransferError> {
    apply_owned_transfer_certificate_v3_at_decision(
        ledger,
        certificate,
        context,
        verification_height,
        certificate.order.recovery.valid_from_height,
        postfiat_types::FastPayFenceOriginV1::Consensusless,
    )
}

fn apply_owned_transfer_certificate_v3_at_decision(
    ledger: &mut LedgerState,
    certificate: &postfiat_types::OwnedTransferCertificateV3,
    context: FastPayRecoveryVerificationContext<'_>,
    verification_height: u64,
    decision_height: u64,
    origin: postfiat_types::FastPayFenceOriginV1,
) -> Result<OwnedTransferOutcome, OwnedTransferError> {
    verify_owned_transfer_certificate_v3(
        certificate,
        context.validator_public_keys,
        context.expected_domain,
        context.committee_epoch,
        context.policy,
        verification_height,
        context.quorum,
    )?;
    ensure_fastpay_inputs_unfenced(ledger, &certificate.order.inputs)?;
    let fence = transfer_confirmed_fence(certificate, decision_height, origin)?;
    ensure_fastpay_fence_capacity(ledger)?;
    let outcome = apply_owned_transfer(
        ledger,
        &transfer_v3_semantic_order(&certificate.order),
        &certificate.owner_pubkey_hex,
    )?;
    ledger.fastpay_version_fences.push(fence);
    Ok(outcome)
}

pub fn apply_owned_unwrap_certificate_v3(
    ledger: &mut LedgerState,
    certificate: &postfiat_types::OwnedUnwrapCertificateV3,
    context: FastPayRecoveryVerificationContext<'_>,
    verification_height: u64,
) -> Result<OwnedUnwrapOutcome, OwnedTransferError> {
    apply_owned_unwrap_certificate_v3_at_decision(
        ledger,
        certificate,
        context,
        verification_height,
        certificate.order.recovery.valid_from_height,
        postfiat_types::FastPayFenceOriginV1::Consensusless,
    )
}

fn apply_owned_unwrap_certificate_v3_at_decision(
    ledger: &mut LedgerState,
    certificate: &postfiat_types::OwnedUnwrapCertificateV3,
    context: FastPayRecoveryVerificationContext<'_>,
    verification_height: u64,
    decision_height: u64,
    origin: postfiat_types::FastPayFenceOriginV1,
) -> Result<OwnedUnwrapOutcome, OwnedTransferError> {
    verify_owned_unwrap_certificate_v3(
        certificate,
        context.validator_public_keys,
        context.expected_domain,
        context.committee_epoch,
        context.policy,
        verification_height,
        context.quorum,
    )?;
    ensure_fastpay_inputs_unfenced(ledger, &certificate.order.inputs)?;
    let fence = unwrap_confirmed_fence(certificate, decision_height, origin)?;
    ensure_fastpay_fence_capacity(ledger)?;
    let outcome = apply_owned_unwrap(
        ledger,
        &unwrap_v3_semantic_order(&certificate.order),
        &certificate.owner_pubkey_hex,
    )?;
    ledger.fastpay_version_fences.push(fence);
    Ok(outcome)
}

pub fn fastpay_transfer_certificate_digest_v3(
    certificate: &postfiat_types::OwnedTransferCertificateV3,
) -> Result<String, OwnedTransferError> {
    let mut bytes = owned_transfer_v3_signing_bytes(&certificate.order);
    append_fastpay_v3_text(&mut bytes, &certificate.owner_pubkey_hex);
    append_fastpay_v3_text(&mut bytes, &certificate.owner_signature_hex);
    let mut votes = certificate.votes.iter().collect::<Vec<_>>();
    votes.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
    let mut seen = std::collections::BTreeSet::new();
    bytes.extend_from_slice(&(votes.len() as u64).to_le_bytes());
    for vote in votes {
        if !seen.insert(vote.validator_id.as_str()) {
            return Err(OwnedTransferError::InvalidRecovery);
        }
        append_fastpay_v3_text(&mut bytes, &vote.validator_id);
        append_fastpay_v3_text(&mut bytes, &vote.signature_hex);
    }
    Ok(postfiat_crypto_provider::bytes_to_hex(
        &postfiat_crypto_provider::hash_bytes(
            postfiat_types::FASTPAY_CERTIFICATE_DIGEST_DOMAIN_V1,
            &bytes,
        ),
    ))
}

pub fn fastpay_unwrap_certificate_digest_v3(
    certificate: &postfiat_types::OwnedUnwrapCertificateV3,
) -> Result<String, OwnedTransferError> {
    let mut bytes = owned_unwrap_v3_signing_bytes(&certificate.order);
    append_fastpay_v3_text(&mut bytes, &certificate.owner_pubkey_hex);
    append_fastpay_v3_text(&mut bytes, &certificate.owner_signature_hex);
    let mut votes = certificate.votes.iter().collect::<Vec<_>>();
    votes.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
    let mut seen = std::collections::BTreeSet::new();
    bytes.extend_from_slice(&(votes.len() as u64).to_le_bytes());
    for vote in votes {
        if !seen.insert(vote.validator_id.as_str()) {
            return Err(OwnedTransferError::InvalidRecovery);
        }
        append_fastpay_v3_text(&mut bytes, &vote.validator_id);
        append_fastpay_v3_text(&mut bytes, &vote.signature_hex);
    }
    Ok(postfiat_crypto_provider::bytes_to_hex(
        &postfiat_crypto_provider::hash_bytes(
            postfiat_types::FASTPAY_CERTIFICATE_DIGEST_DOMAIN_V1,
            &bytes,
        ),
    ))
}

fn ensure_fastpay_fence_capacity(ledger: &LedgerState) -> Result<(), OwnedTransferError> {
    if ledger.fastpay_version_fences.len() >= postfiat_types::MAX_FASTPAY_VERSION_FENCES {
        return Err(OwnedTransferError::ResourceLimitExceeded);
    }
    Ok(())
}

fn ensure_fastpay_inputs_unfenced(
    ledger: &LedgerState,
    inputs: &[postfiat_types::OwnedObjectRef],
) -> Result<(), OwnedTransferError> {
    if inputs.iter().any(|input| {
        ledger.fastpay_version_fences.iter().any(|fence| {
            fence
                .inputs
                .iter()
                .any(|fenced| fenced.id == input.id && fenced.version == input.version)
        })
    }) {
        return Err(OwnedTransferError::VersionFenced);
    }
    Ok(())
}

fn next_fastpay_versions(
    inputs: &[postfiat_types::OwnedObjectRef],
) -> Result<Vec<postfiat_types::OwnedObjectRef>, OwnedTransferError> {
    inputs
        .iter()
        .map(|input| {
            Ok(postfiat_types::OwnedObjectRef {
                id: input.id.clone(),
                version: input
                    .version
                    .checked_add(1)
                    .ok_or(OwnedTransferError::Overflow)?,
            })
        })
        .collect()
}

fn transfer_confirmed_fence(
    certificate: &postfiat_types::OwnedTransferCertificateV3,
    decision_height: u64,
    origin: postfiat_types::FastPayFenceOriginV1,
) -> Result<postfiat_types::FastPayVersionFenceV1, OwnedTransferError> {
    let fence = postfiat_types::FastPayVersionFenceV1 {
        schema: postfiat_types::FASTPAY_VERSION_FENCE_SCHEMA_V1.to_string(),
        operation: postfiat_types::FastPayOperationKindV1::Transfer,
        origin,
        committee_epoch: certificate.order.recovery.committee_epoch,
        registry_root: certificate.order.domain.registry_id.clone(),
        lock_id: certificate.order.recovery.lock_id.clone(),
        inputs: certificate.order.inputs.clone(),
        decision: postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
            order_digest: fastpay_transfer_order_digest_v3(&certificate.order),
            certificate_digest: fastpay_transfer_certificate_digest_v3(certificate)?,
        },
        certificate: Some(postfiat_types::FastPayCertificateV1::Transfer(
            certificate.clone(),
        )),
        decided_at_height: decision_height,
        next_versions: next_fastpay_versions(&certificate.order.inputs)?,
    };
    fence
        .validate_shape()
        .map_err(|_| OwnedTransferError::InvalidRecovery)?;
    Ok(fence)
}

fn unwrap_confirmed_fence(
    certificate: &postfiat_types::OwnedUnwrapCertificateV3,
    decision_height: u64,
    origin: postfiat_types::FastPayFenceOriginV1,
) -> Result<postfiat_types::FastPayVersionFenceV1, OwnedTransferError> {
    let fence = postfiat_types::FastPayVersionFenceV1 {
        schema: postfiat_types::FASTPAY_VERSION_FENCE_SCHEMA_V1.to_string(),
        operation: postfiat_types::FastPayOperationKindV1::Unwrap,
        origin,
        committee_epoch: certificate.order.recovery.committee_epoch,
        registry_root: certificate.order.domain.registry_id.clone(),
        lock_id: certificate.order.recovery.lock_id.clone(),
        inputs: certificate.order.inputs.clone(),
        decision: postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
            order_digest: fastpay_unwrap_order_digest_v3(&certificate.order),
            certificate_digest: fastpay_unwrap_certificate_digest_v3(certificate)?,
        },
        certificate: Some(postfiat_types::FastPayCertificateV1::Unwrap(
            certificate.clone(),
        )),
        decided_at_height: decision_height,
        next_versions: next_fastpay_versions(&certificate.order.inputs)?,
    };
    fence
        .validate_shape()
        .map_err(|_| OwnedTransferError::InvalidRecovery)?;
    Ok(fence)
}

pub fn record_fastpay_recovery_reveal_v1(
    ledger: &mut LedgerState,
    certificate: postfiat_types::FastPayCertificateV1,
    context: FastPayRecoveryVerificationContext<'_>,
    current_height: u64,
) -> Result<postfiat_types::FastPayRecoveryRevealV1, OwnedTransferError> {
    let recovery = certificate.recovery();
    if current_height <= recovery.expires_at_height {
        return Err(OwnedTransferError::NotYetValid);
    }
    if current_height >= recovery.recovery_closes_at_height {
        return Err(OwnedTransferError::Expired);
    }
    ensure_fastpay_inputs_unfenced(ledger, certificate.inputs())?;
    if ledger.fastpay_recovery_reveals.len() >= postfiat_types::MAX_FASTPAY_RECOVERY_REVEALS {
        return Err(OwnedTransferError::ResourceLimitExceeded);
    }
    let (order_digest, certificate_digest) = match &certificate {
        postfiat_types::FastPayCertificateV1::Transfer(value) => {
            verify_owned_transfer_certificate_v3(
                value,
                context.validator_public_keys,
                context.expected_domain,
                context.committee_epoch,
                context.policy,
                value.order.recovery.expires_at_height,
                context.quorum,
            )?;
            (
                fastpay_transfer_order_digest_v3(&value.order),
                fastpay_transfer_certificate_digest_v3(value)?,
            )
        }
        postfiat_types::FastPayCertificateV1::Unwrap(value) => {
            verify_owned_unwrap_certificate_v3(
                value,
                context.validator_public_keys,
                context.expected_domain,
                context.committee_epoch,
                context.policy,
                value.order.recovery.expires_at_height,
                context.quorum,
            )?;
            (
                fastpay_unwrap_order_digest_v3(&value.order),
                fastpay_unwrap_certificate_digest_v3(value)?,
            )
        }
    };
    if let Some(existing) = ledger
        .fastpay_recovery_reveals
        .iter()
        .find(|existing| existing.certificate_digest == certificate_digest)
    {
        return Ok(existing.clone());
    }
    let reveal = postfiat_types::FastPayRecoveryRevealV1 {
        schema: postfiat_types::FASTPAY_RECOVERY_REVEAL_SCHEMA_V1.to_string(),
        lock_id: recovery.lock_id.clone(),
        order_digest,
        certificate_digest,
        revealed_at_height: current_height,
        certificate,
    };
    reveal
        .validate_shape()
        .map_err(|_| OwnedTransferError::InvalidRecovery)?;
    ledger.fastpay_recovery_reveals.push(reveal.clone());
    Ok(reveal)
}

pub fn execute_fastpay_recovery_decision_v1(
    ledger: &mut LedgerState,
    request: &postfiat_types::FastPayRecoveryDecisionRequestV1,
    context: FastPayRecoveryVerificationContext<'_>,
    current_height: u64,
) -> Result<postfiat_types::FastPayVersionFenceV1, OwnedTransferError> {
    request
        .validate_shape()
        .map_err(|_| OwnedTransferError::InvalidRecovery)?;
    if request.submitted_at_height != current_height
        || current_height < request.signed_order.recovery().recovery_closes_at_height
    {
        return Err(OwnedTransferError::NotYetValid);
    }
    let inputs = request.signed_order.inputs();
    if let Some(existing) = ledger.fastpay_version_fences.iter().find(|fence| {
        fence.lock_id == request.signed_order.recovery().lock_id && fence.inputs == inputs
    }) {
        return Ok(existing.clone());
    }
    ensure_fastpay_inputs_unfenced(ledger, inputs)?;
    ensure_fastpay_fence_capacity(ledger)?;

    let lock_id = request.signed_order.recovery().lock_id.as_str();
    let mut revealed = ledger
        .fastpay_recovery_reveals
        .iter()
        .filter(|reveal| reveal.lock_id == lock_id)
        .collect::<Vec<_>>();
    revealed.sort_by(|left, right| left.certificate_digest.cmp(&right.certificate_digest));
    revealed.dedup_by(|left, right| left.certificate_digest == right.certificate_digest);
    if revealed.len() > 1 {
        return Err(OwnedTransferError::InvalidRecovery);
    }

    if let Some(reveal) = revealed.first() {
        let expected_order_digest = match &request.signed_order {
            postfiat_types::FastPaySignedOrderV1::Transfer(value) => {
                fastpay_transfer_order_digest_v3(&value.order)
            }
            postfiat_types::FastPaySignedOrderV1::Unwrap(value) => {
                fastpay_unwrap_order_digest_v3(&value.order)
            }
        };
        if reveal.order_digest != expected_order_digest
            || reveal.certificate.operation() != request.signed_order.operation()
            || reveal.certificate.inputs() != inputs
        {
            return Err(OwnedTransferError::InvalidRecovery);
        }
        let certificate = reveal.certificate.clone();
        let verification_height = certificate.recovery().expires_at_height;
        match certificate {
            postfiat_types::FastPayCertificateV1::Transfer(value) => {
                apply_owned_transfer_certificate_v3_at_decision(
                    ledger,
                    &value,
                    context,
                    verification_height,
                    current_height,
                    postfiat_types::FastPayFenceOriginV1::OrderedRecovery,
                )?;
            }
            postfiat_types::FastPayCertificateV1::Unwrap(value) => {
                apply_owned_unwrap_certificate_v3_at_decision(
                    ledger,
                    &value,
                    context,
                    verification_height,
                    current_height,
                    postfiat_types::FastPayFenceOriginV1::OrderedRecovery,
                )?;
            }
        }
        return ledger
            .fastpay_version_fences
            .last()
            .cloned()
            .ok_or(OwnedTransferError::InvalidRecovery);
    }

    // No full certificate exists in ordered recovery state. Re-validate the
    // owner-authorized order and current inputs at the last valid height before
    // atomically advancing the object versions without changing value/owner.
    match &request.signed_order {
        postfiat_types::FastPaySignedOrderV1::Transfer(value) => {
            validate_owned_transfer_v3_admission(
                ledger,
                value,
                context.expected_domain,
                context.committee_epoch,
                context.policy,
                value.order.recovery.expires_at_height,
            )?;
        }
        postfiat_types::FastPaySignedOrderV1::Unwrap(value) => {
            validate_owned_unwrap_v3_admission(
                ledger,
                value,
                context.expected_domain,
                context.committee_epoch,
                context.policy,
                value.order.recovery.expires_at_height,
            )?;
        }
    }
    let next_versions = next_fastpay_versions(inputs)?;
    let fence = postfiat_types::FastPayVersionFenceV1 {
        schema: postfiat_types::FASTPAY_VERSION_FENCE_SCHEMA_V1.to_string(),
        operation: request.signed_order.operation(),
        origin: postfiat_types::FastPayFenceOriginV1::OrderedRecovery,
        committee_epoch: request.signed_order.recovery().committee_epoch,
        registry_root: context.expected_domain.registry_id.clone(),
        lock_id: lock_id.to_string(),
        inputs: inputs.to_vec(),
        decision: postfiat_types::FastPayRecoveryDecisionV1::Cancelled,
        certificate: None,
        decided_at_height: current_height,
        next_versions,
    };
    fence
        .validate_shape()
        .map_err(|_| OwnedTransferError::InvalidRecovery)?;
    for (input, next) in inputs.iter().zip(&fence.next_versions) {
        let object = ledger
            .owned_objects
            .iter_mut()
            .find(|object| object.id == input.id && object.version == input.version)
            .ok_or(OwnedTransferError::UnknownInput)?;
        object.version = next.version;
    }
    ledger.fastpay_version_fences.push(fence.clone());
    Ok(fence)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FastPayRecoveryGovernanceOutcomeV1 {
    Bootstrapped,
    CommitteeRotated,
}

pub fn execute_fastpay_recovery_governance_update_v1(
    ledger: &mut LedgerState,
    bootstrap: &postfiat_types::FastPayRecoveryGovernanceBootstrapV1,
    finalized_height: u64,
) -> Result<FastPayRecoveryGovernanceOutcomeV1, String> {
    bootstrap.validate_payload_binding()?;
    let mut prospective = ledger.clone();
    let outcome = match (
        prospective.fastpay_recovery_policy.as_ref(),
        prospective.fastpay_recovery_committees.last(),
    ) {
        (None, None) => {
            if !prospective.fastpay_recovery_reveals.is_empty()
                || !prospective.fastpay_version_fences.is_empty()
                || bootstrap.payload.policy.activation_height
                    != bootstrap.payload.committee.valid_from_height
                || bootstrap.payload.policy.activation_height <= finalized_height
            {
                return Err("FastPay recovery bootstrap conflicts with existing state".to_string());
            }
            prospective.fastpay_recovery_policy = Some(bootstrap.payload.policy.clone());
            prospective
                .fastpay_recovery_committees
                .push(bootstrap.payload.committee.clone());
            FastPayRecoveryGovernanceOutcomeV1::Bootstrapped
        }
        (Some(active_policy), Some(previous)) => {
            if active_policy != &bootstrap.payload.policy {
                return Err(
                    "FastPay committee rotation cannot change the recovery policy".to_string(),
                );
            }
            previous.validate()?;
            let next_epoch = previous
                .committee_epoch
                .checked_add(1)
                .ok_or_else(|| "FastPay committee epoch overflow".to_string())?;
            let next_height = previous
                .new_orders_through_height
                .checked_add(1)
                .ok_or_else(|| "FastPay committee admission height overflow".to_string())?;
            let next = &bootstrap.payload.committee;
            if (previous.commitment_version()?
                == postfiat_types::FastPayRecoveryCommitmentVersion::V2
                && next.commitment_version()?
                    == postfiat_types::FastPayRecoveryCommitmentVersion::V1)
                || prospective.fastpay_recovery_committees.len()
                    >= postfiat_types::MAX_FASTPAY_RECOVERY_COMMITTEES
                || next.committee_epoch != next_epoch
                || next.valid_from_height != next_height
                || next.valid_from_height <= finalized_height
                || next.chain_id != previous.chain_id
                || next.genesis_hash != previous.genesis_hash
                || next.protocol_version != previous.protocol_version
                || prospective
                    .fastpay_recovery_committees
                    .iter()
                    .any(|existing| {
                        existing.committee_epoch == next.committee_epoch
                            || existing.registry_root == next.registry_root
                    })
            {
                return Err(
                    "FastPay committee rotation is not the next future non-overlapping epoch"
                        .to_string(),
                );
            }
            prospective.fastpay_recovery_committees.push(next.clone());
            FastPayRecoveryGovernanceOutcomeV1::CommitteeRotated
        }
        _ => {
            return Err("FastPay recovery policy and committee state are inconsistent".to_string())
        }
    };
    // V1 bytes remain unchanged. A V2 installation over historically accepted
    // unencodable certificates fails atomically; never trim or rewrite history.
    if bootstrap.payload.committee.commitment_version()?
        == postfiat_types::FastPayRecoveryCommitmentVersion::V2
    {
        for reveal in &prospective.fastpay_recovery_reveals {
            reveal.state_commitment_bytes_for_version(
                postfiat_types::FastPayRecoveryCommitmentVersion::V2,
            )?;
        }
        for fence in &prospective.fastpay_version_fences {
            fence.state_commitment_bytes_for_version(
                postfiat_types::FastPayRecoveryCommitmentVersion::V2,
            )?;
        }
    }
    *ledger = prospective;
    Ok(outcome)
}

#[cfg(test)]
mod owned_transfer_recovery_tests {
    use super::*;
    use crate::fastlane_primary::execute_fastlane_primary_transaction;

    include!("fastpay_certificate_bound_tests.rs");

    fn recovery_validator_keys() -> Vec<(String, postfiat_crypto_provider::MlDsa65KeyPair)> {
        (0..4)
            .map(|index| {
                (
                    format!("validator-{index}"),
                    postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[50 + index as u8; 32]),
                )
            })
            .collect()
    }

    fn domain() -> postfiat_types::OwnedCertificateDomain {
        postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
            "fastpay-v3-execution".to_string(),
            "11".repeat(48),
            3,
            7,
            90,
            110,
            recovery_validator_keys()
                .into_iter()
                .map(|(validator_id, keypair)| {
                    (
                        validator_id,
                        postfiat_crypto_provider::bytes_to_hex(&keypair.public_key),
                    )
                })
                .collect(),
        )
        .expect("execution recovery committee")
        .certificate_domain()
    }

    fn policy() -> postfiat_types::FastPayRecoveryPolicyV1 {
        postfiat_types::FastPayRecoveryPolicyV1 {
            schema: postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
            activation_height: 90,
            max_validity_blocks: 20,
            max_recovery_blocks: 20,
        }
    }

    fn signed_certificate(
        input_id: &str,
    ) -> (
        postfiat_types::OwnedTransferCertificateV3,
        Vec<(String, String)>,
    ) {
        signed_certificate_for_domain(input_id, domain())
    }

    fn signed_certificate_for_domain(
        input_id: &str,
        certificate_domain: postfiat_types::OwnedCertificateDomain,
    ) -> (
        postfiat_types::OwnedTransferCertificateV3,
        Vec<(String, String)>,
    ) {
        signed_certificate_with_window(input_id, certificate_domain, 7, 100)
    }

    /// Order valid from `valid_from` for 10 blocks, recovery closing 10 later.
    fn signed_certificate_with_window(
        input_id: &str,
        certificate_domain: postfiat_types::OwnedCertificateDomain,
        committee_epoch: u64,
        valid_from: u64,
    ) -> (
        postfiat_types::OwnedTransferCertificateV3,
        Vec<(String, String)>,
    ) {
        let owner = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner keygen");
        let owner_pubkey_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
        let mut order = postfiat_types::OwnedTransferOrderV3 {
            domain: certificate_domain,
            recovery: postfiat_types::FastPayOrderRecoveryV1 {
                schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
                committee_epoch,
                lock_id: "00".repeat(48),
                valid_from_height: valid_from,
                expires_at_height: valid_from + 10,
                recovery_closes_at_height: valid_from + 20,
            },
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: input_id.to_string(),
                version: 1,
            }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: owner_pubkey_hex.clone(),
                value: 99,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 1,
            memos: Vec::new(),
        };
        order.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&order);
        let signing_bytes = owned_transfer_v3_signing_bytes(&order);
        let owner_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &owner.private_key,
            &signing_bytes,
            OWNED_TRANSFER_CONTEXT_V3,
        )
        .expect("owner sign");
        let validators = recovery_validator_keys();
        let votes = validators
            .iter()
            .map(|(validator_id, keypair)| {
                let signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                    &keypair.private_key,
                    &signing_bytes,
                    OWNED_TRANSFER_CONTEXT_V3,
                )
                .expect("validator sign");
                postfiat_types::OwnedTransferVote {
                    validator_id: validator_id.clone(),
                    signature_hex: postfiat_crypto_provider::bytes_to_hex(&signature),
                }
            })
            .collect();
        let public_keys = validators
            .iter()
            .map(|(validator_id, keypair)| {
                (
                    validator_id.clone(),
                    postfiat_crypto_provider::bytes_to_hex(&keypair.public_key),
                )
            })
            .collect();
        (
            postfiat_types::OwnedTransferCertificateV3 {
                order,
                owner_pubkey_hex,
                owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_signature),
                votes,
            },
            public_keys,
        )
    }

    #[test]
    fn v3_certificate_applies_only_inside_window_with_exact_derived_lock() {
        let (certificate, validator_pks) = signed_certificate("input-v3");
        let expected_domain = domain();
        let recovery_policy = policy();
        let context = FastPayRecoveryVerificationContext {
            validator_public_keys: &validator_pks,
            expected_domain: &expected_domain,
            committee_epoch: 7,
            policy: &recovery_policy,
            quorum: 3,
        };
        let mut ledger = LedgerState::empty();
        ledger.owned_objects.push(postfiat_types::OwnedObject {
            id: "input-v3".to_string(),
            version: 1,
            owner_pubkey_hex: certificate.owner_pubkey_hex.clone(),
            value: 100,
            asset: "PFT".to_string(),
        });
        let before = ledger.clone();
        assert_eq!(
            apply_owned_transfer_certificate_v3(&mut ledger, &certificate, context, 111,),
            Err(OwnedTransferError::Expired)
        );
        assert_eq!(ledger, before);

        let outcome = apply_owned_transfer_certificate_v3(&mut ledger, &certificate, context, 110)
            .expect("v3 certificate apply");
        assert_eq!(outcome.consumed, 1);
        assert_eq!(
            outcome
                .created
                .iter()
                .map(|object| object.value)
                .sum::<u64>(),
            99
        );
    }

    #[test]
    fn v3_direct_apply_is_identical_across_honest_validator_tip_heights() {
        let (certificate, validator_pks) = signed_certificate("input-v3-height-stable");
        let expected_domain = domain();
        let recovery_policy = policy();
        let context = FastPayRecoveryVerificationContext {
            validator_public_keys: &validator_pks,
            expected_domain: &expected_domain,
            committee_epoch: 7,
            policy: &recovery_policy,
            quorum: 3,
        };
        let mut early = LedgerState::empty();
        early.owned_objects.push(postfiat_types::OwnedObject {
            id: "input-v3-height-stable".to_string(),
            version: 1,
            owner_pubkey_hex: certificate.owner_pubkey_hex.clone(),
            value: 100,
            asset: "PFT".to_string(),
        });
        let mut late = early.clone();

        apply_owned_transfer_certificate_v3(&mut early, &certificate, context, 100)
            .expect("apply at first valid height");
        apply_owned_transfer_certificate_v3(&mut late, &certificate, context, 110)
            .expect("apply at last valid height");

        assert_eq!(
            early, late,
            "local arrival height must not enter replicated state"
        );
        assert_eq!(
            early.fastpay_version_fences[0].decided_at_height,
            certificate.order.recovery.valid_from_height,
            "direct confirmation uses the owner-signed reference height",
        );
    }

    #[test]
    fn v3_tampered_lock_duplicate_vote_and_foreign_epoch_do_not_mutate() {
        let (certificate, validator_pks) = signed_certificate("input-v3-negative");
        let ledger = LedgerState::empty();
        let signed = postfiat_types::SignedOwnedTransferOrderV3 {
            order: certificate.order.clone(),
            owner_pubkey_hex: certificate.owner_pubkey_hex.clone(),
            owner_signature_hex: certificate.owner_signature_hex.clone(),
        };
        let mut wrong_lock = signed.clone();
        wrong_lock.order.recovery.lock_id = "ff".repeat(48);
        assert_eq!(
            validate_owned_transfer_v3_admission(
                &ledger,
                &wrong_lock,
                &domain(),
                7,
                &policy(),
                105,
            ),
            Err(OwnedTransferError::InvalidRecovery)
        );

        let mut duplicate = certificate.clone();
        duplicate.votes[1] = duplicate.votes[0].clone();
        assert_eq!(
            verify_owned_transfer_certificate_v3(
                &duplicate,
                &validator_pks,
                &domain(),
                7,
                &policy(),
                105,
                3,
            ),
            Err(OwnedTransferError::InvalidRecovery)
        );
        assert_eq!(
            verify_owned_transfer_certificate_v3(
                &certificate,
                &validator_pks,
                &domain(),
                8,
                &policy(),
                105,
                3,
            ),
            Err(OwnedTransferError::InvalidRecovery)
        );
    }

    #[test]
    fn v3_certificate_digest_is_vote_order_independent_and_duplicate_fail_closed() {
        let (certificate, _) = signed_certificate("input-v3-digest");
        let digest = fastpay_transfer_certificate_digest_v3(&certificate).expect("digest");
        let mut reordered = certificate.clone();
        reordered.votes.reverse();
        assert_eq!(
            fastpay_transfer_certificate_digest_v3(&reordered).expect("reordered digest"),
            digest
        );
        reordered.votes[1] = reordered.votes[0].clone();
        assert_eq!(
            fastpay_transfer_certificate_digest_v3(&reordered),
            Err(OwnedTransferError::InvalidRecovery)
        );
    }

    fn ledger_with_input(certificate: &postfiat_types::OwnedTransferCertificateV3) -> LedgerState {
        let mut ledger = LedgerState::empty();
        ledger.fastpay_recovery_policy = Some(policy());
        ledger.owned_objects.push(postfiat_types::OwnedObject {
            id: certificate.order.inputs[0].id.clone(),
            version: certificate.order.inputs[0].version,
            owner_pubkey_hex: certificate.owner_pubkey_hex.clone(),
            value: 100,
            asset: "PFT".to_string(),
        });
        ledger
    }

    fn recovery_request(
        certificate: &postfiat_types::OwnedTransferCertificateV3,
        height: u64,
    ) -> postfiat_types::FastPayRecoveryDecisionRequestV1 {
        postfiat_types::FastPayRecoveryDecisionRequestV1 {
            schema: postfiat_types::FASTPAY_RECOVERY_DECISION_REQUEST_SCHEMA_V1.to_string(),
            submitted_at_height: height,
            signed_order: postfiat_types::FastPaySignedOrderV1::Transfer(
                postfiat_types::SignedOwnedTransferOrderV3 {
                    order: certificate.order.clone(),
                    owner_pubkey_hex: certificate.owner_pubkey_hex.clone(),
                    owner_signature_hex: certificate.owner_signature_hex.clone(),
                },
            ),
        }
    }

    #[test]
    fn abandoned_lock_cancels_at_boundary_and_delayed_certificate_is_fenced() {
        let (certificate, validator_pks) = signed_certificate("input-v3-cancel");
        let expected_domain = domain();
        let recovery_policy = policy();
        let context = FastPayRecoveryVerificationContext {
            validator_public_keys: &validator_pks,
            expected_domain: &expected_domain,
            committee_epoch: 7,
            policy: &recovery_policy,
            quorum: 3,
        };
        let mut ledger = ledger_with_input(&certificate);
        let request = recovery_request(&certificate, 120);
        let fence = execute_fastpay_recovery_decision_v1(&mut ledger, &request, context, 120)
            .expect("cancel abandoned lock");
        assert_eq!(
            fence.decision,
            postfiat_types::FastPayRecoveryDecisionV1::Cancelled
        );
        let advanced = ledger
            .owned_objects
            .iter()
            .find(|object| object.id == "input-v3-cancel")
            .expect("advanced object");
        assert_eq!((advanced.version, advanced.value), (2, 100));
        assert_eq!(
            apply_owned_transfer_certificate_v3(&mut ledger, &certificate, context, 110,),
            Err(OwnedTransferError::VersionFenced)
        );
        assert_eq!(ledger.owned_objects[0].value, 100);
    }

    #[test]
    fn complete_certificate_reveal_confirms_after_expiry_and_conserves() {
        let (certificate, validator_pks) = signed_certificate("input-v3-recover-confirm");
        let expected_domain = domain();
        let recovery_policy = policy();
        let context = FastPayRecoveryVerificationContext {
            validator_public_keys: &validator_pks,
            expected_domain: &expected_domain,
            committee_epoch: 7,
            policy: &recovery_policy,
            quorum: 3,
        };
        let mut ledger = ledger_with_input(&certificate);
        let reveal = record_fastpay_recovery_reveal_v1(
            &mut ledger,
            postfiat_types::FastPayCertificateV1::Transfer(certificate.clone()),
            context,
            111,
        )
        .expect("record full certificate reveal");
        assert_eq!(reveal.lock_id, certificate.order.recovery.lock_id);
        assert_eq!(ledger.owned_objects[0].value, 100);

        let request = recovery_request(&certificate, 120);
        let fence = execute_fastpay_recovery_decision_v1(&mut ledger, &request, context, 120)
            .expect("confirm revealed certificate");
        assert!(matches!(
            fence.decision,
            postfiat_types::FastPayRecoveryDecisionV1::Confirmed { .. }
        ));
        assert!(ledger
            .owned_objects
            .iter()
            .all(|object| object.id != "input-v3-recover-confirm"));
        assert_eq!(
            ledger
                .owned_objects
                .iter()
                .map(|object| object.value)
                .sum::<u64>(),
            99
        );
        assert_eq!(ledger.fastpay_version_fences.len(), 1);
    }

    #[test]
    fn conflicting_recovery_certificates_halt_without_mutation() {
        let (certificate, validator_pks) = signed_certificate("input-v3-conflict");
        let expected_domain = domain();
        let recovery_policy = policy();
        let context = FastPayRecoveryVerificationContext {
            validator_public_keys: &validator_pks,
            expected_domain: &expected_domain,
            committee_epoch: 7,
            policy: &recovery_policy,
            quorum: 3,
        };
        let mut ledger = ledger_with_input(&certificate);
        let reveal = record_fastpay_recovery_reveal_v1(
            &mut ledger,
            postfiat_types::FastPayCertificateV1::Transfer(certificate.clone()),
            context,
            111,
        )
        .expect("first reveal");
        let mut conflicting = reveal;
        conflicting.certificate_digest = "ee".repeat(48);
        ledger.fastpay_recovery_reveals.push(conflicting);
        let before = ledger.clone();
        assert_eq!(
            execute_fastpay_recovery_decision_v1(
                &mut ledger,
                &recovery_request(&certificate, 120),
                context,
                120,
            ),
            Err(OwnedTransferError::InvalidRecovery)
        );
        assert_eq!(ledger, before);
    }

    fn recovery_governance_update(
        policy: postfiat_types::FastPayRecoveryPolicyV1,
        committee: postfiat_types::FastPayRecoveryCommitteeV1,
    ) -> postfiat_types::FastPayRecoveryGovernanceBootstrapV1 {
        let payload = postfiat_types::FastPayRecoveryGovernancePayloadV1 { policy, committee };
        let validator_ids = payload
            .committee
            .validators
            .iter()
            .map(|validator| validator.validator_id.clone())
            .collect::<Vec<_>>();
        let payload_id = payload
            .payload_id()
            .expect("recovery governance payload ID");
        postfiat_types::FastPayRecoveryGovernanceBootstrapV1 {
            amendment: postfiat_types::GovernanceAmendment {
                amendment_id: format!("fastpay-recovery-{payload_id}"),
                chain_id: payload.committee.chain_id.clone(),
                genesis_hash: payload.committee.genesis_hash.clone(),
                protocol_version: payload.committee.protocol_version,
                instance_id: "fastpay-recovery-test".to_string(),
                proposal_id: format!("proposal-{payload_id}"),
                certificate_id: format!("certificate-{payload_id}"),
                proposer: validator_ids[0].clone(),
                validators: validator_ids.clone(),
                quorum: payload.committee.quorum,
                kind: format!(
                    "{}{}",
                    postfiat_types::FASTPAY_RECOVERY_GOVERNANCE_KIND_PREFIX_V1,
                    payload_id
                ),
                value: postfiat_types::FASTPAY_RECOVERY_GOVERNANCE_VERSION_V1,
                activation_height: 0,
                veto_until_height: 0,
                paused: false,
                support: validator_ids,
                votes: Vec::new(),
                signed_authorizations: Vec::new(),
            },
            payload,
        }
    }

    #[test]
    fn governed_committee_rotation_preserves_old_recovery_and_fences_overlap() {
        let policy = policy();
        let validators = recovery_validator_keys()
            .into_iter()
            .map(|(validator_id, keypair)| {
                (
                    validator_id,
                    postfiat_crypto_provider::bytes_to_hex(&keypair.public_key),
                )
            })
            .collect::<Vec<_>>();
        let mut first = postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
            domain().chain_id,
            domain().genesis_hash,
            domain().protocol_version,
            7,
            90,
            110,
            validators.clone(),
        )
        .expect("first recovery committee");
        first.schema = postfiat_types::FASTPAY_RECOVERY_COMMITTEE_SCHEMA_V1.to_string();
        first.registry_root = first.computed_root().unwrap();
        let second = postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
            first.chain_id.clone(),
            first.genesis_hash.clone(),
            first.protocol_version,
            8,
            111,
            140,
            validators.clone(),
        )
        .expect("second recovery committee");
        let mut ledger = LedgerState::empty();
        assert_eq!(
            execute_fastpay_recovery_governance_update_v1(
                &mut ledger,
                &recovery_governance_update(policy.clone(), first.clone()),
                80,
            ),
            Ok(FastPayRecoveryGovernanceOutcomeV1::Bootstrapped)
        );
        assert_eq!(
            execute_fastpay_recovery_governance_update_v1(
                &mut ledger,
                &recovery_governance_update(policy.clone(), second.clone()),
                100,
            ),
            Ok(FastPayRecoveryGovernanceOutcomeV1::CommitteeRotated)
        );
        assert_eq!(ledger.fastpay_recovery_policy, Some(policy.clone()));
        assert_eq!(
            ledger.fastpay_recovery_committees,
            vec![first.clone(), second]
        );

        let (old_certificate, _) = signed_certificate_for_domain(
            "old-committee-recovery-input",
            first.certificate_domain(),
        );
        let mut downgraded = ledger.fastpay_recovery_committees.last().unwrap().clone();
        downgraded.committee_epoch += 1;
        downgraded.valid_from_height = 141;
        downgraded.new_orders_through_height = 160;
        downgraded.schema = postfiat_types::FASTPAY_RECOVERY_COMMITTEE_SCHEMA_V1.to_string();
        downgraded.registry_root = downgraded.computed_root().unwrap();
        let before_downgrade = ledger.clone();
        assert!(execute_fastpay_recovery_governance_update_v1(
            &mut ledger,
            &recovery_governance_update(policy.clone(), downgraded),
            100,
        )
        .is_err());
        assert_eq!(ledger, before_downgrade, "downgrade must reject atomically");
        ledger.owned_objects.push(postfiat_types::OwnedObject {
            id: old_certificate.order.inputs[0].id.clone(),
            version: old_certificate.order.inputs[0].version,
            owner_pubkey_hex: old_certificate.owner_pubkey_hex.clone(),
            value: 100,
            asset: "PFT".to_string(),
        });
        let old_recovery = postfiat_types::FastLanePrimaryTransactionV1 {
            operation: postfiat_types::FastLanePrimaryOperationV1::FastPayRecoveryDecision {
                request: recovery_request(&old_certificate, 120),
            },
        };
        let receipt = execute_fastlane_primary_transaction(&mut ledger, &old_recovery, 120);
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(receipt.code, "fastpay_recovery_cancelled");
        assert_eq!(
            ledger
                .owned_objects
                .iter()
                .find(|object| object.id == "old-committee-recovery-input")
                .expect("old committee input survives cancellation")
                .version,
            2
        );

        let overlapping = postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
            first.chain_id,
            first.genesis_hash,
            first.protocol_version,
            9,
            140,
            160,
            validators,
        )
        .expect("overlapping recovery committee");
        let before = ledger.clone();
        assert!(execute_fastpay_recovery_governance_update_v1(
            &mut ledger,
            &recovery_governance_update(policy, overlapping),
            120,
        )
        .is_err());
        assert_eq!(ledger, before);
    }

    /// Release-day FastPay rotation: epoch 1 accepts new orders through a
    /// height far above the tip (10000 on the live chain). The epoch-2 record
    /// installed far below it activates exactly at 10001; a payment under
    /// epoch 2 is refused at 10000 and accepted at 10001.
    #[test]
    fn fastpay_epoch_two_installed_far_below_deadline_activates_at_previous_end_plus_one() {
        let policy = policy();
        let validators = recovery_validator_keys()
            .into_iter()
            .map(|(validator_id, keypair)| {
                (
                    validator_id,
                    postfiat_crypto_provider::bytes_to_hex(&keypair.public_key),
                )
            })
            .collect::<Vec<_>>();
        let committee = |epoch, from, through| {
            postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
                domain().chain_id,
                domain().genesis_hash,
                domain().protocol_version,
                epoch,
                from,
                through,
                validators.clone(),
            )
            .expect("recovery committee")
        };
        let epoch1 = committee(1, 90, 10_000);
        let epoch2 = committee(2, 10_001, 20_000);
        let mut ledger = LedgerState::empty();
        assert_eq!(
            execute_fastpay_recovery_governance_update_v1(
                &mut ledger,
                &recovery_governance_update(policy.clone(), epoch1.clone()),
                80,
            ),
            Ok(FastPayRecoveryGovernanceOutcomeV1::Bootstrapped)
        );
        // Only previous end + 1 is accepted, and the install is far below it.
        for valid_from in [10_000, 10_002] {
            let before = ledger.clone();
            assert!(execute_fastpay_recovery_governance_update_v1(
                &mut ledger,
                &recovery_governance_update(
                    policy.clone(),
                    committee(2, valid_from, valid_from + 9_999)
                ),
                100,
            )
            .is_err());
            assert_eq!(ledger, before);
        }
        assert_eq!(
            execute_fastpay_recovery_governance_update_v1(
                &mut ledger,
                &recovery_governance_update(policy.clone(), epoch2.clone()),
                100,
            ),
            Ok(FastPayRecoveryGovernanceOutcomeV1::CommitteeRotated)
        );
        let admitting = |height: u64| {
            ledger
                .fastpay_recovery_committees
                .iter()
                .filter(|c| c.valid_from_height <= height && height <= c.new_orders_through_height)
                .map(|c| c.committee_epoch)
                .collect::<Vec<_>>()
        };
        assert_eq!(admitting(100), vec![1]);
        assert_eq!(admitting(10_000), vec![1]);
        assert_eq!(admitting(10_001), vec![2]);

        let epoch2_domain = epoch2.certificate_domain();
        let (certificate, validator_pks) =
            signed_certificate_with_window("epoch2-input", epoch2_domain.clone(), 2, 10_001);
        let context = FastPayRecoveryVerificationContext {
            validator_public_keys: &validator_pks,
            expected_domain: &epoch2_domain,
            committee_epoch: 2,
            policy: &policy,
            quorum: epoch2.quorum,
        };
        ledger.owned_objects.push(postfiat_types::OwnedObject {
            id: "epoch2-input".to_string(),
            version: 1,
            owner_pubkey_hex: certificate.owner_pubkey_hex.clone(),
            value: 100,
            asset: "PFT".to_string(),
        });
        let signed = postfiat_types::SignedOwnedTransferOrderV3 {
            order: certificate.order.clone(),
            owner_pubkey_hex: certificate.owner_pubkey_hex.clone(),
            owner_signature_hex: certificate.owner_signature_hex.clone(),
        };
        // One block earlier: refused by admission and by apply, ledger unchanged.
        assert_eq!(
            validate_owned_transfer_v3_admission(
                &ledger,
                &signed,
                &epoch2_domain,
                2,
                &policy,
                10_000
            ),
            Err(OwnedTransferError::NotYetValid)
        );
        let before = ledger.clone();
        assert_eq!(
            apply_owned_transfer_certificate_v3(&mut ledger, &certificate, context, 10_000),
            Err(OwnedTransferError::NotYetValid)
        );
        assert_eq!(ledger, before);
        // An epoch-2 order dated 10000 is outside epoch 2's admission window,
        // and an epoch-1 order dated 10001 is past epoch 1's.
        let (early, _) =
            signed_certificate_with_window("early-input", epoch2_domain.clone(), 2, 10_000);
        let (late, _) =
            signed_certificate_with_window("late-input", epoch1.certificate_domain(), 1, 10_001);
        for (certificate, committee) in [(&early, &epoch2), (&late, &epoch1)] {
            let recovery = &certificate.order.recovery;
            assert_eq!(recovery.committee_epoch, committee.committee_epoch);
            let reveal = postfiat_types::FastLanePrimaryTransactionV1 {
                operation: postfiat_types::FastLanePrimaryOperationV1::FastPayRecoveryDecision {
                    request: recovery_request(certificate, recovery.recovery_closes_at_height),
                },
            };
            let mut scratch = ledger.clone();
            let receipt = execute_fastlane_primary_transaction(
                &mut scratch,
                &reveal,
                recovery.recovery_closes_at_height,
            );
            assert!(!receipt.accepted, "{receipt:?}");
            assert_eq!(
                receipt.message,
                "FastPay order is outside its committee domain or admission window"
            );
            assert_eq!(scratch, ledger);
        }
        // Exactly previous end + 1: admitted and applied.
        validate_owned_transfer_v3_admission(
            &ledger,
            &signed,
            &epoch2_domain,
            2,
            &policy,
            10_001,
        )
        .expect("admitted at 10001");
        let outcome =
            apply_owned_transfer_certificate_v3(&mut ledger, &certificate, context, 10_001)
                .expect("epoch-2 payment applies at 10001");
        assert_eq!(outcome.consumed, 1);
    }

    #[test]
    fn fence_status_context_and_encoding_are_distinct_from_every_fastpay_message() {
        use postfiat_types::{
            FASTLANE_CHECKPOINT_CONTEXT_V1, FASTLANE_DEPOSIT_CONTEXT_V1, FASTLANE_EXIT_CONTEXT_V1,
            FASTLANE_EXIT_VOTE_CONTEXT_V1, FASTPAY_FENCE_STATUS_CONTEXT_V2,
            FASTPAY_FENCE_STATUS_SIGNING_TAG_V2, FASTSWAP_INTENT_CONTEXT_V1,
            FASTSWAP_VOTE_CONTEXT_V1, OWNED_DEPOSIT_CONTEXT_V1,
        };
        // Every ML-DSA context a FastPay or owned-object message is signed
        // under: v1 prototype orders, v2 and v3 votes and owner signatures
        // (a certificate is a set of these votes), apply acknowledgements,
        // deposits and the FastSwap/FastLane messages beside them.
        let existing: [&[u8]; 13] = [
            b"postfiat-l1-v2/owned-transfer/v1",
            OWNED_TRANSFER_CONTEXT,
            OWNED_UNWRAP_CONTEXT,
            OWNED_TRANSFER_CONTEXT_V3,
            OWNED_UNWRAP_CONTEXT_V3,
            FASTPAY_APPLY_ACK_CONTEXT_V1,
            OWNED_DEPOSIT_CONTEXT_V1,
            FASTSWAP_INTENT_CONTEXT_V1,
            FASTSWAP_VOTE_CONTEXT_V1,
            FASTLANE_DEPOSIT_CONTEXT_V1,
            FASTLANE_CHECKPOINT_CONTEXT_V1,
            FASTLANE_EXIT_CONTEXT_V1,
            FASTLANE_EXIT_VOTE_CONTEXT_V1,
        ];
        for context in existing {
            assert_ne!(context, FASTPAY_FENCE_STATUS_CONTEXT_V2);
            assert!(!context.starts_with(FASTPAY_FENCE_STATUS_CONTEXT_V2));
            assert!(!FASTPAY_FENCE_STATUS_CONTEXT_V2.starts_with(context));
        }
        // The signed bytes' leading tags: none is a prefix of another, so no
        // encoding can be parsed as the fence status or the reverse.
        let tags: [&[u8]; 4] = [
            b"postfiat.owned-transfer.v3\0",
            b"postfiat.owned-unwrap.v3\0",
            b"postfiat.fastpay.apply-ack.v1\0",
            FASTPAY_FENCE_STATUS_SIGNING_TAG_V2,
        ];
        for (index, left) in tags.iter().enumerate() {
            for right in &tags[index + 1..] {
                assert!(!left.starts_with(right) && !right.starts_with(left));
            }
        }
    }

    #[test]
    fn fence_status_quorum_verifies_real_ml_dsa_signatures_under_its_context() {
        let keys = recovery_validator_keys();
        let committee = postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
            "fence-status-ml-dsa".into(),
            "11".repeat(48),
            1,
            1,
            1,
            100,
            keys.iter()
                .map(|(id, key)| (id.clone(), postfiat_crypto_provider::bytes_to_hex(&key.public_key)))
                .collect(),
        )
        .unwrap();
        let lock_id = "22".repeat(48);
        let sign = |index: usize, context: &[u8]| {
            let mut status = postfiat_types::FastPayFenceStatusV2 {
                schema: postfiat_types::FASTPAY_FENCE_STATUS_SCHEMA_V2.into(),
                chain_id: committee.chain_id.clone(),
                genesis_hash: committee.genesis_hash.clone(),
                protocol_version: 1,
                committee_epoch: 1,
                registry_root: committee.registry_root.clone(),
                lock_id: lock_id.clone(),
                decision: Some(postfiat_types::FastPayRecoveryDecisionV1::Cancelled),
                operation: Some(postfiat_types::FastPayOperationKindV1::Unwrap),
                fence_digest: Some("33".repeat(48)),
                anchored_at_or_before_height: Some(4),
                observed_height: 4 + index as u64,
                validator_id: keys[index].0.clone(),
                signature_hex: String::new(),
            };
            status.signature_hex = postfiat_crypto_provider::bytes_to_hex(
                &postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                    &keys[index].1.private_key,
                    &status.signing_bytes().unwrap(),
                    context,
                )
                .unwrap(),
            );
            status
        };
        let statuses = (0..3)
            .map(|index| sign(index, postfiat_types::FASTPAY_FENCE_STATUS_CONTEXT_V2))
            .collect::<Vec<_>>();
        assert_eq!(committee.quorum, 3);
        let accepted =
            verify_fastpay_fence_status_quorum_ml_dsa_v2(&statuses, &committee, &committee.registry_root, &lock_id).unwrap();
        assert_eq!(accepted.anchored_at_or_before_height, Some(4));
        assert!(verify_fastpay_fence_status_v2(
            &statuses[0],
            &committee.validators[0].public_key_hex
        ));
        let mut altered = statuses.clone();
        altered[1].anchored_at_or_before_height = Some(3);
        altered[1].observed_height = 5;
        // A badly signed status counts for nothing and is reported.
        assert_eq!(
            verify_fastpay_fence_status_quorum_ml_dsa_v2(&altered, &committee, &committee.registry_root, &lock_id),
            Err(postfiat_types::FastPayFenceStatusRefusalV2::BelowQuorum { agreeing: 2, quorum: 3 })
        );
        let mut beside = statuses.clone();
        beside.push(altered[1].clone());
        let reported =
            verify_fastpay_fence_status_quorum_ml_dsa_v2(&beside, &committee, &committee.registry_root, &lock_id).unwrap();
        assert_eq!(reported.invalid[0].reason, postfiat_types::FastPayFenceStatusFaultV2::BadSignature);
        // The same bytes signed under another FastPay context do not count.
        let mut cross = statuses;
        cross[2] = sign(2, FASTPAY_APPLY_ACK_CONTEXT_V1);
        assert_eq!(
            verify_fastpay_fence_status_quorum_ml_dsa_v2(&cross, &committee, &committee.registry_root, &lock_id),
            Err(postfiat_types::FastPayFenceStatusRefusalV2::BelowQuorum { agreeing: 2, quorum: 3 })
        );
    }

    /// Six members with real ML-DSA-65 keys (quorum five) and a signer for
    /// their fence statuses on lock `22..`.
    fn ml_dsa_fence_committee() -> (
        postfiat_types::FastPayRecoveryCommitteeV1,
        impl Fn(usize, Option<u64>, u64) -> postfiat_types::FastPayFenceStatusV2,
    ) {
        let keys = (0..6)
            .map(|index| {
                (
                    format!("validator-{index}"),
                    postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[70 + index as u8; 32]),
                )
            })
            .collect::<Vec<_>>();
        let committee = postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
            "fence-status-ml-dsa".into(),
            "11".repeat(48),
            1,
            1,
            1,
            100,
            keys.iter()
                .map(|(id, key)| (id.clone(), postfiat_crypto_provider::bytes_to_hex(&key.public_key)))
                .collect(),
        )
        .unwrap();
        assert_eq!(committee.quorum, 5);
        let lock_id = "22".repeat(48);
        let signer = committee.clone();
        let sign = move |index: usize, bound: Option<u64>, observed: u64| {
            let terminal = bound.is_some();
            let mut status = postfiat_types::FastPayFenceStatusV2 {
                schema: postfiat_types::FASTPAY_FENCE_STATUS_SCHEMA_V2.into(),
                chain_id: signer.chain_id.clone(),
                genesis_hash: signer.genesis_hash.clone(),
                protocol_version: 1,
                committee_epoch: 1,
                registry_root: signer.registry_root.clone(),
                lock_id: lock_id.clone(),
                decision: terminal.then_some(postfiat_types::FastPayRecoveryDecisionV1::Cancelled),
                operation: terminal.then_some(postfiat_types::FastPayOperationKindV1::Unwrap),
                fence_digest: terminal.then(|| "33".repeat(48)),
                anchored_at_or_before_height: bound,
                observed_height: observed,
                validator_id: keys[index].0.clone(),
                signature_hex: String::new(),
            };
            status.signature_hex = postfiat_crypto_provider::bytes_to_hex(
                &postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                    &keys[index].1.private_key,
                    &status.signing_bytes().unwrap(),
                    postfiat_types::FASTPAY_FENCE_STATUS_CONTEXT_V2,
                )
                .unwrap(),
            );
            status
        };
        (committee, sign)
    }

    #[test]
    fn fence_status_false_low_bound_cannot_turn_honest_history_into_a_veto() {
        // R3-N1 with real ML-DSA-65 signatures: six members, quorum five.
        // Members 0-4 said "not terminal" at 6, then terminal by 7; member 5
        // signs the same answer with a false bound 3.
        let (committee, sign) = ml_dsa_fence_committee();
        let lock_id = "22".repeat(48);
        let current = (0..5).map(|index| sign(index, Some(7), 9)).collect::<Vec<_>>();
        let historical = (0..5).map(|index| sign(index, None, 6)).collect::<Vec<_>>();
        let liar = sign(5, Some(3), 9);
        let verify = |statuses: &[postfiat_types::FastPayFenceStatusV2]| {
            verify_fastpay_fence_status_quorum_ml_dsa_v2(statuses, &committee, &committee.registry_root, &lock_id)
        };
        let forward = [current.clone(), historical.clone(), vec![liar.clone()]].concat();
        let reverse = forward.iter().rev().cloned().collect::<Vec<_>>();
        for statuses in [forward, reverse] {
            let accepted = verify(&statuses).expect("the valid terminal quorum stands");
            assert_eq!(accepted.signers.len(), 6);
            assert_eq!(accepted.anchored_at_or_before_height, Some(7));
            assert!(accepted.contradicting.is_empty());
            assert!(accepted.invalid.is_empty());
            assert_eq!(accepted.inconsistent_bounds, vec![liar.clone()]);
        }
        // Control: the true bound 7 lists nothing.
        let control = verify(&[current.clone(), historical, vec![sign(5, Some(7), 9)]].concat()).unwrap();
        assert!(control.contradicting.is_empty() && control.inconsistent_bounds.is_empty());
        // "Not terminal" at the quorum-supported bound is still a contradiction.
        let late = sign(5, None, 7);
        let accepted = verify(&[current, vec![late.clone()]].concat()).unwrap();
        assert_eq!(accepted.contradicting, vec![late]);
    }

    #[test]
    fn fence_status_false_high_observed_height_cannot_blame_honest_terminal_statuses() {
        // The other direction with real ML-DSA-65 signatures: members 0-4 said
        // "not terminal" at 6, members 0-3 then terminal by 10 (short of the
        // quorum); member 5 signs "not terminal" at a false height 20.
        let (committee, sign) = ml_dsa_fence_committee();
        let lock_id = "22".repeat(48);
        let historical = (0..5).map(|index| sign(index, None, 6)).collect::<Vec<_>>();
        let current = (0..4).map(|index| sign(index, Some(10), 20)).collect::<Vec<_>>();
        let liar = sign(5, None, 20);
        let verify = |statuses: &[postfiat_types::FastPayFenceStatusV2]| {
            verify_fastpay_fence_status_quorum_ml_dsa_v2(statuses, &committee, &committee.registry_root, &lock_id)
        };
        let forward = [historical.clone(), vec![liar.clone()], current.clone()].concat();
        let reverse = forward.iter().rev().cloned().collect::<Vec<_>>();
        for statuses in [forward, reverse] {
            let accepted = verify(&statuses).expect("the valid not-terminal quorum stands");
            assert_eq!(accepted.decision, None);
            assert_eq!(accepted.signers.len(), 6);
            assert!(accepted.contradicting.is_empty());
            assert!(accepted.invalid.is_empty());
            assert_eq!(accepted.inconsistent_bounds, vec![liar.clone()]);
        }
        // Control: the true height 6 lists nothing.
        let control = verify(&[historical.clone(), vec![sign(5, None, 6)], current.clone()].concat()).unwrap();
        assert!(control.contradicting.is_empty() && control.inconsistent_bounds.is_empty());
        // A terminal status at the quorum-supported height is still a
        // contradiction (three honest terminal statuses keep it short).
        let early = sign(5, Some(6), 20);
        let accepted = verify(&[historical, vec![early.clone()], current[..3].to_vec()].concat()).unwrap();
        assert_eq!(accepted.contradicting, vec![early]);
    }
}
