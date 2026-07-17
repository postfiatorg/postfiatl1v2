use std::io;
use std::path::Path;

use postfiat_crypto_provider::{
    bytes_to_hex, hex_to_bytes, ml_dsa_65_sign_with_context, ML_DSA_65_ALGORITHM,
};
use postfiat_ordering_fast::{
    certify_consensus_v2_votes, consensus_v2_block_ref,
    consensus_v2_block_ref_with_bridge_exit_root, consensus_v2_commit_from_precommit_qc,
    consensus_v2_genesis_parent_id, consensus_v2_proposal_signing_bytes,
    consensus_v2_timeout_vote_signing_bytes, consensus_v2_vote_signing_bytes,
    verify_consensus_v2_commit, verify_consensus_v2_proposal, verify_consensus_v2_timeout_vote,
    verify_consensus_v2_vote, ConsensusV2ValidatorSet, CONSENSUS_V2_PROPOSAL_CONTEXT,
    CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT, CONSENSUS_V2_VOTE_CONTEXT,
};
use postfiat_types::{
    ConsensusV2Commit, ConsensusV2Phase, ConsensusV2Proposal, ConsensusV2QuorumCertificate,
    ConsensusV2Round, ConsensusV2Signature, ConsensusV2TimeoutCertificate, ConsensusV2TimeoutVote,
    ConsensusV2Vote, Genesis, CONSENSUS_V2_COMMIT_SCHEMA, CONSENSUS_V2_PROPOSAL_SCHEMA,
    CONSENSUS_V2_TIMEOUT_VOTE_SCHEMA, CONSENSUS_V2_VOTE_SCHEMA,
};
use zeroize::Zeroizing;

use crate::{
    live_consensus_v2_context, persist_consensus_v2_precommit_authorization,
    persist_consensus_v2_prepare_authorization, persist_consensus_v2_qc,
    persist_consensus_v2_timeout_authorization, read_consensus_v2_qc_graph,
    read_consensus_v2_safety_state, read_validator_key_file, select_validator_key_record,
    validate_validator_key_file, write_block_certificate_file, BlockCertificateFile,
    BlockProposalFile,
};

pub fn consensus_v2_active_at(genesis: &Genesis, height: u64) -> bool {
    genesis
        .consensus_v2_activation_height
        .is_some_and(|activation| height >= activation)
}

pub fn create_consensus_v2_proposal_for_block(
    data_dir: &Path,
    block_proposal: &BlockProposalFile,
    timeout_certificate: Option<&ConsensusV2TimeoutCertificate>,
    key_file: &Path,
) -> io::Result<ConsensusV2Proposal> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
    let valid_qc = timeout_certificate.and_then(|certificate| certificate.high_qc.clone());
    let timeout_certificate_id =
        timeout_certificate.map(|certificate| certificate.certificate_id.clone());
    let parent_block_id = consensus_v2_parent_block_id(&domain, block_proposal)?;
    let block = match block_proposal.bridge_exit_root.clone() {
        Some(bridge_exit_root) => consensus_v2_block_ref_with_bridge_exit_root(
            &domain,
            block_proposal.block_height,
            parent_block_id,
            block_proposal.payload_hash.clone(),
            block_proposal.state_root.clone(),
            bridge_exit_root,
        ),
        None => consensus_v2_block_ref(
            &domain,
            block_proposal.block_height,
            parent_block_id,
            block_proposal.payload_hash.clone(),
            block_proposal.state_root.clone(),
        ),
    }
    .map_err(ordering_error)?;
    let keys = read_validator_key_file(key_file)?;
    validate_validator_key_file(&keys)?;
    let key = select_validator_key_record(&keys, Some(&block_proposal.proposer))?;
    let mut proposal = ConsensusV2Proposal {
        schema: CONSENSUS_V2_PROPOSAL_SCHEMA.to_string(),
        domain: domain.clone(),
        round: ConsensusV2Round {
            height: block_proposal.block_height,
            view: block_proposal.view,
        },
        block,
        valid_qc,
        timeout_certificate_id,
        proposer: key.node_id.clone(),
        signature: signature_shell(&key.node_id, &key.public_key_hex),
    };
    let message = consensus_v2_proposal_signing_bytes(&proposal).map_err(ordering_error)?;
    proposal.signature.signature_hex = sign_message(
        &key.private_key_hex,
        &message,
        CONSENSUS_V2_PROPOSAL_CONTEXT,
    )?;
    verify_consensus_v2_proposal(&domain, &validators, &proposal, timeout_certificate, &graph)
        .map_err(ordering_error)?;
    verify_consensus_v2_proposal_matches_block(block_proposal, &proposal)?;
    Ok(proposal)
}

