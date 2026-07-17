use postfiat_crypto_provider::{hash_bytes, ml_dsa_65_verify_with_context};
use postfiat_types::{
    FastSwapCertificateDigestV1, FastSwapCertificateV1, FastSwapCodecError, FastSwapCommitteeV1,
    FastSwapDecisionV1, FastSwapEffectsDigestV1, FastSwapEquivocationEvidenceV1, FastSwapIdV1,
    FastSwapNewRoundCertificateV1, FastSwapPhaseV1, FastSwapProposalV1, FASTSWAP_VOTE_CONTEXT_V1,
};
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum FastSwapDecisionError {
    Codec(FastSwapCodecError),
    CertificateBelowQuorum,
    UnknownValidator,
    InvalidVoteSignature,
    MixedCertificate,
    StaleRound,
    UnsafeValueChange,
    TerminalConflict,
    CancelNotObjectivelyValid,
    InvalidLeader,
    InvalidNewRoundCertificate,
    RecoveryTerminalAlreadyKnown,
    NotEquivocation,
}

impl From<FastSwapCodecError> for FastSwapDecisionError {
    fn from(value: FastSwapCodecError) -> Self {
        Self::Codec(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedFastSwapCertificateV1 {
    pub digest: FastSwapCertificateDigestV1,
    pub swap_id: FastSwapIdV1,
    pub phase: FastSwapPhaseV1,
    pub round: u64,
    pub decision: Option<FastSwapDecisionV1>,
    pub effects_digest: FastSwapEffectsDigestV1,
    pub signer_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifiedFastSwapNewRoundCertificateV1 {
    pub digest: FastSwapCertificateDigestV1,
    pub swap_id: FastSwapIdV1,
    pub target_round: u64,
    pub effects_digest: FastSwapEffectsDigestV1,
    pub highest_lock: Option<(u64, FastSwapDecisionV1, FastSwapCertificateDigestV1)>,
    pub terminal: Option<(FastSwapDecisionV1, FastSwapCertificateDigestV1)>,
}

pub fn verify_fastswap_new_round_certificate(
    committee: &FastSwapCommitteeV1,
    certificate: &FastSwapNewRoundCertificateV1,
) -> Result<VerifiedFastSwapNewRoundCertificateV1, FastSwapDecisionError> {
    committee.validate()?;
    certificate.validate_canonical_order()?;
    if certificate.votes.len() < usize::from(committee.domain.quorum) {
        return Err(FastSwapDecisionError::CertificateBelowQuorum);
    }
    let first = certificate
        .votes
        .first()
        .ok_or(FastSwapDecisionError::CertificateBelowQuorum)?;
    let mut highest_lock = None;
    let mut terminal = None;
    for vote in &certificate.votes {
        if vote.domain != committee.domain
            || vote.domain != first.domain
            || vote.swap_id != first.swap_id
            || vote.target_round != first.target_round
            || vote.effects_digest != first.effects_digest
        {
            return Err(FastSwapDecisionError::InvalidNewRoundCertificate);
        }
        verify_fastswap_new_round_vote(committee, vote)?;
        if let (Some(round), Some(value), Some(digest)) = (
            vote.locked_round,
            vote.locked_value,
            vote.locked_certificate_digest,
        ) {
            if highest_lock.is_none_or(|(highest, _, _)| round > highest) {
                highest_lock = Some((round, value, digest));
            } else if highest_lock.is_some_and(|(highest, locked_value, locked_digest)| {
                round == highest && (value != locked_value || digest != locked_digest)
            }) {
                return Err(FastSwapDecisionError::InvalidNewRoundCertificate);
            }
        }
        if let (Some(value), Some(digest)) =
            (vote.terminal_decision, vote.terminal_certificate_digest)
        {
            if terminal.is_some_and(|known| known != (value, digest)) {
                return Err(FastSwapDecisionError::InvalidNewRoundCertificate);
            }
            terminal = Some((value, digest));
        }
    }
    Ok(VerifiedFastSwapNewRoundCertificateV1 {
        digest: certificate.digest()?,
        swap_id: first.swap_id,
        target_round: first.target_round,
        effects_digest: first.effects_digest,
        highest_lock,
        terminal,
    })
}

pub fn verify_fastswap_new_round_vote(
    committee: &FastSwapCommitteeV1,
    vote: &postfiat_types::FastSwapNewRoundVoteV1,
) -> Result<(), FastSwapDecisionError> {
    committee.validate()?;
    if vote.domain != committee.domain
        || vote.target_round == 0
        || vote.target_round <= vote.highest_voted_round
        || (vote.locked_round.is_some()
            != (vote.locked_value.is_some() && vote.locked_certificate_digest.is_some()))
        || (vote.terminal_decision.is_some() != vote.terminal_certificate_digest.is_some())
    {
        return Err(FastSwapDecisionError::InvalidNewRoundCertificate);
    }
    let validator = committee
        .validators
        .iter()
        .find(|validator| validator.validator_id == vote.validator_id)
        .ok_or(FastSwapDecisionError::UnknownValidator)?;
    if !ml_dsa_65_verify_with_context(
        &validator.public_key,
        &vote.signing_bytes()?,
        &vote.signature,
        FASTSWAP_VOTE_CONTEXT_V1,
    ) {
        return Err(FastSwapDecisionError::InvalidVoteSignature);
    }
    Ok(())
}

pub fn verify_fastswap_vote(
    committee: &FastSwapCommitteeV1,
    vote: &postfiat_types::FastSwapVoteV1,
) -> Result<(), FastSwapDecisionError> {
    committee.validate()?;
    if vote.domain != committee.domain {
        return Err(FastSwapDecisionError::MixedCertificate);
    }
    let validator = committee
        .validators
        .iter()
        .find(|validator| validator.validator_id == vote.validator_id)
        .ok_or(FastSwapDecisionError::UnknownValidator)?;
    if !ml_dsa_65_verify_with_context(
        &validator.public_key,
        &vote.signing_bytes()?,
        &vote.signature,
        FASTSWAP_VOTE_CONTEXT_V1,
    ) {
        return Err(FastSwapDecisionError::InvalidVoteSignature);
    }
    Ok(())
}

pub fn verify_fastswap_equivocation(
    committee: &FastSwapCommitteeV1,
    evidence: &FastSwapEquivocationEvidenceV1,
) -> Result<(), FastSwapDecisionError> {
    verify_fastswap_vote(committee, &evidence.first)?;
    verify_fastswap_vote(committee, &evidence.second)?;
    if evidence.first.validator_id != evidence.second.validator_id
        || evidence.first.domain != evidence.second.domain
        || evidence.first.swap_id != evidence.second.swap_id
        || evidence.first.phase != evidence.second.phase
        || evidence.first.round != evidence.second.round
        || evidence.first.signing_bytes()? == evidence.second.signing_bytes()?
    {
        return Err(FastSwapDecisionError::NotEquivocation);
    }
    Ok(())
}

pub fn verify_fastswap_certificate(
    committee: &FastSwapCommitteeV1,
    certificate: &FastSwapCertificateV1,
) -> Result<VerifiedFastSwapCertificateV1, FastSwapDecisionError> {
    committee.validate()?;
    certificate.validate_canonical_order()?;
    if certificate.votes.len() < usize::from(committee.domain.quorum) {
        return Err(FastSwapDecisionError::CertificateBelowQuorum);
    }
    let first = certificate
        .votes
        .first()
        .ok_or(FastSwapDecisionError::CertificateBelowQuorum)?;
    for vote in &certificate.votes {
        if vote.domain != committee.domain
            || vote.domain != first.domain
            || vote.swap_id != first.swap_id
            || vote.phase != first.phase
            || vote.round != first.round
            || vote.decision != first.decision
            || vote.justification_digest != first.justification_digest
            || vote.effects_digest != first.effects_digest
            || vote.receipt_digest != first.receipt_digest
        {
            return Err(FastSwapDecisionError::MixedCertificate);
        }
        verify_fastswap_vote(committee, vote)?;
    }
    Ok(VerifiedFastSwapCertificateV1 {
        digest: certificate.digest()?,
        swap_id: first.swap_id,
        phase: first.phase,
        round: first.round,
        decision: first.decision,
        effects_digest: first.effects_digest,
        signer_count: certificate.votes.len(),
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FastSwapDecisionMachineV1 {
    pub swap_id: FastSwapIdV1,
    pub effects_digest: FastSwapEffectsDigestV1,
    pub highest_voted_round: Option<u64>,
    pub lock_round: Option<u64>,
    pub lock_value: Option<FastSwapDecisionV1>,
    pub terminal: Option<FastSwapDecisionV1>,
}

impl FastSwapDecisionMachineV1 {
    pub fn new(swap_id: FastSwapIdV1, effects_digest: FastSwapEffectsDigestV1) -> Self {
        Self {
            swap_id,
            effects_digest,
            highest_voted_round: None,
            lock_round: None,
            lock_value: None,
            terminal: None,
        }
    }

    pub fn validate_proposal(
        &self,
        committee: &FastSwapCommitteeV1,
        proposal: &FastSwapProposalV1,
        objective_cancel_valid: bool,
    ) -> Result<(), FastSwapDecisionError> {
        if proposal.domain != committee.domain
            || proposal.swap_id != self.swap_id
            || proposal.effects_digest != self.effects_digest
        {
            return Err(FastSwapDecisionError::MixedCertificate);
        }
        if self.terminal.is_some() {
            return Err(FastSwapDecisionError::TerminalConflict);
        }
        if proposal.round == 0 {
            if proposal.new_round_qc.is_some() || proposal.decision != FastSwapDecisionV1::Confirm {
                return Err(FastSwapDecisionError::InvalidNewRoundCertificate);
            }
        } else {
            let new_round = proposal
                .new_round_qc
                .as_ref()
                .ok_or(FastSwapDecisionError::InvalidNewRoundCertificate)?;
            let verified_new_round = verify_fastswap_new_round_certificate(committee, new_round)?;
            if verified_new_round.swap_id != self.swap_id
                || verified_new_round.target_round != proposal.round
                || verified_new_round.effects_digest != self.effects_digest
                || proposal.leader_id != recovery_leader(committee, self.swap_id, proposal.round)?
            {
                return Err(FastSwapDecisionError::InvalidLeader);
            }
            if verified_new_round.terminal.is_some() {
                return Err(FastSwapDecisionError::RecoveryTerminalAlreadyKnown);
            }
            if let Some((locked_round, locked_value, locked_digest)) =
                verified_new_round.highest_lock
            {
                let justification = proposal
                    .justification
                    .as_ref()
                    .ok_or(FastSwapDecisionError::UnsafeValueChange)?;
                let verified = verify_fastswap_certificate(committee, justification)?;
                if justification.digest()? != locked_digest
                    || verified.phase != FastSwapPhaseV1::Precommit
                    || verified.round != locked_round
                    || verified.decision != Some(locked_value)
                    || verified.swap_id != self.swap_id
                    || verified.effects_digest != self.effects_digest
                    || proposal.decision != locked_value
                {
                    return Err(FastSwapDecisionError::UnsafeValueChange);
                }
            } else if proposal.decision == FastSwapDecisionV1::Confirm {
                let justification = proposal
                    .justification
                    .as_ref()
                    .ok_or(FastSwapDecisionError::UnsafeValueChange)?;
                let verified = verify_fastswap_certificate(committee, justification)?;
                if verified.phase != FastSwapPhaseV1::Precommit
                    || verified.decision != Some(FastSwapDecisionV1::Confirm)
                    || verified.swap_id != self.swap_id
                    || verified.effects_digest != self.effects_digest
                {
                    return Err(FastSwapDecisionError::UnsafeValueChange);
                }
            }
        }
        if self
            .highest_voted_round
            .is_some_and(|highest| proposal.round <= highest)
        {
            return Err(FastSwapDecisionError::StaleRound);
        }
        if proposal.decision == FastSwapDecisionV1::Cancel && !objective_cancel_valid {
            return Err(FastSwapDecisionError::CancelNotObjectivelyValid);
        }
        if let (Some(lock_round), Some(lock_value)) = (self.lock_round, self.lock_value) {
            if proposal.decision != lock_value {
                let Some(justification) = proposal.justification.as_ref() else {
                    return Err(FastSwapDecisionError::UnsafeValueChange);
                };
                let verified = verify_fastswap_certificate(committee, justification)?;
                if verified.phase != FastSwapPhaseV1::Precommit
                    || verified.round <= lock_round
                    || verified.decision != Some(proposal.decision)
                    || verified.swap_id != self.swap_id
                    || verified.effects_digest != self.effects_digest
                {
                    return Err(FastSwapDecisionError::UnsafeValueChange);
                }
            }
        }
        Ok(())
    }

    pub fn persist_precommit_vote(&mut self, proposal: &FastSwapProposalV1) {
        self.highest_voted_round = Some(proposal.round);
    }

    pub fn accept_precommit_qc(
        &mut self,
        committee: &FastSwapCommitteeV1,
        certificate: &FastSwapCertificateV1,
    ) -> Result<VerifiedFastSwapCertificateV1, FastSwapDecisionError> {
        let verified = verify_fastswap_certificate(committee, certificate)?;
        if verified.phase != FastSwapPhaseV1::Precommit
            || verified.swap_id != self.swap_id
            || verified.effects_digest != self.effects_digest
            || verified.decision.is_none()
        {
            return Err(FastSwapDecisionError::MixedCertificate);
        }
        if self
            .highest_voted_round
            .is_some_and(|highest| verified.round < highest)
            || self
                .lock_round
                .is_some_and(|locked| verified.round < locked)
        {
            return Err(FastSwapDecisionError::StaleRound);
        }
        if self.lock_round == Some(verified.round) && self.lock_value != verified.decision {
            return Err(FastSwapDecisionError::UnsafeValueChange);
        }
        self.lock_round = Some(verified.round);
        self.lock_value = verified.decision;
        Ok(verified)
    }

    pub fn accept_decision_qc(
        &mut self,
        committee: &FastSwapCommitteeV1,
        certificate: &FastSwapCertificateV1,
    ) -> Result<VerifiedFastSwapCertificateV1, FastSwapDecisionError> {
        let verified = verify_fastswap_certificate(committee, certificate)?;
        if verified.phase != FastSwapPhaseV1::Commit
            || verified.swap_id != self.swap_id
            || verified.effects_digest != self.effects_digest
            || verified.decision.is_none()
        {
            return Err(FastSwapDecisionError::MixedCertificate);
        }
        if self.lock_value != verified.decision
            || self.lock_round.is_none_or(|round| verified.round < round)
        {
            return Err(FastSwapDecisionError::UnsafeValueChange);
        }
        if self
            .terminal
            .is_some_and(|terminal| Some(terminal) != verified.decision)
        {
            return Err(FastSwapDecisionError::TerminalConflict);
        }
        self.terminal = verified.decision;
        Ok(verified)
    }
}

pub fn objective_cancel_valid(
    expires_at_height: u64,
    finalized_primary_height: u64,
    dual_owner_abort: bool,
) -> bool {
    dual_owner_abort || finalized_primary_height > expires_at_height
}

pub fn recovery_leader(
    committee: &FastSwapCommitteeV1,
    swap_id: FastSwapIdV1,
    round: u64,
) -> Result<&str, FastSwapDecisionError> {
    committee.validate()?;
    let mut preimage = Vec::with_capacity(56);
    preimage.extend_from_slice(&swap_id.0);
    preimage.extend_from_slice(&round.to_be_bytes());
    let digest = hash_bytes("postfiat.fastswap.leader.v1", &preimage);
    let prefix: [u8; 8] = digest[..8]
        .try_into()
        .map_err(|_| FastSwapDecisionError::InvalidLeader)?;
    let index = (u64::from_be_bytes(prefix) % committee.validators.len() as u64) as usize;
    committee
        .validators
        .get(index)
        .map(|validator| validator.validator_id.as_str())
        .ok_or(FastSwapDecisionError::InvalidLeader)
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context};
    use postfiat_types::{
        FastSwapChainDomainV1, FastSwapCommitteeDomainV1, FastSwapCommitteeRootV1,
        FastSwapOpaqueHashV1, FastSwapValidatorV1, FastSwapVoteV1,
    };
    use postfiat_types::{FastSwapNewRoundCertificateV1, FastSwapNewRoundVoteV1};

    struct CommitteeKeys {
        committee: FastSwapCommitteeV1,
        private_keys: Vec<Vec<u8>>,
    }

    fn committee() -> CommitteeKeys {
        let keys = (0..6)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index as u8 + 1; 32]))
            .collect::<Vec<_>>();
        let validators = keys
            .iter()
            .enumerate()
            .map(|(index, key)| FastSwapValidatorV1 {
                validator_id: format!("validator-{index}"),
                public_key: key.public_key.clone(),
            })
            .collect::<Vec<_>>();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: "test".to_owned(),
                    genesis_hash: FastSwapOpaqueHashV1([1; 48]),
                    protocol_version: 1,
                },
                fastswap_schema_version: 1,
                committee_epoch: 1,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 6,
                quorum: 5,
            },
            validators,
        };
        committee.domain.committee_root = committee.computed_root().expect("root");
        CommitteeKeys {
            committee,
            private_keys: keys
                .into_iter()
                .map(|key| key.private_key.to_vec())
                .collect(),
        }
    }

