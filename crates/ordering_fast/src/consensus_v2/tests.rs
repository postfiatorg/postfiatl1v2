use super::*;
use postfiat_crypto_provider::{
    bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context, MlDsa65KeyPair,
};
use postfiat_types::ConsensusV2Signature;

fn committee(count: usize) -> (ConsensusV2ValidatorSet, Vec<MlDsa65KeyPair>) {
    let keys = (0..count)
        .map(|index| ml_dsa_65_keygen_from_seed(&[index as u8 + 1; 32]))
        .collect::<Vec<_>>();
    let validators = keys
        .iter()
        .enumerate()
        .map(|(index, key)| ConsensusV2Validator {
            validator_id: format!("validator-{index}"),
            public_key_hex: bytes_to_hex(&key.public_key),
        })
        .collect::<Vec<_>>();
    (
        ConsensusV2ValidatorSet::try_new(validators).expect("validator set"),
        keys,
    )
}

fn domain(validators: &ConsensusV2ValidatorSet) -> ConsensusV2Domain {
    consensus_v2_domain(
        "postfiat-consensus-v2-test",
        "ab".repeat(48),
        2,
        7,
        validators,
    )
}

#[test]
fn consensus_v2_commit_must_bind_the_exact_bridge_exit_root() {
    let (validators, _) = committee(6);
    let domain = domain(&validators);
    let parent = "11".repeat(48);
    let payload = "22".repeat(48);
    let state = "33".repeat(48);
    let committed_exit_root = "44".repeat(48);
    let fabricated_exit_root = "55".repeat(48);

    let legacy = consensus_v2_block_ref(&domain, 9, parent.clone(), payload.clone(), state.clone())
        .expect("legacy block ref");
    assert!(
        verify_consensus_v2_bridge_exit_root(&legacy, &committed_exit_root).is_err(),
        "a legacy finality artifact must not prove any withdrawal packet or exit root"
    );

    let bound = consensus_v2_block_ref_with_bridge_exit_root(
        &domain,
        9,
        parent,
        payload,
        state,
        committed_exit_root.clone(),
    )
    .expect("exit-root-bound block ref");
    verify_consensus_v2_bridge_exit_root(&bound, &committed_exit_root)
        .expect("committed exit root must verify");
    assert!(
        verify_consensus_v2_bridge_exit_root(&bound, &fabricated_exit_root).is_err(),
        "a fabricated withdrawal root must not verify under the same finality artifact"
    );
}

fn empty_signature(validator: &ConsensusV2Validator) -> ConsensusV2Signature {
    ConsensusV2Signature {
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        signer: validator.validator_id.clone(),
        public_key_hex: validator.public_key_hex.clone(),
        signature_hex: "00".to_string(),
    }
}

fn signed_proposal(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    keys: &[MlDsa65KeyPair],
    round: ConsensusV2Round,
    block: ConsensusV2BlockRef,
    valid_qc: Option<ConsensusV2QcRef>,
    timeout_certificate_id: Option<String>,
) -> ConsensusV2Proposal {
    let proposer =
        leader_for_view(&validators.validator_ids(), round.height, round.view).expect("leader");
    let index = validators
        .validators
        .iter()
        .position(|validator| validator.validator_id == proposer)
        .expect("proposer index");
    let mut proposal = ConsensusV2Proposal {
        schema: CONSENSUS_V2_PROPOSAL_SCHEMA.to_string(),
        domain: domain.clone(),
        round,
        block,
        valid_qc,
        timeout_certificate_id,
        proposer: proposer.clone(),
        signature: empty_signature(&validators.validators[index]),
    };
    let message = consensus_v2_proposal_signing_bytes(&proposal).expect("proposal bytes");
    proposal.signature.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &keys[index].private_key,
            &message,
            CONSENSUS_V2_PROPOSAL_CONTEXT,
        )
        .expect("proposal sign"),
    );
    proposal
}

