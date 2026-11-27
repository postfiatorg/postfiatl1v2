use postfiat_crypto_provider::{
    address_from_public_key, bip340_verify, hex_to_bytes, ml_dsa_65_verify_with_context,
    secp256k1_ecdsa_verify_low_s_der,
};
use postfiat_types::{
    identity_controller_link_message, identity_handle_attestation_message, identity_handle_key,
    identity_handle_reassign_cancel_message, identity_handle_reassign_message,
    identity_handle_reservation_commitment, identity_link_id, identity_nostr_proof_message,
    identity_old_chain_link_message, identity_reassignment_id, identity_rotate_account_message,
    identity_unlink_self_message, identity_validator_link_message, Genesis, HandleAttestationV1,
    HandleNamespaceRegistrarV1, HandleReservationV1, IdentityHandleReassignCancelOperation,
    IdentityHandleReassignExecuteOperation, IdentityHandleReassignProposeOperation,
    IdentityHandleReserveOperation, IdentityHistoryEventV1, IdentityLinkKind, IdentityLinkStatus,
    IdentityLinkV1, IdentityRecordV1, IdentityRegistrarRegisterOperation, IdentityRotateMode,
    IdentitySetActionV1, IdentitySetOperation, IdentityStatus, LedgerState,
    PendingHandleReassignmentV1, RegistrarCosignatureV1, SignedAssetTransaction,
    ValidatorIdentityBindingV1, IDENTITY_CONTROLLER_LINK_DOMAIN,
    IDENTITY_HANDLE_ATTESTATION_DOMAIN, IDENTITY_HANDLE_HOLDBACK_BLOCKS,
    IDENTITY_HANDLE_REASSIGN_DOMAIN, IDENTITY_MAX_LINKS, IDENTITY_ROTATE_ACCOUNT_DOMAIN,
    IDENTITY_UNLINK_SELF_DOMAIN, IDENTITY_VALIDATOR_COOLDOWN_BLOCKS,
    IDENTITY_VALIDATOR_LINK_DOMAIN,
};

fn append_identity_history(
    ledger: &mut LedgerState,
    account: &str,
    sequence: u64,
    block_height: u64,
    action: &str,
    detail: String,
) {
    ledger
        .identity_history_by_account
        .entry(account.to_string())
        .or_default()
        .push(IdentityHistoryEventV1 {
            sequence,
            block_height,
            action: action.to_string(),
            detail,
        });
}

fn verify_mldsa_account_signature(
    expected_account: &str,
    pubkey_hex: &str,
    sig_hex: &str,
    message: &[u8],
    context: &[u8],
) -> bool {
    let Ok(pk) = hex_to_bytes(pubkey_hex) else {
        return false;
    };
    let Ok(sig) = hex_to_bytes(sig_hex) else {
        return false;
    };
    if address_from_public_key(&pk) != expected_account {
        return false;
    }
    ml_dsa_65_verify_with_context(&pk, message, &sig, context)
}

fn verify_mldsa_key_signature(
    pubkey_hex: &str,
    sig_hex: &str,
    message: &[u8],
    context: &[u8],
) -> bool {
    let Ok(pk) = hex_to_bytes(pubkey_hex) else {
        return false;
    };
    let Ok(sig) = hex_to_bytes(sig_hex) else {
        return false;
    };
    ml_dsa_65_verify_with_context(&pk, message, &sig, context)
}

fn verify_cold_registrar_quorum(
    registrar: &HandleNamespaceRegistrarV1,
    approvals: &[RegistrarCosignatureV1],
    message: &[u8],
) -> Result<(), (&'static str, String)> {
    let mut verified_signers = std::collections::BTreeSet::new();
    for approval in approvals {
        if !registrar.cold_signers.contains(&approval.signer_account) {
            continue;
        }
        if verify_mldsa_account_signature(
            &approval.signer_account,
            &approval.public_key_hex,
            &approval.signature_hex,
            message,
            IDENTITY_HANDLE_REASSIGN_DOMAIN.as_bytes(),
        ) {
            verified_signers.insert(approval.signer_account.as_str());
        }
    }
    if verified_signers.len() < 2 {
        return Err((
            "identity_reassign_insufficient_quorum",
            format!(
                "handle reassignment requires >= 2 valid cold registrar approvals; got {}",
                verified_signers.len()
            ),
        ));
    }
    Ok(())
}

