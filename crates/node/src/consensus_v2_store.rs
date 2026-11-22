use std::fs::{File, OpenOptions};
use std::io;
use std::path::{Path, PathBuf};

use postfiat_ordering_fast::{
    authorize_consensus_v2_precommit_vote, authorize_consensus_v2_prepare_vote,
    authorize_consensus_v2_timeout_vote, consensus_v2_domain, consensus_v2_qc_ref,
    initial_consensus_v2_safety_state, verify_consensus_v2_qc, verify_consensus_v2_timeout_vote,
    ConsensusV2QcGraph, ConsensusV2Validator, ConsensusV2ValidatorSet,
};
use postfiat_types::{
    ConsensusV2Domain, ConsensusV2Phase, ConsensusV2Proposal, ConsensusV2QcRef,
    ConsensusV2QuorumCertificate, ConsensusV2Round, ConsensusV2SafetyState,
    ConsensusV2TimeoutCertificate, ConsensusV2TimeoutVote, CONSENSUS_V2_SAFETY_STATE_SCHEMA,
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
    let committed_height = match store.read_chain_tip() {
        Ok(tip) => {
            if tip.chain_id != genesis.chain_id
                || tip.genesis_hash != genesis_hash(&genesis)
                || tip.protocol_version != genesis.protocol_version
            {
                return Err(invalid_data(
                    "consensus v2 chain tip does not match local genesis",
                ));
            }
            tip.height
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => store
            .read_blocks()?
            .blocks
            .last()
            .map_or(0, |block| block.header.height),
        Err(error) => return Err(error),
    };
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
    persist_consensus_v2_prepare_authorization_with(
        data_dir,
        proposal,
        timeout_certificate,
        qc_graph,
        |_, _, next| Ok(next.clone()),
    )
}

/// Keep the per-height safety guard held through the caller's signature
/// construction so another phase cannot advance the round floor first.
pub(crate) fn persist_consensus_v2_prepare_authorization_with<T>(
    data_dir: &Path,
    proposal: &ConsensusV2Proposal,
    timeout_certificate: Option<&ConsensusV2TimeoutCertificate>,
    qc_graph: &ConsensusV2QcGraph,
    after_persist: impl FnOnce(
        &ConsensusV2Domain,
        &ConsensusV2ValidatorSet,
        &ConsensusV2SafetyState,
    ) -> io::Result<T>,
) -> io::Result<T> {
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
        after_persist(&domain, &validators, &next)
    })
}

/// Verify a prepare QC and persist the lock/high-QC/precommit round before the
/// caller is allowed to emit a precommit signature.
pub fn persist_consensus_v2_precommit_authorization(
    data_dir: &Path,
    prepare_qc: &ConsensusV2QuorumCertificate,
) -> io::Result<ConsensusV2SafetyState> {
    persist_consensus_v2_precommit_authorization_with(data_dir, prepare_qc, |_, _, next| {
        Ok(next.clone())
    })
}

