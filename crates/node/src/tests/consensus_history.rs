use super::*;

pub(super) fn dummy_block_record(height: u64) -> BlockRecord {
    BlockRecord {
        header: BlockHeader {
            height,
            view: 0,
            parent_hash: format!("parent-{height}"),
            proposer: "validator-0".to_string(),
            batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
            batch_id: format!("batch-{height}"),
            state_root: format!("state-root-{height}"),
            bridge_exit_root: None,
            receipt_count: 0,
            certificate_id: format!("certificate-{height}"),
            certificate: BlockCertificate {
                validators: vec!["validator-0".to_string()],
                quorum: 1,
                registry_root: String::new(),
                votes: Vec::new(),
            },
            consensus_v2_commit: None,
            block_hash: format!("block-{height}"),
        },
        receipt_ids: Vec::new(),
        fastpay_pre_state_effects: Vec::new(),
    }
}

#[test]
fn archived_transparent_replay_accepts_wan_devnet_legacy_batch_id_only() {
    let genesis =
        Genesis::try_new_with_validator_count("postfiat-wan-devnet".to_string(), 1)
            .expect("wan genesis");
    let mut block = dummy_block_record(9);
    block.header.batch_id = "ff".repeat(48);
    block.header.receipt_count = 0;
    let legacy_batch = TransactionBatch::new(block.header.batch_id.clone(), Vec::new());
    let payload_json = serde_json::to_string(&legacy_batch).expect("legacy payload");
    let archive_entry = BatchArchiveEntry {
        batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
        batch_id: block.header.batch_id.clone(),
        payload_hash: batch_archive_payload_hash(
            &genesis,
            BATCH_KIND_TRANSPARENT,
            &block.header.batch_id,
            &payload_json,
        )
        .expect("legacy payload hash"),
        payload_json,
    };
    verify_archived_payload(&genesis, &block, &archive_entry)
        .expect("WAN devnet legacy transparent self-id verifies");

    let non_wan_genesis =
        Genesis::try_new_with_validator_count("postfiat-local".to_string(), 1)
            .expect("local genesis");
    let non_wan_error =
        verify_archived_payload(&non_wan_genesis, &block, &archive_entry)
            .expect_err("legacy transparent id is not accepted outside WAN devnet window");
    assert!(
        non_wan_error
            .to_string()
            .contains("archived transparent payload invalid"),
        "{non_wan_error}"
    );

    let mut block_after_legacy_window = block.clone();
    block_after_legacy_window.header.height = 10;
    let height_error =
        verify_archived_payload(&genesis, &block_after_legacy_window, &archive_entry)
            .expect_err("legacy transparent id is not accepted after WAN devnet window");
    assert!(
        height_error
            .to_string()
            .contains("archived transparent payload invalid"),
        "{height_error}"
    );

    let tampered_batch = TransactionBatch::new("aa".repeat(48), Vec::new());
    let tampered_payload_json =
        serde_json::to_string(&tampered_batch).expect("tampered payload");
    let tampered_archive_entry = BatchArchiveEntry {
        batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
        batch_id: block.header.batch_id.clone(),
        payload_hash: batch_archive_payload_hash(
            &genesis,
            BATCH_KIND_TRANSPARENT,
            &block.header.batch_id,
            &tampered_payload_json,
        )
        .expect("tampered payload hash"),
        payload_json: tampered_payload_json,
    };
    let tampered_error =
        verify_archived_payload(&genesis, &block, &tampered_archive_entry)
            .expect_err("payload self-id must still match block id");
    assert!(
        tampered_error
            .to_string()
            .contains("archived payload batch id mismatch"),
        "{tampered_error}"
    );
}

#[test]
fn wan_devnet_legacy_nav_profile_id_replays_optional_field_schema() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("testdata/wan-devnet-catchup-block-3/batch.json");
    let batch_json = fs::read_to_string(&fixture).unwrap_or_else(|error| {
        panic!("read WAN block-3 fixture at {}: {error}", fixture.display())
    });
    let batch: TransactionBatch =
        serde_json::from_str(&batch_json).expect("parse WAN block-3 profile batch");
    let transaction = batch
        .asset_transactions
        .first()
        .expect("fixture has profile registration");
    let AssetTransactionOperation::NavProfileRegister(operation) =
        &transaction.unsigned.operation
    else {
        panic!("fixture transaction is not nav_profile_register");
    };

    let current_profile = NavProofProfile::new(
        operation.registrant.clone(),
        operation.verifier_kind.clone(),
        operation.effective_source_class(),
        operation.max_snapshot_age_blocks,
        operation.challenge_window_blocks,
        operation.max_epoch_gap_blocks,
        operation.settle_deadline_blocks,
        operation.min_challenge_bond,
        operation.min_attestations,
        operation.tolerance_bp,
        operation.valuation_policy_hash.clone(),
        operation.sp1_program_vkey.clone(),
        operation.sp1_proof_encoding.clone(),
        operation.max_proof_bytes,
        operation.max_public_values_bytes,
    )
    .expect("current profile");
    assert_ne!(
        current_profile.profile_id,
        "75c10ab4aaa8ed3b2fb6400e4d525a1ae443d1762e779d938dc743e478bbe3d83168f7825b1cb290803d07125f6d3b22"
    );
    assert_eq!(
        legacy_nav_profile_id_without_empty_sp1_fields(operation),
        "75c10ab4aaa8ed3b2fb6400e4d525a1ae443d1762e779d938dc743e478bbe3d83168f7825b1cb290803d07125f6d3b22"
    );

    let genesis =
        Genesis::try_new_with_validator_count("postfiat-wan-devnet".to_string(), 1)
            .expect("wan genesis");
    let mut block = dummy_block_record(3);
    let mut ledger = LedgerState::new(Vec::new());
    ledger.nav_proof_profiles.push(current_profile.clone());
    replay_legacy_wan_devnet_nav_profile_ids(&genesis, &block, &mut ledger, &batch)
        .expect("legacy profile id replay");
    assert_eq!(
        ledger.nav_proof_profiles[0].profile_id,
        "75c10ab4aaa8ed3b2fb6400e4d525a1ae443d1762e779d938dc743e478bbe3d83168f7825b1cb290803d07125f6d3b22"
    );

    block.header.height = WAN_DEVNET_LEGACY_NAV_PROFILE_ID_SCHEMA_MAX_HEIGHT + 1;
    let mut ledger_after_window = LedgerState::new(Vec::new());
    ledger_after_window
        .nav_proof_profiles
        .push(current_profile.clone());
    replay_legacy_wan_devnet_nav_profile_ids(
        &genesis,
        &block,
        &mut ledger_after_window,
        &batch,
    )
    .expect("profile id replay outside window is no-op");
    assert_eq!(
        ledger_after_window.nav_proof_profiles[0].profile_id,
        current_profile.profile_id
    );
}

#[test]
fn wan_devnet_legacy_receipt_replay_accepts_tx_id_drift_only() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "testdata/wan-devnet-legacy-round-6-reserve-submit/asset.batch.json",
    );
    let batch_json = fs::read_to_string(&fixture).unwrap_or_else(|error| {
        panic!("read WAN round-6 fixture at {}: {error}", fixture.display())
    });
    let batch: TransactionBatch =
        serde_json::from_str(&batch_json).expect("parse WAN round-6 reserve batch");
    let transaction = batch
        .asset_transactions
        .first()
        .expect("fixture has reserve submit");
    let legacy_signing_bytes =
        legacy_nav_reserve_submit_signing_bytes_without_sp1_evidence_fields(transaction)
            .expect("fixture uses the legacy reserve-submit preimage");
    let public_key = hex_to_bytes(&transaction.public_key_hex).expect("public key hex");
    let signature = hex_to_bytes(&transaction.signature_hex).expect("signature hex");
    assert!(
        ml_dsa_65_verify(&public_key, &legacy_signing_bytes, &signature),
        "historical WAN reserve-submit signature must verify under the legacy replay preimage"
    );
    assert!(
        !ml_dsa_65_verify(&public_key, &transaction.unsigned.signing_bytes(), &signature),
        "historical WAN reserve-submit fixture should not verify under the current preimage"
    );

    let genesis =
        Genesis::try_new_with_validator_count("postfiat-wan-devnet".to_string(), 1)
            .expect("wan genesis");
    let block = dummy_block_record(6);
    let replayed = Receipt::accepted(
        asset_transaction_tx_id(transaction),
        "asset transaction applied; fee burned",
    )
    .with_fee_policy_and_state_expansion(33, 33, 23, 10, 10);
    let mut persisted = replayed.clone();
    persisted.tx_id =
        "ee510658a25a14e1cb9d4f25df2142e90522e5d7d9787d0d642937c1515f6553396d3a640fecffbe630761ea697d6773"
            .to_string();
    assert!(replayed_receipt_matches_persisted(
        &genesis, &block, &replayed, &persisted
    ));

    let mut semantically_different = persisted.clone();
    semantically_different.fee_burned += 1;
    assert!(!replayed_receipt_matches_persisted(
        &genesis,
        &block,
        &replayed,
        &semantically_different
    ));

    let non_wan_genesis =
        Genesis::try_new_with_validator_count("postfiat-local".to_string(), 1)
            .expect("non-wan genesis");
    assert!(!replayed_receipt_matches_persisted(
        &non_wan_genesis,
        &block,
        &replayed,
        &persisted
    ));
}

#[test]
fn wan_devnet2_receipt_id_drift_is_exactly_allowlisted() {
    let genesis =
        Genesis::try_new_with_validator_count("postfiat-wan-devnet-2".to_string(), 6)
            .expect("devnet-2 genesis");
    let mut block = dummy_block_record(17);
    block.header.batch_id =
        "ace64594dd02afedcab12a380d16a4b3e754ec14463f0daf971b9962c0bd9093c9b73f74bd81f4c735b328fee4d9c620"
            .to_string();
    assert!(archived_wan_devnet2_legacy_receipt_id_drift_allowed(
        &genesis, &block
    ));
    assert!(archived_pre_pricing_swap_execution_allowed(
        true,
        &genesis,
        block.header.height,
        &block.header.batch_id,
    ));
    assert!(!archived_pre_pricing_swap_execution_allowed(
        false,
        &genesis,
        block.header.height,
        &block.header.batch_id,
    ));

    let mut archived_egress = dummy_block_record(82);
    archived_egress.header.batch_id =
        "95cfdfc8d5d8f523709431209de3722be2daaa826180068d2eac5974e58355a21216e490a216bf681652bc6a77dacd07"
            .to_string();
    assert!(archived_wan_devnet2_pre_repin_private_egress_allowed(
        &genesis,
        archived_egress.header.height,
        &archived_egress.header.batch_id,
    ));
    assert!(archived_wan_devnet2_legacy_receipt_id_drift_allowed(
        &genesis,
        &archived_egress,
    ));
    assert!(archived_pre_repin_private_egress_execution_allowed(
        true,
        &genesis,
        archived_egress.header.height,
        &archived_egress.header.batch_id,
    ));
    assert!(!archived_pre_repin_private_egress_execution_allowed(
        false,
        &genesis,
        archived_egress.header.height,
        &archived_egress.header.batch_id,
    ));

    let mut wrong_egress_height = archived_egress.clone();
    wrong_egress_height.header.height += 1;
    assert!(!archived_wan_devnet2_pre_repin_private_egress_allowed(
        &genesis,
        wrong_egress_height.header.height,
        &wrong_egress_height.header.batch_id,
    ));

    let mut wrong_height = block.clone();
    wrong_height.header.height = 18;
    assert!(!archived_wan_devnet2_legacy_receipt_id_drift_allowed(
        &genesis,
        &wrong_height
    ));

    let mut wrong_batch = block.clone();
    wrong_batch.header.batch_id = "00".repeat(48);
    assert!(!archived_wan_devnet2_legacy_receipt_id_drift_allowed(
        &genesis,
        &wrong_batch
    ));

    let other_chain =
        Genesis::try_new_with_validator_count("postfiat-wan-devnet".to_string(), 6)
            .expect("other genesis");
    assert!(!archived_wan_devnet2_legacy_receipt_id_drift_allowed(
        &other_chain,
        &block
    ));
}

#[cfg(unix)]
#[test]
fn private_material_reads_reject_group_readable_files() {
    use std::os::unix::fs::PermissionsExt;

    fn make_group_readable(path: &Path) {
        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o644);
        fs::set_permissions(path, permissions).expect("set broad permissions");
    }

    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-private-material-permissions-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&data_dir).expect("create test dir");

    let key_file = data_dir.join("wallet.key.json");
    let backup_file = data_dir.join("wallet.backup.json");
    wallet_keygen(WalletKeygenOptions {
        chain_id: "postfiat-local".to_string(),
        master_seed_hex:
            "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f"
                .to_string(),
        account_index: 0,
        key_file: key_file.clone(),
        backup_file: backup_file.clone(),
        overwrite: false,
    })
    .expect("wallet keygen");

    make_group_readable(&key_file);
    assert_eq!(
        read_key_file(&key_file).expect_err("broad wallet key mode must fail").kind(),
        io::ErrorKind::PermissionDenied
    );

    make_group_readable(&backup_file);
    assert_eq!(
        read_wallet_backup_file(&backup_file)
            .expect_err("broad backup mode must fail")
            .kind(),
        io::ErrorKind::PermissionDenied
    );

    let validator_key_file = data_dir.join("validator_keys.json");
    write_validator_key_file(
        &validator_key_file,
        &ValidatorKeyFile {
            validators: vec![
                create_validator_key_record("validator-0".to_string())
                    .expect("validator key record"),
            ],
        },
    )
    .expect("write validator key");
    make_group_readable(&validator_key_file);
    assert_eq!(
        read_validator_key_file(&validator_key_file)
            .expect_err("broad validator key mode must fail")
            .kind(),
        io::ErrorKind::PermissionDenied
    );

    let orchard_key_file = data_dir.join("orchard.key.json");
    write_orchard_wallet_key_file(
        &orchard_key_file,
        &OrchardWalletKeyFile {
            schema: ORCHARD_WALLET_FILE_SCHEMA.to_string(),
            kdf: ORCHARD_WALLET_DERIVATION_KDF.to_string(),
            derivation_domain: ORCHARD_WALLET_DERIVATION_DOMAIN.to_string(),
            account_index: 0,
            spending_key_hex: bytes_to_hex(&[7u8; 32]),
            address_raw_hex: orchard_default_address_from_spending_key([7u8; 32])
                .expect("orchard address"),
        },
    )
    .expect("write Orchard key");
    make_group_readable(&orchard_key_file);
    assert_eq!(
        read_orchard_wallet_key_file(&orchard_key_file)
            .expect_err("broad Orchard key mode must fail")
            .kind(),
        io::ErrorKind::PermissionDenied
    );
}

fn dummy_mempool_entry(from: &str, sequence: u64) -> MempoolEntry {
    let transfer = SignedTransfer {
        unsigned: UnsignedTransfer {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "a".repeat(96),
            protocol_version: 1,
            address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
            transaction_kind: postfiat_types::TRANSFER_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            from: from.to_string(),
            to: format!("pfrecipient{sequence:030}"),
            amount: ACCOUNT_RESERVE,
            fee: 1,
            sequence,
        },
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: "00".to_string(),
        signature_hex: "11".to_string(),
    };
    MempoolEntry::new(format!("tx-{from}-{sequence}"), transfer)
}

