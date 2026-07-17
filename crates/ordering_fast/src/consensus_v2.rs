use std::collections::{BTreeMap, BTreeSet};

use postfiat_crypto_provider::{hex_to_bytes, ml_dsa_65_verify_with_context, ML_DSA_65_ALGORITHM};
use postfiat_types::{
    ConsensusV2BlockRef, ConsensusV2Commit, ConsensusV2Domain, ConsensusV2Phase,
    ConsensusV2Proposal, ConsensusV2QcRef, ConsensusV2QuorumCertificate, ConsensusV2Round,
    ConsensusV2SafetyState, ConsensusV2TimeoutCertificate, ConsensusV2TimeoutVote, ConsensusV2Vote,
    CONSENSUS_V2_COMMIT_SCHEMA, CONSENSUS_V2_DOMAIN_SCHEMA, CONSENSUS_V2_PROPOSAL_SCHEMA,
    CONSENSUS_V2_QC_SCHEMA, CONSENSUS_V2_SAFETY_STATE_SCHEMA, CONSENSUS_V2_TC_SCHEMA,
    CONSENSUS_V2_TIMEOUT_VOTE_SCHEMA, CONSENSUS_V2_VOTE_SCHEMA,
};
use serde::{Deserialize, Serialize};

use super::{
    append_str_field, append_u32_field, append_u64_field, append_usize_field, bft_quorum_threshold,
    hash_canonical, leader_for_view, OrderingError,
};

pub const CONSENSUS_V2_PROPOSAL_CONTEXT: &[u8] = b"postfiat-l1-v2/consensus-proposal/v2";
pub const CONSENSUS_V2_VOTE_CONTEXT: &[u8] = b"postfiat-l1-v2/consensus-vote/v2";
pub const CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT: &[u8] = b"postfiat-l1-v2/consensus-timeout-vote/v2";

const CONSENSUS_V2_COMMITTEE_ROOT_DOMAIN: &str = "postfiat.consensus.committee-root.v2";
const CONSENSUS_V2_BLOCK_ID_DOMAIN: &str = "postfiat.consensus.block-id.v2";
const CONSENSUS_V2_BRIDGE_EXIT_BLOCK_ID_DOMAIN: &str =
    "postfiat.consensus.block-id.bridge-exit-root.v1";