pub fn create_consensus_v2_prepare_vote(
    data_dir: &Path,
    proposal: &ConsensusV2Proposal,
    timeout_certificate: Option<&ConsensusV2TimeoutCertificate>,
    key_file: &Path,
    validator_id: &str,
) -> io::Result<ConsensusV2Vote> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
    persist_consensus_v2_prepare_authorization(data_dir, proposal, timeout_certificate, &graph)?;
    sign_consensus_v2_vote(
        key_file,
        validator_id,
        domain,
        &validators,
        proposal.round,
        ConsensusV2Phase::Prepare,
        Some(proposal.block.clone()),
    )
}

pub fn create_consensus_v2_precommit_vote(
    data_dir: &Path,
    prepare_qc: &ConsensusV2QuorumCertificate,
    key_file: &Path,
    validator_id: &str,
) -> io::Result<ConsensusV2Vote> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    persist_consensus_v2_qc(data_dir, prepare_qc)?;
    persist_consensus_v2_precommit_authorization(data_dir, prepare_qc)?;
    sign_consensus_v2_vote(
        key_file,
        validator_id,
        domain,
        &validators,
        prepare_qc.round,
        ConsensusV2Phase::Precommit,
        prepare_qc.block.clone(),
    )
}

/// Persist the timeout high-water mark before emitting the timeout signature.
pub fn create_consensus_v2_timeout_vote(
    data_dir: &Path,
    round: ConsensusV2Round,
    key_file: &Path,
    validator_id: &str,
) -> io::Result<ConsensusV2TimeoutVote> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
    let state = read_consensus_v2_safety_state(data_dir, &domain, round.height)?;
    let high_qc = state.high_qc.clone();
    persist_consensus_v2_timeout_authorization(data_dir, round, high_qc.as_ref())?;
    let keys = read_validator_key_file(key_file)?;
    validate_validator_key_file(&keys)?;
    let key = select_validator_key_record(&keys, Some(validator_id))?;
    let mut vote = ConsensusV2TimeoutVote {
        schema: CONSENSUS_V2_TIMEOUT_VOTE_SCHEMA.to_string(),
        domain: domain.clone(),
        round,
        phase: ConsensusV2Phase::Precommit,
        high_qc,
        validator: validator_id.to_string(),
        signature: signature_shell(validator_id, &key.public_key_hex),
    };
    let message = consensus_v2_timeout_vote_signing_bytes(&vote).map_err(ordering_error)?;
    vote.signature.signature_hex = sign_message(
        &key.private_key_hex,
        &message,
        CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT,
    )?;
    verify_consensus_v2_timeout_vote(&domain, &validators, &vote, &graph)
        .map_err(ordering_error)?;
    Ok(vote)
}

pub fn certify_and_persist_consensus_v2_votes(
    data_dir: &Path,
    round: ConsensusV2Round,
    phase: ConsensusV2Phase,
    block: Option<postfiat_types::ConsensusV2BlockRef>,
    votes: Vec<ConsensusV2Vote>,
) -> io::Result<ConsensusV2QuorumCertificate> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    let certificate = certify_consensus_v2_votes(&domain, &validators, round, phase, block, votes)
        .map_err(ordering_error)?;
    persist_consensus_v2_qc(data_dir, &certificate)?;
    Ok(certificate)
}

