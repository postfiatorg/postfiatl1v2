// Local dry run of the release-day signer committee rotation
// (docs/review/signer-committee-rotation-dry-run-20261005.md): six validators,
// quorum 5, validator-5's key rotated from seed 6 to seed 16.

fn dry_run_key(seed: u8) -> MlDsa65KeyPair {
    ml_dsa_65_keygen_from_seed(&[seed; 32])
}

fn dry_run_ethereum_checkpoint(
    fixture: &BridgePolicyFixture,
    committee: &FastSwapCommitteeV1,
) -> postfiat_types::EthereumFinalizedCheckpointV1 {
    postfiat_types::EthereumFinalizedCheckpointV1 {
        schema_version: postfiat_types::ETHEREUM_CHECKPOINT_SCHEMA_V1,
        pftl_domain: committee.domain.chain.clone(),
        route_id: fixture.route_id.clone(),
        route_config_digest: FastSwapOpaqueHashV1([0xa1; 48]),
        ethereum_chain_id: 1,
        block_number: 100,
        block_hash: [0x61; 32],
        receipts_root: [0x62; 32],
        observed_head_number: 120,
        minimum_confirmations: fixture.policy.minimum_confirmations,
        authority_epoch: committee.domain.committee_epoch,
        committee_root: committee.domain.committee_root,
        handoff_controller: [0x11; 20],
        wrapped_navcoin_token: [0x33; 20],
        handoff_controller_code_hash: fixture.policy.handoff_controller_code_hash,
        wrapped_navcoin_code_hash: fixture.policy.wrapped_navcoin_code_hash,
    }
}

/// `signers` are (validator index, key seed) in validator order.
fn dry_run_ethereum_certificate(
    checkpoint: &postfiat_types::EthereumFinalizedCheckpointV1,
    signers: &[(usize, u8)],
) -> postfiat_types::EthereumCheckpointCertificateV1 {
    let votes = signers
        .iter()
        .map(|(index, seed)| {
            let mut vote = postfiat_types::EthereumCheckpointVoteV1 {
                validator_id: format!("validator-{index}"),
                signature: Vec::new(),
            };
            vote.signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                &dry_run_key(*seed).private_key,
                &vote.signing_bytes(checkpoint).expect("vote bytes"),
                postfiat_types::ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
            )
            .expect("sign Ethereum checkpoint vote");
            vote
        })
        .collect();
    postfiat_types::EthereumCheckpointCertificateV1 {
        checkpoint: checkpoint.clone(),
        votes,
    }
}

fn dry_run_verify(
    committee: &FastSwapCommitteeV1,
    certificate: &postfiat_types::EthereumCheckpointCertificateV1,
) -> Result<(), String> {
    postfiat_bridge::verify_ethereum_checkpoint_certificate(committee, certificate)
        .map(|_| ())
        .map_err(|error| format!("{error:?}"))
}

fn dry_run_fastlane_certificate(
    checkpoint: &postfiat_types::FastLaneCheckpointV1,
    signers: &[(usize, u8)],
) -> postfiat_types::FastLaneCheckpointCertificateV1 {
    let votes = signers
        .iter()
        .map(|(index, seed)| {
            let mut vote = postfiat_types::FastLaneCheckpointVoteV1 {
                checkpoint: checkpoint.clone(),
                validator_id: format!("validator-{index}"),
                signature: Vec::new(),
            };
            vote.signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                &dry_run_key(*seed).private_key,
                &vote.signing_bytes().expect("vote bytes"),
                postfiat_types::FASTLANE_CHECKPOINT_CONTEXT_V1,
            )
            .expect("sign FastLane checkpoint vote");
            vote
        })
        .collect();
    postfiat_types::FastLaneCheckpointCertificateV1 { votes }
}

fn dry_run_control_certificate(
    committee: &FastSwapCommitteeV1,
    action: postfiat_types::FastLaneControlActionV1,
    signers: &[(usize, u8)],
) -> postfiat_types::FastLaneControlCertificateV1 {
    let digest = action.digest().expect("control action digest");
    let votes = signers
        .iter()
        .map(|(index, seed)| {
            let mut vote = postfiat_types::FastLaneControlVoteV1 {
                committee: committee.domain.clone(),
                action_digest: digest,
                validator_id: format!("validator-{index}"),
                signature: Vec::new(),
            };
            vote.signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                &dry_run_key(*seed).private_key,
                &vote.signing_bytes().expect("vote bytes"),
                postfiat_types::FASTLANE_CONTROL_CONTEXT_V1,
            )
            .expect("sign control vote");
            vote
        })
        .collect();
    postfiat_types::FastLaneControlCertificateV1 { action, votes }
}

