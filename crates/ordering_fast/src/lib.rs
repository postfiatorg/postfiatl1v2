#![allow(clippy::too_many_arguments)]

use std::collections::{BTreeMap, BTreeSet};

use postfiat_crypto_provider::{
    hash_hex, hex_to_bytes, ml_dsa_65_verify_with_context, ADMISSION_RECEIPT_SIGNATURE_CONTEXT,
};
use postfiat_mempool_dag::BatchReference;
use serde::{Deserialize, Serialize};

mod consensus_v2;
pub use consensus_v2::*;

pub const CRATE_PURPOSE: &str = "fast transaction batch ordering";
pub const HOTSTUFF_PROPOSAL_HASH_DOMAIN: &str = "postfiat.hotstuff.proposal.v1";
pub const HOTSTUFF_VOTE_HASH_DOMAIN: &str = "postfiat.hotstuff.vote.v1";
pub const HOTSTUFF_QC_HASH_DOMAIN: &str = "postfiat.hotstuff.qc.v1";
pub const HOTSTUFF_TIMEOUT_VOTE_HASH_DOMAIN: &str = "postfiat.hotstuff.timeout_vote.v1";
pub const HOTSTUFF_TC_HASH_DOMAIN: &str = "postfiat.hotstuff.tc.v1";
pub const HOTSTUFF_EQUIVOCATION_HASH_DOMAIN: &str = "postfiat.hotstuff.equivocation.v1";
pub const HOTSTUFF_SIMULATION_PAYLOAD_HASH_DOMAIN: &str = "postfiat.hotstuff.simulation_payload.v1";
pub const ADMISSION_RECEIPT_HASH_DOMAIN: &str = "postfiat.admission_receipt.v1";
pub const ADMISSION_RECEIPT_AGGREGATE_HASH_DOMAIN: &str = "postfiat.admission_receipt_aggregate.v1";
pub const OMISSION_EVIDENCE_HASH_DOMAIN: &str = "postfiat.omission_evidence.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderingError {
    message: String,
}

impl OrderingError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl std::fmt::Display for OrderingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for OrderingError {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusDomain {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorSet {
    pub validators: Vec<String>,
    pub quorum: usize,
}

impl ValidatorSet {
    pub fn try_new(validators: Vec<String>) -> Result<Self, OrderingError> {
        let validators = canonical_validators(validators)?;
        let quorum = bft_quorum_threshold(validators.len())?;
        Ok(Self { validators, quorum })
    }