pub fn assemble_consensus_v2_commit(
    data_dir: &Path,
    block_proposal: &BlockProposalFile,
    proposal: ConsensusV2Proposal,
    timeout_certificate: Option<ConsensusV2TimeoutCertificate>,
    prepare_qc: ConsensusV2QuorumCertificate,
    precommit_qc: ConsensusV2QuorumCertificate,
) -> io::Result<ConsensusV2Commit> {
    verify_consensus_v2_proposal_matches_block(block_proposal, &proposal)?;
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
    let mut prior_references = Vec::new();
    if let Some(reference) = proposal.valid_qc.as_ref() {
        prior_references.push(reference);
    }
    if let Some(certificate) = timeout_certificate.as_ref() {
        for vote in &certificate.votes {
            if let Some(reference) = vote.high_qc.as_ref() {
                prior_references.push(reference);
            }
        }
    }
    prior_references.sort_by(|left, right| left.certificate_id.cmp(&right.certificate_id));
    prior_references.dedup_by(|left, right| left.certificate_id == right.certificate_id);
    let prior_qcs = prior_references
        .into_iter()
        .map(|reference| {
            graph
                .resolve_verified(&domain, &validators, reference)
                .cloned()
                .map_err(ordering_error)
        })
        .collect::<io::Result<Vec<_>>>()?;
    let commit = ConsensusV2Commit {
        schema: CONSENSUS_V2_COMMIT_SCHEMA.to_string(),
        proposal,
        prior_qcs,
        timeout_certificate,
        prepare_qc,
        precommit_qc,
    };
    verify_consensus_v2_commit_for_block(data_dir, block_proposal, &commit)?;
    Ok(commit)
}

pub fn verify_consensus_v2_commit_for_block(
    data_dir: &Path,
    block_proposal: &BlockProposalFile,
    commit: &ConsensusV2Commit,
) -> io::Result<()> {
    let (domain, validators) = live_consensus_v2_context(data_dir)?;
    let graph = read_consensus_v2_qc_graph(data_dir, &domain, &validators)?;
    let committed =
        verify_consensus_v2_commit(&domain, &validators, commit, &graph).map_err(ordering_error)?;
    verify_consensus_v2_proposal_matches_block(block_proposal, &commit.proposal)?;
    if committed != commit.proposal.block {
        return Err(invalid_data(
            "consensus v2 commit block does not match signed proposal",
        ));
    }
    consensus_v2_commit_from_precommit_qc(&domain, &validators, &commit.precommit_qc)
        .map_err(ordering_error)?;
    Ok(())
}

pub fn verify_consensus_v2_finality_requirement(
    data_dir: &Path,
    genesis: &Genesis,
    block_proposal: &BlockProposalFile,
    certificate_file: Option<&BlockCertificateFile>,
) -> io::Result<()> {
    let active = consensus_v2_active_at(genesis, block_proposal.block_height);
    match (active, certificate_file) {
        (false, Some(certificate)) if certificate.consensus_v2_commit.is_some() => Err(
            invalid_data("consensus v2 commit supplied before activation height"),
        ),
        (false, _) => Ok(()),
        (true, None) => Err(invalid_data(
            "consensus v2 activation requires an external finality certificate",
        )),
        (true, Some(certificate)) => {
            let commit = certificate.consensus_v2_commit.as_ref().ok_or_else(|| {
                invalid_data("consensus v2 activation requires a precommit QC artifact")
            })?;
            verify_consensus_v2_commit_for_block(data_dir, block_proposal, commit)
        }
    }
}

pub fn write_consensus_v2_block_certificate_file(
    path: &Path,
    certificate: &BlockCertificateFile,
) -> io::Result<()> {
    if certificate.consensus_v2_commit.is_none() {
        return Err(invalid_data(
            "consensus v2 certificate write requires attached commit",
        ));
    }
    write_block_certificate_file(path, certificate)
}

fn sign_consensus_v2_vote(
    key_file: &Path,
    validator_id: &str,
    domain: postfiat_types::ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    round: ConsensusV2Round,
    phase: ConsensusV2Phase,
    block: Option<postfiat_types::ConsensusV2BlockRef>,
) -> io::Result<ConsensusV2Vote> {
    let keys = read_validator_key_file(key_file)?;
    validate_validator_key_file(&keys)?;
    let key = select_validator_key_record(&keys, Some(validator_id))?;
    let mut vote = ConsensusV2Vote {
        schema: CONSENSUS_V2_VOTE_SCHEMA.to_string(),
        domain,
        round,
        phase,
        block,
        validator: key.node_id.clone(),
        signature: signature_shell(&key.node_id, &key.public_key_hex),
    };
    let message = consensus_v2_vote_signing_bytes(&vote).map_err(ordering_error)?;
    vote.signature.signature_hex =
        sign_message(&key.private_key_hex, &message, CONSENSUS_V2_VOTE_CONTEXT)?;
    verify_consensus_v2_vote(&vote.domain, validators, &vote).map_err(ordering_error)?;
    Ok(vote)
}