fn signed_votes(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    keys: &[MlDsa65KeyPair],
    round: ConsensusV2Round,
    phase: ConsensusV2Phase,
    block: Option<ConsensusV2BlockRef>,
) -> Vec<ConsensusV2Vote> {
    validators
        .validators
        .iter()
        .zip(keys)
        .take(validators.quorum)
        .map(|(validator, key)| {
            let mut vote = ConsensusV2Vote {
                schema: CONSENSUS_V2_VOTE_SCHEMA.to_string(),
                domain: domain.clone(),
                round,
                phase,
                block: block.clone(),
                validator: validator.validator_id.clone(),
                signature: empty_signature(validator),
            };
            let message = consensus_v2_vote_signing_bytes(&vote).expect("vote bytes");
            vote.signature.signature_hex = bytes_to_hex(
                &ml_dsa_65_sign_with_context(&key.private_key, &message, CONSENSUS_V2_VOTE_CONTEXT)
                    .expect("vote sign"),
            );
            vote
        })
        .collect()
}

fn signed_timeout_votes(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    keys: &[MlDsa65KeyPair],
    round: ConsensusV2Round,
    high_qcs: &[Option<ConsensusV2QcRef>],
) -> Vec<ConsensusV2TimeoutVote> {
    validators
        .validators
        .iter()
        .zip(keys)
        .take(validators.quorum)
        .enumerate()
        .map(|(index, (validator, key))| {
            let mut vote = ConsensusV2TimeoutVote {
                schema: CONSENSUS_V2_TIMEOUT_VOTE_SCHEMA.to_string(),
                domain: domain.clone(),
                round,
                phase: ConsensusV2Phase::Precommit,
                high_qc: high_qcs.get(index).cloned().unwrap_or(None),
                validator: validator.validator_id.clone(),
                signature: empty_signature(validator),
            };
            let message =
                consensus_v2_timeout_vote_signing_bytes(&vote).expect("timeout vote bytes");
            vote.signature.signature_hex = bytes_to_hex(
                &ml_dsa_65_sign_with_context(
                    &key.private_key,
                    &message,
                    CONSENSUS_V2_TIMEOUT_VOTE_CONTEXT,
                )
                .expect("timeout vote sign"),
            );
            vote
        })
        .collect()
}

#[test]
fn canonical_v2_artifacts_bind_domain_committee_round_block_phase_and_signer() {
    let (validators, keys) = committee(4);
    let domain = domain(&validators);
    let block = consensus_v2_block_ref(
        &domain,
        1,
        "11".repeat(48),
        "22".repeat(48),
        "33".repeat(48),
    )
    .expect("block");
    let round = ConsensusV2Round { height: 1, view: 0 };
    let proposal = signed_proposal(
        &domain,
        &validators,
        &keys,
        round,
        block.clone(),
        None,
        None,
    );
    verify_consensus_v2_proposal(
        &domain,
        &validators,
        &proposal,
        None,
        &ConsensusV2QcGraph::default(),
    )
    .expect("proposal verify");
    let votes = signed_votes(
        &domain,
        &validators,
        &keys,
        round,
        ConsensusV2Phase::Prepare,
        Some(block.clone()),
    );
    let prepare_qc = certify_consensus_v2_votes(
        &domain,
        &validators,
        round,
        ConsensusV2Phase::Prepare,
        Some(block.clone()),
        votes,
    )
    .expect("prepare QC");
    verify_consensus_v2_qc(&domain, &validators, &prepare_qc).expect("QC verify");

    let mut wrong_domain = domain.clone();
    wrong_domain.committee_epoch += 1;
    assert!(verify_consensus_v2_qc(&wrong_domain, &validators, &prepare_qc).is_err());
    let mut wrong_phase = prepare_qc.clone();
    wrong_phase.phase = ConsensusV2Phase::Precommit;
    assert!(verify_consensus_v2_qc(&domain, &validators, &wrong_phase).is_err());
}

