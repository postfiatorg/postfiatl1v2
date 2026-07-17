use super::*;

const FASTPAY_V3_TRANSFER_ORDER_HASH_DOMAIN: &str = "postfiat.fastpay.transfer-order.v3";
const FASTPAY_V3_UNWRAP_ORDER_HASH_DOMAIN: &str = "postfiat.fastpay.unwrap-order.v3";
const FASTPAY_SPECULATIVE_JOURNAL_SCHEMA_V1: &str = "postfiat.fastpay.speculative-effects.v1";

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FastPayPriorObjectV1 {
    index: u64,
    object: postfiat_types::OwnedObject,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FastPaySpeculativeEffectV1 {
    fence: postfiat_types::FastPayVersionFenceV1,
    prior_objects: Vec<FastPayPriorObjectV1>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    prior_unwrap_account: Option<postfiat_types::Account>,
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct FastPaySpeculativeJournalV1 {
    schema: String,
    effects: Vec<FastPaySpeculativeEffectV1>,
}

impl Default for FastPaySpeculativeJournalV1 {
    fn default() -> Self {
        Self {
            schema: FASTPAY_SPECULATIVE_JOURNAL_SCHEMA_V1.to_string(),
            effects: Vec::new(),
        }
    }
}

fn fastpay_invalid_data(message: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_string())
}

fn read_fastpay_speculative_journal(
    data_dir: &std::path::Path,
) -> io::Result<FastPaySpeculativeJournalV1> {
    let path = data_dir.join(FASTPAY_SPECULATIVE_JOURNAL_FILE);
    if !path.exists() {
        return Ok(FastPaySpeculativeJournalV1::default());
    }
    let raw = read_bounded_json_text_file(&path, "FastPay speculative-effect journal")?;
    let journal: FastPaySpeculativeJournalV1 = serde_json::from_str(&raw).map_err(invalid_data)?;
    if journal.schema != FASTPAY_SPECULATIVE_JOURNAL_SCHEMA_V1 {
        return Err(fastpay_invalid_data(
            "FastPay speculative-effect journal schema mismatch",
        ));
    }
    let mut prior_lock_id: Option<&str> = None;
    for effect in &journal.effects {
        effect
            .fence
            .validate_shape()
            .map_err(fastpay_invalid_data)?;
        if effect.fence.origin != postfiat_types::FastPayFenceOriginV1::Consensusless
            || prior_lock_id.is_some_and(|prior| prior >= effect.fence.lock_id.as_str())
        {
            return Err(fastpay_invalid_data(
                "FastPay speculative-effect journal is not canonical",
            ));
        }
        prior_lock_id = Some(&effect.fence.lock_id);
    }
    Ok(journal)
}

fn write_fastpay_speculative_journal(
    data_dir: &std::path::Path,
    journal: &FastPaySpeculativeJournalV1,
) -> io::Result<()> {
    let bytes = format!(
        "{}\n",
        serde_json::to_string_pretty(journal).map_err(invalid_data)?
    );
    if bytes.len() as u64 > MAX_LOCAL_JSON_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "FastPay speculative-effect journal is full; settle existing recovery windows",
        ));
    }
    atomic_write(data_dir.join(FASTPAY_SPECULATIVE_JOURNAL_FILE), bytes)
}

pub(super) fn fastpay_speculative_journal_snapshot_bytes(
    data_dir: &std::path::Path,
) -> io::Result<Vec<u8>> {
    let journal = read_fastpay_speculative_journal(data_dir)?;
    Ok(format!(
        "{}\n",
        serde_json::to_string_pretty(&journal).map_err(invalid_data)?
    )
    .into_bytes())
}

fn prior_objects_for_fastpay_inputs(
    ledger: &LedgerState,
    inputs: &[postfiat_types::OwnedObjectRef],
) -> io::Result<Vec<FastPayPriorObjectV1>> {
    let mut prior = Vec::with_capacity(inputs.len());
    for input in inputs {
        let (index, object) = ledger
            .owned_objects
            .iter()
            .enumerate()
            .find(|(_, object)| object.id == input.id && object.version == input.version)
            .ok_or_else(|| fastpay_invalid_data("FastPay speculative input is absent"))?;
        prior.push(FastPayPriorObjectV1 {
            index: u64::try_from(index).map_err(invalid_data)?,
            object: object.clone(),
        });
    }
    prior.sort_by_key(|entry| entry.index);
    if prior.windows(2).any(|pair| pair[0].index >= pair[1].index) {
        return Err(fastpay_invalid_data(
            "FastPay speculative input indexes are not unique",
        ));
    }
    Ok(prior)
}