    fn certificate(
        keys: &CommitteeKeys,
        phase: FastSwapPhaseV1,
        round: u64,
        decision: FastSwapDecisionV1,
    ) -> FastSwapCertificateV1 {
        let mut votes = keys
            .committee
            .validators
            .iter()
            .zip(&keys.private_keys)
            .take(5)
            .map(|(validator, private_key)| {
                let mut vote = FastSwapVoteV1 {
                    domain: keys.committee.domain.clone(),
                    swap_id: FastSwapIdV1([2; 48]),
                    phase,
                    round,
                    decision: Some(decision),
                    justification_digest: None,
                    effects_digest: FastSwapEffectsDigestV1([3; 48]),
                    receipt_digest: None,
                    validator_id: validator.validator_id.clone(),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context(
                    private_key,
                    &vote.signing_bytes().expect("vote bytes"),
                    FASTSWAP_VOTE_CONTEXT_V1,
                )
                .expect("vote signature");
                vote
            })
            .collect::<Vec<_>>();
        votes.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
        FastSwapCertificateV1 { votes }
    }

    #[test]
    fn quorum_certificate_verification_is_all_or_nothing() {
        let keys = committee();
        let certificate = certificate(
            &keys,
            FastSwapPhaseV1::Precommit,
            0,
            FastSwapDecisionV1::Confirm,
        );
        assert_eq!(
            verify_fastswap_certificate(&keys.committee, &certificate)
                .expect("valid certificate")
                .signer_count,
            5
        );
        let mut under_quorum = certificate.clone();
        under_quorum.votes.pop();
        assert_eq!(
            verify_fastswap_certificate(&keys.committee, &under_quorum),
            Err(FastSwapDecisionError::CertificateBelowQuorum)
        );
        let mut duplicate = certificate.clone();
        duplicate.votes[4] = duplicate.votes[3].clone();
        assert!(verify_fastswap_certificate(&keys.committee, &duplicate).is_err());
        let mut mixed = certificate;
        mixed.votes[4].effects_digest.0[0] ^= 1;
        assert_eq!(
            verify_fastswap_certificate(&keys.committee, &mixed),
            Err(FastSwapDecisionError::MixedCertificate)
        );
    }

    #[test]
    fn stale_lock_qc_is_rejected_after_higher_round_vote() {
        let keys = committee();
        let mut machine =
            FastSwapDecisionMachineV1::new(FastSwapIdV1([2; 48]), FastSwapEffectsDigestV1([3; 48]));
        machine.highest_voted_round = Some(1);
        let lock_qc = certificate(
            &keys,
            FastSwapPhaseV1::Precommit,
            0,
            FastSwapDecisionV1::Confirm,
        );
        assert_eq!(
            machine.accept_precommit_qc(&keys.committee, &lock_qc),
            Err(FastSwapDecisionError::StaleRound)
        );
    }

    #[test]
    fn terminal_decision_cannot_change() {
        let keys = committee();
        let mut machine =
            FastSwapDecisionMachineV1::new(FastSwapIdV1([2; 48]), FastSwapEffectsDigestV1([3; 48]));
        let lock = certificate(
            &keys,
            FastSwapPhaseV1::Precommit,
            0,
            FastSwapDecisionV1::Confirm,
        );
        machine
            .accept_precommit_qc(&keys.committee, &lock)
            .expect("lock");
        let decision = certificate(
            &keys,
            FastSwapPhaseV1::Commit,
            0,
            FastSwapDecisionV1::Confirm,
        );
        machine
            .accept_decision_qc(&keys.committee, &decision)
            .expect("decision");
        let cancel = certificate(
            &keys,
            FastSwapPhaseV1::Commit,
            1,
            FastSwapDecisionV1::Cancel,
        );
        assert!(machine
            .accept_decision_qc(&keys.committee, &cancel)
            .is_err());
    }

    #[test]
    fn cancellation_uses_primary_height_not_wall_clock() {
        assert!(!objective_cancel_valid(100, 100, false));
        assert!(objective_cancel_valid(100, 101, false));
        assert!(objective_cancel_valid(100, 0, true));
    }

    #[test]
    fn recovery_leader_is_deterministic() {
        let keys = committee();
        let first = recovery_leader(&keys.committee, FastSwapIdV1([9; 48]), 7)
            .expect("leader")
            .to_owned();
        assert_eq!(
            recovery_leader(&keys.committee, FastSwapIdV1([9; 48]), 7).expect("same leader"),
            first
        );
    }

    #[test]
    fn new_round_qc_authorizes_objective_cancel_and_rejects_status_tampering() {
        let keys = committee();
        let swap_id = FastSwapIdV1([2; 48]);
        let effects_digest = FastSwapEffectsDigestV1([3; 48]);
        let mut votes = keys
            .committee
            .validators
            .iter()
            .zip(keys.private_keys.iter())
            .take(5)
            .map(|(validator, private_key)| {
                let mut vote = FastSwapNewRoundVoteV1 {
                    domain: keys.committee.domain.clone(),
                    swap_id,
                    target_round: 1,
                    highest_voted_round: 0,
                    locked_round: None,
                    locked_value: None,
                    locked_certificate_digest: None,
                    terminal_decision: None,
                    terminal_certificate_digest: None,
                    effects_digest,
                    validator_id: validator.validator_id.clone(),
                    signature: Vec::new(),
                };
                vote.signature = ml_dsa_65_sign_with_context(
                    private_key,
                    &vote.signing_bytes().expect("new-round bytes"),
                    FASTSWAP_VOTE_CONTEXT_V1,
                )
                .expect("new-round sign");
                vote
            })
            .collect::<Vec<_>>();
        votes.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
        let certificate = FastSwapNewRoundCertificateV1 { votes };
        verify_fastswap_new_round_certificate(&keys.committee, &certificate).expect("new-round QC");
        let leader = recovery_leader(&keys.committee, swap_id, 1)
            .expect("leader")
            .to_owned();
        let proposal = FastSwapProposalV1 {
            domain: keys.committee.domain.clone(),
            swap_id,
            round: 1,
            decision: FastSwapDecisionV1::Cancel,
            effects_digest,
            leader_id: leader,
            new_round_qc: Some(certificate.clone()),
            justification: None,
        };
        FastSwapDecisionMachineV1::new(swap_id, effects_digest)
            .validate_proposal(&keys.committee, &proposal, true)
            .expect("objective cancel proposal");

        let mut under_quorum = certificate.clone();
        under_quorum.votes.pop();
        assert_eq!(
            verify_fastswap_new_round_certificate(&keys.committee, &under_quorum),
            Err(FastSwapDecisionError::CertificateBelowQuorum)
        );
        let mut duplicate_validator = certificate.clone();
        duplicate_validator.votes[4] = duplicate_validator.votes[3].clone();
        assert!(
            verify_fastswap_new_round_certificate(&keys.committee, &duplicate_validator).is_err()
        );

        let mut tampered = certificate;
        tampered.votes[0].highest_voted_round = 1;
        assert_eq!(
            verify_fastswap_new_round_certificate(&keys.committee, &tampered),
            Err(FastSwapDecisionError::InvalidNewRoundCertificate)
        );
    }

    #[test]
    fn conflicting_signed_votes_produce_verifiable_equivocation_evidence() {
        let keys = committee();
        let certificate = certificate(
            &keys,
            FastSwapPhaseV1::Precommit,
            0,
            FastSwapDecisionV1::Confirm,
        );
        let first = certificate.votes[0].clone();
        let mut second = first.clone();
        second.effects_digest.0[0] ^= 1;
        second.signature = ml_dsa_65_sign_with_context(
            &keys.private_keys[0],
            &second.signing_bytes().expect("conflicting vote bytes"),
            FASTSWAP_VOTE_CONTEXT_V1,
        )
        .expect("conflicting vote signature");
        verify_fastswap_equivocation(
            &keys.committee,
            &FastSwapEquivocationEvidenceV1 {
                first: first.clone(),
                second,
            },
        )
        .expect("equivocation evidence");
        assert_eq!(
            verify_fastswap_equivocation(
                &keys.committee,
                &FastSwapEquivocationEvidenceV1 {
                    first: first.clone(),
                    second: first,
                },
            ),
            Err(FastSwapDecisionError::NotEquivocation)
        );
    }
}