#[test]
fn typed_timeout_qc_ranking_uses_numeric_round_and_resolves_every_reference() {
    let (validators, keys) = committee(4);
    let domain = domain(&validators);
    let block = consensus_v2_block_ref(
        &domain,
        1,
        "11".repeat(48),
        "22".repeat(48),
        "33".repeat(48),
    )
    .expect("block");
    let mut graph = ConsensusV2QcGraph::default();
    let low_round = ConsensusV2Round { height: 1, view: 9 };
    let high_round = ConsensusV2Round {
        height: 1,
        view: 10,
    };
    let low_qc = certify_consensus_v2_votes(
        &domain,
        &validators,
        low_round,
        ConsensusV2Phase::Prepare,
        Some(block.clone()),
        signed_votes(
            &domain,
            &validators,
            &keys,
            low_round,
            ConsensusV2Phase::Prepare,
            Some(block.clone()),
        ),
    )
    .expect("low QC");
    let high_qc = certify_consensus_v2_votes(
        &domain,
        &validators,
        high_round,
        ConsensusV2Phase::Prepare,
        Some(block),
        signed_votes(
            &domain,
            &validators,
            &keys,
            high_round,
            ConsensusV2Phase::Prepare,
            Some(low_qc.block.clone().expect("block")),
        ),
    )
    .expect("high QC");
    let low_ref = graph
        .insert_verified(&domain, &validators, low_qc)
        .expect("insert low QC");
    let high_ref = graph
        .insert_verified(&domain, &validators, high_qc)
        .expect("insert high QC");
    assert_ne!(low_ref.certificate_id, high_ref.certificate_id);
    let timeout_round = ConsensusV2Round {
        height: 1,
        view: 10,
    };
    let votes = signed_timeout_votes(
        &domain,
        &validators,
        &keys,
        timeout_round,
        &[
            Some(low_ref),
            Some(high_ref.clone()),
            Some(high_ref.clone()),
        ],
    );
    let certificate = certify_consensus_v2_timeouts(
        &domain,
        &validators,
        timeout_round,
        ConsensusV2Phase::Precommit,
        votes,
        &graph,
    )
    .expect("timeout certificate");
    assert_eq!(certificate.high_qc, Some(high_ref));
}

#[test]
fn durable_lock_rejects_conflict_and_precommit_qc_is_the_only_commit_boundary() {
    let (validators, keys) = committee(4);
    let domain = domain(&validators);
    let round = ConsensusV2Round { height: 1, view: 0 };
    let block_a = consensus_v2_block_ref(
        &domain,
        1,
        "11".repeat(48),
        "aa".repeat(48),
        "33".repeat(48),
    )
    .expect("block A");
    let block_b = consensus_v2_block_ref(
        &domain,
        1,
        "11".repeat(48),
        "bb".repeat(48),
        "44".repeat(48),
    )
    .expect("block B");
    let prepare_qc = certify_consensus_v2_votes(
        &domain,
        &validators,
        round,
        ConsensusV2Phase::Prepare,
        Some(block_a.clone()),
        signed_votes(
            &domain,
            &validators,
            &keys,
            round,
            ConsensusV2Phase::Prepare,
            Some(block_a.clone()),
        ),
    )
    .expect("prepare QC");
    assert!(consensus_v2_commit_from_precommit_qc(&domain, &validators, &prepare_qc).is_err());
    let initial = initial_consensus_v2_safety_state(&domain, 1).expect("safety state");
    let locked = authorize_consensus_v2_precommit_vote(&initial, &domain, &validators, &prepare_qc)
        .expect("durable lock");
    let conflicting = signed_proposal(
        &domain,
        &validators,
        &keys,
        ConsensusV2Round { height: 1, view: 1 },
        block_b,
        None,
        Some("55".repeat(48)),
    );
    assert!(apply_consensus_v2_prepare_vote_to_safety(&locked, &conflicting).is_err());

    let precommit_qc = certify_consensus_v2_votes(
        &domain,
        &validators,
        round,
        ConsensusV2Phase::Precommit,
        Some(block_a.clone()),
        signed_votes(
            &domain,
            &validators,
            &keys,
            round,
            ConsensusV2Phase::Precommit,
            Some(block_a.clone()),
        ),
    )
    .expect("precommit QC");
    assert_eq!(
        consensus_v2_commit_from_precommit_qc(&domain, &validators, &precommit_qc).expect("commit"),
        block_a.clone()
    );
    let proposal = signed_proposal(
        &domain,
        &validators,
        &keys,
        round,
        block_a.clone(),
        None,
        None,
    );
    let commit = ConsensusV2Commit {
        schema: CONSENSUS_V2_COMMIT_SCHEMA.to_string(),
        proposal,
        prior_qcs: Vec::new(),
        timeout_certificate: None,
        prepare_qc,
        precommit_qc,
    };
    assert_eq!(
        verify_consensus_v2_commit(
            &domain,
            &validators,
            &commit,
            &ConsensusV2QcGraph::default(),
        )
        .expect("self-contained commit verifies"),
        block_a
    );
    let mut prepare_only = commit.clone();
    prepare_only.precommit_qc = prepare_only.prepare_qc.clone();
    assert!(verify_consensus_v2_commit(
        &domain,
        &validators,
        &prepare_only,
        &ConsensusV2QcGraph::default(),
    )
    .is_err());
}