const CONSENSUS_V2_GENESIS_PARENT_ID_DOMAIN: &str = "postfiat.consensus.genesis-parent-id.v2";
const CONSENSUS_V2_PROPOSAL_ID_DOMAIN: &str = "postfiat.consensus.proposal-id.v2";
const CONSENSUS_V2_QC_ID_DOMAIN: &str = "postfiat.consensus.qc-id.v2";
const CONSENSUS_V2_TC_ID_DOMAIN: &str = "postfiat.consensus.tc-id.v2";
const CONSENSUS_V2_SAFETY_DIGEST_DOMAIN: &str = "postfiat.consensus.safety-vote.v2";
const CONSENSUS_V2_TIMEOUT_SAFETY_DIGEST_DOMAIN: &str = "postfiat.consensus.safety-timeout-vote.v2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2Validator {
    pub validator_id: String,
    pub public_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2ValidatorSet {
    pub validators: Vec<ConsensusV2Validator>,
    pub quorum: usize,
    pub committee_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2SafetyModelReport {
    pub validator_count: usize,
    pub fault_tolerance: usize,
    pub quorum: usize,
    pub quorum_set_count: usize,
    pub quorum_pair_count: usize,
    pub minimum_intersection: usize,
    pub minimum_honest_intersection: usize,
}

impl ConsensusV2ValidatorSet {
    pub fn try_new(validators: Vec<ConsensusV2Validator>) -> Result<Self, OrderingError> {
        let mut by_id = BTreeMap::new();
        for validator in validators {
            validate_text("validator id", &validator.validator_id)?;
            validate_hex_len("validator public key", &validator.public_key_hex, 1952)?;
            if by_id
                .insert(validator.validator_id.clone(), validator)
                .is_some()
            {
                return Err(OrderingError::new("duplicate consensus v2 validator"));
            }
        }
        if by_id.is_empty() {
            return Err(OrderingError::new(
                "consensus v2 validator set must be nonempty",
            ));
        }
        let validators = by_id.into_values().collect::<Vec<_>>();
        let quorum = bft_quorum_threshold(validators.len())?;
        let committee_root = hash_canonical(CONSENSUS_V2_COMMITTEE_ROOT_DOMAIN, |bytes| {
            append_usize_field(bytes, "validator_count", validators.len());
            append_usize_field(bytes, "quorum", quorum);
            for validator in &validators {
                append_str_field(bytes, "validator_id", &validator.validator_id);
                append_str_field(bytes, "public_key_hex", &validator.public_key_hex);
            }
        });
        Ok(Self {
            validators,
            quorum,
            committee_root,
        })
    }

    pub fn validator_ids(&self) -> Vec<String> {
        self.validators
            .iter()
            .map(|validator| validator.validator_id.clone())
            .collect()
    }

    fn get(&self, validator_id: &str) -> Option<&ConsensusV2Validator> {
        self.validators
            .binary_search_by(|validator| validator.validator_id.as_str().cmp(validator_id))
            .ok()
            .map(|index| &self.validators[index])
    }
}

#[derive(Debug, Clone, Default)]
pub struct ConsensusV2QcGraph {
    certificates: BTreeMap<String, ConsensusV2QuorumCertificate>,
}

impl ConsensusV2QcGraph {
    pub fn insert_verified(
        &mut self,
        domain: &ConsensusV2Domain,
        validators: &ConsensusV2ValidatorSet,
        certificate: ConsensusV2QuorumCertificate,
    ) -> Result<ConsensusV2QcRef, OrderingError> {
        verify_consensus_v2_qc(domain, validators, &certificate)?;
        let reference = consensus_v2_qc_ref(&certificate)?;
        if let Some(existing) = self
            .certificates
            .insert(certificate.certificate_id.clone(), certificate.clone())
        {
            if existing != certificate {
                return Err(OrderingError::new(
                    "consensus v2 QC ID collision or conflicting replacement",
                ));
            }
        }
        Ok(reference)
    }

    pub fn resolve_verified(
        &self,
        domain: &ConsensusV2Domain,
        validators: &ConsensusV2ValidatorSet,
        reference: &ConsensusV2QcRef,
    ) -> Result<&ConsensusV2QuorumCertificate, OrderingError> {
        let certificate = self
            .certificates
            .get(&reference.certificate_id)
            .ok_or_else(|| OrderingError::new("consensus v2 QC reference is unresolved"))?;
        verify_consensus_v2_qc(domain, validators, certificate)?;
        if consensus_v2_qc_ref(certificate)? != *reference {
            return Err(OrderingError::new(
                "consensus v2 QC reference does not match resolved certificate",
            ));
        }
        Ok(certificate)
    }
}

pub fn consensus_v2_domain(
    chain_id: impl Into<String>,
    genesis_hash: impl Into<String>,
    protocol_version: u32,
    committee_epoch: u64,
    validators: &ConsensusV2ValidatorSet,
) -> ConsensusV2Domain {
    ConsensusV2Domain {
        schema: CONSENSUS_V2_DOMAIN_SCHEMA.to_string(),
        chain_id: chain_id.into(),
        genesis_hash: genesis_hash.into(),
        protocol_version,
        committee_epoch,
        committee_root: validators.committee_root.clone(),
    }
}

pub fn consensus_v2_block_ref(
    domain: &ConsensusV2Domain,
    height: u64,
    parent_block_id: impl Into<String>,
    payload_hash: impl Into<String>,
    state_root: impl Into<String>,
) -> Result<ConsensusV2BlockRef, OrderingError> {
    consensus_v2_block_ref_internal(
        domain,
        height,
        parent_block_id.into(),
        payload_hash.into(),
        state_root.into(),
        None,
    )
}

pub fn consensus_v2_block_ref_with_bridge_exit_root(
    domain: &ConsensusV2Domain,
    height: u64,
    parent_block_id: impl Into<String>,
    payload_hash: impl Into<String>,
    state_root: impl Into<String>,
    bridge_exit_root: impl Into<String>,
) -> Result<ConsensusV2BlockRef, OrderingError> {
    consensus_v2_block_ref_internal(
        domain,
        height,
        parent_block_id.into(),
        payload_hash.into(),
        state_root.into(),
        Some(bridge_exit_root.into()),
    )
}

fn consensus_v2_block_ref_internal(
    domain: &ConsensusV2Domain,
    height: u64,
    parent_block_id: String,
    payload_hash: String,
    state_root: String,
    bridge_exit_root: Option<String>,
) -> Result<ConsensusV2BlockRef, OrderingError> {
    validate_consensus_v2_domain(domain)?;
    if height == 0 {
        return Err(OrderingError::new(
            "consensus v2 block height must be nonzero",
        ));
    }
    validate_hash("parent block id", &parent_block_id)?;
    validate_hash("payload hash", &payload_hash)?;
    validate_hash("state root", &state_root)?;
    if let Some(root) = bridge_exit_root.as_deref() {
        validate_hash("bridge exit root", root)?;
    }
    let block_id_domain = if bridge_exit_root.is_some() {
        CONSENSUS_V2_BRIDGE_EXIT_BLOCK_ID_DOMAIN
    } else {
        CONSENSUS_V2_BLOCK_ID_DOMAIN
    };
    let block_id = hash_canonical(block_id_domain, |bytes| {
        append_consensus_v2_domain(bytes, domain);
        append_u64_field(bytes, "height", height);
        append_str_field(bytes, "parent_block_id", &parent_block_id);
        append_str_field(bytes, "payload_hash", &payload_hash);
        append_str_field(bytes, "state_root", &state_root);
        if let Some(root) = bridge_exit_root.as_deref() {
            append_str_field(bytes, "bridge_exit_root", root);
        }
    });
    Ok(ConsensusV2BlockRef {
        height,
        block_id,
        parent_block_id,
        payload_hash,
        state_root,
        bridge_exit_root,
    })
}

pub fn verify_consensus_v2_bridge_exit_root(
    block: &ConsensusV2BlockRef,
    expected_bridge_exit_root: &str,
) -> Result<(), OrderingError> {
    validate_hash("expected bridge exit root", expected_bridge_exit_root)?;
    match block.bridge_exit_root.as_deref() {
        Some(actual) if actual == expected_bridge_exit_root => Ok(()),
        Some(_) => Err(OrderingError::new("consensus v2 bridge exit root mismatch")),
        None => Err(OrderingError::new(
            "legacy consensus v2 block does not bind a bridge exit root",
        )),
    }
}

fn recompute_consensus_v2_block_ref(
    domain: &ConsensusV2Domain,
    block: &ConsensusV2BlockRef,
) -> Result<ConsensusV2BlockRef, OrderingError> {
    match block.bridge_exit_root.as_deref() {
        Some(root) => consensus_v2_block_ref_with_bridge_exit_root(
            domain,
            block.height,
            block.parent_block_id.clone(),
            block.payload_hash.clone(),
            block.state_root.clone(),
            root.to_string(),
        ),
        None => consensus_v2_block_ref(
            domain,
            block.height,
            block.parent_block_id.clone(),
            block.payload_hash.clone(),
            block.state_root.clone(),
        ),
    }
}

pub fn consensus_v2_genesis_parent_id(domain: &ConsensusV2Domain) -> Result<String, OrderingError> {
    validate_consensus_v2_domain(domain)?;
    Ok(hash_canonical(
        CONSENSUS_V2_GENESIS_PARENT_ID_DOMAIN,
        |bytes| append_consensus_v2_domain(bytes, domain),
    ))
}

pub fn consensus_v2_proposal_signing_bytes(
    proposal: &ConsensusV2Proposal,
) -> Result<Vec<u8>, OrderingError> {
    validate_proposal_shape(proposal)?;
    Ok(canonical_consensus_v2_proposal(proposal, false))
}

pub fn consensus_v2_vote_signing_bytes(vote: &ConsensusV2Vote) -> Result<Vec<u8>, OrderingError> {
    validate_vote_shape(vote)?;
    Ok(canonical_consensus_v2_vote(vote, false))
}

pub fn consensus_v2_timeout_vote_signing_bytes(
    vote: &ConsensusV2TimeoutVote,
) -> Result<Vec<u8>, OrderingError> {
    validate_timeout_vote_shape(vote)?;
    Ok(canonical_consensus_v2_timeout_vote(vote, false))
}

pub fn consensus_v2_proposal_id(proposal: &ConsensusV2Proposal) -> Result<String, OrderingError> {
    Ok(hash_canonical(CONSENSUS_V2_PROPOSAL_ID_DOMAIN, |bytes| {
        bytes.extend(
            consensus_v2_proposal_signing_bytes(proposal)
                .expect("shape validated before canonical proposal hash"),
        );
    }))
}

pub fn verify_consensus_v2_proposal(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    proposal: &ConsensusV2Proposal,
    timeout_certificate: Option<&ConsensusV2TimeoutCertificate>,
    qc_graph: &ConsensusV2QcGraph,
) -> Result<(), OrderingError> {
    validate_domain_and_committee(domain, validators)?;
    validate_proposal_shape(proposal)?;
    if proposal.domain != *domain {
        return Err(OrderingError::new("consensus v2 proposal domain mismatch"));
    }
    if proposal.round.height != proposal.block.height {
        return Err(OrderingError::new(
            "consensus v2 proposal block height mismatch",
        ));
    }
    let expected_block = recompute_consensus_v2_block_ref(domain, &proposal.block)?;
    if expected_block != proposal.block {
        return Err(OrderingError::new(
            "consensus v2 proposal block ID mismatch",
        ));
    }
    let expected_proposer = leader_for_view(
        &validators.validator_ids(),
        proposal.round.height,
        proposal.round.view,
    )?;
    if proposal.proposer != expected_proposer {
        return Err(OrderingError::new("consensus v2 proposal leader mismatch"));
    }
    verify_signature(
        validators,
        &proposal.proposer,
        &proposal.signature,
        &consensus_v2_proposal_signing_bytes(proposal)?,
        CONSENSUS_V2_PROPOSAL_CONTEXT,
    )?;

    if proposal.round.view == 0 {
        if timeout_certificate.is_some() || proposal.timeout_certificate_id.is_some() {
            return Err(OrderingError::new(
                "consensus v2 view-zero proposal must not carry timeout evidence",
            ));
        }
        if proposal.valid_qc.is_some() {
            return Err(OrderingError::new(
                "consensus v2 view-zero proposal must not carry a valid-round QC",
            ));
        }
        return Ok(());
    }

    let timeout_certificate = timeout_certificate.ok_or_else(|| {
        OrderingError::new("consensus v2 nonzero-view proposal requires timeout certificate")
    })?;
    verify_consensus_v2_timeout_certificate(domain, validators, timeout_certificate, qc_graph)?;
    if timeout_certificate.round.height != proposal.round.height
        || timeout_certificate.round.view + 1 != proposal.round.view
        || timeout_certificate.phase != ConsensusV2Phase::Precommit
        || proposal.timeout_certificate_id.as_deref()
            != Some(timeout_certificate.certificate_id.as_str())
    {
        return Err(OrderingError::new(
            "consensus v2 proposal timeout certificate target mismatch",
        ));
    }
    if proposal.valid_qc != timeout_certificate.high_qc {
        return Err(OrderingError::new(
            "consensus v2 proposal valid QC does not match timeout high QC",
        ));
    }
    if let Some(valid_qc) = &proposal.valid_qc {
        let certificate = qc_graph.resolve_verified(domain, validators, valid_qc)?;
        if certificate.phase != ConsensusV2Phase::Prepare
            || certificate.round.height != proposal.round.height
            || certificate.round.view >= proposal.round.view
            || certificate.block.as_ref() != Some(&proposal.block)
        {
            return Err(OrderingError::new(
                "consensus v2 proposal valid QC is not a prior prepare QC for its block",
            ));
        }
    }
    Ok(())
}

pub fn verify_consensus_v2_vote(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    vote: &ConsensusV2Vote,
) -> Result<(), OrderingError> {
    validate_domain_and_committee(domain, validators)?;
    validate_vote_shape(vote)?;
    if vote.domain != *domain {
        return Err(OrderingError::new("consensus v2 vote domain mismatch"));
    }
    if let Some(block) = &vote.block {
        if block.height != vote.round.height {
            return Err(OrderingError::new(
                "consensus v2 vote block height mismatch",
            ));
        }
        let expected = recompute_consensus_v2_block_ref(domain, block)?;
        if expected != *block {
            return Err(OrderingError::new("consensus v2 vote block ID mismatch"));
        }
    }
    verify_signature(
        validators,
        &vote.validator,
        &vote.signature,
        &consensus_v2_vote_signing_bytes(vote)?,
        CONSENSUS_V2_VOTE_CONTEXT,
    )
}

pub fn certify_consensus_v2_votes(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    round: ConsensusV2Round,
    phase: ConsensusV2Phase,
    block: Option<ConsensusV2BlockRef>,
    votes: Vec<ConsensusV2Vote>,
) -> Result<ConsensusV2QuorumCertificate, OrderingError> {
    validate_domain_and_committee(domain, validators)?;
    let mut by_validator = BTreeMap::new();
    for vote in votes {
        verify_consensus_v2_vote(domain, validators, &vote)?;
        if vote.round != round || vote.phase != phase || vote.block != block {
            return Err(OrderingError::new("consensus v2 vote target mismatch"));
        }
        if by_validator.insert(vote.validator.clone(), vote).is_some() {
            return Err(OrderingError::new("duplicate consensus v2 validator vote"));
        }
    }
    if by_validator.len() < validators.quorum {
        return Err(OrderingError::new("insufficient consensus v2 votes"));
    }
    let votes = validators
        .validators
        .iter()
        .filter_map(|validator| by_validator.remove(&validator.validator_id))
        .collect::<Vec<_>>();
    let mut certificate = ConsensusV2QuorumCertificate {
        schema: CONSENSUS_V2_QC_SCHEMA.to_string(),
        domain: domain.clone(),
        round,
        phase,
        block,
        validators: validators.validator_ids(),
        quorum: validators.quorum,
        votes,
        certificate_id: String::new(),
    };
    certificate.certificate_id = consensus_v2_qc_id(&certificate)?;
    verify_consensus_v2_qc(domain, validators, &certificate)?;
    Ok(certificate)
}

pub fn verify_consensus_v2_qc(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    certificate: &ConsensusV2QuorumCertificate,
) -> Result<(), OrderingError> {
    validate_domain_and_committee(domain, validators)?;
    if certificate.schema != CONSENSUS_V2_QC_SCHEMA || certificate.domain != *domain {
        return Err(OrderingError::new(
            "consensus v2 QC schema or domain mismatch",
        ));
    }
    if certificate.validators != validators.validator_ids()
        || certificate.quorum != validators.quorum
        || certificate.votes.len() < validators.quorum
    {
        return Err(OrderingError::new("consensus v2 QC committee mismatch"));
    }
    let mut seen = BTreeSet::new();
    let mut prior = None::<&str>;
    for vote in &certificate.votes {
        verify_consensus_v2_vote(domain, validators, vote)?;
        if vote.round != certificate.round
            || vote.phase != certificate.phase
            || vote.block != certificate.block
        {
            return Err(OrderingError::new("consensus v2 QC vote target mismatch"));
        }
        if !seen.insert(vote.validator.as_str())
            || prior.is_some_and(|previous| previous >= vote.validator.as_str())
        {
            return Err(OrderingError::new(
                "consensus v2 QC votes are duplicate or noncanonical",
            ));
        }
        prior = Some(&vote.validator);
    }
    if certificate.certificate_id != consensus_v2_qc_id(certificate)? {
        return Err(OrderingError::new("consensus v2 QC ID mismatch"));
    }
    Ok(())
}

pub fn consensus_v2_qc_ref(
    certificate: &ConsensusV2QuorumCertificate,
) -> Result<ConsensusV2QcRef, OrderingError> {
    let block = certificate
        .block
        .clone()
        .ok_or_else(|| OrderingError::new("nil consensus v2 QC has no block reference"))?;
    Ok(ConsensusV2QcRef {
        certificate_id: certificate.certificate_id.clone(),
        round: certificate.round,
        phase: certificate.phase,
        block,
    })
}

pub fn certify_consensus_v2_timeouts(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    round: ConsensusV2Round,
    phase: ConsensusV2Phase,
    votes: Vec<ConsensusV2TimeoutVote>,
    qc_graph: &ConsensusV2QcGraph,
) -> Result<ConsensusV2TimeoutCertificate, OrderingError> {
    validate_domain_and_committee(domain, validators)?;
    let mut by_validator = BTreeMap::new();
    for vote in votes {
        verify_consensus_v2_timeout_vote(domain, validators, &vote, qc_graph)?;
        if vote.round != round || vote.phase != phase {
            return Err(OrderingError::new(
                "consensus v2 timeout vote target mismatch",
            ));
        }
        if by_validator.insert(vote.validator.clone(), vote).is_some() {
            return Err(OrderingError::new(
                "duplicate consensus v2 timeout validator",
            ));
        }
    }
    if by_validator.len() < validators.quorum {
        return Err(OrderingError::new(
            "insufficient consensus v2 timeout votes",
        ));
    }
    let votes = validators
        .validators
        .iter()
        .filter_map(|validator| by_validator.remove(&validator.validator_id))
        .collect::<Vec<_>>();
    let high_qc = highest_timeout_qc(&votes)?;
    let mut certificate = ConsensusV2TimeoutCertificate {
        schema: CONSENSUS_V2_TC_SCHEMA.to_string(),
        domain: domain.clone(),
        round,
        phase,
        high_qc,
        validators: validators.validator_ids(),
        quorum: validators.quorum,
        votes,
        certificate_id: String::new(),
    };
    certificate.certificate_id = consensus_v2_tc_id(&certificate)?;
    verify_consensus_v2_timeout_certificate(domain, validators, &certificate, qc_graph)?;
    Ok(certificate)
}

pub fn verify_consensus_v2_timeout_vote(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    vote: &ConsensusV2TimeoutVote,
    qc_graph: &ConsensusV2QcGraph,
) -> Result<(), OrderingError> {
    validate_domain_and_committee(domain, validators)?;
    validate_timeout_vote_shape(vote)?;
    if vote.domain != *domain {
        return Err(OrderingError::new(
            "consensus v2 timeout vote domain mismatch",
        ));
    }
    if let Some(high_qc) = &vote.high_qc {
        let certificate = qc_graph.resolve_verified(domain, validators, high_qc)?;
        if certificate.phase != ConsensusV2Phase::Prepare
            || high_qc.round.height != vote.round.height
            || high_qc.round.view > vote.round.view
        {
            return Err(OrderingError::new(
                "consensus v2 timeout high QC target mismatch",
            ));
        }
    }
    verify_signature(
        validators,
        &vote.validator,
        &vote.signature,
        &consensus_v2_timeout_vote_signing_bytes(vote)?,
        CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT,
    )
}

pub fn verify_consensus_v2_timeout_certificate(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    certificate: &ConsensusV2TimeoutCertificate,
    qc_graph: &ConsensusV2QcGraph,
) -> Result<(), OrderingError> {
    validate_domain_and_committee(domain, validators)?;
    if certificate.schema != CONSENSUS_V2_TC_SCHEMA || certificate.domain != *domain {
        return Err(OrderingError::new(
            "consensus v2 TC schema or domain mismatch",
        ));
    }
    if certificate.validators != validators.validator_ids()
        || certificate.quorum != validators.quorum
        || certificate.votes.len() < validators.quorum
    {
        return Err(OrderingError::new("consensus v2 TC committee mismatch"));
    }
    let mut seen = BTreeSet::new();
    let mut prior = None::<&str>;
    for vote in &certificate.votes {
        verify_consensus_v2_timeout_vote(domain, validators, vote, qc_graph)?;
        if vote.round != certificate.round || vote.phase != certificate.phase {
            return Err(OrderingError::new("consensus v2 TC vote target mismatch"));
        }
        if !seen.insert(vote.validator.as_str())
            || prior.is_some_and(|previous| previous >= vote.validator.as_str())
        {
            return Err(OrderingError::new(
                "consensus v2 TC votes are duplicate or noncanonical",
            ));
        }
        prior = Some(&vote.validator);
    }
    let expected_high_qc = highest_timeout_qc(&certificate.votes)?;
    if certificate.high_qc != expected_high_qc {
        return Err(OrderingError::new(
            "consensus v2 TC high QC is not the highest verified typed QC",
        ));
    }
    if certificate.certificate_id != consensus_v2_tc_id(certificate)? {
        return Err(OrderingError::new("consensus v2 TC ID mismatch"));
    }
    Ok(())
}

pub fn initial_consensus_v2_safety_state(
    domain: &ConsensusV2Domain,
    current_height: u64,
) -> Result<ConsensusV2SafetyState, OrderingError> {
    validate_consensus_v2_domain(domain)?;
    if current_height == 0 {
        return Err(OrderingError::new(
            "consensus v2 safety height must be nonzero",
        ));
    }
    Ok(ConsensusV2SafetyState {
        schema: CONSENSUS_V2_SAFETY_STATE_SCHEMA.to_string(),
        domain: domain.clone(),
        current_height,
        highest_prepare_round: None,
        highest_precommit_round: None,
        highest_timeout_round: None,
        locked_qc: None,
        high_qc: None,
        last_signed_vote_digest: None,
        last_signed_timeout_digest: None,
    })
}

/// Verify the timeout target and advance the durable timeout high-water mark.
/// The returned state must be persisted before the caller emits the signature.
pub fn authorize_consensus_v2_timeout_vote(
    state: &ConsensusV2SafetyState,
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    round: ConsensusV2Round,
    high_qc: Option<&ConsensusV2QcRef>,
    qc_graph: &ConsensusV2QcGraph,
) -> Result<ConsensusV2SafetyState, OrderingError> {
    validate_domain_and_committee(domain, validators)?;
    validate_safety_state(state)?;
    if state.domain != *domain
        || round.height != state.current_height
        || state
            .highest_timeout_round
            .is_some_and(|highest| highest >= round)
    {
        return Err(OrderingError::new(
            "consensus v2 timeout vote would violate durable round monotonicity",
        ));
    }
    if let Some(reference) = high_qc {
        if reference.round.height != round.height
            || reference.round.view > round.view
            || reference.phase != ConsensusV2Phase::Prepare
        {
            return Err(OrderingError::new(
                "consensus v2 timeout high QC is not a prior prepare QC at this height",
            ));
        }
        qc_graph.resolve_verified(domain, validators, reference)?;
    }
    let mut next = state.clone();
    next.highest_timeout_round = Some(round);
    if let Some(reference) = high_qc {
        if next
            .high_qc
            .as_ref()
            .is_none_or(|current| qc_rank(reference) > qc_rank(current))
        {
            next.high_qc = Some(reference.clone());
        }
    }
    next.last_signed_timeout_digest = Some(timeout_safety_vote_digest(round, high_qc));
    Ok(next)
}

pub fn authorize_consensus_v2_prepare_vote(
    state: &ConsensusV2SafetyState,
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    proposal: &ConsensusV2Proposal,
    timeout_certificate: Option<&ConsensusV2TimeoutCertificate>,
    qc_graph: &ConsensusV2QcGraph,
) -> Result<ConsensusV2SafetyState, OrderingError> {
    verify_consensus_v2_proposal(domain, validators, proposal, timeout_certificate, qc_graph)?;
    apply_consensus_v2_prepare_vote_to_safety(state, proposal)
}

fn apply_consensus_v2_prepare_vote_to_safety(
    state: &ConsensusV2SafetyState,
    proposal: &ConsensusV2Proposal,
) -> Result<ConsensusV2SafetyState, OrderingError> {
    validate_safety_state(state)?;
    if proposal.domain != state.domain
        || proposal.round.height != state.current_height
        || state
            .highest_prepare_round
            .is_some_and(|round| round >= proposal.round)
    {
        return Err(OrderingError::new(
            "consensus v2 prepare vote would violate durable round monotonicity",
        ));
    }
    if let Some(locked_qc) = &state.locked_qc {
        let unlocks = proposal.valid_qc.as_ref().is_some_and(|valid_qc| {
            valid_qc.round > locked_qc.round
                && valid_qc.phase == ConsensusV2Phase::Prepare
                && valid_qc.block == proposal.block
        });
        if proposal.block != locked_qc.block && !unlocks {
            return Err(OrderingError::new(
                "consensus v2 prepare vote conflicts with durable lock",
            ));
        }
    }
    let mut next = state.clone();
    next.highest_prepare_round = Some(proposal.round);
    if let Some(valid_qc) = &proposal.valid_qc {
        if next
            .high_qc
            .as_ref()
            .is_none_or(|current| qc_rank(valid_qc) > qc_rank(current))
        {
            next.high_qc = Some(valid_qc.clone());
        }
    }
    next.last_signed_vote_digest = Some(safety_vote_digest(
        proposal.round,
        ConsensusV2Phase::Prepare,
        &proposal.block.block_id,
    ));
    Ok(next)
}

pub fn authorize_consensus_v2_precommit_vote(
    state: &ConsensusV2SafetyState,
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    prepare_qc: &ConsensusV2QuorumCertificate,
) -> Result<ConsensusV2SafetyState, OrderingError> {
    verify_consensus_v2_qc(domain, validators, prepare_qc)?;
    apply_consensus_v2_precommit_vote_to_safety(state, prepare_qc)
}

fn apply_consensus_v2_precommit_vote_to_safety(
    state: &ConsensusV2SafetyState,
    prepare_qc: &ConsensusV2QuorumCertificate,
) -> Result<ConsensusV2SafetyState, OrderingError> {
    validate_safety_state(state)?;
    if prepare_qc.domain != state.domain
        || prepare_qc.round.height != state.current_height
        || prepare_qc.phase != ConsensusV2Phase::Prepare
        || prepare_qc.block.is_none()
        || state
            .highest_precommit_round
            .is_some_and(|round| round >= prepare_qc.round)
    {
        return Err(OrderingError::new(
            "consensus v2 precommit vote is not authorized by a newer non-nil prepare QC",
        ));
    }
    let reference = consensus_v2_qc_ref(prepare_qc)?;
    if let Some(locked_qc) = &state.locked_qc {
        if reference.round < locked_qc.round
            || (reference.round == locked_qc.round && reference.block != locked_qc.block)
        {
            return Err(OrderingError::new(
                "consensus v2 precommit would regress or conflict with durable lock",
            ));
        }
    }
    let mut next = state.clone();
    next.highest_precommit_round = Some(prepare_qc.round);
    next.locked_qc = Some(reference.clone());
    if next
        .high_qc
        .as_ref()
        .is_none_or(|current| qc_rank(&reference) > qc_rank(current))
    {
        next.high_qc = Some(reference.clone());
    }
    next.last_signed_vote_digest = Some(safety_vote_digest(
        prepare_qc.round,
        ConsensusV2Phase::Precommit,
        &reference.block.block_id,
    ));
    Ok(next)
}

pub fn consensus_v2_commit_from_precommit_qc(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    certificate: &ConsensusV2QuorumCertificate,
) -> Result<ConsensusV2BlockRef, OrderingError> {
    verify_consensus_v2_qc(domain, validators, certificate)?;
    if certificate.phase != ConsensusV2Phase::Precommit {
        return Err(OrderingError::new(
            "consensus v2 commit requires a precommit QC",
        ));
    }
    certificate
        .block
        .clone()
        .ok_or_else(|| OrderingError::new("nil precommit QC cannot commit a block"))
}

pub fn verify_consensus_v2_commit(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    commit: &ConsensusV2Commit,
    prior_qcs: &ConsensusV2QcGraph,
) -> Result<ConsensusV2BlockRef, OrderingError> {
    if commit.schema != CONSENSUS_V2_COMMIT_SCHEMA {
        return Err(OrderingError::new("unsupported consensus v2 commit schema"));
    }
    // Bound attacker-controlled ancestry before doing signature work. A
    // timeout certificate can reference at most one high QC per signer, and
    // only a quorum of timeout votes is material to the view change.
    if commit.prior_qcs.len() > validators.validators.len() {
        return Err(OrderingError::new(
            "consensus v2 commit contains excessive prior QC ancestry",
        ));
    }
    let mut commit_qcs = prior_qcs.clone();
    let mut prior_ids = BTreeSet::new();
    for certificate in &commit.prior_qcs {
        if certificate.round.height != commit.proposal.round.height
            || certificate.round >= commit.proposal.round
            || certificate.phase != ConsensusV2Phase::Prepare
            || !prior_ids.insert(certificate.certificate_id.as_str())
        {
            return Err(OrderingError::new(
                "consensus v2 commit prior QC ancestry is duplicate or out of scope",
            ));
        }
        commit_qcs.insert_verified(domain, validators, certificate.clone())?;
    }
    verify_consensus_v2_proposal(
        domain,
        validators,
        &commit.proposal,
        commit.timeout_certificate.as_ref(),
        &commit_qcs,
    )?;
    verify_consensus_v2_qc(domain, validators, &commit.prepare_qc)?;
    verify_consensus_v2_qc(domain, validators, &commit.precommit_qc)?;
    if commit.prepare_qc.round != commit.proposal.round
        || commit.prepare_qc.phase != ConsensusV2Phase::Prepare
        || commit.prepare_qc.block.as_ref() != Some(&commit.proposal.block)
        || commit.precommit_qc.round != commit.proposal.round
        || commit.precommit_qc.phase != ConsensusV2Phase::Precommit
        || commit.precommit_qc.block.as_ref() != Some(&commit.proposal.block)
    {
        return Err(OrderingError::new(
            "consensus v2 commit phases do not certify the proposal",
        ));
    }
    consensus_v2_commit_from_precommit_qc(domain, validators, &commit.precommit_qc)
}

/// Exhaustively checks every pair of quorum subsets for a small committee.
/// For `n=4` and `n=6`, every pair intersects beyond the Byzantine allowance,
/// leaving at least one honest durable lock in any candidate conflicting QC.
pub fn model_consensus_v2_quorum_intersection(
    validator_count: usize,
) -> Result<ConsensusV2SafetyModelReport, OrderingError> {
    if !(1..=20).contains(&validator_count) {
        return Err(OrderingError::new(
            "consensus v2 exhaustive model supports 1..=20 validators",
        ));
    }
    let fault_tolerance = super::bft_fault_tolerance(validator_count)?;
    let quorum = bft_quorum_threshold(validator_count)?;
    let limit = 1u64
        .checked_shl(validator_count as u32)
        .ok_or_else(|| OrderingError::new("consensus v2 model set overflow"))?;
    let quorum_sets = (0..limit)
        .filter(|mask| mask.count_ones() as usize >= quorum)
        .collect::<Vec<_>>();
    let mut quorum_pair_count = 0usize;
    let mut minimum_intersection = usize::MAX;
    let mut minimum_honest_intersection = usize::MAX;
    for (index, first) in quorum_sets.iter().enumerate() {
        for second in quorum_sets.iter().skip(index) {
            quorum_pair_count += 1;
            let intersection = (first & second).count_ones() as usize;
            minimum_intersection = minimum_intersection.min(intersection);
            minimum_honest_intersection =
                minimum_honest_intersection.min(intersection.saturating_sub(fault_tolerance));
        }
    }
    if minimum_honest_intersection == 0 {
        return Err(OrderingError::new(
            "consensus v2 quorum intersection contains no guaranteed honest validator",
        ));
    }
    Ok(ConsensusV2SafetyModelReport {
        validator_count,
        fault_tolerance,
        quorum,
        quorum_set_count: quorum_sets.len(),
        quorum_pair_count,
        minimum_intersection,
        minimum_honest_intersection,
    })
}

fn validate_domain_and_committee(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
) -> Result<(), OrderingError> {
    validate_consensus_v2_domain(domain)?;
    if domain.committee_root != validators.committee_root {
        return Err(OrderingError::new("consensus v2 committee root mismatch"));
    }
    Ok(())
}

fn validate_consensus_v2_domain(domain: &ConsensusV2Domain) -> Result<(), OrderingError> {
    if domain.schema != CONSENSUS_V2_DOMAIN_SCHEMA {
        return Err(OrderingError::new("unsupported consensus v2 domain schema"));
    }
    validate_text("consensus v2 chain ID", &domain.chain_id)?;
    validate_hash("consensus v2 genesis hash", &domain.genesis_hash)?;
    if domain.protocol_version == 0 {
        return Err(OrderingError::new(
            "consensus v2 protocol version must be nonzero",
        ));
    }
    validate_hash("consensus v2 committee root", &domain.committee_root)
}

fn validate_proposal_shape(proposal: &ConsensusV2Proposal) -> Result<(), OrderingError> {
    if proposal.schema != CONSENSUS_V2_PROPOSAL_SCHEMA {
        return Err(OrderingError::new(
            "unsupported consensus v2 proposal schema",
        ));
    }
    validate_consensus_v2_domain(&proposal.domain)?;
    if proposal.round.height == 0 {
        return Err(OrderingError::new(
            "consensus v2 proposal height must be nonzero",
        ));
    }
    validate_text("consensus v2 proposer", &proposal.proposer)?;
    validate_signature_shape(&proposal.signature)
}

fn validate_vote_shape(vote: &ConsensusV2Vote) -> Result<(), OrderingError> {
    if vote.schema != CONSENSUS_V2_VOTE_SCHEMA {
        return Err(OrderingError::new("unsupported consensus v2 vote schema"));
    }
    validate_consensus_v2_domain(&vote.domain)?;
    if vote.round.height == 0 {
        return Err(OrderingError::new(
            "consensus v2 vote height must be nonzero",
        ));
    }
    validate_text("consensus v2 vote validator", &vote.validator)?;
    validate_signature_shape(&vote.signature)
}

fn validate_timeout_vote_shape(vote: &ConsensusV2TimeoutVote) -> Result<(), OrderingError> {
    if vote.schema != CONSENSUS_V2_TIMEOUT_VOTE_SCHEMA {
        return Err(OrderingError::new(
            "unsupported consensus v2 timeout vote schema",
        ));
    }
    validate_consensus_v2_domain(&vote.domain)?;
    if vote.round.height == 0 {
        return Err(OrderingError::new(
            "consensus v2 timeout height must be nonzero",
        ));
    }
    validate_text("consensus v2 timeout validator", &vote.validator)?;
    validate_signature_shape(&vote.signature)
}

fn validate_safety_state(state: &ConsensusV2SafetyState) -> Result<(), OrderingError> {
    if state.schema != CONSENSUS_V2_SAFETY_STATE_SCHEMA {
        return Err(OrderingError::new(
            "unsupported consensus v2 safety-state schema",
        ));
    }
    validate_consensus_v2_domain(&state.domain)?;
    if state.current_height == 0 {
        return Err(OrderingError::new(
            "consensus v2 safety-state height must be nonzero",
        ));
    }
    Ok(())
}

fn validate_signature_shape(
    signature: &postfiat_types::ConsensusV2Signature,
) -> Result<(), OrderingError> {
    if signature.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(OrderingError::new(
            "consensus v2 signature algorithm mismatch",
        ));
    }
    validate_text("consensus v2 signature signer", &signature.signer)?;
    validate_hex_len(
        "consensus v2 signature public key",
        &signature.public_key_hex,
        1952,
    )?;
    let signature_bytes = hex_to_bytes(&signature.signature_hex)
        .map_err(|_| OrderingError::new("consensus v2 signature is not valid hex"))?;
    if signature_bytes.is_empty() {
        return Err(OrderingError::new(
            "consensus v2 signature must be nonempty",
        ));
    }
    Ok(())
}

