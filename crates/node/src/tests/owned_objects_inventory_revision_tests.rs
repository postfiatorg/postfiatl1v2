//! `owned_objects` carries `inventory_revision` {finalized_height,
//! fastpay_sequence, incarnation} and the answering `node_id`, all read under
//! the same ordered-commit lock as the page (after recovering a pending
//! ordered journal), so a page and its revision always belong to one view.

use super::*;

fn read(data_dir: &Path, owner: &str, limit: Option<usize>) -> OwnedObjectsOptions {
    OwnedObjectsOptions {
        data_dir: data_dir.to_path_buf(),
        owner_public_key_hex: owner.to_string(),
        asset: None,
        limit,
        cursor: None,
        omit_owner_keys: false,
        max_page_bytes: None,
    }
}

fn revision(report: &OwnedObjectsReport) -> (u64, u64) {
    let revision = report.inventory_revision.as_ref().expect("inventory revision");
    (revision.finalized_height, revision.fastpay_sequence)
}

fn incarnation(report: &OwnedObjectsReport) -> &str {
    let incarnation = &report.inventory_revision.as_ref().expect("inventory revision").incarnation;
    assert_eq!(incarnation.len(), 32, "{incarnation:?}");
    assert!(incarnation.bytes().all(|byte| byte.is_ascii_hexdigit()));
    incarnation
}

fn copy_dir(source: &Path, target: &Path) {
    std::fs::create_dir_all(target).expect("target dir");
    for entry in std::fs::read_dir(source).expect("source dir") {
        let entry = entry.expect("entry");
        let destination = target.join(entry.file_name());
        if entry.file_type().expect("file type").is_dir() {
            copy_dir(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), &destination).expect("copy file");
        }
    }
}

/// Every page of one consistent read: same view digest, same revision.
fn all_pages(options: OwnedObjectsOptions) -> Vec<OwnedObjectsReport> {
    let mut pages = vec![owned_objects(options.clone()).expect("first page")];
    while let Some(cursor) = pages.last().unwrap().next_cursor.clone() {
        pages.push(
            owned_objects(OwnedObjectsOptions {
                cursor: Some(cursor),
                ..options.clone()
            })
            .expect("next page"),
        );
    }
    pages
}

fn signed_votes(dirs: &[PathBuf], signed_json: &str) -> Vec<postfiat_types::OwnedTransferVote> {
    dirs.iter()
        .take(5)
        .enumerate()
        .map(|(index, dir)| {
            serde_json::from_str(
                &owned_sign_v3(
                    NodeOptions {
                        data_dir: dir.clone(),
                    },
                    signed_json,
                    &format!("validator-{index}"),
                )
                .expect("vote"),
            )
            .expect("vote json")
        })
        .collect()
}

fn apply(data_dir: &Path, certificate: &postfiat_types::OwnedTransferCertificateV3, id: &str) {
    owned_apply_v3(
        NodeOptions {
            data_dir: data_dir.to_path_buf(),
        },
        &serde_json::to_string(certificate).expect("certificate json"),
        id,
    )
    .expect("FastPay apply");
}