#[test]
fn mempool_limits_reject_global_and_sender_overflow() {
    let full_mempool = MempoolState {
        pending: (0..MAX_MEMPOOL_PENDING_TRANSACTIONS)
            .map(|index| dummy_mempool_entry(&format!("pfsender{index:030}"), 1))
            .collect(),
        pending_payment_v2: Vec::new(),
        pending_asset_transactions: Vec::new(),
        pending_atomic_swaps: Vec::new(),
        pending_fastlane_primary: Vec::new(),
        pending_escrow_transactions: Vec::new(),
        pending_nft_transactions: Vec::new(),
        pending_offer_transactions: Vec::new(),
    };
    enforce_mempool_state_limits(&full_mempool).expect("full mempool at limit is valid");
    assert!(enforce_mempool_admission_limits(&full_mempool, "pfnewsender").is_err());

    let mut global_overflow = full_mempool;
    global_overflow
        .pending
        .push(dummy_mempool_entry("pfglobaloverflow", 1));
    assert!(enforce_mempool_state_limits(&global_overflow).is_err());

    let sender = "pfsenderlimit000000000000000000000";
    let sender_limited_mempool = MempoolState {
        pending: (1..=MAX_MEMPOOL_PENDING_PER_SENDER as u64)
            .map(|sequence| dummy_mempool_entry(sender, sequence))
            .collect(),
        pending_payment_v2: Vec::new(),
        pending_asset_transactions: Vec::new(),
        pending_atomic_swaps: Vec::new(),
        pending_fastlane_primary: Vec::new(),
        pending_escrow_transactions: Vec::new(),
        pending_nft_transactions: Vec::new(),
        pending_offer_transactions: Vec::new(),
    };
    enforce_mempool_state_limits(&sender_limited_mempool)
        .expect("sender at pending limit is valid");
    assert!(enforce_mempool_admission_limits(&sender_limited_mempool, sender).is_err());

    let mut sender_overflow = sender_limited_mempool;
    sender_overflow.pending.push(dummy_mempool_entry(
        sender,
        MAX_MEMPOOL_PENDING_PER_SENDER as u64 + 1,
    ));
    assert!(enforce_mempool_state_limits(&sender_overflow).is_err());
}

#[test]
fn split_block_votes_reconstruct_certificate() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-split-block-vote-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 2,
    })
    .expect("init");
    let batch_file = data_dir.join("batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfrecipient000000000000000000000000000010".to_string(),
        amount: 25,
        batch_file: batch_file.clone(),
    })
    .expect("create batch");
    let proposal_file = data_dir.join("block_proposal.json");
    let proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: None,
        batch_file: batch_file.clone(),
        proposal_file: proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose batch");
    assert_eq!(proposal.schema, BLOCK_PROPOSAL_FILE_SCHEMA);
    assert_eq!(proposal.block_height, 1);
    assert_eq!(proposal.view, 0);
    assert_eq!(proposal.proposer, "validator-1");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let validator_registry =
        read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
            .expect("validator registry");
    let expected_registry_root = validator_registry_root(
        &validator_registry,
        &local_validator_ids(2).expect("validator ids"),
    )
    .expect("validator registry root");
    let vote_paths = validator_keys
        .validators
        .iter()
        .map(|record| {
            let split_key_path =
                data_dir.join(format!("{}.validator_keys.json", record.node_id));
            write_validator_key_file(
                &split_key_path,
                &ValidatorKeyFile {
                    validators: vec![record.clone()],
                },
            )
            .expect("write split validator key");
            let vote_file = data_dir.join(format!("{}.block_vote.json", record.node_id));
            let vote = create_block_vote(BlockVoteOptions {
                data_dir: data_dir.clone(),
                verify_block_log: true,
                key_file: split_key_path,
                validator_id: None,
                batch_file: Some(batch_file.clone()),
                proposal_file: Some(proposal_file.clone()),
                timeout_certificate_file: None,
                block_height: Some(proposal.block_height),
                vote_file: vote_file.clone(),
            })
            .expect("create proposal split block vote");
            assert_eq!(vote.vote.validator, record.node_id);
            assert_eq!(vote.view, proposal.view);
            assert!(vote.block_hash.is_none());
            assert!(vote.proposal_hash.is_some());
            assert_eq!(vote.vote.registry_root, expected_registry_root);
            assert!(vote.vote.public_key_hex.is_empty());
            vote_file
        })
        .collect::<Vec<_>>();
    let proposal_certificate_file = data_dir.join("proposal_block_certificate.json");
    let proposal_certificate = aggregate_block_certificate(BlockCertificateOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_file: Some(batch_file.clone()),
        proposal_file: Some(proposal_file.clone()),
        timeout_certificate_file: None,
        block_height: Some(proposal.block_height),
        vote_files: vote_paths.clone(),
        certificate_file: proposal_certificate_file.clone(),
    })
    .expect("aggregate proposal split block certificate");
    assert!(proposal_certificate.block_hash.is_none());
    assert!(proposal_certificate.proposal_hash.is_some());
    assert_eq!(proposal_certificate.view, proposal.view);
    assert_eq!(proposal_certificate.proposer, proposal.proposer);
    assert_eq!(
        proposal_certificate.certificate.registry_root,
        expected_registry_root
    );
    assert!(proposal_certificate.certificate.votes.iter().all(|vote| {
        vote.registry_root == expected_registry_root && vote.public_key_hex.is_empty()
    }));
    let proposal_certificate_json =
        serde_json::to_string(&proposal_certificate).expect("proposal certificate json");
    assert!(!proposal_certificate_json.contains("public_key_hex"));
    assert_eq!(
        status(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("status after proposal")
        .block_height,
        0
    );
    let missing_registry_root_file = data_dir.join("missing-root.block_certificate.json");
    let mut missing_registry_root = proposal_certificate.clone();
    missing_registry_root.certificate.registry_root.clear();
    write_block_certificate_file(&missing_registry_root_file, &missing_registry_root)
        .expect("write missing-root certificate");
    let missing_root_error = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: batch_file.clone(),
        certificate_file: Some(missing_registry_root_file),
    })
    .expect_err("live external certificate without registry root must fail closed");
    assert!(
        missing_root_error
            .to_string()
            .contains("external block certificate registry root is required"),
        "{missing_root_error}"
    );
    assert_eq!(
        status(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("status after missing-root rejection")
        .block_height,
        0
    );
    std::fs::remove_file(data_dir.join(VALIDATOR_KEYS_FILE))
        .expect("remove combined validator keys");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: batch_file.clone(),
        certificate_file: Some(proposal_certificate_file),
    })
    .expect("apply batch with external certificate");

    let block_log = blocks(BlockQueryOptions {
        data_dir: data_dir.clone(),
        from_height: None,
        limit: None,
    })
    .expect("blocks");
    let block = block_log.first().expect("committed block");
    assert_eq!(block.header.certificate.validators.len(), 2);
    assert_eq!(proposal.block_height, block.header.height);
    assert_eq!(proposal.view, block.header.view);
    assert_eq!(proposal.parent_hash, block.header.parent_hash);
    assert_eq!(proposal.proposer, block.header.proposer);
    assert_eq!(proposal.batch_kind, block.header.batch_kind);
    assert_eq!(proposal.batch_id, block.header.batch_id);
    assert_eq!(proposal.state_root, block.header.state_root);
    assert_eq!(proposal.receipt_count, block.header.receipt_count);
    assert_eq!(proposal.receipt_ids, block.receipt_ids);
    assert_eq!(
        proposal_certificate.certificate_id,
        block.header.certificate_id
    );
    assert_eq!(proposal_certificate.certificate, block.header.certificate);
    let store = NodeStore::new(&data_dir);
    let blocks_before_proposer_tamper = store.read_blocks().expect("blocks before tamper");
    let mut blocks_with_proposer_tamper = blocks_before_proposer_tamper.clone();
    blocks_with_proposer_tamper.blocks[0].header.proposer = "validator-0".to_string();
    store
        .write_blocks(&blocks_with_proposer_tamper)
        .expect("write proposer tamper");
    let proposer_error = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("tampered block proposer should fail verification");
    assert!(
        proposer_error.to_string().contains("proposer mismatch"),
        "{proposer_error}"
    );
    store
        .write_blocks(&blocks_before_proposer_tamper)
        .expect("restore proposer tamper");
    let committed_vote_paths = validator_keys
        .validators
        .iter()
        .map(|record| {
            let split_key_path =
                data_dir.join(format!("{}.validator_keys.json", record.node_id));
            let vote_file =
                data_dir.join(format!("{}.committed_block_vote.json", record.node_id));
            let vote = create_block_vote(BlockVoteOptions {
                data_dir: data_dir.clone(),
                verify_block_log: true,
                key_file: split_key_path,
                validator_id: None,
                batch_file: None,
                proposal_file: None,
                timeout_certificate_file: None,
                block_height: Some(block.header.height),
                vote_file: vote_file.clone(),
            })
            .expect("create committed split block vote");
            assert_eq!(vote.vote.validator, record.node_id);
            assert_eq!(
                vote.block_hash.as_deref(),
                Some(block.header.block_hash.as_str())
            );
            assert!(vote.proposal_hash.is_none());
            assert_eq!(vote.vote.registry_root, expected_registry_root);
            assert!(vote.vote.public_key_hex.is_empty());
            vote_file
        })
        .collect::<Vec<_>>();

    let certificate_file = data_dir.join("block_certificate.json");
    let certificate = aggregate_block_certificate(BlockCertificateOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_file: None,
        proposal_file: None,
        timeout_certificate_file: None,
        block_height: Some(block.header.height),
        vote_files: committed_vote_paths.clone(),
        certificate_file,
    })
    .expect("aggregate split block certificate");
    assert_eq!(certificate.certificate_id, block.header.certificate_id);
    assert_eq!(certificate.certificate, block.header.certificate);

    let duplicate_error = aggregate_block_certificate(BlockCertificateOptions {
        data_dir,
        verify_block_log: true,
        batch_file: None,
        proposal_file: None,
        timeout_certificate_file: None,
        block_height: Some(block.header.height),
        vote_files: vec![
            committed_vote_paths[0].clone(),
            committed_vote_paths[0].clone(),
        ],
        certificate_file: std::env::temp_dir().join("duplicate-block-certificate.json"),
    })
    .expect_err("duplicate split vote must fail");
    assert!(
        duplicate_error.to_string().contains("duplicate block vote"),
        "{duplicate_error}"
    );
}

#[test]
fn signed_block_proposals_verify_before_votes() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-signed-block-proposal-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 2,
    })
    .expect("init signed proposal test");
    let batch_file = data_dir.join("signed-proposal-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfsignedproposal0000000000000000000000".to_string(),
        amount: 37,
        batch_file: batch_file.clone(),
    })
    .expect("create signed proposal batch");

    let wrong_signer_error = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: batch_file.clone(),
        proposal_file: data_dir.join("wrong-signer.block_proposal.json"),
        view: None,
        timeout_certificate_file: None,
        key_file: Some(data_dir.join(VALIDATOR_KEYS_FILE)),
        validator_id: Some("validator-0".to_string()),
    })
    .expect_err("wrong proposer signer must fail");
    assert!(
        wrong_signer_error
            .to_string()
            .contains("does not match proposer"),
        "{wrong_signer_error}"
    );

    let proposal_file = data_dir.join("signed.block_proposal.json");
    let proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: batch_file.clone(),
        proposal_file: proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: Some(data_dir.join(VALIDATOR_KEYS_FILE)),
        validator_id: None,
    })
    .expect("propose signed batch with inferred proposer key");
    let signature = proposal.signature.as_ref().expect("proposal signature");
    assert_eq!(signature.signer, proposal.proposer);
    assert_eq!(signature.algorithm_id, ML_DSA_65_ALGORITHM);

    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let split_key_paths = write_split_validator_key_files(&data_dir, &validator_keys);
    let validator_0_key = split_key_paths
        .iter()
        .find_map(|(node_id, path)| (node_id == "validator-0").then_some(path.clone()))
        .expect("validator-0 split key");
    let vote_file = data_dir.join("signed-proposal.validator-0.block_vote.json");
    create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: validator_0_key.clone(),
        validator_id: None,
        batch_file: Some(batch_file.clone()),
        proposal_file: Some(proposal_file.clone()),
        timeout_certificate_file: None,
        block_height: Some(proposal.block_height),
        vote_file,
    })
    .expect("vote for signed proposal");

    let mut tampered = proposal;
    tampered.signature.as_mut().expect("signature").signer = "validator-0".to_string();
    let tampered_proposal_file = data_dir.join("tampered-signed.block_proposal.json");
    write_block_proposal_file(&tampered_proposal_file, &tampered)
        .expect("write tampered signed proposal");
    let tampered_error = create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: validator_0_key,
        validator_id: None,
        batch_file: Some(batch_file),
        proposal_file: Some(tampered_proposal_file),
        timeout_certificate_file: None,
        block_height: Some(tampered.block_height),
        vote_file: data_dir.join("tampered-signed-proposal.block_vote.json"),
    })
    .expect_err("tampered proposal signature must reject votes");
    assert!(
        tampered_error
            .to_string()
            .contains("signature signer does not match proposer"),
        "{tampered_error}"
    );

    std::fs::remove_dir_all(data_dir).expect("cleanup signed proposal data");
}

#[test]
fn block_proposer_reports_deterministic_local_status() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-block-proposer-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-1".to_string(),
        validator_count: 4,
    })
    .expect("init block proposer test");

    let height_1 = block_proposer(BlockProposerOptions {
        data_dir: data_dir.clone(),
        block_height: 1,
        view: 0,
    })
    .expect("height 1 proposer");
    assert_eq!(height_1.proposer, "validator-1");
    assert!(height_1.local_is_proposer);

    let height_2_view_1 = block_proposer(BlockProposerOptions {
        data_dir: data_dir.clone(),
        block_height: 2,
        view: 1,
    })
    .expect("height 2 view 1 proposer");
    assert_eq!(height_2_view_1.proposer, "validator-3");
    assert!(!height_2_view_1.local_is_proposer);

    let zero_height_error = block_proposer(BlockProposerOptions {
        data_dir: data_dir.clone(),
        block_height: 0,
        view: 0,
    })
    .expect_err("zero height rejected");
    assert!(
        zero_height_error.to_string().contains("must be positive"),
        "{zero_height_error}"
    );

    std::fs::remove_dir_all(data_dir).expect("cleanup block proposer data");
}

pub(super) fn write_split_validator_key_files(
    data_dir: &Path,
    validator_keys: &ValidatorKeyFile,
) -> Vec<(String, PathBuf)> {
    validator_keys
        .validators
        .iter()
        .map(|record| {
            let split_key_path =
                data_dir.join(format!("{}.validator_keys.json", record.node_id));
            write_validator_key_file(
                &split_key_path,
                &ValidatorKeyFile {
                    validators: vec![record.clone()],
                },
            )
            .expect("write split validator key");
            (record.node_id.clone(), split_key_path)
        })
        .collect()
}

fn copy_test_dir_recursive(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create cloned test dir");
    for entry in fs::read_dir(from).expect("read source test dir") {
        let entry = entry.expect("read source test entry");
        let source_path = entry.path();
        let target_path = to.join(entry.file_name());
        if entry.file_type().expect("source entry type").is_dir() {
            copy_test_dir_recursive(&source_path, &target_path);
        } else {
            fs::copy(&source_path, &target_path).expect("copy test file");
        }
    }
}