fn verify_signature(
    validators: &ConsensusV2ValidatorSet,
    validator_id: &str,
    signature: &postfiat_types::ConsensusV2Signature,
    message: &[u8],
    context: &[u8],
) -> Result<(), OrderingError> {
    let validator = validators
        .get(validator_id)
        .ok_or_else(|| OrderingError::new("consensus v2 signer is not in the committee"))?;
    if signature.signer != validator_id || signature.public_key_hex != validator.public_key_hex {
        return Err(OrderingError::new(
            "consensus v2 signature identity mismatch",
        ));
    }
    let public_key = hex_to_bytes(&signature.public_key_hex)
        .map_err(|_| OrderingError::new("consensus v2 public key decode failed"))?;
    let signature_bytes = hex_to_bytes(&signature.signature_hex)
        .map_err(|_| OrderingError::new("consensus v2 signature decode failed"))?;
    if !ml_dsa_65_verify_with_context(&public_key, message, &signature_bytes, context) {
        return Err(OrderingError::new(
            "consensus v2 signature verification failed",
        ));
    }
    Ok(())
}

fn consensus_v2_qc_id(certificate: &ConsensusV2QuorumCertificate) -> Result<String, OrderingError> {
    if certificate.votes.is_empty() {
        return Err(OrderingError::new("consensus v2 QC has no votes"));
    }
    Ok(hash_canonical(CONSENSUS_V2_QC_ID_DOMAIN, |bytes| {
        append_consensus_v2_domain(bytes, &certificate.domain);
        append_round(bytes, certificate.round);
        append_phase(bytes, certificate.phase);
        append_optional_block(bytes, certificate.block.as_ref());
        append_usize_field(bytes, "validator_count", certificate.validators.len());
        for validator in &certificate.validators {
            append_str_field(bytes, "validator", validator);
        }
        append_usize_field(bytes, "quorum", certificate.quorum);
        append_usize_field(bytes, "vote_count", certificate.votes.len());
        for vote in &certificate.votes {
            bytes.extend(canonical_consensus_v2_vote(vote, true));
        }
    }))
}