fn verify_and_claim_handle(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    account: &str,
    nostr_pubkey: &str,
    att: &HandleAttestationV1,
    reservation_salt_hex: Option<&str>,
    block_height: u64,
) -> Result<(String, String), (&'static str, String)> {
    postfiat_types::validate_identity_namespace(&att.namespace)
        .map_err(|err| ("identity_invalid_handle", err))?;
    postfiat_types::validate_identity_handle(&att.handle)
        .map_err(|err| ("identity_invalid_handle", err))?;
    if att.account != account || block_height > att.expires_at_height {
        return Err((
            "identity_invalid_registrar_attestation",
            "handle attestation account mismatch or expired".to_string(),
        ));
    }
    let registrar = ledger
        .handle_namespace_registrars
        .get(&att.namespace)
        .cloned()
        .ok_or_else(|| {
            (
                "identity_invalid_registrar_attestation",
                format!("unknown handle registrar namespace `{}`", att.namespace),
            )
        })?;
    if !registrar.active || att.registrar_account != registrar.hot_attestation_account {
        return Err((
            "identity_invalid_registrar_attestation",
            "handle attestation signer is not the active hot registrar account".to_string(),
        ));
    }
    let msg = identity_handle_attestation_message(
        &genesis.chain_id,
        &att.namespace,
        &att.handle,
        &att.account,
        att.expires_at_height,
    );
    if !verify_mldsa_account_signature(
        &att.registrar_account,
        &att.registrar_pubkey_hex,
        &att.registrar_signature_hex,
        &msg,
        IDENTITY_HANDLE_ATTESTATION_DOMAIN.as_bytes(),
    ) {
        return Err((
            "identity_invalid_registrar_attestation",
            "handle attestation ML-DSA-65 signature verification failed".to_string(),
        ));
    }
    let key = identity_handle_key(&att.namespace, &att.handle);
    if ledger.identity_account_by_handle.contains_key(&key) {
        return Err((
            "identity_handle_already_claimed",
            format!(
                "handle `{}@{}` is already claimed",
                att.handle, att.namespace
            ),
        ));
    }
    if let Some(&holdback_until) = ledger.retired_handle_holdback.get(&key) {
        if block_height < holdback_until {
            return Err((
                "identity_handle_in_holdback",
                format!(
                    "handle `{}@{}` is in holdback until height {holdback_until}",
                    att.handle, att.namespace
                ),
            ));
        }
        ledger.retired_handle_holdback.remove(&key);
    }
    if let Some(reservation) = ledger.handle_reservations.get(&key).cloned() {
        if block_height <= reservation.expires_at_height {
            let salt_hex = reservation_salt_hex.ok_or_else(|| {
                (
                    "identity_handle_reservation_mismatch",
                    "salted handle reservation requires reservation_salt_hex".to_string(),
                )
            })?;
            let salt_bytes = hex_to_bytes(salt_hex).map_err(|_| {
                (
                    "identity_handle_reservation_mismatch",
                    "reservation_salt_hex is not valid hex".to_string(),
                )
            })?;
            let computed =
                identity_handle_reservation_commitment(&att.handle, nostr_pubkey, &salt_bytes);
            if computed != reservation.commitment_hash {
                return Err((
                    "identity_handle_reservation_mismatch",
                    "handle reservation commitment does not match supplied salt and nostr_pubkey"
                        .to_string(),
                ));
            }
        }
        ledger.handle_reservations.remove(&key);
    }
    ledger
        .identity_account_by_handle
        .insert(key, account.to_string());
    Ok((att.namespace.clone(), att.handle.clone()))
}