#[test]
fn timeout_votes_reconstruct_hotstuff_timeout_certificate() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-timeout-certificate-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init timeout certificate test");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let validator_registry =
        read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
            .expect("validator registry");
    let expected_registry_root = validator_registry_root(
        &validator_registry,
        &local_validator_ids(4).expect("validator ids"),
    )
    .expect("validator registry root");
    let split_key_paths = write_split_validator_key_files(&data_dir, &validator_keys);
    let vote_paths = split_key_paths
        .iter()
        .take(3)
        .map(|(node_id, split_key_path)| {
            let vote_file = data_dir.join(format!("timeout.{node_id}.json"));
            let vote = create_block_timeout_vote(BlockTimeoutVoteOptions {
                data_dir: data_dir.clone(),
                verify_block_log: true,
                key_file: split_key_path.clone(),
                validator_id: None,
                block_height: 1,
                view: 2,
                high_qc_id: "qc-shared".to_string(),
                vote_file: vote_file.clone(),
            })
            .expect("create timeout vote");
            assert_eq!(&vote.vote.validator, node_id);
            assert_eq!(vote.block_height, 1);
            assert_eq!(vote.view, 2);
            assert_eq!(vote.vote.registry_root, expected_registry_root);
            assert!(vote.vote.public_key_hex.is_empty());
            vote_file
        })
        .collect::<Vec<_>>();

    let certificate_file = data_dir.join("timeout-certificate.json");
    let certificate = aggregate_block_timeout_certificate(BlockTimeoutCertificateOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        block_height: 1,
        view: 2,
        vote_files: vote_paths.clone(),
        certificate_file: certificate_file.clone(),
    })
    .expect("aggregate timeout certificate");
    assert_eq!(certificate.schema, BLOCK_TIMEOUT_CERTIFICATE_FILE_SCHEMA);
    assert_eq!(certificate.block_height, 1);
    assert_eq!(certificate.view, 2);
    assert_eq!(certificate.certificate.validators.len(), 4);
    assert_eq!(certificate.certificate.quorum, 3);
    assert_eq!(certificate.certificate.votes.len(), 3);
    assert_eq!(certificate.certificate.high_qc_id, "qc-shared");
    assert_eq!(
        certificate.certificate.registry_root,
        expected_registry_root
    );
    assert!(certificate.certificate.votes.iter().all(|vote| {
        vote.registry_root == expected_registry_root && vote.public_key_hex.is_empty()
    }));
    let timeout_certificate_json =
        serde_json::to_string(&certificate).expect("timeout certificate json");
    assert!(!timeout_certificate_json.contains("public_key_hex"));
    assert!(!certificate.hotstuff_certificate_id.is_empty());
    assert!(!certificate.certificate_id.is_empty());
    let verified =
        verify_block_timeout_certificate_file(BlockTimeoutCertificateVerifyOptions {
            data_dir: data_dir.clone(),
            verify_block_log: true,
            certificate_file: certificate_file.clone(),
        })
        .expect("verify timeout certificate file");
    assert_eq!(verified, certificate);

    let mut tampered_high_qc = certificate.clone();
    tampered_high_qc.certificate.high_qc_id = "qc-other".to_string();
    let tampered_file = data_dir.join("tampered-timeout-certificate.json");
    write_block_timeout_certificate_file(&tampered_file, &tampered_high_qc)
        .expect("write tampered timeout certificate");
    let tampered_error =
        verify_block_timeout_certificate_file(BlockTimeoutCertificateVerifyOptions {
            data_dir: data_dir.clone(),
            verify_block_log: true,
            certificate_file: tampered_file,
        })
        .expect_err("tampered timeout certificate must fail verification");
    assert!(
        tampered_error.to_string().contains("high_qc mismatch"),
        "{tampered_error}"
    );

    let duplicate_error = aggregate_block_timeout_certificate(BlockTimeoutCertificateOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        block_height: 1,
        view: 2,
        vote_files: vec![
            vote_paths[0].clone(),
            vote_paths[0].clone(),
            vote_paths[1].clone(),
            vote_paths[2].clone(),
        ],
        certificate_file: data_dir.join("duplicate-timeout-certificate.json"),
    })
    .expect_err("duplicate timeout vote must fail");
    assert!(
        duplicate_error
            .to_string()
            .contains("duplicate block timeout vote"),
        "{duplicate_error}"
    );

    let wrong_view_error =
        aggregate_block_timeout_certificate(BlockTimeoutCertificateOptions {
            data_dir: data_dir.clone(),
            verify_block_log: true,
            block_height: 1,
            view: 1,
            vote_files: vote_paths,
            certificate_file: data_dir.join("wrong-view-timeout-certificate.json"),
        })
        .expect_err("wrong timeout view must fail");
    assert!(
        wrong_view_error
            .to_string()
            .contains("block timeout vote view 2 does not match view 1"),
        "{wrong_view_error}"
    );
    std::fs::remove_dir_all(data_dir).expect("cleanup timeout certificate data");
}

#[test]
fn certify_batch_round_uses_split_keys_without_combined_file() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-certify-batch-round-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 2,
    })
    .expect("init certify batch round test");
    let batch_file = data_dir.join("transparent-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfcertifybatchround00000000000000000000".to_string(),
        amount: 31,
        batch_file: batch_file.clone(),
    })
    .expect("create transparent batch");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    write_split_validator_key_files(&data_dir, &validator_keys);
    std::fs::remove_file(data_dir.join(VALIDATOR_KEYS_FILE))
        .expect("remove combined validator keys");

    let proposal_file = data_dir.join("round.block_proposal.json");
    let vote_dir = data_dir.join("round-votes");
    let certificate_file = data_dir.join("round.block_certificate.json");
    let report = certify_batch_round(BatchCertificateRoundOptions {
        data_dir: data_dir.clone(),
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: batch_file.clone(),
        validator_key_dir: data_dir.clone(),
        vote_dir: vote_dir.clone(),
        proposal_file: proposal_file.clone(),
        certificate_file: certificate_file.clone(),
        block_height: Some(1),
        view: None,
        timeout_certificate_file: None,
        skip_block_log_verify: false,
    })
    .expect("certify transparent batch from split keys");
    assert_eq!(report.schema, "postfiat.batch_certificate_round.v1");
    assert!(report.round_ok);
    assert_eq!(report.vote_count, 2);
    assert_eq!(
        report.validators,
        vec!["validator-0".to_string(), "validator-1".to_string()]
    );
    assert_eq!(report.vote_files.len(), 2);
    assert!(proposal_file.exists());
    assert!(certificate_file.exists());
    assert!(vote_dir.join("validator-0.block_vote.json").exists());
    let signed_proposal =
        read_block_proposal_file(&proposal_file).expect("read signed certified proposal");
    assert_eq!(
        signed_proposal
            .signature
            .as_ref()
            .expect("certified proposal signature")
            .signer,
        signed_proposal.proposer
    );
    let report_json = serde_json::to_string(&report).expect("serialize report");
    assert!(!report_json.contains("private_key_hex"));
    assert!(!report_json.contains("public_key_hex"));

    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: Some(certificate_file),
    })
    .expect("apply certified batch");
    let block_log = blocks(BlockQueryOptions {
        data_dir: data_dir.clone(),
        from_height: None,
        limit: None,
    })
    .expect("blocks after certified round");
    let block = block_log.first().expect("committed block");
    assert_eq!(block.header.certificate_id, report.certificate_id);
    assert_eq!(block.header.certificate.votes.len(), 2);

    std::fs::remove_dir_all(data_dir).expect("cleanup certify batch round data");
}

#[test]
fn proposal_certificate_accepts_three_of_four_bft_quorum() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-three-of-four-block-certificate-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init three-of-four certificate test");
    let batch_file = data_dir.join("transparent-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfthreeoffourquorum00000000000000000000".to_string(),
        amount: 41,
        batch_file: batch_file.clone(),
    })
    .expect("create transparent batch");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let split_key_paths = write_split_validator_key_files(&data_dir, &validator_keys);
    let timeout_vote_paths = split_key_paths
        .iter()
        .take(3)
        .map(|(node_id, split_key_path)| {
            let vote_file = data_dir.join(format!("quorum.{node_id}.block_timeout_vote.json"));
            let vote = create_block_timeout_vote(BlockTimeoutVoteOptions {
                data_dir: data_dir.clone(),
                verify_block_log: true,
                key_file: split_key_path.clone(),
                validator_id: None,
                block_height: 1,
                view: 1,
                high_qc_id: "qc-shared".to_string(),
                vote_file: vote_file.clone(),
            })
            .expect("create quorum timeout vote");
            assert_eq!(&vote.vote.validator, node_id);
            assert_eq!(vote.block_height, 1);
            assert_eq!(vote.view, 1);
            vote_file
        })
        .collect::<Vec<_>>();
    let timeout_certificate_file = data_dir.join("quorum.block_timeout_certificate.json");
    let timeout_certificate =
        aggregate_block_timeout_certificate(BlockTimeoutCertificateOptions {
            data_dir: data_dir.clone(),
            verify_block_log: true,
            block_height: 1,
            view: 1,
            vote_files: timeout_vote_paths,
            certificate_file: timeout_certificate_file.clone(),
        })
        .expect("aggregate quorum timeout certificate");
    assert_eq!(timeout_certificate.block_height, 1);
    assert_eq!(timeout_certificate.view, 1);
    assert_eq!(timeout_certificate.certificate.quorum, 3);
    let missing_timeout_error = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: batch_file.clone(),
        proposal_file: data_dir.join("missing-timeout.block_proposal.json"),
        view: Some(2),
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect_err("nonzero-view proposal must fail closed");
    assert!(
        missing_timeout_error
            .to_string()
            .contains("nonzero-view block proposals require activated consensus v2"),
        "{missing_timeout_error}"
    );
    let unsupported_timeout_error = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: batch_file.clone(),
        proposal_file: data_dir.join("unsupported-timeout.block_proposal.json"),
        view: Some(2),
        timeout_certificate_file: Some(timeout_certificate_file.clone()),
        key_file: None,
        validator_id: None,
    })
    .expect_err("timeout certificate must not enable unsupported view changes");
    assert!(
        unsupported_timeout_error
            .to_string()
            .contains("nonzero-view block proposals require activated consensus v2"),
        "{unsupported_timeout_error}"
    );
    let view_zero_timeout_error = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: batch_file.clone(),
        proposal_file: data_dir.join("view-zero-timeout.block_proposal.json"),
        view: None,
        timeout_certificate_file: Some(timeout_certificate_file.clone()),
        key_file: None,
        validator_id: None,
    })
    .expect_err("view-zero proposal must reject timeout certificate");
    assert!(
        view_zero_timeout_error
            .to_string()
            .contains("view 0 proposal must not include timeout certificate evidence"),
        "{view_zero_timeout_error}"
    );
    let proposal_file = data_dir.join("quorum.block_proposal.json");
    let proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: batch_file.clone(),
        proposal_file: proposal_file.clone(),
        view: Some(0),
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose quorum batch");
    assert_eq!(proposal.block_height, 1);
    assert_eq!(proposal.view, 0);
    assert_eq!(proposal.proposer, "validator-1");

    std::fs::remove_file(data_dir.join(VALIDATOR_KEYS_FILE))
        .expect("remove combined validator keys");
    let vote_paths = split_key_paths
        .iter()
        .take(3)
        .map(|(node_id, split_key_path)| {
            let vote_file = data_dir.join(format!("quorum.{node_id}.block_vote.json"));
            let vote = create_block_vote(BlockVoteOptions {
                data_dir: data_dir.clone(),
                verify_block_log: true,
                key_file: split_key_path.clone(),
                validator_id: None,
                batch_file: Some(batch_file.clone()),
                proposal_file: Some(proposal_file.clone()),
                timeout_certificate_file: None,
                block_height: Some(proposal.block_height),
                vote_file: vote_file.clone(),
            })
            .expect("create quorum proposal vote");
            assert_eq!(&vote.vote.validator, node_id);
            assert_eq!(vote.view, proposal.view);
            assert!(vote.block_hash.is_none());
            assert!(vote.proposal_hash.is_some());
            vote_file
        })
        .collect::<Vec<_>>();

    let certificate_file = data_dir.join("quorum.block_certificate.json");
    let certificate = aggregate_block_certificate(BlockCertificateOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_file: Some(batch_file.clone()),
        proposal_file: Some(proposal_file),
        timeout_certificate_file: None,
        block_height: Some(proposal.block_height),
        vote_files: vote_paths,
        certificate_file: certificate_file.clone(),
    })
    .expect("aggregate three-of-four block certificate");
    assert_eq!(certificate.certificate.validators.len(), 4);
    assert_eq!(certificate.certificate.quorum, 3);
    assert_eq!(certificate.certificate.votes.len(), 3);

    let mut noncanonical_certificate = certificate.clone();
    noncanonical_certificate.certificate.votes.swap(0, 1);
    let noncanonical_certificate_file = data_dir.join("noncanonical.block_certificate.json");
    write_block_certificate_file(&noncanonical_certificate_file, &noncanonical_certificate)
        .expect("write noncanonical block certificate");
    let noncanonical_error = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: batch_file.clone(),
        certificate_file: Some(noncanonical_certificate_file),
    })
    .expect_err("noncanonical vote order must fail");
    assert!(
        noncanonical_error
            .to_string()
            .contains("vote validator order mismatch"),
        "{noncanonical_error}"
    );

    let mut tampered_proposal_hash = certificate.clone();
    tampered_proposal_hash.proposal_hash = Some("tampered-proposal-hash".to_string());
    let tampered_proposal_hash_file =
        data_dir.join("tampered-proposal-hash.block_certificate.json");
    write_block_certificate_file(&tampered_proposal_hash_file, &tampered_proposal_hash)
        .expect("write proposal-hash tampered block certificate");
    let tampered_hash_error = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: batch_file.clone(),
        certificate_file: Some(tampered_proposal_hash_file),
    })
    .expect_err("tampered proposal hash must fail");
    assert!(
        tampered_hash_error
            .to_string()
            .contains("proposal hash mismatch"),
        "{tampered_hash_error}"
    );

    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: Some(certificate_file),
    })
    .expect("apply batch with three-of-four certificate");

    let block_log = blocks(BlockQueryOptions {
        data_dir: data_dir.clone(),
        from_height: None,
        limit: None,
    })
    .expect("blocks");
    let block = block_log.first().expect("committed block");
    assert_eq!(block.header.certificate.validators.len(), 4);
    assert_eq!(block.header.certificate.quorum, 3);
    assert_eq!(block.header.certificate.votes.len(), 3);
    assert_eq!(block.header.certificate, certificate.certificate);

    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify three-of-four certificate block");
    std::fs::remove_dir_all(data_dir).expect("cleanup three-of-four certificate data");
}