pub fn verify_consensus_v2_proposal_matches_block(
    block: &BlockProposalFile,
    proposal: &ConsensusV2Proposal,
) -> io::Result<()> {
    let expected_parent = consensus_v2_parent_block_id(&proposal.domain, block)?;
    if proposal.round.height != block.block_height
        || proposal.round.view != block.view
        || proposal.proposer != block.proposer
        || proposal.block.height != block.block_height
        || proposal.block.parent_block_id != expected_parent
        || proposal.block.payload_hash != block.payload_hash
        || proposal.block.state_root != block.state_root
        || proposal.block.bridge_exit_root != block.bridge_exit_root
    {
        return Err(invalid_data(
            "consensus v2 proposal does not match ordered block proposal",
        ));
    }
    Ok(())
}

fn consensus_v2_parent_block_id(
    domain: &postfiat_types::ConsensusV2Domain,
    block: &BlockProposalFile,
) -> io::Result<String> {
    if block.block_height == 1 && block.parent_hash == "genesis" {
        return consensus_v2_genesis_parent_id(domain).map_err(ordering_error);
    }
    Ok(block.parent_hash.clone())
}

fn signature_shell(signer: &str, public_key_hex: &str) -> ConsensusV2Signature {
    ConsensusV2Signature {
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        signer: signer.to_string(),
        public_key_hex: public_key_hex.to_string(),
        signature_hex: "00".to_string(),
    }
}

fn sign_message(private_key_hex: &str, message: &[u8], context: &[u8]) -> io::Result<String> {
    let private_key = Zeroizing::new(hex_to_bytes(private_key_hex).map_err(invalid_data)?);
    let signature =
        ml_dsa_65_sign_with_context(&private_key, message, context).map_err(invalid_data)?;
    Ok(bytes_to_hex(&signature))
}

fn ordering_error(error: impl std::fmt::Display) -> io::Error {
    invalid_data(error)
}