/// (a) and (b) on the transactional store: one applied effect raises
/// `fastpay_sequence` by one at the same finalized height; a replayed apply
/// and a restart (a copy of the data directory) answer the same revision;
/// a minority effect is dropped only when a new block finalizes, so the
/// revision then moves to the new height even though the sequence falls.
#[test]
fn owned_inventory_revision_grows_at_one_height_and_drops_minority_only_at_a_new_height() {
    let (root, dirs, owner) = super::fastpay_payment_safety::active_fastpay_fixture();
    let certificate = super::fastpay_payment_safety::active_fastpay_transfer(&dirs, &owner);
    let owner_hex = bytes_to_hex(&owner.public_key);
    let batch_file = root.join("omitting-batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: dirs[0].clone(),
        key_file: None,
        to: "pffastpayanchor00000000000000000000000001".into(),
        amount: 1,
        batch_file: batch_file.clone(),
    })
    .unwrap();
    // Validator 5 never saw the effect, so its block omits it.
    let (cert_file, proposal) =
        super::fastpay_payment_safety::active_fastpay_certify(&dirs[5], &root, batch_file.clone());
    assert!(proposal.fastpay_pre_state_effects.is_empty());

    let before = owned_objects(read(&dirs[0], &owner_hex, None)).unwrap();
    assert_eq!(before.node_id.as_deref(), Some("validator-0"));
    assert_eq!(revision(&before).0, before.height);

    apply(&dirs[0], &certificate, "validator-0");
    let applied = owned_objects(read(&dirs[0], &owner_hex, None)).unwrap();
    assert_eq!((applied.height, &applied.state_root), (before.height, &before.state_root));
    assert_ne!(applied.view_digest, before.view_digest);
    assert_eq!(revision(&applied), (before.height, revision(&before).1 + 1));
    assert!(revision(&applied) > revision(&before));
    assert_eq!(incarnation(&applied), incarnation(&before));

    // Replaying the same certificate is no new effect.
    apply(&dirs[0], &certificate, "validator-0");
    assert_eq!(owned_objects(read(&dirs[0], &owner_hex, None)).unwrap(), applied);

    // (b) Restart: the revision is a function of durable state only.
    let restarted = root.join("restarted-validator-0");
    copy_dir(&dirs[0], &restarted);
    let after_restart = owned_objects(read(&restarted, &owner_hex, None)).unwrap();
    assert_eq!(
        (revision(&after_restart), &after_restart.view_digest, &after_restart.node_id),
        (revision(&applied), &applied.view_digest, &applied.node_id)
    );
    assert_eq!(incarnation(&after_restart), incarnation(&applied));

    // The omitting block finalizes: the minority effect is dropped here, at
    // the new height, and nowhere else.
    apply_batch(ApplyBatchOptions {
        data_dir: dirs[0].clone(),
        batch_file,
        certificate_file: Some(cert_file),
    })
    .unwrap();
    let dropped = owned_objects(read(&dirs[0], &owner_hex, None)).unwrap();
    assert_eq!(revision(&dropped).0, before.height + 1);
    assert_eq!(dropped.objects, before.objects);
    assert!(revision(&dropped).1 < revision(&applied).1);
    assert!(revision(&dropped) > revision(&applied));
    assert_eq!(incarnation(&dropped), incarnation(&before));
    std::fs::remove_dir_all(root).unwrap();
}

/// (c) A snapshot restore answers the revision of the restored view: the
/// journal travels with the snapshot, so the restored node reads the same
/// objects, view digest and revision. The node id is the snapshot's unless
/// the import names another; the incarnation is new on every import. A
/// second effect at the same height then raises the sequence by one again,
/// and restoring the older snapshot under the same node id afterwards
/// answers the lower revision under a different incarnation.
#[test]
fn owned_inventory_revision_follows_the_restored_view_after_snapshot_restore() {
    let (root, dirs, owner) = super::fastpay_payment_safety::active_fastpay_fixture();
    let certificate = super::fastpay_payment_safety::active_fastpay_transfer(&dirs, &owner);
    for (index, dir) in dirs.iter().take(5).enumerate() {
        apply(dir, &certificate, &format!("validator-{index}"));
    }
    let recipient = ml_dsa_65_keygen_from_seed(&[81; 32]);
    let recipient_hex = bytes_to_hex(&recipient.public_key);
    let source = owned_objects(read(&dirs[0], &recipient_hex, None)).unwrap();
    assert_eq!(source.object_count, 1);

    let snapshot_dir = root.join("pending-snapshot");
    export_snapshot_from_finalized_checkpoint(SnapshotExportOptions {
        data_dir: dirs[0].clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .unwrap();
    let mut incarnations = vec![incarnation(&source).to_string()];
    for (name, node_id) in [("same-id", None), ("new-id", Some("validator-restored"))] {
        let restored = root.join(format!("restored-{name}"));
        import_snapshot_from_finalized_checkpoint(SnapshotImportOptions {
            data_dir: restored.clone(),
            snapshot_dir: snapshot_dir.clone(),
            node_id: node_id.map(str::to_string),
        })
        .unwrap();
        let answer = owned_objects(read(&restored, &recipient_hex, None)).unwrap();
        assert_eq!(
            (revision(&answer), &answer.view_digest, &answer.objects),
            (revision(&source), &source.view_digest, &source.objects),
            "{name}"
        );
        assert_eq!(
            answer.node_id.as_deref(),
            node_id.or(source.node_id.as_deref()),
            "{name}"
        );
        let restored_incarnation = incarnation(&answer).to_string();
        assert!(!incarnations.contains(&restored_incarnation), "{name}");
        incarnations.push(restored_incarnation);
    }

    // A dependent effect at the same finalized height.
    let mut child = certificate.order.clone();
    child.inputs = vec![postfiat_types::OwnedObjectRef {
        id: source.objects[0].id.clone(),
        version: source.objects[0].version,
    }];
    child.outputs[0].value = 98;
    child.outputs[0].owner_pubkey_hex = bytes_to_hex(&owner.public_key);
    child.nonce = 100;
    child.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&child);
    let signed = postfiat_types::SignedOwnedTransferOrderV3 {
        owner_pubkey_hex: recipient_hex.clone(),
        owner_signature_hex: bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &recipient.private_key,
                &postfiat_execution::owned_transfer_v3_signing_bytes(&child),
                postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
            )
            .unwrap(),
        ),
        order: child.clone(),
    };
    let votes = signed_votes(&dirs, &serde_json::to_string(&signed).unwrap());
    let child = postfiat_types::OwnedTransferCertificateV3 {
        order: child,
        owner_pubkey_hex: signed.owner_pubkey_hex,
        owner_signature_hex: signed.owner_signature_hex,
        votes,
    };
    apply(&dirs[0], &child, "validator-0");
    let second = owned_objects(read(&dirs[0], &recipient_hex, None)).unwrap();
    assert_eq!(second.object_count, 0);
    assert_eq!(revision(&second), (source.height, revision(&source).1 + 1));
    assert_eq!(incarnation(&second), incarnation(&source));

    // The same validator restored from the older snapshot: same node id,
    // the older revision, a new incarnation, so a client never orders the
    // restored answers against the earlier ones.
    let older = root.join("restored-older");
    import_snapshot_from_finalized_checkpoint(SnapshotImportOptions {
        data_dir: older.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: None,
    })
    .unwrap();
    let rolled_back = owned_objects(read(&older, &recipient_hex, None)).unwrap();
    assert_eq!(rolled_back.node_id, second.node_id);
    assert_eq!(revision(&rolled_back), revision(&source));
    assert!(!incarnations.contains(&incarnation(&rolled_back).to_string()));
    // An intact restart of the restored directory keeps its incarnation.
    let older_restarted = root.join("restored-older-restarted");
    copy_dir(&older, &older_restarted);
    let after_restart = owned_objects(read(&older_restarted, &recipient_hex, None)).unwrap();
    assert_eq!(incarnation(&after_restart), incarnation(&rolled_back));
    std::fs::remove_dir_all(root).unwrap();
}