    pub fn contains(&self, validator: &str) -> bool {
        self.validators
            .binary_search_by(|candidate| candidate.as_str().cmp(validator))
            .is_ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ValidatorAdmissionKey {
    pub validator: String,
    pub public_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionReceipt {
    pub receipt_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub tx_hash: String,
    pub validator: String,
    pub observation_window: u64,
    pub bucket: u32,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdmissionReceiptAggregate {
    pub aggregate_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub tx_hash: String,
    pub observation_window: u64,
    pub conservative_bucket: u32,
    pub validators: Vec<String>,
    pub quorum: usize,
    pub receipts: Vec<AdmissionReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OmissionEvidence {
    pub evidence_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub tx_hash: String,
    pub accused_proposer: String,
    pub first_observation_window: u64,
    pub last_observation_window: u64,
    pub omitted_heights: Vec<u64>,
    pub threshold: usize,
    pub aggregate_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotstuffProposal {
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub height: u64,
    pub view: u64,
    pub parent_block_id: String,
    pub justify_qc_id: String,
    pub payload_hash: String,
    pub proposer: String,
}

impl HotstuffProposal {
    pub fn new(
        domain: &ConsensusDomain,
        height: u64,
        view: u64,
        parent_block_id: impl Into<String>,
        justify_qc_id: impl Into<String>,
        payload_hash: impl Into<String>,
        proposer: impl Into<String>,
    ) -> Self {
        Self {
            chain_id: domain.chain_id.clone(),
            genesis_hash: domain.genesis_hash.clone(),
            protocol_version: domain.protocol_version,
            height,
            view,
            parent_block_id: parent_block_id.into(),
            justify_qc_id: justify_qc_id.into(),
            payload_hash: payload_hash.into(),
            proposer: proposer.into(),
        }
    }

    pub fn proposal_id(&self) -> Result<String, OrderingError> {
        validate_proposal(self)?;
        Ok(hash_canonical(HOTSTUFF_PROPOSAL_HASH_DOMAIN, |bytes| {
            append_str_field(bytes, "chain_id", &self.chain_id);
            append_str_field(bytes, "genesis_hash", &self.genesis_hash);
            append_u32_field(bytes, "protocol_version", self.protocol_version);
            append_u64_field(bytes, "height", self.height);
            append_u64_field(bytes, "view", self.view);
            append_str_field(bytes, "parent_block_id", &self.parent_block_id);
            append_str_field(bytes, "justify_qc_id", &self.justify_qc_id);
            append_str_field(bytes, "payload_hash", &self.payload_hash);
            append_str_field(bytes, "proposer", &self.proposer);
        }))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HotstuffVote {
    pub vote_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub height: u64,
    pub view: u64,
    pub proposal_id: String,
    pub validator: String,
}

impl HotstuffVote {
    pub fn new(
        domain: &ConsensusDomain,
        proposal: &HotstuffProposal,
        validator: impl Into<String>,
    ) -> Result<Self, OrderingError> {
        let proposal_id = proposal.proposal_id()?;
        let validator = validator.into();
        let vote_id = hotstuff_vote_id(
            domain,
            proposal.height,
            proposal.view,
            &proposal_id,
            &validator,
        );
        Ok(Self {
            vote_id,
            chain_id: domain.chain_id.clone(),
            genesis_hash: domain.genesis_hash.clone(),
            protocol_version: domain.protocol_version,
            height: proposal.height,
            view: proposal.view,
            proposal_id,
            validator,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuorumCertificate {
    pub certificate_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub height: u64,
    pub view: u64,
    pub proposal_id: String,
    pub validators: Vec<String>,
    pub quorum: usize,
    pub votes: Vec<HotstuffVote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutVote {
    pub vote_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub height: u64,
    pub view: u64,
    pub high_qc_id: String,
    pub validator: String,
}

impl TimeoutVote {
    pub fn new(
        domain: &ConsensusDomain,
        height: u64,
        view: u64,
        high_qc_id: impl Into<String>,
        validator: impl Into<String>,
    ) -> Result<Self, OrderingError> {
        validate_domain(domain)?;
        if height == 0 {
            return Err(OrderingError::new("timeout vote height must be nonzero"));
        }
        let high_qc_id = high_qc_id.into();
        let validator = validator.into();
        validate_nonempty("timeout high_qc_id", &high_qc_id)?;
        validate_validator_id(&validator)?;
        let vote_id = timeout_vote_id(domain, height, view, &high_qc_id, &validator);
        Ok(Self {
            vote_id,
            chain_id: domain.chain_id.clone(),
            genesis_hash: domain.genesis_hash.clone(),
            protocol_version: domain.protocol_version,
            height,
            view,
            high_qc_id,
            validator,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TimeoutCertificate {
    pub certificate_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub height: u64,
    pub view: u64,
    pub high_qc_id: String,
    pub validators: Vec<String>,
    pub quorum: usize,
    pub votes: Vec<TimeoutVote>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EquivocationEvidence {
    pub evidence_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub height: u64,
    pub view: u64,
    pub validator: String,
    pub first_id: String,
    pub second_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderedBatchReference {
    pub height: u64,
    pub reference: BatchReference,
}

pub fn order_references(references: Vec<BatchReference>) -> Vec<OrderedBatchReference> {
    let mut references = references;
    references.sort_by(|left, right| {
        left.batch_id
            .cmp(&right.batch_id)
            .then(left.payload_hash.cmp(&right.payload_hash))
            .then(left.transaction_count.cmp(&right.transaction_count))
    });
    references.dedup();
    references
        .into_iter()
        .enumerate()
        .map(|(index, reference)| OrderedBatchReference {
            height: index as u64 + 1,
            reference,
        })
        .collect()
}

pub fn next_reference(references: Vec<BatchReference>) -> Option<BatchReference> {
    order_references(references)
        .into_iter()
        .next()
        .map(|ordered| ordered.reference)
}

pub fn bft_fault_tolerance(validator_count: usize) -> Result<usize, OrderingError> {
    if validator_count == 0 {
        return Err(OrderingError::new("validator set must be nonempty"));
    }
    Ok((validator_count - 1) / 3)
}

pub fn bft_quorum_threshold(validator_count: usize) -> Result<usize, OrderingError> {
    if validator_count == 0 {
        return Err(OrderingError::new("validator set must be nonempty"));
    }
    let doubled = validator_count
        .checked_mul(2)
        .ok_or_else(|| OrderingError::new("validator count overflow"))?;
    Ok((doubled / 3) + 1)
}

pub fn leader_for_view(
    validators: &[String],
    height: u64,
    view: u64,
) -> Result<String, OrderingError> {
    let validator_set = ValidatorSet::try_new(validators.to_vec())?;
    let count = validator_set.validators.len() as u64;
    let index = ((height % count) + (view % count)) % count;
    Ok(validator_set.validators[index as usize].clone())
}

pub fn certify_proposal(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    proposal: &HotstuffProposal,
    votes: Vec<HotstuffVote>,
) -> Result<QuorumCertificate, OrderingError> {
    validate_domain_matches_proposal(domain, proposal)?;
    let expected_leader =
        leader_for_view(&validator_set.validators, proposal.height, proposal.view)?;
    if proposal.proposer != expected_leader {
        return Err(OrderingError::new(format!(
            "proposal leader mismatch: expected {expected_leader}, got {}",
            proposal.proposer
        )));
    }
    let proposal_id = proposal.proposal_id()?;
    let votes = canonical_votes(
        domain,
        validator_set,
        proposal.height,
        proposal.view,
        &proposal_id,
        votes,
    )?;
    if votes.len() < validator_set.quorum {
        return Err(OrderingError::new(format!(
            "insufficient proposal votes: got {}, need {}",
            votes.len(),
            validator_set.quorum
        )));
    }
    let certificate_id = quorum_certificate_id(
        domain,
        proposal.height,
        proposal.view,
        &proposal_id,
        &validator_set.validators,
        validator_set.quorum,
        &votes,
    );
    Ok(QuorumCertificate {
        certificate_id,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        height: proposal.height,
        view: proposal.view,
        proposal_id,
        validators: validator_set.validators.clone(),
        quorum: validator_set.quorum,
        votes,
    })
}

pub fn certify_timeout(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    height: u64,
    view: u64,
    votes: Vec<TimeoutVote>,
) -> Result<TimeoutCertificate, OrderingError> {
    validate_domain(domain)?;
    if height == 0 {
        return Err(OrderingError::new(
            "timeout certificate height must be nonzero",
        ));
    }
    let votes = canonical_timeout_votes(domain, validator_set, height, view, votes)?;
    if votes.len() < validator_set.quorum {
        return Err(OrderingError::new(format!(
            "insufficient timeout votes: got {}, need {}",
            votes.len(),
            validator_set.quorum
        )));
    }
    let high_qc_id = highest_timeout_qc_id(&votes)?;
    let certificate_id = timeout_certificate_id(
        domain,
        height,
        view,
        &high_qc_id,
        &validator_set.validators,
        validator_set.quorum,
        &votes,
    );
    Ok(TimeoutCertificate {
        certificate_id,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        height,
        view,
        high_qc_id,
        validators: validator_set.validators.clone(),
        quorum: validator_set.quorum,
        votes,
    })
}

pub fn verify_quorum_certificate(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    certificate: &QuorumCertificate,
) -> Result<(), OrderingError> {
    validate_domain(domain)?;
    validate_certificate_domain(
        domain,
        &certificate.chain_id,
        &certificate.genesis_hash,
        certificate.protocol_version,
        "quorum certificate",
    )?;
    if certificate.validators != validator_set.validators {
        return Err(OrderingError::new(
            "quorum certificate validator set mismatch",
        ));
    }
    if certificate.quorum != validator_set.quorum {
        return Err(OrderingError::new("quorum certificate threshold mismatch"));
    }
    validate_nonempty("quorum certificate proposal_id", &certificate.proposal_id)?;
    let votes = canonical_votes(
        domain,
        validator_set,
        certificate.height,
        certificate.view,
        &certificate.proposal_id,
        certificate.votes.clone(),
    )?;
    if votes.len() < validator_set.quorum {
        return Err(OrderingError::new(format!(
            "insufficient quorum certificate votes: got {}, need {}",
            votes.len(),
            validator_set.quorum
        )));
    }
    if votes != certificate.votes {
        return Err(OrderingError::new(
            "quorum certificate votes are not canonical",
        ));
    }
    let expected = quorum_certificate_id(
        domain,
        certificate.height,
        certificate.view,
        &certificate.proposal_id,
        &validator_set.validators,
        validator_set.quorum,
        &certificate.votes,
    );
    if certificate.certificate_id != expected {
        return Err(OrderingError::new("quorum certificate id mismatch"));
    }
    Ok(())
}

pub fn verify_timeout_certificate(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    certificate: &TimeoutCertificate,
) -> Result<(), OrderingError> {
    validate_domain(domain)?;
    validate_certificate_domain(
        domain,
        &certificate.chain_id,
        &certificate.genesis_hash,
        certificate.protocol_version,
        "timeout certificate",
    )?;
    if certificate.validators != validator_set.validators {
        return Err(OrderingError::new(
            "timeout certificate validator set mismatch",
        ));
    }
    if certificate.quorum != validator_set.quorum {
        return Err(OrderingError::new("timeout certificate threshold mismatch"));
    }
    let votes = canonical_timeout_votes(
        domain,
        validator_set,
        certificate.height,
        certificate.view,
        certificate.votes.clone(),
    )?;
    if votes.len() < validator_set.quorum {
        return Err(OrderingError::new(format!(
            "insufficient timeout certificate votes: got {}, need {}",
            votes.len(),
            validator_set.quorum
        )));
    }
    if votes != certificate.votes {
        return Err(OrderingError::new(
            "timeout certificate votes are not canonical",
        ));
    }
    let expected_high_qc_id = highest_timeout_qc_id(&certificate.votes)?;
    if certificate.high_qc_id != expected_high_qc_id {
        return Err(OrderingError::new("timeout certificate high_qc mismatch"));
    }
    let expected = timeout_certificate_id(
        domain,
        certificate.height,
        certificate.view,
        &certificate.high_qc_id,
        &validator_set.validators,
        validator_set.quorum,
        &certificate.votes,
    );
    if certificate.certificate_id != expected {
        return Err(OrderingError::new("timeout certificate id mismatch"));
    }
    Ok(())
}

pub fn detect_proposal_equivocation(
    domain: &ConsensusDomain,
    first: &HotstuffProposal,
    second: &HotstuffProposal,
) -> Result<Option<EquivocationEvidence>, OrderingError> {
    validate_domain_matches_proposal(domain, first)?;
    validate_domain_matches_proposal(domain, second)?;
    if first.height != second.height
        || first.view != second.view
        || first.proposer != second.proposer
    {
        return Ok(None);
    }
    let first_id = first.proposal_id()?;
    let second_id = second.proposal_id()?;
    if first_id == second_id {
        return Ok(None);
    }
    Ok(Some(equivocation_evidence(
        domain,
        first.height,
        first.view,
        &first.proposer,
        first_id,
        second_id,
    )))
}

pub fn detect_vote_equivocation(
    domain: &ConsensusDomain,
    first: &HotstuffVote,
    second: &HotstuffVote,
) -> Result<Option<EquivocationEvidence>, OrderingError> {
    validate_vote_domain(domain, first)?;
    validate_vote_domain(domain, second)?;
    if first.height != second.height
        || first.view != second.view
        || first.validator != second.validator
    {
        return Ok(None);
    }
    if first.proposal_id == second.proposal_id {
        return Ok(None);
    }
    Ok(Some(equivocation_evidence(
        domain,
        first.height,
        first.view,
        &first.validator,
        first.proposal_id.clone(),
        second.proposal_id.clone(),
    )))
}

pub fn two_chain_commit_candidate(
    parent: &HotstuffProposal,
    child: &HotstuffProposal,
    child_qc: &QuorumCertificate,
) -> Result<Option<String>, OrderingError> {
    let parent_id = parent.proposal_id()?;
    let child_id = child.proposal_id()?;
    if child.height != parent.height.saturating_add(1) {
        return Ok(None);
    }
    if child.parent_block_id != parent_id {
        return Ok(None);
    }
    if child_qc.height != child.height
        || child_qc.view != child.view
        || child_qc.proposal_id != child_id
    {
        return Ok(None);
    }
    Ok(Some(parent_id))
}

pub fn verified_two_chain_commit_candidate(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    parent: &HotstuffProposal,
    child: &HotstuffProposal,
    child_qc: &QuorumCertificate,
) -> Result<Option<String>, OrderingError> {
    verify_quorum_certificate(domain, validator_set, child_qc)?;
    two_chain_commit_candidate(parent, child, child_qc)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderingSimulationScenario {
    pub heights: u64,
    pub max_views_per_height: u64,
    pub faults: Vec<OrderingAdversaryFault>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OrderingAdversaryFault {
    DelayProposal {
        height: u64,
        view: u64,
    },
    FailedLeader {
        height: u64,
        view: u64,
    },
    DuplicateVotes {
        height: u64,
        view: u64,
    },
    DropVotes {
        height: u64,
        view: u64,
        validators: Vec<String>,
    },
    PartitionVotes {
        height: u64,
        view: u64,
        partitions: Vec<Vec<String>>,
    },
    EquivocateProposal {
        height: u64,
        view: u64,
    },
    StaleVotes {
        height: u64,
        view: u64,
        stale_height: u64,
        stale_view: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderingSimulationReport {
    pub certified: Vec<QuorumCertificate>,
    pub timeouts: Vec<TimeoutCertificate>,
    pub commits: Vec<SimulatedCommit>,
    pub equivocations: Vec<EquivocationEvidence>,
    pub stalled_views: Vec<StalledView>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulatedCommit {
    pub height: u64,
    pub view: u64,
    pub proposal_id: String,
    pub parent_block_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StalledView {
    pub height: u64,
    pub view: u64,
    pub reason: String,
}

pub fn simulate_adversarial_ordering(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    scenario: &OrderingSimulationScenario,
) -> Result<OrderingSimulationReport, OrderingError> {
    validate_domain(domain)?;
    validate_simulation_scenario(validator_set, scenario)?;

    let byzantine_validators = validator_set
        .validators
        .iter()
        .take(bft_fault_tolerance(validator_set.validators.len())?)
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut report = OrderingSimulationReport {
        certified: Vec::new(),
        timeouts: Vec::new(),
        commits: Vec::new(),
        equivocations: Vec::new(),
        stalled_views: Vec::new(),
    };
    let mut parent_block_id = "genesis".to_string();
    let mut high_qc_id = "genesis-qc".to_string();
    let mut pending_parent = None::<HotstuffProposal>;

    for height in 1..=scenario.heights {
        let mut height_certified = false;
        for view in 0..scenario.max_views_per_height {
            let faults = scenario_faults_for(scenario, height, view);
            if faults.iter().any(|fault| {
                matches!(
                    fault,
                    OrderingAdversaryFault::DelayProposal { .. }
                        | OrderingAdversaryFault::FailedLeader { .. }
                )
            }) {
                let timeout_votes = validator_set
                    .validators
                    .iter()
                    .map(|validator| {
                        TimeoutVote::new(domain, height, view, high_qc_id.clone(), validator)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let timeout = certify_timeout(domain, validator_set, height, view, timeout_votes)?;
                verify_timeout_certificate(domain, validator_set, &timeout)?;
                report.timeouts.push(timeout);
                continue;
            }

            let proposal = simulated_proposal(
                domain,
                validator_set,
                height,
                view,
                &parent_block_id,
                &high_qc_id,
                "primary",
            )?;
            simulate_equivocation_faults(
                domain,
                validator_set,
                &proposal,
                &byzantine_validators,
                &faults,
                &mut report,
            )?;

            let voters = simulated_voters(validator_set, &faults)?;
            if voters.len() < validator_set.quorum {
                report.stalled_views.push(StalledView {
                    height,
                    view,
                    reason: format!(
                        "insufficient reachable voters: got {}, need {}",
                        voters.len(),
                        validator_set.quorum
                    ),
                });
                continue;
            }

            let mut votes = voters
                .iter()
                .map(|validator| HotstuffVote::new(domain, &proposal, validator))
                .collect::<Result<Vec<_>, _>>()?;
            if faults
                .iter()
                .any(|fault| matches!(fault, OrderingAdversaryFault::DuplicateVotes { .. }))
            {
                if let Some(duplicate) = votes.first().cloned() {
                    votes.push(duplicate);
                }
            }
            if let Some((stale_height, stale_view)) = stale_vote_target(&faults) {
                let stale = simulated_proposal(
                    domain,
                    validator_set,
                    stale_height,
                    stale_view,
                    &parent_block_id,
                    &high_qc_id,
                    "stale",
                )?;
                if let Some((validator, vote)) = voters.first().zip(votes.first_mut()) {
                    *vote = HotstuffVote::new(domain, &stale, validator)?;
                }
            }

            let qc = match certify_proposal(domain, validator_set, &proposal, votes) {
                Ok(qc) => qc,
                Err(error) => {
                    report.stalled_views.push(StalledView {
                        height,
                        view,
                        reason: error.to_string(),
                    });
                    continue;
                }
            };

            if let Some(parent) = pending_parent.as_ref() {
                if let Some(proposal_id) = verified_two_chain_commit_candidate(
                    domain,
                    validator_set,
                    parent,
                    &proposal,
                    &qc,
                )? {
                    report.commits.push(SimulatedCommit {
                        height: parent.height,
                        view: parent.view,
                        proposal_id,
                        parent_block_id: parent.parent_block_id.clone(),
                    });
                }
            }
            parent_block_id = proposal.proposal_id()?;
            high_qc_id = qc.certificate_id.clone();
            pending_parent = Some(proposal);
            report.certified.push(qc);
            height_certified = true;
            break;
        }
        if !height_certified {
            return Err(OrderingError::new(format!(
                "height {height} did not certify within {} views",
                scenario.max_views_per_height
            )));
        }
    }

    verify_no_conflicting_commits(&report.commits)?;
    Ok(report)
}

pub fn verify_no_conflicting_commits(commits: &[SimulatedCommit]) -> Result<(), OrderingError> {
    let mut by_height = BTreeMap::<u64, &str>::new();
    for commit in commits {
        validate_nonempty("simulated commit proposal_id", &commit.proposal_id)?;
        if let Some(existing) = by_height.insert(commit.height, &commit.proposal_id) {
            if existing != commit.proposal_id {
                return Err(OrderingError::new(format!(
                    "conflicting commits at height {}",
                    commit.height
                )));
            }
        }
    }
    Ok(())
}

pub fn admission_receipt_signing_bytes(
    domain: &ConsensusDomain,
    tx_hash: &str,
    validator: &str,
    observation_window: u64,
    bucket: u32,
) -> Result<Vec<u8>, OrderingError> {
    validate_domain(domain)?;
    validate_hash_like("admission receipt tx_hash", tx_hash)?;
    validate_validator_id(validator)?;
    if observation_window == 0 {
        return Err(OrderingError::new(
            "admission receipt observation_window must be nonzero",
        ));
    }
    Ok(hash_canonical(ADMISSION_RECEIPT_HASH_DOMAIN, |bytes| {
        append_domain_fields(bytes, domain);
        append_str_field(bytes, "tx_hash", tx_hash);
        append_str_field(bytes, "validator", validator);
        append_u64_field(bytes, "observation_window", observation_window);
        append_u32_field(bytes, "bucket", bucket);
    })
    .into_bytes())
}

pub fn admission_receipt_id(
    domain: &ConsensusDomain,
    tx_hash: &str,
    validator: &str,
    observation_window: u64,
    bucket: u32,
) -> Result<String, OrderingError> {
    let signing_bytes =
        admission_receipt_signing_bytes(domain, tx_hash, validator, observation_window, bucket)?;
    Ok(hash_hex("postfiat.admission_receipt.id.v1", &signing_bytes))
}

pub fn build_admission_receipt(
    domain: &ConsensusDomain,
    tx_hash: impl Into<String>,
    validator: impl Into<String>,
    observation_window: u64,
    bucket: u32,
    signature_hex: impl Into<String>,
) -> Result<AdmissionReceipt, OrderingError> {
    let tx_hash = tx_hash.into();
    let validator = validator.into();
    let signature_hex = signature_hex.into();
    validate_lower_hex("admission receipt signature", &signature_hex)?;
    let receipt_id =
        admission_receipt_id(domain, &tx_hash, &validator, observation_window, bucket)?;
    Ok(AdmissionReceipt {
        receipt_id,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        tx_hash,
        validator,
        observation_window,
        bucket,
        signature_hex,
    })
}

pub fn certify_admission_receipts(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    admission_keys: &[ValidatorAdmissionKey],
    tx_hash: &str,
    observation_window: u64,
    current_observation_window: u64,
    max_receipt_age_windows: u64,
    receipts: Vec<AdmissionReceipt>,
) -> Result<AdmissionReceiptAggregate, OrderingError> {
    validate_domain(domain)?;
    validate_hash_like("admission receipt aggregate tx_hash", tx_hash)?;
    if observation_window == 0 {
        return Err(OrderingError::new(
            "admission receipt aggregate observation_window must be nonzero",
        ));
    }
    validate_observation_window_age(
        observation_window,
        current_observation_window,
        max_receipt_age_windows,
    )?;
    let key_map = canonical_admission_keys(validator_set, admission_keys)?;
    let receipts = canonical_admission_receipts(
        domain,
        validator_set,
        &key_map,
        tx_hash,
        observation_window,
        current_observation_window,
        max_receipt_age_windows,
        receipts,
    )?;
    if receipts.len() < validator_set.quorum {
        return Err(OrderingError::new(format!(
            "insufficient admission receipts: got {}, need {}",
            receipts.len(),
            validator_set.quorum
        )));
    }
    let conservative_bucket = receipts
        .iter()
        .map(|receipt| receipt.bucket)
        .max()
        .ok_or_else(|| OrderingError::new("admission receipt aggregate has no receipts"))?;
    let aggregate_id = admission_receipt_aggregate_id(
        domain,
        tx_hash,
        observation_window,
        conservative_bucket,
        &validator_set.validators,
        validator_set.quorum,
        &receipts,
    );
    Ok(AdmissionReceiptAggregate {
        aggregate_id,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        tx_hash: tx_hash.to_string(),
        observation_window,
        conservative_bucket,
        validators: validator_set.validators.clone(),
        quorum: validator_set.quorum,
        receipts,
    })
}

pub fn verify_admission_receipt_aggregate(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    admission_keys: &[ValidatorAdmissionKey],
    aggregate: &AdmissionReceiptAggregate,
) -> Result<(), OrderingError> {
    validate_certificate_domain(
        domain,
        &aggregate.chain_id,
        &aggregate.genesis_hash,
        aggregate.protocol_version,
        "admission receipt aggregate",
    )?;
    if aggregate.validators != validator_set.validators {
        return Err(OrderingError::new(
            "admission receipt aggregate validator set mismatch",
        ));
    }
    if aggregate.quorum != validator_set.quorum {
        return Err(OrderingError::new(
            "admission receipt aggregate quorum mismatch",
        ));
    }
    let recertified = certify_admission_receipts(
        domain,
        validator_set,
        admission_keys,
        &aggregate.tx_hash,
        aggregate.observation_window,
        aggregate.observation_window,
        0,
        aggregate.receipts.clone(),
    )?;
    if recertified != *aggregate {
        return Err(OrderingError::new(
            "admission receipt aggregate id or body mismatch",
        ));
    }
    Ok(())
}

pub fn build_omission_evidence(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    admission_keys: &[ValidatorAdmissionKey],
    accused_proposer: impl Into<String>,
    aggregates: Vec<AdmissionReceiptAggregate>,
    omitted_heights: Vec<u64>,
    threshold: usize,
) -> Result<OmissionEvidence, OrderingError> {
    validate_domain(domain)?;
    let accused_proposer = accused_proposer.into();
    validate_validator_id(&accused_proposer)?;
    if !validator_set.contains(&accused_proposer) {
        return Err(OrderingError::new(format!(
            "omission evidence accused proposer `{accused_proposer}` is not in validator set"
        )));
    }
    if threshold == 0 {
        return Err(OrderingError::new(
            "omission evidence threshold must be nonzero",
        ));
    }
    let omitted_heights = canonical_omitted_heights(omitted_heights)?;
    if omitted_heights.len() < threshold {
        return Err(OrderingError::new(format!(
            "insufficient omitted heights: got {}, need {threshold}",
            omitted_heights.len()
        )));
    }
    if aggregates.len() < threshold {
        return Err(OrderingError::new(format!(
            "insufficient admission receipt aggregates: got {}, need {threshold}",
            aggregates.len()
        )));
    }
    let mut tx_hash = None::<String>;
    let mut by_window = BTreeMap::<u64, AdmissionReceiptAggregate>::new();
    for aggregate in aggregates {
        verify_admission_receipt_aggregate(domain, validator_set, admission_keys, &aggregate)?;
        match tx_hash.as_ref() {
            Some(existing) if existing != &aggregate.tx_hash => {
                return Err(OrderingError::new(
                    "omission evidence aggregate tx_hash mismatch",
                ));
            }
            None => tx_hash = Some(aggregate.tx_hash.clone()),
            _ => {}
        }
        if let Some(existing) = by_window.insert(aggregate.observation_window, aggregate.clone()) {
            if existing != aggregate {
                return Err(OrderingError::new(
                    "conflicting duplicate omission aggregate window",
                ));
            }
        }
    }
    if by_window.len() < threshold {
        return Err(OrderingError::new(format!(
            "insufficient distinct omission windows: got {}, need {threshold}",
            by_window.len()
        )));
    }
    let tx_hash = tx_hash.ok_or_else(|| {
        OrderingError::new("omission evidence requires at least one admission aggregate")
    })?;
    let first_observation_window = *by_window
        .keys()
        .next()
        .ok_or_else(|| OrderingError::new("omission evidence has no aggregate window"))?;
    let last_observation_window = *by_window
        .keys()
        .next_back()
        .ok_or_else(|| OrderingError::new("omission evidence has no aggregate window"))?;
    let aggregate_ids = by_window
        .values()
        .map(|aggregate| aggregate.aggregate_id.clone())
        .collect::<Vec<_>>();
    let evidence_id = omission_evidence_id(
        domain,
        &tx_hash,
        &accused_proposer,
        first_observation_window,
        last_observation_window,
        &omitted_heights,
        threshold,
        &aggregate_ids,
    );
    Ok(OmissionEvidence {
        evidence_id,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        tx_hash,
        accused_proposer,
        first_observation_window,
        last_observation_window,
        omitted_heights,
        threshold,
        aggregate_ids,
    })
}

fn validate_simulation_scenario(
    validator_set: &ValidatorSet,
    scenario: &OrderingSimulationScenario,
) -> Result<(), OrderingError> {
    if scenario.heights == 0 {
        return Err(OrderingError::new("simulation heights must be nonzero"));
    }
    if scenario.max_views_per_height == 0 {
        return Err(OrderingError::new(
            "simulation max_views_per_height must be nonzero",
        ));
    }
    for fault in &scenario.faults {
        let (height, view) = fault_height_view(fault);
        if height == 0 || height > scenario.heights {
            return Err(OrderingError::new(format!(
                "simulation fault height {height} out of range"
            )));
        }
        if view >= scenario.max_views_per_height {
            return Err(OrderingError::new(format!(
                "simulation fault view {view} out of range"
            )));
        }
        match fault {
            OrderingAdversaryFault::DropVotes { validators, .. } => {
                canonical_simulation_validators(validator_set, validators, "dropped validator")?;
            }
            OrderingAdversaryFault::StaleVotes {
                height,
                view,
                stale_height,
                stale_view,
            } => {
                if *stale_height == 0 || *stale_height > scenario.heights {
                    return Err(OrderingError::new(format!(
                        "simulation stale vote height {stale_height} out of range"
                    )));
                }
                if *stale_view >= scenario.max_views_per_height {
                    return Err(OrderingError::new(format!(
                        "simulation stale vote view {stale_view} out of range"
                    )));
                }
                if (*stale_height, *stale_view) >= (*height, *view) {
                    return Err(OrderingError::new(
                        "simulation stale vote target must precede fault target",
                    ));
                }
            }
            OrderingAdversaryFault::PartitionVotes { partitions, .. } => {
                if partitions.is_empty() {
                    return Err(OrderingError::new(
                        "simulation partition fault must include partitions",
                    ));
                }
                let mut seen = BTreeSet::new();
                for partition in partitions {
                    let partition_validators = canonical_simulation_validators(
                        validator_set,
                        partition,
                        "partition validator",
                    )?;
                    if partition_validators.is_empty() {
                        return Err(OrderingError::new(
                            "simulation partition must include validators",
                        ));
                    }
                    for validator in partition_validators {
                        if !seen.insert(validator.clone()) {
                            return Err(OrderingError::new(format!(
                                "validator `{validator}` appears in multiple partitions"
                            )));
                        }
                    }
                }
            }
            OrderingAdversaryFault::DelayProposal { .. }
            | OrderingAdversaryFault::FailedLeader { .. }
            | OrderingAdversaryFault::DuplicateVotes { .. }
            | OrderingAdversaryFault::EquivocateProposal { .. } => {}
        }
    }
    Ok(())
}

fn scenario_faults_for(
    scenario: &OrderingSimulationScenario,
    height: u64,
    view: u64,
) -> Vec<&OrderingAdversaryFault> {
    scenario
        .faults
        .iter()
        .filter(|fault| fault_height_view(fault) == (height, view))
        .collect()
}

fn fault_height_view(fault: &OrderingAdversaryFault) -> (u64, u64) {
    match fault {
        OrderingAdversaryFault::DelayProposal { height, view }
        | OrderingAdversaryFault::FailedLeader { height, view }
        | OrderingAdversaryFault::DuplicateVotes { height, view }
        | OrderingAdversaryFault::DropVotes { height, view, .. }
        | OrderingAdversaryFault::PartitionVotes { height, view, .. }
        | OrderingAdversaryFault::EquivocateProposal { height, view }
        | OrderingAdversaryFault::StaleVotes { height, view, .. } => (*height, *view),
    }
}

fn simulated_proposal(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    height: u64,
    view: u64,
    parent_block_id: &str,
    justify_qc_id: &str,
    payload_variant: &str,
) -> Result<HotstuffProposal, OrderingError> {
    Ok(HotstuffProposal::new(
        domain,
        height,
        view,
        parent_block_id,
        justify_qc_id,
        simulation_payload_hash(height, view, payload_variant),
        leader_for_view(&validator_set.validators, height, view)?,
    ))
}

fn simulation_payload_hash(height: u64, view: u64, payload_variant: &str) -> String {
    hash_hex(
        HOTSTUFF_SIMULATION_PAYLOAD_HASH_DOMAIN,
        format!("{height}:{view}:{payload_variant}").as_bytes(),
    )
}

fn simulate_equivocation_faults(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    proposal: &HotstuffProposal,
    byzantine_validators: &BTreeSet<String>,
    faults: &[&OrderingAdversaryFault],
    report: &mut OrderingSimulationReport,
) -> Result<(), OrderingError> {
    if !faults
        .iter()
        .any(|fault| matches!(fault, OrderingAdversaryFault::EquivocateProposal { .. }))
    {
        return Ok(());
    }
    let equivocated = simulated_proposal(
        domain,
        validator_set,
        proposal.height,
        proposal.view,
        &proposal.parent_block_id,
        &proposal.justify_qc_id,
        "equivocated",
    )?;
    if let Some(evidence) = detect_proposal_equivocation(domain, proposal, &equivocated)? {
        report.equivocations.push(evidence);
    }

    let mut byzantine_votes = Vec::new();
    for validator in byzantine_validators {
        let first = HotstuffVote::new(domain, proposal, validator)?;
        let second = HotstuffVote::new(domain, &equivocated, validator)?;
        if let Some(evidence) = detect_vote_equivocation(domain, &first, &second)? {
            report.equivocations.push(evidence);
        }
        byzantine_votes.push(second);
    }
    if certify_proposal(domain, validator_set, &equivocated, byzantine_votes).is_ok() {
        return Err(OrderingError::new(
            "byzantine-only equivocation unexpectedly certified",
        ));
    }
    Ok(())
}

fn simulated_voters(
    validator_set: &ValidatorSet,
    faults: &[&OrderingAdversaryFault],
) -> Result<Vec<String>, OrderingError> {
    let mut dropped = BTreeSet::new();
    let mut partition_voters = None::<Vec<String>>;
    for fault in faults {
        match fault {
            OrderingAdversaryFault::DropVotes { validators, .. } => {
                for validator in
                    canonical_simulation_validators(validator_set, validators, "dropped validator")?
                {
                    dropped.insert(validator);
                }
            }
            OrderingAdversaryFault::PartitionVotes { partitions, .. } => {
                partition_voters = Some(largest_partition_voters(validator_set, partitions)?);
            }
            OrderingAdversaryFault::DelayProposal { .. }
            | OrderingAdversaryFault::FailedLeader { .. }
            | OrderingAdversaryFault::DuplicateVotes { .. }
            | OrderingAdversaryFault::EquivocateProposal { .. }
            | OrderingAdversaryFault::StaleVotes { .. } => {}
        }
    }
    let voters = partition_voters.unwrap_or_else(|| validator_set.validators.clone());
    Ok(voters
        .into_iter()
        .filter(|validator| !dropped.contains(validator))
        .collect())
}

fn stale_vote_target(faults: &[&OrderingAdversaryFault]) -> Option<(u64, u64)> {
    faults.iter().find_map(|fault| match fault {
        OrderingAdversaryFault::StaleVotes {
            stale_height,
            stale_view,
            ..
        } => Some((*stale_height, *stale_view)),
        _ => None,
    })
}

fn largest_partition_voters(
    validator_set: &ValidatorSet,
    partitions: &[Vec<String>],
) -> Result<Vec<String>, OrderingError> {
    let mut largest = Vec::new();
    for partition in partitions {
        let partition_validators =
            canonical_simulation_validators(validator_set, partition, "partition validator")?;
        if partition_validators.len() > largest.len() {
            largest = partition_validators;
        }
    }
    Ok(largest)
}

fn canonical_simulation_validators(
    validator_set: &ValidatorSet,
    validators: &[String],
    label: &str,
) -> Result<Vec<String>, OrderingError> {
    let mut unique = BTreeSet::new();
    for validator in validators {
        validate_validator_id(validator)?;
        if !validator_set.contains(validator) {
            return Err(OrderingError::new(format!(
                "{label} `{validator}` is not in validator set"
            )));
        }
        if !unique.insert(validator.clone()) {
            return Err(OrderingError::new(format!(
                "duplicate {label} `{validator}`"
            )));
        }
    }
    Ok(unique.into_iter().collect())
}

fn canonical_validators(validators: Vec<String>) -> Result<Vec<String>, OrderingError> {
    let mut unique = BTreeSet::new();
    for validator in validators {
        validate_validator_id(&validator)?;
        if !unique.insert(validator.clone()) {
            return Err(OrderingError::new(format!(
                "duplicate validator `{validator}`"
            )));
        }
    }
    if unique.is_empty() {
        return Err(OrderingError::new("validator set must be nonempty"));
    }
    Ok(unique.into_iter().collect())
}

fn validate_domain(domain: &ConsensusDomain) -> Result<(), OrderingError> {
    validate_nonempty("chain_id", &domain.chain_id)?;
    validate_nonempty("genesis_hash", &domain.genesis_hash)?;
    if domain.protocol_version == 0 {
        return Err(OrderingError::new("protocol_version must be nonzero"));
    }
    Ok(())
}

fn validate_domain_matches_proposal(
    domain: &ConsensusDomain,
    proposal: &HotstuffProposal,
) -> Result<(), OrderingError> {
    validate_domain(domain)?;
    validate_proposal(proposal)?;
    if proposal.chain_id != domain.chain_id
        || proposal.genesis_hash != domain.genesis_hash
        || proposal.protocol_version != domain.protocol_version
    {
        return Err(OrderingError::new("proposal domain mismatch"));
    }
    Ok(())
}

fn validate_proposal(proposal: &HotstuffProposal) -> Result<(), OrderingError> {
    validate_nonempty("proposal chain_id", &proposal.chain_id)?;
    validate_nonempty("proposal genesis_hash", &proposal.genesis_hash)?;
    if proposal.protocol_version == 0 {
        return Err(OrderingError::new(
            "proposal protocol_version must be nonzero",
        ));
    }
    if proposal.height == 0 {
        return Err(OrderingError::new("proposal height must be nonzero"));
    }
    validate_nonempty("proposal parent_block_id", &proposal.parent_block_id)?;
    validate_nonempty("proposal justify_qc_id", &proposal.justify_qc_id)?;
    validate_nonempty("proposal payload_hash", &proposal.payload_hash)?;
    validate_validator_id(&proposal.proposer)?;
    Ok(())
}

fn validate_vote_domain(
    domain: &ConsensusDomain,
    vote: &HotstuffVote,
) -> Result<(), OrderingError> {
    validate_domain(domain)?;
    validate_certificate_domain(
        domain,
        &vote.chain_id,
        &vote.genesis_hash,
        vote.protocol_version,
        "vote",
    )
}

fn validate_certificate_domain(
    domain: &ConsensusDomain,
    chain_id: &str,
    genesis_hash: &str,
    protocol_version: u32,
    label: &str,
) -> Result<(), OrderingError> {
    if chain_id != domain.chain_id
        || genesis_hash != domain.genesis_hash
        || protocol_version != domain.protocol_version
    {
        return Err(OrderingError::new(format!("{label} domain mismatch")));
    }
    Ok(())
}

fn validate_nonempty(label: &str, value: &str) -> Result<(), OrderingError> {
    if value.trim().is_empty() {
        return Err(OrderingError::new(format!("{label} must be nonempty")));
    }
    Ok(())
}

fn validate_validator_id(validator: &str) -> Result<(), OrderingError> {
    validate_nonempty("validator", validator)?;
    if validator.chars().any(char::is_whitespace) {
        return Err(OrderingError::new(format!(
            "validator `{validator}` contains whitespace"
        )));
    }
    Ok(())
}

fn canonical_votes(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    height: u64,
    view: u64,
    proposal_id: &str,
    votes: Vec<HotstuffVote>,
) -> Result<Vec<HotstuffVote>, OrderingError> {
    let mut by_validator = BTreeMap::<String, HotstuffVote>::new();
    for vote in votes {
        validate_vote_domain(domain, &vote)?;
        if vote.height != height || vote.view != view || vote.proposal_id != proposal_id {
            return Err(OrderingError::new("vote target mismatch"));
        }
        if !validator_set.contains(&vote.validator) {
            return Err(OrderingError::new(format!(
                "vote from non-validator `{}`",
                vote.validator
            )));
        }
        let expected_vote_id = hotstuff_vote_id(domain, height, view, proposal_id, &vote.validator);
        if vote.vote_id != expected_vote_id {
            return Err(OrderingError::new("vote id mismatch"));
        }
        if let Some(existing) = by_validator.get(&vote.validator) {
            if existing == &vote {
                continue;
            }
            return Err(OrderingError::new("conflicting duplicate vote validator"));
        }
        by_validator.insert(vote.validator.clone(), vote);
    }
    Ok(votes_in_validator_order(validator_set, by_validator))
}

fn canonical_timeout_votes(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    height: u64,
    view: u64,
    votes: Vec<TimeoutVote>,
) -> Result<Vec<TimeoutVote>, OrderingError> {
    let mut by_validator = BTreeMap::<String, TimeoutVote>::new();
    for vote in votes {
        if vote.chain_id != domain.chain_id
            || vote.genesis_hash != domain.genesis_hash
            || vote.protocol_version != domain.protocol_version
            || vote.height != height
            || vote.view != view
        {
            return Err(OrderingError::new("timeout vote target mismatch"));
        }
        if !validator_set.contains(&vote.validator) {
            return Err(OrderingError::new(format!(
                "timeout vote from non-validator `{}`",
                vote.validator
            )));
        }
        let expected = TimeoutVote::new(
            domain,
            height,
            view,
            vote.high_qc_id.clone(),
            vote.validator.clone(),
        )?;
        if vote.vote_id != expected.vote_id {
            return Err(OrderingError::new("timeout vote id mismatch"));
        }
        if let Some(existing) = by_validator.get(&vote.validator) {
            if existing == &vote {
                continue;
            }
            return Err(OrderingError::new(
                "conflicting duplicate timeout vote validator",
            ));
        }
        by_validator.insert(vote.validator.clone(), vote);
    }
    Ok(timeout_votes_in_validator_order(
        validator_set,
        by_validator,
    ))
}

fn votes_in_validator_order(
    validator_set: &ValidatorSet,
    mut by_validator: BTreeMap<String, HotstuffVote>,
) -> Vec<HotstuffVote> {
    validator_set
        .validators
        .iter()
        .filter_map(|validator| by_validator.remove(validator))
        .collect()
}

fn timeout_votes_in_validator_order(
    validator_set: &ValidatorSet,
    mut by_validator: BTreeMap<String, TimeoutVote>,
) -> Vec<TimeoutVote> {
    validator_set
        .validators
        .iter()
        .filter_map(|validator| by_validator.remove(validator))
        .collect()
}

fn highest_timeout_qc_id(votes: &[TimeoutVote]) -> Result<String, OrderingError> {
    let high_qc_ids = votes
        .iter()
        .map(|vote| vote.high_qc_id.as_str())
        .collect::<BTreeSet<_>>();
    match high_qc_ids.len() {
        0 => Err(OrderingError::new("timeout certificate has no votes")),
        1 => Ok(high_qc_ids
            .into_iter()
            .next()
            .expect("one high-QC ID")
            .to_string()),
        _ => Err(OrderingError::new(
            "cannot rank heterogeneous opaque high_qc_id values; use typed verified QC references",
        )),
    }
}

fn hotstuff_vote_id(
    domain: &ConsensusDomain,
    height: u64,
    view: u64,
    proposal_id: &str,
    validator: &str,
) -> String {
    hash_canonical(HOTSTUFF_VOTE_HASH_DOMAIN, |bytes| {
        append_domain_fields(bytes, domain);
        append_u64_field(bytes, "height", height);
        append_u64_field(bytes, "view", view);
        append_str_field(bytes, "proposal_id", proposal_id);
        append_str_field(bytes, "validator", validator);
    })
}

fn timeout_vote_id(
    domain: &ConsensusDomain,
    height: u64,
    view: u64,
    high_qc_id: &str,
    validator: &str,
) -> String {
    hash_canonical(HOTSTUFF_TIMEOUT_VOTE_HASH_DOMAIN, |bytes| {
        append_domain_fields(bytes, domain);
        append_u64_field(bytes, "height", height);
        append_u64_field(bytes, "view", view);
        append_str_field(bytes, "high_qc_id", high_qc_id);
        append_str_field(bytes, "validator", validator);
    })
}

fn quorum_certificate_id(
    domain: &ConsensusDomain,
    height: u64,
    view: u64,
    proposal_id: &str,
    validators: &[String],
    quorum: usize,
    votes: &[HotstuffVote],
) -> String {
    hash_canonical(HOTSTUFF_QC_HASH_DOMAIN, |bytes| {
        append_domain_fields(bytes, domain);
        append_u64_field(bytes, "height", height);
        append_u64_field(bytes, "view", view);
        append_str_field(bytes, "proposal_id", proposal_id);
        append_string_list(bytes, "validator", validators);
        append_usize_field(bytes, "quorum", quorum);
        append_u64_field(bytes, "vote_count", votes.len() as u64);
        for vote in votes {
            append_hotstuff_vote(bytes, vote);
        }
    })
}

fn timeout_certificate_id(
    domain: &ConsensusDomain,
    height: u64,
    view: u64,
    high_qc_id: &str,
    validators: &[String],
    quorum: usize,
    votes: &[TimeoutVote],
) -> String {
    hash_canonical(HOTSTUFF_TC_HASH_DOMAIN, |bytes| {
        append_domain_fields(bytes, domain);
        append_u64_field(bytes, "height", height);
        append_u64_field(bytes, "view", view);
        append_str_field(bytes, "high_qc_id", high_qc_id);
        append_string_list(bytes, "validator", validators);
        append_usize_field(bytes, "quorum", quorum);
        append_u64_field(bytes, "vote_count", votes.len() as u64);
        for vote in votes {
            append_timeout_vote(bytes, vote);
        }
    })
}

fn equivocation_evidence(
    domain: &ConsensusDomain,
    height: u64,
    view: u64,
    validator: &str,
    first_id: String,
    second_id: String,
) -> EquivocationEvidence {
    let (first_id, second_id) = if first_id <= second_id {
        (first_id, second_id)
    } else {
        (second_id, first_id)
    };
    let evidence_id = hash_canonical(HOTSTUFF_EQUIVOCATION_HASH_DOMAIN, |bytes| {
        append_domain_fields(bytes, domain);
        append_u64_field(bytes, "height", height);
        append_u64_field(bytes, "view", view);
        append_str_field(bytes, "validator", validator);
        append_str_field(bytes, "first_id", &first_id);
        append_str_field(bytes, "second_id", &second_id);
    });
    EquivocationEvidence {
        evidence_id,
        chain_id: domain.chain_id.clone(),
        genesis_hash: domain.genesis_hash.clone(),
        protocol_version: domain.protocol_version,
        height,
        view,
        validator: validator.to_string(),
        first_id,
        second_id,
    }
}

fn canonical_admission_keys(
    validator_set: &ValidatorSet,
    keys: &[ValidatorAdmissionKey],
) -> Result<BTreeMap<String, Vec<u8>>, OrderingError> {
    let mut by_validator = BTreeMap::new();
    for key in keys {
        validate_validator_id(&key.validator)?;
        if !validator_set.contains(&key.validator) {
            return Err(OrderingError::new(format!(
                "admission key for non-validator `{}`",
                key.validator
            )));
        }
        validate_lower_hex("admission public key", &key.public_key_hex)?;
        let public_key = hex_to_bytes(&key.public_key_hex)
            .map_err(|error| OrderingError::new(error.to_string()))?;
        if let Some(existing) = by_validator.insert(key.validator.clone(), public_key.clone()) {
            if existing != public_key {
                return Err(OrderingError::new("conflicting duplicate admission key"));
            }
        }
    }
    Ok(by_validator)
}

fn canonical_admission_receipts(
    domain: &ConsensusDomain,
    validator_set: &ValidatorSet,
    key_map: &BTreeMap<String, Vec<u8>>,
    tx_hash: &str,
    observation_window: u64,
    current_observation_window: u64,
    max_receipt_age_windows: u64,
    receipts: Vec<AdmissionReceipt>,
) -> Result<Vec<AdmissionReceipt>, OrderingError> {
    let mut by_validator = BTreeMap::<String, AdmissionReceipt>::new();
    for receipt in receipts {
        validate_admission_receipt_domain(domain, &receipt)?;
        if receipt.tx_hash != tx_hash || receipt.observation_window != observation_window {
            return Err(OrderingError::new("admission receipt target mismatch"));
        }
        validate_observation_window_age(
            receipt.observation_window,
            current_observation_window,
            max_receipt_age_windows,
        )?;
        if !validator_set.contains(&receipt.validator) {
            return Err(OrderingError::new(format!(
                "admission receipt from non-validator `{}`",
                receipt.validator
            )));
        }
        let expected_id = admission_receipt_id(
            domain,
            &receipt.tx_hash,
            &receipt.validator,
            receipt.observation_window,
            receipt.bucket,
        )?;
        if receipt.receipt_id != expected_id {
            return Err(OrderingError::new("admission receipt id mismatch"));
        }
        validate_lower_hex("admission receipt signature", &receipt.signature_hex)?;
        let signature = hex_to_bytes(&receipt.signature_hex)
            .map_err(|error| OrderingError::new(error.to_string()))?;
        let public_key = key_map.get(&receipt.validator).ok_or_else(|| {
            OrderingError::new(format!(
                "missing admission public key for `{}`",
                receipt.validator
            ))
        })?;
        let signing_bytes = admission_receipt_signing_bytes(
            domain,
            &receipt.tx_hash,
            &receipt.validator,
            receipt.observation_window,
            receipt.bucket,
        )?;
        if !ml_dsa_65_verify_with_context(
            public_key,
            &signing_bytes,
            &signature,
            ADMISSION_RECEIPT_SIGNATURE_CONTEXT,
        ) {
            return Err(OrderingError::new(
                "admission receipt signature verification failed",
            ));
        }
        if let Some(existing) = by_validator.get(&receipt.validator) {
            if existing == &receipt {
                continue;
            }
            return Err(OrderingError::new(
                "conflicting duplicate admission receipt validator",
            ));
        }
        by_validator.insert(receipt.validator.clone(), receipt);
    }
    Ok(validator_set
        .validators
        .iter()
        .filter_map(|validator| by_validator.remove(validator))
        .collect())
}

fn validate_admission_receipt_domain(
    domain: &ConsensusDomain,
    receipt: &AdmissionReceipt,
) -> Result<(), OrderingError> {
    validate_certificate_domain(
        domain,
        &receipt.chain_id,
        &receipt.genesis_hash,
        receipt.protocol_version,
        "admission receipt",
    )?;
    validate_hash_like("admission receipt tx_hash", &receipt.tx_hash)?;
    validate_validator_id(&receipt.validator)?;
    if receipt.observation_window == 0 {
        return Err(OrderingError::new(
            "admission receipt observation_window must be nonzero",
        ));
    }
    Ok(())
}

fn validate_observation_window_age(
    observation_window: u64,
    current_observation_window: u64,
    max_receipt_age_windows: u64,
) -> Result<(), OrderingError> {
    if current_observation_window < observation_window {
        return Err(OrderingError::new(
            "admission receipt observation window is in the future",
        ));
    }
    if current_observation_window.saturating_sub(observation_window) > max_receipt_age_windows {
        return Err(OrderingError::new("admission receipt is stale"));
    }
    Ok(())
}

fn canonical_omitted_heights(heights: Vec<u64>) -> Result<Vec<u64>, OrderingError> {
    let mut unique = BTreeSet::new();
    for height in heights {
        if height == 0 {
            return Err(OrderingError::new(
                "omission evidence height must be nonzero",
            ));
        }
        if !unique.insert(height) {
            return Err(OrderingError::new("duplicate omission evidence height"));
        }
    }
    if unique.is_empty() {
        return Err(OrderingError::new(
            "omission evidence requires omitted heights",
        ));
    }
    Ok(unique.into_iter().collect())
}

fn admission_receipt_aggregate_id(
    domain: &ConsensusDomain,
    tx_hash: &str,
    observation_window: u64,
    conservative_bucket: u32,
    validators: &[String],
    quorum: usize,
    receipts: &[AdmissionReceipt],
) -> String {
    hash_canonical(ADMISSION_RECEIPT_AGGREGATE_HASH_DOMAIN, |bytes| {
        append_domain_fields(bytes, domain);
        append_str_field(bytes, "tx_hash", tx_hash);
        append_u64_field(bytes, "observation_window", observation_window);
        append_u32_field(bytes, "conservative_bucket", conservative_bucket);
        append_string_list(bytes, "validator_set", validators);
        append_usize_field(bytes, "quorum", quorum);
        append_u64_field(bytes, "receipt_count", receipts.len() as u64);
        for receipt in receipts {
            append_admission_receipt(bytes, receipt);
        }
    })
}

fn omission_evidence_id(
    domain: &ConsensusDomain,
    tx_hash: &str,
    accused_proposer: &str,
    first_observation_window: u64,
    last_observation_window: u64,
    omitted_heights: &[u64],
    threshold: usize,
    aggregate_ids: &[String],
) -> String {
    hash_canonical(OMISSION_EVIDENCE_HASH_DOMAIN, |bytes| {
        append_domain_fields(bytes, domain);
        append_str_field(bytes, "tx_hash", tx_hash);
        append_str_field(bytes, "accused_proposer", accused_proposer);
        append_u64_field(bytes, "first_observation_window", first_observation_window);
        append_u64_field(bytes, "last_observation_window", last_observation_window);
        append_u64_field(bytes, "omitted_height_count", omitted_heights.len() as u64);
        for height in omitted_heights {
            append_u64_field(bytes, "omitted_height", *height);
        }
        append_usize_field(bytes, "threshold", threshold);
        append_string_list(bytes, "aggregate_id", aggregate_ids);
    })
}

fn validate_hash_like(label: &str, value: &str) -> Result<(), OrderingError> {
    validate_nonempty(label, value)?;
    validate_lower_hex(label, value)?;
    if !value.len().is_multiple_of(2) {
        return Err(OrderingError::new(format!(
            "{label} must have even hex length"
        )));
    }
    if value.len() < 32 || value.len() > 128 {
        return Err(OrderingError::new(format!(
            "{label} hex length must be between 32 and 128"
        )));
    }
    Ok(())
}

fn validate_lower_hex(label: &str, value: &str) -> Result<(), OrderingError> {
    validate_nonempty(label, value)?;
    if !value
        .bytes()
        .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(OrderingError::new(format!("{label} must be lowercase hex")));
    }
    Ok(())
}

fn hash_canonical(domain: &str, write_fields: impl FnOnce(&mut Vec<u8>)) -> String {
    let mut bytes = Vec::new();
    write_fields(&mut bytes);
    hash_hex(domain, &bytes)
}

fn append_domain_fields(bytes: &mut Vec<u8>, domain: &ConsensusDomain) {
    append_str_field(bytes, "chain_id", &domain.chain_id);
    append_str_field(bytes, "genesis_hash", &domain.genesis_hash);
    append_u32_field(bytes, "protocol_version", domain.protocol_version);
}

fn append_string_list(bytes: &mut Vec<u8>, label: &str, values: &[String]) {
    append_u64_field(bytes, &format!("{label}_count"), values.len() as u64);
    for value in values {
        append_str_field(bytes, label, value);
    }
}

fn append_hotstuff_vote(bytes: &mut Vec<u8>, vote: &HotstuffVote) {
    append_str_field(bytes, "vote.vote_id", &vote.vote_id);
    append_str_field(bytes, "vote.chain_id", &vote.chain_id);
    append_str_field(bytes, "vote.genesis_hash", &vote.genesis_hash);
    append_u32_field(bytes, "vote.protocol_version", vote.protocol_version);
    append_u64_field(bytes, "vote.height", vote.height);
    append_u64_field(bytes, "vote.view", vote.view);
    append_str_field(bytes, "vote.proposal_id", &vote.proposal_id);
    append_str_field(bytes, "vote.validator", &vote.validator);
}

fn append_timeout_vote(bytes: &mut Vec<u8>, vote: &TimeoutVote) {
    append_str_field(bytes, "vote.vote_id", &vote.vote_id);
    append_str_field(bytes, "vote.chain_id", &vote.chain_id);
    append_str_field(bytes, "vote.genesis_hash", &vote.genesis_hash);
    append_u32_field(bytes, "vote.protocol_version", vote.protocol_version);
    append_u64_field(bytes, "vote.height", vote.height);
    append_u64_field(bytes, "vote.view", vote.view);
    append_str_field(bytes, "vote.high_qc_id", &vote.high_qc_id);
    append_str_field(bytes, "vote.validator", &vote.validator);
}

fn append_admission_receipt(bytes: &mut Vec<u8>, receipt: &AdmissionReceipt) {
    append_str_field(bytes, "receipt.receipt_id", &receipt.receipt_id);
    append_str_field(bytes, "receipt.chain_id", &receipt.chain_id);
    append_str_field(bytes, "receipt.genesis_hash", &receipt.genesis_hash);
    append_u32_field(bytes, "receipt.protocol_version", receipt.protocol_version);
    append_str_field(bytes, "receipt.tx_hash", &receipt.tx_hash);
    append_str_field(bytes, "receipt.validator", &receipt.validator);
    append_u64_field(
        bytes,
        "receipt.observation_window",
        receipt.observation_window,
    );
    append_u32_field(bytes, "receipt.bucket", receipt.bucket);
    append_str_field(bytes, "receipt.signature_hex", &receipt.signature_hex);
}

fn append_str_field(bytes: &mut Vec<u8>, label: &str, value: &str) {
    bytes.extend_from_slice(label.as_bytes());
    bytes.push(b'=');
    bytes.extend_from_slice(value.len().to_string().as_bytes());
    bytes.push(b':');
    bytes.extend_from_slice(value.as_bytes());
    bytes.push(b'\n');
}

fn append_u64_field(bytes: &mut Vec<u8>, label: &str, value: u64) {
    bytes.extend_from_slice(label.as_bytes());
    bytes.push(b'=');
    bytes.extend_from_slice(value.to_string().as_bytes());
    bytes.push(b'\n');
}

fn append_u32_field(bytes: &mut Vec<u8>, label: &str, value: u32) {
    append_u64_field(bytes, label, value as u64);
}

fn append_usize_field(bytes: &mut Vec<u8>, label: &str, value: usize) {
    append_u64_field(bytes, label, value as u64);
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{
        bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed,
    };

    fn reference(batch_id: &str, payload_hash: &str) -> BatchReference {
        BatchReference {
            batch_id: batch_id.to_string(),
            payload_hash: payload_hash.to_string(),
            transaction_count: 1,
        }
    }

    #[test]
    fn orders_references_deterministically() {
        let ordered = order_references(vec![
            reference("c", "payload-c"),
            reference("a", "payload-a"),
            reference("b", "payload-b"),
        ]);

        assert_eq!(ordered[0].height, 1);
        assert_eq!(ordered[0].reference.batch_id, "a");
        assert_eq!(ordered[1].height, 2);
        assert_eq!(ordered[1].reference.batch_id, "b");
        assert_eq!(ordered[2].height, 3);
        assert_eq!(ordered[2].reference.batch_id, "c");
    }

    #[test]
    fn deduplicates_exact_references_before_assigning_heights() {
        let ordered = order_references(vec![
            reference("b", "payload-b"),
            reference("a", "payload-a"),
            reference("b", "payload-b"),
            reference("a", "payload-a"),
        ]);

        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].height, 1);
        assert_eq!(ordered[0].reference.batch_id, "a");
        assert_eq!(ordered[1].height, 2);
        assert_eq!(ordered[1].reference.batch_id, "b");
    }

    #[test]
    fn keeps_same_batch_id_with_distinct_payload_evidence() {
        let ordered = order_references(vec![
            reference("a", "payload-b"),
            reference("a", "payload-a"),
            reference("a", "payload-b"),
        ]);

        assert_eq!(ordered.len(), 2);
        assert_eq!(ordered[0].reference.payload_hash, "payload-a");
        assert_eq!(ordered[1].reference.payload_hash, "payload-b");
    }

    fn domain() -> ConsensusDomain {
        ConsensusDomain {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "genesis-hash".to_string(),
            protocol_version: 1,
        }
    }

    fn validators() -> ValidatorSet {
        ValidatorSet::try_new(vec![
            "validator-2".to_string(),
            "validator-0".to_string(),
            "validator-3".to_string(),
            "validator-1".to_string(),
        ])
        .expect("validator set")
    }

    fn tx_hash() -> String {
        "ab".repeat(48)
    }

    fn admission_key_material() -> Vec<(String, Vec<u8>, ValidatorAdmissionKey)> {
        (0..4u8)
            .map(|index| {
                let validator = format!("validator-{index}");
                let key_pair = ml_dsa_65_keygen_from_seed(&[index + 1; 32]);
                let key = ValidatorAdmissionKey {
                    validator: validator.clone(),
                    public_key_hex: bytes_to_hex(&key_pair.public_key),
                };
                (validator, key_pair.private_key.as_slice().to_vec(), key)
            })
            .collect()
    }

    fn admission_keys(
        material: &[(String, Vec<u8>, ValidatorAdmissionKey)],
    ) -> Vec<ValidatorAdmissionKey> {
        material.iter().map(|(_, _, key)| key.clone()).collect()
    }

    fn signed_admission_receipt(
        domain: &ConsensusDomain,
        validator: &str,
        private_key: &[u8],
        tx_hash: &str,
        observation_window: u64,
        bucket: u32,
        seed: u8,
    ) -> AdmissionReceipt {
        let signing_bytes =
            admission_receipt_signing_bytes(domain, tx_hash, validator, observation_window, bucket)
                .expect("admission signing bytes");
        let signature = ml_dsa_65_sign_with_context_seed(
            private_key,
            &signing_bytes,
            ADMISSION_RECEIPT_SIGNATURE_CONTEXT,
            &[seed; 32],
        )
        .expect("sign admission receipt");
        build_admission_receipt(
            domain,
            tx_hash,
            validator,
            observation_window,
            bucket,
            bytes_to_hex(&signature),
        )
        .expect("admission receipt")
    }

    fn proposal(
        domain: &ConsensusDomain,
        validators: &ValidatorSet,
        height: u64,
        view: u64,
    ) -> HotstuffProposal {
        let proposer = leader_for_view(&validators.validators, height, view).expect("leader");
        HotstuffProposal::new(
            domain,
            height,
            view,
            "parent",
            "justify-qc",
            format!("payload-{height}-{view}"),
            proposer,
        )
    }

    #[test]
    fn proposal_id_has_canonical_golden_vector() {
        let domain = ConsensusDomain {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "a".repeat(96),
            protocol_version: 1,
        };
        let proposal =
            HotstuffProposal::new(&domain, 1, 2, "parent", "qc", "payload", "validator-0");

        assert_eq!(
            proposal.proposal_id().expect("proposal id"),
            "7acb8820f03eb683e8a91f3409dc1d24865a5b84922c13159c84b05910e67cadde86542773d5c1657091db56743f73e1"
        );
    }

    fn votes(
        domain: &ConsensusDomain,
        proposal: &HotstuffProposal,
        validator_set: &ValidatorSet,
        count: usize,
    ) -> Vec<HotstuffVote> {
        validator_set
            .validators
            .iter()
            .take(count)
            .map(|validator| HotstuffVote::new(domain, proposal, validator).expect("vote"))
            .collect()
    }

    #[test]
    fn computes_supermajority_quorum_thresholds() {
        assert!(bft_quorum_threshold(0).is_err());
        // Exhaust the largest committee size supported by the public FastSwap
        // encoding so the shared quorum helper cannot silently diverge at a
        // committee size accepted elsewhere in the protocol.
        for validator_count in 1..=64 {
            let fault_tolerance = bft_fault_tolerance(validator_count).expect("fault tolerance");
            let quorum = bft_quorum_threshold(validator_count).expect("quorum");
            assert_eq!(fault_tolerance, (validator_count - 1) / 3);
            assert_eq!(quorum, (2 * validator_count) / 3 + 1);
            assert!(2 * quorum > validator_count + fault_tolerance);
            assert!(quorum > fault_tolerance);
        }
    }

    #[test]
    fn canonicalizes_validator_set_and_rotates_leaders() {
        let validators = validators();
        assert_eq!(
            validators.validators,
            vec![
                "validator-0".to_string(),
                "validator-1".to_string(),
                "validator-2".to_string(),
                "validator-3".to_string()
            ]
        );
        assert_eq!(validators.quorum, 3);
        assert_eq!(
            leader_for_view(&validators.validators, 1, 0).expect("leader"),
            "validator-1"
        );
        assert_eq!(
            leader_for_view(&validators.validators, 1, 1).expect("leader"),
            "validator-2"
        );
        assert!(
            ValidatorSet::try_new(vec!["validator-0".to_string(), "validator-0".to_string()])
                .is_err()
        );
    }

    #[test]
    fn admission_receipts_form_quorum_aggregate_with_conservative_bucket() {
        let domain = domain();
        let validators = validators();
        let material = admission_key_material();
        let keys = admission_keys(&material);
        let tx_hash = tx_hash();
        let receipts = material
            .iter()
            .take(3)
            .enumerate()
            .map(|(index, (validator, private_key, _))| {
                signed_admission_receipt(
                    &domain,
                    validator,
                    private_key,
                    &tx_hash,
                    7,
                    [2, 4, 1][index],
                    index as u8 + 10,
                )
            })
            .collect::<Vec<_>>();

        let aggregate =
            certify_admission_receipts(&domain, &validators, &keys, &tx_hash, 7, 7, 0, receipts)
                .expect("aggregate");

        assert_eq!(aggregate.quorum, 3);
        assert_eq!(aggregate.receipts.len(), 3);
        assert_eq!(aggregate.conservative_bucket, 4);
        assert_eq!(aggregate.receipts[0].validator, "validator-0");
        assert_eq!(aggregate.receipts[1].validator, "validator-1");
        assert_eq!(aggregate.receipts[2].validator, "validator-2");
        verify_admission_receipt_aggregate(&domain, &validators, &keys, &aggregate)
            .expect("verify aggregate");
    }

    #[test]
    fn admission_receipts_reject_missing_quorum_stale_and_forged_inputs() {
        let domain = domain();
        let validators = validators();
        let material = admission_key_material();
        let keys = admission_keys(&material);
        let tx_hash = tx_hash();
        let two_receipts = material
            .iter()
            .take(2)
            .enumerate()
            .map(|(index, (validator, private_key, _))| {
                signed_admission_receipt(
                    &domain,
                    validator,
                    private_key,
                    &tx_hash,
                    8,
                    1,
                    index as u8 + 20,
                )
            })
            .collect::<Vec<_>>();
        let under_quorum = certify_admission_receipts(
            &domain,
            &validators,
            &keys,
            &tx_hash,
            8,
            8,
            0,
            two_receipts,
        )
        .expect_err("under quorum rejected");
        assert!(under_quorum
            .to_string()
            .contains("insufficient admission receipts"));

        let stale_receipts = material
            .iter()
            .take(3)
            .enumerate()
            .map(|(index, (validator, private_key, _))| {
                signed_admission_receipt(
                    &domain,
                    validator,
                    private_key,
                    &tx_hash,
                    5,
                    1,
                    index as u8 + 30,
                )
            })
            .collect::<Vec<_>>();
        let stale = certify_admission_receipts(
            &domain,
            &validators,
            &keys,
            &tx_hash,
            5,
            8,
            2,
            stale_receipts,
        )
        .expect_err("stale rejected");
        assert!(stale.to_string().contains("stale"));

        let mut forged = material
            .iter()
            .take(3)
            .enumerate()
            .map(|(index, (validator, private_key, _))| {
                signed_admission_receipt(
                    &domain,
                    validator,
                    private_key,
                    &tx_hash,
                    9,
                    1,
                    index as u8 + 40,
                )
            })
            .collect::<Vec<_>>();
        forged[0].signature_hex = "00".repeat(forged[0].signature_hex.len() / 2);
        let forged_error =
            certify_admission_receipts(&domain, &validators, &keys, &tx_hash, 9, 9, 0, forged)
                .expect_err("forged signature rejected");
        assert!(forged_error
            .to_string()
            .contains("signature verification failed"));
    }

    #[test]
    fn admission_receipts_reject_conflicting_duplicate_validator() {
        let domain = domain();
        let validators = validators();
        let material = admission_key_material();
        let keys = admission_keys(&material);
        let tx_hash = tx_hash();
        let mut receipts = material
            .iter()
            .take(3)
            .enumerate()
            .map(|(index, (validator, private_key, _))| {
                signed_admission_receipt(
                    &domain,
                    validator,
                    private_key,
                    &tx_hash,
                    10,
                    1,
                    index as u8 + 50,
                )
            })
            .collect::<Vec<_>>();
        let (validator, private_key, _) = &material[0];
        receipts.push(signed_admission_receipt(
            &domain,
            validator,
            private_key,
            &tx_hash,
            10,
            2,
            60,
        ));

        let error =
            certify_admission_receipts(&domain, &validators, &keys, &tx_hash, 10, 10, 0, receipts)
                .expect_err("conflicting duplicate rejected");
        assert!(error
            .to_string()
            .contains("conflicting duplicate admission receipt"));
    }

    #[test]
    fn omission_evidence_requires_valid_threshold_aggregates() {
        let domain = domain();
        let validators = validators();
        let material = admission_key_material();
        let keys = admission_keys(&material);
        let tx_hash = tx_hash();
        let mut aggregates = Vec::new();
        for window in [11, 12] {
            let receipts = material
                .iter()
                .take(3)
                .enumerate()
                .map(|(index, (validator, private_key, _))| {
                    signed_admission_receipt(
                        &domain,
                        validator,
                        private_key,
                        &tx_hash,
                        window,
                        index as u32 + 1,
                        index as u8 + window as u8,
                    )
                })
                .collect::<Vec<_>>();
            aggregates.push(
                certify_admission_receipts(
                    &domain,
                    &validators,
                    &keys,
                    &tx_hash,
                    window,
                    window,
                    0,
                    receipts,
                )
                .expect("aggregate"),
            );
        }

        let evidence = build_omission_evidence(
            &domain,
            &validators,
            &keys,
            "validator-3",
            aggregates.clone(),
            vec![21, 22],
            2,
        )
        .expect("omission evidence");
        assert_eq!(evidence.tx_hash, tx_hash);
        assert_eq!(evidence.accused_proposer, "validator-3");
        assert_eq!(evidence.first_observation_window, 11);
        assert_eq!(evidence.last_observation_window, 12);
        assert_eq!(evidence.aggregate_ids.len(), 2);

        let threshold_error = build_omission_evidence(
            &domain,
            &validators,
            &keys,
            "validator-3",
            aggregates.clone(),
            vec![21],
            2,
        )
        .expect_err("insufficient omitted heights rejected");
        assert!(threshold_error
            .to_string()
            .contains("insufficient omitted heights"));

        let mut tampered = aggregates;
        tampered[0].receipts[0].bucket = 99;
        let tampered_error = build_omission_evidence(
            &domain,
            &validators,
            &keys,
            "validator-3",
            tampered,
            vec![21, 22],
            2,
        )
        .expect_err("tampered aggregate rejected");
        assert!(tampered_error
            .to_string()
            .contains("admission receipt id mismatch"));
    }

    #[test]
    fn quorum_certificate_accepts_supermajority_subset() {
        let domain = domain();
        let validators = validators();
        let proposal = proposal(&domain, &validators, 1, 0);
        let certificate = certify_proposal(
            &domain,
            &validators,
            &proposal,
            votes(&domain, &proposal, &validators, 3),
        )
        .expect("qc");

        assert_eq!(certificate.quorum, 3);
        assert_eq!(certificate.votes.len(), 3);
        assert_eq!(certificate.validators, validators.validators);
        assert_eq!(certificate.height, 1);
        assert_eq!(certificate.view, 0);
        assert_eq!(
            certificate.proposal_id,
            proposal.proposal_id().expect("proposal id")
        );
        verify_quorum_certificate(&domain, &validators, &certificate).expect("verify qc");
    }

    #[test]
    fn quorum_certificate_rejects_under_quorum_duplicate_and_wrong_target_votes() {
        let domain = domain();
        let validators = validators();
        let base_proposal = proposal(&domain, &validators, 1, 0);
        assert!(certify_proposal(
            &domain,
            &validators,
            &base_proposal,
            votes(&domain, &base_proposal, &validators, 2)
        )
        .is_err());

        let mut duplicate_votes = votes(&domain, &base_proposal, &validators, 4);
        duplicate_votes.push(duplicate_votes[0].clone());
        let duplicate_certificate =
            certify_proposal(&domain, &validators, &base_proposal, duplicate_votes)
                .expect("exact duplicate votes are idempotent");
        assert_eq!(duplicate_certificate.votes.len(), 4);

        let mut under_quorum_with_duplicate = votes(&domain, &base_proposal, &validators, 2);
        under_quorum_with_duplicate.push(under_quorum_with_duplicate[0].clone());
        assert!(certify_proposal(
            &domain,
            &validators,
            &base_proposal,
            under_quorum_with_duplicate
        )
        .is_err());

        let other = proposal(&domain, &validators, 1, 1);
        let mut wrong_target_votes = votes(&domain, &base_proposal, &validators, 3);
        wrong_target_votes[0] = HotstuffVote::new(&domain, &other, &validators.validators[0])
            .expect("wrong target vote");
        assert!(
            certify_proposal(&domain, &validators, &base_proposal, wrong_target_votes).is_err()
        );
    }

    #[test]
    fn quorum_certificate_rejects_wrong_leader() {
        let domain = domain();
        let validators = validators();
        let mut proposal = proposal(&domain, &validators, 1, 0);
        proposal.proposer = "validator-0".to_string();
        assert!(certify_proposal(
            &domain,
            &validators,
            &proposal,
            votes(&domain, &proposal, &validators, 3)
        )
        .is_err());
    }

    #[test]
    fn timeout_certificate_accepts_quorum_with_one_shared_high_qc() {
        let domain = domain();
        let validators = validators();
        let votes = validators
            .validators
            .iter()
            .take(3)
            .map(|validator| {
                TimeoutVote::new(&domain, 2, 4, "qc-2", validator.clone()).expect("timeout vote")
            })
            .collect::<Vec<_>>();
        let certificate = certify_timeout(&domain, &validators, 2, 4, votes).expect("tc");
        assert_eq!(certificate.quorum, 3);
        assert_eq!(certificate.votes.len(), 3);
        assert_eq!(certificate.high_qc_id, "qc-2");
        verify_timeout_certificate(&domain, &validators, &certificate).expect("verify tc");
    }

    #[test]
    fn timeout_certificate_rejects_lexicographic_selection_of_opaque_high_qc_ids() {
        let domain = domain();
        let validators = validators();
        let high_qc_ids = ["qc-view-9", "qc-view-10", "qc-view-10"];
        let votes = validators
            .validators
            .iter()
            .take(3)
            .zip(high_qc_ids)
            .map(|(validator, high_qc_id)| {
                TimeoutVote::new(&domain, 2, 4, high_qc_id, validator.clone())
                    .expect("timeout vote")
            })
            .collect::<Vec<_>>();

        let error = certify_timeout(&domain, &validators, 2, 4, votes)
            .expect_err("opaque high-QC IDs must never be ranked lexicographically");
        assert!(error
            .to_string()
            .contains("cannot rank heterogeneous opaque high_qc_id values"));
    }

    #[test]
    fn certificate_verifiers_reject_tampered_evidence() {
        let domain = domain();
        let validators = validators();
        let proposal = proposal(&domain, &validators, 1, 0);
        let certificate = certify_proposal(
            &domain,
            &validators,
            &proposal,
            votes(&domain, &proposal, &validators, 3),
        )
        .expect("qc");

        let mut tampered_id = certificate.clone();
        tampered_id.certificate_id = "wrong-certificate".to_string();
        assert!(verify_quorum_certificate(&domain, &validators, &tampered_id).is_err());

        let mut tampered_votes = certificate.clone();
        tampered_votes.votes.reverse();
        assert!(verify_quorum_certificate(&domain, &validators, &tampered_votes).is_err());

        let timeout_votes = validators
            .validators
            .iter()
            .take(3)
            .map(|validator| {
                TimeoutVote::new(&domain, 2, 4, "qc-2", validator.clone()).expect("timeout vote")
            })
            .collect::<Vec<_>>();
        let timeout = certify_timeout(&domain, &validators, 2, 4, timeout_votes).expect("tc");
        let mut tampered_timeout = timeout.clone();
        tampered_timeout.high_qc_id = "stale-qc".to_string();
        assert!(verify_timeout_certificate(&domain, &validators, &tampered_timeout).is_err());
    }

    #[test]
    fn detects_proposal_and_vote_equivocation() {
        let domain = domain();
        let validators = validators();
        let first = proposal(&domain, &validators, 1, 0);
        let mut second = first.clone();
        second.payload_hash = "different-payload".to_string();
        let proposal_evidence = detect_proposal_equivocation(&domain, &first, &second)
            .expect("proposal equivocation")
            .expect("proposal evidence");
        assert_eq!(proposal_evidence.validator, first.proposer);
        assert_ne!(proposal_evidence.first_id, proposal_evidence.second_id);

        let first_vote = HotstuffVote::new(&domain, &first, "validator-0").expect("first vote");
        let second_vote = HotstuffVote::new(&domain, &second, "validator-0").expect("second vote");
        let vote_evidence = detect_vote_equivocation(&domain, &first_vote, &second_vote)
            .expect("vote equivocation")
            .expect("vote evidence");
        assert_eq!(vote_evidence.validator, "validator-0");
        assert_eq!(vote_evidence.height, 1);
        assert_eq!(vote_evidence.view, 0);
    }

    #[test]
    fn two_chain_commit_candidate_commits_parent_when_child_is_certified() {
        let domain = domain();
        let validators = validators();
        let parent = proposal(&domain, &validators, 1, 0);
        let parent_id = parent.proposal_id().expect("parent id");
        let child = HotstuffProposal::new(
            &domain,
            2,
            1,
            parent_id.clone(),
            "parent-qc",
            "child-payload",
            leader_for_view(&validators.validators, 2, 1).expect("child leader"),
        );
        let child_qc = certify_proposal(
            &domain,
            &validators,
            &child,
            votes(&domain, &child, &validators, 3),
        )
        .expect("child qc");

        assert_eq!(
            two_chain_commit_candidate(&parent, &child, &child_qc).expect("commit"),
            Some(parent_id.clone())
        );
        assert_eq!(
            verified_two_chain_commit_candidate(&domain, &validators, &parent, &child, &child_qc)
                .expect("verified commit"),
            Some(parent_id)
        );

        let mut tampered_qc = child_qc;
        tampered_qc.certificate_id = "tampered-child-qc".to_string();
        assert!(verified_two_chain_commit_candidate(
            &domain,
            &validators,
            &parent,
            &child,
            &tampered_qc
        )
        .is_err());
    }

    #[test]
    fn adversarial_simulation_preserves_safety_under_network_faults() {
        let domain = domain();
        let validators = validators();
        let scenario = OrderingSimulationScenario {
            heights: 5,
            max_views_per_height: 3,
            faults: vec![
                OrderingAdversaryFault::DelayProposal { height: 1, view: 0 },
                OrderingAdversaryFault::DuplicateVotes { height: 1, view: 1 },
                OrderingAdversaryFault::FailedLeader { height: 2, view: 0 },
                OrderingAdversaryFault::EquivocateProposal { height: 2, view: 1 },
                OrderingAdversaryFault::PartitionVotes {
                    height: 3,
                    view: 0,
                    partitions: vec![
                        vec!["validator-0".to_string(), "validator-1".to_string()],
                        vec!["validator-2".to_string(), "validator-3".to_string()],
                    ],
                },
                OrderingAdversaryFault::DuplicateVotes { height: 3, view: 1 },
                OrderingAdversaryFault::DropVotes {
                    height: 4,
                    view: 0,
                    validators: vec!["validator-3".to_string()],
                },
                OrderingAdversaryFault::StaleVotes {
                    height: 4,
                    view: 0,
                    stale_height: 3,
                    stale_view: 1,
                },
            ],
        };

        let report = simulate_adversarial_ordering(&domain, &validators, &scenario)
            .expect("adversarial simulation");

        assert_eq!(report.certified.len(), 5);
        assert_eq!(report.timeouts.len(), 2);
        assert_eq!(report.commits.len(), 4);
        assert!(report
            .commits
            .windows(2)
            .all(|window| window[0].height < window[1].height));
        assert_eq!(report.stalled_views.len(), 2);
        assert_eq!(report.stalled_views[0].height, 3);
        assert_eq!(report.stalled_views[0].view, 0);
        assert_eq!(report.stalled_views[1].height, 4);
        assert_eq!(report.stalled_views[1].view, 0);
        assert!(report.stalled_views[1]
            .reason
            .contains("vote target mismatch"));
        assert!(report.equivocations.len() >= 2);
        verify_no_conflicting_commits(&report.commits).expect("no conflicting commits");
    }

    #[test]
    fn adversarial_simulation_rejects_non_stale_vote_faults() {
        let domain = domain();
        let validators = validators();
        let scenario = OrderingSimulationScenario {
            heights: 1,
            max_views_per_height: 2,
            faults: vec![OrderingAdversaryFault::StaleVotes {
                height: 1,
                view: 1,
                stale_height: 1,
                stale_view: 1,
            }],
        };

        assert!(simulate_adversarial_ordering(&domain, &validators, &scenario).is_err());
    }

    #[test]
    fn conflicting_simulated_commits_are_rejected() {
        let commits = vec![
            SimulatedCommit {
                height: 7,
                view: 0,
                proposal_id: "proposal-a".to_string(),
                parent_block_id: "parent".to_string(),
            },
            SimulatedCommit {
                height: 7,
                view: 1,
                proposal_id: "proposal-b".to_string(),
                parent_block_id: "parent".to_string(),
            },
        ];

        assert!(verify_no_conflicting_commits(&commits).is_err());
    }
}
