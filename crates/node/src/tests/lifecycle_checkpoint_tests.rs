use super::*;

fn checkpoint_identity_pins() -> LifecycleCheckpointIdentityPins {
    LifecycleCheckpointIdentityPins {
        asset_id: "aa".repeat(48),
        successor_profile_id: "ab".repeat(48),
        source_manifest_hash: "ac".repeat(48),
        valuation_policy_hash: "ad".repeat(32),
        guest_elf_sha256: "ae".repeat(32),
        sp1_verification_key: format!("0x{}", "af".repeat(32)),
        epoch7_proof_sha256: "b0".repeat(32),
        epoch7_public_values_sha256: "b1".repeat(32),
        epoch8_proof_sha256: "b2".repeat(32),
        epoch8_public_values_sha256: "b3".repeat(32),
    }
}

fn copy_tree(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).expect("create copy target");
    for entry in std::fs::read_dir(src).expect("read copy source") {
        let entry = entry.expect("copy entry");
        let target = dst.join(entry.file_name());
        if entry.file_type().expect("entry type").is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy file");
        }
    }
}

fn rewrite_node_id(data_dir: &Path, node_id: &str) {
    let store = NodeStore::new(data_dir);
    let mut state = store.read_node_state().expect("read node state");
    state.node_id = node_id.to_string();
    store.write_node_state(&state).expect("write node state");
}

struct CheckpointFixture {
    root: PathBuf,
    checkpoint_dir: PathBuf,
    trusted_key_file: PathBuf,
    manifest: LifecycleCheckpointManifest,
}

impl CheckpointFixture {
    fn new(label: &str) -> Self {
        let root = unique_test_dir(&format!("postfiat-lifecycle-checkpoint-{label}"));
        let seed_dir = root.join("seed");
        init(InitOptions {
            data_dir: seed_dir.clone(),
            chain_id: "postfiat-local".to_string(),
            node_id: "validator-0".to_string(),
            validator_count: 3,
        })
        .expect("init lifecycle checkpoint seed");
        let mut data_dirs = Vec::new();
        for index in 0..3 {
            let node_id = format!("validator-{index}");
            let data_dir = root.join(&node_id);
            copy_tree(&seed_dir, &data_dir);
            rewrite_node_id(&data_dir, &node_id);
            data_dirs.push(data_dir);
        }

        let publisher_key_file = root.join("checkpoint-publisher.private.json");
        let publisher_key = create_dev_key_file().expect("create checkpoint publisher key");
        write_key_file(&publisher_key_file, &publisher_key)
            .expect("write checkpoint publisher key");
        let trusted_key_file = root.join("checkpoint-publisher.public.json");
        export_snapshot_publisher_public_key(SnapshotPublisherKeyExportOptions {
            publisher_key_file: publisher_key_file.clone(),
            public_key_file: trusted_key_file.clone(),
        })
        .expect("export trusted checkpoint publisher key");

        let checkpoint_dir = root.join("checkpoint");
        let manifest = create_lifecycle_checkpoint(LifecycleCheckpointCreateOptions {
            data_dirs,
            checkpoint_dir: checkpoint_dir.clone(),
            publisher_key_file,
            identity_pins: checkpoint_identity_pins(),
            source_dirty: false,
            creation_command: format!("test fixture {label}"),
            snapshot_basis: LifecycleCheckpointSnapshotBasis::FullHistory,
        })
        .expect("create lifecycle checkpoint");
        Self {
            root,
            checkpoint_dir,
            trusted_key_file,
            manifest,
        }
    }

    fn import_options(&self, label: &str) -> LifecycleCheckpointImportOptions {
        LifecycleCheckpointImportOptions {
            checkpoint_dir: self.checkpoint_dir.clone(),
            trusted_publisher_key_file: self.trusted_key_file.clone(),
            target_root: self.root.join(format!("restored-{label}")),
            report_file: self.root.join(format!("import-report-{label}.json")),
        }
    }

    fn manifest_path(&self) -> PathBuf {
        self.checkpoint_dir.join(LIFECYCLE_CHECKPOINT_MANIFEST_FILE)
    }

