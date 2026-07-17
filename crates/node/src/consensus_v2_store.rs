use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use postfiat_ordering_fast::{
    authorize_consensus_v2_precommit_vote, authorize_consensus_v2_prepare_vote,
    authorize_consensus_v2_timeout_vote, consensus_v2_domain, initial_consensus_v2_safety_state,
    ConsensusV2QcGraph, ConsensusV2Validator, ConsensusV2ValidatorSet,
};
use postfiat_types::{
    ConsensusV2Domain, ConsensusV2Proposal, ConsensusV2QcRef, ConsensusV2QuorumCertificate,
    ConsensusV2Round, ConsensusV2SafetyState, ConsensusV2TimeoutCertificate,
    CONSENSUS_V2_SAFETY_STATE_SCHEMA,
};

use crate::{atomic_write, genesis_hash, load_validator_pubkeys, NodeStore};

#[doc(hidden)]
pub const CONSENSUS_V2_SAFETY_DIR: &str = "consensus-v2-safety";
#[doc(hidden)]
pub const CONSENSUS_V2_QC_DIR: &str = "consensus-v2-qcs";

pub fn live_consensus_v2_context(
    data_dir: &Path,
) -> io::Result<(ConsensusV2Domain, ConsensusV2ValidatorSet)> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let committed_height = store
        .read_blocks()?
        .blocks
        .last()
        .map_or(0, |block| block.header.height);
    let committee_epoch = 1u64
        .checked_add(
            governance
                .validator_registry_updates
                .iter()
                .filter(|update| update.activation_height <= committed_height)
                .count() as u64,
        )
        .ok_or_else(|| invalid_data("consensus v2 committee epoch overflow"))?;
    let validators = load_validator_pubkeys(data_dir)?
        .into_iter()
        .map(|(validator_id, public_key_hex)| ConsensusV2Validator {
            validator_id,
            public_key_hex,
        })
        .collect::<Vec<_>>();
    let validators = ConsensusV2ValidatorSet::try_new(validators)
        .map_err(|error| invalid_data(format!("consensus v2 validator set: {error}")))?;
    let live_genesis_hash = genesis_hash(&genesis);
    let domain = consensus_v2_domain(
        genesis.chain_id,
        live_genesis_hash,
        genesis.protocol_version,
        committee_epoch,
        &validators,
    );
    Ok((domain, validators))
}

pub fn read_consensus_v2_safety_state(
    data_dir: &Path,
    domain: &ConsensusV2Domain,
    height: u64,
) -> io::Result<ConsensusV2SafetyState> {
    let path = consensus_v2_safety_path(data_dir, domain, height);
    match std::fs::read(&path) {
        Ok(bytes) => {
            let state: ConsensusV2SafetyState =
                serde_json::from_slice(&bytes).map_err(|error| {
                    invalid_data(format!(
                        "consensus v2 safety state `{}` parse failed: {error}",
                        path.display()
                    ))
                })?;
            validate_live_safety_state(&state, domain, height)?;
            Ok(state)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            // Compatibility with v2 safety state written before artifacts were
            // namespaced by committee domain.  The embedded domain remains the
            // authority, so a stale/mismatched legacy file still fails closed.
            let legacy_path = legacy_consensus_v2_safety_path(data_dir, height);
            match std::fs::read(&legacy_path) {
                Ok(bytes) => {
                    let state: ConsensusV2SafetyState =
                        serde_json::from_slice(&bytes).map_err(|error| {
                            invalid_data(format!(
                                "consensus v2 safety state `{}` parse failed: {error}",
                                legacy_path.display()
                            ))
                        })?;
                    validate_live_safety_state(&state, domain, height)?;
                    Ok(state)
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {
                    initial_consensus_v2_safety_state(domain, height).map_err(|error| {
                        invalid_data(format!("consensus v2 initial safety state: {error}"))
                    })
                }
                Err(error) => Err(error),
            }
        }
        Err(error) => Err(error),
    }
}

/// Verify a complete proposal and persist the resulting prepare-vote safety
/// state before the caller is allowed to emit a signature.
pub fn persist_consensus_v2_prepare_authorization(
    data_dir: &Path,
    proposal: &ConsensusV2Proposal,
    timeout_certificate: Option<&ConsensusV2TimeoutCertificate>,
    qc_graph: &ConsensusV2QcGraph,
) -> io::Result<ConsensusV2SafetyState> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    if proposal.domain != domain {
        return Err(invalid_data(
            "consensus v2 proposal does not use live domain",
        ));
    }
    with_safety_guard(data_dir, proposal.round.height, || {
        let current = read_consensus_v2_safety_state(data_dir, &domain, proposal.round.height)?;
        let next = authorize_consensus_v2_prepare_vote(
            &current,
            &domain,
            &validators,
            proposal,
            timeout_certificate,
            qc_graph,
        )
        .map_err(|error| invalid_data(format!("consensus v2 prepare authorization: {error}")))?;
        write_consensus_v2_safety_state(data_dir, &next)?;
        Ok(next)
    })
}