/// (d) and (e) on the non-transactional store, where an effect is written
/// straight into the ledger file: the same revision rule holds, every page
/// of a multi-page read carries its own view's revision, and the
/// `view_digest` binding still refuses a cursor from the earlier view.
#[test]
fn owned_inventory_revision_on_every_page_and_on_the_non_transactional_store() {
    let (data_dir, owner, validators) = super::fastpay_payment_safety::fastpay_committee_fixture(4);
    let store = NodeStore::new(&data_dir);
    assert!(!store.transactional_storage_active().unwrap());
    let owner_hex = bytes_to_hex(&owner.public_key);
    let mut ledger = store.read_ledger().unwrap();
    for index in 0..3u64 {
        ledger.owned_objects.push(postfiat_types::OwnedObject {
            id: format!("{index:064x}"),
            version: 1,
            owner_pubkey_hex: owner_hex.clone(),
            value: 10 + index,
            asset: "PFT".into(),
        });
    }
    store.write_ledger(&ledger).unwrap();

    let first_view = all_pages(read(&data_dir, &owner_hex, Some(1)));
    assert_eq!(first_view.len(), 4);
    for page in &first_view {
        assert_eq!(revision(page), (105, 0));
        assert_eq!(page.view_digest, first_view[0].view_digest);
        assert_eq!(page.node_id, None, "the fixture has no node state");
    }

    let mut order = postfiat_types::OwnedTransferOrderV3 {
        domain: owned_certificate_domain_v3(&data_dir).unwrap(),
        recovery: postfiat_types::FastPayOrderRecoveryV1 {
            schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.into(),
            committee_epoch: 1,
            lock_id: "00".repeat(48),
            valid_from_height: 100,
            expires_at_height: 110,
            recovery_closes_at_height: 120,
        },
        inputs: vec![postfiat_types::OwnedObjectRef {
            id: "v3-node-input".into(),
            version: 7,
        }],
        outputs: vec![postfiat_types::OwnedOutputSpec {
            owner_pubkey_hex: "recipient-v3".into(),
            value: 99,
            asset: "PFT".into(),
        }],
        fee: 1,
        nonce: 1,
        memos: Vec::new(),
    };
    order.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&order);
    let signed = postfiat_types::SignedOwnedTransferOrderV3 {
        owner_pubkey_hex: owner_hex.clone(),
        owner_signature_hex: bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &owner.private_key,
                &postfiat_execution::owned_transfer_v3_signing_bytes(&order),
                postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
            )
            .unwrap(),
        ),
        order: order.clone(),
    };
    let signed_json = serde_json::to_string(&signed).unwrap();
    let votes = validators
        .iter()
        .take(3)
        .map(|(id, _)| {
            serde_json::from_str(
                &owned_sign_v3(
                    NodeOptions {
                        data_dir: data_dir.clone(),
                    },
                    &signed_json,
                    id,
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    let certificate = postfiat_types::OwnedTransferCertificateV3 {
        order,
        owner_pubkey_hex: owner_hex.clone(),
        owner_signature_hex: signed.owner_signature_hex,
        votes,
    };
    apply(&data_dir, &certificate, "validator-0");
    apply(&data_dir, &certificate, "validator-0");

    let stale_cursor = first_view[0].next_cursor.clone().unwrap();
    let refused = owned_objects(OwnedObjectsOptions {
        cursor: Some(stale_cursor),
        ..read(&data_dir, &owner_hex, Some(1))
    })
    .expect_err("view changed")
    .to_string();
    assert_eq!(
        owned_objects_error_code(&refused),
        Some(OWNED_OBJECTS_VIEW_CHANGED)
    );
    let second_view = all_pages(read(&data_dir, &owner_hex, Some(1)));
    assert_eq!(second_view.len(), 3);
    for page in &second_view {
        assert_eq!(revision(page), (105, 1));
        assert_eq!(page.view_digest, second_view[0].view_digest);
    }
    assert_ne!(second_view[0].view_digest, first_view[0].view_digest);

    let restarted = data_dir.with_extension("restarted");
    copy_dir(&data_dir, &restarted);
    assert_eq!(
        revision(&owned_objects(read(&restarted, &owner_hex, Some(1))).unwrap()),
        (105, 1)
    );
    std::fs::remove_dir_all(data_dir).unwrap();
    std::fs::remove_dir_all(restarted).unwrap();
}

/// A read after an interrupted non-transactional commit (journal written,
/// new ledger written, tip not yet advanced) recovers the journal under the
/// commit lock first, as `status` does, so the page and its revision carry
/// the new tip with the new ledger instead of the old tip.
#[test]
fn owned_objects_recovers_a_pending_ordered_journal_before_reading() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-owned-objects-pending-journal-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-owned-objects-pending-journal".to_owned(),
        node_id: "validator-0".to_owned(),
        validator_count: 1,
    })
    .unwrap();
    let store = NodeStore::new(&data_dir);
    assert!(!store.transactional_storage_active().unwrap());
    let genesis = store.read_genesis().unwrap();
    let governance = store.read_governance().unwrap();
    let ledger = store.read_ledger().unwrap();
    let batch =
        postfiat_mempool_dag::build_transaction_batch(&mempool_batch_domain(&genesis), Vec::new())
            .unwrap()
            .batch;
    let journal = super::replicated_state_activation::activation_test_journal(
        &store,
        &genesis,
        &governance,
        &ledger,
        BATCH_KIND_TRANSPARENT,
        &batch.batch_id,
        &batch,
        &[],
        false,
    );
    let old_height = store.read_chain_tip().unwrap().height;
    let owner_hex = bytes_to_hex(&ml_dsa_65_keygen_from_seed(&[82; 32]).public_key);
    let before = owned_objects(read(&data_dir, &owner_hex, None)).unwrap();
    assert_eq!(revision(&before).0, old_height);

    // The ledger-written prefix of an ordered commit.
    store.write_ordered_commit_journal(&journal).unwrap();
    store.write_ledger(journal.ledger.as_ref().unwrap()).unwrap();

    let answer = owned_objects(read(&data_dir, &owner_hex, None)).unwrap();
    assert_eq!(answer.height, old_height + 1);
    assert_eq!(revision(&answer).0, old_height + 1);
    assert_eq!(incarnation(&answer), incarnation(&before));
    assert_eq!(store.read_ordered_commit_journal_raw().unwrap(), None);
    assert_eq!(store.read_chain_tip().unwrap().height, old_height + 1);
    std::fs::remove_dir_all(data_dir).unwrap();
}
