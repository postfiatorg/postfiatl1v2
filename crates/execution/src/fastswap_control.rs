use crate::fastswap_bridge::asset_definition_hash;
use crate::fastswap_checkpoint::verify_fastlane_checkpoint_certificate;
use postfiat_crypto_provider::ml_dsa_65_verify_with_context;
use postfiat_types::{
    FastAssetIdV1, FastAssetRuleV1, FastHolderPermitV1, FastLaneCheckpointCertificateV1,
    FastLaneControlActionV1, FastLaneControlCertificateV1, FastLanePrepareFenceV1,
    FastSwapCommitteeV1, FastSwapGovernanceBootstrapV1, FastSwapPolicySnapshotV1, LedgerState,
    FASTLANE_CONTROL_CONTEXT_V1, FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1,
    FASTSWAP_SCHEMA_VERSION_V1,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastSwapControlError {
    InvalidCommittee,
    CommitteeConflict,
    CommitteeEpochGap,
    CommitteeChainMismatch,
    MissingFinalCheckpoint,
    NotDrained,
    InvalidRule,
    RuleConflict,
    InvalidHolderPermit,
    HolderPermitConflict,
    InvalidPolicy,
    PolicyConflict,
    MissingRule,
    InvalidFence,
    FenceConflict,
    InvalidControlCertificate,
    InvalidActivation,
    ActivationConflict,
    BootstrapConflict,
    InvalidBootstrap,
}

pub fn execute_fastswap_governance_bootstrap(
    ledger: &mut LedgerState,
    bootstrap: &FastSwapGovernanceBootstrapV1,
    finalized_height: u64,
) -> Result<(), FastSwapControlError> {
    bootstrap
        .validate_payload()
        .map_err(|_| FastSwapControlError::InvalidBootstrap)?;
    let bootstrap_id = bootstrap
        .bootstrap_id()
        .map_err(|_| FastSwapControlError::InvalidBootstrap)?;
    let expected_kind = format!(
        "{}{}",
        FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1,
        postfiat_crypto_provider::bytes_to_hex(&bootstrap_id.0)
    );
    let amendment = &bootstrap.amendment;
    let committee_validators = bootstrap
        .payload
        .committee
        .validators
        .iter()
        .map(|validator| validator.validator_id.as_str())
        .collect::<Vec<_>>();
    if amendment.kind != expected_kind
        || amendment.value != FASTSWAP_SCHEMA_VERSION_V1
        || amendment.chain_id != bootstrap.payload.committee.domain.chain.chain_id
        || amendment.genesis_hash
            != postfiat_crypto_provider::bytes_to_hex(
                &bootstrap.payload.committee.domain.chain.genesis_hash.0,
            )
        || amendment.protocol_version != bootstrap.payload.committee.domain.chain.protocol_version
        || committee_validators
            != amendment
                .validators
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
        || amendment.paused
        || bootstrap.payload.activation_height <= finalized_height
    {
        return Err(FastSwapControlError::InvalidBootstrap);
    }
    if !ledger.fastswap_committees.is_empty() || ledger.fastswap_activation_height.is_some() {
        return Err(FastSwapControlError::BootstrapConflict);
    }

    let mut next = ledger.clone();
    register_fastswap_committee(&mut next, bootstrap.payload.committee.clone(), None)?;
    for rule in &bootstrap.payload.asset_rules {
        register_fastlane_asset_rule(&mut next, rule.clone())?;
    }
    for policy in &bootstrap.payload.policies {
        register_fastswap_policy(&mut next, policy.clone())?;
    }
    next.fastswap_activation_height = Some(bootstrap.payload.activation_height);
    *ledger = next;
    Ok(())
}

pub fn verify_fastlane_control_certificate(
    committee: &FastSwapCommitteeV1,
    certificate: &FastLaneControlCertificateV1,
) -> Result<(), FastSwapControlError> {
    committee
        .validate()
        .map_err(|_| FastSwapControlError::InvalidControlCertificate)?;
    certificate
        .validate_canonical_order()
        .map_err(|_| FastSwapControlError::InvalidControlCertificate)?;
    if certificate.votes.len() < usize::from(committee.domain.quorum) {
        return Err(FastSwapControlError::InvalidControlCertificate);
    }
    let digest = certificate
        .action
        .digest()
        .map_err(|_| FastSwapControlError::InvalidControlCertificate)?;
    for vote in &certificate.votes {
        if vote.committee != committee.domain || vote.action_digest != digest {
            return Err(FastSwapControlError::InvalidControlCertificate);
        }
        let validator = committee
            .validators
            .iter()
            .find(|validator| validator.validator_id == vote.validator_id)
            .ok_or(FastSwapControlError::InvalidControlCertificate)?;
        if !ml_dsa_65_verify_with_context(
            &validator.public_key,
            &vote
                .signing_bytes()
                .map_err(|_| FastSwapControlError::InvalidControlCertificate)?,
            &vote.signature,
            FASTLANE_CONTROL_CONTEXT_V1,
        ) {
            return Err(FastSwapControlError::InvalidControlCertificate);
        }
    }
    Ok(())
}

pub fn execute_fastlane_control(
    ledger: &mut LedgerState,
    certificate: &FastLaneControlCertificateV1,
    finalized_height: u64,
) -> Result<(), FastSwapControlError> {
    let active = ledger
        .fastswap_committees
        .last()
        .cloned()
        .ok_or(FastSwapControlError::InvalidControlCertificate)?;
    verify_fastlane_control_certificate(&active, certificate)?;
    let mut next = ledger.clone();
    match &certificate.action {
        FastLaneControlActionV1::RegisterAssetRule { rule } => {
            register_fastlane_asset_rule(&mut next, rule.clone())?;
        }
        FastLaneControlActionV1::RegisterHolderPermit { permit } => {
            register_fastlane_holder_permit(&mut next, permit.clone())?;
        }
        FastLaneControlActionV1::RegisterPolicy { policy } => {
            register_fastswap_policy(&mut next, policy.clone())?;
        }
        FastLaneControlActionV1::StopPrepare { fence } => {
            if fence.finalized_primary_height == 0
                || fence.finalized_primary_height > finalized_height
            {
                return Err(FastSwapControlError::InvalidFence);
            }
            apply_fastlane_prepare_fence(&mut next, fence.clone())?;
        }
        FastLaneControlActionV1::ActivateCommittee {
            committee,
            final_checkpoint,
        } => {
            register_fastswap_committee(&mut next, committee.clone(), Some(final_checkpoint))?;
        }
        FastLaneControlActionV1::ActivateProtocol { activation_height } => {
            if let Some(existing) = next.fastswap_activation_height {
                if existing != *activation_height {
                    return Err(FastSwapControlError::ActivationConflict);
                }
            } else {
                if *activation_height <= finalized_height {
                    return Err(FastSwapControlError::InvalidActivation);
                }
                next.fastswap_activation_height = Some(*activation_height);
            }
        }
    }
    *ledger = next;
    Ok(())
}

pub fn register_fastlane_asset_rule(
    ledger: &mut LedgerState,
    rule: FastAssetRuleV1,
) -> Result<(), FastSwapControlError> {
    let hash = rule
        .rule_hash()
        .map_err(|_| FastSwapControlError::InvalidRule)?;
    if !rule.fast_lane_enabled || rule.valid_from_height > rule.valid_through_height {
        return Err(FastSwapControlError::InvalidRule);
    }
    if rule.asset_id == FastAssetIdV1::native_pft() {
        if rule.asset_definition_hash != postfiat_types::FastAssetDefinitionHashV1::ZERO
            || rule.requires_authorization
            || rule.freeze_enabled
            || rule.clawback_enabled
        {
            return Err(FastSwapControlError::InvalidRule);
        }
    } else {
        let asset_hex = postfiat_crypto_provider::bytes_to_hex(&rule.asset_id.0);
        let definition = ledger
            .asset_definitions
            .iter()
            .find(|definition| definition.asset_id == asset_hex)
            .ok_or(FastSwapControlError::InvalidRule)?;
        if asset_definition_hash(definition).map_err(|_| FastSwapControlError::InvalidRule)?
            != rule.asset_definition_hash
            || definition.issuer != rule.issuer_address
            || definition.requires_authorization != rule.requires_authorization
            || definition.freeze_enabled != rule.freeze_enabled
            || definition.clawback_enabled != rule.clawback_enabled
        {
            return Err(FastSwapControlError::InvalidRule);
        }
    }
    if let Some(existing) = ledger
        .fast_lane_asset_rules
        .iter()
        .find(|existing| existing.rule_hash().ok() == Some(hash))
    {
        return if existing == &rule {
            Ok(())
        } else {
            Err(FastSwapControlError::RuleConflict)
        };
    }
    ledger.fast_lane_asset_rules.push(rule);
    ledger.fast_lane_asset_rules.sort_by_key(|row| {
        row.rule_hash()
            .unwrap_or(postfiat_types::FastAssetRuleHashV1::ZERO)
    });
    Ok(())
}

pub fn register_fastlane_holder_permit(
    ledger: &mut LedgerState,
    permit: FastHolderPermitV1,
) -> Result<(), FastSwapControlError> {
    let permit_id = permit
        .computed_id()
        .map_err(|_| FastSwapControlError::InvalidHolderPermit)?;
    if permit_id != permit.permit_id
        || !ledger.fast_lane_asset_rules.iter().any(|rule| {
            rule.asset_id == permit.asset_id
                && rule.requires_authorization
                && rule.fast_lane_enabled
        })
    {
        return Err(FastSwapControlError::InvalidHolderPermit);
    }
    if let Some(existing) = ledger
        .fast_lane_holder_permits
        .iter()
        .find(|existing| existing.permit_id == permit_id)
    {
        return if existing == &permit {
            Ok(())
        } else {
            Err(FastSwapControlError::HolderPermitConflict)
        };
    }
    ledger.fast_lane_holder_permits.push(permit);
    ledger
        .fast_lane_holder_permits
        .sort_by_key(|row| row.permit_id);
    Ok(())
}

pub fn register_fastswap_policy(
    ledger: &mut LedgerState,
    policy: FastSwapPolicySnapshotV1,
) -> Result<(), FastSwapControlError> {
    policy
        .validate()
        .map_err(|_| FastSwapControlError::InvalidPolicy)?;
    if !ledger
        .fastswap_committees
        .iter()
        .any(|committee| committee.domain.chain == policy.domain)
    {
        return Err(FastSwapControlError::InvalidPolicy);
    }
    for (asset, hash) in [
        (policy.pair_asset_0, policy.asset_rule_hash_0),
        (policy.pair_asset_1, policy.asset_rule_hash_1),
    ] {
        if !ledger.fast_lane_asset_rules.iter().any(|rule| {
            rule.asset_id == asset && rule.rule_hash().ok().is_some_and(|value| value == hash)
        }) {
            return Err(FastSwapControlError::MissingRule);
        }
    }
    if let Some(existing) = ledger
        .fastswap_policy_snapshots
        .iter()
        .find(|existing| existing.policy_hash == policy.policy_hash)
    {
        return if existing == &policy {
            Ok(())
        } else {
            Err(FastSwapControlError::PolicyConflict)
        };
    }
    if ledger.fastswap_policy_snapshots.iter().any(|existing| {
        existing.policy_epoch == policy.policy_epoch
            && existing.pair_asset_0 == policy.pair_asset_0
            && existing.pair_asset_1 == policy.pair_asset_1
    }) {
        return Err(FastSwapControlError::PolicyConflict);
    }
    ledger.fastswap_policy_snapshots.push(policy);
    ledger
        .fastswap_policy_snapshots
        .sort_by_key(|row| (row.policy_epoch, row.policy_hash));
    Ok(())
}

pub fn apply_fastlane_prepare_fence(
    ledger: &mut LedgerState,
    fence: FastLanePrepareFenceV1,
) -> Result<(), FastSwapControlError> {
    if fence.finalized_primary_height == 0
        || !ledger
            .fastswap_committees
            .iter()
            .any(|committee| committee.domain.committee_epoch == fence.committee_epoch)
        || !ledger
            .fastswap_policy_snapshots
            .iter()
            .any(|policy| policy.policy_epoch == fence.policy_epoch)
    {
        return Err(FastSwapControlError::InvalidFence);
    }
    if let Some(existing) = ledger.fast_lane_prepare_fences.iter().find(|existing| {
        existing.committee_epoch == fence.committee_epoch
            && existing.policy_epoch == fence.policy_epoch
    }) {
        return if existing == &fence {
            Ok(())
        } else {
            Err(FastSwapControlError::FenceConflict)
        };
    }
    ledger.fast_lane_prepare_fences.push(fence);
    ledger
        .fast_lane_prepare_fences
        .sort_by_key(|row| (row.committee_epoch, row.policy_epoch));
    Ok(())
}

pub fn register_fastswap_committee(
    ledger: &mut LedgerState,
    committee: FastSwapCommitteeV1,
    final_checkpoint: Option<&FastLaneCheckpointCertificateV1>,
) -> Result<(), FastSwapControlError> {
    committee
        .validate()
        .map_err(|_| FastSwapControlError::InvalidCommittee)?;
    if let Some(existing) = ledger
        .fastswap_committees
        .iter()
        .find(|existing| existing.domain.committee_epoch == committee.domain.committee_epoch)
    {
        return if existing == &committee {
            Ok(())
        } else {
            Err(FastSwapControlError::CommitteeConflict)
        };
    }
    if let Some(previous) = ledger.fastswap_committees.last() {
        if committee.domain.committee_epoch != previous.domain.committee_epoch.saturating_add(1) {
            return Err(FastSwapControlError::CommitteeEpochGap);
        }
        if committee.domain.chain != previous.domain.chain {
            return Err(FastSwapControlError::CommitteeChainMismatch);
        }
        let certificate = final_checkpoint.ok_or(FastSwapControlError::MissingFinalCheckpoint)?;
        let checkpoint = verify_fastlane_checkpoint_certificate(previous, certificate)
            .map_err(|_| FastSwapControlError::MissingFinalCheckpoint)?;
        let checkpoint_id = checkpoint
            .checkpoint_id()
            .map_err(|_| FastSwapControlError::MissingFinalCheckpoint)?;
        let mut expected_fences = ledger
            .fastswap_policy_snapshots
            .iter()
            .map(|policy| policy.policy_epoch)
            .collect::<Vec<_>>();
        expected_fences.sort();
        expected_fences.dedup();
        if checkpoint.committee != previous.domain
            || !checkpoint.drain_ready
            || !checkpoint.exit_claim_totals.is_empty()
            || checkpoint.fenced_policy_epochs != expected_fences
            || !expected_fences.iter().all(|policy_epoch| {
                ledger.fast_lane_prepare_fences.iter().any(|fence| {
                    fence.committee_epoch == previous.domain.committee_epoch
                        && fence.policy_epoch == *policy_epoch
                })
            })
        {
            return Err(FastSwapControlError::NotDrained);
        }
        if !ledger.fast_lane_checkpoint_anchors.iter().any(|anchor| {
            anchor
                .votes
                .first()
                .and_then(|vote| vote.checkpoint.checkpoint_id().ok())
                == Some(checkpoint_id)
        }) {
            return Err(FastSwapControlError::MissingFinalCheckpoint);
        }
    } else if committee.domain.committee_epoch != 1 {
        return Err(FastSwapControlError::CommitteeEpochGap);
    }
    ledger.fastswap_committees.push(committee);
    ledger
        .fastswap_committees
        .sort_by_key(|row| row.domain.committee_epoch);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{
        hex_to_bytes, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context,
    };
    use postfiat_types::{
        AssetDefinition, FastAssetDefinitionHashV1, FastHolderPermitIdV1, FastLaneControlVoteV1,
        FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1,
        FastSwapGovernanceBootstrapPayloadV1, FastSwapOpaqueHashV1, FastSwapValidatorV1,
        GovernanceAmendment, FASTSWAP_SCHEMA_VERSION_V1,
    };

    #[test]
    fn control_requires_distinct_quorum_signatures_and_applies_atomically() {
        let keys = (0..6)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 50; 32]))
            .collect::<Vec<_>>();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: "fastlane-control-test".to_owned(),
                    genesis_hash: FastSwapOpaqueHashV1([51; 48]),
                    protocol_version: 1,
                },
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 6,
                quorum: 5,
            },
            validators: keys
                .iter()
                .enumerate()
                .map(|(index, key)| FastSwapValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: key.public_key.clone(),
                })
                .collect(),
        };
        committee.domain.committee_root = committee.computed_root().expect("root");
        let rule = FastAssetRuleV1 {
            asset_id: FastAssetIdV1::native_pft(),
            asset_definition_hash: FastAssetDefinitionHashV1::ZERO,
            issuer_address: "native".to_owned(),
            issuer_control_pubkey: vec![1],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 1,
            valid_through_height: 100,
        };
        let action = FastLaneControlActionV1::RegisterAssetRule { rule: rule.clone() };
        let digest = action.digest().expect("digest");
        let votes = keys
            .iter()
            .enumerate()
            .take(5)
            .map(|(index, key)| {
                let mut vote = FastLaneControlVoteV1 {
                    committee: committee.domain.clone(),
                    action_digest: digest,
                    validator_id: format!("validator-{index}"),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context(
                    &key.private_key,
                    &vote.signing_bytes().expect("vote bytes"),
                    FASTLANE_CONTROL_CONTEXT_V1,
                )
                .expect("sign");
                vote
            })
            .collect();
        let certificate = FastLaneControlCertificateV1 { action, votes };
        let mut ledger = LedgerState::empty();
        ledger.fastswap_committees.push(committee);
        execute_fastlane_control(&mut ledger, &certificate, 10).expect("control");
        assert_eq!(ledger.fast_lane_asset_rules, vec![rule.clone()]);
        let after = ledger.clone();

        let mut under_quorum = certificate.clone();
        under_quorum.votes.pop();
        assert_eq!(
            execute_fastlane_control(&mut ledger, &under_quorum, 10),
            Err(FastSwapControlError::InvalidControlCertificate)
        );
        assert_eq!(ledger, after);

        let mut duplicate_validator = certificate.clone();
        duplicate_validator.votes[4] = duplicate_validator.votes[3].clone();
        assert_eq!(
            execute_fastlane_control(&mut ledger, &duplicate_validator, 10),
            Err(FastSwapControlError::InvalidControlCertificate)
        );
        assert_eq!(ledger, after);

        let mut corrupt = certificate;
        corrupt.votes[0].signature[0] ^= 1;
        assert_eq!(
            execute_fastlane_control(&mut ledger, &corrupt, 10),
            Err(FastSwapControlError::InvalidControlCertificate)
        );
        assert_eq!(ledger, after);

        let mut definition = AssetDefinition::new("fastlane-control-test", "issuer", "AUTH", 1, 6)
            .expect("issued asset");
        definition.requires_authorization = true;
        definition.freeze_enabled = true;
        let asset_id = FastAssetIdV1(
            hex_to_bytes(&definition.asset_id)
                .expect("asset id hex")
                .try_into()
                .expect("asset id width"),
        );
        let privileged = FastAssetRuleV1 {
            asset_id,
            asset_definition_hash: asset_definition_hash(&definition).expect("definition hash"),
            issuer_address: definition.issuer.clone(),
            issuer_control_pubkey: keys[0].public_key.clone(),
            requires_authorization: true,
            freeze_enabled: true,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 1,
            valid_through_height: 100,
        };
        ledger.asset_definitions.push(definition);
        register_fastlane_asset_rule(&mut ledger, privileged).expect("privileged rule");

        let mut permit = FastHolderPermitV1 {
            permit_id: FastHolderPermitIdV1::ZERO,
            asset_id,
            owner_pubkey: keys[5].public_key.clone(),
            valid_from_height: 1,
            valid_through_height: 100,
            consensus_receipt_digest: FastSwapOpaqueHashV1([81; 48]),
        };
        permit.permit_id = permit.computed_id().expect("permit id");
        let permit_action = FastLaneControlActionV1::RegisterHolderPermit {
            permit: permit.clone(),
        };
        let permit_digest = permit_action.digest().expect("permit digest");
        let permit_votes = keys
            .iter()
            .enumerate()
            .take(5)
            .map(|(index, key)| {
                let mut vote = FastLaneControlVoteV1 {
                    committee: ledger.fastswap_committees[0].domain.clone(),
                    action_digest: permit_digest,
                    validator_id: format!("validator-{index}"),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context(
                    &key.private_key,
                    &vote.signing_bytes().expect("permit vote bytes"),
                    FASTLANE_CONTROL_CONTEXT_V1,
                )
                .expect("permit vote");
                vote
            })
            .collect();
        execute_fastlane_control(
            &mut ledger,
            &FastLaneControlCertificateV1 {
                action: permit_action,
                votes: permit_votes,
            },
            10,
        )
        .expect("holder permit control");
        assert_eq!(ledger.fast_lane_holder_permits, vec![permit.clone()]);
        register_fastlane_holder_permit(&mut ledger, permit.clone()).expect("idempotent replay");
        let mut tampered = permit;
        tampered.valid_through_height -= 1;
        assert!(
            FastLaneControlActionV1::RegisterHolderPermit { permit: tampered }
                .digest()
                .is_err()
        );

        let activation = FastLaneControlActionV1::ActivateProtocol {
            activation_height: 20,
        };
        let activation_digest = activation.digest().expect("activation digest");
        let activation_votes = keys
            .iter()
            .enumerate()
            .take(5)
            .map(|(index, key)| {
                let mut vote = FastLaneControlVoteV1 {
                    committee: ledger.fastswap_committees[0].domain.clone(),
                    action_digest: activation_digest,
                    validator_id: format!("validator-{index}"),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context(
                    &key.private_key,
                    &vote.signing_bytes().expect("activation vote bytes"),
                    FASTLANE_CONTROL_CONTEXT_V1,
                )
                .expect("activation vote");
                vote
            })
            .collect();
        execute_fastlane_control(
            &mut ledger,
            &FastLaneControlCertificateV1 {
                action: activation,
                votes: activation_votes,
            },
            10,
        )
        .expect("future activation");
        assert_eq!(ledger.fastswap_activation_height, Some(20));
    }

    #[test]
    fn governance_bootstrap_binds_payload_and_installs_epoch_one_atomically() {
        let keys = (0..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 90; 32]))
            .collect::<Vec<_>>();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: "fastswap-bootstrap-test".to_owned(),
                    genesis_hash: FastSwapOpaqueHashV1([91; 48]),
                    protocol_version: 1,
                },
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 4,
                quorum: 3,
            },
            validators: keys
                .iter()
                .enumerate()
                .map(|(index, key)| FastSwapValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: key.public_key.clone(),
                })
                .collect(),
        };
        committee.domain.committee_root = committee.computed_root().expect("committee root");
        let rule = FastAssetRuleV1 {
            asset_id: FastAssetIdV1::native_pft(),
            asset_definition_hash: FastAssetDefinitionHashV1::ZERO,
            issuer_address: "native".to_owned(),
            issuer_control_pubkey: vec![1],
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
            fast_lane_enabled: true,
            valid_from_height: 1,
            valid_through_height: 100,
        };
        let payload = FastSwapGovernanceBootstrapPayloadV1 {
            committee: committee.clone(),
            asset_rules: vec![rule.clone()],
            policies: Vec::new(),
            activation_height: 20,
        };
        let bootstrap_id = payload.bootstrap_id().expect("bootstrap id");
        let amendment = GovernanceAmendment {
            amendment_id: "bootstrap-amendment".to_owned(),
            chain_id: committee.domain.chain.chain_id.clone(),
            genesis_hash: postfiat_crypto_provider::bytes_to_hex(
                &committee.domain.chain.genesis_hash.0,
            ),
            protocol_version: 1,
            instance_id: "instance".to_owned(),
            proposal_id: "proposal".to_owned(),
            certificate_id: "certificate".to_owned(),
            proposer: "validator-0".to_owned(),
            validators: (0..4).map(|index| format!("validator-{index}")).collect(),
            quorum: 3,
            kind: format!(
                "{}{}",
                FASTSWAP_GOVERNANCE_BOOTSTRAP_KIND_PREFIX_V1,
                postfiat_crypto_provider::bytes_to_hex(&bootstrap_id.0)
            ),
            value: FASTSWAP_SCHEMA_VERSION_V1,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            support: (0..3).map(|index| format!("validator-{index}")).collect(),
            votes: Vec::new(),
            signed_authorizations: Vec::new(),
        };
        let bootstrap = FastSwapGovernanceBootstrapV1 { amendment, payload };
        let mut ledger = LedgerState::empty();
        execute_fastswap_governance_bootstrap(&mut ledger, &bootstrap, 10)
            .expect("bootstrap epoch one");
        assert_eq!(ledger.fastswap_committees, vec![committee]);
        assert_eq!(ledger.fast_lane_asset_rules, vec![rule]);
        assert_eq!(ledger.fastswap_activation_height, Some(20));

        let after = ledger.clone();
        assert_eq!(
            execute_fastswap_governance_bootstrap(&mut ledger, &bootstrap, 10),
            Err(FastSwapControlError::BootstrapConflict)
        );
        assert_eq!(ledger, after);

        let mut tampered = bootstrap;
        tampered.payload.activation_height = 21;
        let mut fresh = LedgerState::empty();
        assert_eq!(
            execute_fastswap_governance_bootstrap(&mut fresh, &tampered, 10),
            Err(FastSwapControlError::InvalidBootstrap)
        );
        assert_eq!(fresh, LedgerState::empty());
    }
}