/// Verify a prepare QC and persist the lock/high-QC/precommit round before the
/// caller is allowed to emit a precommit signature.
pub fn persist_consensus_v2_precommit_authorization(
    data_dir: &Path,
    prepare_qc: &ConsensusV2QuorumCertificate,
) -> io::Result<ConsensusV2SafetyState> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    if prepare_qc.domain != domain {
        return Err(invalid_data(
            "consensus v2 prepare QC does not use live domain",
        ));
    }
    with_safety_guard(data_dir, prepare_qc.round.height, || {
        let current = read_consensus_v2_safety_state(data_dir, &domain, prepare_qc.round.height)?;
        let next =
            authorize_consensus_v2_precommit_vote(&current, &domain, &validators, prepare_qc)
                .map_err(|error| {
                    invalid_data(format!("consensus v2 precommit authorization: {error}"))
                })?;
        write_consensus_v2_safety_state(data_dir, &next)?;
        Ok(next)
    })
}

/// Persist a verified QC before it may be referenced by a later-view artifact.
/// Existing IDs are immutable: a conflicting replacement fails closed.
pub fn persist_consensus_v2_qc(
    data_dir: &Path,
    certificate: &ConsensusV2QuorumCertificate,
) -> io::Result<ConsensusV2QcRef> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    if certificate.domain != domain {
        return Err(invalid_data("consensus v2 QC does not use live domain"));
    }
    let mut graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
    let reference = graph
        .insert_verified(&domain, &validators, certificate.clone())
        .map_err(|error| invalid_data(format!("consensus v2 QC: {error}")))?;
    with_safety_guard(data_dir, certificate.round.height, || {
        let path = consensus_v2_qc_path(data_dir, &certificate.domain, &certificate.certificate_id);
        match std::fs::read(&path) {
            Ok(bytes) => {
                let existing: ConsensusV2QuorumCertificate = serde_json::from_slice(&bytes)
                    .map_err(|error| {
                        invalid_data(format!(
                            "consensus v2 QC `{}` parse failed: {error}",
                            path.display()
                        ))
                    })?;
                if existing != *certificate {
                    return Err(invalid_data(
                        "consensus v2 QC ID already contains different certificate",
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                let mut bytes = serde_json::to_vec_pretty(certificate).map_err(invalid_data)?;
                bytes.push(b'\n');
                atomic_write(&path, bytes)?;
            }
            Err(error) => return Err(error),
        }
        Ok(reference.clone())
    })
}

pub fn read_consensus_v2_qc_graph(
    data_dir: &Path,
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
) -> io::Result<ConsensusV2QcGraph> {
    let qc_dir = data_dir.join(CONSENSUS_V2_QC_DIR);
    let mut paths = match std::fs::read_dir(&qc_dir) {
        Ok(entries) => entries
            .map(|entry| entry.map(|entry| entry.path()))
            .collect::<io::Result<Vec<_>>>()?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error),
    };
    paths.sort();
    let mut graph = ConsensusV2QcGraph::default();
    for path in paths {
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path)?;
        let certificate: ConsensusV2QuorumCertificate =
            serde_json::from_slice(&bytes).map_err(|error| {
                invalid_data(format!(
                    "consensus v2 QC `{}` parse failed: {error}",
                    path.display()
                ))
            })?;
        let expected_path =
            consensus_v2_qc_path(data_dir, &certificate.domain, &certificate.certificate_id);
        let legacy_path = legacy_consensus_v2_qc_path(data_dir, &certificate.certificate_id);
        if path != expected_path && path != legacy_path {
            return Err(invalid_data(format!(
                "consensus v2 QC path `{}` does not match certificate ID",
                path.display()
            )));
        }
        if certificate.domain != *domain {
            // QCs from prior committee epochs remain durable audit evidence but
            // are not part of the live epoch's lock graph.
            continue;
        }
        graph
            .insert_verified(domain, validators, certificate)
            .map_err(|error| invalid_data(format!("consensus v2 persisted QC: {error}")))?;
    }
    Ok(graph)
}

