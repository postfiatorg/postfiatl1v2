use super::*;

// Regression coverage for the 2026-08-30 devnet G6 rehearsal stop
// (`VALIDATOR_REGISTRY_HISTORY_REAPPLICATION_ROOT_MISMATCH`): live and
// ordered validator-registry activation must treat accepted, superseded
// updates as applied history instead of reapplying them, while every pending
// update still fails closed on wrong-root, reordered, or missing history.

const CONTINUATION_SUBJECT: &str = "validator-3";

fn continuation_registry_entry(
    node_id: &str,
    algorithm_id: &str,
    public_key_hex: &str,
) -> ValidatorRegistryEntry {
    ValidatorRegistryEntry {
        node_id: node_id.to_string(),
        algorithm_id: algorithm_id.to_string(),
        public_key_hex: public_key_hex.to_string(),
        active: true,
    }
}

fn write_continuation_entry_file(path: &std::path::Path, entry: &ValidatorRegistryEntry) {
    let json = serde_json::to_string_pretty(entry).expect("serialize registry entry");
    atomic_write(path, format!("{json}\n")).expect("write registry entry file");
}

fn registry_with_subject_key(
    registry: &ValidatorRegistry,
    subject_node_id: &str,
    algorithm_id: &str,
    public_key_hex: &str,
) -> ValidatorRegistry {
    let mut rotated = registry.clone();
    let record = rotated
        .validators
        .iter_mut()
        .find(|record| record.node_id == subject_node_id)
        .expect("rotation subject present in registry");
    record.algorithm_id = algorithm_id.to_string();
    record.public_key_hex = public_key_hex.to_string();
    rotated
}

fn keys_with_subject_record(
    keys: &ValidatorKeyFile,
    replacement: &ValidatorKeyRecord,
) -> ValidatorKeyFile {
    ValidatorKeyFile {
        validators: keys
            .validators
            .iter()
            .map(|record| {
                if record.node_id == replacement.node_id {
                    replacement.clone()
                } else {
                    record.clone()
                }
            })
            .collect(),
    }
}

#[allow(clippy::too_many_arguments)]
fn certified_subject_rotation_update(
    data_dir: &std::path::Path,
    validators: &[String],
    label: &str,
    activation_height: u64,
    previous_registry_root: &str,
    new_registry_root: &str,
    previous_entry: &ValidatorRegistryEntry,
    new_entry: &ValidatorRegistryEntry,
) -> (ValidatorRegistryUpdateRecord, PathBuf) {
    let previous_entry_file = data_dir.join(format!("{label}-previous-entry.json"));
    write_continuation_entry_file(&previous_entry_file, previous_entry);
    let new_entry_file = data_dir.join(format!("{label}-new-entry.json"));
    write_continuation_entry_file(&new_entry_file, new_entry);
    let update_file = data_dir.join(format!("{label}.update.json"));
    let update = create_validator_registry_update(ValidatorRegistryUpdateOptions {
        data_dir: data_dir.to_path_buf(),
        validators: validators.to_vec(),
        support: validators.to_vec(),
        activation_height,
        previous_registry_root: previous_registry_root.to_string(),
        new_registry_root: new_registry_root.to_string(),
        previous_validators: validators.to_vec(),
        new_validators: validators.to_vec(),
        operation: VALIDATOR_REGISTRY_OP_ROTATE_KEY.to_string(),
        subject_node_id: CONTINUATION_SUBJECT.to_string(),
        previous_record_file: Some(previous_entry_file),
        new_record_file: Some(new_entry_file),
        update_file: update_file.clone(),
    })
    .expect("create rotation registry update");
    (update, update_file)
}