pub(crate) fn apply_identity_set(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    operation: &IdentitySetOperation,
    block_height: u64,
    compatibility: crate::AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.hive_identity_v1_active(block_height) {
        return Err((
            "hive_identity_not_active",
            format!("identity_set is not active at block height {block_height}"),
        ));
    }
    let classical_disabled = compatibility.hive_identity_classical_proof_disabled(block_height);
    let tx_seq = transaction.unsigned.sequence;

    match &operation.payload {
        IdentitySetActionV1::Mint {
            nostr_pubkey,
            nostr_proof_sig_hex,
            derivation_version,
            key_index,
            identity_kind,
            handle_attestation,
            reservation_salt_hex,
            relays,
            avatar_nft_id,
            message_key,
            pq_receive_address,
        } => {
            if classical_disabled {
                return Err((
                    "identity_classical_proof_disabled",
                    "classical BIP-340 proofs are disabled by governance kill-switch".to_string(),
                ));
            }
            if let Some(existing) = ledger.identities_by_account.get(&operation.account) {
                if existing.status == IdentityStatus::Active {
                    return Err((
                        "identity_already_exists",
                        format!(
                            "account `{}` already has an active identity",
                            operation.account
                        ),
                    ));
                }
            }
            if ledger.compromised_nostr_pubkeys.contains(nostr_pubkey) {
                return Err((
                    "identity_nostr_pubkey_compromised",
                    "nostr_pubkey is permanently tombstoned as compromised".to_string(),
                ));
            }
            if ledger.identity_account_by_nostr.contains_key(nostr_pubkey) {
                return Err((
                    "identity_nostr_pubkey_taken",
                    "nostr_pubkey is already bound to another active identity".to_string(),
                ));
            }
            let pk_bytes = hex_to_bytes(nostr_pubkey).map_err(|_| {
                (
                    "identity_invalid_bip340_proof",
                    "invalid nostr pubkey hex".into(),
                )
            })?;
            let sig_bytes = hex_to_bytes(nostr_proof_sig_hex).map_err(|_| {
                (
                    "identity_invalid_bip340_proof",
                    "invalid bip340 sig hex".into(),
                )
            })?;
            let msg =
                identity_nostr_proof_message("mint", &genesis.chain_id, &operation.account, tx_seq);
            if !bip340_verify(&pk_bytes, &msg, &sig_bytes) {
                return Err((
                    "identity_invalid_bip340_proof",
                    "BIP-340 Schnorr proof verification failed for identity mint".to_string(),
                ));
            }
            let (registered_namespace, handle) = if let Some(att) = handle_attestation {
                let (ns, h) = verify_and_claim_handle(
                    genesis,
                    ledger,
                    &operation.account,
                    nostr_pubkey,
                    att,
                    reservation_salt_hex.as_deref(),
                    block_height,
                )?;
                (Some(ns), Some(h))
            } else {
                (None, None)
            };
            let next_seq = ledger
                .identities_by_account
                .get(&operation.account)
                .map_or(1, |prev| prev.sequence.saturating_add(1));
            let record = IdentityRecordV1 {
                account: operation.account.clone(),
                nostr_pubkey: nostr_pubkey.clone(),
                derivation_version: *derivation_version,
                key_index: *key_index,
                identity_kind: *identity_kind,
                sequence: next_seq,
                status: IdentityStatus::Active,
                registered_namespace: registered_namespace.clone(),
                handle: handle.clone(),
                relays: relays.clone(),
                avatar_nft_id: avatar_nft_id.clone(),
                message_key: message_key.clone(),
                pq_receive_address: pq_receive_address.clone(),
                links: Vec::new(),
                created_at_height: block_height,
                updated_at_height: block_height,
                revoked_at_height: None,
            };
            record
                .validate()
                .map_err(|err| ("bad_identity_record", err))?;
            ledger
                .identities_by_account
                .insert(operation.account.clone(), record);
            ledger
                .identity_account_by_nostr
                .insert(nostr_pubkey.clone(), operation.account.clone());
            let detail = match (handle, registered_namespace) {
                (Some(h), Some(ns)) => format!("minted npub={nostr_pubkey} handle={h}@{ns}"),
                _ => format!("minted npub={nostr_pubkey}"),
            };
            append_identity_history(
                ledger,
                &operation.account,
                next_seq,
                block_height,
                "mint",
                detail,
            );
            Ok(())
        }
        IdentitySetActionV1::Update {
            relays,
            avatar_nft_id,
            message_key,
            pq_receive_address,
            handle_attestation,
            reservation_salt_hex,
        } => {
            let existing = ledger
                .identities_by_account
                .get(&operation.account)
                .cloned()
                .ok_or_else(|| {
                    (
                        "identity_not_found",
                        format!("no identity found for account `{}`", operation.account),
                    )
                })?;
            if existing.status == IdentityStatus::Revoked {
                return Err((
                    "identity_revoked",
                    "cannot update a revoked identity".to_string(),
                ));
            }
            let mut updated = existing.clone();
            if let Some(new_relays) = relays {
                updated.relays = new_relays.clone();
            }
            if let Some(new_avatar) = avatar_nft_id {
                updated.avatar_nft_id = new_avatar.clone();
            }
            if let Some(new_mk) = message_key {
                updated.message_key = new_mk.clone();
            }
            if let Some(new_pq) = pq_receive_address {
                updated.pq_receive_address = new_pq.clone();
            }
            if let Some(att) = handle_attestation {
                if let (Some(old_ns), Some(old_h)) =
                    (&updated.registered_namespace, &updated.handle)
                {
                    let old_key = identity_handle_key(old_ns, old_h);
                    ledger.identity_account_by_handle.remove(&old_key);
                    ledger.retired_handle_holdback.insert(
                        old_key,
                        block_height.saturating_add(IDENTITY_HANDLE_HOLDBACK_BLOCKS),
                    );
                }
                let (ns, h) = verify_and_claim_handle(
                    genesis,
                    ledger,
                    &operation.account,
                    &updated.nostr_pubkey,
                    att,
                    reservation_salt_hex.as_deref(),
                    block_height,
                )?;
                updated.registered_namespace = Some(ns);
                updated.handle = Some(h);
            }
            updated.sequence = updated.sequence.saturating_add(1);
            updated.updated_at_height = block_height;
            updated
                .validate()
                .map_err(|err| ("bad_identity_record", err))?;
            let seq = updated.sequence;
            ledger
                .identities_by_account
                .insert(operation.account.clone(), updated);
            append_identity_history(
                ledger,
                &operation.account,
                seq,
                block_height,
                "update",
                "updated profile fields".to_string(),
            );
            Ok(())
        }
        IdentitySetActionV1::Link {
            kind,
            target,
            proof_pubkey_hex,
            proof_sig_hex,
            metadata_hash,
        } => {
            let mut identity = ledger
                .identities_by_account
                .get(&operation.account)
                .cloned()
                .ok_or_else(|| {
                    (
                        "identity_not_found",
                        format!("no identity found for account `{}`", operation.account),
                    )
                })?;
            if identity.status == IdentityStatus::Revoked {
                return Err((
                    "identity_revoked",
                    "cannot add links to a revoked identity".to_string(),
                ));
            }
            let pending_idx = identity.links.iter().position(|l| {
                l.kind == *kind
                    && l.target == *target
                    && l.status == IdentityLinkStatus::PendingReconfirmation
            });
            if pending_idx.is_none() && identity.links.len() >= IDENTITY_MAX_LINKS {
                return Err((
                    "identity_link_limit_exceeded",
                    format!("identity links limit ({IDENTITY_MAX_LINKS}) reached"),
                ));
            }
            match kind {
                IdentityLinkKind::ValidatorNode => {
                    let (pk, sig) = proof_pubkey_hex
                        .as_deref()
                        .zip(proof_sig_hex.as_deref())
                        .ok_or_else(|| {
                            (
                                "identity_invalid_link_proof",
                                "validator link requires proof_pubkey_hex and proof_sig_hex"
                                    .to_string(),
                            )
                        })?;
                    let msg = identity_validator_link_message(
                        &genesis.chain_id,
                        &operation.account,
                        target,
                        tx_seq,
                    );
                    if !verify_mldsa_key_signature(
                        pk,
                        sig,
                        &msg,
                        IDENTITY_VALIDATOR_LINK_DOMAIN.as_bytes(),
                    ) {
                        return Err((
                            "identity_invalid_link_proof",
                            "validator node ML-DSA-65 link proof failed".to_string(),
                        ));
                    }
                    if let Some(binding) = ledger.identity_validator_bindings.get(target) {
                        if let Some(bound_acct) = &binding.account {
                            if bound_acct != &operation.account {
                                return Err((
                                    "identity_validator_already_linked",
                                    format!(
                                        "validator `{target}` is already linked to another account"
                                    ),
                                ));
                            }
                        } else if let Some(unbound_at) = binding.unbound_at_height {
                            let cooldown_end =
                                unbound_at.saturating_add(IDENTITY_VALIDATOR_COOLDOWN_BLOCKS);
                            if block_height < cooldown_end {
                                return Err((
                                    "identity_validator_cooldown_active",
                                    format!(
                                        "validator `{target}` is in 30-day cooldown until height {cooldown_end}"
                                    ),
                                ));
                            }
                        }
                    }
                    ledger.identity_validator_bindings.insert(
                        target.clone(),
                        ValidatorIdentityBindingV1 {
                            node_id: target.clone(),
                            account: Some(operation.account.clone()),
                            bound_at_height: block_height,
                            unbound_at_height: None,
                        },
                    );
                }
                IdentityLinkKind::OldChainAccount => {
                    if classical_disabled {
                        return Err((
                            "identity_classical_proof_disabled",
                            "old-chain secp256k1 ECDSA links are disabled by classical kill-switch"
                                .to_string(),
                        ));
                    }
                    if let Some(bound_acct) = ledger.identity_account_by_old_chain.get(target) {
                        if bound_acct != &operation.account {
                            return Err((
                                "identity_old_chain_already_linked",
                                format!(
                                    "old-chain account `{target}` is already linked to another identity"
                                ),
                            ));
                        }
                    }
                    let (pk_hex, sig_hex) = proof_pubkey_hex
                        .as_deref()
                        .zip(proof_sig_hex.as_deref())
                        .ok_or_else(|| {
                            (
                                "identity_invalid_link_proof",
                                "old-chain link requires proof_pubkey_hex and proof_sig_hex"
                                    .to_string(),
                            )
                        })?;
                    let pk_bytes = hex_to_bytes(pk_hex).map_err(|_| {
                        (
                            "identity_invalid_link_proof",
                            "invalid secp256k1 pubkey hex".into(),
                        )
                    })?;
                    let sig_bytes = hex_to_bytes(sig_hex).map_err(|_| {
                        ("identity_invalid_link_proof", "invalid DER sig hex".into())
                    })?;
                    let msg = identity_old_chain_link_message(
                        &genesis.chain_id,
                        &operation.account,
                        target,
                        tx_seq,
                    );
                    if !secp256k1_ecdsa_verify_low_s_der(&pk_bytes, &msg, &sig_bytes) {
                        return Err((
                            "identity_invalid_link_proof",
                            "old-chain strict low-S DER secp256k1 ECDSA proof failed".to_string(),
                        ));
                    }
                    ledger
                        .identity_account_by_old_chain
                        .insert(target.clone(), operation.account.clone());
                }
                IdentityLinkKind::AgentController => {
                    let (pk, sig) = proof_pubkey_hex
                        .as_deref()
                        .zip(proof_sig_hex.as_deref())
                        .ok_or_else(|| {
                            (
                                "identity_invalid_link_proof",
                                "agent controller link requires proof_pubkey_hex and proof_sig_hex"
                                    .to_string(),
                            )
                        })?;
                    let msg = identity_controller_link_message(
                        &genesis.chain_id,
                        &operation.account,
                        target,
                        tx_seq,
                    );
                    if !verify_mldsa_account_signature(
                        target,
                        pk,
                        sig,
                        &msg,
                        IDENTITY_CONTROLLER_LINK_DOMAIN.as_bytes(),
                    ) {
                        return Err((
                            "identity_invalid_link_proof",
                            "agent controller ML-DSA-65 co-signature failed".to_string(),
                        ));
                    }
                }
                IdentityLinkKind::CustomNip05
                | IdentityLinkKind::EvmAddress
                | IdentityLinkKind::SolanaAddress => {}
            }
            identity.sequence = identity.sequence.saturating_add(1);
            identity.updated_at_height = block_height;
            if let Some(idx) = pending_idx {
                identity.links[idx].status = IdentityLinkStatus::Active;
                identity.links[idx].linked_at_height = block_height;
                identity.links[idx].metadata_hash = metadata_hash.clone();
            } else {
                let link_id =
                    identity_link_id(&operation.account, *kind, target, identity.sequence);
                identity.links.push(IdentityLinkV1 {
                    link_id,
                    kind: *kind,
                    target: target.clone(),
                    consensus_verified: kind.is_consensus_verified(),
                    status: IdentityLinkStatus::Active,
                    linked_at_height: block_height,
                    metadata_hash: metadata_hash.clone(),
                });
            }
            identity
                .validate()
                .map_err(|err| ("bad_identity_record", err))?;
            let seq = identity.sequence;
            ledger
                .identities_by_account
                .insert(operation.account.clone(), identity);
            append_identity_history(
                ledger,
                &operation.account,
                seq,
                block_height,
                "link",
                format!("linked {}:{}", kind.as_str(), target),
            );
            Ok(())
        }
        IdentitySetActionV1::Unlink {
            link_id,
            self_unlink_pubkey_hex,
            self_unlink_sig_hex,
        } => {
            let mut identity = ledger
                .identities_by_account
                .get(&operation.account)
                .cloned()
                .ok_or_else(|| {
                    (
                        "identity_not_found",
                        format!("no identity found for account `{}`", operation.account),
                    )
                })?;
            let link_idx = identity
                .links
                .iter()
                .position(|l| &l.link_id == link_id)
                .ok_or_else(|| {
                    (
                        "identity_link_not_found",
                        format!("link `{link_id}` not found on identity"),
                    )
                })?;
            let removed = identity.links[link_idx].clone();
            if transaction.unsigned.source != operation.account {
                let (pk, sig) = self_unlink_pubkey_hex
                    .as_deref()
                    .zip(self_unlink_sig_hex.as_deref())
                    .ok_or_else(|| {
                        (
                            "identity_invalid_link_proof",
                            "self-unlink requires self_unlink_pubkey_hex and self_unlink_sig_hex"
                                .to_string(),
                        )
                    })?;
                let msg = identity_unlink_self_message(&genesis.chain_id, link_id);
                match removed.kind {
                    IdentityLinkKind::ValidatorNode => {
                        if !verify_mldsa_key_signature(
                            pk,
                            sig,
                            &msg,
                            IDENTITY_UNLINK_SELF_DOMAIN.as_bytes(),
                        ) {
                            return Err((
                                "identity_invalid_link_proof",
                                "validator self-unlink ML-DSA-65 proof failed".to_string(),
                            ));
                        }
                    }
                    IdentityLinkKind::AgentController => {
                        if !verify_mldsa_account_signature(
                            &removed.target,
                            pk,
                            sig,
                            &msg,
                            IDENTITY_UNLINK_SELF_DOMAIN.as_bytes(),
                        ) {
                            return Err((
                                "identity_invalid_link_proof",
                                "agent controller self-unlink ML-DSA-65 proof failed".to_string(),
                            ));
                        }
                    }
                    IdentityLinkKind::OldChainAccount => {
                        if classical_disabled {
                            return Err((
                                "identity_classical_proof_disabled",
                                "old-chain self-unlink disabled by classical kill-switch"
                                    .to_string(),
                            ));
                        }
                        let pk_bytes = hex_to_bytes(pk).map_err(|_| {
                            (
                                "identity_invalid_link_proof",
                                "invalid secp256k1 pubkey hex".into(),
                            )
                        })?;
                        let sig_bytes = hex_to_bytes(sig).map_err(|_| {
                            ("identity_invalid_link_proof", "invalid DER sig hex".into())
                        })?;
                        if !secp256k1_ecdsa_verify_low_s_der(&pk_bytes, &msg, &sig_bytes) {
                            return Err((
                                "identity_invalid_link_proof",
                                "old-chain self-unlink ECDSA proof failed".to_string(),
                            ));
                        }
                    }
                    _ => {
                        return Err((
                            "identity_invalid_link_proof",
                            "only the identity owner may unlink client-verified links".to_string(),
                        ));
                    }
                }
            }
            identity.links.remove(link_idx);
            match removed.kind {
                IdentityLinkKind::ValidatorNode => {
                    if let Some(binding) =
                        ledger.identity_validator_bindings.get_mut(&removed.target)
                    {
                        binding.account = None;
                        binding.unbound_at_height = Some(block_height);
                    }
                }
                IdentityLinkKind::OldChainAccount => {
                    ledger.identity_account_by_old_chain.remove(&removed.target);
                }
                _ => {}
            }
            identity.sequence = identity.sequence.saturating_add(1);
            identity.updated_at_height = block_height;
            let seq = identity.sequence;
            ledger
                .identities_by_account
                .insert(operation.account.clone(), identity);
            append_identity_history(
                ledger,
                &operation.account,
                seq,
                block_height,
                "unlink",
                format!("unlinked {}:{}", removed.kind.as_str(), removed.target),
            );
            Ok(())
        }
        IdentitySetActionV1::Rotate {
            mode,
            new_nostr_pubkey,
            new_nostr_proof_sig_hex,
            derivation_version,
            key_index,
        } => {
            if classical_disabled {
                return Err((
                    "identity_classical_proof_disabled",
                    "classical BIP-340 proofs are disabled by governance kill-switch".to_string(),
                ));
            }
            let mut identity = ledger
                .identities_by_account
                .get(&operation.account)
                .cloned()
                .ok_or_else(|| {
                    (
                        "identity_not_found",
                        format!("no identity found for account `{}`", operation.account),
                    )
                })?;
            if identity.status == IdentityStatus::Revoked {
                return Err((
                    "identity_revoked",
                    "cannot rotate a revoked identity".to_string(),
                ));
            }
            if ledger.compromised_nostr_pubkeys.contains(new_nostr_pubkey) {
                return Err((
                    "identity_nostr_pubkey_compromised",
                    "new_nostr_pubkey is permanently tombstoned as compromised".to_string(),
                ));
            }
            if new_nostr_pubkey != &identity.nostr_pubkey
                && ledger
                    .identity_account_by_nostr
                    .contains_key(new_nostr_pubkey)
            {
                return Err((
                    "identity_nostr_pubkey_taken",
                    "new_nostr_pubkey is already bound to another active identity".to_string(),
                ));
            }
            let pk_bytes = hex_to_bytes(new_nostr_pubkey).map_err(|_| {
                (
                    "identity_invalid_bip340_proof",
                    "invalid nostr pubkey hex".into(),
                )
            })?;
            let sig_bytes = hex_to_bytes(new_nostr_proof_sig_hex).map_err(|_| {
                (
                    "identity_invalid_bip340_proof",
                    "invalid bip340 sig hex".into(),
                )
            })?;
            let msg = identity_nostr_proof_message(
                "rotate",
                &genesis.chain_id,
                &operation.account,
                tx_seq,
            );
            if !bip340_verify(&pk_bytes, &msg, &sig_bytes) {
                return Err((
                    "identity_invalid_bip340_proof",
                    "BIP-340 Schnorr proof verification failed for rotate".to_string(),
                ));
            }
            let old_npub = identity.nostr_pubkey.clone();
            ledger.identity_account_by_nostr.remove(&old_npub);
            if *mode == IdentityRotateMode::Compromised {
                ledger.compromised_nostr_pubkeys.insert(old_npub.clone());
            }
            identity.nostr_pubkey = new_nostr_pubkey.clone();
            identity.derivation_version = *derivation_version;
            identity.key_index = *key_index;
            identity.sequence = identity.sequence.saturating_add(1);
            identity.updated_at_height = block_height;
            let seq = identity.sequence;
            ledger
                .identity_account_by_nostr
                .insert(new_nostr_pubkey.clone(), operation.account.clone());
            ledger
                .identities_by_account
                .insert(operation.account.clone(), identity);
            append_identity_history(
                ledger,
                &operation.account,
                seq,
                block_height,
                "rotate",
                format!(
                    "mode={} old_npub={} new_npub={}",
                    mode.as_str(),
                    old_npub,
                    new_nostr_pubkey
                ),
            );
            Ok(())
        }
        IdentitySetActionV1::RotateAccount {
            new_account,
            new_account_pubkey_hex,
            new_account_sig_hex,
        } => {
            let mut identity = ledger
                .identities_by_account
                .get(&operation.account)
                .cloned()
                .ok_or_else(|| {
                    (
                        "identity_not_found",
                        format!("no identity found for account `{}`", operation.account),
                    )
                })?;
            if identity.status == IdentityStatus::Revoked {
                return Err((
                    "identity_revoked",
                    "cannot rotate account on a revoked identity".to_string(),
                ));
            }
            if ledger.identities_by_account.contains_key(new_account) {
                return Err((
                    "identity_rotate_account_cosig_invalid",
                    "destination account already owns an identity".to_string(),
                ));
            }
            let msg = identity_rotate_account_message(
                &genesis.chain_id,
                &operation.account,
                new_account,
                identity.sequence,
            );
            if !verify_mldsa_account_signature(
                new_account,
                new_account_pubkey_hex,
                new_account_sig_hex,
                &msg,
                IDENTITY_ROTATE_ACCOUNT_DOMAIN.as_bytes(),
            ) {
                return Err((
                    "identity_rotate_account_cosig_invalid",
                    "destination account ML-DSA-65 co-signature failed".to_string(),
                ));
            }
            ledger.identities_by_account.remove(&operation.account);
            identity.account = new_account.clone();
            identity.sequence = identity.sequence.saturating_add(1);
            identity.updated_at_height = block_height;
            for link in &mut identity.links {
                if matches!(
                    link.kind,
                    IdentityLinkKind::ValidatorNode | IdentityLinkKind::AgentController
                ) {
                    link.status = IdentityLinkStatus::PendingReconfirmation;
                }
                if link.kind == IdentityLinkKind::ValidatorNode {
                    if let Some(binding) = ledger.identity_validator_bindings.get_mut(&link.target)
                    {
                        binding.account = Some(new_account.clone());
                    }
                }
                if link.kind == IdentityLinkKind::OldChainAccount {
                    ledger
                        .identity_account_by_old_chain
                        .insert(link.target.clone(), new_account.clone());
                }
            }
            ledger
                .identity_account_by_nostr
                .insert(identity.nostr_pubkey.clone(), new_account.clone());
            if let (Some(ns), Some(h)) = (&identity.registered_namespace, &identity.handle) {
                ledger
                    .identity_account_by_handle
                    .insert(identity_handle_key(ns, h), new_account.clone());
            }
            let seq = identity.sequence;
            ledger
                .identities_by_account
                .insert(new_account.clone(), identity);
            let mut history = ledger
                .identity_history_by_account
                .remove(&operation.account)
                .unwrap_or_default();
            history.push(IdentityHistoryEventV1 {
                sequence: seq,
                block_height,
                action: "rotate_account".to_string(),
                detail: format!("rotated from {} to {}", operation.account, new_account),
            });
            ledger
                .identity_history_by_account
                .insert(new_account.clone(), history);
            Ok(())
        }
        IdentitySetActionV1::Revoke { reason_code } => {
            let mut identity = ledger
                .identities_by_account
                .get(&operation.account)
                .cloned()
                .ok_or_else(|| {
                    (
                        "identity_not_found",
                        format!("no identity found for account `{}`", operation.account),
                    )
                })?;
            if identity.status == IdentityStatus::Revoked {
                return Err((
                    "identity_revoked",
                    "identity is already revoked".to_string(),
                ));
            }
            ledger
                .identity_account_by_nostr
                .remove(&identity.nostr_pubkey);
            if let (Some(ns), Some(h)) = (&identity.registered_namespace, &identity.handle) {
                let key = identity_handle_key(ns, h);
                ledger.identity_account_by_handle.remove(&key);
                ledger.retired_handle_holdback.insert(
                    key,
                    block_height.saturating_add(IDENTITY_HANDLE_HOLDBACK_BLOCKS),
                );
            }
            for link in &identity.links {
                match link.kind {
                    IdentityLinkKind::ValidatorNode => {
                        if let Some(binding) =
                            ledger.identity_validator_bindings.get_mut(&link.target)
                        {
                            binding.account = None;
                            binding.unbound_at_height = Some(block_height);
                        }
                    }
                    IdentityLinkKind::OldChainAccount => {
                        ledger.identity_account_by_old_chain.remove(&link.target);
                    }
                    _ => {}
                }
            }
            identity.status = IdentityStatus::Revoked;
            identity.revoked_at_height = Some(block_height);
            identity.sequence = identity.sequence.saturating_add(1);
            identity.updated_at_height = block_height;
            let seq = identity.sequence;
            ledger
                .identities_by_account
                .insert(operation.account.clone(), identity);
            append_identity_history(
                ledger,
                &operation.account,
                seq,
                block_height,
                "revoke",
                format!("reason={reason_code}"),
            );
            Ok(())
        }
    }
}

