use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

fn test_dir(label: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("postfiat-cobalt-shadow-{label}-{nonce}"))
}

fn identity(node_id: &str) -> CobaltShadowIdentity {
    CobaltShadowIdentity {
        node_id: node_id.to_string(),
        chain_id: "postfiat-shadow-test".to_string(),
        genesis_hash: "01".repeat(48),
        protocol_version: 1,
    }
}

fn two_node_fleet(root: &Path, limits: CobaltShadowLimits) -> Vec<CobaltShadowService> {
    let mut fleet = (0..2)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                identity(&format!("validator-{index}")),
                limits.clone(),
            )
            .expect("initialize")
        })
        .collect::<Vec<_>>();
    let peers = fleet
        .iter()
        .map(|service| {
            (
                service.state.identity.node_id.clone(),
                service.state.public_key_hex.clone(),
            )
        })
        .collect::<BTreeMap<_, _>>();
    for service in &mut fleet {
        service
            .replace_peer_registry(peers.clone())
            .expect("replace peers");
    }
    let participants = peers.keys().cloned().collect::<Vec<_>>();
    let commitments = fleet
        .iter_mut()
        .map(|service| service.create_beacon_commitment(1).expect("commit"))
        .collect::<Vec<_>>();
    let reveals = fleet
        .iter_mut()
        .map(|service| service.create_beacon_reveal(1).expect("reveal"))
        .collect::<Vec<_>>();
    for service in &mut fleet {
        service
            .install_common_randomness(
                1,
                participants.clone(),
                2,
                commitments.clone(),
                reveals.clone(),
            )
            .expect("install randomness");
    }
    fleet
}