fn commit_governance_registry_update(data_dir: &std::path::Path, label: &str, update_file: PathBuf) {
    let batch_file = data_dir.join(format!("{label}.governance.json"));
    create_governance_batch(GovernanceBatchOptions {
        data_dir: data_dir.to_path_buf(),
        amendment_file: None,
        registry_update_file: Some(update_file),
        batch_file: batch_file.clone(),
    })
    .expect("create governance batch for registry update");
    let receipts = apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file,
        certificate_file: None,
    })
    .expect("commit governance batch for registry update");
    assert!(
        receipts.iter().any(|receipt| receipt.accepted),
        "{receipts:?}"
    );
}

fn chain_tip_height(data_dir: &std::path::Path) -> u64 {
    blocks(BlockQueryOptions {
        data_dir: data_dir.to_path_buf(),
        from_height: None,
        limit: None,
    })
    .expect("read committed blocks")
    .iter()
    .map(|block| block.header.height)
    .max()
    .unwrap_or(0)
}

/// Exact defect shape from the height-924 devnet history: a drill rotation,
/// its signed rollback, and a later legitimate rotation all touch the same
/// validator record. The chain must then continue to the next certified
/// height instead of reapplying the superseded rotation and failing with
/// `live validator registry activation previous validator registry root
/// mismatch`.
#[test]
fn superseded_registry_rotation_history_continues_to_next_certified_height() {
    let data_dir = unique_test_dir("postfiat-superseded-rotation-continuation-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-superseded-rotation-continuation".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init superseded rotation continuation test");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let validators = local_validator_ids(4).expect("validators");
    let registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let keys_path = data_dir.join(VALIDATOR_KEYS_FILE);
    let original_registry = read_validator_registry_file(&registry_path).expect("registry");
    let original_keys = read_validator_key_file(&keys_path).expect("validator keys");
    let original_record = validator_registry_record(&original_registry, CONTINUATION_SUBJECT)
        .expect("subject record")
        .clone();
    let drill_key =
        create_validator_key_record(CONTINUATION_SUBJECT.to_string()).expect("drill key");
    let final_key =
        create_validator_key_record(CONTINUATION_SUBJECT.to_string()).expect("final key");

    let drill_registry = registry_with_subject_key(
        &original_registry,
        CONTINUATION_SUBJECT,
        &drill_key.algorithm_id,
        &drill_key.public_key_hex,
    );
    let final_registry = registry_with_subject_key(
        &original_registry,
        CONTINUATION_SUBJECT,
        &final_key.algorithm_id,
        &final_key.public_key_hex,
    );
    let original_root =
        validator_registry_root(&original_registry, &validators).expect("original root");
    let drill_root = validator_registry_root(&drill_registry, &validators).expect("drill root");
    let final_root = validator_registry_root(&final_registry, &validators).expect("final root");
    assert_ne!(original_root, drill_root);
    assert_ne!(original_root, final_root);
    assert_ne!(drill_root, final_root);

    let original_entry = continuation_registry_entry(
        &original_record.node_id,
        &original_record.algorithm_id,
        &original_record.public_key_hex,
    );
    let drill_entry = continuation_registry_entry(
        &drill_key.node_id,
        &drill_key.algorithm_id,
        &drill_key.public_key_hex,
    );
    let final_entry = continuation_registry_entry(
        &final_key.node_id,
        &final_key.algorithm_id,
        &final_key.public_key_hex,
    );

    // Height 1: drill rotation to the drill key.
    let (_, drill_update_file) = certified_subject_rotation_update(
        &data_dir,
        &validators,
        "drill-rotation",
        1,
        &original_root,
        &drill_root,
        &original_entry,
        &drill_entry,
    );
    commit_governance_registry_update(&data_dir, "drill-rotation", drill_update_file);
    assert_eq!(
        validator_registry_root(
            &read_validator_registry_file(&registry_path).expect("registry after drill"),
            &validators,
        )
        .expect("root after drill"),
        drill_root
    );
    write_validator_key_file(&keys_path, &keys_with_subject_record(&original_keys, &drill_key))
        .expect("stage drill signing key");

    // Height 2: signed rollback to the original key.
    let (_, rollback_update_file) = certified_subject_rotation_update(
        &data_dir,
        &validators,
        "rollback-rotation",
        2,
        &drill_root,
        &original_root,
        &drill_entry,
        &original_entry,
    );
    commit_governance_registry_update(&data_dir, "rollback-rotation", rollback_update_file);
    assert_eq!(
        validator_registry_root(
            &read_validator_registry_file(&registry_path).expect("registry after rollback"),
            &validators,
        )
        .expect("root after rollback"),
        original_root
    );
    write_validator_key_file(&keys_path, &original_keys).expect("restore original signing key");

    // Height 3: legitimate rotation to the final key. This supersedes the
    // drill rotation: its roots are no longer reproducible from the final
    // registry.
    let (_, final_update_file) = certified_subject_rotation_update(
        &data_dir,
        &validators,
        "final-rotation",
        3,
        &original_root,
        &final_root,
        &original_entry,
        &final_entry,
    );
    commit_governance_registry_update(&data_dir, "final-rotation", final_update_file);
    assert_eq!(
        validator_registry_root(
            &read_validator_registry_file(&registry_path).expect("registry after final rotation"),
            &validators,
        )
        .expect("root after final rotation"),
        final_root
    );
    write_validator_key_file(&keys_path, &keys_with_subject_record(&original_keys, &final_key))
        .expect("stage final signing key");
    assert_eq!(chain_tip_height(&data_dir), 3);

    // Height 4: the first certified round after the superseding history must
    // commit instead of reapplying the drill rotation.
    let transfer_batch_file = data_dir.join("continuation-transfer.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: format!("pf{:0<38}", "supersededcontinuation"),
        amount: 7,
        batch_file: transfer_batch_file.clone(),
    })
    .expect("create continuation transfer batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: transfer_batch_file,
        certificate_file: None,
    })
    .expect("continuation past superseded registry history must commit the next height");
    assert_eq!(chain_tip_height(&data_dir), 4);
    assert_eq!(
        validator_registry_root(
            &read_validator_registry_file(&registry_path).expect("registry after continuation"),
            &validators,
        )
        .expect("root after continuation"),
        final_root
    );

    // The activation scan itself must report the whole recorded history as
    // applied with no further changes.
    let governance = store.read_governance().expect("governance");
    assert_eq!(governance.validator_registry_updates.len(), 3);
    let live_update =
        live_validator_registry_after_due_updates(&store, &genesis, &governance, 5)
            .expect("live activation scan over superseded history");
    assert!(live_update.is_none(), "superseded history must not reapply");

    fs::remove_dir_all(data_dir).expect("cleanup superseded rotation continuation data");
}