/// Advance and persist the timeout high-water mark before timeout signing.
pub fn persist_consensus_v2_timeout_authorization(
    data_dir: &Path,
    round: ConsensusV2Round,
    high_qc: Option<&ConsensusV2QcRef>,
) -> io::Result<ConsensusV2SafetyState> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    with_safety_guard(data_dir, round.height, || {
        let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
        let current = read_consensus_v2_safety_state(data_dir, &domain, round.height)?;
        let next = authorize_consensus_v2_timeout_vote(
            &current,
            &domain,
            &validators,
            round,
            high_qc,
            &graph,
        )
        .map_err(|error| invalid_data(format!("consensus v2 timeout authorization: {error}")))?;
        write_consensus_v2_safety_state(data_dir, &next)?;
        Ok(next)
    })
}

fn write_consensus_v2_safety_state(
    data_dir: &Path,
    state: &ConsensusV2SafetyState,
) -> io::Result<()> {
    validate_live_safety_state(state, &state.domain, state.current_height)?;
    let path = consensus_v2_safety_path(data_dir, &state.domain, state.current_height);
    let mut bytes = serde_json::to_vec_pretty(state).map_err(invalid_data)?;
    bytes.push(b'\n');
    atomic_write(path, bytes)
}

fn validate_live_safety_state(
    state: &ConsensusV2SafetyState,
    domain: &ConsensusV2Domain,
    height: u64,
) -> io::Result<()> {
    if state.schema != CONSENSUS_V2_SAFETY_STATE_SCHEMA
        || state.domain != *domain
        || state.current_height != height
    {
        return Err(invalid_data(
            "consensus v2 safety state schema, domain, or height mismatch",
        ));
    }
    Ok(())
}

fn consensus_v2_artifact_prefix(domain: &ConsensusV2Domain) -> String {
    format!("epoch-{}-{}", domain.committee_epoch, domain.committee_root)
}

fn consensus_v2_safety_path(data_dir: &Path, domain: &ConsensusV2Domain, height: u64) -> PathBuf {
    data_dir.join(CONSENSUS_V2_SAFETY_DIR).join(format!(
        "{}-height-{height}.json",
        consensus_v2_artifact_prefix(domain)
    ))
}

fn legacy_consensus_v2_safety_path(data_dir: &Path, height: u64) -> PathBuf {
    data_dir
        .join(CONSENSUS_V2_SAFETY_DIR)
        .join(format!("height-{height}.json"))
}

fn consensus_v2_qc_path(
    data_dir: &Path,
    domain: &ConsensusV2Domain,
    certificate_id: &str,
) -> PathBuf {
    data_dir.join(CONSENSUS_V2_QC_DIR).join(format!(
        "{}-{certificate_id}.json",
        consensus_v2_artifact_prefix(domain)
    ))
}

fn legacy_consensus_v2_qc_path(data_dir: &Path, certificate_id: &str) -> PathBuf {
    data_dir
        .join(CONSENSUS_V2_QC_DIR)
        .join(format!("{certificate_id}.json"))
}

fn with_safety_guard<T>(
    data_dir: &Path,
    height: u64,
    action: impl FnOnce() -> io::Result<T>,
) -> io::Result<T> {
    let safety_dir = data_dir.join(CONSENSUS_V2_SAFETY_DIR);
    std::fs::create_dir_all(&safety_dir)?;
    let guard_path = safety_dir.join(format!("height-{height}.guard"));
    let guard = open_guard(&guard_path)?;
    guard.lock()?;
    action()
}

fn open_guard(path: &Path) -> io::Result<File> {
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
}