fn retain_fastpay_speculative_effect(
    store: &NodeStore,
    ledger: &LedgerState,
    effect: FastPaySpeculativeEffectV1,
) -> io::Result<()> {
    let mut journal = read_fastpay_speculative_journal(store.data_dir())?;
    if let Some(existing) = journal
        .effects
        .iter()
        .find(|existing| existing.fence.lock_id == effect.fence.lock_id)
    {
        return if existing == &effect {
            Ok(())
        } else {
            Err(fastpay_invalid_data(
                "FastPay speculative journal contains a conflicting lock",
            ))
        };
    }

    let anchored = store
        .read_blocks()?
        .blocks
        .into_iter()
        .flat_map(|block| block.fastpay_pre_state_effects)
        .map(|fence| fence.lock_id)
        .collect::<BTreeSet<_>>();
    let height = fastpay_height(store)?;
    journal.effects.retain(|record| {
        let recovery = record
            .fence
            .certificate
            .as_ref()
            .map(postfiat_types::FastPayCertificateV1::recovery);
        let ordered_terminal = ledger.fastpay_version_fences.iter().any(|fence| {
            fence.lock_id == record.fence.lock_id
                && fence.origin == postfiat_types::FastPayFenceOriginV1::OrderedRecovery
        });
        !anchored.contains(&record.fence.lock_id)
            && !ordered_terminal
            && (ledger.fastpay_version_fences.contains(&record.fence)
                || recovery.is_some_and(|window| height < window.recovery_closes_at_height))
    });
    journal.effects.push(effect);
    journal
        .effects
        .sort_by(|left, right| left.fence.lock_id.cmp(&right.fence.lock_id));
    write_fastpay_speculative_journal(store.data_dir(), &journal)
}

fn fastpay_height(store: &NodeStore) -> io::Result<u64> {
    store
        .read_chain_tip()
        .map(|tip| tip.height)
        .or_else(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                Ok(store
                    .read_blocks()?
                    .blocks
                    .last()
                    .map_or(0, |block| block.header.height))
            } else {
                Err(error)
            }
        })
}

fn fastpay_recovery_policy(
    ledger: &LedgerState,
) -> io::Result<postfiat_types::FastPayRecoveryPolicyV1> {
    let policy = ledger.fastpay_recovery_policy.clone().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::Unsupported,
            "FastPay v3 recovery policy is not active",
        )
    })?;
    policy.validate().map_err(fastpay_invalid_data)?;
    Ok(policy)
}

fn fastpay_recovery_committee(
    ledger: &LedgerState,
    committee_epoch: u64,
    registry_root: &str,
) -> io::Result<postfiat_types::FastPayRecoveryCommitteeV1> {
    let committee = ledger
        .fastpay_recovery_committees
        .iter()
        .find(|committee| {
            committee.committee_epoch == committee_epoch && committee.registry_root == registry_root
        })
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "FastPay v3 recovery committee is not active",
            )
        })?;
    committee.validate().map_err(fastpay_invalid_data)?;
    Ok(committee)
}

fn active_fastpay_recovery_committee(
    ledger: &LedgerState,
    height: u64,
) -> io::Result<postfiat_types::FastPayRecoveryCommitteeV1> {
    let committee = ledger
        .fastpay_recovery_committees
        .iter()
        .filter(|committee| {
            committee.valid_from_height <= height && height <= committee.new_orders_through_height
        })
        .max_by_key(|committee| committee.committee_epoch)
        .cloned()
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "no FastPay v3 committee admits new orders at the current height",
            )
        })?;
    committee.validate().map_err(fastpay_invalid_data)?;
    Ok(committee)
}

fn validate_local_fastpay_committee(
    data_dir: &std::path::Path,
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
) -> io::Result<()> {
    let mut local = load_validator_pubkeys(data_dir)?;
    local.sort_by(|left, right| left.0.cmp(&right.0));
    if local != committee.validator_public_keys() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "local validator registry does not match the replicated FastPay committee",
        ));
    }
    Ok(())
}