struct RotationHistoryFixture {
    data_dir: PathBuf,
    store: NodeStore,
    genesis: Genesis,
    validators: Vec<String>,
    registry_path: PathBuf,
    base_registry: ValidatorRegistry,
    first_registry: ValidatorRegistry,
    second_registry: ValidatorRegistry,
    third_registry: ValidatorRegistry,
    first_update: ValidatorRegistryUpdateRecord,
    second_update: ValidatorRegistryUpdateRecord,
    third_update: ValidatorRegistryUpdateRecord,
}

/// Chained rotation history for the same subject without committing blocks:
/// base -> first (activation 1) -> second (activation 2) -> third
/// (activation 3), each to a distinct key so no two registry states repeat.
fn rotation_history_fixture(label: &str) -> RotationHistoryFixture {
    let data_dir = unique_test_dir(label);
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: format!("{label}-chain"),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init rotation history fixture");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let validators = local_validator_ids(4).expect("validators");
    let registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let base_registry = read_validator_registry_file(&registry_path).expect("registry");
    let base_record = validator_registry_record(&base_registry, CONTINUATION_SUBJECT)
        .expect("subject record")
        .clone();
    let first_key =
        create_validator_key_record(CONTINUATION_SUBJECT.to_string()).expect("first key");
    let second_key =
        create_validator_key_record(CONTINUATION_SUBJECT.to_string()).expect("second key");
    let third_key =
        create_validator_key_record(CONTINUATION_SUBJECT.to_string()).expect("third key");
    let first_registry = registry_with_subject_key(
        &base_registry,
        CONTINUATION_SUBJECT,
        &first_key.algorithm_id,
        &first_key.public_key_hex,
    );
    let second_registry = registry_with_subject_key(
        &base_registry,
        CONTINUATION_SUBJECT,
        &second_key.algorithm_id,
        &second_key.public_key_hex,
    );
    let third_registry = registry_with_subject_key(
        &base_registry,
        CONTINUATION_SUBJECT,
        &third_key.algorithm_id,
        &third_key.public_key_hex,
    );
    let base_entry = continuation_registry_entry(
        &base_record.node_id,
        &base_record.algorithm_id,
        &base_record.public_key_hex,
    );
    let first_entry = continuation_registry_entry(
        &first_key.node_id,
        &first_key.algorithm_id,
        &first_key.public_key_hex,
    );
    let second_entry = continuation_registry_entry(
        &second_key.node_id,
        &second_key.algorithm_id,
        &second_key.public_key_hex,
    );
    let third_entry = continuation_registry_entry(
        &third_key.node_id,
        &third_key.algorithm_id,
        &third_key.public_key_hex,
    );
    let base_root = validator_registry_root(&base_registry, &validators).expect("base root");
    let first_root = validator_registry_root(&first_registry, &validators).expect("first root");
    let second_root = validator_registry_root(&second_registry, &validators).expect("second root");
    let third_root = validator_registry_root(&third_registry, &validators).expect("third root");
    let (first_update, _) = certified_subject_rotation_update(
        &data_dir,
        &validators,
        "first-rotation",
        1,
        &base_root,
        &first_root,
        &base_entry,
        &first_entry,
    );
    let (second_update, _) = certified_subject_rotation_update(
        &data_dir,
        &validators,
        "second-rotation",
        2,
        &first_root,
        &second_root,
        &first_entry,
        &second_entry,
    );
    let (third_update, _) = certified_subject_rotation_update(
        &data_dir,
        &validators,
        "third-rotation",
        3,
        &second_root,
        &third_root,
        &second_entry,
        &third_entry,
    );
    RotationHistoryFixture {
        data_dir,
        store,
        genesis,
        validators,
        registry_path,
        base_registry,
        first_registry,
        second_registry,
        third_registry,
        first_update,
        second_update,
        third_update,
    }
}

