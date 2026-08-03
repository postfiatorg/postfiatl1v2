//! Six-validator lifecycle checkpoint: a signed, content-addressed bundle of
//! per-validator signed snapshots plus pinned lifecycle identities.
//!
//! This is the acceleration layer for the A666 public-successor lifecycle
//! qualification (recovery spec section 6.2). A checkpoint captures the exact
//! converged pre-migration state of every validator so the lifecycle test can
//! start in seconds instead of regenerating hundreds of blocks. The importer
//! fails closed: publisher signature, schema, content hashes, chain identity,
//! terminal tuple, entry allowlist, and symlink containment are all verified
//! before any validator state is loaded, and each per-validator import
//! re-verifies the underlying signed snapshot and replays its history via the
//! full-history snapshot basis. An import report is written even on failure.

use super::*;

pub const LIFECYCLE_CHECKPOINT_SCHEMA: &str = "postfiat.a666.lifecycle-checkpoint.v1";
pub const LIFECYCLE_CHECKPOINT_MANIFEST_FILE: &str = "lifecycle-checkpoint.signed-manifest.json";
pub const LIFECYCLE_CHECKPOINT_SIGNATURE_CONTEXT: &[u8] =
    b"postfiat.a666.lifecycle-checkpoint.signature.v1";
pub const LIFECYCLE_CHECKPOINT_IMPORT_REPORT_SCHEMA: &str =
    "postfiat.a666.lifecycle-checkpoint.import-report.v1";