#[test]
fn adversarial_drill_converges_without_live_authority() {
    let root = test_dir("drill");
    let report = run_cobalt_shadow_adversarial_drill(&root).expect("drill");
    assert!(report.ok, "{report:#?}");
    assert!(report.checks.restart_recovered_queue);
    assert!(report.checks.partition_healed);
    assert!(report.checks.censorship_healed);
    assert!(report.checks.member_loss_converged);
    assert!(report.checks.equivocation_rejected);
    assert!(report.checks.bad_signature_rejected);
    assert!(report.checks.randomness_failure_fails_closed);
    assert!(report.checks.live_authority_remained_disabled);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn queue_bound_is_enforced_before_durable_acceptance() {
    let root = test_dir("queue-bound");
    let limits = CobaltShadowLimits {
        max_queue_messages: 1,
        max_seen_messages: 1,
        ..CobaltShadowLimits::default()
    };
    let mut fleet = two_node_fleet(&root, limits);
    let first = fleet[0]
        .sign_message(
            1,
            CobaltShadowMessageKind::Rbc,
            hash_hex("test.payload", b"first"),
        )
        .expect("first");
    let second = fleet[0]
        .sign_message(
            1,
            CobaltShadowMessageKind::Abba,
            hash_hex("test.payload", b"second"),
        )
        .expect("second");
    fleet[1].receive(first).expect("queue first");
    let error = fleet[1].receive(second).expect_err("queue must be full");
    assert_eq!(error.kind(), io::ErrorKind::WouldBlock);
    assert_eq!(fleet[1].state.queued_messages.len(), 1);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn restart_verifies_state_signature_and_private_permissions() {
    let root = test_dir("restart");
    let fleet = two_node_fleet(&root, CobaltShadowLimits::default());
    let before = fleet[0].status();
    drop(fleet);
    let reopened = CobaltShadowService::open(root.join("validator-0")).expect("verified restart");
    assert_eq!(reopened.status().boot_count, before.boot_count + 1);
    assert!(!reopened.status().live_authority);
    #[cfg(unix)]
    {
        let mode = fs::metadata(root.join("validator-0").join(PRIVATE_FILE))
            .expect("private metadata")
            .permissions()
            .mode();
        assert_eq!(mode & 0o077, 0);
    }
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn live_registry_binding_requires_validator_signed_sidecar_keys() {
    let root = test_dir("live-registry-binding");
    let mut fleet = (0..6)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                identity(&format!("validator-{index}")),
                CobaltShadowLimits::default(),
            )
            .expect("initialize")
        })
        .collect::<Vec<_>>();
    let key_records = (0..6)
        .map(|index| {
            crate::create_validator_key_record(format!("validator-{index}")).expect("validator key")
        })
        .collect::<Vec<_>>();
    let registry = ValidatorRegistry {
        validators: key_records
            .iter()
            .map(|record| crate::ValidatorRegistryRecord {
                node_id: record.node_id.clone(),
                algorithm_id: record.algorithm_id.clone(),
                public_key_hex: record.public_key_hex.clone(),
            })
            .collect(),
    };
    let active_validators = key_records
        .iter()
        .map(|record| record.node_id.clone())
        .collect::<Vec<_>>();
    let registry_root =
        validator_registry_root(&registry, &active_validators).expect("registry root");
    let bindings = fleet
        .iter()
        .zip(&key_records)
        .map(|(service, record)| {
            let key_path = root.join(format!("{}.validator-keys.json", record.node_id));
            crate::write_validator_key_file(
                &key_path,
                &crate::ValidatorKeyFile {
                    validators: vec![record.clone()],
                },
            )
            .expect("write validator key");
            service
                .create_validator_binding(registry_root.clone(), &key_path)
                .expect("create validator binding")
        })
        .collect::<Vec<_>>();
    let manifest =
        build_registry_binding_manifest(registry_root.clone(), registry, bindings.clone(), 5, 911)
            .expect("build registry binding");
    for service in &mut fleet {
        service
            .bind_registry_manifest(&manifest)
            .expect("bind live registry");
        assert_eq!(service.status().registry_root, registry_root);
        assert_eq!(service.status().peer_count, 6);
    }
    let payload_hash = hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"distributed");
    let proposal = fleet[0]
        .create_protocol_proposal(&manifest, 912, payload_hash)
        .expect("create proposal");
    let contributions = fleet
        .iter_mut()
        .map(|service| {
            service
                .create_protocol_contribution(&manifest, &proposal)
                .expect("create contribution")
        })
        .collect::<Vec<_>>();
    let mut five_transcripts = Vec::new();
    let mut canonical_decision_ids = BTreeSet::new();
    for omitted in 0..6 {
        let subset = contributions
            .iter()
            .enumerate()
            .filter(|(index, _)| *index != omitted)
            .map(|(_, contribution)| contribution.clone())
            .collect::<Vec<_>>();
        let transcript = assemble_protocol_transcript(&manifest, proposal.clone(), subset)
            .expect("every five-of-six subset must assemble");
        let (decision, _) = fleet[0]
            .validate_protocol_transcript(&transcript, None)
            .expect("every five-of-six subset must validate");
        canonical_decision_ids.insert(decision.decision_id);
        five_transcripts.push(transcript);
    }
    assert_eq!(canonical_decision_ids.len(), 1);
    let mut duplicate_contributors = contributions[..5].to_vec();
    duplicate_contributors.push(contributions[0].clone());
    assert!(
        assemble_protocol_transcript(&manifest, proposal.clone(), duplicate_contributors,).is_err()
    );
    for first_omitted in 0..6 {
        for second_omitted in (first_omitted + 1)..6 {
            let subset = contributions
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != first_omitted && *index != second_omitted)
                .map(|(_, contribution)| contribution.clone())
                .collect::<Vec<_>>();
            assert!(
                assemble_protocol_transcript(&manifest, proposal.clone(), subset).is_err(),
                "every four-of-six subset must fail"
            );
        }
    }
    let left_transcript = five_transcripts
        .first()
        .expect("first five-of-six transcript")
        .clone();
    let right_transcript = five_transcripts
        .last()
        .expect("last five-of-six transcript")
        .clone();
    assert_ne!(
        hash_serialized(
            "postfiat.cobalt.shadow.protocol-transcript.v1",
            &left_transcript,
        )
        .expect("left transcript hash"),
        hash_serialized(
            "postfiat.cobalt.shadow.protocol-transcript.v1",
            &right_transcript,
        )
        .expect("right transcript hash")
    );
    let mut decisions = Vec::new();
    for (index, service) in fleet.iter_mut().enumerate() {
        let transcript = if index < 3 {
            &left_transcript
        } else {
            &right_transcript
        };
        decisions.push(
            service
                .commit_protocol_transcript(transcript)
                .expect("commit distributed transcript"),
        );
    }
    assert!(decisions
        .windows(2)
        .all(|pair| pair[0].decision_id == pair[1].decision_id));
    assert_ne!(
        decisions[0].support_certificate_hash,
        decisions[5].support_certificate_hash
    );
    assert!(fleet
        .windows(2)
        .all(|pair| pair[0].state.governance_digest == pair[1].state.governance_digest));
    assert_eq!(decisions[0].certificate_signer_count, 5);
    assert_eq!(decisions[0].signed_message_count, 41);
    let mut tampered = manifest;
    tampered.validator_bindings[0].cobalt_public_key_hex = "00".repeat(1952);
    assert!(fleet[0].bind_registry_manifest(&tampered).is_err());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn authority_lineage_reset_archives_drills_and_requires_active_transition_binding() {
    let root = test_dir("authority-lineage-reset");
    let mut fleet = (0..6)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                identity(&format!("validator-{index}")),
                CobaltShadowLimits::default(),
            )
            .expect("initialize")
        })
        .collect::<Vec<_>>();
    let key_records = (0..6)
        .map(|index| {
            crate::create_validator_key_record(format!("validator-{index}")).expect("validator key")
        })
        .collect::<Vec<_>>();
    let registry = ValidatorRegistry {
        validators: key_records
            .iter()
            .map(|record| crate::ValidatorRegistryRecord {
                node_id: record.node_id.clone(),
                algorithm_id: record.algorithm_id.clone(),
                public_key_hex: record.public_key_hex.clone(),
            })
            .collect(),
    };
    let active_validators = key_records
        .iter()
        .map(|record| record.node_id.clone())
        .collect::<Vec<_>>();
    let registry_root =
        validator_registry_root(&registry, &active_validators).expect("registry root");
    let bindings = fleet
        .iter()
        .zip(&key_records)
        .map(|(service, record)| {
            let key_path = root.join(format!("{}.validator-keys.json", record.node_id));
            crate::write_validator_key_file(
                &key_path,
                &crate::ValidatorKeyFile {
                    validators: vec![record.clone()],
                },
            )
            .expect("write validator key");
            service
                .create_validator_binding(registry_root.clone(), &key_path)
                .expect("create validator binding")
        })
        .collect::<Vec<_>>();
    let manifest =
        build_registry_binding_manifest(registry_root.clone(), registry, bindings, 5, 916)
            .expect("build registry binding");
    for service in &mut fleet {
        service
            .bind_registry_manifest(&manifest)
            .expect("bind live registry");
    }

    let payload_hash = hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"drill");
    let proposal = fleet[0]
        .create_protocol_proposal(&manifest, 1_203, payload_hash)
        .expect("create drill proposal");
    let contributions = fleet
        .iter_mut()
        .map(|service| {
            service
                .create_protocol_contribution(&manifest, &proposal)
                .expect("create drill contribution")
        })
        .collect::<Vec<_>>();
    let transcript =
        assemble_protocol_transcript(&manifest, proposal, contributions).expect("transcript");
    fleet[0]
        .commit_protocol_transcript(&transcript)
        .expect("commit drill transcript");
    assert_eq!(fleet[0].state.protocol_high_watermark, 1_203);
    assert_eq!(fleet[0].state.protocol_signer_high_watermark, 1_203);
    assert_eq!(fleet[0].state.contiguous_sequence, 1);

    let next_manifest = build_registry_binding_manifest(
        registry_root.clone(),
        manifest.validator_registry.clone(),
        manifest.validator_bindings.clone(),
        5,
        1_204,
    )
    .expect("build next registry binding");
    assert_ne!(
        manifest.trust_graph.trust_graph_root,
        next_manifest.trust_graph.trust_graph_root
    );
    let bind_error = fleet[0]
        .bind_registry_manifest(&next_manifest)
        .expect_err("committed history must reject an ordinary root change");
    assert!(bind_error.to_string().contains("registry-lineage-reset"));

    let mut transition = postfiat_types::CobaltGovernanceAuthorityTransitionV1 {
        schema: postfiat_types::COBALT_AUTHORITY_TRANSITION_SCHEMA_V1.to_string(),
        transition_id: String::new(),
        chain_id: fleet[0].state.identity.chain_id.clone(),
        genesis_hash: fleet[0].state.identity.genesis_hash.clone(),
        from_authority_mode: postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION,
        to_authority_mode: postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED,
        transition_kind: postfiat_types::COBALT_AUTHORITY_TRANSITION_ACTIVATE.to_string(),
        previous_transition_id: None,
        old_registry_root: registry_root.clone(),
        cobalt_lock_hash: "22".repeat(48),
        trust_graph_root: manifest.trust_graph.trust_graph_root.clone(),
        cobalt_registry_root: registry_root,
        amendment_sequence: 1,
        activation_height: 916,
        cobalt_protocol_version: 1,
        authority_scope: postfiat_types::COBALT_AUTHORITY_SCOPE_VALIDATOR_TRUST_V1.to_string(),
        validators: active_validators,
        approval_quorum: 5,
        approvals: Vec::new(),
    };
    transition.transition_id =
        crate::cobalt_handoff::cobalt_authority_transition_id(&transition).expect("transition id");

    let state_hash_before_rejection = fleet[0].state.state_hash.clone();
    let mut wrong_transition = transition.clone();
    wrong_transition.trust_graph_root = "33".repeat(48);
    wrong_transition.transition_id =
        crate::cobalt_handoff::cobalt_authority_transition_id(&wrong_transition)
            .expect("wrong transition id");
    let rejected_archive = root.join("rejected-archive");
    assert!(fleet[0]
        .reset_authority_lineage(&manifest, &wrong_transition, None, &rejected_archive)
        .is_err());
    assert_eq!(fleet[0].state.state_hash, state_hash_before_rejection);
    assert!(!rejected_archive.exists());

    let archive = root.join("accepted-archive");
    let receipt = fleet[0]
        .reset_authority_lineage(&manifest, &transition, None, &archive)
        .expect("reset authority lineage");
    assert_eq!(receipt.archived_protocol_high_watermark, 1_203);
    assert_eq!(receipt.archived_protocol_signer_high_watermark, 1_203);
    assert_eq!(receipt.archived_contiguous_sequence, 1);
    assert_eq!(receipt.ratification_anchor_sequence, None);
    assert_eq!(receipt.ratification_anchor_id, None);
    assert!(archive.join(STATE_FILE).is_file());
    assert!(archive.join(HISTORY_FILE).is_file());
    assert!(
        fs::metadata(archive.join(HISTORY_FILE))
            .expect("archive history metadata")
            .len()
            > 0
    );
    assert_eq!(fleet[0].state.protocol_high_watermark, 0);
    assert_eq!(fleet[0].state.protocol_signer_high_watermark, 0);
    assert_eq!(fleet[0].state.contiguous_sequence, 0);
    assert!(fleet[0].state.protocol_decisions.is_empty());
    assert!(fleet[0].history.is_empty());
    assert_eq!(
        fs::metadata(root.join("validator-0").join(HISTORY_FILE))
            .expect("current history metadata")
            .len(),
        0
    );
    drop(fleet);

    let state_path = root.join("validator-0").join(STATE_FILE);
    let state_before_guarded_open = fs::read(&state_path).expect("read reset state");
    let guarded = CobaltShadowService::open_for_authority_lineage_reset(root.join("validator-0"))
        .expect("guarded authority reset open");
    assert_eq!(guarded.status().ratification_anchor_sequence, None);
    drop(guarded);
    assert_eq!(
        fs::read(&state_path).expect("reread reset state"),
        state_before_guarded_open
    );

    let mut reopened =
        CobaltShadowService::open(root.join("validator-0")).expect("reopen reset service");
    reopened
        .create_protocol_proposal(
            &manifest,
            916,
            hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"live authority"),
        )
        .expect("lower first live authority round after reset");
    assert_eq!(reopened.state.protocol_signer_high_watermark, 916);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn registry_lineage_reset_preserves_global_dabc_anchor_with_local_history() {
    let root = test_dir("registry-lineage-anchor");
    let mut fleet = (0..6)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                identity(&format!("validator-{index}")),
                CobaltShadowLimits::default(),
            )
            .expect("initialize")
        })
        .collect::<Vec<_>>();
    let key_records = (0..6)
        .map(|index| {
            crate::create_validator_key_record(format!("validator-{index}")).expect("validator key")
        })
        .collect::<Vec<_>>();
    let registry = ValidatorRegistry {
        validators: key_records
            .iter()
            .map(|record| crate::ValidatorRegistryRecord {
                node_id: record.node_id.clone(),
                algorithm_id: record.algorithm_id.clone(),
                public_key_hex: record.public_key_hex.clone(),
            })
            .collect(),
    };
    let active_validators = key_records
        .iter()
        .map(|record| record.node_id.clone())
        .collect::<Vec<_>>();
    let registry_root =
        validator_registry_root(&registry, &active_validators).expect("registry root");
    let bindings = fleet
        .iter()
        .zip(&key_records)
        .map(|(service, record)| {
            let key_path = root.join(format!("{}.validator-keys.json", record.node_id));
            crate::write_validator_key_file(
                &key_path,
                &crate::ValidatorKeyFile {
                    validators: vec![record.clone()],
                },
            )
            .expect("write validator key");
            service
                .create_validator_binding(registry_root.clone(), &key_path)
                .expect("create validator binding")
        })
        .collect::<Vec<_>>();
    let previous_binding = build_registry_binding_manifest(
        registry_root.clone(),
        registry.clone(),
        bindings.clone(),
        5,
        1_203,
    )
    .expect("previous binding");
    let next_binding =
        build_registry_binding_manifest(registry_root.clone(), registry, bindings, 5, 1_204)
            .expect("next binding");
    assert_ne!(
        previous_binding.trust_graph.trust_graph_root,
        next_binding.trust_graph.trust_graph_root
    );
    for service in &mut fleet {
        service
            .bind_registry_manifest(&previous_binding)
            .expect("bind previous lineage");
    }

    let authorization = postfiat_types::SignedCobaltValidatorUpdateAuthorizationV1 {
        schema: postfiat_types::SIGNED_COBALT_VALIDATOR_UPDATE_AUTHORIZATION_SCHEMA_V1.to_string(),
        validator: "validator-0".to_string(),
        authority_transition_id: "11".repeat(48),
        parent_cobalt_lock_hash: "22".repeat(48),
        amendment_sequence: 1,
        proposal_slot: 1_204,
        expires_at_height: 1_204,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        signature_hex: "00".to_string(),
    };
    let mut update = postfiat_types::ValidatorRegistryUpdateRecord {
        schema: "postfiat-validator-registry-update-v1".to_string(),
        update_id: "33".repeat(48),
        chain_id: fleet[0].state.identity.chain_id.clone(),
        genesis_hash: fleet[0].state.identity.genesis_hash.clone(),
        protocol_version: fleet[0].state.identity.protocol_version,
        instance_id: "34".repeat(48),
        proposal_id: "35".repeat(48),
        certificate_id: "36".repeat(48),
        proposer: "validator-0".to_string(),
        validators: active_validators.clone(),
        quorum: 5,
        support: active_validators[..5].to_vec(),
        votes: Vec::new(),
        signed_authorizations: Vec::new(),
        cobalt_authorizations: vec![authorization; 5],
        cobalt_decision_certificate: None,
        activation_height: 1_204,
        previous_registry_root: previous_binding.registry_root.clone(),
        new_registry_root: next_binding.registry_root.clone(),
        previous_trust_graph_root: Some(previous_binding.trust_graph.trust_graph_root.clone()),
        new_trust_graph_root: Some(next_binding.trust_graph.trust_graph_root.clone()),
        trust_graph_transition_id: Some("37".repeat(48)),
        previous_validators: active_validators.clone(),
        new_validators: active_validators.clone(),
        operation: postfiat_consensus_cobalt::VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
        subject_node_id: "validator-5".to_string(),
        previous_record: None,
        new_record: None,
    };
    let payload_hash =
        crate::cobalt_authority_certificate::cobalt_validator_update_payload_hash(&update)
            .expect("update payload hash");
    let first_proposal = fleet[0]
        .create_protocol_proposal(&previous_binding, 1_203, payload_hash)
        .expect("first proposal");
    let first_contributions = fleet
        .iter_mut()
        .map(|service| {
            service
                .create_protocol_contribution(&previous_binding, &first_proposal)
                .expect("first contribution")
        })
        .collect::<Vec<_>>();
    let first =
        assemble_protocol_transcript(&previous_binding, first_proposal, first_contributions)
            .expect("first transcript");
    for service in &mut fleet {
        service
            .commit_protocol_transcript(&first)
            .expect("commit first transcript");
    }
    assert_eq!(first.ratification.sequence, 1);

    update.cobalt_decision_certificate = Some(
        crate::cobalt_authority_certificate::compact_cobalt_validator_update_decision_certificate(
            postfiat_types::CobaltValidatorUpdateDecisionCertificateV1 {
                schema: postfiat_types::COBALT_VALIDATOR_UPDATE_DECISION_CERTIFICATE_SCHEMA_V1
                    .to_string(),
                registry_binding: serde_json::to_value(&previous_binding)
                    .expect("serialize previous binding"),
                protocol_transcript: serde_json::to_value(&first)
                    .expect("serialize first transcript"),
            },
        )
        .expect("compact decision certificate"),
    );
    for (index, service) in fleet.iter_mut().enumerate() {
        let receipt = service
            .reset_registry_lineage(
                &previous_binding,
                &next_binding,
                &update,
                root.join(format!("archive-{index}")),
            )
            .expect("reset registry lineage");
        assert_eq!(receipt.ratification_anchor_sequence, 1);
        assert_eq!(
            receipt.ratification_anchor_id,
            first.ratification.ratification_id
        );
        assert_eq!(service.status().ratification_anchor_sequence, Some(1));
        assert_eq!(service.status().contiguous_sequence, 0);
    }

    // Model the already-live h917 sidecars: the next binding is persisted,
    // the old implementation cleared history, and it did not retain the
    // ratification anchor. The guarded reset opener must reconstruct the
    // previous lineage from the live update before the next decision.
    fleet[5].state.ratification_anchor = None;
    fleet[5]
        .persist_state()
        .expect("persist legacy post-reset state");
    let _legacy_service = fleet.remove(5);
    let mut migrated = CobaltShadowService::open_for_registry_lineage_reset(
        root.join("validator-5"),
        &previous_binding,
        &next_binding,
    )
    .expect("open persisted-next lineage migration");
    let migration_receipt = migrated
        .reset_registry_lineage(
            &previous_binding,
            &next_binding,
            &update,
            root.join("archive-legacy-next"),
        )
        .expect("restore legacy lineage anchor");
    assert_eq!(migration_receipt.ratification_anchor_sequence, 1);
    assert_eq!(migrated.status().ratification_anchor_sequence, Some(1));
    fleet.push(migrated);

    let second_proposal = fleet[0]
        .create_protocol_proposal(
            &next_binding,
            1_204,
            hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"second update"),
        )
        .expect("second proposal");
    let second_contributions = fleet
        .iter_mut()
        .map(|service| {
            service
                .create_protocol_contribution_at_activation_height(
                    &next_binding,
                    &second_proposal,
                    1_300,
                )
                .expect("second contribution")
        })
        .collect::<Vec<_>>();
    let second = assemble_protocol_transcript_at_activation_height_extending(
        &next_binding,
        second_proposal,
        second_contributions,
        Some(&first.ratification),
        1_300,
    )
    .expect("second transcript across a chain-height gap");
    assert_eq!(second.ratification.sequence, 2);
    assert_eq!(second.ratification.amendment_slot, 1_204);
    assert_eq!(second.ratification.activation_height, 1_300);

    let mut skipped = second.clone();
    skipped.ratification.sequence = 3;
    skipped.ratification.ratification_id = postfiat_consensus_cobalt::dabc_ratification_id(
        &fleet[5].cobalt_domain(),
        &skipped.ratification,
    )
    .expect("recompute skipped id");
    assert!(fleet[5]
        .commit_protocol_transcript(&skipped)
        .expect_err("global sequence gap must fail")
        .to_string()
        .contains("catch_up_required"));

    for service in &mut fleet {
        service
            .commit_protocol_transcript(&second)
            .expect("commit cross-root transcript");
        assert_eq!(service.state.contiguous_sequence, 1);
        assert_eq!(service.history[0].sequence, 1);
        assert_eq!(service.history[0].transcript.ratification.sequence, 2);
    }
    drop(fleet);

    let mut reopened =
        CobaltShadowService::open(root.join("validator-0")).expect("reopen anchored lineage");
    assert_eq!(reopened.status().ratification_anchor_sequence, Some(1));
    assert_eq!(reopened.replay_protocol_state().expect("replay").len(), 1);

    let return_binding = build_registry_binding_manifest(
        next_binding.registry_root.clone(),
        next_binding.validator_registry.clone(),
        next_binding.validator_bindings.clone(),
        5,
        1_400,
    )
    .expect("build reactivation binding");
    let mut return_transition = postfiat_types::CobaltGovernanceAuthorityTransitionV1 {
        schema: postfiat_types::COBALT_AUTHORITY_TRANSITION_SCHEMA_V1.to_string(),
        transition_id: String::new(),
        chain_id: reopened.state.identity.chain_id.clone(),
        genesis_hash: reopened.state.identity.genesis_hash.clone(),
        from_authority_mode: postfiat_types::GOVERNANCE_AUTHORITY_MODE_FOUNDATION,
        to_authority_mode: postfiat_types::GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED,
        transition_kind: postfiat_types::COBALT_AUTHORITY_TRANSITION_ACTIVATE.to_string(),
        previous_transition_id: Some("55".repeat(48)),
        old_registry_root: return_binding.registry_root.clone(),
        cobalt_lock_hash: "66".repeat(48),
        trust_graph_root: return_binding.trust_graph.trust_graph_root.clone(),
        cobalt_registry_root: return_binding.registry_root.clone(),
        amendment_sequence: 4,
        activation_height: return_binding.trust_graph.activation_height,
        cobalt_protocol_version: 4,
        authority_scope: postfiat_types::COBALT_AUTHORITY_SCOPE_VALIDATOR_TRUST_V1.to_string(),
        validators: return_binding.active_validators.clone(),
        approval_quorum: 5,
        approvals: Vec::new(),
    };
    return_transition.transition_id =
        crate::cobalt_handoff::cobalt_authority_transition_id(&return_transition)
            .expect("return transition id");
    let state_before_wrong_anchor = reopened.state.state_hash.clone();
    let wrong_anchor_archive = root.join("authority-return-wrong-anchor");
    assert!(reopened
        .reset_authority_lineage(
            &return_binding,
            &return_transition,
            Some(&first.ratification),
            &wrong_anchor_archive,
        )
        .expect_err("return must bind the latest committed governance anchor")
        .to_string()
        .contains("committed governance history"));
    assert_eq!(reopened.state.state_hash, state_before_wrong_anchor);
    assert!(!wrong_anchor_archive.exists());

    let return_receipt = reopened
        .reset_authority_lineage(
            &return_binding,
            &return_transition,
            Some(&second.ratification),
            root.join("authority-return-archive"),
        )
        .expect("reset reactivated authority lineage");
    assert_eq!(return_receipt.ratification_anchor_sequence, Some(2));
    assert_eq!(
        return_receipt.ratification_anchor_id.as_deref(),
        Some(second.ratification.ratification_id.as_str())
    );
    assert_eq!(reopened.status().ratification_anchor_sequence, Some(2));
    assert_eq!(reopened.status().contiguous_sequence, 0);
    assert_eq!(
        reopened.status().trust_graph_root,
        return_binding.trust_graph.trust_graph_root
    );
    assert!(reopened
        .replay_protocol_state()
        .expect("replay return")
        .is_empty());

    let tampered_dir = root.join("validator-1");
    let mut tampered = CobaltShadowService::open(&tampered_dir).expect("open for tamper");
    tampered
        .state
        .ratification_anchor
        .as_mut()
        .expect("anchor")
        .ratification_id = "44".repeat(48);
    tampered
        .persist_state()
        .expect("persist signed semantic tamper");
    drop(tampered);
    assert!(CobaltShadowService::open(tampered_dir).is_err());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn signed_protocol_transcript_converges_and_replays_after_restart() {
    let root = test_dir("signed-protocol");
    let mut fleet = (0..3)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                identity(&format!("validator-{index}")),
                CobaltShadowLimits::default(),
            )
            .expect("initialize")
        })
        .collect::<Vec<_>>();
    let transcript = build_signed_protocol_transcript(
        &mut fleet,
        7,
        hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"ratify"),
    )
    .expect("build signed transcript");
    let decisions = fleet
        .iter_mut()
        .map(|service| {
            service
                .commit_protocol_transcript(&transcript)
                .expect("commit transcript")
        })
        .collect::<Vec<_>>();
    assert!(decisions.windows(2).all(|pair| pair[0] == pair[1]));
    assert_eq!(decisions[0].signed_message_count, 25);
    assert!(fleet.iter().all(|service| {
        !service.status().live_authority
            && !service.status().controls_block_consensus
            && service.status().protocol_decision_count == 1
    }));
    drop(fleet);
    let mut restarted =
        CobaltShadowService::open(root.join("validator-1")).expect("restart service");
    let replay = restarted.replay_protocol_state().expect("replay state");
    assert_eq!(replay, vec![decisions[1].clone()]);
    assert_eq!(restarted.status().ratification_lock_count, 1);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn v2_state_migrates_without_relabeling_legacy_decisions() {
    let root = test_dir("v2-migration");
    let service = CobaltShadowService::initialize(
        root.join("validator-0"),
        identity("validator-0"),
        CobaltShadowLimits::default(),
    )
    .expect("initialize");
    let mut value = serde_json::to_value(&service.state).expect("serialize v3 state");
    let object = value.as_object_mut().expect("state object");
    object.insert(
        "schema".to_string(),
        serde_json::Value::String(COBALT_SHADOW_STATE_V2_SCHEMA.to_string()),
    );
    for field in [
        "protocol_signer_high_watermark",
        "contiguous_sequence",
        "history_anchor_round",
        "history_head",
        "v2_migration",
    ] {
        object.remove(field);
    }
    let limits = object
        .get_mut("limits")
        .and_then(serde_json::Value::as_object_mut)
        .expect("limits object");
    limits.remove("max_history_entries");
    limits.remove("max_history_range_entries");
    let mut legacy: CobaltShadowStateV2 =
        serde_json::from_value(value).expect("deserialize v2 state");
    let legacy_decision = CobaltShadowProtocolDecisionV1 {
        round: 40,
        transcript_hash: "10".repeat(48),
        payload_hash: "11".repeat(48),
        agreement_id: "12".repeat(48),
        output_candidate_id: "13".repeat(48),
        ratification_id: "14".repeat(48),
        registry_root: "15".repeat(48),
        trust_graph_root: "16".repeat(48),
        signed_message_count: 49,
    };
    legacy
        .ratification_locks
        .insert(40, legacy_decision.ratification_id.clone());
    legacy.protocol_decisions.insert(40, legacy_decision);
    legacy.protocol_high_watermark = 40;
    legacy.state_hash.clear();
    legacy.state_signature_hex.clear();
    let canonical = serde_json::to_vec(&legacy).expect("canonical v2 state");
    legacy.state_hash = hash_hex("postfiat.cobalt.shadow.state.v1", &canonical);
    legacy.state_signature_hex = service
        .sign_bytes(legacy.state_hash.as_bytes(), STATE_SIGNATURE_CONTEXT)
        .expect("sign v2 state");
    let encoded = serde_json::to_vec_pretty(&legacy).expect("encode v2 state");
    postfiat_storage::atomic_write(root.join("validator-0").join(STATE_FILE), encoded)
        .expect("write v2 state");
    fs::remove_file(root.join("validator-0").join(HISTORY_FILE)).expect("remove v3 journal");
    drop(service);

    let migrated = CobaltShadowService::open(root.join("validator-0")).expect("migrate v2 state");
    assert_eq!(migrated.state.schema, COBALT_SHADOW_STATE_SCHEMA);
    assert_eq!(migrated.state.contiguous_sequence, 0);
    assert!(migrated.history.is_empty());
    assert!(migrated.state.protocol_decisions.is_empty());
    assert!(migrated.state.ratification_locks.is_empty());
    assert_eq!(migrated.state.protocol_signer_high_watermark, 40);
    let receipt = migrated.state.v2_migration.expect("migration receipt");
    assert_eq!(receipt.legacy_protocol_decisions.len(), 1);
    assert_eq!(receipt.legacy_protocol_high_watermark, 40);
    assert!(!receipt.receipt_id.is_empty());
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn signed_history_catch_up_refuses_gap_then_converges() {
    let root = test_dir("signed-history-catch-up");
    let mut fleet = (0..3)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                identity(&format!("validator-{index}")),
                CobaltShadowLimits::default(),
            )
            .expect("initialize")
        })
        .collect::<Vec<_>>();
    let first = build_signed_protocol_transcript(
        &mut fleet,
        20,
        hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"first"),
    )
    .expect("build first transcript");
    fleet[0]
        .commit_protocol_transcript(&first)
        .expect("validator 0 commits first");
    fleet[1]
        .commit_protocol_transcript(&first)
        .expect("validator 1 commits first");
    let second = build_signed_protocol_transcript_extending(
        &mut fleet,
        21,
        hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"second"),
        Some(&first.ratification),
    )
    .expect("build second transcript");
    fleet[0]
        .commit_protocol_transcript(&second)
        .expect("validator 0 commits second");
    fleet[1]
        .commit_protocol_transcript(&second)
        .expect("validator 1 commits second");
    let gap = fleet[2]
        .commit_protocol_transcript(&second)
        .expect_err("validator 2 must refuse a history gap");
    assert!(gap.to_string().contains("catch_up_required"));
    assert_eq!(fleet[2].state.contiguous_sequence, 0);
    assert_eq!(fleet[2].state.protocol_decisions.len(), 0);
    assert_eq!(fleet[2].status().missing_ranges.len(), 1);
    let range = fleet[0]
        .signed_history_range(1, 1)
        .expect("export signed first entry");
    fleet[2]
        .catch_up_signed_history(&range)
        .expect("catch up signed first entry");
    let recovered = fleet[2]
        .commit_protocol_transcript(&second)
        .expect("commit second after catch-up");
    assert_eq!(
        recovered.decision_id,
        fleet[0].history[1].decision.decision_id
    );
    assert_eq!(fleet[2].state.contiguous_sequence, 2);
    assert_eq!(fleet[2].state.history_head, fleet[0].state.history_head);
    assert_eq!(
        fleet[2].state.governance_digest,
        fleet[0].state.governance_digest
    );
    drop(fleet);
    let restarted =
        CobaltShadowService::open(root.join("validator-2")).expect("restart caught-up node");
    assert_eq!(restarted.state.contiguous_sequence, 2);
    assert_eq!(restarted.history.len(), 2);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn catch_up_rejects_malformed_batches_without_durable_mutation() {
    let root = test_dir("catch-up-rejections");
    let mut fleet = (0..3)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                identity(&format!("validator-{index}")),
                CobaltShadowLimits::default(),
            )
            .expect("initialize")
        })
        .collect::<Vec<_>>();
    let first = build_signed_protocol_transcript(
        &mut fleet,
        50,
        hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"reject-first"),
    )
    .expect("first transcript");
    let second = build_signed_protocol_transcript_extending(
        &mut fleet,
        51,
        hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"reject-second"),
        Some(&first.ratification),
    )
    .expect("second transcript");
    fleet[0]
        .commit_protocol_transcript(&first)
        .expect("commit first");
    fleet[0]
        .commit_protocol_transcript(&second)
        .expect("commit second");
    let valid = fleet[0].history_range(1, 2).expect("history range");
    let target_journal = root.join("validator-2").join(HISTORY_FILE);
    let initial_journal_bytes = fs::metadata(&target_journal).expect("target journal").len();
    let initial_state_hash = fleet[2].state.state_hash.clone();

    let mut announced_latest = second.clone();
    announced_latest.ratification.sequence = 3;
    assert!(fleet[2]
        .commit_protocol_transcript(&announced_latest)
        .expect_err("future transcript must require catch-up")
        .to_string()
        .contains("catch_up_required"));
    let omitted = fleet[0].history_range(1, 1).expect("omitted range");
    assert!(fleet[2]
        .catch_up_history(&omitted)
        .expect_err("known latest update cannot be omitted")
        .to_string()
        .contains("omits required latest update"));

    let mut reordered = valid.clone();
    reordered.entries.swap(0, 1);
    reordered.range_hash = history_range_hash(&reordered).expect("reordered hash");
    assert!(fleet[2].catch_up_history(&reordered).is_err());

    let mut wrong_root = valid.clone();
    wrong_root.trust_graph_root = "ff".repeat(48);
    wrong_root.range_hash = history_range_hash(&wrong_root).expect("wrong-root hash");
    assert!(fleet[2].catch_up_history(&wrong_root).is_err());

    let mut conflicting_parent = valid.clone();
    conflicting_parent.entries[0].parent_entry_hash = "ee".repeat(48);
    conflicting_parent.entries[0].entry_hash =
        history_entry_hash(&conflicting_parent.entries[0]).expect("entry hash");
    conflicting_parent.range_hash =
        history_range_hash(&conflicting_parent).expect("conflicting-parent hash");
    assert!(fleet[2].catch_up_history(&conflicting_parent).is_err());

    let mut partially_valid = valid.clone();
    partially_valid.entries[1].transcript.rbc_echoes[0]
        .signature_hex
        .replace_range(0..2, "00");
    partially_valid.entries[1].transcript_hash = hash_serialized(
        "postfiat.cobalt.shadow.protocol-transcript.v1",
        &partially_valid.entries[1].transcript,
    )
    .expect("tampered transcript hash");
    partially_valid.entries[1].decision.transcript_hash =
        partially_valid.entries[1].transcript_hash.clone();
    partially_valid.entries[1].entry_hash =
        history_entry_hash(&partially_valid.entries[1]).expect("tampered entry hash");
    partially_valid.range_hash = history_range_hash(&partially_valid).expect("partial range hash");
    assert!(fleet[2].catch_up_history(&partially_valid).is_err());

    let mut duplicate = valid.clone();
    duplicate.entries[1] = duplicate.entries[0].clone();
    duplicate.range_hash = history_range_hash(&duplicate).expect("duplicate hash");
    assert!(fleet[2].catch_up_history(&duplicate).is_err());

    fleet[2].state.limits.max_history_range_entries = 1;
    assert!(fleet[2].catch_up_history(&valid).is_err());
    fleet[2].state.limits.max_history_range_entries = default_max_history_range_entries();
    assert_eq!(fleet[2].state.state_hash, initial_state_hash);
    assert_eq!(fleet[2].state.contiguous_sequence, 0);
    assert_eq!(fleet[2].state.protocol_decisions.len(), 0);
    assert_eq!(
        fs::metadata(&target_journal).expect("target journal").len(),
        initial_journal_bytes
    );

    OpenOptions::new()
        .append(true)
        .open(&target_journal)
        .and_then(|mut file| file.write_all(b"{"))
        .expect("append truncated record");
    drop(fleet);
    let truncated = CobaltShadowService::open(root.join("validator-2"))
        .err()
        .expect("truncated journal must fail closed");
    assert!(truncated.to_string().contains("truncated"));
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn protocol_journal_recovers_journal_before_state_and_stable_state_restart() {
    let root = test_dir("journal-crash-recovery");
    let mut fleet = (0..3)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                identity(&format!("validator-{index}")),
                CobaltShadowLimits::default(),
            )
            .expect("initialize")
        })
        .collect::<Vec<_>>();
    let transcript = build_signed_protocol_transcript(
        &mut fleet,
        30,
        hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"crash-points"),
    )
    .expect("build transcript");

    let before_journal_bytes = fs::metadata(root.join("validator-1").join(HISTORY_FILE))
        .expect("journal metadata")
        .len();
    let (decision, _) = fleet[1]
        .validate_protocol_transcript(&transcript, None)
        .expect("validate before journal");
    let pending_entry = fleet[1]
        .build_history_entry(transcript.clone(), decision)
        .expect("build pending entry");
    assert_eq!(
        fs::metadata(root.join("validator-1").join(HISTORY_FILE))
            .expect("journal metadata")
            .len(),
        before_journal_bytes
    );
    assert_eq!(fleet[1].state.contiguous_sequence, 0);

    drop(fleet.remove(1));
    let before_journal_restart =
        CobaltShadowService::open(root.join("validator-1")).expect("restart before journal write");
    assert_eq!(before_journal_restart.state.contiguous_sequence, 0);
    assert!(before_journal_restart.history.is_empty());
    assert_eq!(
        fs::metadata(root.join("validator-1").join(HISTORY_FILE))
            .expect("journal metadata")
            .len(),
        before_journal_bytes
    );

    fleet[0]
        .commit_protocol_transcript(&transcript)
        .expect("normal commit");
    before_journal_restart
        .append_history_entries(std::slice::from_ref(&pending_entry))
        .expect("journal append before simulated crash");
    drop(before_journal_restart);
    let recovered = CobaltShadowService::open(root.join("validator-1")).expect("reconcile journal");
    assert_eq!(recovered.state.contiguous_sequence, 1);
    assert_eq!(recovered.state.history_head, Some(pending_entry.entry_hash));

    let last = fleet.last_mut().expect("remaining validator");
    last.commit_protocol_transcript(&transcript)
        .expect("normal commit through state persistence");
    drop(fleet);
    let stable = CobaltShadowService::open(root.join("validator-2")).expect("restart stable state");
    assert_eq!(stable.state.contiguous_sequence, 1);
    assert_eq!(stable.history.len(), 1);
    fs::remove_dir_all(root).expect("cleanup");
}