fn consensus_v2_tc_id(
    certificate: &ConsensusV2TimeoutCertificate,
) -> Result<String, OrderingError> {
    if certificate.votes.is_empty() {
        return Err(OrderingError::new("consensus v2 TC has no votes"));
    }
    Ok(hash_canonical(CONSENSUS_V2_TC_ID_DOMAIN, |bytes| {
        append_consensus_v2_domain(bytes, &certificate.domain);
        append_round(bytes, certificate.round);
        append_phase(bytes, certificate.phase);
        append_optional_qc_ref(bytes, certificate.high_qc.as_ref());
        append_usize_field(bytes, "validator_count", certificate.validators.len());
        for validator in &certificate.validators {
            append_str_field(bytes, "validator", validator);
        }
        append_usize_field(bytes, "quorum", certificate.quorum);
        append_usize_field(bytes, "vote_count", certificate.votes.len());
        for vote in &certificate.votes {
            bytes.extend(canonical_consensus_v2_timeout_vote(vote, true));
        }
    }))
}

fn canonical_consensus_v2_proposal(
    proposal: &ConsensusV2Proposal,
    include_signature: bool,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_str_field(&mut bytes, "schema", &proposal.schema);
    append_consensus_v2_domain(&mut bytes, &proposal.domain);
    append_round(&mut bytes, proposal.round);
    append_block(&mut bytes, &proposal.block);
    append_optional_qc_ref(&mut bytes, proposal.valid_qc.as_ref());
    append_optional_string(
        &mut bytes,
        "timeout_certificate_id",
        proposal.timeout_certificate_id.as_deref(),
    );
    append_str_field(&mut bytes, "proposer", &proposal.proposer);
    if include_signature {
        append_signature(&mut bytes, &proposal.signature);
    }
    bytes
}