pub(crate) fn apply_identity_registrar_register(
    ledger: &mut LedgerState,
    operation: &IdentityRegistrarRegisterOperation,
    block_height: u64,
    compatibility: crate::AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.hive_identity_v1_active(block_height) {
        return Err((
            "hive_identity_not_active",
            format!("identity_registrar_register is not active at block height {block_height}"),
        ));
    }
    if ledger
        .handle_namespace_registrars
        .contains_key(&operation.namespace)
    {
        return Err((
            "identity_registrar_already_exists",
            format!(
                "registrar for namespace `{}` already exists",
                operation.namespace
            ),
        ));
    }
    let registrar = HandleNamespaceRegistrarV1 {
        namespace: operation.namespace.clone(),
        hot_attestation_account: operation.hot_attestation_account.clone(),
        cold_signers: operation.cold_signers.clone(),
        reassignment_timelock_blocks: operation.reassignment_timelock_blocks,
        active: true,
        registered_at_height: block_height,
    };
    registrar
        .validate()
        .map_err(|err| ("bad_identity_registrar", err))?;
    ledger
        .handle_namespace_registrars
        .insert(operation.namespace.clone(), registrar);
    Ok(())
}

pub(crate) fn apply_identity_handle_reserve(
    ledger: &mut LedgerState,
    operation: &IdentityHandleReserveOperation,
    block_height: u64,
    compatibility: crate::AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.hive_identity_v1_active(block_height) {
        return Err((
            "hive_identity_not_active",
            format!("identity_handle_reserve is not active at block height {block_height}"),
        ));
    }
    let registrar = ledger
        .handle_namespace_registrars
        .get(&operation.namespace)
        .ok_or_else(|| {
            (
                "identity_invalid_registrar_attestation",
                format!("unknown registrar namespace `{}`", operation.namespace),
            )
        })?;
    if !registrar.active || operation.registrar_account != registrar.hot_attestation_account {
        return Err((
            "identity_invalid_registrar_attestation",
            "handle reservation requires active namespace hot_attestation_account".to_string(),
        ));
    }
    if operation.expires_at_height <= block_height {
        return Err((
            "identity_invalid_registrar_attestation",
            "handle reservation expires_at_height must exceed block_height".to_string(),
        ));
    }
    let key = identity_handle_key(&operation.namespace, &operation.handle);
    if ledger.identity_account_by_handle.contains_key(&key) {
        return Err((
            "identity_handle_already_claimed",
            format!(
                "handle `{}@{}` is already claimed",
                operation.handle, operation.namespace
            ),
        ));
    }
    let reservation = HandleReservationV1 {
        namespace: operation.namespace.clone(),
        handle: operation.handle.clone(),
        commitment_hash: operation.commitment_hash.clone(),
        reserved_at_height: block_height,
        expires_at_height: operation.expires_at_height,
    };
    reservation
        .validate()
        .map_err(|err| ("bad_handle_reservation", err))?;
    ledger.handle_reservations.insert(key, reservation);
    Ok(())
}

