use postfiat_execution::fastswap_bridge::verify_fastlane_exit_certificate;
use postfiat_execution::fastswap_decision::{
    verify_fastswap_certificate, verify_fastswap_new_round_certificate,
    verify_fastswap_new_round_vote, verify_fastswap_vote,
};
use postfiat_types::{
    FastLaneExitCertificateV1, FastLaneExitEffectsV1, FastLaneExitVoteV1, FastSwapCertificateV1,
    FastSwapCommitteeV1, FastSwapDecisionV1, FastSwapEffectsV1, FastSwapNewRoundCertificateV1,
    FastSwapNewRoundVoteV1, FastSwapObjectsResponseV1, FastSwapPhaseV1, FastSwapPolicyResponseV1,
    FastSwapVoteV1,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastSwapWalletError {
    InvalidVote,
    BelowQuorum { valid: usize, required: usize },
    MixedVoteSet,
    InvalidLockQc,
    InvalidDecisionQc,
    InvalidEffectsQc,
    EffectsMismatch,
    ReceiptNotAccepted,
    InvalidExitCertificate,
    InconsistentReadViews,
}

pub fn reconcile_fastswap_object_views(
    committee: &FastSwapCommitteeV1,
    responses: impl IntoIterator<Item = FastSwapObjectsResponseV1>,
) -> Result<FastSwapObjectsResponseV1, FastSwapWalletError> {
    let mut groups = BTreeMap::<Vec<u8>, (usize, FastSwapObjectsResponseV1)>::new();
    let committee_ids = committee
        .validators
        .iter()
        .map(|validator| validator.validator_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for response in responses {
        if !committee_ids.contains(response.validator_id.as_str())
            || !seen.insert(response.validator_id.clone())
            || response.committee != committee.domain
            || !response
                .objects
                .windows(2)
                .all(|pair| pair[0].key < pair[1].key)
        {
            continue;
        }
        let mut key = Vec::new();
        for object in &response.objects {
            let bytes = object
                .canonical_bytes()
                .map_err(|_| FastSwapWalletError::InconsistentReadViews)?;
            key.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            key.extend_from_slice(&bytes);
        }
        if let Some(cursor) = response.next_cursor {
            key.extend_from_slice(&cursor.object_id.0);
            key.extend_from_slice(&cursor.version.to_be_bytes());
        }
        groups
            .entry(key)
            .and_modify(|(count, _)| *count += 1)
            .or_insert((1, response));
    }
    groups
        .into_values()
        .filter(|(count, _)| *count >= usize::from(committee.domain.quorum))
        .max_by_key(|(count, _)| *count)
        .map(|(_, response)| response)
        .ok_or(FastSwapWalletError::InconsistentReadViews)
}

pub fn reconcile_fastswap_policy_views(
    committee: &FastSwapCommitteeV1,
    responses: impl IntoIterator<Item = FastSwapPolicyResponseV1>,
) -> Result<FastSwapPolicyResponseV1, FastSwapWalletError> {
    let mut groups = BTreeMap::<
        Option<postfiat_types::FastSwapPolicyHashV1>,
        (usize, FastSwapPolicyResponseV1),
    >::new();
    let committee_ids = committee
        .validators
        .iter()
        .map(|validator| validator.validator_id.as_str())
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for response in responses {
        if !committee_ids.contains(response.validator_id.as_str())
            || !seen.insert(response.validator_id.clone())
            || response.policy.as_ref().is_some_and(|policy| {
                policy.validate().is_err() || policy.domain != committee.domain.chain
            })
        {
            continue;
        }
        let key = response.policy.as_ref().map(|policy| policy.policy_hash);
        groups
            .entry(key)
            .and_modify(|(count, _)| *count += 1)
            .or_insert((1, response));
    }
    groups
        .into_values()
        .filter(|(count, _)| *count >= usize::from(committee.domain.quorum))
        .max_by_key(|(count, _)| *count)
        .map(|(_, response)| response)
        .ok_or(FastSwapWalletError::InconsistentReadViews)
}

pub fn aggregate_fastlane_exit_votes(
    committee: &FastSwapCommitteeV1,
    effects: &FastLaneExitEffectsV1,
    votes: impl IntoIterator<Item = FastLaneExitVoteV1>,
) -> Result<FastLaneExitCertificateV1, FastSwapWalletError> {
    let expected_digest = effects
        .digest()
        .map_err(|_| FastSwapWalletError::InvalidExitCertificate)?;
    let distinct = votes
        .into_iter()
        .filter(|vote| {
            vote.committee == committee.domain
                && vote.exit_id == effects.exit_id
                && vote.effects_digest == expected_digest
        })
        .map(|vote| (vote.validator_id.clone(), vote))
        .collect::<BTreeMap<_, _>>();
    let required = usize::from(committee.domain.quorum);
    if distinct.len() < required {
        return Err(FastSwapWalletError::BelowQuorum {
            valid: distinct.len(),
            required,
        });
    }
    let certificate = FastLaneExitCertificateV1 {
        effects: effects.clone(),
        votes: distinct.into_values().take(required).collect(),
    };
    verify_fastlane_exit_certificate(committee, &certificate)
        .map_err(|_| FastSwapWalletError::InvalidExitCertificate)?;
    Ok(certificate)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedFastSwapTerminalV1 {
    pub lock_qc: FastSwapCertificateV1,
    pub decision_qc: FastSwapCertificateV1,
    pub effects_qc: FastSwapCertificateV1,
    pub effects: FastSwapEffectsV1,
}

pub fn aggregate_fastswap_votes(
    committee: &FastSwapCommitteeV1,
    votes: impl IntoIterator<Item = FastSwapVoteV1>,
    phase: FastSwapPhaseV1,
) -> Result<FastSwapCertificateV1, FastSwapWalletError> {
    let mut groups = BTreeMap::<Vec<u8>, BTreeMap<String, FastSwapVoteV1>>::new();
    for vote in votes {
        if vote.phase != phase || verify_fastswap_vote(committee, &vote).is_err() {
            continue;
        }
        let mut value = vote.clone();
        value.validator_id.clear();
        value.signature.clear();
        let key = value
            .signing_bytes()
            .map_err(|_| FastSwapWalletError::InvalidVote)?;
        groups
            .entry(key)
            .or_default()
            .entry(vote.validator_id.clone())
            .or_insert(vote);
    }
    let required = usize::from(committee.domain.quorum);
    let best_valid = groups.values().map(BTreeMap::len).max().unwrap_or(0);
    let best = groups
        .into_values()
        .filter(|group| group.len() >= required)
        .max_by_key(BTreeMap::len);
    let Some(valid) = best else {
        return Err(FastSwapWalletError::BelowQuorum {
            valid: best_valid,
            required,
        });
    };
    let certificate = FastSwapCertificateV1 {
        votes: valid.into_values().take(required).collect(),
    };
    verify_fastswap_certificate(committee, &certificate)
        .map_err(|_| FastSwapWalletError::InvalidVote)?;
    Ok(certificate)
}

pub fn aggregate_fastswap_new_round_votes(
    committee: &FastSwapCommitteeV1,
    votes: impl IntoIterator<Item = FastSwapNewRoundVoteV1>,
) -> Result<FastSwapNewRoundCertificateV1, FastSwapWalletError> {
    let mut groups = BTreeMap::<Vec<u8>, BTreeMap<String, FastSwapNewRoundVoteV1>>::new();
    for vote in votes {
        if verify_fastswap_new_round_vote(committee, &vote).is_err() {
            continue;
        }
        let mut common = vote.clone();
        common.highest_voted_round = 0;
        common.locked_round = None;
        common.locked_value = None;
        common.locked_certificate_digest = None;
        common.terminal_decision = None;
        common.terminal_certificate_digest = None;
        common.validator_id.clear();
        common.signature.clear();
        let key = common
            .signing_bytes()
            .map_err(|_| FastSwapWalletError::InvalidVote)?;
        groups
            .entry(key)
            .or_default()
            .entry(vote.validator_id.clone())
            .or_insert(vote);
    }
    let required = usize::from(committee.domain.quorum);
    let best_valid = groups.values().map(BTreeMap::len).max().unwrap_or(0);
    let Some(distinct) = groups
        .into_values()
        .filter(|group| group.len() >= required)
        .max_by_key(BTreeMap::len)
    else {
        return Err(FastSwapWalletError::BelowQuorum {
            valid: best_valid,
            required,
        });
    };
    let certificate = FastSwapNewRoundCertificateV1 {
        votes: distinct.into_values().take(required).collect(),
    };
    verify_fastswap_new_round_certificate(committee, &certificate)
        .map_err(|_| FastSwapWalletError::InvalidVote)?;
    Ok(certificate)
}

pub fn verify_fastswap_terminal(
    committee: &FastSwapCommitteeV1,
    expected_effects: &FastSwapEffectsV1,
    lock_qc: &FastSwapCertificateV1,
    decision_qc: &FastSwapCertificateV1,
    effects_qc: &FastSwapCertificateV1,
) -> Result<VerifiedFastSwapTerminalV1, FastSwapWalletError> {
    verify_fastlane_terminal(
        committee,
        expected_effects,
        lock_qc,
        decision_qc,
        effects_qc,
        "fastswap_applied",
    )
}

pub fn verify_fast_asset_control_terminal(
    committee: &FastSwapCommitteeV1,
    expected_effects: &FastSwapEffectsV1,
    lock_qc: &FastSwapCertificateV1,
    decision_qc: &FastSwapCertificateV1,
    effects_qc: &FastSwapCertificateV1,
) -> Result<VerifiedFastSwapTerminalV1, FastSwapWalletError> {
    if expected_effects.policy_hash != postfiat_types::FastSwapPolicyHashV1::ZERO {
        return Err(FastSwapWalletError::EffectsMismatch);
    }
    verify_fastlane_terminal(
        committee,
        expected_effects,
        lock_qc,
        decision_qc,
        effects_qc,
        "fastlane_asset_control_applied",
    )
}

fn verify_fastlane_terminal(
    committee: &FastSwapCommitteeV1,
    expected_effects: &FastSwapEffectsV1,
    lock_qc: &FastSwapCertificateV1,
    decision_qc: &FastSwapCertificateV1,
    effects_qc: &FastSwapCertificateV1,
    receipt_code: &str,
) -> Result<VerifiedFastSwapTerminalV1, FastSwapWalletError> {
    if !expected_effects.receipt.accepted || expected_effects.receipt.code != receipt_code {
        return Err(FastSwapWalletError::ReceiptNotAccepted);
    }
    let expected_digest = expected_effects
        .digest()
        .map_err(|_| FastSwapWalletError::EffectsMismatch)?;
    let receipt_digest = expected_effects
        .receipt
        .digest()
        .map_err(|_| FastSwapWalletError::EffectsMismatch)?;
    let lock = verify_fastswap_certificate(committee, lock_qc)
        .map_err(|_| FastSwapWalletError::InvalidLockQc)?;
    if lock.phase != FastSwapPhaseV1::Precommit
        || lock.decision != Some(FastSwapDecisionV1::Confirm)
        || lock.effects_digest != expected_digest
        || lock.swap_id != expected_effects.swap_id
    {
        return Err(FastSwapWalletError::InvalidLockQc);
    }
    let decision = verify_fastswap_certificate(committee, decision_qc)
        .map_err(|_| FastSwapWalletError::InvalidDecisionQc)?;
    if decision.phase != FastSwapPhaseV1::Commit
        || decision.round != lock.round
        || decision.decision != Some(FastSwapDecisionV1::Confirm)
        || decision.effects_digest != expected_digest
        || decision.swap_id != expected_effects.swap_id
        || decision_qc
            .votes
            .iter()
            .any(|vote| vote.justification_digest != Some(lock.digest))
    {
        return Err(FastSwapWalletError::InvalidDecisionQc);
    }
    let effects = verify_fastswap_certificate(committee, effects_qc)
        .map_err(|_| FastSwapWalletError::InvalidEffectsQc)?;
    if effects.phase != FastSwapPhaseV1::Effects
        || effects.round != decision.round
        || effects.decision != Some(FastSwapDecisionV1::Confirm)
        || effects.effects_digest != expected_digest
        || effects.swap_id != expected_effects.swap_id
        || effects_qc.votes.iter().any(|vote| {
            vote.justification_digest != Some(decision.digest)
                || vote.receipt_digest != Some(receipt_digest)
        })
    {
        return Err(FastSwapWalletError::InvalidEffectsQc);
    }
    Ok(VerifiedFastSwapTerminalV1 {
        lock_qc: lock_qc.clone(),
        decision_qc: decision_qc.clone(),
        effects_qc: effects_qc.clone(),
        effects: expected_effects.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_types::{
        FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1,
        FastSwapOpaqueHashV1, FastSwapValidatorV1, FASTSWAP_SCHEMA_VERSION_V1,
    };

    fn committee() -> FastSwapCommitteeV1 {
        let validators = (0..4)
            .map(|index| FastSwapValidatorV1 {
                validator_id: format!("validator-{index}"),
                public_key: vec![index; 32],
            })
            .collect::<Vec<_>>();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: "fastswap-wallet-test".to_owned(),
                    genesis_hash: FastSwapOpaqueHashV1([7; 48]),
                    protocol_version: 1,
                },
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 4,
                quorum: 3,
            },
            validators,
        };
        committee.domain.committee_root = committee.computed_root().expect("committee root");
        committee
    }

    fn objects_response(
        committee: &FastSwapCommitteeV1,
        validator_id: &str,
    ) -> FastSwapObjectsResponseV1 {
        FastSwapObjectsResponseV1 {
            schema: "postfiat-fastswap-objects-v1".to_owned(),
            validator_id: validator_id.to_owned(),
            committee: committee.domain.clone(),
            objects: Vec::new(),
            next_cursor: None,
        }
    }

    fn policy_response(validator_id: &str) -> FastSwapPolicyResponseV1 {
        FastSwapPolicyResponseV1 {
            schema: "postfiat-fastswap-policy-v1".to_owned(),
            validator_id: validator_id.to_owned(),
            policy: None,
        }
    }

    #[test]
    fn read_quorum_requires_distinct_committee_validators() {
        let committee = committee();
        let duplicate_objects = (0..3)
            .map(|_| objects_response(&committee, "validator-0"))
            .collect::<Vec<_>>();
        assert_eq!(
            reconcile_fastswap_object_views(&committee, duplicate_objects),
            Err(FastSwapWalletError::InconsistentReadViews)
        );
        let foreign_policies = ["validator-0", "validator-1", "outsider"]
            .into_iter()
            .map(policy_response)
            .collect::<Vec<_>>();
        assert_eq!(
            reconcile_fastswap_policy_views(&committee, foreign_policies),
            Err(FastSwapWalletError::InconsistentReadViews)
        );

        let object_quorum = ["validator-0", "validator-1", "validator-2"]
            .into_iter()
            .map(|id| objects_response(&committee, id))
            .collect::<Vec<_>>();
        assert!(reconcile_fastswap_object_views(&committee, object_quorum).is_ok());
        let policy_quorum = ["validator-0", "validator-1", "validator-2"]
            .into_iter()
            .map(policy_response)
            .collect::<Vec<_>>();
        assert!(reconcile_fastswap_policy_views(&committee, policy_quorum).is_ok());
    }
}
