//! `fastswap-committee-migrate` against six-validator fixture stores at the
//! drained epoch-1 state, plus the real epoch-2 committee and anchored drain
//! checkpoint from the fastlane-cap-20261014 release packet.

use super::committee_migration::{self, FastSwapCommitteeMigrationReport, FAIL_AFTER_WRITE};
use super::tests::fixture;
use super::*;
use postfiat_crypto_provider::bytes_to_hex;
use postfiat_storage::fastswap_store::FastSwapStore;
use postfiat_types::FastAssetIdV1;
use postfiat_types::{FastSwapEffectsDigestV1, FastSwapIntentIdV1};
use std::collections::BTreeMap;
use std::path::PathBuf;

const REAL_EPOCH_2_ROOT: &str = "72836827f9f3ecd3042261a77abff72235f0238b6a81f673630a62199415021a1be0cbd5451878edca0e110434c1da37";
const REAL_EPOCH_1_ROOT: &str = "a2eebcbaa026e9a527861001dcc8f362f1173c5c2001a0eb4b3b7b5114e959c4ccdc6b01cc400f985e34fbc02095968e";
const REAL_ANCHOR_ID: &str = "355fe6080068e961ed1f295217d3f0f54a7b7b6a2fcb883824a7c6e9f3a8f49869fdd12b46be88eb5509e1a1f8120ede";

#[derive(Clone, Copy, PartialEq)]
enum Shape {
    /// Validators 0–4: fence and anchor applied, compacted to a snapshot.
    Snapshot,
    /// Fence and anchor applied as WAL records over base-state.json.
    BaseWal,
    /// Validator-5: base-state.json only, fence and anchor never applied.
    LegacyNoRecords,
    /// Validator-5 with a pre-anchor reservation still in its WAL.
    LegacyReservation,
    /// Snapshot shape with an open reservation.
    OpenReservation,
    /// Snapshot shape, fence still at epoch 1, with a pending fee burn.
    PendingBurn,
}

struct Node {
    root: PathBuf,
    data_dir: PathBuf,
    committee_file: PathBuf,
    anchor_file: PathBuf,
    next: FastSwapCommitteeV1,
    anchor_id: FastLaneCheckpointIdV1,
}

impl Node {
    fn store(&self) -> PathBuf {
        self.data_dir.join(FASTSWAP_DIRECTORY)
    }

    fn options(&self, dry_run: bool) -> FastSwapCommitteeMigrationOptions {
        FastSwapCommitteeMigrationOptions {
            data_dir: self.data_dir.clone(),
            committee_file: self.committee_file.clone(),
            anchor_file: self.anchor_file.clone(),
            backup_directory: Some(self.root.join("backup")),
            validator_id: None,
            dry_run,
        }
    }

    fn files(&self) -> BTreeMap<String, Vec<u8>> {
        let mut files = BTreeMap::new();
        collect(&self.store(), "", &mut files);
        files
    }
}

