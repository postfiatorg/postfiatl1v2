// `owned_fence_status`: a validator signs "anchored" or "cancelled" only from
// certified-block state, and "not terminal at H" otherwise.

fn fence_status_at(dir: &Path, index: usize, lock_id: &str) -> postfiat_types::FastPayFenceStatusV1 {
    owned_fence_status_v3(
        NodeOptions {
            data_dir: dir.to_path_buf(),
        },
        &format!("validator-{index}"),
        lock_id,
    )
    .expect("fence status")
}

/// Not terminal at `observed_height`, and signed by a committee member.
fn assert_not_terminal(
    dir: &Path,
    status: &postfiat_types::FastPayFenceStatusV1,
    observed_height: u64,
) {
    assert_eq!(status.terminal_answer(), (None, None, None, None));
    assert_eq!(status.observed_height, observed_height);
    let ledger = NodeStore::new(dir).read_ledger().unwrap();
    let key = ledger
        .fastpay_recovery_committees
        .iter()
        .find(|committee| committee.committee_epoch == status.committee_epoch)
        .and_then(|committee| {
            committee
                .validators
                .iter()
                .find(|member| member.validator_id == status.validator_id)
        })
        .map(|member| member.public_key_hex.clone())
        .expect("committee member");
    assert!(postfiat_execution::verify_fastpay_fence_status_v1(
        status, &key
    ));
}

fn active_fastpay_unwrap(
    dirs: &[PathBuf],
    owner: &postfiat_crypto_provider::MlDsa65KeyPair,
    transfer: &postfiat_types::OwnedTransferCertificateV3,
) -> postfiat_types::OwnedUnwrapCertificateV3 {
    let mut order = postfiat_types::OwnedUnwrapOrderV3 {
        domain: transfer.order.domain.clone(),
        recovery: transfer.order.recovery.clone(),
        inputs: transfer.order.inputs.clone(),
        to_address: "pffencestatusunwrap000000000000000000001".into(),
        amount: 99,
        fee: 1,
        asset: "PFT".into(),
        nonce: 44,
        memos: Vec::new(),
    };
    order.recovery.lock_id = postfiat_types::fastpay_unwrap_lock_id_v1(&order);
    let bytes = postfiat_execution::owned_unwrap_v3_signing_bytes(&order);
    let owner_signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &owner.private_key,
            &bytes,
            postfiat_execution::OWNED_UNWRAP_CONTEXT_V3,
        )
        .unwrap(),
    );
    let keys = read_validator_key_file(&dirs[0].join(VALIDATOR_KEYS_FILE)).unwrap();
    let votes = keys
        .validators
        .iter()
        .take(5)
        .map(|key| postfiat_types::OwnedUnwrapVote {
            validator_id: key.node_id.clone(),
            signature_hex: bytes_to_hex(
                &ml_dsa_65_sign_with_context(
                    &hex_to_bytes(&key.private_key_hex).unwrap(),
                    &bytes,
                    postfiat_execution::OWNED_UNWRAP_CONTEXT_V3,
                )
                .unwrap(),
            ),
        })
        .collect();
    postfiat_types::OwnedUnwrapCertificateV3 {
        order,
        owner_pubkey_hex: bytes_to_hex(&owner.public_key),
        owner_signature_hex,
        votes,
    }
}