pub(crate) fn apply_identity_handle_reassign_propose(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    operation: &IdentityHandleReassignProposeOperation,
    block_height: u64,
    compatibility: crate::AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.hive_identity_v1_active(block_height) {
        return Err((
            "hive_identity_not_active",
            format!(
                "identity_handle_reassign_propose is not active at block height {block_height}"
            ),
        ));
    }
    let registrar = ledger
        .handle_namespace_registrars
        .get(&operation.namespace)
        .cloned()
        .ok_or_else(|| {
            (
                "identity_invalid_registrar_attestation",
                format!("unknown registrar namespace `{}`", operation.namespace),
            )
        })?;
    if !registrar.active || !registrar.cold_signers.contains(&operation.proposer) {
        return Err((
            "identity_reassign_insufficient_quorum",
            "proposer must be an active cold registrar signer".to_string(),
        ));
    }
    let msg = identity_handle_reassign_message(
        &genesis.chain_id,
        &operation.namespace,
        &operation.handle,
        &operation.from_account,
        &operation.to_account,
        &operation.reason_code,
        operation.emergency_compromise,
    );
    verify_cold_registrar_quorum(&registrar, &operation.approvals, &msg)?;
    let key = identity_handle_key(&operation.namespace, &operation.handle);
    if ledger
        .identity_account_by_handle
        .get(&key)
        .map(String::as_str)
        != Some(operation.from_account.as_str())
    {
        return Err((
            "identity_not_found",
            "from_account does not currently hold the specified handle".to_string(),
        ));
    }
    if ledger
        .identities_by_account
        .get(&operation.to_account)
        .is_none_or(|id| id.status != IdentityStatus::Active)
    {
        return Err((
            "identity_not_found",
            "to_account must have an active identity to receive a reassigned handle".to_string(),
        ));
    }
    let reassignment_id = identity_reassignment_id(
        &operation.namespace,
        &operation.handle,
        &operation.from_account,
        &operation.to_account,
        block_height,
        transaction.unsigned.sequence,
    );
    let effective_at_height = if operation.emergency_compromise {
        block_height
    } else {
        block_height.saturating_add(registrar.reassignment_timelock_blocks)
    };
    let pending = PendingHandleReassignmentV1 {
        reassignment_id: reassignment_id.clone(),
        namespace: operation.namespace.clone(),
        handle: operation.handle.clone(),
        from_account: operation.from_account.clone(),
        to_account: operation.to_account.clone(),
        reason_code: operation.reason_code.clone(),
        emergency_compromise: operation.emergency_compromise,
        proposed_at_height: block_height,
        effective_at_height,
        approvals: operation.approvals.clone(),
        state: "pending".to_string(),
    };
    pending
        .validate()
        .map_err(|err| ("bad_handle_reassignment", err))?;
    ledger
        .pending_handle_reassignments
        .insert(reassignment_id, pending);
    Ok(())
}

