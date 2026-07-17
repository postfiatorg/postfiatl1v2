pub const CONSENSUS_V2_DOMAIN_SCHEMA: &str = "postfiat-consensus-domain-v2";
pub const CONSENSUS_V2_PROPOSAL_SCHEMA: &str = "postfiat-consensus-proposal-v2";
pub const CONSENSUS_V2_VOTE_SCHEMA: &str = "postfiat-consensus-vote-v2";
pub const CONSENSUS_V2_QC_SCHEMA: &str = "postfiat-consensus-qc-v2";
pub const CONSENSUS_V2_TIMEOUT_VOTE_SCHEMA: &str = "postfiat-consensus-timeout-vote-v2";
pub const CONSENSUS_V2_TC_SCHEMA: &str = "postfiat-consensus-tc-v2";
pub const CONSENSUS_V2_SAFETY_STATE_SCHEMA: &str = "postfiat-consensus-safety-state-v2";
pub const CONSENSUS_V2_COMMIT_SCHEMA: &str = "postfiat-consensus-commit-v2";

/// Exact chain and committee identity covered by every v2 consensus artifact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2Domain {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub committee_epoch: u64,
    pub committee_root: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ConsensusV2Round {
    pub height: u64,
    pub view: u64,
}

/// V2 uses two explicit voting phases. A prepare QC is the only evidence that
/// can update a lock; a precommit QC is the commit certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConsensusV2Phase {
    Prepare,
    Precommit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2BlockRef {
    pub height: u64,
    pub block_id: String,
    pub parent_block_id: String,
    pub payload_hash: String,
    pub state_root: String,
    /// Present only at and after the independently governed Tier-4 bridge-exit
    /// commitment activation. Legacy blocks omit this field and retain their
    /// original block-ID and signing encodings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge_exit_root: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2QcRef {
    pub certificate_id: String,
    pub round: ConsensusV2Round,
    pub phase: ConsensusV2Phase,
    pub block: ConsensusV2BlockRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2Signature {
    pub algorithm_id: String,
    pub signer: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2Proposal {
    pub schema: String,
    pub domain: ConsensusV2Domain,
    pub round: ConsensusV2Round,
    pub block: ConsensusV2BlockRef,
    /// A typed, verified prepare QC from an earlier view at this height. This
    /// replaces the legacy opaque `high_qc_id` string.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub valid_qc: Option<ConsensusV2QcRef>,
    /// Required for nonzero views and bound into the proposal signature.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_certificate_id: Option<String>,
    pub proposer: String,
    pub signature: ConsensusV2Signature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2Vote {
    pub schema: String,
    pub domain: ConsensusV2Domain,
    pub round: ConsensusV2Round,
    pub phase: ConsensusV2Phase,
    /// `None` is an explicit nil vote. It can advance a round but can never be
    /// used as a state-transition or commit certificate.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<ConsensusV2BlockRef>,
    pub validator: String,
    pub signature: ConsensusV2Signature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2QuorumCertificate {
    pub schema: String,
    pub domain: ConsensusV2Domain,
    pub round: ConsensusV2Round,
    pub phase: ConsensusV2Phase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block: Option<ConsensusV2BlockRef>,
    pub validators: Vec<String>,
    pub quorum: usize,
    pub votes: Vec<ConsensusV2Vote>,
    pub certificate_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2TimeoutVote {
    pub schema: String,
    pub domain: ConsensusV2Domain,
    pub round: ConsensusV2Round,
    pub phase: ConsensusV2Phase,
    /// Typed QC reference. Ranking is numeric by `(height, view, phase)` and
    /// every referenced QC must be resolved and verified before aggregation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_qc: Option<ConsensusV2QcRef>,
    pub validator: String,
    pub signature: ConsensusV2Signature,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2TimeoutCertificate {
    pub schema: String,
    pub domain: ConsensusV2Domain,
    pub round: ConsensusV2Round,
    pub phase: ConsensusV2Phase,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_qc: Option<ConsensusV2QcRef>,
    pub validators: Vec<String>,
    pub quorum: usize,
    pub votes: Vec<ConsensusV2TimeoutVote>,
    pub certificate_id: String,
}

/// Self-contained finality artifact. A non-nil precommit QC is the only v2
/// commit authority; the prepare QC and optional timeout certificate make its
/// lock/view ancestry independently verifiable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2Commit {
    pub schema: String,
    pub proposal: ConsensusV2Proposal,
    /// Earlier quorum certificates referenced by a view-change proposal or
    /// timeout certificate. Embedding them makes the finality artifact
    /// independently verifiable during replay and snapshot restore.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub prior_qcs: Vec<ConsensusV2QuorumCertificate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_certificate: Option<ConsensusV2TimeoutCertificate>,
    pub prepare_qc: ConsensusV2QuorumCertificate,
    pub precommit_qc: ConsensusV2QuorumCertificate,
}

/// Persisted and fsynced before emitting any vote signature. One record covers
/// both phases so restart cannot roll back the validator's lock or vote round.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConsensusV2SafetyState {
    pub schema: String,
    pub domain: ConsensusV2Domain,
    pub current_height: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highest_prepare_round: Option<ConsensusV2Round>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highest_precommit_round: Option<ConsensusV2Round>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub highest_timeout_round: Option<ConsensusV2Round>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locked_qc: Option<ConsensusV2QcRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub high_qc: Option<ConsensusV2QcRef>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_signed_vote_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_signed_timeout_digest: Option<String>,
}