/// Keep the per-height safety guard held through the caller's signature
/// construction so another phase cannot advance the round floor first.
pub(crate) fn persist_consensus_v2_precommit_authorization_with<T>(
    data_dir: &Path,
    prepare_qc: &ConsensusV2QuorumCertificate,
    after_persist: impl FnOnce(
        &ConsensusV2Domain,
        &ConsensusV2ValidatorSet,
        &ConsensusV2SafetyState,
    ) -> io::Result<T>,
) -> io::Result<T> {
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
        after_persist(&domain, &validators, &next)
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
    // A QC is self-contained: its committee, target, signatures, quorum and
    // canonical ID can be verified without rebuilding unrelated historical
    // QCs. Cross-QC references are resolved when proposals, timeouts and
    // commits are verified. Re-reading the complete append-only graph here
    // made every new QC persistence operation grow linearly with chain
    // height without adding a safety check for this certificate.
    verify_consensus_v2_qc(&domain, &validators, certificate)
        .map_err(|error| invalid_data(format!("consensus v2 QC: {error}")))?;
    let reference = consensus_v2_qc_ref(certificate)
        .map_err(|error| invalid_data(format!("consensus v2 QC reference: {error}")))?;
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

/// The stored QC `reference` names, if this validator holds it. Not verified
/// here: a consumer verifies it when it stores it.
pub fn read_stored_consensus_v2_qc(
    data_dir: &Path,
    domain: &ConsensusV2Domain,
    reference: &ConsensusV2QcRef,
) -> io::Result<Option<ConsensusV2QuorumCertificate>> {
    for path in [
        consensus_v2_qc_path(data_dir, domain, &reference.certificate_id),
        legacy_consensus_v2_qc_path(data_dir, &reference.certificate_id),
    ] {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let certificate: ConsensusV2QuorumCertificate =
            serde_json::from_slice(&bytes).map_err(|error| {
                invalid_data(format!(
                    "consensus v2 QC `{}` parse failed: {error}",
                    path.display()
                ))
            })?;
        if consensus_v2_qc_ref(&certificate).ok().as_ref() == Some(reference) {
            return Ok(Some(certificate));
        }
    }
    Ok(None)
}

/// Each distinct QC the votes of `certificate` name, in vote order, resolved
/// in `graph`. The timeout certificate file carries them.
pub fn consensus_v2_timeout_certificate_high_qcs(
    graph: &ConsensusV2QcGraph,
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    certificate: &ConsensusV2TimeoutCertificate,
) -> Result<Vec<ConsensusV2QuorumCertificate>, postfiat_ordering_fast::OrderingError> {
    let mut qcs = Vec::new();
    for reference in certificate.votes.iter().filter_map(|vote| vote.high_qc.as_ref()) {
        let qc = graph.resolve_verified(domain, validators, reference)?;
        if !qcs.contains(qc) {
            qcs.push(qc.clone());
        }
    }
    Ok(qcs)
}

/// Adds to `graph` the full QCs a timeout vote or certificate at `round`
/// carries (unsigned hints). A carried QC is used only if it is a prepare QC
/// of the same height from no later view, one of the `named` (signed)
/// references, and verifies. Any other is ignored, logged once under
/// `context`, and the graph is used as it is. Returns the QCs added, which
/// the caller persists only after the vote or certificate verifies.
pub fn add_carried_consensus_v2_qcs(
    graph: &mut ConsensusV2QcGraph,
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    round: ConsensusV2Round,
    named: &[&ConsensusV2QcRef],
    carried: &[ConsensusV2QuorumCertificate],
    context: &str,
) -> Vec<ConsensusV2QuorumCertificate> {
    let mut added = Vec::new();
    let mut ignored = 0usize;
    for certificate in carried {
        let Some(reference) = consensus_v2_qc_ref(certificate).ok().filter(|reference| {
            named.contains(&reference)
                && reference.phase == ConsensusV2Phase::Prepare
                && reference.round.height == round.height
                && reference.round.view <= round.view
        }) else {
            ignored += 1;
            continue;
        };
        if graph.resolve_verified(domain, validators, &reference).is_ok() {
            continue;
        }
        match graph.insert_verified(domain, validators, certificate.clone()) {
            Ok(_) => added.push(certificate.clone()),
            Err(_) => ignored += 1,
        }
    }
    if ignored > 0 {
        eprintln!(
            "WARN {context}: ignored {ignored} carried consensus v2 QC(s) that it does not name or that do not verify"
        );
    }
    added
}

/// Persists the QCs `add_carried_consensus_v2_qcs` added, once the vote or
/// certificate that carried them verified. Holding a verified QC changes no
/// safety record (lock and high QC move only on authorization).
pub fn persist_carried_consensus_v2_qcs(
    data_dir: &Path,
    added: &[ConsensusV2QuorumCertificate],
) -> io::Result<()> {
    for certificate in added {
        persist_consensus_v2_qc(data_dir, certificate)?;
    }
    Ok(())
}

/// View-zero proposals cannot carry timeout evidence or a valid-round QC, so
/// their verification has no historical QC dependency. Nonzero views retain
/// complete graph reconstruction and signature verification.
pub fn read_consensus_v2_qc_graph_for_view(
    data_dir: &Path,
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    view: u64,
) -> io::Result<ConsensusV2QcGraph> {
    if view == 0 {
        Ok(ConsensusV2QcGraph::default())
    } else {
        read_consensus_v2_qc_graph(data_dir, domain, validators)
    }
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
    let high_qc = high_qc.cloned();
    persist_consensus_v2_timeout_authorization_selected_with(
        data_dir,
        round,
        move |_| high_qc,
        |_, _, _, _, next| Ok(next.clone()),
    )
}

/// The validator's timeout vote at `round`: the one it already signed there,
/// or a newly authorized and signed one. Both happen under the per-height
/// safety guard. A validator signs at most one timeout vote per (height,
/// view); asking again returns that same signed vote, so a recovery that lost
/// its votes can collect them again, and nothing new is ever signed for a
/// round it already timed out.
pub(crate) fn consensus_v2_timeout_vote_once(
    data_dir: &Path,
    round: ConsensusV2Round,
    validator_id: &str,
    sign: impl FnOnce(
        &ConsensusV2Domain,
        &ConsensusV2ValidatorSet,
        &ConsensusV2QcGraph,
        Option<&ConsensusV2QcRef>,
        &ConsensusV2SafetyState,
    ) -> io::Result<ConsensusV2TimeoutVote>,
) -> io::Result<ConsensusV2TimeoutVote> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    with_safety_guard(data_dir, round.height, || {
        let vote_path = consensus_v2_timeout_vote_path(data_dir, &domain, round);
        match std::fs::read(&vote_path) {
            Ok(bytes) => {
                let stored: ConsensusV2TimeoutVote =
                    serde_json::from_slice(&bytes).map_err(|error| {
                        invalid_data(format!(
                            "consensus v2 stored timeout vote `{}` parse failed: {error}",
                            vote_path.display()
                        ))
                    })?;
                let current = read_consensus_v2_safety_state(data_dir, &domain, round.height)?;
                if stored.domain != domain
                    || stored.round != round
                    || stored.validator != validator_id
                    || current
                        .highest_timeout_round
                        .is_none_or(|highest| highest < round)
                {
                    return Err(invalid_data(format!(
                        "consensus v2 stored timeout vote `{}` does not match this validator's safety record",
                        vote_path.display()
                    )));
                }
                let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
                verify_consensus_v2_timeout_vote(&domain, &validators, &stored, &graph)
                    .map_err(|error| invalid_data(format!("consensus v2 stored timeout vote: {error}")))?;
                return Ok(stored);
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
        let current = read_consensus_v2_safety_state(data_dir, &domain, round.height)?;
        let high_qc = current.high_qc.clone();
        let next = authorize_consensus_v2_timeout_vote(
            &current,
            &domain,
            &validators,
            round,
            high_qc.as_ref(),
            &graph,
        )
        .map_err(|error| invalid_data(format!("consensus v2 timeout authorization: {error}")))?;
        write_consensus_v2_safety_state(data_dir, &next)?;
        let vote = sign(&domain, &validators, &graph, high_qc.as_ref(), &next)?;
        let mut bytes = serde_json::to_vec_pretty(&vote).map_err(invalid_data)?;
        bytes.push(b'\n');
        atomic_write(vote_path, bytes)?;
        Ok(vote)
    })
}

fn consensus_v2_timeout_vote_path(
    data_dir: &Path,
    domain: &ConsensusV2Domain,
    round: ConsensusV2Round,
) -> PathBuf {
    data_dir.join(CONSENSUS_V2_SAFETY_DIR).join(format!(
        "{}-height-{}-view-{}.timeout-vote.json",
        consensus_v2_artifact_prefix(domain),
        round.height,
        round.view
    ))
}

fn persist_consensus_v2_timeout_authorization_selected_with<T>(
    data_dir: &Path,
    round: ConsensusV2Round,
    select_high_qc: impl FnOnce(&ConsensusV2SafetyState) -> Option<ConsensusV2QcRef>,
    after_persist: impl FnOnce(
        &ConsensusV2Domain,
        &ConsensusV2ValidatorSet,
        &ConsensusV2QcGraph,
        Option<&ConsensusV2QcRef>,
        &ConsensusV2SafetyState,
    ) -> io::Result<T>,
) -> io::Result<T> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    with_safety_guard(data_dir, round.height, || {
        let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
        let current = read_consensus_v2_safety_state(data_dir, &domain, round.height)?;
        let high_qc = select_high_qc(&current);
        let next = authorize_consensus_v2_timeout_vote(
            &current,
            &domain,
            &validators,
            round,
            high_qc.as_ref(),
            &graph,
        )
        .map_err(|error| invalid_data(format!("consensus v2 timeout authorization: {error}")))?;
        write_consensus_v2_safety_state(data_dir, &next)?;
        after_persist(&domain, &validators, &graph, high_qc.as_ref(), &next)
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
        certify_consensus_v2_timeouts, certify_consensus_v2_votes, consensus_v2_block_ref,
        consensus_v2_proposal_signing_bytes, consensus_v2_timeout_vote_signing_bytes,
        consensus_v2_vote_signing_bytes, leader_for_view, CONSENSUS_V2_PROPOSAL_CONTEXT,
        CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT, CONSENSUS_V2_VOTE_CONTEXT,
    };
    use postfiat_types::{
        ConsensusV2Phase, ConsensusV2Proposal, ConsensusV2Round, ConsensusV2Signature,
        ConsensusV2Vote, CONSENSUS_V2_PROPOSAL_SCHEMA, CONSENSUS_V2_TIMEOUT_VOTE_SCHEMA,
        CONSENSUS_V2_VOTE_SCHEMA,
    };
    use std::sync::mpsc;
    use std::thread;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
            storage_activation_height: Some(1),
        })
        .expect("init consensus v2 activation chain");
        let genesis = NodeStore::new(&data_dir)
            .read_genesis()
            .expect("read v2 genesis");
        assert_eq!(genesis.consensus_v2_activation_height, Some(7));
        assert_eq!(genesis.ordered_history_v2_activation_height, Some(1));
        let topology = write_consensus_v2_topology(TopologyConsensusV2Options {
            chain_id: genesis.chain_id.clone(),
            validators: 4,
            base_port: postfiat_network::DEFAULT_BASE_PORT,
            rpc_base_port: None,
            hosts: None,
            output_file: topology_file.clone(),
            activation_height: 7,
            storage_activation_height: Some(1),
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

        let (callback_started_tx, callback_started_rx) = mpsc::channel();
        let (contender_attempting_tx, contender_attempting_rx) = mpsc::channel();
        let (contender_entered_tx, contender_entered_rx) = mpsc::channel();
        let contender_data_dir = data_dir.clone();
        let contender = thread::spawn(move || {
            callback_started_rx
                .recv_timeout(Duration::from_secs(1))
                .expect("signing callback starts");
            contender_attempting_tx
                .send(())
                .expect("announce competing authorization");
            with_safety_guard(&contender_data_dir, 1, || {
                contender_entered_tx
                    .send(())
                    .expect("announce acquired safety guard");
                Ok(())
            })
        });
        let persisted = persist_consensus_v2_prepare_authorization_with(
            &data_dir,
            &proposal,
            None,
            &ConsensusV2QcGraph::default(),
            |_, _, next| {
                callback_started_tx
                    .send(())
                    .expect("announce signing callback");
                contender_attempting_rx
                    .recv_timeout(Duration::from_secs(1))
                    .expect("competing authorization attempts safety guard");
                assert_eq!(
                    contender_entered_rx.recv_timeout(Duration::from_millis(100)),
                    Err(mpsc::RecvTimeoutError::Timeout),
                    "another authorization entered while signature construction held the guard"
                );
                Ok(next.clone())
            },
        )
        .expect("persist prepare authorization");
        contender
            .join()
            .expect("competing authorization thread")
            .expect("competing authorization acquires released guard");
        contender_entered_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("competing authorization enters after signature construction");
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
        let mut tampered_qc = prepare_qc.clone();
        tampered_qc.votes[0]
            .signature
            .signature_hex
            .replace_range(0..2, "00");
        if tampered_qc.votes[0].signature.signature_hex
            == prepare_qc.votes[0].signature.signature_hex
        {
            tampered_qc.votes[0]
                .signature
                .signature_hex
                .replace_range(0..2, "ff");
        }
        let tampered_error = persist_consensus_v2_qc(&data_dir, &tampered_qc)
            .expect_err("direct QC persistence must reject a tampered signature");
        assert!(tampered_error.to_string().contains("consensus v2 QC"));
        let persisted_qc =
            persist_consensus_v2_qc(&data_dir, &prepare_qc).expect("persist verified prepare QC");
        let view_zero_graph =
            read_consensus_v2_qc_graph_for_view(&data_dir, &domain, &validators, 0)
                .expect("view-zero dependency-free graph");
        assert!(view_zero_graph
            .resolve_verified(&domain, &validators, &persisted_qc)
            .is_err());
        let nonzero_graph = read_consensus_v2_qc_graph_for_view(&data_dir, &domain, &validators, 1)
            .expect("nonzero view verified graph");
        nonzero_graph
            .resolve_verified(&domain, &validators, &persisted_qc)
            .expect("nonzero view resolves persisted dependency");
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

    fn carried_qc_test_chain() -> (
        PathBuf,
        ConsensusV2Domain,
        ConsensusV2ValidatorSet,
        ValidatorKeyFile,
    ) {
        let data_dir = unique_data_dir();
        init(InitOptions {
            data_dir: data_dir.clone(),
            chain_id: "postfiat-consensus-v2-store-test".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 4,
        })
        .expect("init consensus v2 store");
        let (domain, validators) = live_consensus_v2_context(&data_dir).expect("live context");
        let keys = serde_json::from_slice(
            &std::fs::read(data_dir.join(VALIDATOR_KEYS_FILE)).expect("read validator keys"),
        )
        .expect("parse validator keys");
        (data_dir, domain, validators, keys)
    }

    fn test_signature(
        keys: &ValidatorKeyFile,
        validator: &ConsensusV2Validator,
        message: &[u8],
        context: &[u8],
    ) -> ConsensusV2Signature {
        let record = keys
            .validators
            .iter()
            .find(|record| record.node_id == validator.validator_id)
            .expect("validator key");
        ConsensusV2Signature {
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            signer: validator.validator_id.clone(),
            public_key_hex: validator.public_key_hex.clone(),
            signature_hex: bytes_to_hex(
                &ml_dsa_65_sign_with_context(
                    &hex_to_bytes(&record.private_key_hex).expect("private key"),
                    message,
                    context,
                )
                .expect("signature"),
            ),
        }
    }

    /// A QC on a test block at `height`, signed by a quorum.
    fn test_qc(
        domain: &ConsensusV2Domain,
        validators: &ConsensusV2ValidatorSet,
        keys: &ValidatorKeyFile,
        round: ConsensusV2Round,
        phase: ConsensusV2Phase,
    ) -> ConsensusV2QuorumCertificate {
        let block = consensus_v2_block_ref(
            domain,
            round.height,
            "11".repeat(48),
            "22".repeat(48),
            "33".repeat(48),
        )
        .expect("block reference");
        let votes = validators
            .validators
            .iter()
            .take(validators.quorum)
            .map(|validator| {
                let mut vote = ConsensusV2Vote {
                    schema: CONSENSUS_V2_VOTE_SCHEMA.to_string(),
                    domain: domain.clone(),
                    round,
                    phase,
                    block: Some(block.clone()),
                    validator: validator.validator_id.clone(),
                    signature: test_signature(keys, validator, b"", CONSENSUS_V2_VOTE_CONTEXT),
                };
                let message = consensus_v2_vote_signing_bytes(&vote).expect("vote signing bytes");
                vote.signature = test_signature(keys, validator, &message, CONSENSUS_V2_VOTE_CONTEXT);
                vote
            })
            .collect();
        certify_consensus_v2_votes(domain, validators, round, phase, Some(block), votes)
            .expect("QC")
    }

    #[test]
    fn add_carried_consensus_v2_qcs_ignores_an_unnamed_precommit_other_height_or_later_view_qc() {
        let (data_dir, domain, validators, keys) = carried_qc_test_chain();
        let qc = |height, view, phase| {
            test_qc(&domain, &validators, &keys, ConsensusV2Round { height, view }, phase)
        };
        let round = ConsensusV2Round { height: 1, view: 2 };
        let named = qc(1, 1, ConsensusV2Phase::Prepare);
        let ignored = [
            (qc(1, 0, ConsensusV2Phase::Prepare), "a valid QC that is not named"),
            (qc(1, 1, ConsensusV2Phase::Precommit), "a precommit QC"),
            (qc(2, 1, ConsensusV2Phase::Prepare), "a QC of another height"),
            (qc(1, 3, ConsensusV2Phase::Prepare), "a QC of a later view"),
        ];
        // Every QC but the unnamed one is named, so only its own defect rejects it.
        let references = std::iter::once(&named)
            .chain(ignored.iter().skip(1).map(|(qc, _)| qc))
            .map(|qc| consensus_v2_qc_ref(qc).expect("QC reference"))
            .collect::<Vec<_>>();
        let references = references.iter().collect::<Vec<_>>();
        for (carried, case) in &ignored {
            let mut graph = ConsensusV2QcGraph::default();
            let added = add_carried_consensus_v2_qcs(
                &mut graph,
                &domain,
                &validators,
                round,
                &references,
                std::slice::from_ref(carried),
                "test",
            );
            assert!(added.is_empty(), "{case}");
            let reference = consensus_v2_qc_ref(carried).expect("QC reference");
            assert!(graph.resolve_verified(&domain, &validators, &reference).is_err(), "{case}");
        }
        let mut graph = ConsensusV2QcGraph::default();
        let added = add_carried_consensus_v2_qcs(
            &mut graph,
            &domain,
            &validators,
            round,
            &references,
            std::slice::from_ref(&named),
            "test",
        );
        assert_eq!(added, vec![named]);
        assert!(graph.resolve_verified(&domain, &validators, references[0]).is_ok());
        std::fs::remove_dir_all(data_dir).expect("cleanup carried QC chain");
    }

    #[test]
    fn a_timeout_certificate_carries_each_distinct_qc_its_votes_name() {
        let (data_dir, domain, validators, keys) = carried_qc_test_chain();
        let first = test_qc(
            &domain,
            &validators,
            &keys,
            ConsensusV2Round { height: 1, view: 0 },
            ConsensusV2Phase::Prepare,
        );
        let second = test_qc(
            &domain,
            &validators,
            &keys,
            ConsensusV2Round { height: 1, view: 1 },
            ConsensusV2Phase::Prepare,
        );
        let mut graph = ConsensusV2QcGraph::default();
        graph
            .insert_verified(&domain, &validators, first.clone())
            .expect("first QC");
        graph
            .insert_verified(&domain, &validators, second.clone())
            .expect("second QC");
        let round = ConsensusV2Round { height: 1, view: 2 };
        let votes = validators
            .validators
            .iter()
            .zip([&first, &second, &first, &second])
            .map(|(validator, qc)| {
                let mut vote = ConsensusV2TimeoutVote {
                    schema: CONSENSUS_V2_TIMEOUT_VOTE_SCHEMA.to_string(),
                    domain: domain.clone(),
                    round,
                    phase: ConsensusV2Phase::Precommit,
                    high_qc: Some(consensus_v2_qc_ref(qc).expect("QC reference")),
                    validator: validator.validator_id.clone(),
                    signature: test_signature(&keys, validator, b"", CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT),
                };
                let message =
                    consensus_v2_timeout_vote_signing_bytes(&vote).expect("timeout signing bytes");
                vote.signature =
                    test_signature(&keys, validator, &message, CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT);
                vote
            })
            .collect();
        let certificate = certify_consensus_v2_timeouts(
            &domain,
            &validators,
            round,
            ConsensusV2Phase::Precommit,
            votes,
            &graph,
        )
        .expect("timeout certificate");
        assert_eq!(
            consensus_v2_timeout_certificate_high_qcs(&graph, &domain, &validators, &certificate)
                .expect("carried QCs"),
            vec![first, second]
        );
        std::fs::remove_dir_all(data_dir).expect("cleanup carried QC chain");
    }
}