#[test]
fn block_vote_equivocation_evidence_detects_conflicting_signed_votes() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-block-vote-equivocation-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 2,
    })
    .expect("init block vote equivocation test");
    let first_batch_file = data_dir.join("first-transparent-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfequivocationfirst0000000000000000000".to_string(),
        amount: 43,
        batch_file: first_batch_file.clone(),
    })
    .expect("create first transparent batch");
    let second_batch_file = data_dir.join("second-transparent-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfequivocationsecond000000000000000000".to_string(),
        amount: 44,
        batch_file: second_batch_file.clone(),
    })
    .expect("create second transparent batch");
    let first_proposal_file = data_dir.join("first.block_proposal.json");
    let first_proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: first_batch_file.clone(),
        proposal_file: first_proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose first batch");
    let second_proposal_file = data_dir.join("second.block_proposal.json");
    let second_proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: second_batch_file.clone(),
        proposal_file: second_proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose second batch");
    assert_eq!(first_proposal.block_height, second_proposal.block_height);
    assert_eq!(first_proposal.view, second_proposal.view);
    assert_ne!(first_proposal.batch_id, second_proposal.batch_id);

    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let split_key_paths = write_split_validator_key_files(&data_dir, &validator_keys);
    let validator_0_key = split_key_paths
        .iter()
        .find_map(|(node_id, path)| (node_id == "validator-0").then_some(path.clone()))
        .expect("validator-0 split key");
    let isolated_data_dir = unique_test_dir("postfiat-block-vote-equivocation-isolated");
    copy_test_dir_recursive(&data_dir, &isolated_data_dir);
    let first_vote_file = data_dir.join("first.validator-0.block_vote.json");
    let first_vote = create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: validator_0_key.clone(),
        validator_id: None,
        batch_file: Some(first_batch_file.clone()),
        proposal_file: Some(first_proposal_file.clone()),
        timeout_certificate_file: None,
        block_height: Some(first_proposal.block_height),
        vote_file: first_vote_file.clone(),
    })
    .expect("create first proposal vote");
    let second_vote_file = data_dir.join("second.validator-0.block_vote.json");
    let same_node_conflict = create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: validator_0_key,
        validator_id: None,
        batch_file: Some(second_batch_file.clone()),
        proposal_file: Some(second_proposal_file.clone()),
        timeout_certificate_file: None,
        block_height: Some(second_proposal.block_height),
        vote_file: second_vote_file.clone(),
    })
    .expect_err("same node must not sign a conflicting proposal vote");
    assert!(
        same_node_conflict
            .to_string()
            .contains("conflicting block proposal vote already recorded"),
        "{same_node_conflict}"
    );
    let isolated_validator_0_key =
        isolated_data_dir.join("validator-0.validator_keys.json");
    let second_vote = create_block_vote(BlockVoteOptions {
        data_dir: isolated_data_dir.clone(),
        verify_block_log: true,
        key_file: isolated_validator_0_key,
        validator_id: None,
        batch_file: Some(isolated_data_dir.join("second-transparent-batch.json")),
        proposal_file: Some(isolated_data_dir.join("second.block_proposal.json")),
        timeout_certificate_file: None,
        block_height: Some(second_proposal.block_height),
        vote_file: second_vote_file.clone(),
    })
    .expect("create isolated conflicting proposal vote for evidence verification");
    assert_eq!(first_vote.vote.validator, second_vote.vote.validator);
    assert_ne!(first_vote.vote.vote_id, second_vote.vote.vote_id);

    let evidence_file = data_dir.join("validator-0.block_equivocation_evidence.json");
    let evidence = detect_block_vote_equivocation(BlockVoteEquivocationOptions {
        data_dir: data_dir.clone(),
        first_proposal_file: first_proposal_file.clone(),
        second_proposal_file: second_proposal_file.clone(),
        first_vote_file: first_vote_file.clone(),
        second_vote_file: second_vote_file.clone(),
        evidence_file: evidence_file.clone(),
    })
    .expect("detect signed block vote equivocation");
    assert_eq!(evidence.schema, BLOCK_EQUIVOCATION_EVIDENCE_FILE_SCHEMA);
    assert_eq!(evidence.kind, "block_vote");
    assert_eq!(evidence.block_height, 1);
    assert_eq!(evidence.view, 0);
    assert_eq!(evidence.validator, "validator-0");
    assert_eq!(evidence.first_evidence_kind, "block_vote");
    assert_eq!(evidence.second_evidence_kind, "block_vote");
    assert_ne!(evidence.first_evidence_id, evidence.second_evidence_id);
    assert_eq!(evidence.first_target_kind, "proposal");
    assert_eq!(evidence.second_target_kind, "proposal");
    assert_ne!(evidence.first_target_hash, evidence.second_target_hash);
    assert_eq!(
        read_block_equivocation_evidence_file(&evidence_file)
            .expect("read equivocation evidence"),
        evidence
    );
    let verified = verify_block_vote_equivocation(BlockVoteEquivocationOptions {
        data_dir: data_dir.clone(),
        first_proposal_file: first_proposal_file.clone(),
        second_proposal_file: second_proposal_file.clone(),
        first_vote_file: first_vote_file.clone(),
        second_vote_file: second_vote_file.clone(),
        evidence_file: evidence_file.clone(),
    })
    .expect("verify block vote equivocation evidence");
    assert_eq!(verified, evidence);

    let nonconflicting_error = detect_block_vote_equivocation(BlockVoteEquivocationOptions {
        data_dir: data_dir.clone(),
        first_proposal_file: first_proposal_file.clone(),
        second_proposal_file: first_proposal_file,
        first_vote_file: first_vote_file.clone(),
        second_vote_file: first_vote_file,
        evidence_file: data_dir.join("nonconflicting.block_equivocation_evidence.json"),
    })
    .expect_err("same proposal target is not equivocation");
    assert!(
        nonconflicting_error
            .to_string()
            .contains("conflicting proposal targets"),
        "{nonconflicting_error}"
    );

    std::fs::remove_dir_all(data_dir).expect("cleanup block vote equivocation data");
    std::fs::remove_dir_all(isolated_data_dir)
        .expect("cleanup isolated block vote equivocation data");
}

#[test]
fn cross_view_vote_and_legacy_lock_migration_fail_closed() {
    let data_dir = unique_test_dir("postfiat-cross-view-vote-lock-exploit");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init cross-view exploit test");

    let first_batch_file = data_dir.join("first-cross-view-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfcrossviewfirst000000000000000000000".to_string(),
        amount: 45,
        batch_file: first_batch_file.clone(),
    })
    .expect("create first cross-view batch");
    let second_batch_file = data_dir.join("second-cross-view-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfcrossviewsecond00000000000000000000".to_string(),
        amount: 46,
        batch_file: second_batch_file.clone(),
    })
    .expect("create second cross-view batch");

    let first_proposal_file = data_dir.join("first-cross-view.block_proposal.json");
    let first_proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: first_batch_file.clone(),
        proposal_file: first_proposal_file.clone(),
        view: Some(0),
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose first cross-view batch");

    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let split_key_paths = write_split_validator_key_files(&data_dir, &validator_keys);
    let validator_0_key = split_key_paths
        .iter()
        .find_map(|(node_id, path)| (node_id == "validator-0").then_some(path.clone()))
        .expect("validator-0 split key");
    let first_vote = create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: validator_0_key.clone(),
        validator_id: None,
        batch_file: Some(first_batch_file),
        proposal_file: Some(first_proposal_file),
        timeout_certificate_file: None,
        block_height: Some(first_proposal.block_height),
        vote_file: data_dir.join("first-cross-view.validator-0.block_vote.json"),
    })
    .expect("first proposal vote");

    let timeout_vote_files = split_key_paths
        .iter()
        .take(3)
        .map(|(node_id, key_path)| {
            let vote_file = data_dir.join(format!("cross-view.{node_id}.timeout_vote.json"));
            create_block_timeout_vote(BlockTimeoutVoteOptions {
                data_dir: data_dir.clone(),
                verify_block_log: true,
                key_file: key_path.clone(),
                validator_id: None,
                block_height: first_proposal.block_height,
                view: 1,
                high_qc_id: "fabricated-unresolved-qc".to_string(),
                vote_file: vote_file.clone(),
            })
            .expect("create timeout vote with unresolved high QC");
            vote_file
        })
        .collect::<Vec<_>>();
    let timeout_certificate_file = data_dir.join("cross-view.timeout_certificate.json");
    aggregate_block_timeout_certificate(BlockTimeoutCertificateOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        block_height: first_proposal.block_height,
        view: 1,
        vote_files: timeout_vote_files,
        certificate_file: timeout_certificate_file.clone(),
    })
    .expect("aggregate timeout certificate with unresolved high QC");

    let disabled_proposal_error = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: second_batch_file.clone(),
        proposal_file: data_dir.join("disabled-cross-view.block_proposal.json"),
        view: Some(2),
        timeout_certificate_file: Some(timeout_certificate_file.clone()),
        key_file: None,
        validator_id: None,
    })
    .expect_err("unresolved high-QC evidence must not authorize a nonzero-view proposal");
    assert!(
        disabled_proposal_error
            .to_string()
            .contains("nonzero-view block proposals require activated consensus v2"),
        "{disabled_proposal_error}"
    );

    let lock_dir = data_dir.join("block_proposal_vote_locks");
    let current_lock_path = fs::read_dir(&lock_dir)
        .expect("read current vote-lock directory")
        .map(|entry| entry.expect("read vote-lock entry").path())
        .find(|path| path.extension().and_then(|value| value.to_str()) == Some("json"))
        .expect("current height-wide vote lock");
    let mut legacy_lock: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(&current_lock_path).expect("read current vote lock"),
    )
    .expect("parse current vote lock");
    legacy_lock["schema"] = serde_json::Value::String(
        "postfiat.block_proposal_vote_lock.v1".to_string(),
    );
    let legacy_lock_path = lock_dir.join("1.0.legacy-v1.json");
    fs::write(
        &legacy_lock_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&legacy_lock).expect("serialize legacy vote lock")
        ),
    )
    .expect("write legacy vote lock");
    fs::remove_file(current_lock_path).expect("remove v2 lock to exercise migration scan");

    let second_proposal_file = data_dir.join("second-view-zero.block_proposal.json");
    let second_proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: second_batch_file.clone(),
        proposal_file: second_proposal_file.clone(),
        view: Some(0),
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("build conflicting view-zero proposal for legacy-lock migration test");
    assert_eq!(first_proposal.block_height, second_proposal.block_height);
    assert_ne!(first_proposal.batch_id, second_proposal.batch_id);

    let legacy_lock_error = create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: validator_0_key,
        validator_id: None,
        batch_file: Some(second_batch_file),
        proposal_file: Some(second_proposal_file),
        timeout_certificate_file: None,
        block_height: Some(second_proposal.block_height),
        vote_file: data_dir.join("second-cross-view.validator-0.block_vote.json"),
    })
    .expect_err("legacy view-scoped vote lock must remain binding after upgrade");

    assert_eq!(first_vote.vote.validator, "validator-0");
    assert!(
        legacy_lock_error
            .to_string()
            .contains("conflicting block proposal vote already recorded"),
        "{legacy_lock_error}"
    );
    std::fs::remove_dir_all(data_dir).expect("cleanup cross-view exploit test");
}

#[test]
fn block_proposal_equivocation_evidence_requires_signed_conflicts() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-block-proposal-equivocation-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 2,
    })
    .expect("init block proposal equivocation test");
    let first_batch_file = data_dir.join("first-signed-proposal-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfproposalfirst0000000000000000000000".to_string(),
        amount: 45,
        batch_file: first_batch_file.clone(),
    })
    .expect("create first proposal batch");
    let second_batch_file = data_dir.join("second-signed-proposal-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfproposalsecond000000000000000000000".to_string(),
        amount: 46,
        batch_file: second_batch_file.clone(),
    })
    .expect("create second proposal batch");

    let first_proposal_file = data_dir.join("first-signed.block_proposal.json");
    let first_proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: first_batch_file,
        proposal_file: first_proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: Some(data_dir.join(VALIDATOR_KEYS_FILE)),
        validator_id: Some("validator-1".to_string()),
    })
    .expect("create first signed proposal");
    let second_proposal_file = data_dir.join("second-signed.block_proposal.json");
    let second_proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: second_batch_file,
        proposal_file: second_proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: Some(data_dir.join(VALIDATOR_KEYS_FILE)),
        validator_id: Some("validator-1".to_string()),
    })
    .expect("create second signed proposal");
    assert_eq!(first_proposal.block_height, second_proposal.block_height);
    assert_eq!(first_proposal.view, second_proposal.view);
    assert_eq!(first_proposal.proposer, second_proposal.proposer);
    assert!(first_proposal.signature.is_some());
    assert!(second_proposal.signature.is_some());

    let evidence_file = data_dir.join("validator-1.block_proposal_equivocation.json");
    let evidence = detect_block_proposal_equivocation(BlockProposalEquivocationOptions {
        data_dir: data_dir.clone(),
        first_proposal_file: first_proposal_file.clone(),
        second_proposal_file: second_proposal_file.clone(),
        evidence_file: evidence_file.clone(),
    })
    .expect("detect signed block proposal equivocation");
    assert_eq!(evidence.schema, BLOCK_EQUIVOCATION_EVIDENCE_FILE_SCHEMA);
    assert_eq!(evidence.kind, "block_proposal");
    assert_eq!(evidence.block_height, 1);
    assert_eq!(evidence.view, 0);
    assert_eq!(evidence.validator, "validator-1");
    assert_eq!(evidence.first_evidence_kind, "block_proposal");
    assert_eq!(evidence.second_evidence_kind, "block_proposal");
    assert_ne!(evidence.first_evidence_id, evidence.second_evidence_id);
    assert_eq!(evidence.first_target_kind, "proposal");
    assert_eq!(evidence.second_target_kind, "proposal");
    assert_ne!(evidence.first_target_hash, evidence.second_target_hash);
    assert_eq!(
        read_block_equivocation_evidence_file(&evidence_file)
            .expect("read proposal equivocation evidence"),
        evidence
    );
    let verified = verify_block_proposal_equivocation(BlockProposalEquivocationOptions {
        data_dir: data_dir.clone(),
        first_proposal_file: first_proposal_file.clone(),
        second_proposal_file: second_proposal_file.clone(),
        evidence_file: evidence_file.clone(),
    })
    .expect("verify block proposal equivocation evidence");
    assert_eq!(verified, evidence);
    let mut tampered_evidence = evidence.clone();
    tampered_evidence.evidence_id = "tampered-proposal-equivocation".to_string();
    let tampered_evidence_file = data_dir.join("tampered.block_proposal_equivocation.json");
    write_block_equivocation_evidence_file(&tampered_evidence_file, &tampered_evidence)
        .expect("write tampered proposal equivocation evidence");
    let tampered_error = verify_block_proposal_equivocation(BlockProposalEquivocationOptions {
        data_dir: data_dir.clone(),
        first_proposal_file: first_proposal_file.clone(),
        second_proposal_file: second_proposal_file.clone(),
        evidence_file: tampered_evidence_file,
    })
    .expect_err("tampered proposal equivocation evidence must fail verification");
    assert!(
        tampered_error.to_string().contains("evidence mismatch"),
        "{tampered_error}"
    );

    let unsigned_proposal_file = data_dir.join("unsigned.block_proposal.json");
    let unsigned_proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: data_dir.join("second-signed-proposal-batch.json"),
        proposal_file: unsigned_proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("create unsigned proposal");
    assert!(unsigned_proposal.signature.is_none());
    let unsigned_error = detect_block_proposal_equivocation(BlockProposalEquivocationOptions {
        data_dir: data_dir.clone(),
        first_proposal_file: first_proposal_file.clone(),
        second_proposal_file: unsigned_proposal_file,
        evidence_file: data_dir.join("unsigned.block_proposal_equivocation.json"),
    })
    .expect_err("unsigned proposal must not prove proposal equivocation");
    assert!(
        unsigned_error
            .to_string()
            .contains("requires signed proposals"),
        "{unsigned_error}"
    );

    let nonconflicting_error =
        detect_block_proposal_equivocation(BlockProposalEquivocationOptions {
            data_dir: data_dir.clone(),
            first_proposal_file: first_proposal_file.clone(),
            second_proposal_file: first_proposal_file,
            evidence_file: data_dir.join("nonconflicting.block_proposal_equivocation.json"),
        })
        .expect_err("same signed proposal is not proposal equivocation");
    assert!(
        nonconflicting_error
            .to_string()
            .contains("conflicting proposal targets"),
        "{nonconflicting_error}"
    );

    std::fs::remove_dir_all(data_dir).expect("cleanup block proposal equivocation data");
}

fn aggregate_proposal_certificate_from_split_keys(
    data_dir: &Path,
    split_key_paths: &[(String, PathBuf)],
    batch_kind: &str,
    batch_file: &Path,
    label: &str,
) -> (BlockProposalFile, PathBuf, BlockCertificateFile) {
    let proposal_file = data_dir.join(format!("{label}.block_proposal.json"));
    let proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.to_path_buf(),
        verify_block_log: true,
        batch_kind: Some(batch_kind.to_string()),
        batch_file: batch_file.to_path_buf(),
        proposal_file: proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose ordered batch");
    assert_eq!(proposal.batch_kind, batch_kind);

    let vote_paths = split_key_paths
        .iter()
        .map(|(node_id, split_key_path)| {
            let vote_file = data_dir.join(format!("{label}.{node_id}.block_vote.json"));
            let vote = create_block_vote(BlockVoteOptions {
                data_dir: data_dir.to_path_buf(),
                verify_block_log: true,
                key_file: split_key_path.clone(),
                validator_id: None,
                batch_file: Some(batch_file.to_path_buf()),
                proposal_file: Some(proposal_file.clone()),
                timeout_certificate_file: None,
                block_height: Some(proposal.block_height),
                vote_file: vote_file.clone(),
            })
            .expect("create split proposal vote");
            assert_eq!(&vote.vote.validator, node_id);
            assert!(vote.block_hash.is_none());
            assert!(vote.proposal_hash.is_some());
            vote_file
        })
        .collect::<Vec<_>>();

    let certificate_file = data_dir.join(format!("{label}.block_certificate.json"));
    let certificate = aggregate_block_certificate(BlockCertificateOptions {
        data_dir: data_dir.to_path_buf(),
        verify_block_log: true,
        batch_file: Some(batch_file.to_path_buf()),
        proposal_file: Some(proposal_file),
        timeout_certificate_file: None,
        block_height: Some(proposal.block_height),
        vote_files: vote_paths,
        certificate_file: certificate_file.clone(),
    })
    .expect("aggregate proposal certificate");
    assert!(certificate.block_hash.is_none());
    assert!(certificate.proposal_hash.is_some());
    (proposal, certificate_file, certificate)
}