pub(crate) fn apply_identity_handle_reassign_execute(
    ledger: &mut LedgerState,
    operation: &IdentityHandleReassignExecuteOperation,
    block_height: u64,
    compatibility: crate::AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.hive_identity_v1_active(block_height) {
        return Err((
            "hive_identity_not_active",
            format!(
                "identity_handle_reassign_execute is not active at block height {block_height}"
            ),
        ));
    }
    let pending = ledger
        .pending_handle_reassignments
        .get(&operation.reassignment_id)
        .cloned()
        .ok_or_else(|| {
            (
                "identity_reassign_not_found",
                format!(
                    "handle reassignment `{}` does not exist",
                    operation.reassignment_id
                ),
            )
        })?;
    if pending.state != "pending" {
        return Err((
            "identity_reassign_not_pending",
            format!("handle reassignment is `{}`", pending.state),
        ));
    }
    if block_height < pending.effective_at_height {
        return Err((
            "identity_reassign_timelock_active",
            format!(
                "handle reassignment cannot execute before height {}",
                pending.effective_at_height
            ),
        ));
    }
    let key = identity_handle_key(&pending.namespace, &pending.handle);
    if let Some(from_id) = ledger.identities_by_account.get_mut(&pending.from_account) {
        if from_id.registered_namespace.as_deref() == Some(pending.namespace.as_str())
            && from_id.handle.as_deref() == Some(pending.handle.as_str())
        {
            from_id.registered_namespace = None;
            from_id.handle = None;
            from_id.sequence = from_id.sequence.saturating_add(1);
            from_id.updated_at_height = block_height;
            let seq = from_id.sequence;
            append_identity_history(
                ledger,
                &pending.from_account,
                seq,
                block_height,
                "handle_reassigned_out",
                format!(
                    "handle {}@{} reassigned to {} (reason={})",
                    pending.handle, pending.namespace, pending.to_account, pending.reason_code
                ),
            );
        }
    }
    let to_id = ledger
        .identities_by_account
        .get_mut(&pending.to_account)
        .ok_or_else(|| {
            (
                "identity_not_found",
                "target identity missing during handle reassignment execute".to_string(),
            )
        })?;
    if to_id.status != IdentityStatus::Active {
        return Err(("identity_revoked", "target identity is revoked".to_string()));
    }
    if let (Some(old_ns), Some(old_h)) = (&to_id.registered_namespace, &to_id.handle) {
        let old_key = identity_handle_key(old_ns, old_h);
        ledger.identity_account_by_handle.remove(&old_key);
    }
    to_id.registered_namespace = Some(pending.namespace.clone());
    to_id.handle = Some(pending.handle.clone());
    to_id.sequence = to_id.sequence.saturating_add(1);
    to_id.updated_at_height = block_height;
    let to_seq = to_id.sequence;
    ledger
        .identity_account_by_handle
        .insert(key.clone(), pending.to_account.clone());
    ledger.retired_handle_holdback.remove(&key);
    if let Some(entry) = ledger
        .pending_handle_reassignments
        .get_mut(&operation.reassignment_id)
    {
        entry.state = "executed".to_string();
    }
    append_identity_history(
        ledger,
        &pending.to_account,
        to_seq,
        block_height,
        "handle_reassigned_in",
        format!(
            "handle {}@{} reassigned from {} (reason={})",
            pending.handle, pending.namespace, pending.from_account, pending.reason_code
        ),
    );
    Ok(())
}