#[test]
fn owned_fence_status_anchors_an_unwrap_only_at_its_certified_block_and_after_restart() {
    let (root, dirs, owner) = active_fastpay_fixture();
    let transfer = active_fastpay_transfer(&dirs, &owner);
    let unwrap = active_fastpay_unwrap(&dirs, &owner, &transfer);
    let lock_id = unwrap.order.recovery.lock_id.clone();
    let json = serde_json::to_string(&unwrap).unwrap();
    let mut acks = Vec::new();
    for (index, dir) in dirs.iter().take(5).enumerate() {
        // Applied speculatively and acknowledged: still not terminal.
        let ack: postfiat_types::FastPayApplyAckV1 = serde_json::from_str(
            &owned_unwrap_apply_v3(
                NodeOptions {
                    data_dir: dir.clone(),
                },
                &json,
                &format!("validator-{index}"),
            )
            .unwrap(),
        )
        .unwrap();
        acks.push(ack);
        assert_not_terminal(dir, &fence_status_at(dir, index, &lock_id), 1);
    }
    let batch_file = root.join("fence-status-anchor-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: dirs[0].clone(),
        key_file: None,
        to: "pffastpayanchor00000000000000000000000001".into(),
        amount: 1,
        batch_file: batch_file.clone(),
    })
    .unwrap();
    let (cert_file, proposal) = active_fastpay_certify(&dirs[0], &root, batch_file.clone());
    assert_eq!(proposal.fastpay_pre_state_effects.len(), 1);
    // A certified proposal that is not yet this node's committed block does
    // not anchor anything here.
    for (index, dir) in dirs.iter().enumerate().take(5) {
        assert_not_terminal(dir, &fence_status_at(dir, index, &lock_id), 1);
    }
    for dir in &dirs {
        apply_batch(ApplyBatchOptions {
            data_dir: dir.clone(),
            batch_file: batch_file.clone(),
            certificate_file: Some(cert_file.clone()),
        })
        .unwrap();
    }
    let committee = NodeStore::new(&dirs[0]).read_ledger().unwrap().fastpay_recovery_committees[0].clone();
    let statuses = dirs
        .iter()
        .take(5)
        .enumerate()
        .map(|(index, dir)| fence_status_at(dir, index, &lock_id))
        .collect::<Vec<_>>();
    for status in &statuses {
        assert_eq!(status.anchored_height, Some(2));
        assert_eq!(status.observed_height, 2);
        assert_eq!(status.operation, Some(postfiat_types::FastPayOperationKindV1::Unwrap));
        // The fence digest is the apply acknowledgement's terminal state.
        assert_eq!(status.fence_digest.as_deref(), Some(acks[0].terminal_state_digest.as_str()));
        assert!(matches!(
            status.decision,
            Some(postfiat_types::FastPayRecoveryDecisionV1::Confirmed { ref certificate_digest, .. })
                if *certificate_digest == acks[0].certificate_digest
        ));
    }
    let proved = postfiat_execution::verify_fastpay_fence_status_quorum_ml_dsa_v1(
        &statuses,
        &committee,
        &lock_id,
    )
    .expect("a quorum of distinct members proves the anchored unwrap");
    assert_eq!((proved.anchored_height, proved.signers.len()), (Some(2), 5));
    // A restart (no cached answer) gives the same answer at the same tip.
    clear_fastpay_fence_status_cache();
    for (index, dir) in dirs.iter().take(5).enumerate() {
        assert_eq!(
            fence_status_at(dir, index, &lock_id).terminal_answer(),
            statuses[index].terminal_answer()
        );
    }
    // Repeated reads at one tip are served from the cache: no new signature.
    assert_eq!(fence_status_at(&dirs[0], 0, &lock_id), fence_status_at(&dirs[0], 0, &lock_id));
    // A registered validator outside the committee (validator-5 here) gets a
    // typed refusal, not an unsigned answer; an unknown one fails closed.
    for (validator, code) in [
        ("validator-5", "fastpay_fence_status_not_a_signer"),
        ("validator-9", "not in the local validator registry"),
    ] {
        let refused = owned_fence_status_v3(
            NodeOptions {
                data_dir: dirs[5].clone(),
            },
            validator,
            &lock_id,
        )
        .unwrap_err();
        assert!(refused.to_string().contains(code), "{refused}");
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn owned_fence_status_never_reports_a_rolled_back_speculative_effect_anchored() {
    let (root, dirs, owner) = active_fastpay_fixture();
    let certificate = active_fastpay_transfer(&dirs, &owner);
    let lock_id = certificate.order.recovery.lock_id.clone();
    let batch_file = root.join("omitting-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: dirs[0].clone(),
        key_file: None,
        to: "pffastpayanchor00000000000000000000000001".into(),
        amount: 1,
        batch_file: batch_file.clone(),
    })
    .unwrap();
    let (cert_file, proposal) = active_fastpay_certify(&dirs[5], &root, batch_file.clone());
    assert!(proposal.fastpay_pre_state_effects.is_empty());
    owned_apply_v3(
        NodeOptions {
            data_dir: dirs[0].clone(),
        },
        &serde_json::to_string(&certificate).unwrap(),
        "validator-0",
    )
    .unwrap();
    // Speculatively applied on node 0 only.
    assert_not_terminal(&dirs[0], &fence_status_at(&dirs[0], 0, &lock_id), 1);
    for (index, dir) in dirs.iter().enumerate() {
        apply_batch(ApplyBatchOptions {
            data_dir: dir.clone(),
            batch_file: batch_file.clone(),
            certificate_file: Some(cert_file.clone()),
        })
        .unwrap();
        // The certified block omitted the effect and node 0 rolled it back.
        if index < 5 {
            assert_not_terminal(dir, &fence_status_at(dir, index, &lock_id), 2);
        }
    }
    clear_fastpay_fence_status_cache();
    assert_not_terminal(&dirs[0], &fence_status_at(&dirs[0], 0, &lock_id), 2);
    std::fs::remove_dir_all(root).unwrap();
}