impl Drop for Node {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn collect(directory: &Path, prefix: &str, files: &mut BTreeMap<String, Vec<u8>>) {
    for entry in fs::read_dir(directory).expect("read store") {
        let entry = entry.expect("entry");
        let name = format!("{prefix}{}", entry.file_name().to_string_lossy());
        if name == "fastswap-v1.lock" {
            continue;
        }
        if entry.file_type().expect("type").is_dir() {
            collect(&entry.path(), &format!("{name}/"), files);
        } else {
            files.insert(name, fs::read(entry.path()).expect("read file"));
        }
    }
}

fn node(shape: Shape) -> Node {
    let fixture = fixture();
    let data_dir = fixture.root.join("validator-0");
    let store_dir = data_dir.join(FASTSWAP_DIRECTORY);
    fs::create_dir_all(&store_dir).expect("store dir");
    let policy_epoch = fixture
        .base
        .policy_snapshots
        .values()
        .next()
        .expect("policy")
        .policy_epoch;
    let fence = postfiat_types::FastLanePrepareFenceV1 {
        committee_epoch: 1,
        policy_epoch,
        finalized_primary_height: 110,
    };

    // Canonical ledger: epoch 1 fenced, drained, anchored; epoch 2 activated.
    let mut drained = fixture.base.clone();
    drained.prepare_fences.insert(policy_epoch, fence.clone());
    let mut ledger = LedgerState::empty();
    ledger.fastswap_committees.push(fixture.committee.clone());
    ledger.fast_lane_asset_rules = fixture.base.asset_rules.values().cloned().collect();
    ledger.fastswap_policy_snapshots = fixture.base.policy_snapshots.values().cloned().collect();
    ledger.fast_lane_prepare_fences.push(fence);
    ledger.fastswap_activation_height = Some(1);
    let mut totals = BTreeMap::<FastAssetIdV1, u128>::new();
    for object in drained.objects.values() {
        *totals.entry(object.asset_id).or_default() += u128::from(object.amount_atoms);
    }
    ledger.fast_lane_reserves = totals
        .into_iter()
        .map(
            |(asset_id, amount_atoms)| postfiat_types::FastLaneReserveBalanceV1 {
                asset_id,
                amount_atoms,
            },
        )
        .collect();
    let checkpoint =
        build_fastlane_checkpoint(&drained, &ledger, None, 2).expect("drain checkpoint");
    assert!(checkpoint.drain_ready);
    let anchor_id = checkpoint.checkpoint_id().expect("checkpoint id");
    let votes = fixture
        .committee
        .validators
        .iter()
        .zip(fixture.validator_keys.iter())
        .take(5)
        .map(|(member, key)| {
            let mut vote = FastLaneCheckpointVoteV1 {
                checkpoint: checkpoint.clone(),
                validator_id: member.validator_id.clone(),
                signature: Vec::new(),
            };
            vote.signature = ml_dsa_65_sign_with_context(
                key,
                &vote.signing_bytes().expect("vote bytes"),
                FASTLANE_CHECKPOINT_CONTEXT_V1,
            )
            .expect("sign");
            vote
        })
        .collect();
    let certificate = postfiat_types::FastLaneCheckpointCertificateV1 { votes };
    postfiat_execution::fastswap_checkpoint::anchor_fastlane_checkpoint(&mut ledger, &certificate)
        .expect("anchor");
    let mut next = fixture.committee.clone();
    next.domain.committee_epoch = 2;
    next.domain.committee_root = postfiat_types::FastSwapCommitteeRootV1::ZERO;
    next.domain.committee_root = next.computed_root().expect("next root");
    ledger.fastswap_committees.push(next.clone());
    let node_store = NodeStore::new(&data_dir);
    node_store.write_ledger(&ledger).expect("ledger");
    node_store
        .write_chain_tip(&ChainTipState {
            schema: "postfiat-chain-tip-v1".to_owned(),
            chain_id: "test".to_owned(),
            genesis_hash: "genesis".to_owned(),
            protocol_version: 1,
            height: 110,
            block_hash: "block".to_owned(),
            state_root: "root".to_owned(),
            ordered_batch_count: 0,
            receipt_count: 0,
            history_base_height: 0,
        })
        .expect("tip");
    fs::write(
        data_dir.join("validator_keys.json"),
        serde_json::to_vec(&serde_json::json!({
            "validators": [{
                "node_id": "validator-0",
                "private_key_hex": bytes_to_hex(&fixture.validator_keys[0]),
            }]
        }))
        .expect("key JSON"),
    )
    .expect("keys");

    // Release-packet file shapes: the full transactions, not bare objects.
    let committee_file = fixture.root.join("7.2-activate-committee.json");
    fs::write(
        &committee_file,
        serde_json::to_vec(&serde_json::json!({
            "operation": {"kind": "control", "certificate": {"action": {
                "kind": "activate_committee", "committee": next,
            }}}
        }))
        .expect("committee JSON"),
    )
    .expect("committee file");
    let anchor_file = fixture.root.join("7.1-anchor-checkpoint.json");
    fs::write(
        &anchor_file,
        serde_json::to_vec(&serde_json::json!({
            "operation": {"kind": "anchor_checkpoint", "certificate": certificate}
        }))
        .expect("anchor JSON"),
    )
    .expect("anchor file");

    // Epoch-1 store.
    fs::write(
        store_dir.join(FASTSWAP_BASE_STATE),
        postfiat_storage::fastswap_store::encode_fastlane_state_file(&fixture.base).expect("base"),
    )
    .expect("base file");
    fs::write(
        store_dir.join(FASTSWAP_COMMITTEE),
        serde_json::to_vec(&fixture.committee).expect("committee"),
    )
    .expect("committee file");
    if !matches!(shape, Shape::LegacyNoRecords) {
        let mut store = FastSwapStore::open(&store_dir).expect("open store");
        let mut state = store.replay(&fixture.base).expect("replay");
        if !matches!(shape, Shape::LegacyReservation) {
            store
                .persist_prepare_fence(&mut state, 1, policy_epoch, 110)
                .expect("fence");
            store
                .apply_anchored_checkpoint(&mut state, anchor_id, &[])
                .expect("anchor record");
        }
        if matches!(shape, Shape::LegacyReservation | Shape::OpenReservation) {
            let input = *state.objects.keys().next().expect("object");
            store
                .reserve_all(
                    &mut state,
                    FastSwapIdV1([5; 48]),
                    FastSwapIntentIdV1([6; 48]),
                    FastSwapEffectsDigestV1([7; 48]),
                    120,
                    &[input],
                )
                .expect("reserve");
        }
        if matches!(shape, Shape::PendingBurn) {
            state
                .pending_fee_burns
                .insert(FastAssetIdV1::native_pft(), 1);
        }
        if matches!(
            shape,
            Shape::Snapshot | Shape::OpenReservation | Shape::PendingBurn
        ) {
            store.compact_snapshot(&state).expect("snapshot");
        }
    }
    Node {
        root: fixture.root.clone(),
        data_dir,
        committee_file,
        anchor_file,
        next,
        anchor_id,
    }
}

fn assert_migrates(node: &Node, expected_load_path: &str) -> FastSwapCommitteeMigrationReport {
    let before = node.files();
    let dry = fastswap_committee_migrate(&node.options(true)).expect("dry run");
    assert_eq!(dry.outcome, "would-migrate");
    assert_eq!(dry.load_path, expected_load_path);
    assert_eq!(node.files(), before, "dry run must not write");
    assert!(!node.root.join("backup").exists());

    let report = fastswap_committee_migrate(&node.options(false)).expect("migrate");
    assert_eq!(report.outcome, "migrated");
    assert_eq!(report.validator_id, "validator-0");
    assert_eq!(report.before.committee_epoch, 1);
    assert_eq!(report.after.committee_epoch, 2);
    assert_eq!(
        report.after.committee_root,
        bytes_to_hex(&node.next.domain.committee_root.0)
    );
    assert!(report.after.prepare_fences.is_empty());
    assert_eq!(
        report.after.anchored_checkpoints,
        vec![bytes_to_hex(&node.anchor_id.0)]
    );
    assert_eq!(
        report.after.file_sha3_384[FASTSWAP_BASE_STATE],
        dry.after.file_sha3_384[FASTSWAP_BASE_STATE],
        "dry run predicts the written base-state"
    );
    assert!(
        report.verification.contains("enabled true"),
        "{}",
        report.verification
    );
    assert!(!report.backup_manifest.is_empty());
    for entry in &report.backup_manifest {
        assert_eq!(
            Some(entry.sha3_384.clone()),
            postfiat_storage::fastswap_store::fastswap_file_sha3_384(
                &node.root.join("backup").join(&entry.path)
            )
            .expect("hash")
        );
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for name in [FASTSWAP_BASE_STATE, FASTSWAP_COMMITTEE] {
            let mode = fs::metadata(node.store().join(name))
                .expect("meta")
                .permissions()
                .mode();
            assert_eq!(mode & 0o777, 0o600, "{name}");
        }
    }
    assert!(!node.store().join("fastswap-v1.snapshot.json").exists());

    let service =
        FastSwapValidatorServiceV1::open(&node.data_dir, "validator-0").expect("open epoch 2");
    let capabilities = service.capabilities();
    assert_eq!(capabilities.committee, node.next.domain);
    assert_eq!(capabilities.committee.committee_epoch, 2);
    assert!(capabilities.enabled);
    report
}

#[test]
fn committee_migrate_moves_drained_snapshot_store_to_epoch_two() {
    let node = node(Shape::Snapshot);
    let report = assert_migrates(&node, "snapshot");
    assert!(report.notes.is_empty(), "{:?}", report.notes);
    assert_eq!(report.before.prepare_fences.len(), 1);
    assert!(report
        .moved_aside
        .contains(&"fastswap-v1.snapshot.json".to_owned()));
}

#[test]
fn committee_migrate_moves_drained_base_wal_store_to_epoch_two() {
    let node = node(Shape::BaseWal);
    let report = assert_migrates(&node, "base+wal");
    assert_eq!(report.wal_records, 2);
    assert!(report.notes.is_empty(), "{:?}", report.notes);
}

#[test]
fn committee_migrate_accepts_legacy_store_equal_to_the_drained_state() {
    let node = node(Shape::LegacyNoRecords);
    let report = assert_migrates(&node, "base+wal");
    assert!(
        report.notes[0].contains("never applied the epoch-1 prepare fence for policy epoch(s)")
            && report.notes[0].contains("equals the anchored drained state"),
        "{:?}",
        report.notes
    );
}

fn assert_refused(node: &Node, expected: &str) -> String {
    let before = node.files();
    for dry_run in [true, false] {
        let error = fastswap_committee_migrate(&node.options(dry_run)).expect_err("refuse");
        assert!(error.contains(expected), "{error}");
        assert_eq!(
            node.files(),
            before,
            "refusal must leave the store byte-identical"
        );
        assert!(!node.root.join("backup").exists());
    }
    fastswap_committee_migrate(&node.options(true)).expect_err("refuse")
}

#[test]
fn committee_migrate_refuses_store_with_open_reservation() {
    let node = node(Shape::OpenReservation);
    let error = assert_refused(
        &node,
        "FastSwap store is not drained: 1 open reservation(s)",
    );
    eprintln!("refuse reason: {error}");
}

#[test]
fn committee_migrate_refuses_store_with_pending_fee_burn() {
    let node = node(Shape::PendingBurn);
    let error = assert_refused(
        &node,
        "FastSwap store is not drained: pending fee burns for 1 asset(s)",
    );
    eprintln!("refuse reason: {error}");
}

#[test]
fn committee_migrate_refuses_unapplied_legacy_wal_with_pre_anchor_records() {
    let node = node(Shape::LegacyReservation);
    let error = assert_refused(
        &node,
        "store (base+WAL load, 1 WAL record(s), 0 fence and 0 anchor record(s)) never applied the epoch-1 prepare fence for policy epoch(s)",
    );
    assert!(
        error.contains("or the anchored drain checkpoint"),
        "{error}"
    );
    assert!(
        error.contains("and the replayed state does not equal the anchored drained state: 1 open reservation(s)"),
        "{error}"
    );
    eprintln!("refuse reason: {error}");
}

#[test]
fn committee_migrate_refuses_held_store_lock() {
    let node = node(Shape::Snapshot);
    let held = FastSwapStore::open(node.store()).expect("hold lock");
    let before = node.files();
    for dry_run in [true, false] {
        let error = fastswap_committee_migrate(&node.options(dry_run)).expect_err("locked");
        assert!(
            error.contains("already locked") && error.contains("stop both validator units"),
            "{error}"
        );
    }
    assert_eq!(node.files(), before);
    drop(held);
}

#[test]
fn committee_migrate_restores_backup_when_verification_fails_after_write() {
    let node = node(Shape::Snapshot);
    let before = node.files();
    FAIL_AFTER_WRITE.with(|flag| flag.set(true));
    let error = fastswap_committee_migrate(&node.options(false)).expect_err("injected");
    assert!(
        error.contains("injected post-write verification failure")
            && error.contains("store restored"),
        "{error}"
    );
    assert_eq!(node.files(), before, "restore must put every byte back");
    // The restored epoch-1 store migrates normally afterwards.
    fs::remove_dir_all(node.root.join("backup")).expect("clear backup");
    assert_migrates(&node, "snapshot");
}

#[test]
fn committee_migrate_reads_the_real_release_packet_files() {
    let fixtures =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fastswap-committee-migration");
    let committee = committee_migration::read_committee_file(
        &fixtures.join("7.2-activate-committee.fastlane-primary.json"),
    )
    .expect("real epoch-2 committee");
    assert_eq!(committee.domain.committee_epoch, 2);
    assert_eq!(
        bytes_to_hex(&committee.domain.committee_root.0),
        REAL_EPOCH_2_ROOT
    );
    assert_eq!(committee.validators.len(), 6);
    assert_eq!(committee.domain.quorum, 5);
    let (anchor_id, certificate) = committee_migration::read_anchor_file(
        &fixtures.join("7.1-anchor-checkpoint.fastlane-primary.json"),
    )
    .expect("real anchor");
    assert_eq!(bytes_to_hex(&anchor_id.0), REAL_ANCHOR_ID);
    let checkpoint = &certificate.votes[0].checkpoint;
    assert_eq!(certificate.votes.len(), 5);
    assert!(checkpoint.drain_ready && checkpoint.exit_claim_totals.is_empty());
    assert!(checkpoint.pending_fee_burn_totals.is_empty());
    assert_eq!(checkpoint.fenced_policy_epochs, vec![1]);
    assert_eq!(checkpoint.committee.committee_epoch, 1);
    assert_eq!(
        bytes_to_hex(&checkpoint.committee.committee_root.0),
        REAL_EPOCH_1_ROOT
    );
    assert_eq!(checkpoint.committee.chain, committee.domain.chain);
    // The epoch-2 base the command builds from a drained epoch-1 state.
    let mut drained = FastLaneStateV1::empty(checkpoint.committee.clone());
    drained.prepare_fences.insert(
        1,
        postfiat_types::FastLanePrepareFenceV1 {
            committee_epoch: 1,
            policy_epoch: 1,
            finalized_primary_height: 1121,
        },
    );
    let migrated = committee_migration::migrate_drained_fastlane_state(
        &drained,
        &committee.domain,
        Some(anchor_id),
    );
    assert_eq!(migrated.committee, committee.domain);
    assert!(migrated.prepare_fences.is_empty());
    assert!(migrated.anchored_checkpoints.contains(&anchor_id));
}

fn reserve_one_object_in_epoch_two(node: &Node) {
    let mut service =
        FastSwapValidatorServiceV1::open(&node.data_dir, "validator-0").expect("open epoch 2");
    let input = *service.state.objects.keys().next().expect("object");
    service
        .store
        .reserve_all(
            &mut service.state,
            FastSwapIdV1([5; 48]),
            FastSwapIntentIdV1([6; 48]),
            FastSwapEffectsDigestV1([7; 48]),
            120,
            &[input],
        )
        .expect("reserve in epoch 2");
}

fn assert_reopens(node: &Node) -> FastSwapValidatorServiceV1 {
    let service = FastSwapValidatorServiceV1::open(&node.data_dir, "validator-0")
        .unwrap_or_else(|error| panic!("reopen epoch 2: {error}"));
    let capabilities = service.capabilities();
    assert_eq!(capabilities.committee, node.next.domain);
    assert!(capabilities.enabled);
    service
}

#[test]
fn committee_migrate_store_reopens_after_epoch_two_reservation() {
    let node = node(Shape::Snapshot);
    assert_migrates(&node, "snapshot");
    reserve_one_object_in_epoch_two(&node);
    let service = assert_reopens(&node);
    assert_eq!(service.state.reservations.len(), 1);
}

#[test]
fn committee_migrate_legacy_store_reopens_after_epoch_two_reservation() {
    let node = node(Shape::LegacyNoRecords);
    assert_migrates(&node, "base+wal");
    reserve_one_object_in_epoch_two(&node);
    let service = assert_reopens(&node);
    assert_eq!(service.state.reservations.len(), 1);
}

#[test]
fn committee_migrate_store_reopens_after_epoch_two_deposit_import() {
    let node = node(Shape::Snapshot);
    assert_migrates(&node, "snapshot");
    let node_store = NodeStore::new(&node.data_dir);
    let mut ledger = node_store.read_ledger().expect("ledger");
    let rule = ledger.fast_lane_asset_rules[0].clone();
    let receipt = postfiat_types::FastLaneDepositReceiptV1 {
        deposit_id: postfiat_types::FastSwapDepositIdV1([23; 48]),
        accepted: true,
        code: "fastlane_deposit_applied".to_owned(),
        destination_owner_pubkey: vec![24; 32],
        asset_id: rule.asset_id,
        asset_rule_hash: rule.rule_hash().expect("rule hash"),
        amount_atoms: 1,
        initial_object_key: postfiat_types::FastObjectKeyV1 {
            object_id: postfiat_types::FastObjectIdV1([25; 32]),
            version: 1,
        },
    };
    // A finalized deposit credits the FastLane reserve and records the receipt.
    ledger
        .fast_lane_reserves
        .iter_mut()
        .find(|reserve| reserve.asset_id == rule.asset_id)
        .expect("reserve")
        .amount_atoms += 1;
    ledger.fast_lane_deposit_receipts.push(receipt.clone());
    node_store.write_ledger(&ledger).expect("ledger");
    for _ in 0..2 {
        let service = assert_reopens(&node);
        assert!(service
            .state
            .imported_deposits
            .contains(&receipt.deposit_id));
        assert!(service
            .state
            .objects
            .contains_key(&receipt.initial_object_key));
    }
}

#[test]
fn committee_migrated_open_refuses_relabelled_epoch_one_base() {
    let node = node(Shape::Snapshot);
    let store_dir = node.store();
    let base = read_fastlane_base_state(&store_dir.join(FASTSWAP_BASE_STATE)).expect("base");
    let store = FastSwapStore::open(&store_dir).expect("open store");
    let mut relabelled = store.replay(&base).expect("replay");
    drop(store);
    assert!(relabelled
        .prepare_fences
        .values()
        .any(|fence| fence.committee_epoch == 1));
    relabelled.committee = node.next.domain.clone();
    fs::write(
        store_dir.join(FASTSWAP_BASE_STATE),
        postfiat_storage::fastswap_store::encode_fastlane_state_file(&relabelled).expect("encode"),
    )
    .expect("base file");
    fs::write(
        store_dir.join(FASTSWAP_COMMITTEE),
        serde_json::to_vec(&node.next).expect("committee"),
    )
    .expect("committee file");
    for name in ["fastswap-v1.wal", "fastswap-v1.snapshot.json"] {
        let _ = fs::remove_file(store_dir.join(name));
    }
    let Err(error) = FastSwapValidatorServiceV1::open(&node.data_dir, "validator-0") else {
        panic!("an un-migrated base relabelled epoch 2 must be refused");
    };
    assert!(
        error
            .to_string()
            .contains("FastSwap migration base is not a clean drained state"),
        "{error}"
    );
}

#[test]
fn committee_migrated_open_refuses_edited_base() {
    let node = node(Shape::Snapshot);
    assert_migrates(&node, "snapshot");
    let base_path = node.store().join(FASTSWAP_BASE_STATE);
    let mut base = read_fastlane_base_state(&base_path).expect("base");
    let first_key = *base.objects.keys().next().expect("object");
    base.objects
        .get_mut(&first_key)
        .expect("object")
        .amount_atoms -= 1;
    fs::write(
        &base_path,
        postfiat_storage::fastswap_store::encode_fastlane_state_file(&base).expect("encode"),
    )
    .expect("base file");
    let Err(error) = FastSwapValidatorServiceV1::open(&node.data_dir, "validator-0") else {
        panic!("an edited migrated base must be refused");
    };
    assert!(
        error.to_string().starts_with("FastSwap migration base"),
        "{error}"
    );
}