    fn read_report(&self, label: &str) -> LifecycleCheckpointImportReport {
        serde_json::from_slice(
            &std::fs::read(self.root.join(format!("import-report-{label}.json")))
                .expect("import report must exist"),
        )
        .expect("parse import report")
    }

    fn cleanup(self) {
        std::fs::remove_dir_all(self.root).expect("cleanup lifecycle checkpoint fixture");
    }
}

#[test]
fn lifecycle_checkpoint_roundtrip_restores_all_validators_and_reports() {
    let fixture = CheckpointFixture::new("roundtrip");
    assert_eq!(fixture.manifest.validators.len(), 3);
    assert!(fixture.manifest.private_material_absent);

    let report = import_lifecycle_checkpoint(fixture.import_options("ok"))
        .expect("import lifecycle checkpoint");
    assert!(report.ok);
    assert!(report.first_failure.is_none());
    assert_eq!(report.validators.len(), 3);
    for validator in &report.validators {
        assert_eq!(validator.block_height, fixture.manifest.block_height);
        assert_eq!(validator.block_tip_hash, fixture.manifest.block_tip_hash);
        assert_eq!(validator.state_root, fixture.manifest.state_root);
        assert!(
            !Path::new(&validator.data_dir)
                .join(VALIDATOR_KEYS_FILE)
                .exists(),
            "restored validator must not contain signer material"
        );
    }
    let written = fixture.read_report("ok");
    assert_eq!(written, report);
    fixture.cleanup();
}

#[test]
fn lifecycle_checkpoint_import_rejects_tampered_manifest_with_failure_report() {
    let fixture = CheckpointFixture::new("tampered-manifest");
    let manifest_path = fixture.manifest_path();
    let mut tampered: LifecycleCheckpointManifest = serde_json::from_slice(
        &std::fs::read(&manifest_path).expect("read checkpoint manifest"),
    )
    .expect("parse checkpoint manifest");
    tampered.block_height += 1;
    atomic_write(
        &manifest_path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&tampered).expect("tampered manifest json")
        ),
    )
    .expect("write tampered manifest");

    let error = import_lifecycle_checkpoint(fixture.import_options("tampered"))
        .expect_err("tampered manifest must fail");
    assert!(
        error.to_string().contains("signature verification"),
        "{error}"
    );
    let report = fixture.read_report("tampered");
    assert!(!report.ok);
    assert!(report
        .first_failure
        .expect("failure recorded")
        .contains("signature verification"));
    fixture.cleanup();
}

#[test]
fn lifecycle_checkpoint_import_rejects_schema_drift_and_unknown_fields() {
    let fixture = CheckpointFixture::new("schema-drift");
    let manifest_path = fixture.manifest_path();
    let mut value: serde_json::Value = serde_json::from_slice(
        &std::fs::read(&manifest_path).expect("read checkpoint manifest"),
    )
    .expect("parse checkpoint manifest");
    value["unexpected_field"] = serde_json::Value::Bool(true);
    atomic_write(
        &manifest_path,
        serde_json::to_string_pretty(&value).expect("drifted manifest json"),
    )
    .expect("write drifted manifest");

    let error = import_lifecycle_checkpoint(fixture.import_options("drift"))
        .expect_err("unknown manifest field must fail");
    assert!(error.to_string().contains("unknown field"), "{error}");
    assert!(!fixture.read_report("drift").ok);
    fixture.cleanup();
}

