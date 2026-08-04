use super::*;
use super::consensus_history::write_split_validator_key_files;

// AR-03 canonical input vector (recovery spec section 6.1). Production BFT
// quorum is `(2n / 3) + 1`, so three registered validators require 3-of-3 and
// cannot tolerate an offline validator. The smallest topology that exhibits
// quorum-first commit with one intentionally offline validator is four
// registered active validators with quorum 3.
const AR03_CHAIN_ID: &str = "postfiat-local";
const AR03_VALIDATOR_COUNT: u32 = 4;
const AR03_OFFLINE_VALIDATOR: &str = "validator-3";
const AR03_BLOCK_ONE_AMOUNT: u64 = 41;
const AR03_BLOCK_TWO_AMOUNT: u64 = 42;

// AR-04 canonical input vector (recovery spec section 6.1, AR-04).
const AR04_CHAIN_ID: &str = "postfiat-local";
const AR04_BLOCK_ONE_AMOUNT: u64 = 43;

fn copy_recovery_test_dir(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("create recovery validator directory");
    for entry in std::fs::read_dir(source).expect("read recovery seed directory") {
        let entry = entry.expect("read recovery seed entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        if entry
            .file_type()
            .expect("read recovery seed entry type")
            .is_dir()
        {
            copy_recovery_test_dir(&source_path, &destination_path);
        } else {
            std::fs::copy(&source_path, &destination_path).expect("copy recovery seed entry");
        }
    }
}

fn write_replay_block_file(path: &Path, block: &BlockRecord) {
    std::fs::write(
        path,
        serde_json::to_string_pretty(block)
            .expect("serialize replay block record")
            .as_bytes(),
    )
    .expect("write replay block record");
}

// Stage exactly what rpc_catch_up_preflight_blocks stages before the
// certified-delta apply: the canonical archived batch payload text, the
// external certificate reconstructed from the archived block, and the
// canonical replay block record.
fn stage_certified_catch_up_artifacts(
    source_dir: &Path,
    destination_dir: &Path,
    work_dir: &Path,
    height: u64,
    block: &BlockRecord,
) -> (PathBuf, PathBuf, PathBuf) {
    let height_dir = work_dir.join(format!("height-{height}"));
    std::fs::create_dir_all(&height_dir).expect("create catch-up staging directory");
    let block_file = height_dir.join("block.json");
    let batch_file = height_dir.join("batch.json");
    let certificate_file = height_dir.join("block-certificate.json");
    write_replay_block_file(&block_file, block);
    let archives = batch_archive(BatchArchiveQueryOptions {
        data_dir: source_dir.to_path_buf(),
        batch_kind: Some(block.header.batch_kind.clone()),
        batch_id: Some(block.header.batch_id.clone()),
        limit: Some(1),
    })
    .expect("query source batch archive");
    assert_eq!(archives.len(), 1, "source archive must hold the block batch");
    let archive = &archives[0];
    assert_eq!(archive.batch_kind, block.header.batch_kind);
    assert_eq!(archive.batch_id, block.header.batch_id);
    std::fs::write(&batch_file, archive.payload_json.as_bytes())
        .expect("write archived batch payload");
    reconstruct_block_certificate_from_archive(BlockCertificateFromArchiveOptions {
        data_dir: destination_dir.to_path_buf(),
        block_file: block_file.clone(),
        batch_file: batch_file.clone(),
        certificate_file: certificate_file.clone(),
    })
    .expect("reconstruct external certificate from the archived block");
    (block_file, batch_file, certificate_file)
}

fn status_tuple(data_dir: &Path) -> (u64, String, String) {
    let report = status(NodeOptions {
        data_dir: data_dir.to_path_buf(),
    })
    .expect("status for convergence tuple");
    (
        report.block_height,
        report.block_tip_hash,
        report.state_root,
    )
}

#[test]
fn ar03_quorum_first_commit_with_offline_validator_converges_after_recovery() {
    assert_eq!(
        bft_quorum_threshold(AR03_VALIDATOR_COUNT as usize).expect("quorum for four validators"),
        3,
        "AR-03 requires quorum 3 for four registered validators"
    );
    assert_eq!(
        bft_quorum_threshold(3).expect("quorum for three validators"),
        3,
        "three registered validators require 3-of-3, so AR-03 cannot use a three-validator set"
    );

    let root_dir = std::env::temp_dir().join(format!(
        "postfiat-ar03-quorum-first-recovery-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let source_dir = root_dir.join("validator-0");
    let online_peer_dirs = [root_dir.join("validator-1"), root_dir.join("validator-2")];
    let offline_dir = root_dir.join(AR03_OFFLINE_VALIDATOR);

    init(InitOptions {
        data_dir: source_dir.clone(),
        chain_id: AR03_CHAIN_ID.to_string(),
        node_id: "validator-0".to_string(),
        validator_count: AR03_VALIDATOR_COUNT,
    })
    .expect("init four-validator AR-03 source");

    // Snapshot every validator directory at genesis. validator-3 is held at
    // this state while the quorum commits without it.
    for data_dir in online_peer_dirs
        .iter()
        .chain(std::iter::once(&offline_dir))
    {
        copy_recovery_test_dir(&source_dir, data_dir);
    }

    let validator_keys =
        read_validator_key_file(&source_dir.join(VALIDATOR_KEYS_FILE)).expect("validator keys");
    assert_eq!(
        validator_keys.validators.len(),
        AR03_VALIDATOR_COUNT as usize
    );
    let split_key_paths = write_split_validator_key_files(&source_dir, &validator_keys);
    let online_key_paths = split_key_paths
        .iter()
        .filter(|(node_id, _)| node_id != AR03_OFFLINE_VALIDATOR)
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(online_key_paths.len(), 3);

    // Block 1: quorum-first commit. Only validator-0, validator-1, and
    // validator-2 vote; validator-3 is intentionally offline and contributes
    // no vote, proposal, or key material to this commit.
    let block_one_batch_file = source_dir.join("ar03-block-1.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: source_dir.clone(),
        key_file: None,
        to: format!("pf{:0<38}", "ar03quorumfirstcommitblockone"),
        amount: AR03_BLOCK_ONE_AMOUNT,
        batch_file: block_one_batch_file.clone(),
    })
    .expect("create block-one transfer batch");
    let block_one_proposal_file = source_dir.join("ar03-block-1.block_proposal.json");
    let block_one_proposal = propose_batch(BatchProposalOptions {
        data_dir: source_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: block_one_batch_file.clone(),
        proposal_file: block_one_proposal_file.clone(),
        view: Some(0),
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose block one");
    assert_eq!(block_one_proposal.block_height, 1);

    let block_one_vote_files = online_key_paths
        .iter()
        .map(|(node_id, split_key_path)| {
            let vote_file = root_dir.join(format!("ar03-block-1.{node_id}.block_vote.json"));
            let vote = create_block_vote(BlockVoteOptions {
                data_dir: source_dir.clone(),
                verify_block_log: true,
                key_file: split_key_path.clone(),
                validator_id: None,
                batch_file: Some(block_one_batch_file.clone()),
                proposal_file: Some(block_one_proposal_file.clone()),
                timeout_certificate_file: None,
                block_height: Some(block_one_proposal.block_height),
                vote_file: vote_file.clone(),
            })
            .expect("create online validator block-one vote");
            assert_eq!(&vote.vote.validator, node_id);
            assert_ne!(vote.vote.validator, AR03_OFFLINE_VALIDATOR);
            vote_file
        })
        .collect::<Vec<_>>();
    assert_eq!(block_one_vote_files.len(), 3);

    let block_one_certificate_file = source_dir.join("ar03-block-1.block_certificate.json");
    let block_one_certificate = aggregate_block_certificate(BlockCertificateOptions {
        data_dir: source_dir.clone(),
        verify_block_log: true,
        batch_file: Some(block_one_batch_file.clone()),
        proposal_file: Some(block_one_proposal_file.clone()),
        timeout_certificate_file: None,
        block_height: Some(block_one_proposal.block_height),
        vote_files: block_one_vote_files,
        certificate_file: block_one_certificate_file.clone(),
    })
    .expect("aggregate three-of-four block-one certificate");
    assert_eq!(block_one_certificate.certificate.validators.len(), 4);
    assert_eq!(block_one_certificate.certificate.quorum, 3);
    assert_eq!(block_one_certificate.certificate.votes.len(), 3);
    assert!(
        block_one_certificate
            .certificate
            .votes
            .iter()
            .all(|vote| vote.validator != AR03_OFFLINE_VALIDATOR),
        "offline validator must not contribute a vote to the first commit"
    );

    apply_batch(ApplyBatchOptions {
        data_dir: source_dir.clone(),
        batch_file: block_one_batch_file.clone(),
        certificate_file: Some(block_one_certificate_file.clone()),
    })
    .expect("commit block one with the three-validator quorum");
    let source_blocks = blocks(BlockQueryOptions {
        data_dir: source_dir.clone(),
        from_height: None,
        limit: None,
    })
    .expect("source blocks after block one");
    let block_one = source_blocks
        .iter()
        .find(|block| block.header.height == 1)
        .expect("committed block one")
        .clone();
    assert_eq!(
        block_one.header.certificate, block_one_certificate.certificate,
        "committed block one must carry the exact three-vote certificate"
    );
    // Propagation and recovery use the real authenticated certified path: the
    // archived payload, reconstructed external certificate, and canonical
    // replay block record, staged exactly like rpc-catch-up-certified-delta.
    for data_dir in online_peer_dirs
        .iter()
        .chain(std::iter::once(&offline_dir))
    {
        let staging_dir = root_dir.join(format!(
            "catch-up-{}",
            data_dir.file_name().expect("validator dir name").to_string_lossy()
        ));
        let (block_file, batch_file, certificate_file) = stage_certified_catch_up_artifacts(
            &source_dir,
            data_dir,
            &staging_dir,
            1,
            &block_one,
        );
        apply_batch_with_replay(
            ApplyBatchOptions {
                data_dir: data_dir.clone(),
                batch_file,
                certificate_file: Some(certificate_file),
            },
            Some(block_file),
        )
        .unwrap_or_else(|error| {
            panic!(
                "certified propagation of block one to {} failed: {error}",
                data_dir.display()
            )
        });
    }
    assert_eq!(status_tuple(&offline_dir).0, 1);

    // Block 2: the recovered offline validator signs from its own restored
    // directory and rejoins the full four-validator committee.
    let block_two_batch_file = source_dir.join("ar03-block-2.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: source_dir.clone(),
        key_file: None,
        to: format!("pf{:0<38}", "ar03quorumfirstcommitblocktwo"),
        amount: AR03_BLOCK_TWO_AMOUNT,
        batch_file: block_two_batch_file.clone(),
    })
    .expect("create block-two transfer batch");
    let block_two_proposal_file = source_dir.join("ar03-block-2.block_proposal.json");
    let block_two_proposal = propose_batch(BatchProposalOptions {
        data_dir: source_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: block_two_batch_file.clone(),
        proposal_file: block_two_proposal_file.clone(),
        view: Some(0),
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose block two");
    assert_eq!(block_two_proposal.block_height, 2);

    let mut block_two_vote_files = online_key_paths
        .iter()
        .map(|(node_id, split_key_path)| {
            let vote_file = root_dir.join(format!("ar03-block-2.{node_id}.block_vote.json"));
            let vote = create_block_vote(BlockVoteOptions {
                data_dir: source_dir.clone(),
                verify_block_log: true,
                key_file: split_key_path.clone(),
                validator_id: None,
                batch_file: Some(block_two_batch_file.clone()),
                proposal_file: Some(block_two_proposal_file.clone()),
                timeout_certificate_file: None,
                block_height: Some(block_two_proposal.block_height),
                vote_file: vote_file.clone(),
            })
            .expect("create online validator block-two vote");
            assert_eq!(&vote.vote.validator, node_id);
            vote_file
        })
        .collect::<Vec<_>>();
    let recovered_vote_file = root_dir.join("ar03-block-2.validator-3.block_vote.json");
    let recovered_vote = create_block_vote(BlockVoteOptions {
        data_dir: offline_dir.clone(),
        verify_block_log: true,
        key_file: offline_dir.join(VALIDATOR_KEYS_FILE),
        validator_id: Some(AR03_OFFLINE_VALIDATOR.to_string()),
        batch_file: Some(block_two_batch_file.clone()),
        proposal_file: Some(block_two_proposal_file.clone()),
        timeout_certificate_file: None,
        block_height: Some(block_two_proposal.block_height),
        vote_file: recovered_vote_file.clone(),
    })
    .expect("recovered validator creates its block-two vote from its restored directory");
    assert_eq!(recovered_vote.vote.validator, AR03_OFFLINE_VALIDATOR);
    block_two_vote_files.push(recovered_vote_file);
    assert_eq!(block_two_vote_files.len(), 4);

    let block_two_certificate_file = source_dir.join("ar03-block-2.block_certificate.json");
    let block_two_certificate = aggregate_block_certificate(BlockCertificateOptions {
        data_dir: source_dir.clone(),
        verify_block_log: true,
        batch_file: Some(block_two_batch_file.clone()),
        proposal_file: Some(block_two_proposal_file.clone()),
        timeout_certificate_file: None,
        block_height: Some(block_two_proposal.block_height),
        vote_files: block_two_vote_files,
        certificate_file: block_two_certificate_file.clone(),
    })
    .expect("aggregate four-of-four block-two certificate");
    assert_eq!(block_two_certificate.certificate.validators.len(), 4);
    assert_eq!(block_two_certificate.certificate.quorum, 3);
    assert_eq!(block_two_certificate.certificate.votes.len(), 4);
    assert!(
        block_two_certificate
            .certificate
            .votes
            .iter()
            .any(|vote| vote.validator == AR03_OFFLINE_VALIDATOR),
        "recovered validator must vote in the post-recovery commit"
    );

    apply_batch(ApplyBatchOptions {
        data_dir: source_dir.clone(),
        batch_file: block_two_batch_file.clone(),
        certificate_file: Some(block_two_certificate_file.clone()),
    })
    .expect("commit block two with the full committee");
    let source_blocks = blocks(BlockQueryOptions {
        data_dir: source_dir.clone(),
        from_height: None,
        limit: None,
    })
    .expect("source blocks after block two");
    let block_two = source_blocks
        .iter()
        .find(|block| block.header.height == 2)
        .expect("committed block two")
        .clone();
    for data_dir in online_peer_dirs
        .iter()
        .chain(std::iter::once(&offline_dir))
    {
        let staging_dir = root_dir.join(format!(
            "catch-up-{}",
            data_dir.file_name().expect("validator dir name").to_string_lossy()
        ));
        let (block_file, batch_file, certificate_file) = stage_certified_catch_up_artifacts(
            &source_dir,
            data_dir,
            &staging_dir,
            2,
            &block_two,
        );
        apply_batch_with_replay(
            ApplyBatchOptions {
                data_dir: data_dir.clone(),
                batch_file,
                certificate_file: Some(certificate_file),
            },
            Some(block_file),
        )
        .unwrap_or_else(|error| {
            panic!(
                "certified propagation of block two to {} failed: {error}",
                data_dir.display()
            )
        });
    }

    // Terminal convergence: all four validators report the identical height,
    // tip, and state root, and every certified history verifies.
    let expected_tuple = status_tuple(&source_dir);
    assert_eq!(expected_tuple.0, 2);
    for data_dir in online_peer_dirs
        .iter()
        .chain(std::iter::once(&offline_dir))
    {
        assert_eq!(
            status_tuple(data_dir),
            expected_tuple,
            "validator directory {} must converge on the source height, tip, and state root",
            data_dir.display()
        );
    }
    for data_dir in std::iter::once(&source_dir)
        .chain(online_peer_dirs.iter())
        .chain(std::iter::once(&offline_dir))
    {
        let verified = verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("verify certified history");
        assert_eq!(verified.block_count, 2);
        assert!(verified.verified);
    }

    std::fs::remove_dir_all(root_dir).expect("cleanup AR-03 recovery test");
}

fn mutate_hex_digest(digest: &str) -> String {
    let replacement = if digest.starts_with('0') { '1' } else { '0' };
    let mut mutated = digest.to_string();
    mutated.replace_range(..1, &replacement.to_string());
    assert_ne!(mutated, digest);
    assert_eq!(mutated.len(), digest.len());
    mutated
}

#[test]
fn ar04_authenticated_catchup_requires_pinned_height_tip_and_state_root() {
    let root_dir = std::env::temp_dir().join(format!(
        "postfiat-ar04-pinned-catch-up-test-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    let source_dir = root_dir.join("validator-0");
    let genesis_seed_dir = root_dir.join("genesis-seed");

    init(InitOptions {
        data_dir: source_dir.clone(),
        chain_id: AR04_CHAIN_ID.to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init AR-04 source");
    copy_recovery_test_dir(&source_dir, &genesis_seed_dir);

    let batch_file = source_dir.join("ar04-block-1.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: source_dir.clone(),
        key_file: None,
        to: format!("pf{:0<38}", "ar04pinnedcatchupblockone"),
        amount: AR04_BLOCK_ONE_AMOUNT,
        batch_file: batch_file.clone(),
    })
    .expect("create AR-04 transfer batch");
    apply_batch(ApplyBatchOptions {
        data_dir: source_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("commit AR-04 block one on the source");
    let source_blocks = blocks(BlockQueryOptions {
        data_dir: source_dir.clone(),
        from_height: None,
        limit: None,
    })
    .expect("source blocks");
    let block_one = source_blocks
        .iter()
        .find(|block| block.header.height == 1)
        .expect("committed AR-04 block one")
        .clone();

    let pin = ExpectedBatchCommitIdentity {
        block_height: block_one.header.height,
        block_hash: block_one.header.block_hash.clone(),
        state_root: block_one.header.state_root.clone(),
        certificate_id: block_one.header.certificate_id.clone(),
    };

    // Adversarial vectors: each axis mutates exactly one pinned field and is
    // applied to an independent genesis clone of the destination.
    let wrong_height_pin = ExpectedBatchCommitIdentity {
        block_height: pin.block_height + 1,
        ..pin.clone()
    };
    let wrong_tip_pin = ExpectedBatchCommitIdentity {
        block_hash: mutate_hex_digest(&pin.block_hash),
        ..pin.clone()
    };
    let wrong_root_pin = ExpectedBatchCommitIdentity {
        state_root: mutate_hex_digest(&pin.state_root),
        ..pin.clone()
    };
    let axes: [(&str, ExpectedBatchCommitIdentity, String); 3] = [
        (
            "height",
            wrong_height_pin.clone(),
            format!(
                "expected height {} hash {} root {}",
                wrong_height_pin.block_height, wrong_height_pin.block_hash, wrong_height_pin.state_root
            ),
        ),
        (
            "tip",
            wrong_tip_pin.clone(),
            format!("hash {}", wrong_tip_pin.block_hash),
        ),
        (
            "root",
            wrong_root_pin.clone(),
            format!("root {}", wrong_root_pin.state_root),
        ),
    ];

    for (axis, mutated_pin, expected_message_fragment) in &axes {
        let destination_dir = root_dir.join(format!("reject-{axis}"));
        copy_recovery_test_dir(&genesis_seed_dir, &destination_dir);
        let staging_dir = root_dir.join(format!("staging-reject-{axis}"));
        let (_block_file, staged_batch_file, staged_certificate_file) =
            stage_certified_catch_up_artifacts(
                &source_dir,
                &destination_dir,
                &staging_dir,
                1,
                &block_one,
            );
        let store = NodeStore::new(&destination_dir);
        let tuple_before = status_tuple(&destination_dir);
        let ledger_before = store.read_ledger().expect("ledger before rejection");
        let blocks_before = store.read_blocks().expect("blocks before rejection");

        let error = apply_batch_with_expected_commit_identity(
            ApplyBatchOptions {
                data_dir: destination_dir.clone(),
                batch_file: staged_batch_file,
                certificate_file: Some(staged_certificate_file),
            },
            mutated_pin,
        )
        .expect_err("mutated pin must be rejected");
        let message = error.to_string();
        assert!(
            message.contains("prepared commit identity mismatch"),
            "{axis} axis rejection must be the typed pin mismatch: {message}"
        );
        assert!(
            message.contains(expected_message_fragment),
            "{axis} axis rejection must name the mutated pin field: {message}"
        );

        assert_eq!(
            status_tuple(&destination_dir),
            tuple_before,
            "{axis} axis rejection must leave destination height, tip, and state root unchanged"
        );
        assert_eq!(
            store.read_ledger().expect("ledger after rejection"),
            ledger_before,
            "{axis} axis rejection must leave the ledger unchanged"
        );
        assert_eq!(
            store.read_blocks().expect("blocks after rejection"),
            blocks_before,
            "{axis} axis rejection must leave the block log unchanged"
        );
    }

    // Positive vector: the exact pinned height, tip, and state root accepts
    // and the destination converges onto the certified block.
    let destination_dir = root_dir.join("accept");
    copy_recovery_test_dir(&genesis_seed_dir, &destination_dir);
    let staging_dir = root_dir.join("staging-accept");
    let (_block_file, staged_batch_file, staged_certificate_file) =
        stage_certified_catch_up_artifacts(&source_dir, &destination_dir, &staging_dir, 1, &block_one);
    apply_batch_with_expected_commit_identity(
        ApplyBatchOptions {
            data_dir: destination_dir.clone(),
            batch_file: staged_batch_file,
            certificate_file: Some(staged_certificate_file),
        },
        &pin,
    )
    .expect("exact pinned height, tip, and state root must be accepted");
    assert_eq!(
        status_tuple(&destination_dir),
        (pin.block_height, pin.block_hash.clone(), pin.state_root.clone()),
        "accepted catch-up must converge onto the pinned height, tip, and state root"
    );
    let verified = verify_blocks(NodeOptions {
        data_dir: destination_dir.clone(),
    })
    .expect("verify destination certified history");
    assert_eq!(verified.block_count, 1);
    assert!(verified.verified);

    std::fs::remove_dir_all(root_dir).expect("cleanup AR-04 pinned catch-up test");
}