#[test]
fn exhaustive_quorum_model_covers_n4_and_n6() {
    let n4 = model_consensus_v2_quorum_intersection(4).expect("n=4 model");
    assert_eq!(n4.fault_tolerance, 1);
    assert_eq!(n4.quorum, 3);
    assert_eq!(n4.minimum_intersection, 2);
    assert_eq!(n4.minimum_honest_intersection, 1);

    let n6 = model_consensus_v2_quorum_intersection(6).expect("n=6 model");
    assert_eq!(n6.fault_tolerance, 1);
    assert_eq!(n6.quorum, 5);
    assert_eq!(n6.minimum_intersection, 4);
    assert_eq!(n6.minimum_honest_intersection, 3);
}

#[test]
fn failed_proposer_advances_and_commits_for_n4_and_n6_with_restart_safe_locks() {
    for validator_count in [4, 6] {
        let (validators, keys) = committee(validator_count);
        let domain = domain(&validators);
        let graph = ConsensusV2QcGraph::default();
        let failed_round = ConsensusV2Round { height: 1, view: 0 };
        let timeout_votes = signed_timeout_votes(
            &domain,
            &validators,
            &keys,
            failed_round,
            &vec![None; validators.quorum],
        );
        let timeout = certify_consensus_v2_timeouts(
            &domain,
            &validators,
            failed_round,
            ConsensusV2Phase::Precommit,
            timeout_votes,
            &graph,
        )
        .expect("failed-proposer timeout certificate");

        let recovery_round = ConsensusV2Round { height: 1, view: 1 };
        let block = consensus_v2_block_ref(
            &domain,
            1,
            "11".repeat(48),
            "22".repeat(48),
            "33".repeat(48),
        )
        .expect("recovery block");
        let proposal = signed_proposal(
            &domain,
            &validators,
            &keys,
            recovery_round,
            block.clone(),
            None,
            Some(timeout.certificate_id.clone()),
        );
        let initial = initial_consensus_v2_safety_state(&domain, 1).expect("initial state");
        let prepared = authorize_consensus_v2_prepare_vote(
            &initial,
            &domain,
            &validators,
            &proposal,
            Some(&timeout),
            &graph,
        )
        .expect("authorize recovery prepare");
        let prepare_qc = certify_consensus_v2_votes(
            &domain,
            &validators,
            recovery_round,
            ConsensusV2Phase::Prepare,
            Some(block.clone()),
            signed_votes(
                &domain,
                &validators,
                &keys,
                recovery_round,
                ConsensusV2Phase::Prepare,
                Some(block.clone()),
            ),
        )
        .expect("recovery prepare QC");
        let precommitted =
            authorize_consensus_v2_precommit_vote(&prepared, &domain, &validators, &prepare_qc)
                .expect("authorize recovery precommit");
        let persisted = serde_json::to_vec(&precommitted).expect("persist safety state");
        let restarted: ConsensusV2SafetyState =
            serde_json::from_slice(&persisted).expect("restart safety state");
        assert_eq!(restarted, precommitted);
        assert!(authorize_consensus_v2_precommit_vote(
            &restarted,
            &domain,
            &validators,
            &prepare_qc,
        )
        .is_err());

        let precommit_qc = certify_consensus_v2_votes(
            &domain,
            &validators,
            recovery_round,
            ConsensusV2Phase::Precommit,
            Some(block.clone()),
            signed_votes(
                &domain,
                &validators,
                &keys,
                recovery_round,
                ConsensusV2Phase::Precommit,
                Some(block.clone()),
            ),
        )
        .expect("recovery precommit QC");
        assert_eq!(
            consensus_v2_commit_from_precommit_qc(&domain, &validators, &precommit_qc)
                .expect("recovery commit"),
            block
        );
    }
}