fn ensure_fastpay_unanchored_capacity(store: &NodeStore, ledger: &LedgerState) -> io::Result<()> {
    let pending = fastpay_pre_state_effects_for_next_block(store, ledger)?.len();
    if pending >= postfiat_types::MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "FastPay unanchored-effect window is full; wait for the next certified block",
        ));
    }
    Ok(())
}

pub fn owned_certificate_domain_v3(
    data_dir: &std::path::Path,
) -> io::Result<postfiat_types::OwnedCertificateDomain> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let committee = active_fastpay_recovery_committee(&ledger, fastpay_height(&store)?)?;
    let domain = committee.certificate_domain();
    if domain.chain_id != genesis.chain_id
        || domain.genesis_hash != postfiat_execution::genesis_hash(&genesis)
        || domain.protocol_version != genesis.protocol_version
    {
        return Err(fastpay_invalid_data(
            "replicated FastPay committee does not match local genesis",
        ));
    }
    Ok(domain)
}

fn fastpay_domain_for_committee(
    data_dir: &std::path::Path,
    committee: &postfiat_types::FastPayRecoveryCommitteeV1,
) -> io::Result<postfiat_types::OwnedCertificateDomain> {
    let genesis = NodeStore::new(data_dir).read_genesis()?;
    let domain = committee.certificate_domain();
    if domain.chain_id != genesis.chain_id
        || domain.genesis_hash != postfiat_execution::genesis_hash(&genesis)
        || domain.protocol_version != genesis.protocol_version
    {
        return Err(fastpay_invalid_data(
            "replicated FastPay committee does not match local genesis",
        ));
    }
    Ok(domain)
}

pub fn owned_recovery_capabilities_v3(options: NodeOptions) -> io::Result<String> {
    let store = NodeStore::new(&options.data_dir);
    let _read_lock = store.lock_ordered_commit()?;
    let ledger = store.read_ledger()?;
    ensure_fastpay_unanchored_capacity(&store, &ledger)?;
    let height = fastpay_height(&store)?;
    let committee = active_fastpay_recovery_committee(&ledger, height)?;
    validate_local_fastpay_committee(&options.data_dir, &committee)?;
    let report = postfiat_types::FastPayRecoveryCapabilitiesV1 {
        schema: postfiat_types::FASTPAY_RECOVERY_CAPABILITIES_SCHEMA_V1.to_string(),
        domain: owned_certificate_domain_v3(&options.data_dir)?,
        committee_epoch: committee.committee_epoch,
        current_height: height,
        validator_count: committee.validators.len(),
        quorum: committee.quorum,
        policy: fastpay_recovery_policy(&ledger)?,
    };
    report.validate().map_err(fastpay_invalid_data)?;
    serde_json::to_string(&report).map_err(invalid_data)
}

pub fn owned_sign_v3(
    options: NodeOptions,
    signed_order_json: &str,
    validator_id: &str,
) -> io::Result<String> {
    let signed: postfiat_types::SignedOwnedTransferOrderV3 =
        serde_json::from_str(signed_order_json).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("signed FastPay v3 transfer parse failed: {error}"),
            )
        })?;
    let store = NodeStore::new(&options.data_dir);
    let _mutation_lock = store.lock_ordered_commit()?;
    let ledger = store.read_ledger()?;
    ensure_fastpay_unanchored_capacity(&store, &ledger)?;
    let height = fastpay_height(&store)?;
    let committee = fastpay_recovery_committee(
        &ledger,
        signed.order.recovery.committee_epoch,
        &signed.order.domain.registry_id,
    )?;
    validate_local_fastpay_committee(&options.data_dir, &committee)?;
    if height < committee.valid_from_height
        || height > committee.new_orders_through_height
        || signed.order.recovery.valid_from_height > committee.new_orders_through_height
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "FastPay v3 transfer is outside the committee admission window",
        ));
    }
    let committee_epoch = committee.committee_epoch;
    let domain = fastpay_domain_for_committee(&options.data_dir, &committee)?;
    let policy = fastpay_recovery_policy(&ledger)?;
    postfiat_execution::validate_owned_transfer_v3_admission(
        &ledger,
        &signed,
        &domain,
        committee_epoch,
        &policy,
        height,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("FastPay v3 transfer admission failed: {error:?}"),
        )
    })?;
    let signing_bytes = postfiat_execution::owned_transfer_v3_signing_bytes(&signed.order);
    let order_digest = bytes_to_hex(&hash_bytes(
        FASTPAY_V3_TRANSFER_ORDER_HASH_DOMAIN,
        &signing_bytes,
    ));
    reserve_owned_input_locks(
        &options.data_dir,
        &signed.order.inputs,
        &domain.registry_id,
        &signed.order.recovery.lock_id,
        "owned-sign-v3",
    )?;
    if order_digest != postfiat_execution::fastpay_transfer_order_digest_v3(&signed.order) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay v3 transfer order digest mismatch",
        ));
    }
    let secret_key = load_owned_validator_secret_key(&options.data_dir, validator_id)?;
    let signature = ml_dsa_65_sign_with_context(
        &secret_key,
        &signing_bytes,
        postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
    )
    .map_err(|error| io::Error::other(format!("FastPay v3 transfer sign failed: {error}")))?;
    serde_json::to_string(&postfiat_types::OwnedTransferVote {
        validator_id: validator_id.to_string(),
        signature_hex: bytes_to_hex(&signature),
    })
    .map_err(invalid_data)
}