#[test]
fn unsigned_governance_support_cannot_enter_live_block_proposal() {
    let data_dir = unique_test_dir("postfiat-unsigned-governance-live-admission-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-unsigned-governance-live-admission".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init unsigned governance admission test");

    let validators = local_validator_ids(4).expect("validator ids");
    let amendment_file = data_dir.join("forged-name-support.amendment.json");
    ratify_governance(RatifyGovernanceOptions {
        data_dir: data_dir.clone(),
        validators: validators.clone(),
        support: validators,
        kind: postfiat_types::GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
        value: 2,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: amendment_file.clone(),
    })
    .expect("the legacy artifact builder demonstrates no private key is required");
    let batch_file = data_dir.join("forged-name-support.batch.json");
    create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.clone(),
        amendment_file: Some(amendment_file),
        registry_update_file: None,
        batch_file: batch_file.clone(),
    })
    .expect("construct unsigned governance artifact");

    let error = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_GOVERNANCE.to_string()),
        batch_file,
        proposal_file: data_dir.join("forged-name-support.block_proposal.json"),
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect_err("unsigned governance evidence must be historical-replay-only");
    assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        error.to_string().contains("signed governance authorization"),
        "{error}"
    );
    let direct_apply_error = apply_governance_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: data_dir.join("forged-name-support.batch.json"),
        certificate_file: None,
    })
    .expect_err("direct unsigned governance apply must also be disabled");
    assert_eq!(direct_apply_error.kind(), io::ErrorKind::PermissionDenied);
    assert!(
        NodeStore::new(&data_dir)
            .read_blocks()
            .expect("blocks")
            .blocks
            .is_empty()
    );

    std::fs::remove_dir_all(data_dir).expect("cleanup unsigned governance admission test");
}

#[test]
fn signed_governance_authorizations_from_isolated_validator_keys_enter_live_proposal() {
    let data_dir = unique_test_dir("postfiat-signed-governance-live-admission-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-signed-governance-live-admission".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init signed governance admission test");
    let validators = local_validator_ids(4).expect("validator ids");
    let unsigned_amendment_file = data_dir.join("crypto-policy-2.unsigned.json");
    ratify_governance(RatifyGovernanceOptions {
        data_dir: data_dir.clone(),
        validators: validators.clone(),
        support: validators,
        kind: postfiat_types::GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
        value: 2,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: unsigned_amendment_file.clone(),
    })
    .expect("create governance proposal");
    let validator_keys = read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE))
        .expect("validator keys");
    let split_key_files = write_split_validator_key_files(&data_dir, &validator_keys);
    let authorization_files = split_key_files
        .iter()
        .map(|(validator, key_file)| {
            let authorization_file =
                data_dir.join(format!("{validator}.governance-authorization.json"));
            sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                data_dir: data_dir.clone(),
                amendment_file: unsigned_amendment_file.clone(),
                validator: validator.clone(),
                validator_key_file: key_file.clone(),
                proposal_slot: 1,
                expires_at_height: 8,
                authorization_file: authorization_file.clone(),
            })
            .expect("sign governance authorization");
            authorization_file
        })
        .collect::<Vec<_>>();
    let signed_amendment_file = data_dir.join("crypto-policy-2.signed.json");
    let amendment = assemble_signed_governance_amendment(
        GovernanceAmendmentAssembleOptions {
            data_dir: data_dir.clone(),
            amendment_file: unsigned_amendment_file,
            authorization_files,
            proposal_slot: 1,
            output_file: signed_amendment_file.clone(),
        },
    )
    .expect("assemble signed governance amendment");
    assert_eq!(amendment.signed_authorizations.len(), 4);

    let batch_file = data_dir.join("crypto-policy-2.batch.json");
    create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.clone(),
        amendment_file: Some(signed_amendment_file),
        registry_update_file: None,
        batch_file: batch_file.clone(),
    })
    .expect("create signed governance batch");
    let signed_batch = read_governance_action_batch_file(&batch_file).expect("signed batch");
    let store = NodeStore::new(&data_dir);
    let governance_before = store.read_governance().expect("governance before tamper cases");
    let registry = read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE))
        .expect("registry before tamper cases");
    let assert_rejected = |batch: &GovernanceActionBatch, label: &str| {
        let result = verify_live_signed_governance_batch(
            &store.read_genesis().expect("genesis"),
            &governance_before,
            &registry,
            batch,
            1,
        );
        let error = match result {
            Ok(()) => panic!("{label} must reject"),
            Err(error) => error,
        };
        assert!(!error.to_string().is_empty());
        assert_eq!(
            store.read_governance().expect("governance after rejection"),
            governance_before,
            "{label} must not mutate governance"
        );
    };
    let mut missing = signed_batch.clone();
    missing.amendments[0].signed_authorizations.pop();
    assert_rejected(&missing, "missing authorization");
    let mut duplicate = signed_batch.clone();
    duplicate.amendments[0].signed_authorizations[1] =
        duplicate.amendments[0].signed_authorizations[0].clone();
    assert_rejected(&duplicate, "duplicate validator authorization");
    let mut wrong_registry = signed_batch.clone();
    wrong_registry.amendments[0].signed_authorizations[0].old_registry_root = "00".repeat(48);
    assert_rejected(&wrong_registry, "wrong registry root");
    let mut wrong_epoch = signed_batch.clone();
    wrong_epoch.amendments[0].signed_authorizations[0].committee_epoch = 2;
    assert_rejected(&wrong_epoch, "wrong committee epoch");
    let mut wrong_slot = signed_batch.clone();
    wrong_slot.amendments[0].signed_authorizations[0].proposal_slot = 2;
    assert_rejected(&wrong_slot, "wrong proposal slot");
    let mut expired = signed_batch.clone();
    expired.amendments[0].signed_authorizations[0].expires_at_height = 0;
    assert_rejected(&expired, "expired authorization");
    let mut stale_key = signed_batch.clone();
    stale_key.amendments[0].signed_authorizations[0].signature_hex =
        stale_key.amendments[0].signed_authorizations[1]
            .signature_hex
            .clone();
    assert_rejected(&stale_key, "stale or wrong validator key");
    let mut altered_payload = signed_batch.clone();
    altered_payload.amendments[0].value = 3;
    assert_rejected(&altered_payload, "altered proposal payload");
    let mut wrong_chain_genesis = store.read_genesis().expect("wrong-chain genesis base");
    wrong_chain_genesis.chain_id = "postfiat-wrong-chain".to_string();
    let wrong_chain_error = verify_live_signed_governance_batch(
        &wrong_chain_genesis,
        &governance_before,
        &registry,
        &signed_batch,
        1,
    )
    .expect_err("authorization must not cross chain domains");
    assert!(!wrong_chain_error.to_string().is_empty());
    assert_eq!(
        store
            .read_governance()
            .expect("governance after wrong-chain rejection"),
        governance_before,
        "wrong chain must not mutate governance"
    );
    let proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_GOVERNANCE.to_string()),
        batch_file,
        proposal_file: data_dir.join("crypto-policy-2.block-proposal.json"),
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("signed governance proposal admitted");
    assert_eq!(proposal.block_height, 1);
    assert_eq!(proposal.batch_kind, BATCH_KIND_GOVERNANCE);
    let receipts = apply_governance_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: data_dir.join("crypto-policy-2.batch.json"),
        certificate_file: None,
    })
    .expect("apply signed governance batch");
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted, "{receipts:?}");
    assert_eq!(
        NodeStore::new(&data_dir)
            .read_governance()
            .expect("governance after signed apply")
            .crypto_policy_version,
        2
    );
    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("signed governance block replay verifies");
    std::fs::remove_dir_all(data_dir).expect("cleanup signed governance admission test");
}

#[test]
fn every_live_governance_amendment_kind_uses_the_signed_authorization_boundary() {
    let data_dir = unique_test_dir("postfiat-signed-governance-kind-matrix-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-signed-governance-kind-matrix".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init signed governance kind matrix");
    let validators = local_validator_ids(4).expect("validator ids");
    let validator_keys = read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE))
        .expect("validator keys");
    let split_key_files = write_split_validator_key_files(&data_dir, &validator_keys);
    let cases = [
        (GOVERNANCE_KIND_CRYPTO_POLICY, 2),
        (GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH, 2),
        (GOVERNANCE_KIND_AUTHORITY_MODE, 1),
        (GOVERNANCE_KIND_ORCHARD_POOL_PAUSE, 1),
        (GOVERNANCE_KIND_ORCHARD_POOL_PAUSE, 0),
        (GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE, 1),
        (GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE, 0),
        (GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT, 1000),
        (GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT, 1001),
        (GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT, 1002),
        (GOVERNANCE_KIND_CRYPTO_POLICY, 3),
        (GOVERNANCE_KIND_CRYPTO_POLICY, 2),
    ];
    for (index, (kind, value)) in cases.iter().enumerate() {
        let proposal_slot = (index as u64) + 1;
        let is_explicit_rollback = index + 1 == cases.len();
        let unsigned_amendment_file =
            data_dir.join(format!("{proposal_slot}-{kind}.unsigned.json"));
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: (*kind).to_string(),
            value: *value,
            activation_height: if is_explicit_rollback {
                proposal_slot
            } else {
                0
            },
            veto_until_height: if is_explicit_rollback {
                proposal_slot - 1
            } else {
                0
            },
            paused: false,
            amendment_file: unsigned_amendment_file.clone(),
        })
        .expect("create governance kind proposal");
        let authorization_files = split_key_files
            .iter()
            .map(|(validator, key_file)| {
                let authorization_file = data_dir.join(format!(
                    "{proposal_slot}-{kind}-{validator}.authorization.json"
                ));
                sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                    data_dir: data_dir.clone(),
                    amendment_file: unsigned_amendment_file.clone(),
                    validator: validator.clone(),
                    validator_key_file: key_file.clone(),
                    proposal_slot,
                    expires_at_height: proposal_slot + 8,
                    authorization_file: authorization_file.clone(),
                })
                .expect("sign governance kind authorization");
                authorization_file
            })
            .collect::<Vec<_>>();
        let signed_amendment_file =
            data_dir.join(format!("{proposal_slot}-{kind}.signed.json"));
        assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
            data_dir: data_dir.clone(),
            amendment_file: unsigned_amendment_file,
            authorization_files,
            proposal_slot,
            output_file: signed_amendment_file.clone(),
        })
        .expect("assemble signed governance kind amendment");
        let batch_file = data_dir.join(format!("{proposal_slot}-{kind}.batch.json"));
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(signed_amendment_file),
            registry_update_file: None,
            batch_file: batch_file.clone(),
        })
        .expect("create signed governance kind batch");
        let receipts = apply_governance_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file,
            certificate_file: None,
        })
        .expect("apply signed governance kind batch");
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].accepted, "{kind}: {receipts:?}");
    }
    let governance = NodeStore::new(&data_dir)
        .read_governance()
        .expect("governance after signed kind matrix");
    assert_eq!(governance.crypto_policy_version, 2);
    assert_eq!(governance.bridge_witness_epoch, 2);
    assert_eq!(governance.authority_mode, 1);
    assert!(!governance.orchard_pool_paused);
    assert!(!governance.atomic_swap_paused);
    assert_eq!(governance.bridge_verification_activation_height(), Some(1000));
    assert_eq!(governance.atomic_swap_activation_height(), Some(1001));
    assert_eq!(governance.replicated_state_v2_activation_height(), Some(1002));
    assert_eq!(governance.amendment_rollback_records.len(), 1);
    assert_eq!(governance.amendment_rollback_records[0].previous_value, 3);
    assert_eq!(governance.amendment_rollback_records[0].restored_value, 2);
    let verified = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify signed governance kind matrix replay");
    assert_eq!(verified.block_count, cases.len());
    assert!(verified.verified);
    std::fs::remove_dir_all(data_dir).expect("cleanup signed governance kind matrix");
}

#[test]
fn concurrent_signed_governance_amendments_cannot_cross_slots_after_restart() {
    let data_dir = unique_test_dir("postfiat-concurrent-signed-governance-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-concurrent-signed-governance".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init concurrent signed governance test");
    let validators = local_validator_ids(4).expect("validator ids");
    let validator_keys = read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE))
        .expect("validator keys");
    let split_key_files = write_split_validator_key_files(&data_dir, &validator_keys);
    let build_batch = |label: &str, value: u32| {
        let unsigned_file = data_dir.join(format!("{label}.unsigned.json"));
        ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
            value,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: unsigned_file.clone(),
        })
        .expect("create concurrent amendment");
        let authorization_files = split_key_files
            .iter()
            .map(|(validator, key_file)| {
                let authorization_file =
                    data_dir.join(format!("{label}-{validator}.authorization.json"));
                sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                    data_dir: data_dir.clone(),
                    amendment_file: unsigned_file.clone(),
                    validator: validator.clone(),
                    validator_key_file: key_file.clone(),
                    proposal_slot: 1,
                    expires_at_height: 8,
                    authorization_file: authorization_file.clone(),
                })
                .expect("sign concurrent amendment");
                authorization_file
            })
            .collect::<Vec<_>>();
        let signed_file = data_dir.join(format!("{label}.signed.json"));
        assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
            data_dir: data_dir.clone(),
            amendment_file: unsigned_file,
            authorization_files,
            proposal_slot: 1,
            output_file: signed_file.clone(),
        })
        .expect("assemble concurrent amendment");
        let batch_file = data_dir.join(format!("{label}.batch.json"));
        create_governance_batch(GovernanceBatchOptions {
            data_dir: data_dir.clone(),
            amendment_file: Some(signed_file),
            registry_update_file: None,
            batch_file: batch_file.clone(),
        })
        .expect("create concurrent governance batch");
        batch_file
    };
    let chosen_batch = build_batch("chosen-policy-2", 2);
    let conflicting_batch = build_batch("conflicting-policy-3", 3);
    let chosen_receipts = apply_governance_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: chosen_batch,
        certificate_file: None,
    })
    .expect("apply chosen concurrent amendment");
    assert!(chosen_receipts[0].accepted, "{chosen_receipts:?}");
    let governance_after_chosen = NodeStore::new(&data_dir)
        .read_governance()
        .expect("governance after chosen amendment");
    assert_eq!(governance_after_chosen.crypto_policy_version, 2);

    // Simulate a process restart by discarding all in-memory handles and
    // re-entering through the persisted apply/replay APIs.
    let conflict_error = apply_governance_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: conflicting_batch,
        certificate_file: None,
    })
    .expect_err("same-slot competing amendment must be stale after restart");
    assert!(
        conflict_error
            .to_string()
            .contains("signed authorization binding mismatch"),
        "{conflict_error}"
    );
    assert_eq!(
        NodeStore::new(&data_dir)
            .read_governance()
            .expect("governance after conflicting amendment"),
        governance_after_chosen
    );
    let verified = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify chosen amendment after restart/conflict");
    assert_eq!(verified.block_count, 1);
    assert!(verified.verified);
    std::fs::remove_dir_all(data_dir).expect("cleanup concurrent signed governance test");
}

