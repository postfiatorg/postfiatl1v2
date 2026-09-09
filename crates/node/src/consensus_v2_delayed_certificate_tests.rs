use super::*;
use crate::{
    init_consensus_v2, InitConsensusV2Options, StatusReport, BLOCK_PROPOSAL_FILE_SCHEMA,
    VALIDATOR_KEYS_FILE, VALIDATOR_REGISTRY_FILE,
};
use postfiat_ordering_fast::{
    certify_consensus_v2_timeouts, certify_consensus_v2_votes, leader_for_view, ConsensusV2QcGraph,
};
use postfiat_types::ConsensusV2Domain;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fleet {
    root: PathBuf,
    dirs: Vec<PathBuf>,
    status: StatusReport,
    domain: ConsensusV2Domain,
    validators: ConsensusV2ValidatorSet,
}

impl Fleet {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!(
            "postfiat-delayed-certificate-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let mut dirs = Vec::new();
        let mut statuses = Vec::new();
        for index in 0..4 {
            let data_dir = root.join(format!("validator-{index}"));
            statuses.push(
                init_consensus_v2(InitConsensusV2Options {
                    data_dir: data_dir.clone(),
                    chain_id: "postfiat-delayed-certificate-test".to_string(),
                    node_id: format!("validator-{index}"),
                    validator_count: 4,
                    activation_height: 1,
                    storage_activation_height: None,
                })
                .expect("init validator"),
            );
            dirs.push(data_dir);
        }
        for name in [VALIDATOR_KEYS_FILE, VALIDATOR_REGISTRY_FILE] {
            let bytes = std::fs::read(dirs[0].join(name)).expect("shared fixture");
            for dir in &dirs[1..] {
                std::fs::write(dir.join(name), &bytes).expect("stage common committee");
            }
        }
        let (domain, validators) = live_consensus_v2_context(&dirs[0]).unwrap();
        Self {
            root,
            dirs,
            status: statuses.remove(0),
            domain,
            validators,
        }
    }

    fn proposal(
        &self,
        view: u64,
        payload: &str,
        timeout: Option<&ConsensusV2TimeoutCertificate>,
    ) -> ConsensusV2Proposal {
        let proposer = leader_for_view(&self.validators.validator_ids(), 1, view).unwrap();
        let index = self
            .validators
            .validator_ids()
            .iter()
            .position(|id| id == &proposer)
            .unwrap();
        let block = BlockProposalFile {
            schema: BLOCK_PROPOSAL_FILE_SCHEMA.to_string(),
            chain_id: self.domain.chain_id.clone(),
            genesis_hash: self.domain.genesis_hash.clone(),
            protocol_version: self.domain.protocol_version,
            block_height: 1,
            view,
            parent_hash: self.status.block_tip_hash.clone(),
            proposer,
            batch_kind: "transparent".to_string(),
            batch_id: payload.repeat(48),
            payload_hash: payload.repeat(48),
            state_root: payload.repeat(48),
            bridge_exit_root: None,
            pftl_uniswap_receipt_root: None,
            receipt_count: 0,
            receipt_ids: Vec::new(),
            fastpay_pre_state_effects: Vec::new(),
            signature: None,
        };
        create_consensus_v2_proposal_for_block(
            &self.dirs[index],
            &block,
            timeout,
            &self.dirs[index].join(VALIDATOR_KEYS_FILE),
        )
        .expect("signed proposal")
    }

    fn prepares(
        &self,
        proposal: &ConsensusV2Proposal,
        timeout: Option<&ConsensusV2TimeoutCertificate>,
    ) -> ConsensusV2QuorumCertificate {
        let votes = self
            .dirs
            .iter()
            .enumerate()
            .take(self.validators.quorum)
            .map(|(index, dir)| {
                create_consensus_v2_prepare_vote(
                    dir,
                    proposal,
                    timeout,
                    &dir.join(VALIDATOR_KEYS_FILE),
                    &format!("validator-{index}"),
                )
                .expect("persist and sign prepare")
            })
            .collect();
        // The network has assembled this certificate but has delivered it to no signer.
        certify_consensus_v2_votes(
            &self.domain,
            &self.validators,
            proposal.round,
            ConsensusV2Phase::Prepare,
            Some(proposal.block.clone()),
            votes,
        )
        .expect("verify withheld prepare certificate")
    }