impl RotationHistoryFixture {
    fn write_registry(&self, registry: &ValidatorRegistry) {
        write_validator_registry_file(&self.registry_path, registry)
            .expect("write registry fixture state");
    }

    fn write_recorded_updates(&self, updates: Vec<ValidatorRegistryUpdateRecord>) {
        let mut governance = self.store.read_governance().expect("governance");
        governance.validator_registry_updates = updates;
        self.store
            .write_governance(&governance)
            .expect("write governance fixture state");
    }

    fn live_scan(&self, block_height: u64) -> io::Result<Option<ValidatorRegistry>> {
        let governance = self.store.read_governance().expect("governance");
        live_validator_registry_after_due_updates(
            &self.store,
            &self.genesis,
            &governance,
            block_height,
        )
    }

    fn registry_file_root(&self) -> String {
        validator_registry_root(
            &read_validator_registry_file(&self.registry_path).expect("registry file"),
            &self.validators,
        )
        .expect("registry file root")
    }

    fn cleanup(self) {
        fs::remove_dir_all(self.data_dir).expect("cleanup rotation history fixture");
    }
}

#[test]
fn live_registry_activation_treats_superseded_rotations_as_applied_history() {
    let fixture = rotation_history_fixture("postfiat-superseded-rotation-applied-test");

    // Stale, superseded history: the registry already reflects the second
    // rotation, so the first rotation is applied history even though its
    // roots are no longer reproducible.
    fixture.write_registry(&fixture.second_registry);
    fixture.write_recorded_updates(vec![
        fixture.first_update.clone(),
        fixture.second_update.clone(),
    ]);
    let unchanged = fixture.live_scan(3).expect("superseded history scan");
    assert!(unchanged.is_none(), "superseded rotation must not reapply");

    // A due pending update after superseded history must still apply.
    fixture.write_recorded_updates(vec![
        fixture.first_update.clone(),
        fixture.second_update.clone(),
        fixture.third_update.clone(),
    ]);
    let advanced = fixture
        .live_scan(3)
        .expect("pending update after superseded history")
        .expect("pending update must apply");
    assert_eq!(
        validator_registry_root(&advanced, &fixture.validators).expect("advanced root"),
        validator_registry_root(&fixture.third_registry, &fixture.validators)
            .expect("third root"),
    );

    // A duplicated already-applied record stays inert.
    fixture.write_registry(&fixture.first_registry);
    fixture.write_recorded_updates(vec![
        fixture.first_update.clone(),
        fixture.first_update.clone(),
    ]);
    let duplicate = fixture.live_scan(3).expect("duplicated history scan");
    assert!(duplicate.is_none(), "duplicated applied history must stay inert");

    fixture.cleanup();
}