pub(crate) fn apply_identity_handle_reassign_cancel(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    operation: &IdentityHandleReassignCancelOperation,
    block_height: u64,
    compatibility: crate::AssetExecutionCompatibility,
) -> Result<(), (&'static str, String)> {
    if !compatibility.hive_identity_v1_active(block_height) {
        return Err((
            "hive_identity_not_active",
            format!("identity_handle_reassign_cancel is not active at block height {block_height}"),
        ));
    }
    let pending = ledger
        .pending_handle_reassignments
        .get(&operation.reassignment_id)
        .cloned()
        .ok_or_else(|| {
            (
                "identity_reassign_not_found",
                format!(
                    "handle reassignment `{}` does not exist",
                    operation.reassignment_id
                ),
            )
        })?;
    if pending.state != "pending" {
        return Err((
            "identity_reassign_not_pending",
            format!("handle reassignment is `{}`", pending.state),
        ));
    }
    if operation.caller == pending.from_account {
        if pending.emergency_compromise {
            return Err((
                "identity_reassign_cannot_cancel_emergency",
                "owner cannot cancel a 2-of-3 emergency compromise handle reassignment".to_string(),
            ));
        }
    } else {
        let registrar = ledger
            .handle_namespace_registrars
            .get(&pending.namespace)
            .cloned()
            .ok_or_else(|| {
                (
                    "identity_invalid_registrar_attestation",
                    "registrar missing for reassignment cancel".to_string(),
                )
            })?;
        let msg =
            identity_handle_reassign_cancel_message(&genesis.chain_id, &operation.reassignment_id);
        verify_cold_registrar_quorum(&registrar, &operation.approvals, &msg)?;
    }
    if let Some(entry) = ledger
        .pending_handle_reassignments
        .get_mut(&operation.reassignment_id)
    {
        entry.state = "canceled".to_string();
    }
    Ok(())
}