fn canonical_consensus_v2_vote(vote: &ConsensusV2Vote, include_signature: bool) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_str_field(&mut bytes, "schema", &vote.schema);
    append_consensus_v2_domain(&mut bytes, &vote.domain);
    append_round(&mut bytes, vote.round);
    append_phase(&mut bytes, vote.phase);
    append_optional_block(&mut bytes, vote.block.as_ref());
    append_str_field(&mut bytes, "validator", &vote.validator);
    if include_signature {
        append_signature(&mut bytes, &vote.signature);
    }
    bytes
}

fn canonical_consensus_v2_timeout_vote(
    vote: &ConsensusV2TimeoutVote,
    include_signature: bool,
) -> Vec<u8> {
    let mut bytes = Vec::new();
    append_str_field(&mut bytes, "schema", &vote.schema);
    append_consensus_v2_domain(&mut bytes, &vote.domain);
    append_round(&mut bytes, vote.round);
    append_phase(&mut bytes, vote.phase);
    append_optional_qc_ref(&mut bytes, vote.high_qc.as_ref());
    append_str_field(&mut bytes, "validator", &vote.validator);
    if include_signature {
        append_signature(&mut bytes, &vote.signature);
    }
    bytes
}

fn append_consensus_v2_domain(bytes: &mut Vec<u8>, domain: &ConsensusV2Domain) {
    append_str_field(bytes, "domain.schema", &domain.schema);
    append_str_field(bytes, "domain.chain_id", &domain.chain_id);
    append_str_field(bytes, "domain.genesis_hash", &domain.genesis_hash);
    append_u32_field(bytes, "domain.protocol_version", domain.protocol_version);
    append_u64_field(bytes, "domain.committee_epoch", domain.committee_epoch);
    append_str_field(bytes, "domain.committee_root", &domain.committee_root);
}

