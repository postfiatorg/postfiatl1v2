use super::*;

fn proposal_and_qc(
    validators: &ConsensusV2ValidatorSet,
    keys: &[MlDsa65KeyPair],
    domain: &ConsensusV2Domain,
    round: ConsensusV2Round,
    block: &ConsensusV2BlockRef,
) -> (ConsensusV2Proposal, ConsensusV2QuorumCertificate) {
    let proposal = signed_proposal(domain, validators, keys, round, block.clone(), None, None);
    let qc = certify_consensus_v2_votes(
        domain,
        validators,
        round,
        ConsensusV2Phase::Prepare,
        Some(block.clone()),
        signed_votes(
            domain,
            validators,
            keys,
            round,
            ConsensusV2Phase::Prepare,
            Some(block.clone()),
        ),
    )
    .unwrap();
    (proposal, qc)
}

#[test]
fn every_phase_rejects_a_round_below_any_persisted_phase() {
    let (validators, keys) = committee(4);
    let domain = domain(&validators);
    let old = ConsensusV2Round { height: 1, view: 0 };
    let newer = ConsensusV2Round { height: 1, view: 1 };
    let block = consensus_v2_block_ref(
        &domain,
        1,
        "11".repeat(48),
        "22".repeat(48),
        "33".repeat(48),
    )
    .unwrap();
    let (proposal, old_qc) = proposal_and_qc(&validators, &keys, &domain, old, &block);
    let (_, new_qc) = proposal_and_qc(&validators, &keys, &domain, newer, &block);
    let mut graph = ConsensusV2QcGraph::default();
    graph
        .insert_verified(&domain, &validators, new_qc.clone())
        .unwrap();
    let initial = initial_consensus_v2_safety_state(&domain, 1).unwrap();
    let timeout = certify_consensus_v2_timeouts(
        &domain,
        &validators,
        old,
        ConsensusV2Phase::Precommit,
        signed_timeout_votes(&domain, &validators, &keys, old, &[None, None, None]),
        &ConsensusV2QcGraph::default(),
    )
    .unwrap();
    let new_proposal = signed_proposal(
        &domain,
        &validators,
        &keys,
        newer,
        block,
        None,
        Some(timeout.certificate_id.clone()),
    );
    let prepared = authorize_consensus_v2_prepare_vote(
        &initial,
        &domain,
        &validators,
        &new_proposal,
        Some(&timeout),
        &graph,
    )
    .unwrap();
    let precommitted =
        authorize_consensus_v2_precommit_vote(&initial, &domain, &validators, &new_qc).unwrap();
    let timed_out =
        authorize_consensus_v2_timeout_vote(&initial, &domain, &validators, newer, None, &graph)
            .unwrap();
    for state in [prepared, precommitted, timed_out] {
        let serialized = serde_json::to_vec(&state).unwrap();
        let restarted: ConsensusV2SafetyState = serde_json::from_slice(&serialized).unwrap();
        assert!(authorize_consensus_v2_prepare_vote(
            &restarted,
            &domain,
            &validators,
            &proposal,
            None,
            &graph,
        )
        .is_err());
        assert!(
            authorize_consensus_v2_precommit_vote(&restarted, &domain, &validators, &old_qc,)
                .is_err()
        );
        assert!(authorize_consensus_v2_timeout_vote(
            &restarted,
            &domain,
            &validators,
            old,
            None,
            &graph,
        )
        .is_err());
        assert_eq!(serde_json::to_vec(&restarted).unwrap(), serialized);
    }
}