#[test]
fn conflicting_certified_proposal_cannot_apply_after_height_committed() {
    let data_dir = unique_test_dir("postfiat-conflicting-certified-proposal-test");

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init conflicting certified proposal test");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let split_key_paths = write_split_validator_key_files(&data_dir, &validator_keys);
    let isolated_data_dir = unique_test_dir("postfiat-conflicting-certified-isolated");

    let first_batch_file = data_dir.join("first-conflict.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pffinalityconflictfirst00000000000000".to_string(),
        amount: 51,
        batch_file: first_batch_file.clone(),
    })
    .expect("create first conflicting batch");
    let second_batch_file = data_dir.join("second-conflict.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pffinalityconflictsecond0000000000000".to_string(),
        amount: 52,
        batch_file: second_batch_file.clone(),
    })
    .expect("create second conflicting batch");
    copy_test_dir_recursive(&data_dir, &isolated_data_dir);
    let isolated_split_key_paths = split_key_paths
        .iter()
        .map(|(node_id, path)| {
            (
                node_id.clone(),
                isolated_data_dir.join(path.file_name().expect("split key file name")),
            )
        })
        .collect::<Vec<_>>();

    let (first_proposal, first_certificate_file, first_certificate) =
        aggregate_proposal_certificate_from_split_keys(
            &data_dir,
            &split_key_paths,
            BATCH_KIND_TRANSPARENT,
            &first_batch_file,
            "first-conflict",
        );
    let (second_proposal, second_certificate_file, second_certificate) =
        aggregate_proposal_certificate_from_split_keys(
            &isolated_data_dir,
            &isolated_split_key_paths,
            BATCH_KIND_TRANSPARENT,
            &isolated_data_dir.join("second-conflict.batch.json"),
            "second-conflict",
        );
    assert_eq!(first_proposal.block_height, second_proposal.block_height);
    assert_eq!(first_proposal.view, second_proposal.view);
    assert_ne!(first_certificate.proposal_hash, second_certificate.proposal_hash);

    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: first_batch_file,
        certificate_file: Some(first_certificate_file),
    })
    .expect("apply first certified proposal");
    let second_error = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: second_batch_file,
        certificate_file: Some(second_certificate_file),
    })
    .expect_err("stale conflicting certificate must not apply at the next height");
    assert!(
        second_error
            .to_string()
            .contains("external block certificate height 1 does not match block 2"),
        "{second_error}"
    );
    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify after rejecting conflicting certificate");

    std::fs::remove_dir_all(data_dir).expect("cleanup conflicting certified proposal test");
    std::fs::remove_dir_all(isolated_data_dir)
        .expect("cleanup isolated conflicting certified proposal test");
}

#[test]
fn stale_proposal_vote_cannot_be_reused_for_next_height_certificate() {
    let data_dir = unique_test_dir("postfiat-stale-proposal-vote-test");

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init stale vote test");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let split_key_paths = write_split_validator_key_files(&data_dir, &validator_keys);

    let first_batch_file = data_dir.join("first-stale-vote.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfstalevotefirst0000000000000000000".to_string(),
        amount: 61,
        batch_file: first_batch_file.clone(),
    })
    .expect("create first stale-vote batch");
    let (first_proposal, first_certificate_file, _) =
        aggregate_proposal_certificate_from_split_keys(
            &data_dir,
            &split_key_paths,
            BATCH_KIND_TRANSPARENT,
            &first_batch_file,
            "first-stale-vote",
        );
    let stale_vote_file = data_dir.join("stale.validator-0.block_vote.json");
    create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: data_dir.join("validator-0.validator_keys.json"),
        validator_id: None,
        batch_file: Some(first_batch_file.clone()),
        proposal_file: Some(data_dir.join("first-stale-vote.block_proposal.json")),
        timeout_certificate_file: None,
        block_height: Some(first_proposal.block_height),
        vote_file: stale_vote_file.clone(),
    })
    .expect("create stale proposal vote");

    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: first_batch_file,
        certificate_file: Some(first_certificate_file),
    })
    .expect("apply first stale-vote batch");

    let second_batch_file = data_dir.join("second-stale-vote.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfstalevotesecond000000000000000000".to_string(),
        amount: 62,
        batch_file: second_batch_file.clone(),
    })
    .expect("create second stale-vote batch");
    let second_proposal_file = data_dir.join("second-stale-vote.block_proposal.json");
    let second_proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: second_batch_file.clone(),
        proposal_file: second_proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose second stale-vote batch");
    assert_eq!(second_proposal.block_height, 2);

    let mut second_vote_files = vec![stale_vote_file];
    for (node_id, split_key_path) in split_key_paths.iter().skip(1) {
        let vote_file = data_dir.join(format!("second-stale-vote.{node_id}.block_vote.json"));
        create_block_vote(BlockVoteOptions {
            data_dir: data_dir.clone(),
            verify_block_log: true,
            key_file: split_key_path.clone(),
            validator_id: None,
            batch_file: Some(second_batch_file.clone()),
            proposal_file: Some(second_proposal_file.clone()),
            timeout_certificate_file: None,
            block_height: Some(second_proposal.block_height),
            vote_file: vote_file.clone(),
        })
        .expect("create second proposal vote");
        second_vote_files.push(vote_file);
    }
    let stale_error = aggregate_block_certificate(BlockCertificateOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_file: Some(second_batch_file),
        proposal_file: Some(second_proposal_file),
        timeout_certificate_file: None,
        block_height: Some(second_proposal.block_height),
        vote_files: second_vote_files,
        certificate_file: data_dir.join("second-stale-vote.block_certificate.json"),
    })
    .expect_err("stale height-1 vote must not certify height 2");
    assert!(
        stale_error
            .to_string()
            .contains("block vote height 1 does not match block 2"),
        "{stale_error}"
    );

    std::fs::remove_dir_all(data_dir).expect("cleanup stale vote test");
}

#[test]
fn tampered_parent_or_state_root_proposal_rejects_votes() {
    let data_dir = unique_test_dir("postfiat-tampered-proposal-vote-test");

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init tampered proposal vote test");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    write_split_validator_key_files(&data_dir, &validator_keys);
    let batch_file = data_dir.join("tampered-proposal.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pftamperedproposal000000000000000000".to_string(),
        amount: 71,
        batch_file: batch_file.clone(),
    })
    .expect("create tampered proposal batch");
    let proposal_file = data_dir.join("tampered-proposal.block_proposal.json");
    let proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: batch_file.clone(),
        proposal_file: proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("create base proposal");

    let mut wrong_parent = proposal.clone();
    wrong_parent.parent_hash = "wrong-parent".to_string();
    let wrong_parent_file = data_dir.join("wrong-parent.block_proposal.json");
    write_block_proposal_file(&wrong_parent_file, &wrong_parent)
        .expect("write wrong-parent proposal");
    let wrong_parent_error = create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: data_dir.join("validator-0.validator_keys.json"),
        validator_id: None,
        batch_file: Some(batch_file.clone()),
        proposal_file: Some(wrong_parent_file),
        timeout_certificate_file: None,
        block_height: Some(proposal.block_height),
        vote_file: data_dir.join("wrong-parent.block_vote.json"),
    })
    .expect_err("wrong parent proposal must reject vote");
    assert!(
        wrong_parent_error
            .to_string()
            .contains("block proposal does not match local batch and state"),
        "{wrong_parent_error}"
    );

    let mut wrong_state_root = proposal;
    wrong_state_root.state_root = "wrong-state-root".to_string();
    let wrong_state_root_file = data_dir.join("wrong-state-root.block_proposal.json");
    write_block_proposal_file(&wrong_state_root_file, &wrong_state_root)
        .expect("write wrong-state-root proposal");
    let wrong_state_root_error = create_block_vote(BlockVoteOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        key_file: data_dir.join("validator-0.validator_keys.json"),
        validator_id: None,
        batch_file: Some(batch_file),
        proposal_file: Some(wrong_state_root_file),
        timeout_certificate_file: None,
        block_height: Some(wrong_state_root.block_height),
        vote_file: data_dir.join("wrong-state-root.block_vote.json"),
    })
    .expect_err("wrong state-root proposal must reject vote");
    assert!(
        wrong_state_root_error
            .to_string()
            .contains("block proposal does not match local batch and state"),
        "{wrong_state_root_error}"
    );

    std::fs::remove_dir_all(data_dir).expect("cleanup tampered proposal vote test");
}

#[test]
fn under_quorum_partition_votes_cannot_form_certificate() {
    let data_dir = unique_test_dir("postfiat-under-quorum-partition-test");

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 6,
    })
    .expect("init under-quorum partition test");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let split_key_paths = write_split_validator_key_files(&data_dir, &validator_keys);
    let batch_file = data_dir.join("under-quorum.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfunderquorumpartition00000000000000".to_string(),
        amount: 81,
        batch_file: batch_file.clone(),
    })
    .expect("create under-quorum batch");
    let proposal_file = data_dir.join("under-quorum.block_proposal.json");
    let proposal = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: batch_file.clone(),
        proposal_file: proposal_file.clone(),
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("create under-quorum proposal");
    let vote_files = split_key_paths
        .iter()
        .take(4)
        .map(|(node_id, split_key_path)| {
            let vote_file = data_dir.join(format!("under-quorum.{node_id}.block_vote.json"));
            create_block_vote(BlockVoteOptions {
                data_dir: data_dir.clone(),
                verify_block_log: true,
                key_file: split_key_path.clone(),
                validator_id: None,
                batch_file: Some(batch_file.clone()),
                proposal_file: Some(proposal_file.clone()),
                timeout_certificate_file: None,
                block_height: Some(proposal.block_height),
                vote_file: vote_file.clone(),
            })
            .expect("create under-quorum vote");
            vote_file
        })
        .collect::<Vec<_>>();
    let partition_error = aggregate_block_certificate(BlockCertificateOptions {
        data_dir: data_dir.clone(),
        verify_block_log: true,
        batch_file: Some(batch_file),
        proposal_file: Some(proposal_file),
        timeout_certificate_file: None,
        block_height: Some(proposal.block_height),
        vote_files,
        certificate_file: data_dir.join("under-quorum.block_certificate.json"),
    })
    .expect_err("four of six partition must not certify");
    assert!(
        partition_error
            .to_string()
            .contains("insufficient block votes: got 4, need 5"),
        "{partition_error}"
    );

    std::fs::remove_dir_all(data_dir).expect("cleanup under-quorum partition test");
}

#[test]
fn verify_blocks_replays_historical_registry_after_live_key_rotation() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-registry-history-replay-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 2,
    })
    .expect("init registry replay test");

    let batch_file = data_dir.join("pre-rotation-transfer.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfregistryhistory0000000000000000000001".to_string(),
        amount: 25,
        batch_file: batch_file.clone(),
    })
    .expect("create pre-rotation transfer batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply pre-rotation transfer batch");

    let validators = vec!["validator-0".to_string(), "validator-1".to_string()];
    let registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let genesis_registry_path = data_dir.join(VALIDATOR_REGISTRY_GENESIS_FILE);
    let previous_registry =
        read_validator_registry_file(&registry_path).expect("previous registry");
    let genesis_registry =
        read_validator_registry_file(&genesis_registry_path).expect("genesis registry");
    assert_eq!(genesis_registry, previous_registry);
    let previous_root =
        validator_registry_root(&previous_registry, &validators).expect("previous root");
    let previous_record = previous_registry
        .validators
        .iter()
        .find(|record| record.node_id == "validator-1")
        .expect("validator-1 previous record");
    let previous_entry = ValidatorRegistryEntry {
        node_id: previous_record.node_id.clone(),
        algorithm_id: previous_record.algorithm_id.clone(),
        public_key_hex: previous_record.public_key_hex.clone(),
        active: true,
    };

    let new_key = create_validator_key_record("validator-1".to_string()).expect("new key");
    let new_entry = ValidatorRegistryEntry {
        node_id: new_key.node_id.clone(),
        algorithm_id: new_key.algorithm_id.clone(),
        public_key_hex: new_key.public_key_hex.clone(),
        active: true,
    };
    let mut new_registry = previous_registry.clone();
    let rotation_index = new_registry
        .validators
        .iter()
        .position(|record| record.node_id == "validator-1")
        .expect("validator-1 index");
    new_registry.validators[rotation_index] = ValidatorRegistryRecord {
        node_id: new_key.node_id,
        algorithm_id: new_key.algorithm_id,
        public_key_hex: new_key.public_key_hex,
    };
    let new_root = validator_registry_root(&new_registry, &validators).expect("new root");
    assert_ne!(previous_root, new_root);

    let previous_record_file = data_dir.join("validator-1.previous-record.json");
    let new_record_file = data_dir.join("validator-1.new-record.json");
    atomic_write(
        &previous_record_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&previous_entry).expect("previous entry json")
        ),
    )
    .expect("write previous record");
    atomic_write(
        &new_record_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&new_entry).expect("new entry json")
        ),
    )
    .expect("write new record");

    let update_file = data_dir.join("validator-1-rotate-key.update.json");
    create_validator_registry_update(ValidatorRegistryUpdateOptions {
        data_dir: data_dir.clone(),
        validators: validators.clone(),
        support: validators.clone(),
        activation_height: 3,
        previous_registry_root: previous_root.clone(),
        new_registry_root: new_root.clone(),
        previous_validators: validators.clone(),
        new_validators: validators.clone(),
        operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
        subject_node_id: "validator-1".to_string(),
        previous_record_file: Some(previous_record_file),
        new_record_file: Some(new_record_file),
        update_file: update_file.clone(),
    })
    .expect("create registry update");

    let old_validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("old validator keys");
    let split_old_key_files = write_split_validator_key_files(&data_dir, &old_validator_keys);
    let authorization_files = split_old_key_files
        .iter()
        .map(|(validator, key_file)| {
            let authorization_file =
                data_dir.join(format!("{validator}.registry-rotation-authorization.json"));
            sign_validator_registry_update_authorization(
                ValidatorRegistryAuthorizationSignOptions {
                    data_dir: data_dir.clone(),
                    update_file: update_file.clone(),
                    validator: validator.clone(),
                    validator_key_file: key_file.clone(),
                    proposal_slot: 2,
                    expires_at_height: 8,
                    authorization_file: authorization_file.clone(),
                },
            )
            .expect("old validator signs registry rotation");
            authorization_file
        })
        .collect::<Vec<_>>();
    let signed_update_file = data_dir.join("validator-1-rotate-key.signed.update.json");
    let signed_update = assemble_signed_validator_registry_update(
        ValidatorRegistryUpdateAssembleOptions {
            data_dir: data_dir.clone(),
            update_file,
            authorization_files,
            proposal_slot: 2,
            output_file: signed_update_file.clone(),
        },
    )
    .expect("assemble old-rule-authorized registry rotation");
    assert_eq!(signed_update.signed_authorizations.len(), validators.len());

    let governance_batch_file = data_dir.join("validator-1-rotate-key.governance.json");
    create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.clone(),
        amendment_file: None,
        registry_update_file: Some(signed_update_file),
        batch_file: governance_batch_file.clone(),
    })
    .expect("create registry update governance batch");
    let rotation_receipts = apply_governance_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: governance_batch_file,
        certificate_file: None,
    })
    .expect("apply signed registry update governance batch");
    assert_eq!(rotation_receipts.len(), 1);
    assert!(rotation_receipts[0].accepted, "{rotation_receipts:?}");

    let live_registry_before_activation =
        read_validator_registry_file(&registry_path).expect("live registry before activation");
    let live_root_before_activation =
        validator_registry_root(&live_registry_before_activation, &validators)
            .expect("live root before activation");
    assert_eq!(live_root_before_activation, previous_root);

    let activation_batch_file = data_dir.join("activation-height-transfer.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfregistryhistory0000000000000000000002".to_string(),
        amount: 13,
        batch_file: activation_batch_file.clone(),
    })
    .expect("create activation-height transfer batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: activation_batch_file,
        certificate_file: None,
    })
    .expect("apply activation-height transfer batch");

    let live_registry = read_validator_registry_file(&registry_path).expect("live registry");
    let live_root = validator_registry_root(&live_registry, &validators).expect("live root");
    assert_eq!(live_root, new_root);
    let preserved_genesis_registry = read_validator_registry_file(&genesis_registry_path)
        .expect("preserved genesis registry");
    assert_eq!(preserved_genesis_registry, previous_registry);

    let verified = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify blocks after live key rotation");
    assert_eq!(verified.block_count, 3);
    assert!(verified.verified);

    std::fs::remove_dir_all(data_dir).expect("cleanup registry replay test");
}