fn append_round(bytes: &mut Vec<u8>, round: ConsensusV2Round) {
    append_u64_field(bytes, "round.height", round.height);
    append_u64_field(bytes, "round.view", round.view);
}

fn append_phase(bytes: &mut Vec<u8>, phase: ConsensusV2Phase) {
    append_str_field(
        bytes,
        "phase",
        match phase {
            ConsensusV2Phase::Prepare => "prepare",
            ConsensusV2Phase::Precommit => "precommit",
        },
    );
}

fn append_block(bytes: &mut Vec<u8>, block: &ConsensusV2BlockRef) {
    append_u64_field(bytes, "block.height", block.height);
    append_str_field(bytes, "block.block_id", &block.block_id);
    append_str_field(bytes, "block.parent_block_id", &block.parent_block_id);
    append_str_field(bytes, "block.payload_hash", &block.payload_hash);
    append_str_field(bytes, "block.state_root", &block.state_root);
    if let Some(root) = block.bridge_exit_root.as_deref() {
        append_str_field(bytes, "block.bridge_exit_root", root);
    }
}

fn append_optional_block(bytes: &mut Vec<u8>, block: Option<&ConsensusV2BlockRef>) {
    append_u32_field(bytes, "block.present", u32::from(block.is_some()));
    if let Some(block) = block {
        append_block(bytes, block);
    }
}