    fn timeout(&self, view: u64) -> ConsensusV2TimeoutCertificate {
        let round = ConsensusV2Round { height: 1, view };
        let votes = self
            .dirs
            .iter()
            .enumerate()
            .take(self.validators.quorum)
            .map(|(index, dir)| {
                create_consensus_v2_timeout_vote(
                    dir,
                    round,
                    &dir.join(VALIDATOR_KEYS_FILE),
                    &format!("validator-{index}"),
                )
                .expect("persist and sign timeout")
            })
            .collect();
        certify_consensus_v2_timeouts(
            &self.domain,
            &self.validators,
            round,
            ConsensusV2Phase::Precommit,
            votes,
            &ConsensusV2QcGraph::default(),
        )
        .expect("verify timeout certificate")
    }
}

impl Drop for Fleet {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn delayed_precommit_is_rejected_after_preparing_a_newer_view() {
    let fleet = Fleet::new();
    let first = fleet.proposal(0, "11", None);
    let delayed_qc = fleet.prepares(&first, None);
    let timeout = fleet.timeout(0);
    assert!(timeout.high_qc.is_none());
    let second = fleet.proposal(1, "22", Some(&timeout));
    let newer_qc = fleet.prepares(&second, Some(&timeout));

    // Every entrypoint reopens the on-disk safety state. This checks the
    // production signer/store boundary, including a fresh read after view change.
    for (index, dir) in fleet.dirs.iter().enumerate().take(fleet.validators.quorum) {
        let before = read_consensus_v2_safety_state(dir, &fleet.domain, 1).unwrap();
        assert_eq!(before.highest_prepare_round, Some(second.round));
        let result = create_consensus_v2_precommit_vote(
            dir,
            &delayed_qc,
            &dir.join(VALIDATOR_KEYS_FILE),
            &format!("validator-{index}"),
        );
        assert!(
            result.is_err(),
            "the production signer returned a lower-view precommit after preparing view 1"
        );
        assert_eq!(
            read_consensus_v2_safety_state(dir, &fleet.domain, 1).unwrap(),
            before,
            "rejection must preserve durable signing state"
        );
    }
    let votes = fleet
        .dirs
        .iter()
        .enumerate()
        .take(fleet.validators.quorum)
        .map(|(index, dir)| {
            create_consensus_v2_precommit_vote(
                dir,
                &newer_qc,
                &dir.join(VALIDATOR_KEYS_FILE),
                &format!("validator-{index}"),
            )
            .expect("current view still precommits")
        })
        .collect();
    let commit_qc = certify_consensus_v2_votes(
        &fleet.domain,
        &fleet.validators,
        second.round,
        ConsensusV2Phase::Precommit,
        Some(second.block.clone()),
        votes,
    )
    .expect("recovery commit QC");
    assert_eq!(
        consensus_v2_commit_from_precommit_qc(&fleet.domain, &fleet.validators, &commit_qc)
            .unwrap(),
        second.block,
    );
}

#[test]
fn snapshot_restore_preserves_the_cross_phase_signing_floor() {
    let fleet = Fleet::new();
    let first = fleet.proposal(0, "11", None);
    let delayed_qc = fleet.prepares(&first, None);
    let timeout = fleet.timeout(0);
    let second = fleet.proposal(1, "22", Some(&timeout));
    let newer_qc = fleet.prepares(&second, Some(&timeout));
    let snapshot_dir = fleet.root.join("snapshot");
    let restored_dir = fleet.root.join("restored");
    crate::export_snapshot(crate::SnapshotExportOptions {
        data_dir: fleet.dirs[0].clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export newer-view signing state");
    crate::import_snapshot(crate::SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir,
        node_id: None,
    })
    .expect("restore newer-view signing state");
    assert_eq!(
        read_consensus_v2_safety_state(&restored_dir, &fleet.domain, 1).unwrap(),
        read_consensus_v2_safety_state(&fleet.dirs[0], &fleet.domain, 1).unwrap(),
    );
    // Snapshot excludes private keys. The fixture uses its original key file.
    let keys = fleet.dirs[0].join(VALIDATOR_KEYS_FILE);
    let rejected =
        create_consensus_v2_precommit_vote(&restored_dir, &delayed_qc, &keys, "validator-0")
            .expect_err("restored node rejects stale precommit");
    assert!(rejected.to_string().contains("durable cross-phase round"));
    let vote = create_consensus_v2_precommit_vote(&restored_dir, &newer_qc, &keys, "validator-0")
        .expect("restored node signs current-view precommit");
    verify_consensus_v2_vote(&fleet.domain, &fleet.validators, &vote).unwrap();
    assert_eq!(vote.round, second.round);
}