#[test]
fn signed_protocol_transcript_rejects_tamper_and_wrong_root() {
    let root = test_dir("signed-protocol-tamper");
    let mut fleet = (0..3)
        .map(|index| {
            CobaltShadowService::initialize(
                root.join(format!("validator-{index}")),
                identity(&format!("validator-{index}")),
                CobaltShadowLimits::default(),
            )
            .expect("initialize")
        })
        .collect::<Vec<_>>();
    let transcript = build_signed_protocol_transcript(
        &mut fleet,
        9,
        hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"tamper"),
    )
    .expect("build signed transcript");
    let mut tampered = transcript.clone();
    tampered.rbc_echoes[0]
        .signature_hex
        .replace_range(0..2, "00");
    assert!(fleet[0].commit_protocol_transcript(&tampered).is_err());
    let valid_old = transcript.clone();
    let mut wrong_root = transcript;
    wrong_root.registry_root = "ff".repeat(48);
    assert!(fleet[0].commit_protocol_transcript(&wrong_root).is_err());
    assert_eq!(fleet[0].status().protocol_decision_count, 0);
    let newer = build_signed_protocol_transcript(
        &mut fleet,
        10,
        hash_hex("postfiat.cobalt.shadow.test.payload.v1", b"newer"),
    )
    .expect("build newer transcript");
    fleet[0]
        .commit_protocol_transcript(&newer)
        .expect("commit newer transcript");
    assert!(fleet[0].commit_protocol_transcript(&valid_old).is_err());
    assert_eq!(fleet[0].status().protocol_decision_count, 1);
    fs::remove_dir_all(root).expect("cleanup");
}