#[test]
fn governance_contiguous_admission_live_applies_for_next_block() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-registry-contiguous-admission-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 2,
    })
    .expect("init contiguous admission test");

    let validators = vec!["validator-0".to_string(), "validator-1".to_string()];
    let admitted_validators = vec![
        "validator-0".to_string(),
        "validator-1".to_string(),
        "validator-2".to_string(),
    ];
    let registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let previous_registry =
        read_validator_registry_file(&registry_path).expect("previous registry");
    let previous_root =
        validator_registry_root(&previous_registry, &validators).expect("previous root");

    let admitted_key =
        create_validator_key_record("validator-2".to_string()).expect("admitted key");
    let admitted_key_source_file = data_dir.join("validator-2.admit-key-source.json");
    write_validator_key_file(
        &admitted_key_source_file,
        &ValidatorKeyFile {
            validators: vec![admitted_key.clone()],
        },
    )
    .expect("write admitted key source");
    let admitted_entry = ValidatorRegistryEntry {
        node_id: admitted_key.node_id.clone(),
        algorithm_id: admitted_key.algorithm_id.clone(),
        public_key_hex: admitted_key.public_key_hex.clone(),
        active: true,
    };
    let mut admitted_registry = previous_registry.clone();
    admitted_registry.validators.push(ValidatorRegistryRecord {
        node_id: admitted_key.node_id.clone(),
        algorithm_id: admitted_key.algorithm_id.clone(),
        public_key_hex: admitted_key.public_key_hex.clone(),
    });
    sort_validator_registry_records(&mut admitted_registry.validators);
    let admitted_root =
        validator_registry_root(&admitted_registry, &admitted_validators).expect("new root");
    assert_ne!(previous_root, admitted_root);

    let admitted_record_file = data_dir.join("validator-2.admit-record.json");
    atomic_write(
        &admitted_record_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&admitted_entry).expect("admitted entry json")
        ),
    )
    .expect("write admitted record");

    let registry_update_file = data_dir.join("validator-2-admit.update.json");
    create_validator_registry_update(ValidatorRegistryUpdateOptions {
        data_dir: data_dir.clone(),
        validators: validators.clone(),
        support: validators.clone(),
        activation_height: 1,
        previous_registry_root: previous_root.clone(),
        new_registry_root: admitted_root.clone(),
        previous_validators: validators.clone(),
        new_validators: admitted_validators.clone(),
        operation: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
        subject_node_id: "validator-2".to_string(),
        previous_record_file: None,
        new_record_file: Some(admitted_record_file),
        update_file: registry_update_file.clone(),
    })
    .expect("create admission registry update");

    let amendment_file = data_dir.join("validator-count-3.amendment.json");
    ratify_validator_set(RatifyValidatorSetOptions {
        data_dir: data_dir.clone(),
        validators: validators.clone(),
        support: validators.clone(),
        validator_count: 3,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: amendment_file.clone(),
    })
    .expect("ratify validator count 3");

    let governance_batch_file = data_dir.join("validator-2-admit.governance.json");
    create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.clone(),
        amendment_file: Some(amendment_file),
        registry_update_file: Some(registry_update_file),
        batch_file: governance_batch_file.clone(),
    })
    .expect("create admission governance batch");
    let early_stage_error = stage_validator_key(ValidatorKeyStageOptions {
        data_dir: data_dir.clone(),
        source_key_file: admitted_key_source_file.clone(),
        validator_id: "validator-2".to_string(),
        source_validator_id: None,
        replace: false,
    })
    .expect_err("early key staging must fail before live registry admission");
    assert!(
        early_stage_error
            .to_string()
            .contains("missing validator registry key `validator-2`"),
        "{early_stage_error}"
    );
    let receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: governance_batch_file,
        certificate_file: None,
    })
    .expect("apply admission governance batch");
    assert!(
        receipts.iter().all(|receipt| receipt.accepted),
        "{receipts:?}"
    );

    let store = NodeStore::new(&data_dir);
    let governance = store.read_governance().expect("governance");
    assert_eq!(governance.active_validator_count, 3);
    let live_registry = read_validator_registry_file(&registry_path).expect("live registry");
    let live_root =
        validator_registry_root(&live_registry, &admitted_validators).expect("live root");
    assert_eq!(live_root, admitted_root);

    let stage_report = stage_validator_key(ValidatorKeyStageOptions {
        data_dir: data_dir.clone(),
        source_key_file: admitted_key_source_file,
        validator_id: "validator-2".to_string(),
        source_validator_id: None,
        replace: false,
    })
    .expect("stage admitted validator key");
    assert_eq!(stage_report.action, "added");
    assert!(stage_report.registry_public_key_matched);

    let batch_file = data_dir.join("post-admission-transfer.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfregistryadmission00000000000000000001".to_string(),
        amount: 19,
        batch_file: batch_file.clone(),
    })
    .expect("create post-admission transfer batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply post-admission transfer batch");

    let blocks = store.read_blocks().expect("blocks");
    let latest = blocks.blocks.last().expect("latest block");
    assert_eq!(latest.header.certificate.validators, admitted_validators);
    assert_eq!(latest.header.certificate.registry_root, admitted_root);
    let verified = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify blocks after contiguous admission");
    assert_eq!(verified.block_count, 2);
    assert!(verified.verified);

    std::fs::remove_dir_all(data_dir).expect("cleanup contiguous admission test");
}

#[test]
fn governance_suspend_activates_non_contiguous_validator_list_for_next_block() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-registry-non-contiguous-suspend-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 3,
    })
    .expect("init non-contiguous suspend test");

    let previous_validators = vec![
        "validator-0".to_string(),
        "validator-1".to_string(),
        "validator-2".to_string(),
    ];
    let new_validators = vec!["validator-0".to_string(), "validator-2".to_string()];
    let registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let previous_registry =
        read_validator_registry_file(&registry_path).expect("previous registry");
    let previous_root = validator_registry_root(&previous_registry, &previous_validators)
        .expect("previous root");
    let previous_record = previous_registry
        .validators
        .iter()
        .find(|record| record.node_id == "validator-1")
        .expect("validator-1 record");
    let previous_entry = ValidatorRegistryEntry {
        node_id: previous_record.node_id.clone(),
        algorithm_id: previous_record.algorithm_id.clone(),
        public_key_hex: previous_record.public_key_hex.clone(),
        active: true,
    };
    let suspended_entry = ValidatorRegistryEntry {
        node_id: previous_record.node_id.clone(),
        algorithm_id: previous_record.algorithm_id.clone(),
        public_key_hex: previous_record.public_key_hex.clone(),
        active: false,
    };
    let mut suspended_registry = previous_registry.clone();
    suspended_registry
        .validators
        .retain(|record| record.node_id != "validator-1");
    let new_root =
        validator_registry_root(&suspended_registry, &new_validators).expect("new root");
    assert_ne!(previous_root, new_root);

    let previous_record_file = data_dir.join("validator-1.suspend-previous.json");
    let suspended_record_file = data_dir.join("validator-1.suspend-new.json");
    atomic_write(
        &previous_record_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&previous_entry).expect("previous entry json")
        ),
    )
    .expect("write previous record");
    atomic_write(
        &suspended_record_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&suspended_entry).expect("suspend entry json")
        ),
    )
    .expect("write suspended record");

    let update_file = data_dir.join("validator-1-suspend.update.json");
    create_validator_registry_update(ValidatorRegistryUpdateOptions {
        data_dir: data_dir.clone(),
        validators: previous_validators.clone(),
        support: previous_validators.clone(),
        activation_height: 1,
        previous_registry_root: previous_root,
        new_registry_root: new_root.clone(),
        previous_validators: previous_validators.clone(),
        new_validators: new_validators.clone(),
        operation: VALIDATOR_REGISTRY_OP_SUSPEND.to_string(),
        subject_node_id: "validator-1".to_string(),
        previous_record_file: Some(previous_record_file),
        new_record_file: Some(suspended_record_file),
        update_file: update_file.clone(),
    })
    .expect("create suspend registry update");

    let governance_batch_file = data_dir.join("validator-1-suspend.governance.json");
    create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.clone(),
        amendment_file: None,
        registry_update_file: Some(update_file),
        batch_file: governance_batch_file.clone(),
    })
    .expect("create suspend governance batch");
    let receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: governance_batch_file,
        certificate_file: None,
    })
    .expect("apply suspend governance batch");
    assert!(
        receipts.iter().all(|receipt| receipt.accepted),
        "{receipts:?}"
    );

    let live_registry = read_validator_registry_file(&registry_path)
        .expect("live registry after suspend activation block");
    let live_root =
        validator_registry_root(&live_registry, &new_validators).expect("live root");
    assert_eq!(live_root, new_root);

    let batch_file = data_dir.join("post-suspend-transfer.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfregistrysuspend00000000000000000001".to_string(),
        amount: 21,
        batch_file: batch_file.clone(),
    })
    .expect("create post-suspend transfer batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply post-suspend transfer batch");

    let store = NodeStore::new(&data_dir);
    let governance = store.read_governance().expect("governance");
    assert_eq!(governance.active_validator_count, 2);
    assert_eq!(governance.active_validators, new_validators);
    let blocks = store.read_blocks().expect("blocks");
    let latest = blocks.blocks.last().expect("latest block");
    assert_eq!(latest.header.certificate.validators, new_validators);
    assert_eq!(latest.header.certificate.registry_root, new_root);
    let verified = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("verify blocks after non-contiguous suspend");
    assert_eq!(verified.block_count, 2);
    assert!(verified.verified);

    std::fs::remove_dir_all(data_dir).expect("cleanup non-contiguous suspend test");
}

#[test]
fn external_proposal_certificates_apply_non_transparent_batches() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-external-certificate-apply-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));

    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 2,
    })
    .expect("init external certificate apply test");
    let validator_keys =
        read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    let split_key_paths = write_split_validator_key_files(&data_dir, &validator_keys);
    std::fs::remove_file(data_dir.join(VALIDATOR_KEYS_FILE))
        .expect("remove combined validator keys");

    let validators = vec!["validator-0".to_string(), "validator-1".to_string()];
    let amendment_file = data_dir.join("crypto-policy-v2.json");
    ratify_governance(RatifyGovernanceOptions {
        data_dir: data_dir.clone(),
        validators: validators.clone(),
        support: validators.clone(),
        kind: postfiat_types::GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
        value: 2,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file: amendment_file.clone(),
    })
    .expect("ratify crypto policy");
    let governance_batch_file = data_dir.join("crypto-policy-v2.batch.json");
    create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.clone(),
        amendment_file: Some(amendment_file),
        registry_update_file: None,
        batch_file: governance_batch_file.clone(),
    })
    .expect("create governance batch");
    let governance_error = propose_batch(BatchProposalOptions {
        data_dir: data_dir.clone(),
        batch_file: governance_batch_file,
        batch_kind: Some(BATCH_KIND_GOVERNANCE.to_string()),
        proposal_file: data_dir.join("governance.block_proposal.json"),
        verify_block_log: true,
        view: None,
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect_err("unsigned governance cannot enter the external-certificate path");
    assert_eq!(governance_error.kind(), io::ErrorKind::PermissionDenied);

    let shielded_batch_file = data_dir.join("shielded-migrate.batch.json");
    create_shielded_migrate_batch(ShieldMigrateBatchOptions {
        data_dir: data_dir.clone(),
        note_id: "historical-note-not-present".to_string(),
        target_pool: "asset-orchard-migration".to_string(),
        memo: "external certificate legacy retirement".to_string(),
        batch_file: shielded_batch_file.clone(),
    })
    .expect("create explicit shielded migration batch");
    let (shielded_proposal, shielded_certificate_file, shielded_certificate) =
        aggregate_proposal_certificate_from_split_keys(
            &data_dir,
            &split_key_paths,
            BATCH_KIND_SHIELDED,
            &shielded_batch_file,
            "shielded",
        );
    let shielded_receipts = apply_shielded_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: shielded_batch_file,
        certificate_file: Some(shielded_certificate_file),
    })
    .expect("apply shielded with external certificate");
    assert!(!shielded_receipts[0].accepted, "{shielded_receipts:?}");

    let bridge_batch_file = data_dir.join("bridge-domain.batch.json");
    create_bridge_domain_batch(BridgeDomainBatchOptions {
        data_dir: data_dir.clone(),
        domain_id: "xrpl-mainnet".to_string(),
        name: "XRPL Mainnet".to_string(),
        source_chain: "xrpl".to_string(),
        target_chain: "postfiat-local".to_string(),
        bridge_id: "bridge-xrpl".to_string(),
        door_account: "rDoor".to_string(),
        inbound_cap: 1_000,
        outbound_cap: 500,
        batch_file: bridge_batch_file.clone(),
    })
    .expect("create bridge batch");
    let (bridge_proposal, bridge_certificate_file, bridge_certificate) =
        aggregate_proposal_certificate_from_split_keys(
            &data_dir,
            &split_key_paths,
            BATCH_KIND_BRIDGE,
            &bridge_batch_file,
            "bridge",
        );
    let bridge_receipts = apply_bridge_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: bridge_batch_file,
        certificate_file: Some(bridge_certificate_file),
    })
    .expect("apply bridge with external certificate");
    assert!(bridge_receipts[0].accepted, "{bridge_receipts:?}");

    let block_log = blocks(BlockQueryOptions {
        data_dir: data_dir.clone(),
        from_height: None,
        limit: None,
    })
    .expect("blocks");
    assert_eq!(block_log.len(), 2);
    let expected = [
        (BATCH_KIND_SHIELDED, shielded_proposal, shielded_certificate),
        (BATCH_KIND_BRIDGE, bridge_proposal, bridge_certificate),
    ];
    for (block, (batch_kind, proposal, certificate)) in block_log.iter().zip(expected.iter()) {
        assert_eq!(block.header.batch_kind, *batch_kind);
        assert_eq!(block.header.height, proposal.block_height);
        assert_eq!(block.header.parent_hash, proposal.parent_hash);
        assert_eq!(block.header.batch_id, proposal.batch_id);
        assert_eq!(block.header.state_root, proposal.state_root);
        assert_eq!(block.header.receipt_count, proposal.receipt_count);
        assert_eq!(block.receipt_ids, proposal.receipt_ids);
        assert_eq!(block.header.certificate_id, certificate.certificate_id);
        assert_eq!(block.header.certificate, certificate.certificate);

        let archive = batch_archive(BatchArchiveQueryOptions {
            data_dir: data_dir.clone(),
            batch_kind: Some((*batch_kind).to_string()),
            batch_id: Some(block.header.batch_id.clone()),
            limit: Some(1),
        })
        .expect("batch archive lookup");
        assert_eq!(archive.len(), 1);
        let block_file = data_dir.join(format!("{batch_kind}.block_record.json"));
        let payload_file = data_dir.join(format!("{batch_kind}.archive_payload.json"));
        let reconstructed_certificate_file =
            data_dir.join(format!("{batch_kind}.reconstructed.block_certificate.json"));
        fs::write(
            &block_file,
            serde_json::to_string_pretty(block).expect("serialize block record"),
        )
        .expect("write block record");
        fs::write(&payload_file, archive[0].payload_json.as_bytes())
            .expect("write archived payload");
        let reconstructed =
            reconstruct_block_certificate_from_archive(BlockCertificateFromArchiveOptions {
                data_dir: data_dir.clone(),
                block_file: block_file.clone(),
                batch_file: payload_file,
                certificate_file: reconstructed_certificate_file.clone(),
            })
            .expect("reconstruct certificate from archived block and payload");
        let expected_proposal_hash = block_proposal_hash(proposal).expect("proposal hash");
        assert_eq!(reconstructed.certificate_id, certificate.certificate_id);
        assert_eq!(reconstructed.certificate, certificate.certificate);
        assert_eq!(
            reconstructed.block_hash,
            Some(block.header.block_hash.clone())
        );
        assert_eq!(reconstructed.proposal_hash, Some(expected_proposal_hash));
        assert_eq!(reconstructed.proposal_hash, certificate.proposal_hash);
        assert_eq!(
            read_block_certificate_file(&reconstructed_certificate_file)
                .expect("read reconstructed certificate"),
            reconstructed
        );
    }

    verify_blocks(NodeOptions { data_dir }).expect("verify external certificate blocks");
}