#[test]
fn adversarial_delay_loss_duplication_reorder_partition_byzantine_and_restart_are_safe_n4_n6() {
    for validator_count in [4, 6] {
        let (validators, keys) = committee(validator_count);
        let domain = domain(&validators);
        let round = ConsensusV2Round { height: 1, view: 0 };
        let block_a = consensus_v2_block_ref(
            &domain,
            1,
            "11".repeat(48),
            "aa".repeat(48),
            "33".repeat(48),
        )
        .expect("block A");
        let block_b = consensus_v2_block_ref(
            &domain,
            1,
            "11".repeat(48),
            "bb".repeat(48),
            "44".repeat(48),
        )
        .expect("block B");
        let proposal_a = signed_proposal(
            &domain,
            &validators,
            &keys,
            round,
            block_a.clone(),
            None,
            None,
        );

        // Network reorder is canonicalized, while duplicate delivery and every
        // under-quorum loss/partition fail closed without a certificate.
        let ordered_votes = signed_votes(
            &domain,
            &validators,
            &keys,
            round,
            ConsensusV2Phase::Prepare,
            Some(block_a.clone()),
        );
        let ordered_qc = certify_consensus_v2_votes(
            &domain,
            &validators,
            round,
            ConsensusV2Phase::Prepare,
            Some(block_a.clone()),
            ordered_votes.clone(),
        )
        .expect("ordered QC");
        let mut reordered_votes = ordered_votes.clone();
        reordered_votes.reverse();
        let reordered_qc = certify_consensus_v2_votes(
            &domain,
            &validators,
            round,
            ConsensusV2Phase::Prepare,
            Some(block_a.clone()),
            reordered_votes,
        )
        .expect("reordered QC");
        assert_eq!(ordered_qc, reordered_qc);
        let mut duplicated_votes = ordered_votes.clone();
        duplicated_votes.push(ordered_votes[0].clone());
        assert!(certify_consensus_v2_votes(
            &domain,
            &validators,
            round,
            ConsensusV2Phase::Prepare,
            Some(block_a.clone()),
            duplicated_votes,
        )
        .is_err());
        for partition in [
            ordered_votes[..validators.quorum - 1].to_vec(),
            ordered_votes[validators.quorum - 1..].to_vec(),
        ] {
            assert!(certify_consensus_v2_votes(
                &domain,
                &validators,
                round,
                ConsensusV2Phase::Prepare,
                Some(block_a.clone()),
                partition,
            )
            .is_err());
        }

        // Even with one Byzantine equivocator, a conflicting later-view block
        // cannot cite the locked prepare QC. Restart preserves the honest lock.
        let mut graph = ConsensusV2QcGraph::default();
        let high_qc = graph
            .insert_verified(&domain, &validators, ordered_qc.clone())
            .expect("insert prepare QC");
        let initial = initial_consensus_v2_safety_state(&domain, 1).expect("initial safety");
        let prepared = authorize_consensus_v2_prepare_vote(
            &initial,
            &domain,
            &validators,
            &proposal_a,
            None,
            &graph,
        )
        .expect("prepare safety");
        let locked =
            authorize_consensus_v2_precommit_vote(&prepared, &domain, &validators, &ordered_qc)
                .expect("precommit lock");
        let restarted: ConsensusV2SafetyState =
            serde_json::from_slice(&serde_json::to_vec(&locked).expect("serialize durable lock"))
                .expect("restart durable lock");
        let timeout_votes = signed_timeout_votes(
            &domain,
            &validators,
            &keys,
            round,
            &vec![Some(high_qc.clone()); validators.quorum],
        );
        let mut delayed_reordered_timeouts = timeout_votes;
        delayed_reordered_timeouts.reverse();
        let timeout = certify_consensus_v2_timeouts(
            &domain,
            &validators,
            round,
            ConsensusV2Phase::Precommit,
            delayed_reordered_timeouts,
            &graph,
        )
        .expect("delayed/reordered timeout quorum");
        let conflicting = signed_proposal(
            &domain,
            &validators,
            &keys,
            ConsensusV2Round { height: 1, view: 1 },
            block_b,
            Some(high_qc),
            Some(timeout.certificate_id.clone()),
        );
        assert!(verify_consensus_v2_proposal(
            &domain,
            &validators,
            &conflicting,
            Some(&timeout),
            &graph,
        )
        .is_err());
        assert!(authorize_consensus_v2_prepare_vote(
            &restarted,
            &domain,
            &validators,
            &conflicting,
            Some(&timeout),
            &graph,
        )
        .is_err());
    }
}