pub fn owned_unwrap_sign_v3(
    options: NodeOptions,
    signed_order_json: &str,
    validator_id: &str,
) -> io::Result<String> {
    let signed: postfiat_types::SignedOwnedUnwrapOrderV3 = serde_json::from_str(signed_order_json)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("signed FastPay v3 unwrap parse failed: {error}"),
            )
        })?;
    let store = NodeStore::new(&options.data_dir);
    let _mutation_lock = store.lock_ordered_commit()?;
    let ledger = store.read_ledger()?;
    let height = fastpay_height(&store)?;
    let committee = fastpay_recovery_committee(
        &ledger,
        signed.order.recovery.committee_epoch,
        &signed.order.domain.registry_id,
    )?;
    validate_local_fastpay_committee(&options.data_dir, &committee)?;
    if height < committee.valid_from_height
        || height > committee.new_orders_through_height
        || signed.order.recovery.valid_from_height > committee.new_orders_through_height
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "FastPay v3 unwrap is outside the committee admission window",
        ));
    }
    let committee_epoch = committee.committee_epoch;
    let domain = fastpay_domain_for_committee(&options.data_dir, &committee)?;
    let policy = fastpay_recovery_policy(&ledger)?;
    postfiat_execution::validate_owned_unwrap_v3_admission(
        &ledger,
        &signed,
        &domain,
        committee_epoch,
        &policy,
        height,
    )
    .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("FastPay v3 unwrap admission failed: {error:?}"),
        )
    })?;
    let signing_bytes = postfiat_execution::owned_unwrap_v3_signing_bytes(&signed.order);
    let order_digest = bytes_to_hex(&hash_bytes(
        FASTPAY_V3_UNWRAP_ORDER_HASH_DOMAIN,
        &signing_bytes,
    ));
    reserve_owned_input_locks(
        &options.data_dir,
        &signed.order.inputs,
        &domain.registry_id,
        &signed.order.recovery.lock_id,
        "owned-unwrap-sign-v3",
    )?;
    if order_digest != postfiat_execution::fastpay_unwrap_order_digest_v3(&signed.order) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "FastPay v3 unwrap order digest mismatch",
        ));
    }
    let secret_key = load_owned_validator_secret_key(&options.data_dir, validator_id)?;
    let signature = ml_dsa_65_sign_with_context(
        &secret_key,
        &signing_bytes,
        postfiat_execution::OWNED_UNWRAP_CONTEXT_V3,
    )
    .map_err(|error| io::Error::other(format!("FastPay v3 unwrap sign failed: {error}")))?;
    serde_json::to_string(&postfiat_types::OwnedUnwrapVote {
        validator_id: validator_id.to_string(),
        signature_hex: bytes_to_hex(&signature),
    })
    .map_err(invalid_data)
}