#[test]
fn live_registry_activation_fails_closed_on_wrong_root_reordered_or_missing_history() {
    let fixture = rotation_history_fixture("postfiat-rotation-history-fail-closed-test");
    let base_root = fixture.registry_file_root();

    // Reordered history: the second rotation cannot apply before the first.
    fixture.write_registry(&fixture.base_registry);
    fixture.write_recorded_updates(vec![
        fixture.second_update.clone(),
        fixture.first_update.clone(),
    ]);
    let reordered_error = fixture
        .live_scan(3)
        .expect_err("reordered history must fail closed");
    assert!(
        reordered_error
            .to_string()
            .contains("previous validator registry root mismatch"),
        "{reordered_error}"
    );
    assert_eq!(fixture.registry_file_root(), base_root);

    // Missing history: a chained update without its predecessor must fail
    // closed instead of applying against the wrong registry.
    fixture.write_recorded_updates(vec![fixture.second_update.clone()]);
    let missing_error = fixture
        .live_scan(3)
        .expect_err("missing predecessor history must fail closed");
    assert!(
        missing_error
            .to_string()
            .contains("previous validator registry root mismatch"),
        "{missing_error}"
    );
    assert_eq!(fixture.registry_file_root(), base_root);

    // Wrong-root pending history: the first rotation recorded *after* the
    // update it chains below is internally certified but does not chain from
    // the current registry. It must fail closed without durable mutation
    // instead of being silently absorbed.
    fixture.write_registry(&fixture.second_registry);
    let second_root =
        validator_registry_root(&fixture.second_registry, &fixture.validators)
            .expect("second root");
    fixture.write_recorded_updates(vec![
        fixture.second_update.clone(),
        fixture.first_update.clone(),
    ]);
    let wrong_root_error = fixture
        .live_scan(3)
        .expect_err("wrong-root pending update must fail closed");
    assert!(
        wrong_root_error
            .to_string()
            .contains("previous validator registry root mismatch"),
        "{wrong_root_error}"
    );
    assert_eq!(fixture.registry_file_root(), second_root);

    fixture.cleanup();
}