fn append_qc_ref(bytes: &mut Vec<u8>, reference: &ConsensusV2QcRef) {
    append_str_field(bytes, "qc.certificate_id", &reference.certificate_id);
    append_round(bytes, reference.round);
    append_phase(bytes, reference.phase);
    append_block(bytes, &reference.block);
}

fn append_optional_qc_ref(bytes: &mut Vec<u8>, reference: Option<&ConsensusV2QcRef>) {
    append_u32_field(bytes, "qc.present", u32::from(reference.is_some()));
    if let Some(reference) = reference {
        append_qc_ref(bytes, reference);
    }
}

fn append_optional_string(bytes: &mut Vec<u8>, label: &str, value: Option<&str>) {
    append_u32_field(
        bytes,
        &format!("{label}.present"),
        u32::from(value.is_some()),
    );
    if let Some(value) = value {
        append_str_field(bytes, label, value);
    }
}

fn append_signature(bytes: &mut Vec<u8>, signature: &postfiat_types::ConsensusV2Signature) {
    append_str_field(bytes, "signature.algorithm_id", &signature.algorithm_id);
    append_str_field(bytes, "signature.signer", &signature.signer);
    append_str_field(bytes, "signature.public_key_hex", &signature.public_key_hex);
    append_str_field(bytes, "signature.signature_hex", &signature.signature_hex);
}