fn fastpay_ack_for_fence(
    options: &NodeOptions,
    validator_id: &str,
    domain: postfiat_types::OwnedCertificateDomain,
    fence: &postfiat_types::FastPayVersionFenceV1,
) -> io::Result<postfiat_types::FastPayApplyAckV1> {
    let (order_digest, certificate_digest) = match &fence.decision {
        postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
            order_digest,
            certificate_digest,
        } => (order_digest.clone(), certificate_digest.clone()),
        postfiat_types::FastPayRecoveryDecisionV1::Cancelled => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "cancelled FastPay fence cannot produce an apply acknowledgement",
            ));
        }
    };
    let terminal_bytes = fence
        .state_commitment_bytes()
        .map_err(fastpay_invalid_data)?;
    let mut acknowledgement = postfiat_types::FastPayApplyAckV1 {
        schema: postfiat_types::FASTPAY_APPLY_ACK_SCHEMA_V1.to_string(),
        domain,
        committee_epoch: fence.committee_epoch,
        lock_id: fence.lock_id.clone(),
        order_digest,
        certificate_digest,
        terminal_state_digest: bytes_to_hex(&hash_bytes(
            "postfiat.fastpay.terminal-state.v1",
            &terminal_bytes,
        )),
        validator_id: validator_id.to_string(),
        signature_hex: String::new(),
    };
    let signing_bytes = postfiat_execution::fastpay_apply_ack_signing_bytes_v1(&acknowledgement)
        .map_err(|error| fastpay_invalid_data(format!("FastPay apply ack encoding: {error:?}")))?;
    let secret_key = load_owned_validator_secret_key(&options.data_dir, validator_id)?;
    acknowledgement.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &secret_key,
            &signing_bytes,
            postfiat_execution::FASTPAY_APPLY_ACK_CONTEXT_V1,
        )
        .map_err(|error| io::Error::other(format!("FastPay apply ack sign failed: {error}")))?,
    );
    acknowledgement
        .validate_shape()
        .map_err(fastpay_invalid_data)?;
    Ok(acknowledgement)
}

fn matching_confirmed_fence<'a>(
    ledger: &'a LedgerState,
    lock_id: &str,
    certificate_digest: &str,
) -> Option<&'a postfiat_types::FastPayVersionFenceV1> {
    ledger.fastpay_version_fences.iter().find(|fence| {
        fence.lock_id == lock_id
            && matches!(
                &fence.decision,
                postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
                    certificate_digest: existing,
                    ..
                } if existing == certificate_digest
            )
    })
}

fn restore_fastpay_swap_removed_inputs(
    owned_objects: &mut Vec<postfiat_types::OwnedObject>,
    prior_objects: &[FastPayPriorObjectV1],
) -> io::Result<()> {
    for prior in prior_objects {
        let index = usize::try_from(prior.index).map_err(invalid_data)?;
        let prior_len = owned_objects
            .len()
            .checked_add(1)
            .ok_or_else(|| fastpay_invalid_data("FastPay rollback length overflow"))?;
        if index >= prior_len
            || owned_objects
                .iter()
                .any(|object| object.id == prior.object.id)
        {
            return Err(fastpay_invalid_data(
                "FastPay rollback input position or identity mismatch",
            ));
        }
        if index == owned_objects.len() {
            owned_objects.push(prior.object.clone());
        } else {
            let swapped_tail = owned_objects[index].clone();
            owned_objects.push(swapped_tail);
            owned_objects[index] = prior.object.clone();
        }
    }
    Ok(())
}