fn invalid_data(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{
        bytes_to_hex, hex_to_bytes, ml_dsa_65_sign_with_context, ML_DSA_65_ALGORITHM,
    };
    use postfiat_ordering_fast::{
        certify_consensus_v2_votes, consensus_v2_block_ref, consensus_v2_proposal_signing_bytes,
        consensus_v2_vote_signing_bytes, leader_for_view, CONSENSUS_V2_PROPOSAL_CONTEXT,
        CONSENSUS_V2_VOTE_CONTEXT,
    };
    use postfiat_types::{
        ConsensusV2Phase, ConsensusV2Proposal, ConsensusV2Round, ConsensusV2Signature,
        ConsensusV2Vote, CONSENSUS_V2_PROPOSAL_SCHEMA, CONSENSUS_V2_VOTE_SCHEMA,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{
        init, init_consensus_v2, write_consensus_v2_topology, InitConsensusV2Options, InitOptions,
        TopologyConsensusV2Options, ValidatorKeyFile, VALIDATOR_KEYS_FILE,
    };

    fn unique_data_dir() -> PathBuf {
        std::env::temp_dir().join(format!(
            "postfiat-consensus-v2-safety-store-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn consensus_v2_genesis_and_topology_share_exact_activation_domain() {
        let data_dir = unique_data_dir();
        let topology_file = data_dir.with_extension("topology.json");
        let status = init_consensus_v2(InitConsensusV2Options {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-consensus-v2-activation-test".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 4,
            activation_height: 7,
        })
        .expect("init consensus v2 activation chain");
        let genesis = NodeStore::new(&data_dir)
            .read_genesis()
            .expect("read v2 genesis");
        assert_eq!(genesis.consensus_v2_activation_height, Some(7));
        let topology = write_consensus_v2_topology(TopologyConsensusV2Options {
            chain_id: genesis.chain_id.clone(),
            validators: 4,
            base_port: postfiat_network::DEFAULT_BASE_PORT,
            rpc_base_port: None,
            hosts: None,
            output_file: topology_file.clone(),
            activation_height: 7,
        })
        .expect("write consensus v2 topology");
        assert_eq!(topology.chain_id, status.chain_id);
        assert_eq!(topology.genesis_hash, status.genesis_hash);
        assert_eq!(topology.protocol_version, status.protocol_version);

        std::fs::remove_dir_all(data_dir).expect("cleanup consensus v2 activation chain");
        std::fs::remove_file(topology_file).expect("cleanup consensus v2 topology");
    }

    #[test]
    fn prepare_authorization_is_persisted_before_duplicate_or_conflicting_vote() {
        let data_dir = unique_data_dir();
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-consensus-v2-store-test".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 4,
        })
        .expect("init consensus v2 store");
        let (domain, validators) = live_consensus_v2_context(&data_dir).expect("live context");
        let round = ConsensusV2Round { height: 1, view: 0 };
        let block = consensus_v2_block_ref(
            &domain,
            1,
            "11".repeat(48),
            "22".repeat(48),
            "33".repeat(48),
        )
        .expect("block reference");
        let proposer = leader_for_view(&validators.validator_ids(), 1, 0).expect("leader");
        let key_file: ValidatorKeyFile = serde_json::from_slice(
            &std::fs::read(data_dir.join(VALIDATOR_KEYS_FILE)).expect("read validator keys"),
        )
        .expect("parse validator keys");
        let key = key_file
            .validators
            .iter()
            .find(|record| record.node_id == proposer)
            .expect("proposer key");
        let mut proposal = ConsensusV2Proposal {
            schema: CONSENSUS_V2_PROPOSAL_SCHEMA.to_string(),
            domain: domain.clone(),
            round,
            block,
            valid_qc: None,
            timeout_certificate_id: None,
            proposer: proposer.clone(),
            signature: ConsensusV2Signature {
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                signer: proposer,
                public_key_hex: key.public_key_hex.clone(),
                signature_hex: "00".to_string(),
            },
        };
        let signing_bytes =
            consensus_v2_proposal_signing_bytes(&proposal).expect("proposal signing bytes");
        proposal.signature.signature_hex = bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &hex_to_bytes(&key.private_key_hex).expect("private key"),
                &signing_bytes,
                CONSENSUS_V2_PROPOSAL_CONTEXT,
            )
            .expect("proposal signature"),
        );

        let persisted = persist_consensus_v2_prepare_authorization(
            &data_dir,
            &proposal,
            None,
            &ConsensusV2QcGraph::default(),
        )
        .expect("persist prepare authorization");
        assert_eq!(persisted.highest_prepare_round, Some(round));
        let restarted =
            read_consensus_v2_safety_state(&data_dir, &domain, 1).expect("restart read");
        assert_eq!(restarted, persisted);
        let duplicate = persist_consensus_v2_prepare_authorization(
            &data_dir,
            &proposal,
            None,
            &ConsensusV2QcGraph::default(),
        )
        .expect_err("duplicate vote after restart must fail closed");
        assert!(duplicate.to_string().contains("round monotonicity"));

        let prepare_votes = validators
            .validators
            .iter()
            .take(validators.quorum)
            .map(|validator| {
                let record = key_file
                    .validators
                    .iter()
                    .find(|record| record.node_id == validator.validator_id)
                    .expect("prepare voter key");
                let mut vote = ConsensusV2Vote {
                    schema: CONSENSUS_V2_VOTE_SCHEMA.to_string(),
                    domain: domain.clone(),
                    round,
                    phase: ConsensusV2Phase::Prepare,
                    block: Some(proposal.block.clone()),
                    validator: validator.validator_id.clone(),
                    signature: ConsensusV2Signature {
                        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                        signer: validator.validator_id.clone(),
                        public_key_hex: validator.public_key_hex.clone(),
                        signature_hex: "00".to_string(),
                    },
                };
                let signing_bytes =
                    consensus_v2_vote_signing_bytes(&vote).expect("vote signing bytes");
                vote.signature.signature_hex = bytes_to_hex(
                    &ml_dsa_65_sign_with_context(
                        &hex_to_bytes(&record.private_key_hex).expect("voter private key"),
                        &signing_bytes,
                        CONSENSUS_V2_VOTE_CONTEXT,
                    )
                    .expect("prepare vote signature"),
                );
                vote
            })
            .collect::<Vec<_>>();
        let prepare_qc = certify_consensus_v2_votes(
            &domain,
            &validators,
            round,
            ConsensusV2Phase::Prepare,
            Some(proposal.block.clone()),
            prepare_votes,
        )
        .expect("prepare QC");
        let persisted_qc =
            persist_consensus_v2_qc(&data_dir, &prepare_qc).expect("persist verified prepare QC");
        let reloaded_graph = read_consensus_v2_qc_graph(&data_dir, &domain, &validators)
            .expect("reload verified QC graph");
        reloaded_graph
            .resolve_verified(&domain, &validators, &persisted_qc)
            .expect("resolve persisted prepare QC after restart");
        let timeout_state =
            persist_consensus_v2_timeout_authorization(&data_dir, round, Some(&persisted_qc))
                .expect("persist timeout authorization");
        assert_eq!(timeout_state.highest_timeout_round, Some(round));
        assert!(timeout_state.last_signed_timeout_digest.is_some());
        let duplicate_timeout =
            persist_consensus_v2_timeout_authorization(&data_dir, round, Some(&persisted_qc))
                .expect_err("duplicate timeout after restart must fail closed");
        assert!(duplicate_timeout
            .to_string()
            .contains("timeout vote would violate durable round monotonicity"));
        let locked = persist_consensus_v2_precommit_authorization(&data_dir, &prepare_qc)
            .expect("persist precommit authorization");
        assert_eq!(
            locked.locked_qc.as_ref().map(|qc| qc.block.clone()),
            Some(proposal.block.clone())
        );
        let restarted_locked =
            read_consensus_v2_safety_state(&data_dir, &domain, 1).expect("restart locked state");
        assert_eq!(restarted_locked, locked);
        let duplicate_precommit =
            persist_consensus_v2_precommit_authorization(&data_dir, &prepare_qc)
                .expect_err("duplicate precommit after restart must fail closed");
        assert!(duplicate_precommit
            .to_string()
            .contains("newer non-nil prepare QC"));

        std::fs::remove_dir_all(data_dir).expect("cleanup consensus v2 store");
    }
}