/// The ordered (commit-side) activation path shares the applied-prefix rule:
/// an admit-then-remove pair whose net effect is already reflected must not
/// resurrect the removed validator when the history is scanned again.
#[test]
fn ordered_activation_does_not_resurrect_superseded_membership_history() {
    let data_dir = unique_test_dir("postfiat-superseded-membership-history-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-superseded-membership-history".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init superseded membership history test");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let validators = local_validator_ids(4).expect("validators");
    let registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let base_registry = read_validator_registry_file(&registry_path).expect("registry");
    let base_root = validator_registry_root(&base_registry, &validators).expect("base root");

    let admitted_key =
        create_validator_key_record("validator-4".to_string()).expect("admitted key");
    let admitted_entry = continuation_registry_entry(
        &admitted_key.node_id,
        &admitted_key.algorithm_id,
        &admitted_key.public_key_hex,
    );
    let mut admitted_registry = base_registry.clone();
    admitted_registry.validators.push(ValidatorRegistryRecord {
        node_id: admitted_key.node_id.clone(),
        algorithm_id: admitted_key.algorithm_id.clone(),
        public_key_hex: admitted_key.public_key_hex.clone(),
    });
    sort_validator_registry_records(&mut admitted_registry.validators);
    let admitted_validators = local_validator_ids(5).expect("admitted validators");
    let admitted_root =
        validator_registry_root(&admitted_registry, &admitted_validators).expect("admitted root");

    let admitted_entry_file = data_dir.join("membership-admit-entry.json");
    write_continuation_entry_file(&admitted_entry_file, &admitted_entry);
    let admit_update = create_validator_registry_update(ValidatorRegistryUpdateOptions {
        data_dir: data_dir.clone(),
        validators: validators.clone(),
        support: validators.clone(),
        activation_height: 1,
        previous_registry_root: base_root.clone(),
        new_registry_root: admitted_root.clone(),
        previous_validators: validators.clone(),
        new_validators: admitted_validators.clone(),
        operation: VALIDATOR_REGISTRY_OP_ADMIT.to_string(),
        subject_node_id: admitted_key.node_id.clone(),
        previous_record_file: None,
        new_record_file: Some(admitted_entry_file.clone()),
        update_file: data_dir.join("membership-admit.update.json"),
    })
    .expect("create admit update");
    let remove_update = create_validator_registry_update(ValidatorRegistryUpdateOptions {
        data_dir: data_dir.clone(),
        validators: admitted_validators.clone(),
        support: admitted_validators.clone(),
        activation_height: 2,
        previous_registry_root: admitted_root,
        new_registry_root: base_root.clone(),
        previous_validators: admitted_validators,
        new_validators: validators.clone(),
        operation: VALIDATOR_REGISTRY_OP_REMOVE.to_string(),
        subject_node_id: admitted_key.node_id.clone(),
        previous_record_file: Some(admitted_entry_file),
        new_record_file: None,
        update_file: data_dir.join("membership-remove.update.json"),
    })
    .expect("create remove update");

    // The registry already reflects the accepted admit-then-remove history.
    let mut governance = store.read_governance().expect("governance");
    governance.validator_registry_updates = vec![admit_update, remove_update];
    let activations =
        activate_due_validator_registry_updates_for_commit(&store, &genesis, &mut governance, 5)
            .expect("ordered activation over settled membership history");
    assert!(
        activations.registry.is_none(),
        "settled admit-then-remove history must not resurrect the removed validator"
    );
    let persisted_registry = read_validator_registry_file(&registry_path).expect("registry file");
    assert!(
        persisted_registry
            .validators
            .iter()
            .all(|record| record.node_id != admitted_key.node_id),
        "removed validator must stay absent from the persisted registry"
    );

    fs::remove_dir_all(data_dir).expect("cleanup superseded membership history data");
}

/// Replay/export shape of the fleet-wide block-924 snapshot failure: the
/// recorded drill rotation is scoped to the subject record only, so live
/// activation never applies it, and the later legitimate rotation of the
/// same record supersedes it. The certificate replay behind checkpoint
/// export must treat the recorded certificate lineage as the applied
/// history instead of reapplying the superseded rotation and failing with
/// `block 2 certificate registry root mismatch`.
#[test]
fn superseded_unapplied_rotation_history_replays_for_checkpoint_export() {
    let data_dir = unique_test_dir("postfiat-superseded-unapplied-rotation-export-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-superseded-unapplied-rotation-export".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init superseded unapplied rotation export test");
    let validators = local_validator_ids(4).expect("validators");
    let subject_validators = vec![CONTINUATION_SUBJECT.to_string()];
    let registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let keys_path = data_dir.join(VALIDATOR_KEYS_FILE);
    let original_registry = read_validator_registry_file(&registry_path).expect("registry");
    let original_keys = read_validator_key_file(&keys_path).expect("validator keys");
    let original_record = validator_registry_record(&original_registry, CONTINUATION_SUBJECT)
        .expect("subject record")
        .clone();
    let drill_key =
        create_validator_key_record(CONTINUATION_SUBJECT.to_string()).expect("drill key");
    let final_key =
        create_validator_key_record(CONTINUATION_SUBJECT.to_string()).expect("final key");
    let drill_registry = registry_with_subject_key(
        &original_registry,
        CONTINUATION_SUBJECT,
        &drill_key.algorithm_id,
        &drill_key.public_key_hex,
    );
    let final_registry = registry_with_subject_key(
        &original_registry,
        CONTINUATION_SUBJECT,
        &final_key.algorithm_id,
        &final_key.public_key_hex,
    );
    let original_root =
        validator_registry_root(&original_registry, &validators).expect("original root");
    let final_root = validator_registry_root(&final_registry, &validators).expect("final root");
    let subject_original_root =
        validator_registry_root(&original_registry, &subject_validators).expect("subject root");
    let subject_drill_root = validator_registry_root(&drill_registry, &subject_validators)
        .expect("subject drill root");
    let original_entry = continuation_registry_entry(
        &original_record.node_id,
        &original_record.algorithm_id,
        &original_record.public_key_hex,
    );
    let drill_entry = continuation_registry_entry(
        &drill_key.node_id,
        &drill_key.algorithm_id,
        &drill_key.public_key_hex,
    );
    let final_entry = continuation_registry_entry(
        &final_key.node_id,
        &final_key.algorithm_id,
        &final_key.public_key_hex,
    );

    // Height 1: the drill rotation is recorded subject-scoped, so live
    // activation skips it forever and the persisted registry keeps the
    // original key.
    let (_, drill_update_file) = certified_subject_rotation_update(
        &data_dir,
        &subject_validators,
        "drill-subject-rotation",
        1,
        &subject_original_root,
        &subject_drill_root,
        &original_entry,
        &drill_entry,
    );
    commit_governance_registry_update(&data_dir, "drill-subject-rotation", drill_update_file);
    assert_eq!(
        validator_registry_root(
            &read_validator_registry_file(&registry_path).expect("registry after drill record"),
            &validators,
        )
        .expect("root after drill record"),
        original_root,
        "live activation must not apply the subject-scoped drill rotation"
    );

    // Height 2: the legitimate rotation of the same record supersedes the
    // recorded drill rotation.
    let (_, final_update_file) = certified_subject_rotation_update(
        &data_dir,
        &validators,
        "final-rotation",
        2,
        &original_root,
        &final_root,
        &original_entry,
        &final_entry,
    );
    commit_governance_registry_update(&data_dir, "final-rotation", final_update_file);
    assert_eq!(
        validator_registry_root(
            &read_validator_registry_file(&registry_path).expect("registry after final rotation"),
            &validators,
        )
        .expect("root after final rotation"),
        final_root
    );
    write_validator_key_file(&keys_path, &keys_with_subject_record(&original_keys, &final_key))
        .expect("stage final signing key");

    // Height 3: continuation past the superseded history.
    let transfer_batch_file = data_dir.join("export-continuation-transfer.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: format!("pf{:0<38}", "supersededexportcontinuation"),
        amount: 5,
        batch_file: transfer_batch_file.clone(),
    })
    .expect("create export continuation transfer batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: transfer_batch_file,
        certificate_file: None,
    })
    .expect("continuation past superseded registry history must commit");
    assert_eq!(chain_tip_height(&data_dir), 3);

    // The certificate replay used by snapshot checkpoint export must accept
    // the recorded certificate lineage over the superseded history.
    let report = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("certificate replay over superseded unapplied rotation history");
    assert!(report.verified);
    assert_eq!(report.block_count, 3);

    let snapshot_dir = data_dir.join("superseded-history-snapshot");
    export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir,
    })
    .expect("snapshot export over superseded unapplied rotation history");

    fs::remove_dir_all(data_dir).expect("cleanup superseded unapplied rotation export data");
}