/// Content identities the checkpointed lifecycle must run against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCheckpointIdentityPins {
    pub asset_id: String,
    pub successor_profile_id: String,
    pub source_manifest_hash: String,
    pub valuation_policy_hash: String,
    pub guest_elf_sha256: String,
    pub sp1_verification_key: String,
    pub epoch7_proof_sha256: String,
    pub epoch7_public_values_sha256: String,
    pub epoch8_proof_sha256: String,
    pub epoch8_public_values_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCheckpointValidatorEntry {
    pub node_id: String,
    /// Directory name inside the checkpoint root holding this validator's
    /// signed snapshot. Must be a plain path component.
    pub snapshot_dir: String,
    pub signed_manifest_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCheckpointManifest {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub block_height: u64,
    pub block_tip_hash: String,
    pub state_root: String,
    pub validators: Vec<LifecycleCheckpointValidatorEntry>,
    pub identity_pins: LifecycleCheckpointIdentityPins,
    pub source_git_revision: String,
    pub source_dirty: bool,
    pub creation_command: String,
    pub created_unix: u64,
    pub private_material_absent: bool,
    pub publisher: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleCheckpointCreateOptions {
    pub data_dirs: Vec<PathBuf>,
    pub checkpoint_dir: PathBuf,
    pub publisher_key_file: PathBuf,
    pub identity_pins: LifecycleCheckpointIdentityPins,
    pub source_dirty: bool,
    pub creation_command: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleCheckpointImportOptions {
    pub checkpoint_dir: PathBuf,
    pub trusted_publisher_key_file: PathBuf,
    /// Each validator is restored into `target_root/<node_id>`.
    pub target_root: PathBuf,
    /// The import report is written here even when the import fails.
    pub report_file: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCheckpointImportCheck {
    pub name: String,
    pub expected: String,
    pub observed: String,
    pub ok: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCheckpointImportedValidator {
    pub node_id: String,
    pub data_dir: String,
    pub block_height: u64,
    pub block_tip_hash: String,
    pub state_root: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LifecycleCheckpointImportReport {
    pub schema: String,
    pub started_unix: u64,
    pub finished_unix: u64,
    pub checkpoint_dir: String,
    pub checkpoint_manifest_sha256: Option<String>,
    pub checks: Vec<LifecycleCheckpointImportCheck>,
    pub validators: Vec<LifecycleCheckpointImportedValidator>,
    pub first_failure: Option<String>,
    pub ok: bool,
}

fn checkpoint_error(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

fn lifecycle_checkpoint_signature_payload(
    manifest: &LifecycleCheckpointManifest,
) -> io::Result<Vec<u8>> {
    let mut unsigned = manifest.clone();
    unsigned.signature_hex = String::new();
    serde_json::to_vec(&unsigned).map_err(|error| checkpoint_error(error.to_string()))
}

fn require_plain_component(name: &str) -> io::Result<()> {
    let valid = !name.is_empty()
        && name != "."
        && name != ".."
        && !name.contains('/')
        && !name.contains('\\');
    if !valid {
        return Err(checkpoint_error(format!(
            "checkpoint entry name `{name}` is not a plain path component"
        )));
    }
    Ok(())
}

fn reject_symlinks_recursively(root: &Path) -> io::Result<()> {
    let metadata = std::fs::symlink_metadata(root)?;
    if metadata.file_type().is_symlink() {
        return Err(checkpoint_error(format!(
            "checkpoint path `{}` is a symlink; symlinks are rejected",
            root.display()
        )));
    }
    if metadata.is_dir() {
        for entry in std::fs::read_dir(root)? {
            reject_symlinks_recursively(&entry?.path())?;
        }
    }
    Ok(())
}

fn verify_lifecycle_checkpoint_manifest(
    manifest: &LifecycleCheckpointManifest,
    trusted: &SnapshotPublisherPublicKey,
) -> io::Result<()> {
    if manifest.schema != LIFECYCLE_CHECKPOINT_SCHEMA {
        return Err(checkpoint_error(format!(
            "unsupported lifecycle checkpoint schema `{}`",
            manifest.schema
        )));
    }
    if !manifest.private_material_absent {
        return Err(checkpoint_error(
            "lifecycle checkpoint must declare private material absent",
        ));
    }
    if manifest.validators.is_empty() {
        return Err(checkpoint_error(
            "lifecycle checkpoint must contain at least one validator",
        ));
    }
    if manifest.publisher != trusted.publisher
        || manifest.algorithm_id != trusted.algorithm_id
        || manifest.public_key_hex != trusted.public_key_hex
    {
        return Err(checkpoint_error(
            "lifecycle checkpoint publisher does not match trusted publisher key",
        ));
    }
    if manifest.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(checkpoint_error(format!(
            "unsupported lifecycle checkpoint algorithm `{}`",
            manifest.algorithm_id
        )));
    }
    let public_key = hex_to_bytes(&manifest.public_key_hex)
        .map_err(|error| checkpoint_error(error.to_string()))?;
    let signature = hex_to_bytes(&manifest.signature_hex)
        .map_err(|error| checkpoint_error(error.to_string()))?;
    let payload = lifecycle_checkpoint_signature_payload(manifest)?;
    if !ml_dsa_65_verify_with_context(
        &public_key,
        &payload,
        &signature,
        LIFECYCLE_CHECKPOINT_SIGNATURE_CONTEXT,
    ) {
        return Err(checkpoint_error(
            "lifecycle checkpoint manifest signature verification failed",
        ));
    }
    Ok(())
}

/// Creates a signed lifecycle checkpoint from converged validator data dirs.
pub fn create_lifecycle_checkpoint(
    options: LifecycleCheckpointCreateOptions,
) -> io::Result<LifecycleCheckpointManifest> {
    if options.data_dirs.is_empty() {
        return Err(checkpoint_error(
            "lifecycle checkpoint requires at least one validator data dir",
        ));
    }
    let mut statuses = Vec::new();
    for data_dir in &options.data_dirs {
        statuses.push(status(NodeOptions {
            data_dir: data_dir.clone(),
        })?);
    }
    let head = &statuses[0];
    let mut node_ids = std::collections::BTreeSet::new();
    for report in &statuses {
        if report.chain_id != head.chain_id
            || report.genesis_hash != head.genesis_hash
            || report.block_height != head.block_height
            || report.block_tip_hash != head.block_tip_hash
            || report.state_root != head.state_root
        {
            return Err(checkpoint_error(format!(
                "validator `{}` has not converged with `{}`; refusing to checkpoint divergent state",
                report.node_id, head.node_id
            )));
        }
        if !node_ids.insert(report.node_id.clone()) {
            return Err(checkpoint_error(format!(
                "duplicate node id `{}` in checkpoint inputs",
                report.node_id
            )));
        }
    }
    std::fs::create_dir_all(&options.checkpoint_dir)?;

    let mut validators = Vec::new();
    for (data_dir, report) in options.data_dirs.iter().zip(&statuses) {
        require_plain_component(&report.node_id)?;
        let snapshot_dir = options.checkpoint_dir.join(&report.node_id);
        export_signed_snapshot(SignedSnapshotExportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir: snapshot_dir.clone(),
            publisher_key_file: options.publisher_key_file.clone(),
        })?;
        let signed_manifest_sha256 = crate::batch_snapshot::sha256_file_hex(
            &snapshot_dir.join(crate::lifecycle_queries::SIGNED_SNAPSHOT_MANIFEST_FILE),
            "signed snapshot manifest",
        )?;
        validators.push(LifecycleCheckpointValidatorEntry {
            node_id: report.node_id.clone(),
            snapshot_dir: report.node_id.clone(),
            signed_manifest_sha256,
        });
    }

    let publisher_key = crate::storage_commit::read_key_file(&options.publisher_key_file)?;
    let mut manifest = LifecycleCheckpointManifest {
        schema: LIFECYCLE_CHECKPOINT_SCHEMA.to_string(),
        chain_id: head.chain_id.clone(),
        genesis_hash: head.genesis_hash.clone(),
        block_height: head.block_height,
        block_tip_hash: head.block_tip_hash.clone(),
        state_root: head.state_root.clone(),
        validators,
        identity_pins: options.identity_pins,
        source_git_revision: head.build_git_revision.clone(),
        source_dirty: options.source_dirty,
        creation_command: options.creation_command,
        created_unix: crate::consensus_artifacts::unix_now(),
        private_material_absent: true,
        publisher: publisher_key.address.clone(),
        algorithm_id: publisher_key.algorithm_id.clone(),
        public_key_hex: publisher_key.public_key_hex.clone(),
        signature_hex: String::new(),
    };
    let private_key = Zeroizing::new(
        hex_to_bytes(&publisher_key.private_key_hex)
            .map_err(|error| checkpoint_error(error.to_string()))?,
    );
    let payload = lifecycle_checkpoint_signature_payload(&manifest)?;
    let signature = ml_dsa_65_sign_with_context(
        &private_key,
        &payload,
        LIFECYCLE_CHECKPOINT_SIGNATURE_CONTEXT,
    )
    .map_err(|error| checkpoint_error(error.to_string()))?;
    manifest.signature_hex = bytes_to_hex(&signature);
    let trusted_self = SnapshotPublisherPublicKey {
        schema: crate::lifecycle_queries::SNAPSHOT_PUBLISHER_PUBLIC_KEY_SCHEMA.to_string(),
        publisher: manifest.publisher.clone(),
        algorithm_id: manifest.algorithm_id.clone(),
        public_key_hex: manifest.public_key_hex.clone(),
    };
    verify_lifecycle_checkpoint_manifest(&manifest, &trusted_self)?;
    let json = serde_json::to_string_pretty(&manifest)
        .map_err(|error| checkpoint_error(error.to_string()))?;
    atomic_write(
        options
            .checkpoint_dir
            .join(LIFECYCLE_CHECKPOINT_MANIFEST_FILE),
        format!("{json}\n"),
    )?;
    Ok(manifest)
}

struct ImportContext<'a> {
    options: &'a LifecycleCheckpointImportOptions,
    report: &'a mut LifecycleCheckpointImportReport,
}

impl ImportContext<'_> {
    fn check(
        &mut self,
        name: &str,
        expected: impl std::fmt::Display,
        observed: impl std::fmt::Display,
    ) -> io::Result<()> {
        let expected = expected.to_string();
        let observed = observed.to_string();
        let ok = expected == observed;
        self.report.checks.push(LifecycleCheckpointImportCheck {
            name: name.to_string(),
            expected: expected.clone(),
            observed: observed.clone(),
            ok,
        });
        if !ok {
            return Err(checkpoint_error(format!(
                "{name}: expected `{expected}`, observed `{observed}`"
            )));
        }
        Ok(())
    }
}

fn import_lifecycle_checkpoint_inner(context: &mut ImportContext<'_>) -> io::Result<()> {
    let checkpoint_dir = context.options.checkpoint_dir.clone();
    reject_symlinks_recursively(&checkpoint_dir)?;

    let manifest_path = checkpoint_dir.join(LIFECYCLE_CHECKPOINT_MANIFEST_FILE);
    context.report.checkpoint_manifest_sha256 = Some(crate::batch_snapshot::sha256_file_hex(
        &manifest_path,
        "lifecycle checkpoint manifest",
    )?);
    let manifest_text = std::fs::read_to_string(&manifest_path)?;
    let manifest: LifecycleCheckpointManifest = serde_json::from_str(&manifest_text)
        .map_err(|error| checkpoint_error(format!("lifecycle checkpoint manifest: {error}")))?;
    let trusted: SnapshotPublisherPublicKey = crate::consensus_artifacts::read_json_file(
        &context.options.trusted_publisher_key_file,
        "trusted snapshot publisher key",
    )?;
    verify_lifecycle_checkpoint_manifest(&manifest, &trusted)?;
    context.check("schema", LIFECYCLE_CHECKPOINT_SCHEMA, &manifest.schema)?;
    context.check(
        "private_material_absent",
        true,
        manifest.private_material_absent,
    )?;

    // The checkpoint root may contain only the manifest and the pinned
    // validator snapshot dirs.
    let mut allowed = std::collections::BTreeSet::new();
    allowed.insert(LIFECYCLE_CHECKPOINT_MANIFEST_FILE.to_string());
    for validator in &manifest.validators {
        require_plain_component(&validator.snapshot_dir)?;
        require_plain_component(&validator.node_id)?;
        if !allowed.insert(validator.snapshot_dir.clone()) {
            return Err(checkpoint_error(format!(
                "duplicate checkpoint snapshot dir `{}`",
                validator.snapshot_dir
            )));
        }
    }
    for entry in std::fs::read_dir(&checkpoint_dir)? {
        let name = entry?.file_name().to_string_lossy().to_string();
        if !allowed.contains(&name) {
            return Err(checkpoint_error(format!(
                "unexpected entry `{name}` in checkpoint root; refusing to import"
            )));
        }
    }

    for validator in &manifest.validators {
        let snapshot_dir = checkpoint_dir.join(&validator.snapshot_dir);
        let signed_manifest_path =
            snapshot_dir.join(crate::lifecycle_queries::SIGNED_SNAPSHOT_MANIFEST_FILE);
        let observed_sha = crate::batch_snapshot::sha256_file_hex(
            &signed_manifest_path,
            "signed snapshot manifest",
        )?;
        context.check(
            &format!("{}.signed_manifest_sha256", validator.node_id),
            &validator.signed_manifest_sha256,
            &observed_sha,
        )?;
        let signed: SignedSnapshotManifest = crate::consensus_artifacts::read_json_file(
            &signed_manifest_path,
            "signed snapshot manifest",
        )?;
        context.check(
            &format!("{}.chain_id", validator.node_id),
            &manifest.chain_id,
            &signed.manifest.chain_id,
        )?;
        context.check(
            &format!("{}.genesis_hash", validator.node_id),
            &manifest.genesis_hash,
            &signed.manifest.genesis_hash,
        )?;
        context.check(
            &format!("{}.block_height", validator.node_id),
            manifest.block_height,
            signed.manifest.block_height,
        )?;
        context.check(
            &format!("{}.block_tip_hash", validator.node_id),
            &manifest.block_tip_hash,
            &signed.manifest.block_tip_hash,
        )?;
        context.check(
            &format!("{}.state_root", validator.node_id),
            &manifest.state_root,
            &signed.manifest.state_root,
        )?;

        let data_dir = context.options.target_root.join(&validator.node_id);
        let restored = import_signed_snapshot(SignedSnapshotImportOptions {
            data_dir: data_dir.clone(),
            snapshot_dir,
            trusted_publisher_key_file: context.options.trusted_publisher_key_file.clone(),
            node_id: Some(validator.node_id.clone()),
        })?;
        context.check(
            &format!("{}.restored_height", validator.node_id),
            manifest.block_height,
            restored.block_height,
        )?;
        context.check(
            &format!("{}.restored_tip", validator.node_id),
            &manifest.block_tip_hash,
            &restored.block_tip_hash,
        )?;
        context.check(
            &format!("{}.restored_state_root", validator.node_id),
            &manifest.state_root,
            &restored.state_root,
        )?;
        context
            .report
            .validators
            .push(LifecycleCheckpointImportedValidator {
                node_id: validator.node_id.clone(),
                data_dir: data_dir.display().to_string(),
                block_height: restored.block_height,
                block_tip_hash: restored.block_tip_hash,
                state_root: restored.state_root,
            });
    }
    Ok(())
}

/// Imports a signed lifecycle checkpoint, restoring every validator into
/// `target_root/<node_id>`. The import report is written even on failure.
pub fn import_lifecycle_checkpoint(
    options: LifecycleCheckpointImportOptions,
) -> io::Result<LifecycleCheckpointImportReport> {
    let mut report = LifecycleCheckpointImportReport {
        schema: LIFECYCLE_CHECKPOINT_IMPORT_REPORT_SCHEMA.to_string(),
        started_unix: crate::consensus_artifacts::unix_now(),
        finished_unix: 0,
        checkpoint_dir: options.checkpoint_dir.display().to_string(),
        checkpoint_manifest_sha256: None,
        checks: Vec::new(),
        validators: Vec::new(),
        first_failure: None,
        ok: false,
    };
    let outcome = {
        let mut context = ImportContext {
            options: &options,
            report: &mut report,
        };
        import_lifecycle_checkpoint_inner(&mut context)
    };
    report.ok = outcome.is_ok();
    if let Err(error) = &outcome {
        report.first_failure = Some(error.to_string());
    }
    report.finished_unix = crate::consensus_artifacts::unix_now();
    if let Some(parent) = options.report_file.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&report)
        .map_err(|error| checkpoint_error(error.to_string()))?;
    atomic_write(&options.report_file, format!("{json}\n"))?;
    outcome?;
    Ok(report)
}
