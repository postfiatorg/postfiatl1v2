//! Executable safety model for bounded FastPay payment recovery.
//!
//! Normal settlement remains consensusless. An owner-authorized order may be
//! applied through the owned lane only while its signed validity window is open
//! and only with an `n-f` validator certificate. Before acknowledging an
//! apply, an honest validator must durably persist the complete certificate.
//! Product finality requires `n-f` distinct durable-apply acknowledgements.
//!
//! Recovery is deliberately stricter than the discarded partial-vote design.
//! After expiry, a complete normal certificate may be revealed to the ordered
//! recovery lane during a bounded challenge window. At the end of that window,
//! the ordered lane either confirms the revealed certificate or cancels the
//! locked object version. Cancellation advances the version, so every delayed
//! certificate for the old version is permanently fenced. Partial votes can
//! never confirm recovery.
//!
//! The safety argument depends on four production invariants represented here:
//! 1. honest validators durably lock before voting and never vote twice for the
//!    same object version;
//! 2. two `n-f` certificates intersect in more than `f` validators;
//! 3. an apply acknowledgement is signed only after the full certificate and
//!    effect are durable;
//! 4. recovery decisions are ordered atomically with the object-version fence.

use std::collections::{BTreeMap, BTreeSet};

pub const FASTPAY_RECOVERY_SCHEMA_V1: &str = "postfiat-fastpay-recovery-v1";

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct VoteDomain {
    pub chain_id: String,
    pub genesis_hash: [u8; 48],
    pub protocol_version: u32,
    pub committee_epoch: u64,
    pub committee_root: [u8; 48],
    pub object_id: [u8; 32],
    pub object_version: u64,
    pub lock_id: [u8; 32],
    pub valid_from_height: u64,
    pub expires_at_height: u64,
    pub recovery_closes_at_height: u64,
}