#[test]
fn lifecycle_checkpoint_import_rejects_extra_entries_and_symlinks() {
    let fixture = CheckpointFixture::new("containment");
    std::fs::write(fixture.checkpoint_dir.join("extra.bin"), b"stray").expect("write extra file");
    let error = import_lifecycle_checkpoint(fixture.import_options("extra"))
        .expect_err("extra checkpoint entry must fail");
    assert!(error.to_string().contains("unexpected entry"), "{error}");
    assert!(!fixture.read_report("extra").ok);
    std::fs::remove_file(fixture.checkpoint_dir.join("extra.bin")).expect("remove extra file");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink("/etc/hostname", fixture.checkpoint_dir.join("validator-0-link"))
            .expect("create symlink");
        let error = import_lifecycle_checkpoint(fixture.import_options("symlink"))
            .expect_err("symlink inside checkpoint must fail");
        assert!(error.to_string().contains("symlink"), "{error}");
        assert!(!fixture.read_report("symlink").ok);
        std::fs::remove_file(fixture.checkpoint_dir.join("validator-0-link"))
            .expect("remove symlink");
    }
    fixture.cleanup();
}

#[test]
fn lifecycle_checkpoint_import_rejects_tampered_validator_snapshot() {
    let fixture = CheckpointFixture::new("tampered-snapshot");
    let ledger_path = fixture.checkpoint_dir.join("validator-1").join(LEDGER_FILE);
    let mut bytes = std::fs::read(&ledger_path).expect("read snapshot ledger");
    let last = bytes.len() - 2;
    bytes[last] = bytes[last].wrapping_add(1);
    std::fs::write(&ledger_path, bytes).expect("write tampered snapshot ledger");

    let error = import_lifecycle_checkpoint(fixture.import_options("snapshot"))
        .expect_err("tampered validator snapshot must fail");
    let message = error.to_string();
    assert!(
        message.contains("hash") || message.contains("ledger") || message.contains("parse"),
        "{message}"
    );
    let report = fixture.read_report("snapshot");
    assert!(!report.ok);
    assert!(report.first_failure.is_some());
    fixture.cleanup();
}

#[test]
fn lifecycle_checkpoint_import_rejects_wrong_trusted_publisher() {
    let fixture = CheckpointFixture::new("wrong-publisher");
    let wrong_private = fixture.root.join("wrong-publisher.private.json");
    let wrong_key = create_dev_key_file().expect("create wrong publisher key");
    write_key_file(&wrong_private, &wrong_key).expect("write wrong publisher key");
    let wrong_public = fixture.root.join("wrong-publisher.public.json");
    export_snapshot_publisher_public_key(SnapshotPublisherKeyExportOptions {
        publisher_key_file: wrong_private,
        public_key_file: wrong_public.clone(),
    })
    .expect("export wrong publisher key");

    let mut options = fixture.import_options("wrong-publisher");
    options.trusted_publisher_key_file = wrong_public;
    let error = import_lifecycle_checkpoint(options)
        .expect_err("wrong trusted publisher must fail");
    assert!(
        error
            .to_string()
            .contains("does not match trusted publisher"),
        "{error}"
    );
    assert!(!fixture.read_report("wrong-publisher").ok);
    fixture.cleanup();
}

#[test]
fn lifecycle_checkpoint_creation_rejects_divergent_validators() {
    let root = unique_test_dir("postfiat-lifecycle-checkpoint-divergent");
    let first = root.join("validator-0");
    init(InitOptions {
        data_dir: first.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 3,
    })
    .expect("init first validator");
    // A separately initialized chain has a different genesis identity.
    let second = root.join("validator-1");
    init(InitOptions {
        data_dir: second.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-1".to_string(),
        validator_count: 3,
    })
    .expect("init second validator");

    let publisher_key_file = root.join("publisher.private.json");
    let publisher_key = create_dev_key_file().expect("create publisher key");
    write_key_file(&publisher_key_file, &publisher_key).expect("write publisher key");
    let error = create_lifecycle_checkpoint(LifecycleCheckpointCreateOptions {
        data_dirs: vec![first, second],
        checkpoint_dir: root.join("checkpoint"),
        publisher_key_file,
        identity_pins: checkpoint_identity_pins(),
        source_dirty: false,
        creation_command: "divergence test".to_string(),
        snapshot_basis: LifecycleCheckpointSnapshotBasis::FullHistory,
    })
    .expect_err("divergent validators must not checkpoint");
    assert!(error.to_string().contains("not converged"), "{error}");
    std::fs::remove_dir_all(root).expect("cleanup divergence fixture");
}