fn rollback_fastpay_speculative_effect(
    ledger: &mut LedgerState,
    record: &FastPaySpeculativeEffectV1,
) -> io::Result<()> {
    if ledger.fastpay_version_fences.last() != Some(&record.fence) {
        return Err(fastpay_invalid_data(
            "FastPay speculative rollback is not in reverse application order",
        ));
    }
    let certificate =
        record.fence.certificate.as_ref().ok_or_else(|| {
            fastpay_invalid_data("FastPay speculative rollback omitted certificate")
        })?;
    let input_refs = certificate.inputs();
    if record.prior_objects.len() != input_refs.len()
        || record
            .prior_objects
            .iter()
            .zip(input_refs)
            .any(|(prior, input)| {
                prior.object.id != input.id || prior.object.version != input.version
            })
    {
        return Err(fastpay_invalid_data(
            "FastPay speculative rollback inputs do not match certificate",
        ));
    }

    let mut scratch = LedgerState::empty();
    scratch.owned_objects = record
        .prior_objects
        .iter()
        .map(|entry| entry.object.clone())
        .collect();
    match certificate {
        postfiat_types::FastPayCertificateV1::Transfer(certificate) => {
            if record.prior_unwrap_account.is_some() {
                return Err(fastpay_invalid_data(
                    "FastPay transfer rollback unexpectedly contains account state",
                ));
            }
            let order = postfiat_types::OwnedTransferOrder {
                domain: certificate.order.domain.clone(),
                inputs: certificate.order.inputs.clone(),
                outputs: certificate.order.outputs.clone(),
                fee: certificate.order.fee,
                nonce: certificate.order.nonce,
                memos: certificate.order.memos.clone(),
            };
            let outcome = postfiat_execution::apply_owned_transfer(
                &mut scratch,
                &order,
                &certificate.owner_pubkey_hex,
            )
            .map_err(|error| {
                fastpay_invalid_data(format!("FastPay transfer rollback model: {error:?}"))
            })?;
            if ledger.owned_objects.len() < outcome.created.len()
                || ledger.owned_objects[ledger.owned_objects.len() - outcome.created.len()..]
                    != outcome.created
            {
                return Err(fastpay_invalid_data(
                    "FastPay transfer rollback outputs were changed or reordered",
                ));
            }
            ledger
                .owned_objects
                .truncate(ledger.owned_objects.len() - outcome.created.len());
        }
        postfiat_types::FastPayCertificateV1::Unwrap(certificate) => {
            if let Some(account) = &record.prior_unwrap_account {
                scratch.accounts.push(account.clone());
            }
            let order = postfiat_types::OwnedUnwrapOrder {
                domain: certificate.order.domain.clone(),
                inputs: certificate.order.inputs.clone(),
                to_address: certificate.order.to_address.clone(),
                amount: certificate.order.amount,
                asset: certificate.order.asset.clone(),
                fee: certificate.order.fee,
                nonce: certificate.order.nonce,
                memos: certificate.order.memos.clone(),
            };
            let outcome = postfiat_execution::apply_owned_unwrap(
                &mut scratch,
                &order,
                &certificate.owner_pubkey_hex,
            )
            .map_err(|error| {
                fastpay_invalid_data(format!("FastPay unwrap rollback model: {error:?}"))
            })?;
            if let Some(change) = outcome.change_object {
                if ledger.owned_objects.last() != Some(&change) {
                    return Err(fastpay_invalid_data(
                        "FastPay unwrap rollback change object was changed or reordered",
                    ));
                }
                ledger.owned_objects.pop();
            }
            let expected_account = scratch
                .account(&certificate.order.to_address)
                .ok_or_else(|| fastpay_invalid_data("FastPay unwrap rollback account missing"))?;
            if ledger.account(&certificate.order.to_address) != Some(expected_account) {
                return Err(fastpay_invalid_data(
                    "FastPay unwrap rollback account was changed after speculative apply",
                ));
            }
            match &record.prior_unwrap_account {
                Some(prior) => {
                    *ledger
                        .account_mut(&certificate.order.to_address)
                        .ok_or_else(|| fastpay_invalid_data("FastPay unwrap account missing"))? =
                        prior.clone();
                }
                None => ledger
                    .accounts
                    .retain(|account| account.address != certificate.order.to_address),
            }
        }
    }
    restore_fastpay_swap_removed_inputs(&mut ledger.owned_objects, &record.prior_objects)?;
    ledger.fastpay_version_fences.pop();
    Ok(())
}