impl VoteDomain {
    fn validate(&self, policy: RecoveryPolicy) -> Result<(), ModelError> {
        if self.chain_id.is_empty()
            || self.genesis_hash == [0; 48]
            || self.protocol_version == 0
            || self.committee_epoch == 0
            || self.committee_root == [0; 48]
            || self.object_id == [0; 32]
            || self.object_version == 0
            || self.lock_id == [0; 32]
            || self.valid_from_height == 0
            || self.expires_at_height < self.valid_from_height
            || self.recovery_closes_at_height <= self.expires_at_height
        {
            return Err(ModelError::InvalidDomain);
        }
        let validity = self
            .expires_at_height
            .checked_sub(self.valid_from_height)
            .ok_or(ModelError::InvalidDomain)?;
        let recovery = self
            .recovery_closes_at_height
            .checked_sub(self.expires_at_height)
            .ok_or(ModelError::InvalidDomain)?;
        if validity > policy.max_validity_blocks || recovery > policy.max_recovery_blocks {
            return Err(ModelError::UnboundedWindow);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableVote {
    pub validator: usize,
    pub domain: VoteDomain,
    pub order_digest: [u8; 48],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FullCertificate {
    pub schema: &'static str,
    pub domain: VoteDomain,
    pub order_digest: [u8; 48],
    pub certificate_digest: [u8; 48],
    pub votes: Vec<DurableVote>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DurableApplyAck {
    pub validator: usize,
    pub domain: VoteDomain,
    pub order_digest: [u8; 48],
    pub certificate_digest: [u8; 48],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RecoveryDecision {
    Confirm {
        order_digest: [u8; 48],
        certificate_digest: [u8; 48],
    },
    Cancel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecoveryDecisionCertificate {
    pub schema: &'static str,
    pub domain: VoteDomain,
    pub decision: RecoveryDecision,
    pub decided_at_height: u64,
    pub next_object_version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecoveryPolicy {
    pub validator_count: usize,
    pub max_byzantine: usize,
    pub normal_quorum: usize,
    pub max_validity_blocks: u64,
    pub max_recovery_blocks: u64,
}

impl RecoveryPolicy {
    pub fn new(
        validator_count: usize,
        max_validity_blocks: u64,
        max_recovery_blocks: u64,
    ) -> Result<Self, ModelError> {
        if validator_count < 4 || max_validity_blocks == 0 || max_recovery_blocks == 0 {
            return Err(ModelError::InvalidPolicy);
        }
        let max_byzantine = (validator_count - 1) / 3;
        let normal_quorum = validator_count - max_byzantine;
        // Two normal certificates must intersect in at least one honest
        // validator. `2q - n > f` is the exact set-intersection condition.
        if normal_quorum.saturating_mul(2) <= validator_count.saturating_add(max_byzantine) {
            return Err(ModelError::UnsafeCertificateIntersection);
        }
        Ok(Self {
            validator_count,
            max_byzantine,
            normal_quorum,
            max_validity_blocks,
            max_recovery_blocks,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ModelError {
    InvalidPolicy,
    UnsafeCertificateIntersection,
    InvalidDomain,
    UnboundedWindow,
    InvalidOrderDigest,
    InvalidCertificateDigest,
    CertificateUnavailable,
    UnknownValidator,
    DuplicateValidator,
    ConflictingHonestVote,
    DomainMismatch,
    BeforeValidityWindow,
    NormalCertificateExpired,
    UnderQuorum,
    RecoveryBeforeExpiry,
    RecoveryRevealWindowClosed,
    RecoveryRevealWindowOpen,
    ConflictingFullCertificates,
    TerminalDecisionConflict,
    VersionFenced,
    VersionOverflow,
}

/// Deterministic one-object recovery model. The production multi-input path
/// must execute all input fences and effects in one ordered storage transaction.
#[derive(Clone, Debug)]
pub struct RecoveryMachine {
    policy: RecoveryPolicy,
    domain: VoteDomain,
    honest_validators: BTreeSet<usize>,
    votes: Vec<DurableVote>,
    persisted_certificates: BTreeMap<[u8; 48], FullCertificate>,
    revealed_certificates: BTreeMap<[u8; 48], FullCertificate>,
    terminal: Option<RecoveryDecisionCertificate>,
}

impl RecoveryMachine {
    pub fn new(
        policy: RecoveryPolicy,
        domain: VoteDomain,
        honest_validators: BTreeSet<usize>,
    ) -> Result<Self, ModelError> {
        domain.validate(policy)?;
        if honest_validators.len() < policy.validator_count - policy.max_byzantine
            || honest_validators
                .iter()
                .any(|validator| *validator >= policy.validator_count)
        {
            return Err(ModelError::InvalidPolicy);
        }
        Ok(Self {
            policy,
            domain,
            honest_validators,
            votes: Vec::new(),
            persisted_certificates: BTreeMap::new(),
            revealed_certificates: BTreeMap::new(),
            terminal: None,
        })
    }

    pub fn record_vote(&mut self, height: u64, vote: DurableVote) -> Result<(), ModelError> {
        vote.domain.validate(self.policy)?;
        if height < self.domain.valid_from_height {
            return Err(ModelError::BeforeValidityWindow);
        }
        if height > self.domain.expires_at_height {
            return Err(ModelError::NormalCertificateExpired);
        }
        if vote.validator >= self.policy.validator_count {
            return Err(ModelError::UnknownValidator);
        }
        if vote.order_digest == [0; 48] {
            return Err(ModelError::InvalidOrderDigest);
        }
        if vote.domain != self.domain {
            return Err(ModelError::DomainMismatch);
        }
        if self.honest_validators.contains(&vote.validator)
            && self.votes.iter().any(|existing| {
                existing.validator == vote.validator && existing.order_digest != vote.order_digest
            })
        {
            return Err(ModelError::ConflictingHonestVote);
        }
        if !self.votes.contains(&vote) {
            self.votes.push(vote);
        }
        Ok(())
    }

    /// Models the validator's persist-before-apply boundary. A restart after
    /// this returns may lose the in-memory effect, but not the recoverable full
    /// certificate. No apply acknowledgement may be emitted at this stage.
    pub fn persist_certificate_before_apply(
        &mut self,
        height: u64,
        certificate: FullCertificate,
    ) -> Result<(), ModelError> {
        if height < self.domain.valid_from_height {
            return Err(ModelError::BeforeValidityWindow);
        }
        if height > self.domain.expires_at_height {
            return Err(ModelError::NormalCertificateExpired);
        }
        self.validate_certificate(&certificate)?;
        self.persisted_certificates
            .insert(certificate.certificate_digest, certificate);
        Ok(())
    }

    /// Applies only while the signed validity window is open. The complete
    /// certificate is persisted before the terminal effect is recorded.
    pub fn apply_normal_certificate(
        &mut self,
        height: u64,
        certificate: FullCertificate,
    ) -> Result<RecoveryDecisionCertificate, ModelError> {
        if let Some(terminal) = self.terminal.as_ref() {
            let same_confirm = matches!(
                terminal.decision,
                RecoveryDecision::Confirm {
                    order_digest,
                    certificate_digest,
                } if order_digest == certificate.order_digest
                    && certificate_digest == certificate.certificate_digest
            );
            return if same_confirm {
                Ok(terminal.clone())
            } else if terminal.decision == RecoveryDecision::Cancel {
                Err(ModelError::VersionFenced)
            } else {
                Err(ModelError::TerminalDecisionConflict)
            };
        }
        if height < self.domain.valid_from_height {
            return Err(ModelError::BeforeValidityWindow);
        }
        if height > self.domain.expires_at_height {
            return Err(ModelError::NormalCertificateExpired);
        }
        self.persist_certificate_before_apply(height, certificate.clone())?;
        self.record_terminal(
            height,
            RecoveryDecision::Confirm {
                order_digest: certificate.order_digest,
                certificate_digest: certificate.certificate_digest,
            },
        )
    }

    /// Persists a complete certificate in the ordered recovery lane. Partial
    /// votes are intentionally not accepted by this API.
    pub fn reveal_certificate(
        &mut self,
        height: u64,
        certificate: FullCertificate,
    ) -> Result<(), ModelError> {
        if height <= self.domain.expires_at_height {
            return Err(ModelError::RecoveryBeforeExpiry);
        }
        if height >= self.domain.recovery_closes_at_height {
            return Err(ModelError::RecoveryRevealWindowClosed);
        }
        self.validate_certificate(&certificate)?;
        self.persisted_certificates
            .insert(certificate.certificate_digest, certificate.clone());
        self.revealed_certificates
            .insert(certificate.certificate_digest, certificate);
        Ok(())
    }

    /// Recovery agents retrieve a complete certificate from durable validator
    /// state and submit it during the ordered reveal window.
    pub fn reveal_persisted_certificate(
        &mut self,
        height: u64,
        certificate_digest: [u8; 48],
    ) -> Result<(), ModelError> {
        let certificate = self
            .persisted_certificates
            .get(&certificate_digest)
            .cloned()
            .ok_or(ModelError::CertificateUnavailable)?;
        self.reveal_certificate(height, certificate)
    }

    /// Produces the single ordered consume-or-cancel decision at the challenge
    /// boundary. Any conflicting valid full certificates halt rather than
    /// selecting one; under the stated `f` fault bound they cannot exist.
    pub fn recover_after_reveal_window(
        &mut self,
        height: u64,
    ) -> Result<RecoveryDecisionCertificate, ModelError> {
        if let Some(terminal) = self.terminal.as_ref() {
            return Ok(terminal.clone());
        }
        if height < self.domain.recovery_closes_at_height {
            return Err(ModelError::RecoveryRevealWindowOpen);
        }
        let decisions = self
            .revealed_certificates
            .values()
            .map(|certificate| (certificate.order_digest, certificate.certificate_digest))
            .collect::<BTreeSet<_>>();
        let decision = match decisions.len() {
            0 => RecoveryDecision::Cancel,
            1 => {
                let (order_digest, certificate_digest) = decisions
                    .iter()
                    .next()
                    .copied()
                    .ok_or(ModelError::ConflictingFullCertificates)?;
                RecoveryDecision::Confirm {
                    order_digest,
                    certificate_digest,
                }
            }
            _ => return Err(ModelError::ConflictingFullCertificates),
        };
        self.record_terminal(height, decision)
    }

    /// A wallet may report product finality only after a normal quorum of
    /// distinct validators has acknowledged durable certificate+effect state.
    pub fn verify_product_finality(
        &self,
        certificate: &FullCertificate,
        acknowledgements: &[DurableApplyAck],
    ) -> Result<(), ModelError> {
        self.validate_certificate(certificate)?;
        let mut validators = BTreeSet::new();
        for acknowledgement in acknowledgements {
            if acknowledgement.validator >= self.policy.validator_count {
                return Err(ModelError::UnknownValidator);
            }
            if acknowledgement.domain != certificate.domain
                || acknowledgement.order_digest != certificate.order_digest
                || acknowledgement.certificate_digest != certificate.certificate_digest
            {
                return Err(ModelError::DomainMismatch);
            }
            if !validators.insert(acknowledgement.validator) {
                return Err(ModelError::DuplicateValidator);
            }
        }
        if validators.len() < self.policy.normal_quorum {
            return Err(ModelError::UnderQuorum);
        }
        Ok(())
    }

    pub fn terminal(&self) -> Option<&RecoveryDecisionCertificate> {
        self.terminal.as_ref()
    }

    fn validate_certificate(&self, certificate: &FullCertificate) -> Result<(), ModelError> {
        if certificate.schema != FASTPAY_RECOVERY_SCHEMA_V1 {
            return Err(ModelError::InvalidDomain);
        }
        certificate.domain.validate(self.policy)?;
        if certificate.domain != self.domain {
            return Err(ModelError::DomainMismatch);
        }
        if certificate.order_digest == [0; 48] {
            return Err(ModelError::InvalidOrderDigest);
        }
        if certificate.certificate_digest == [0; 48] {
            return Err(ModelError::InvalidCertificateDigest);
        }
        let mut validators = BTreeSet::new();
        for vote in &certificate.votes {
            if vote.validator >= self.policy.validator_count {
                return Err(ModelError::UnknownValidator);
            }
            if vote.domain != certificate.domain || vote.order_digest != certificate.order_digest {
                return Err(ModelError::DomainMismatch);
            }
            if !validators.insert(vote.validator) {
                return Err(ModelError::DuplicateValidator);
            }
        }
        if validators.len() < self.policy.normal_quorum {
            return Err(ModelError::UnderQuorum);
        }
        Ok(())
    }

    fn record_terminal(
        &mut self,
        height: u64,
        decision: RecoveryDecision,
    ) -> Result<RecoveryDecisionCertificate, ModelError> {
        if let Some(existing) = self.terminal.as_ref() {
            return if existing.decision == decision {
                Ok(existing.clone())
            } else {
                Err(ModelError::TerminalDecisionConflict)
            };
        }
        let next_object_version = self
            .domain
            .object_version
            .checked_add(1)
            .ok_or(ModelError::VersionOverflow)?;
        self.terminal = Some(RecoveryDecisionCertificate {
            schema: FASTPAY_RECOVERY_SCHEMA_V1,
            domain: self.domain.clone(),
            decision,
            decided_at_height: height,
            next_object_version,
        });
        self.terminal
            .as_ref()
            .cloned()
            .ok_or(ModelError::TerminalDecisionConflict)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ORDER_A: [u8; 48] = [0xAA; 48];
    const ORDER_B: [u8; 48] = [0xBB; 48];
    const CERT_A: [u8; 48] = [0xCA; 48];
    const CERT_B: [u8; 48] = [0xCB; 48];

    fn policy(validator_count: usize) -> RecoveryPolicy {
        RecoveryPolicy::new(validator_count, 20, 20).expect("safe policy")
    }

    fn domain() -> VoteDomain {
        VoteDomain {
            chain_id: "postfiat-fastpay-model".to_string(),
            genesis_hash: [0x11; 48],
            protocol_version: 3,
            committee_epoch: 7,
            committee_root: [0x22; 48],
            object_id: [0x33; 32],
            object_version: 9,
            lock_id: [0x44; 32],
            valid_from_height: 100,
            expires_at_height: 110,
            recovery_closes_at_height: 120,
        }
    }

    fn machine(validator_count: usize) -> RecoveryMachine {
        let max_byzantine = policy(validator_count).max_byzantine;
        RecoveryMachine::new(
            policy(validator_count),
            domain(),
            (0..validator_count - max_byzantine).collect(),
        )
        .expect("recovery machine")
    }

    fn vote(validator: usize, order_digest: [u8; 48]) -> DurableVote {
        DurableVote {
            validator,
            domain: domain(),
            order_digest,
        }
    }

    fn certificate(
        order_digest: [u8; 48],
        certificate_digest: [u8; 48],
        validators: impl IntoIterator<Item = usize>,
    ) -> FullCertificate {
        FullCertificate {
            schema: FASTPAY_RECOVERY_SCHEMA_V1,
            domain: domain(),
            order_digest,
            certificate_digest,
            votes: validators
                .into_iter()
                .map(|validator| vote(validator, order_digest))
                .collect(),
        }
    }

    fn acknowledgements(
        cert: &FullCertificate,
        validators: impl IntoIterator<Item = usize>,
    ) -> Vec<DurableApplyAck> {
        validators
            .into_iter()
            .map(|validator| DurableApplyAck {
                validator,
                domain: cert.domain.clone(),
                order_digest: cert.order_digest,
                certificate_digest: cert.certificate_digest,
            })
            .collect()
    }

    #[test]
    fn n4_and_n6_normal_quorums_have_honest_intersection() {
        let n4 = policy(4);
        assert_eq!((n4.max_byzantine, n4.normal_quorum), (1, 3));
        assert!(2 * n4.normal_quorum - n4.validator_count > n4.max_byzantine);
        let n6 = policy(6);
        assert_eq!((n6.max_byzantine, n6.normal_quorum), (1, 5));
        assert!(2 * n6.normal_quorum - n6.validator_count > n6.max_byzantine);
        assert_eq!(
            RecoveryPolicy::new(3, 20, 20),
            Err(ModelError::InvalidPolicy)
        );
    }

    #[test]
    fn full_certificate_confirms_and_product_finality_requires_distinct_quorum_acks() {
        let mut model = machine(6);
        let cert = certificate(ORDER_A, CERT_A, 0..5);
        let fence = model
            .apply_normal_certificate(110, cert.clone())
            .expect("normal certificate at expiry");
        assert_eq!(
            fence.decision,
            RecoveryDecision::Confirm {
                order_digest: ORDER_A,
                certificate_digest: CERT_A,
            }
        );
        assert_eq!(fence.next_object_version, 10);
        assert_eq!(
            model.verify_product_finality(&cert, &acknowledgements(&cert, 0..4)),
            Err(ModelError::UnderQuorum)
        );
        assert!(model
            .verify_product_finality(&cert, &acknowledgements(&cert, 0..5))
            .is_ok());
        assert_eq!(
            model
                .recover_after_reveal_window(120)
                .expect("confirmed recovery is idempotent")
                .decision,
            RecoveryDecision::Confirm {
                order_digest: ORDER_A,
                certificate_digest: CERT_A,
            }
        );
    }

    #[test]
    fn partial_locks_cancel_and_delayed_old_certificate_is_version_fenced() {
        let mut model = machine(6);
        for validator in 0..4 {
            model
                .record_vote(105, vote(validator, ORDER_A))
                .expect("partial vote");
        }
        assert_eq!(
            model
                .recover_after_reveal_window(120)
                .expect("partial lock cancellation")
                .decision,
            RecoveryDecision::Cancel
        );
        assert_eq!(model.terminal().expect("fence").next_object_version, 10);
        let delayed = certificate(ORDER_A, CERT_A, 0..5);
        assert_eq!(
            model.apply_normal_certificate(110, delayed.clone()),
            Err(ModelError::VersionFenced)
        );
        assert_eq!(
            model.reveal_certificate(121, delayed),
            Err(ModelError::RecoveryRevealWindowClosed)
        );
    }

    #[test]
    fn full_certificate_revealed_after_expiry_confirms_at_recovery_boundary() {
        let mut model = machine(6);
        let cert = certificate(ORDER_A, CERT_A, 0..5);
        assert_eq!(
            model.reveal_certificate(110, cert.clone()),
            Err(ModelError::RecoveryBeforeExpiry)
        );
        model
            .reveal_certificate(111, cert)
            .expect("full certificate reveal");
        assert_eq!(
            model.recover_after_reveal_window(119),
            Err(ModelError::RecoveryRevealWindowOpen)
        );
        assert_eq!(
            model
                .recover_after_reveal_window(120)
                .expect("recovered confirmation")
                .decision,
            RecoveryDecision::Confirm {
                order_digest: ORDER_A,
                certificate_digest: CERT_A,
            }
        );
    }

    #[test]
    fn withheld_broker_certificate_is_not_product_final_and_cancels_safely() {
        let mut model = machine(6);
        let cert = certificate(ORDER_A, CERT_A, 0..5);
        assert_eq!(
            model.verify_product_finality(&cert, &[]),
            Err(ModelError::UnderQuorum)
        );
        assert_eq!(
            model
                .recover_after_reveal_window(120)
                .expect("unseen certificate cancels")
                .decision,
            RecoveryDecision::Cancel
        );
        assert_eq!(
            model.apply_normal_certificate(109, cert),
            Err(ModelError::VersionFenced)
        );
    }

    #[test]
    fn duplicate_under_quorum_and_foreign_domain_certificates_fail_closed() {
        let model = machine(6);
        let under = certificate(ORDER_A, CERT_A, 0..4);
        assert_eq!(
            model.verify_product_finality(&under, &[]),
            Err(ModelError::UnderQuorum)
        );
        let duplicate = certificate(ORDER_A, CERT_A, [0, 1, 2, 3, 3]);
        assert_eq!(
            model.verify_product_finality(&duplicate, &[]),
            Err(ModelError::DuplicateValidator)
        );
        let mut foreign = certificate(ORDER_A, CERT_A, 0..5);
        foreign.domain.committee_root = [0x66; 48];
        assert_eq!(
            model.verify_product_finality(&foreign, &[]),
            Err(ModelError::DomainMismatch)
        );
    }

    #[test]
    fn restart_partition_and_committee_rotation_preserve_the_version_fence() {
        let mut before_restart = machine(6);
        let cert = certificate(ORDER_A, CERT_A, 0..5);
        before_restart
            .reveal_certificate(111, cert.clone())
            .expect("durable reveal");
        let mut restarted = before_restart.clone();
        assert_eq!(
            restarted
                .recover_after_reveal_window(120)
                .expect("restart retains reveal")
                .decision,
            RecoveryDecision::Confirm {
                order_digest: ORDER_A,
                certificate_digest: CERT_A,
            }
        );

        let mut next_domain = domain();
        next_domain.committee_epoch += 1;
        next_domain.committee_root = [0x77; 48];
        next_domain.object_version += 1;
        next_domain.lock_id = [0x88; 32];
        next_domain.valid_from_height = 121;
        next_domain.expires_at_height = 130;
        next_domain.recovery_closes_at_height = 140;
        let next = RecoveryMachine::new(policy(6), next_domain, (0..5).collect())
            .expect("next committee machine");
        assert_eq!(
            next.verify_product_finality(&cert, &acknowledgements(&cert, 0..5)),
            Err(ModelError::DomainMismatch)
        );
    }

    #[test]
    fn crash_after_certificate_persist_recovers_without_an_unsafe_unlock() {
        let mut before_crash = machine(6);
        let cert = certificate(ORDER_A, CERT_A, 0..5);
        before_crash
            .persist_certificate_before_apply(109, cert)
            .expect("certificate persisted before apply");

        // A crash loses no durable certificate. After restart, a recovery
        // agent retrieves the exact record and submits it in the reveal window.
        let mut restarted = before_crash.clone();
        assert!(restarted.terminal().is_none());
        restarted
            .reveal_persisted_certificate(111, CERT_A)
            .expect("durable certificate retrieved after restart");
        assert_eq!(
            restarted
                .recover_after_reveal_window(120)
                .expect("persisted certificate confirms")
                .decision,
            RecoveryDecision::Confirm {
                order_digest: ORDER_A,
                certificate_digest: CERT_A,
            }
        );
    }

    #[test]
    fn delayed_votes_and_certificates_cannot_cross_expiry() {
        let mut model = machine(6);
        assert_eq!(
            model.record_vote(111, vote(0, ORDER_A)),
            Err(ModelError::NormalCertificateExpired)
        );
        let cert = certificate(ORDER_A, CERT_A, 0..5);
        assert_eq!(
            model.persist_certificate_before_apply(111, cert.clone()),
            Err(ModelError::NormalCertificateExpired)
        );
        assert_eq!(
            model.apply_normal_certificate(111, cert),
            Err(ModelError::NormalCertificateExpired)
        );
    }

    #[test]
    fn conflicting_honest_votes_and_conflicting_full_certificates_halt() {
        let mut model = machine(6);
        model
            .record_vote(105, vote(0, ORDER_A))
            .expect("honest vote");
        assert_eq!(
            model.record_vote(105, vote(0, ORDER_B)),
            Err(ModelError::ConflictingHonestVote)
        );

        // This input violates the f=1 assumption by fabricating two valid
        // full certificates. Recovery must halt instead of choosing either.
        let cert_a = certificate(ORDER_A, CERT_A, 0..5);
        let cert_b = certificate(ORDER_B, CERT_B, 1..6);
        model
            .reveal_certificate(111, cert_a)
            .expect("first full certificate");
        model
            .reveal_certificate(112, cert_b)
            .expect("second full certificate");
        assert_eq!(
            model.recover_after_reveal_window(120),
            Err(ModelError::ConflictingFullCertificates)
        );
    }

    fn exhaustive_quorum_check(validator_count: usize) {
        let recovery_policy = policy(validator_count);
        let honest_count = validator_count - recovery_policy.max_byzantine;
        let assignments = 3_u32.pow(u32::try_from(honest_count).expect("small model"));
        for honest_assignment in 0..assignments {
            for byzantine_assignment in 0_u8..4 {
                let mut digits = honest_assignment;
                let mut votes_a = BTreeSet::new();
                let mut votes_b = BTreeSet::new();
                for validator in 0..honest_count {
                    match digits % 3 {
                        1 => {
                            votes_a.insert(validator);
                        }
                        2 => {
                            votes_b.insert(validator);
                        }
                        _ => {}
                    }
                    digits /= 3;
                }
                let byzantine = honest_count;
                if byzantine_assignment & 1 != 0 {
                    votes_a.insert(byzantine);
                }
                if byzantine_assignment & 2 != 0 {
                    votes_b.insert(byzantine);
                }
                assert!(
                    votes_a.len() < recovery_policy.normal_quorum
                        || votes_b.len() < recovery_policy.normal_quorum,
                    "two conflicting quorums with n={validator_count}"
                );
            }
        }
    }

    #[test]
    fn exhaustive_n4_and_n6_byzantine_assignments_never_form_two_full_certificates() {
        exhaustive_quorum_check(4);
        exhaustive_quorum_check(6);
    }

    #[test]
    fn validity_and_recovery_windows_are_bounded() {
        let recovery_policy = policy(6);
        let mut unbounded = domain();
        unbounded.expires_at_height = 121;
        unbounded.recovery_closes_at_height = 130;
        assert_eq!(
            RecoveryMachine::new(recovery_policy, unbounded, (0..5).collect())
                .expect_err("validity must be bounded"),
            ModelError::UnboundedWindow
        );
    }
}