/// Replay/export shape for a recorded signed restore of off-chain drill
/// history: the recorded rollback's predecessor state never entered the
/// certified lineage, so its effect is already reflected by the replayed
/// registry. Certificate replay must treat it as applied history instead of
/// failing its previous-root check.
#[test]
fn recorded_offchain_rollback_history_replays_for_checkpoint_export() {
    let data_dir = unique_test_dir("postfiat-recorded-offchain-rollback-export-test");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-recorded-offchain-rollback-export".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init recorded offchain rollback export test");
    let validators = local_validator_ids(4).expect("validators");
    let registry_path = data_dir.join(VALIDATOR_REGISTRY_FILE);
    let original_registry = read_validator_registry_file(&registry_path).expect("registry");
    let original_record = validator_registry_record(&original_registry, CONTINUATION_SUBJECT)
        .expect("subject record")
        .clone();
    let drill_key =
        create_validator_key_record(CONTINUATION_SUBJECT.to_string()).expect("drill key");
    let drill_registry = registry_with_subject_key(
        &original_registry,
        CONTINUATION_SUBJECT,
        &drill_key.algorithm_id,
        &drill_key.public_key_hex,
    );
    let original_root =
        validator_registry_root(&original_registry, &validators).expect("original root");
    let drill_root = validator_registry_root(&drill_registry, &validators).expect("drill root");
    let original_entry = continuation_registry_entry(
        &original_record.node_id,
        &original_record.algorithm_id,
        &original_record.public_key_hex,
    );
    let drill_entry = continuation_registry_entry(
        &drill_key.node_id,
        &drill_key.algorithm_id,
        &drill_key.public_key_hex,
    );

    // Height 1: record the signed rollback of an off-chain drill rotation
    // whose effect never reached any certified round; the persisted registry
    // already holds the restored original key.
    let (_, rollback_update_file) = certified_subject_rotation_update(
        &data_dir,
        &validators,
        "offchain-rollback",
        1,
        &drill_root,
        &original_root,
        &drill_entry,
        &original_entry,
    );
    commit_governance_registry_update(&data_dir, "offchain-rollback", rollback_update_file);
    assert_eq!(
        validator_registry_root(
            &read_validator_registry_file(&registry_path).expect("registry after rollback record"),
            &validators,
        )
        .expect("root after rollback record"),
        original_root,
        "the recorded restore is already reflected by the persisted registry"
    );

    // Height 2: continuation past the recorded restore.
    let transfer_batch_file = data_dir.join("rollback-continuation-transfer.batch.json");
    create_transfer_batch(BatchTransferOptions {
        data_dir: data_dir.clone(),
        key_file: None,
        to: format!("pf{:0<38}", "rollbackexportcontinuation"),
        amount: 3,
        batch_file: transfer_batch_file.clone(),
    })
    .expect("create rollback continuation transfer batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: transfer_batch_file,
        certificate_file: None,
    })
    .expect("continuation past recorded off-chain rollback must commit");
    assert_eq!(chain_tip_height(&data_dir), 2);

    let report = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("certificate replay over recorded off-chain rollback history");
    assert!(report.verified);
    assert_eq!(report.block_count, 2);

    let snapshot_dir = data_dir.join("recorded-rollback-snapshot");
    export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir,
    })
    .expect("snapshot export over recorded off-chain rollback history");

    fs::remove_dir_all(data_dir).expect("cleanup recorded offchain rollback export data");
}