pub(super) fn rollback_unanchored_fastpay_effects_for_certified_block(
    store: &NodeStore,
    ledger: &mut LedgerState,
    local_unanchored: &[postfiat_types::FastPayVersionFenceV1],
) -> io::Result<()> {
    if local_unanchored.is_empty() {
        return Ok(());
    }
    let journal = read_fastpay_speculative_journal(store.data_dir())?;
    let local_locks = local_unanchored
        .iter()
        .map(|fence| fence.lock_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut records = ledger
        .fastpay_version_fences
        .iter()
        .rev()
        .filter(|fence| local_locks.contains(fence.lock_id.as_str()))
        .map(|fence| {
            journal
                .effects
                .iter()
                .find(|record| record.fence == *fence)
                .cloned()
                .ok_or_else(|| {
                    fastpay_invalid_data(
                        "certified FastPay rollback is missing its durable inverse journal",
                    )
                })
        })
        .collect::<io::Result<Vec<_>>>()?;
    if records.len() != local_unanchored.len() {
        return Err(fastpay_invalid_data(
            "certified FastPay rollback did not resolve every unanchored effect",
        ));
    }
    for record in records.drain(..) {
        rollback_fastpay_speculative_effect(ledger, &record)?;
    }
    Ok(())
}

pub fn owned_apply_v3(
    options: NodeOptions,
    cert_json: &str,
    validator_id: &str,
) -> io::Result<String> {
    let certificate: postfiat_types::OwnedTransferCertificateV3 = serde_json::from_str(cert_json)
        .map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("FastPay v3 transfer certificate parse failed: {error}"),
        )
    })?;
    let certificate_digest = postfiat_execution::fastpay_transfer_certificate_digest_v3(
        &certificate,
    )
    .map_err(|error| fastpay_invalid_data(format!("FastPay v3 certificate digest: {error:?}")))?;
    let store = NodeStore::new(&options.data_dir);
    let _mutation_lock = store.lock_ordered_commit()?;
    let mut ledger = store.read_ledger()?;
    let height = fastpay_height(&store)?;
    let committee = fastpay_recovery_committee(
        &ledger,
        certificate.order.recovery.committee_epoch,
        &certificate.order.domain.registry_id,
    )?;
    validate_local_fastpay_committee(&options.data_dir, &committee)?;
    let domain = fastpay_domain_for_committee(&options.data_dir, &committee)?;
    let policy = fastpay_recovery_policy(&ledger)?;
    let fence = if let Some(existing) = matching_confirmed_fence(
        &ledger,
        &certificate.order.recovery.lock_id,
        &certificate_digest,
    ) {
        existing.clone()
    } else {
        ensure_fastpay_unanchored_capacity(&store, &ledger)?;
        let validator_pks = committee.validator_public_keys();
        let context = postfiat_execution::FastPayRecoveryVerificationContext {
            validator_public_keys: &validator_pks,
            expected_domain: &domain,
            committee_epoch: committee.committee_epoch,
            policy: &policy,
            quorum: committee.quorum,
        };
        let mut next_ledger = ledger.clone();
        postfiat_execution::apply_owned_transfer_certificate_v3(
            &mut next_ledger,
            &certificate,
            context,
            height,
        )
        .map_err(|error| {
            io::Error::other(format!("FastPay v3 transfer apply failed: {error:?}"))
        })?;
        let prior_objects = prior_objects_for_fastpay_inputs(&ledger, &certificate.order.inputs)?;
        let fence = next_ledger
            .fastpay_version_fences
            .last()
            .cloned()
            .ok_or_else(|| fastpay_invalid_data("FastPay v3 transfer apply omitted its fence"))?;
        retain_fastpay_speculative_effect(
            &store,
            &next_ledger,
            FastPaySpeculativeEffectV1 {
                fence: fence.clone(),
                prior_objects,
                prior_unwrap_account: None,
            },
        )?;
        ledger = next_ledger;
        store.write_ledger(&ledger)?;
        fence
    };
    let acknowledgement = fastpay_ack_for_fence(&options, validator_id, domain, &fence)?;
    serde_json::to_string(&acknowledgement).map_err(invalid_data)
}