fn qc_rank(reference: &ConsensusV2QcRef) -> (u64, u64, ConsensusV2Phase) {
    (
        reference.round.height,
        reference.round.view,
        reference.phase,
    )
}

fn highest_timeout_qc(
    votes: &[ConsensusV2TimeoutVote],
) -> Result<Option<ConsensusV2QcRef>, OrderingError> {
    let mut highest = None::<ConsensusV2QcRef>;
    for reference in votes.iter().filter_map(|vote| vote.high_qc.as_ref()) {
        match highest.as_ref() {
            None => highest = Some(reference.clone()),
            Some(current) if qc_rank(reference) > qc_rank(current) => {
                highest = Some(reference.clone());
            }
            Some(current) if qc_rank(reference) == qc_rank(current) && reference != current => {
                return Err(OrderingError::new(
                    "consensus v2 timeout votes contain conflicting QCs at one numeric rank",
                ));
            }
            Some(_) => {}
        }
    }
    Ok(highest)
}

fn safety_vote_digest(round: ConsensusV2Round, phase: ConsensusV2Phase, block_id: &str) -> String {
    hash_canonical(CONSENSUS_V2_SAFETY_DIGEST_DOMAIN, |bytes| {
        append_round(bytes, round);
        append_phase(bytes, phase);
        append_str_field(bytes, "block_id", block_id);
    })
}

fn timeout_safety_vote_digest(
    round: ConsensusV2Round,
    high_qc: Option<&ConsensusV2QcRef>,
) -> String {
    hash_canonical(CONSENSUS_V2_TIMEOUT_SAFETY_DIGEST_DOMAIN, |bytes| {
        append_round(bytes, round);
        match high_qc {
            Some(reference) => {
                append_u32_field(bytes, "has_high_qc", 1);
                append_qc_ref(bytes, reference);
            }
            None => append_u32_field(bytes, "has_high_qc", 0),
        }
    })
}

fn validate_text(label: &str, value: &str) -> Result<(), OrderingError> {
    if value.trim().is_empty() || value.len() > 256 || value.chars().any(char::is_control) {
        return Err(OrderingError::new(format!("{label} is invalid")));
    }
    Ok(())
}

fn validate_hash(label: &str, value: &str) -> Result<(), OrderingError> {
    validate_hex_len(label, value, 48)
}

fn validate_hex_len(label: &str, value: &str, bytes: usize) -> Result<(), OrderingError> {
    let decoded =
        hex_to_bytes(value).map_err(|_| OrderingError::new(format!("{label} is not hex")))?;
    if decoded.len() != bytes || value.bytes().any(|byte| byte.is_ascii_uppercase()) {
        return Err(OrderingError::new(format!(
            "{label} must be {bytes}-byte lowercase hex"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests;