#[test]
fn same_view_phase_progression_and_historical_verification_remain_valid() {
    let (validators, keys) = committee(4);
    let domain = domain(&validators);
    let round = ConsensusV2Round { height: 1, view: 0 };
    let block = consensus_v2_block_ref(
        &domain,
        1,
        "11".repeat(48),
        "22".repeat(48),
        "33".repeat(48),
    )
    .unwrap();
    let (proposal, prepare_qc) = proposal_and_qc(&validators, &keys, &domain, round, &block);
    let mut graph = ConsensusV2QcGraph::default();
    graph
        .insert_verified(&domain, &validators, prepare_qc.clone())
        .unwrap();
    let initial = initial_consensus_v2_safety_state(&domain, 1).unwrap();
    let prepared = authorize_consensus_v2_prepare_vote(
        &initial,
        &domain,
        &validators,
        &proposal,
        None,
        &graph,
    )
    .unwrap();
    let timed_out =
        authorize_consensus_v2_timeout_vote(&prepared, &domain, &validators, round, None, &graph)
            .unwrap();
    let precommitted =
        authorize_consensus_v2_precommit_vote(&timed_out, &domain, &validators, &prepare_qc)
            .expect("a timeout at the same view permits a delayed prepare certificate");
    assert_eq!(precommitted.highest_precommit_round, Some(round));
    let precommit_qc = certify_consensus_v2_votes(
        &domain,
        &validators,
        round,
        ConsensusV2Phase::Precommit,
        Some(block.clone()),
        signed_votes(
            &domain,
            &validators,
            &keys,
            round,
            ConsensusV2Phase::Precommit,
            Some(block.clone()),
        ),
    )
    .unwrap();
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
            &ConsensusV2QcGraph::default()
        )
        .unwrap(),
        block,
    );
    let next_height = initial_consensus_v2_safety_state(&domain, 2).unwrap();
    assert!(next_height.highest_prepare_round.is_none());
    assert!(next_height.highest_precommit_round.is_none());
    assert!(next_height.highest_timeout_round.is_none());
}

#[test]
fn delayed_certificate_quorum_schedules_preserve_unique_commit_for_n4_and_n6() {
    // Exhaust all two-quorum memberships and fault sets for this specific
    // delayed-certificate schedule. This is a bounded state-machine model;
    // the node regression separately exercises signatures and disk state.
    let mut schedules = 0;
    for n in [4usize, 6] {
        let (validators, keys) = committee(n);
        let domain = domain(&validators);
        let q = validators.quorum;
        let f = (n - 1) / 3;
        let old = ConsensusV2Round { height: 1, view: 0 };
        let new = ConsensusV2Round { height: 1, view: 1 };
        let block_x = consensus_v2_block_ref(
            &domain,
            1,
            "11".repeat(48),
            "22".repeat(48),
            "33".repeat(48),
        )
        .unwrap();
        let block_y = consensus_v2_block_ref(
            &domain,
            1,
            "11".repeat(48),
            "44".repeat(48),
            "55".repeat(48),
        )
        .unwrap();
        let (proposal_x, qc_x) = proposal_and_qc(&validators, &keys, &domain, old, &block_x);
        let (proposal_y, qc_y) = proposal_and_qc(&validators, &keys, &domain, new, &block_y);
        let quorums: Vec<usize> = (0..1usize << n)
            .filter(|mask| mask.count_ones() as usize == q)
            .collect();
        let faults: Vec<usize> = (0..1usize << n)
            .filter(|mask| mask.count_ones() as usize <= f)
            .collect();
        for &x_signers in &quorums {
            for &y_signers in &quorums {
                for &faulty in &faults {
                    let initial = initial_consensus_v2_safety_state(&domain, 1).unwrap();
                    let mut states = vec![initial; n];
                    for (i, state) in states.iter_mut().enumerate() {
                        if x_signers & (1 << i) != 0 && faulty & (1 << i) == 0 {
                            *state = apply_consensus_v2_prepare_vote_to_safety(state, &proposal_x)
                                .unwrap();
                        }
                        if y_signers & (1 << i) != 0 && faulty & (1 << i) == 0 {
                            *state = apply_consensus_v2_prepare_vote_to_safety(state, &proposal_y)
                                .unwrap();
                        }
                    }
                    let mut old_precommits = faulty.count_ones() as usize;
                    for (i, state) in states.iter_mut().enumerate() {
                        if faulty & (1 << i) == 0 {
                            if let Ok(next) =
                                apply_consensus_v2_precommit_vote_to_safety(state, &qc_x)
                            {
                                old_precommits += 1;
                                *state = next;
                            }
                        }
                    }
                    assert!(
                        old_precommits < q,
                        "delayed X certificate reached a commit quorum"
                    );
                    let mut new_precommits = 0;
                    for (i, state) in states.iter_mut().enumerate() {
                        if y_signers & (1 << i) != 0 {
                            if faulty & (1 << i) == 0 {
                                *state = apply_consensus_v2_precommit_vote_to_safety(state, &qc_y)
                                    .unwrap();
                            }
                            new_precommits += 1;
                        }
                    }
                    assert_eq!(new_precommits, q);
                    schedules += 1;
                }
            }
        }
    }
    assert_eq!(schedules, 332);
}