#[test]
fn status_recovers_pending_ordered_commit_journal() {
    let data_dir = unique_test_dir("postfiat-ordered-commit-journal-recover-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    let store = NodeStore::new(&data_dir);
    let mut ledger = store.read_ledger().expect("ledger");
    ledger.accounts[0].balance = ledger.accounts[0].balance.saturating_sub(1);
    let journal = OrderedCommitJournal {
        schema: "postfiat-ordered-commit-journal-v1".to_string(),
        height: 1,
        ledger: Some(ledger.clone()),
        governance: None,
        shielded: None,
        bridge: None,
        receipts: store.read_receipts().expect("receipts"),
        ordered_batches: store.read_ordered_batches().expect("ordered batches"),
        archive: store.read_batch_archive().expect("batch archive"),
        blocks: store.read_blocks().expect("blocks"),
        validator_registry: None,
    };
    store
        .write_ordered_commit_journal(&journal)
        .expect("write journal");

    status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status recovers journal");

    assert_eq!(store.read_ledger().expect("recovered ledger"), ledger);
    assert!(store
        .read_ordered_commit_journal::<OrderedCommitJournal>()
        .expect("journal read")
        .is_none());
}

#[test]
fn status_recovers_pending_ordered_commit_delta_journal() {
    let data_dir = unique_test_dir("postfiat-ordered-commit-delta-journal-recover-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    let store = NodeStore::new(&data_dir);
    let mut ledger = store.read_ledger().expect("ledger");
    ledger.accounts[0].balance = ledger.accounts[0].balance.saturating_sub(7);
    let redemption: NavRedemption = serde_json::from_str(include_str!(
        "fixtures/height_795_nav_redemption_with_u128_unit_scale.json"
    ))
    .expect("parse preserved height-795 NAV redemption");
    ledger.nav_redemptions.push(redemption);
    let archive_entry = BatchArchiveEntry {
        batch_kind: "transparent".to_string(),
        batch_id: "delta-batch-1".to_string(),
        payload_hash: "delta-payload-hash".to_string(),
        payload_json: "{}".to_string(),
    };
    let block = BlockRecord {
        header: BlockHeader {
            height: 1,
            view: 0,
            parent_hash: "genesis".to_string(),
            proposer: "validator-0".to_string(),
            batch_kind: archive_entry.batch_kind.clone(),
            batch_id: archive_entry.batch_id.clone(),
            state_root: "delta-state-root".to_string(),
            bridge_exit_root: None,
            receipt_count: 0,
            certificate_id: "delta-certificate-id".to_string(),
            certificate: BlockCertificate {
                validators: Vec::new(),
                quorum: 0,
                registry_root: String::new(),
                votes: Vec::new(),
            },
            consensus_v2_commit: None,
            block_hash: "delta-block-hash".to_string(),
        },
        receipt_ids: Vec::new(),
        fastpay_pre_state_effects: Vec::new(),
    };
    let journal = OrderedCommitDeltaJournal {
        schema: "postfiat-ordered-commit-delta-journal-v1".to_string(),
        height: 1,
        ledger: Some(ledger.clone()),
        governance: None,
        shielded: None,
        bridge: None,
        receipt_delta: Vec::new(),
        ordered_batch_id: archive_entry.batch_id.clone(),
        archive_entry: archive_entry.clone(),
        block: block.clone(),
        validator_registry: None,
    };
    store
        .write_ordered_commit_journal(&journal)
        .expect("write delta journal");

    status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status recovers delta journal");

    assert_eq!(store.read_ledger().expect("recovered ledger"), ledger);
    assert_eq!(
        store.read_ordered_batches().expect("ordered batches"),
        vec![archive_entry.batch_id.clone()]
    );
    assert_eq!(
        store.read_batch_archive().expect("batch archive"),
        BatchArchive {
            batches: vec![archive_entry]
        }
    );
    assert_eq!(
        store.read_blocks().expect("blocks"),
        BlockLog {
            blocks: vec![block]
        }
    );
    assert!(store
        .read_ordered_commit_journal::<OrderedCommitDeltaJournal>()
        .expect("delta journal read")
        .is_none());
}

#[test]
fn account_tx_index_explicit_build_catches_up_after_archive_prune() {
    let data_dir = unique_test_dir("postfiat-account-tx-index-explicit-build-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    run_once(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("run once");
    let key_file = faucet_key(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("faucet key");
    let first_batch_file = data_dir.join("first-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfrecipient000000000000000000000000000011".to_string(),
        amount: 11,
        batch_file: first_batch_file.clone(),
    })
    .expect("first batch");
    let first_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: first_batch_file,
        certificate_file: None,
    })
    .expect("apply first batch");
    assert_eq!(first_receipts.len(), 1);

    let first_stale_status = account_tx_index_status(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("first account_tx index status before explicit build");
    assert!(!first_stale_status.index_present);
    assert!(!first_stale_status.index_usable);
    assert!(!first_stale_status.disk_index_present);
    assert!(!first_stale_status.disk_index_usable);
    assert_eq!(first_stale_status.indexed_block_count, 0);
    assert_eq!(first_stale_status.indexed_row_count, 0);

    let first_scan_history = account_tx(AccountTxQueryOptions {
        data_dir: data_dir.clone(),
        address: key_file.address.clone(),
        from_height: Some(1),
        to_height: Some(1),
        limit: Some(10),
    })
    .expect("scan faucet account history before explicit index build");
    assert!(!first_scan_history.index_used);
    assert_eq!(first_scan_history.scanned_block_count, 1);
    assert_eq!(first_scan_history.archive_lookup_count, 1);
    assert_eq!(first_scan_history.row_count, 1);
    assert_eq!(first_scan_history.rows[0].tx_id, first_receipts[0].tx_id);

    let first_index_build = rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("explicit first account_tx index build");
    assert!(first_index_build.index_usable);
    assert!(first_index_build.disk_index_usable);
    assert_eq!(first_index_build.indexed_block_count, 1);
    assert_eq!(first_index_build.indexed_row_count, 1);
    let first_index = account_tx_index_status(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("first account_tx index");
    assert!(first_index.index_usable);
    assert!(first_index.disk_index_usable);
    assert_eq!(first_index.indexed_block_count, 1);
    assert_eq!(first_index.indexed_row_count, 1);
    let index_path = account_tx_index_path(&data_dir);
    let first_index_file =
        read_account_tx_index_file(&index_path).expect("read first account_tx index file");

    let second_batch_file = data_dir.join("second-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfrecipient000000000000000000000000000012".to_string(),
        amount: 12,
        batch_file: second_batch_file.clone(),
    })
    .expect("second batch");
    let second_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: second_batch_file,
        certificate_file: None,
    })
    .expect("apply second batch");
    assert_eq!(second_receipts.len(), 1);
    let second_stale_index = account_tx_index_status(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("second account_tx index status before explicit catch-up");
    assert!(second_stale_index.index_present);
    assert!(!second_stale_index.index_usable);
    assert_eq!(
        second_stale_index.reason.as_deref(),
        Some("account_tx index tip hash is stale")
    );
    assert!(second_stale_index.disk_index_present);
    assert!(!second_stale_index.disk_index_usable);
    assert_eq!(
        second_stale_index.disk_index_reason.as_deref(),
        Some("account_tx disk index tip hash is stale")
    );
    let second_scan_history = account_tx(AccountTxQueryOptions {
        data_dir: data_dir.clone(),
        address: key_file.address.clone(),
        from_height: Some(1),
        to_height: Some(2),
        limit: Some(10),
    })
    .expect("scan faucet account history with stale index");
    assert!(!second_scan_history.index_used);
    assert_eq!(second_scan_history.scanned_block_count, 2);
    assert_eq!(second_scan_history.archive_lookup_count, 2);
    assert_eq!(second_scan_history.row_count, 2);
    assert_eq!(second_scan_history.rows[0].tx_id, first_receipts[0].tx_id);
    assert_eq!(second_scan_history.rows[1].tx_id, second_receipts[0].tx_id);

    let store = NodeStore::new(&data_dir);
    let mut archive = store.read_batch_archive().expect("batch archive");
    assert_eq!(archive.batches.len(), 2);
    archive.batches.remove(0);
    store
        .write_batch_archive(&archive)
        .expect("prune historical archive payload");
    let second_index_build = rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("explicit catch-up should extend stale index after archive prune");
    assert!(second_index_build.index_usable);
    assert!(second_index_build.disk_index_usable);
    assert_eq!(second_index_build.indexed_block_count, 2);
    assert_eq!(second_index_build.indexed_row_count, 2);
    write_account_tx_index_file(&index_path, &first_index_file)
        .expect("restore stale first-block account_tx index");

    let third_batch_file = data_dir.join("third-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfrecipient000000000000000000000000000013".to_string(),
        amount: 13,
        batch_file: third_batch_file.clone(),
    })
    .expect("third batch");
    let third_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: third_batch_file,
        certificate_file: None,
    })
    .expect("apply third batch");
    assert_eq!(third_receipts.len(), 1);

    let third_stale_index = account_tx_index_status(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("third account_tx index status before explicit catch-up");
    assert!(third_stale_index.index_present);
    assert!(!third_stale_index.index_usable);
    assert_eq!(
        third_stale_index.reason.as_deref(),
        Some("account_tx index tip hash is stale")
    );

    let incremental_index_build = rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("explicit third account_tx index catch-up");
    assert!(incremental_index_build.index_usable);
    assert!(incremental_index_build.disk_index_usable);
    assert_eq!(incremental_index_build.indexed_block_count, 3);
    assert_eq!(incremental_index_build.indexed_row_count, 3);

    let incremental_index = account_tx_index_status(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("incremental account_tx index");
    assert!(incremental_index.index_usable);
    assert!(incremental_index.disk_index_present);
    assert!(incremental_index.disk_index_usable);
    assert_eq!(
        incremental_index.disk_index_path,
        "account_tx_index_meta.json"
    );
    assert!(incremental_index.disk_account_shard_count >= 4);
    assert_eq!(incremental_index.indexed_block_count, 3);
    assert_eq!(incremental_index.indexed_row_count, 3);
    assert_ne!(incremental_index.tip_hash, first_index.tip_hash);

    fs::remove_file(account_tx_index_path(&data_dir)).expect("remove aggregate account_tx index");
    let disk_only_index = account_tx_index_status(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("disk-only account_tx index status");
    assert!(!disk_only_index.index_present);
    assert!(!disk_only_index.index_usable);
    assert!(disk_only_index.disk_index_present);
    assert!(disk_only_index.disk_index_usable);
    assert!(disk_only_index.disk_account_shard_count >= 4);

    let faucet_history = account_tx(AccountTxQueryOptions {
        data_dir,
        address: key_file.address,
        from_height: Some(1),
        to_height: Some(3),
        limit: Some(10),
    })
    .expect("indexed faucet account history");
    assert!(faucet_history.index_used);
    assert_eq!(faucet_history.archive_lookup_count, 0);
    assert_eq!(faucet_history.row_count, 3);
    assert_eq!(faucet_history.rows[0].tx_id, first_receipts[0].tx_id);
    assert_eq!(faucet_history.rows[1].tx_id, second_receipts[0].tx_id);
    assert_eq!(faucet_history.rows[2].tx_id, third_receipts[0].tx_id);
}

#[test]
fn missing_chain_tip_reconstructs_on_next_commit() {
    let data_dir = unique_test_dir("postfiat-chain-tip-reconstruct-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");

    let first_batch_file = data_dir.join("first-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfchaintiprecipient000000000000000001".to_string(),
        amount: 11,
        batch_file: first_batch_file.clone(),
    })
    .expect("first batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: first_batch_file,
        certificate_file: None,
    })
    .expect("apply first batch");

    let store = NodeStore::new(&data_dir);
    let first_tip = store.read_chain_tip().expect("first chain tip");
    assert_eq!(first_tip.height, 1);
    fs::remove_file(data_dir.join("chain_tip.json")).expect("remove chain tip");

    let second_batch_file = data_dir.join("second-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfchaintiprecipient000000000000000002".to_string(),
        amount: 12,
        batch_file: second_batch_file.clone(),
    })
    .expect("second batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: second_batch_file,
        certificate_file: None,
    })
    .expect("apply second batch after tip reconstruction");

    let status_report = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status");
    let reconstructed_tip = store.read_chain_tip().expect("reconstructed chain tip");
    assert_eq!(reconstructed_tip.height, 2);
    assert_eq!(reconstructed_tip.block_hash, status_report.block_tip_hash);
    assert_eq!(reconstructed_tip.state_root, status_report.state_root);
}

#[test]
fn corrupt_chain_tip_fails_closed_for_commit() {
    let data_dir = unique_test_dir("postfiat-chain-tip-corrupt-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");

    let first_batch_file = data_dir.join("first-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfchaintiprecipient000000000000000011".to_string(),
        amount: 21,
        batch_file: first_batch_file.clone(),
    })
    .expect("first batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: first_batch_file,
        certificate_file: None,
    })
    .expect("apply first batch");

    let store = NodeStore::new(&data_dir);
    let mut corrupt_tip = store.read_chain_tip().expect("chain tip");
    corrupt_tip.schema = "postfiat-chain-tip-corrupt-test".to_string();
    store
        .write_chain_tip(&corrupt_tip)
        .expect("write corrupt chain tip");

    let second_batch_file = data_dir.join("second-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: "pfchaintiprecipient000000000000000012".to_string(),
        amount: 22,
        batch_file: second_batch_file.clone(),
    })
    .expect("second batch");
    let error = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: second_batch_file,
        certificate_file: None,
    })
    .expect_err("corrupt chain tip must fail closed");
    assert!(
        error.to_string().contains("unsupported chain tip schema"),
        "{error}"
    );

    let block_log = NodeStore::new(&data_dir)
        .read_blocks()
        .expect("block log after corrupt tip rejection");
    assert_eq!(block_log.len(), 1);
}

#[test]
fn status_reports_complete_active_nav_profile_configuration() {
    let data_dir = unique_test_dir("postfiat-status-active-nav-profile-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    let store = NodeStore::new(&data_dir);
    let mut ledger = store.read_ledger().expect("ledger");
    let profile = NavProofProfile::new_with_bridge_observer_min_confirmations(
        "issuer",
        "multi-fetch-quorum",
        "vault_bridge:42161",
        100,
        5,
        120,
        30,
        7,
        2,
        10,
        6,
        "42".repeat(48),
        "",
        "",
        0,
        0,
    )
    .expect("profile");
    let mut asset = NavTrackedAsset::new(
        "34".repeat(48),
        "issuer",
        "reserve-operator",
        profile.profile_id.clone(),
        "USDC",
        "redemption-account",
    )
    .expect("NAV asset");
    asset.finalized_epoch = 9;
    asset.nav_per_unit = 8_201_021;
    asset.finalized_reserve_packet_hash = "55".repeat(48);
    ledger.nav_proof_profiles.push(profile.clone());
    ledger.nav_assets.push(asset.clone());
    store.write_ledger(&ledger).expect("write ledger");

    let report = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("status");
    assert_eq!(report.active_nav_profiles.len(), 1);
    let active = &report.active_nav_profiles[0];
    assert_eq!(active.asset_id, asset.asset_id);
    assert_eq!(active.profile_id, profile.profile_id);
    assert_eq!(active.bridge_observer_min_confirmations, 6);
    assert_eq!(active.min_attestations, 2);
    assert_eq!(active.max_snapshot_age_blocks, 100);
    assert_eq!(active.finalized_epoch, 9);
    assert_eq!(active.nav_per_unit, 8_201_021);
    assert_eq!(active.finalized_reserve_packet_hash, "55".repeat(48));
    fs::remove_dir_all(data_dir).expect("cleanup");
}