fn invalid_data(error: impl std::fmt::Display) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use postfiat_ordering_fast::{certify_consensus_v2_timeouts, leader_for_view};

    use crate::{
        export_snapshot, import_snapshot, init_consensus_v2, InitConsensusV2Options, NodeStore,
        SnapshotExportOptions, SnapshotImportOptions, BLOCK_PROPOSAL_FILE_SCHEMA,
        VALIDATOR_KEYS_FILE, VALIDATOR_REGISTRY_FILE,
    };

    fn unique_root() -> PathBuf {
        std::env::temp_dir().join(format!(
            "postfiat-consensus-v2-finality-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ))
    }

    #[test]
    fn four_nodes_require_prepare_and_precommit_qcs_for_exact_block() {
        let root = unique_root();
        let mut data_dirs = Vec::new();
        let mut statuses = Vec::new();
        for index in 0..4 {
            let data_dir = root.join(format!("validator-{index}"));
            let status = init_consensus_v2(InitConsensusV2Options {
                data_dir: data_dir.clone(),
                chain_id: "postfiat-consensus-v2-finality-test".to_string(),
                node_id: format!("validator-{index}"),
                validator_count: 4,
                activation_height: 1,
            })
            .expect("init validator");
            data_dirs.push(data_dir);
            statuses.push(status);
        }
        assert!(statuses
            .windows(2)
            .all(|pair| pair[0].genesis_hash == pair[1].genesis_hash));
        let shared_keys = std::fs::read(data_dirs[0].join(VALIDATOR_KEYS_FILE))
            .expect("read shared validator keys");
        let shared_registry = std::fs::read(data_dirs[0].join(VALIDATOR_REGISTRY_FILE))
            .expect("read shared validator registry");
        for data_dir in data_dirs.iter().skip(1) {
            std::fs::write(data_dir.join(VALIDATOR_KEYS_FILE), &shared_keys)
                .expect("stage shared validator keys");
            std::fs::write(data_dir.join(VALIDATOR_REGISTRY_FILE), &shared_registry)
                .expect("stage shared validator registry");
        }
        let (domain, validators) = live_consensus_v2_context(&data_dirs[0]).expect("live context");
        let proposer = leader_for_view(&validators.validator_ids(), 1, 0).expect("leader");
        let proposer_index = validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &proposer)
            .expect("proposer index");
        let block = BlockProposalFile {
            schema: BLOCK_PROPOSAL_FILE_SCHEMA.to_string(),
            chain_id: domain.chain_id.clone(),
            genesis_hash: domain.genesis_hash.clone(),
            protocol_version: domain.protocol_version,
            block_height: 1,
            view: 0,
            parent_hash: statuses[0].block_tip_hash.clone(),
            proposer,
            batch_kind: "transparent".to_string(),
            batch_id: "11".repeat(48),
            payload_hash: "22".repeat(48),
            state_root: "33".repeat(48),
            bridge_exit_root: None,
            receipt_count: 0,
            receipt_ids: Vec::new(),
            fastpay_pre_state_effects: Vec::new(),
            signature: None,
        };
        let proposal = create_consensus_v2_proposal_for_block(
            &data_dirs[proposer_index],
            &block,
            None,
            &data_dirs[proposer_index].join(VALIDATOR_KEYS_FILE),
        )
        .expect("create v2 proposal");
        let prepare_votes = data_dirs
            .iter()
            .enumerate()
            .map(|(index, data_dir)| {
                create_consensus_v2_prepare_vote(
                    data_dir,
                    &proposal,
                    None,
                    &data_dir.join(VALIDATOR_KEYS_FILE),
                    &format!("validator-{index}"),
                )
                .expect("prepare vote")
            })
            .collect::<Vec<_>>();
        let prepare_qc = certify_and_persist_consensus_v2_votes(
            &data_dirs[proposer_index],
            proposal.round,
            ConsensusV2Phase::Prepare,
            Some(proposal.block.clone()),
            prepare_votes,
        )
        .expect("prepare QC");
        let precommit_votes = data_dirs
            .iter()
            .enumerate()
            .map(|(index, data_dir)| {
                create_consensus_v2_precommit_vote(
                    data_dir,
                    &prepare_qc,
                    &data_dir.join(VALIDATOR_KEYS_FILE),
                    &format!("validator-{index}"),
                )
                .expect("precommit vote")
            })
            .collect::<Vec<_>>();
        let precommit_qc = certify_and_persist_consensus_v2_votes(
            &data_dirs[proposer_index],
            proposal.round,
            ConsensusV2Phase::Precommit,
            Some(proposal.block.clone()),
            precommit_votes,
        )
        .expect("precommit QC");
        let commit = assemble_consensus_v2_commit(
            &data_dirs[proposer_index],
            &block,
            proposal.clone(),
            None,
            prepare_qc.clone(),
            precommit_qc.clone(),
        )
        .expect("assemble commit");
        for data_dir in &data_dirs {
            verify_consensus_v2_commit_for_block(data_dir, &block, &commit)
                .expect("every node verifies commit");
        }
        let mut wrong_block = block.clone();
        wrong_block.state_root = "44".repeat(48);
        assert!(
            verify_consensus_v2_commit_for_block(&data_dirs[0], &wrong_block, &commit).is_err()
        );

        let genesis = NodeStore::new(&data_dirs[0])
            .read_genesis()
            .expect("read genesis");
        assert!(consensus_v2_active_at(&genesis, 1));

        // A timeout after view zero is signed only after each node has
        // durably advanced its timeout high-water mark. The recovered leader
        // must re-propose the locked block and obtain a fresh two-phase commit.
        let timeout_round = ConsensusV2Round { height: 1, view: 0 };
        let timeout_votes = data_dirs
            .iter()
            .enumerate()
            .map(|(index, data_dir)| {
                create_consensus_v2_timeout_vote(
                    data_dir,
                    timeout_round,
                    &data_dir.join(VALIDATOR_KEYS_FILE),
                    &format!("validator-{index}"),
                )
                .expect("timeout vote after restart-safe lock")
            })
            .collect::<Vec<_>>();
        let graph = read_consensus_v2_qc_graph(&data_dirs[proposer_index], &domain, &validators)
            .expect("read persisted QC graph");
        let timeout_certificate = certify_consensus_v2_timeouts(
            &domain,
            &validators,
            timeout_round,
            ConsensusV2Phase::Precommit,
            timeout_votes,
            &graph,
        )
        .expect("timeout certificate");
        let recovery_view = 1;
        let recovery_proposer =
            leader_for_view(&validators.validator_ids(), 1, recovery_view).expect("leader");
        let recovery_index = validators
            .validator_ids()
            .iter()
            .position(|validator| validator == &recovery_proposer)
            .expect("recovery proposer index");
        let mut recovery_block = block.clone();
        recovery_block.view = recovery_view;
        recovery_block.proposer = recovery_proposer;
        let recovery_proposal = create_consensus_v2_proposal_for_block(
            &data_dirs[recovery_index],
            &recovery_block,
            Some(&timeout_certificate),
            &data_dirs[recovery_index].join(VALIDATOR_KEYS_FILE),
        )
        .expect("recovery proposal");
        assert_eq!(recovery_proposal.block, proposal.block);
        let recovery_prepare_votes = data_dirs
            .iter()
            .enumerate()
            .map(|(index, data_dir)| {
                create_consensus_v2_prepare_vote(
                    data_dir,
                    &recovery_proposal,
                    Some(&timeout_certificate),
                    &data_dir.join(VALIDATOR_KEYS_FILE),
                    &format!("validator-{index}"),
                )
                .expect("recovery prepare")
            })
            .collect::<Vec<_>>();
        let recovery_prepare_qc = certify_and_persist_consensus_v2_votes(
            &data_dirs[recovery_index],
            recovery_proposal.round,
            ConsensusV2Phase::Prepare,
            Some(recovery_proposal.block.clone()),
            recovery_prepare_votes,
        )
        .expect("recovery prepare QC");
        let recovery_precommit_votes = data_dirs
            .iter()
            .enumerate()
            .map(|(index, data_dir)| {
                create_consensus_v2_precommit_vote(
                    data_dir,
                    &recovery_prepare_qc,
                    &data_dir.join(VALIDATOR_KEYS_FILE),
                    &format!("validator-{index}"),
                )
                .expect("recovery precommit")
            })
            .collect::<Vec<_>>();
        let recovery_precommit_qc = certify_and_persist_consensus_v2_votes(
            &data_dirs[recovery_index],
            recovery_proposal.round,
            ConsensusV2Phase::Precommit,
            Some(recovery_proposal.block.clone()),
            recovery_precommit_votes,
        )
        .expect("recovery precommit QC");
        let recovery_commit = assemble_consensus_v2_commit(
            &data_dirs[recovery_index],
            &recovery_block,
            recovery_proposal,
            Some(timeout_certificate),
            recovery_prepare_qc,
            recovery_precommit_qc,
        )
        .expect("recovery commit");
        assert_eq!(recovery_commit.prior_qcs, vec![prepare_qc]);
        verify_consensus_v2_commit(
            &domain,
            &validators,
            &recovery_commit,
            &postfiat_ordering_fast::ConsensusV2QcGraph::default(),
        )
        .expect("self-contained recovery commit");

        let snapshot_dir = root.join("snapshot");
        let restored_dir = root.join("restored");
        export_snapshot(SnapshotExportOptions {
            data_dir: data_dirs[0].clone(),
            snapshot_dir: snapshot_dir.clone(),
        })
        .expect("export consensus v2 safety snapshot");
        import_snapshot(SnapshotImportOptions {
            data_dir: restored_dir.clone(),
            snapshot_dir,
            node_id: None,
        })
        .expect("restore consensus v2 safety snapshot");
        let (restored_domain, restored_validators) =
            live_consensus_v2_context(&restored_dir).expect("restored context");
        assert_eq!(
            read_consensus_v2_safety_state(&restored_dir, &restored_domain, 1)
                .expect("restored safety state"),
            read_consensus_v2_safety_state(&data_dirs[0], &domain, 1).expect("source safety state")
        );
        read_consensus_v2_qc_graph(&restored_dir, &restored_domain, &restored_validators)
            .expect("restored QC graph verifies");
        std::fs::remove_dir_all(root).expect("cleanup finality test");
    }
}