#[test]
fn signer_committee_rotation_dry_run_six_validators() {
    let timer = std::time::Instant::now();
    let mut lap = std::time::Instant::now();
    let mut step = |name: &str, detail: String| {
        eprintln!(
            "dry-run | {name} | {detail} | {} ms",
            lap.elapsed().as_millis()
        );
        lap = std::time::Instant::now();
    };
    let mut fixture = bridge_policy_fixture();
    let epoch1 = fixture.epoch1.clone();
    let epoch2 = fixture.epoch2.clone();
    // Genesis keys are seeds 1..=6; validator-5 now holds seed 16.
    let others = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5)];
    let rotated_v5 = (5, 16);
    let four_and_rotated_v5 = [(0, 1), (1, 2), (2, 3), (3, 4), rotated_v5];
    assert_eq!(
        (epoch1.domain.validator_count, epoch1.domain.quorum),
        (6, 5)
    );
    assert_eq!(
        (epoch2.domain.validator_count, epoch2.domain.quorum),
        (6, 5)
    );
    step(
        "setup",
        format!(
            "route {} authority_epoch 1 root {}",
            fixture.route_id,
            bytes_to_hex(&epoch1.domain.committee_root.0)
        ),
    );

    // 1. Live defect: under epoch 1 the rotated validator-5 cannot sign, so a
    // certificate needs all five of validators 0-4.
    let checkpoint1 = dry_run_ethereum_checkpoint(&fixture, &epoch1);
    let with_rotated = dry_run_verify(
        &epoch1,
        &dry_run_ethereum_certificate(&checkpoint1, &four_and_rotated_v5),
    );
    assert!(with_rotated.is_err());
    let without_one = dry_run_verify(
        &epoch1,
        &dry_run_ethereum_certificate(&checkpoint1, &others[..4]),
    );
    assert!(without_one.is_err());
    dry_run_verify(
        &epoch1,
        &dry_run_ethereum_certificate(&checkpoint1, &others),
    )
    .expect("validators 0-4 still sign epoch 1");
    step(
        "defect reproduced",
        format!("4+rotated v5: {with_rotated:?}; 4 only: {without_one:?}; 0-4: ok"),
    );

    // 2. Issuer update before activation is rejected: epoch 2 is not a ledger committee.
    let early = {
        let operation = bridge_policy_update(&fixture, &fixture.issuer, &epoch2);
        bridge_policy_apply(&mut fixture, false, operation)
    };
    assert!(!early.accepted, "{early:?}");
    step(
        "update before activation",
        format!("rejected {}", early.code),
    );

    // 3. Epoch-1 FastLane checkpoints: one intermediate, then the drained final one.
    let state = postfiat_types::FastLaneStateV1 {
        schema_version: 1,
        committee: epoch1.domain.clone(),
        objects: std::collections::BTreeMap::new(),
        reservations: std::collections::BTreeMap::new(),
        swaps: std::collections::BTreeMap::new(),
        imported_deposits: std::collections::BTreeSet::new(),
        exit_claims: std::collections::BTreeMap::new(),
        terminal_tombstones: std::collections::BTreeMap::new(),
        asset_rules: std::collections::BTreeMap::new(),
        holder_permits: std::collections::BTreeMap::new(),
        revoked_holder_permits: std::collections::BTreeMap::new(),
        policy_snapshots: std::collections::BTreeMap::new(),
        prepare_fences: std::collections::BTreeMap::new(),
        pending_fee_burns: std::collections::BTreeMap::new(),
        anchored_checkpoints: std::collections::BTreeSet::new(),
    };
    let first =
        crate::fastswap_checkpoint::build_fastlane_checkpoint(&state, &fixture.ledger, None, 1)
            .expect("first checkpoint");
    crate::fastswap_checkpoint::anchor_fastlane_checkpoint(
        &mut fixture.ledger,
        &dry_run_fastlane_certificate(&first, &others),
    )
    .expect("anchor first checkpoint");
    let first_id = first.checkpoint_id().expect("first id");
    step(
        "first epoch-1 checkpoint anchored",
        format!("id {}", bytes_to_hex(&first_id.0)),
    );

    let final_checkpoint = crate::fastswap_checkpoint::build_fastlane_checkpoint(
        &state,
        &fixture.ledger,
        Some(first_id),
        2,
    )
    .expect("final checkpoint");
    assert!(final_checkpoint.drain_ready && final_checkpoint.exit_claim_totals.is_empty());
    let final_certificate = dry_run_fastlane_certificate(&final_checkpoint, &others);
    let activate = postfiat_types::FastLaneControlActionV1::ActivateCommittee {
        committee: epoch2.clone(),
        final_checkpoint: final_certificate.clone(),
    };
    // Activation before the final checkpoint is anchored is refused.
    let mut unanchored = fixture.ledger.clone();
    let not_anchored = crate::fastswap_control::execute_fastlane_control(
        &mut unanchored,
        &dry_run_control_certificate(&epoch1, activate.clone(), &others),
        10,
    );
    assert_eq!(
        not_anchored,
        Err(crate::fastswap_control::FastSwapControlError::MissingFinalCheckpoint)
    );
    let mut scratch_ledger = fixture.ledger.clone();
    let rotated_final = crate::fastswap_checkpoint::anchor_fastlane_checkpoint(
        &mut scratch_ledger,
        &dry_run_fastlane_certificate(&final_checkpoint, &four_and_rotated_v5),
    );
    assert!(rotated_final.is_err());
    crate::fastswap_checkpoint::anchor_fastlane_checkpoint(&mut fixture.ledger, &final_certificate)
        .expect("anchor final checkpoint");
    step(
        "final drained epoch-1 checkpoint anchored",
        format!(
            "id {}; drain_ready true; activate before anchor: {not_anchored:?}; 4+rotated v5 anchor: {rotated_final:?}",
            bytes_to_hex(&final_checkpoint.checkpoint_id().expect("final id").0)
        ),
    );

    // 4. ActivateCommittee: epoch-1 control certificate, so validators 0-4 sign.
    let mut scratch_ledger = fixture.ledger.clone();
    let rotated_control = crate::fastswap_control::execute_fastlane_control(
        &mut scratch_ledger,
        &dry_run_control_certificate(&epoch1, activate.clone(), &four_and_rotated_v5),
        10,
    );
    assert_eq!(
        rotated_control,
        Err(crate::fastswap_control::FastSwapControlError::InvalidControlCertificate)
    );
    crate::fastswap_control::execute_fastlane_control(
        &mut fixture.ledger,
        &dry_run_control_certificate(&epoch1, activate, &others),
        10,
    )
    .expect("validators 0-4 activate epoch 2");
    assert_eq!(fixture.ledger.fastswap_committees.len(), 2);
    let route_policy = fixture
        .ledger
        .pftl_uniswap_route(&fixture.route_id)
        .unwrap()
        .ethereum_verification_policy
        .clone()
        .unwrap();
    assert_eq!(
        route_policy.authority_epoch, 1,
        "route waits for the issuer"
    );
    step(
        "ActivateCommittee epoch 2",
        format!(
            "accepted with 0-4; 4+rotated v5: {rotated_control:?}; route still epoch 1; root {}",
            bytes_to_hex(&epoch2.domain.committee_root.0)
        ),
    );

    // 5. Issuer update; a route with an open handoff is refused first.
    fixture.ledger.pftl_uniswap_routes[0].outstanding_bridge_claims_atoms = 1;
    fixture.ledger.pftl_uniswap_routes[0].authorized_valid_supply_atoms = 1;
    let busy = {
        let operation = bridge_policy_update(&fixture, &fixture.issuer, &epoch2);
        bridge_policy_apply(&mut fixture, false, operation)
    };
    assert_eq!(busy.code, "pftl_uniswap_bridge_policy_handoff_active");
    fixture.ledger.pftl_uniswap_routes[0].outstanding_bridge_claims_atoms = 0;
    fixture.ledger.pftl_uniswap_routes[0].authorized_valid_supply_atoms = 0;
    let update = {
        let operation = bridge_policy_update(&fixture, &fixture.issuer, &epoch2);
        bridge_policy_apply(&mut fixture, false, operation)
    };
    assert!(update.accepted, "{update:?}");
    let route = fixture
        .ledger
        .pftl_uniswap_route(&fixture.route_id)
        .unwrap();
    let committee = crate::pftl_uniswap_ethereum_verification::committee_for_policy(
        &fixture.genesis,
        &fixture.ledger,
        route.ethereum_verification_policy.as_ref().unwrap(),
    )
    .expect("route committee")
    .clone();
    assert_eq!(committee, epoch2);
    step(
        "issuer bridge policy update",
        format!(
            "open handoff: {}; accepted {} at height 10; authority_epoch 2",
            busy.code, update.code
        ),
    );

    // 6. Epoch 2: rotated validator-5 plus three others with validator-4 absent... 5 of 6.
    let checkpoint2 = dry_run_ethereum_checkpoint(&fixture, &committee);
    dry_run_verify(
        &committee,
        &dry_run_ethereum_certificate(&checkpoint2, &four_and_rotated_v5),
    )
    .expect("5 of 6 with the rotated validator-5 and validator-4 absent");
    let stale_v5 = dry_run_verify(
        &committee,
        &dry_run_ethereum_certificate(&checkpoint2, &[(0, 1), (1, 2), (2, 3), (3, 4), (5, 6)]),
    );
    assert!(stale_v5.is_err());
    let epoch1_after = dry_run_verify(
        &committee,
        &dry_run_ethereum_certificate(&checkpoint1, &others),
    );
    assert!(epoch1_after.is_err());
    step(
        "epoch-2 checkpoint certificate",
        format!("0-3 + rotated v5 (v4 absent): ok; stale v5 key: {stale_v5:?}; epoch-1 checkpoint: {epoch1_after:?}"),
    );
    eprintln!("dry-run | total | {} ms", timer.elapsed().as_millis());
}