pub fn owned_unwrap_apply_v3(
    options: NodeOptions,
    cert_json: &str,
    validator_id: &str,
) -> io::Result<String> {
    let certificate: postfiat_types::OwnedUnwrapCertificateV3 = serde_json::from_str(cert_json)
        .map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("FastPay v3 unwrap certificate parse failed: {error}"),
            )
        })?;
    let certificate_digest = postfiat_execution::fastpay_unwrap_certificate_digest_v3(&certificate)
        .map_err(|error| {
            fastpay_invalid_data(format!("FastPay v3 certificate digest: {error:?}"))
        })?;
    let store = NodeStore::new(&options.data_dir);
    let _mutation_lock = store.lock_ordered_commit()?;
    let mut ledger = store.read_ledger()?;
    let height = fastpay_height(&store)?;
    let committee = fastpay_recovery_committee(
        &ledger,
        certificate.order.recovery.committee_epoch,
        &certificate.order.domain.registry_id,
    )?;
    validate_local_fastpay_committee(&options.data_dir, &committee)?;
    let domain = fastpay_domain_for_committee(&options.data_dir, &committee)?;
    let policy = fastpay_recovery_policy(&ledger)?;
    let fence = if let Some(existing) = matching_confirmed_fence(
        &ledger,
        &certificate.order.recovery.lock_id,
        &certificate_digest,
    ) {
        existing.clone()
    } else {
        ensure_fastpay_unanchored_capacity(&store, &ledger)?;
        let validator_pks = committee.validator_public_keys();
        let context = postfiat_execution::FastPayRecoveryVerificationContext {
            validator_public_keys: &validator_pks,
            expected_domain: &domain,
            committee_epoch: committee.committee_epoch,
            policy: &policy,
            quorum: committee.quorum,
        };
        let mut next_ledger = ledger.clone();
        postfiat_execution::apply_owned_unwrap_certificate_v3(
            &mut next_ledger,
            &certificate,
            context,
            height,
        )
        .map_err(|error| io::Error::other(format!("FastPay v3 unwrap apply failed: {error:?}")))?;
        let prior_objects = prior_objects_for_fastpay_inputs(&ledger, &certificate.order.inputs)?;
        let prior_unwrap_account = ledger.account(&certificate.order.to_address).cloned();
        let fence = next_ledger
            .fastpay_version_fences
            .last()
            .cloned()
            .ok_or_else(|| fastpay_invalid_data("FastPay v3 unwrap apply omitted its fence"))?;
        retain_fastpay_speculative_effect(
            &store,
            &next_ledger,
            FastPaySpeculativeEffectV1 {
                fence: fence.clone(),
                prior_objects,
                prior_unwrap_account,
            },
        )?;
        ledger = next_ledger;
        store.write_ledger(&ledger)?;
        fence
    };
    let acknowledgement = fastpay_ack_for_fence(&options, validator_id, domain, &fence)?;
    serde_json::to_string(&acknowledgement).map_err(invalid_data)
}

pub fn owned_certificate_v3(options: NodeOptions, selector: &str) -> io::Result<String> {
    validate_hex_string("FastPay certificate selector", selector, Some(96))?;
    let ledger = NodeStore::new(&options.data_dir).read_ledger()?;
    let mut certificate = ledger
        .fastpay_version_fences
        .iter()
        .find(|fence| {
            fence.lock_id == selector
                || matches!(
                    &fence.decision,
                    postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
                        certificate_digest,
                        ..
                    } if certificate_digest == selector
                )
        })
        .and_then(|fence| fence.certificate.clone())
        .or_else(|| {
            ledger
                .fastpay_recovery_reveals
                .iter()
                .find(|reveal| reveal.lock_id == selector || reveal.certificate_digest == selector)
                .map(|reveal| reveal.certificate.clone())
        });
    if certificate.is_none() {
        certificate = read_fastpay_speculative_journal(&options.data_dir)?
            .effects
            .into_iter()
            .find(|record| {
                record.fence.lock_id == selector
                    || matches!(
                        &record.fence.decision,
                        postfiat_types::FastPayRecoveryDecisionV1::Confirmed {
                            certificate_digest,
                            ..
                        } if certificate_digest == selector
                    )
            })
            .and_then(|record| record.fence.certificate);
    }
    let certificate = certificate
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "FastPay certificate not found"))?;
    serde_json::to_string(&certificate).map_err(invalid_data)
}

pub fn owned_recovery_status_v3(options: NodeOptions, lock_id: &str) -> io::Result<String> {
    validate_hex_string("FastPay lock ID", lock_id, Some(96))?;
    let ledger = NodeStore::new(&options.data_dir).read_ledger()?;
    let fence = ledger
        .fastpay_version_fences
        .iter()
        .find(|fence| fence.lock_id == lock_id);
    let reveals = ledger
        .fastpay_recovery_reveals
        .iter()
        .filter(|reveal| reveal.lock_id == lock_id)
        .count();
    serde_json::to_string(&serde_json::json!({
        "schema": "postfiat-fastpay-recovery-status-v1",
        "lock_id": lock_id,
        "status": match fence.map(|value| &value.decision) {
            Some(postfiat_types::FastPayRecoveryDecisionV1::Confirmed { .. }) => "confirmed",
            Some(postfiat_types::FastPayRecoveryDecisionV1::Cancelled) => "cancelled",
            None if reveals > 0 => "certificate_revealed",
            None => "open_or_unknown",
        },
        "reveal_count": reveals,
        "fence": fence,
    }))
    .map_err(invalid_data)
}
