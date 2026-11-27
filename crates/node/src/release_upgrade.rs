use std::collections::{BTreeMap, BTreeSet};
use std::ffi::CString;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::{Duration, Instant};

use postfiat_crypto_provider::{
    bytes_to_hex, hex_to_bytes, ml_dsa_65_sign_with_context, ml_dsa_65_validate_public_key,
    ml_dsa_65_verify_with_context, ML_DSA_65_ALGORITHM, ML_DSA_65_PUBLIC_KEY_BYTES,
    ML_DSA_65_SIGNATURE_BYTES,
};
use postfiat_rpc_sdk::{RpcRequest, RpcResponse};
use serde::{Deserialize, Serialize};
use sha2::{Digest as Sha2Digest, Sha256};

use crate::unix_now;
use crate::validator_uptime::{canonical_json_bytes, is_lowercase_hex_len};

pub const RELEASE_KEYSET_SCHEMA: &str = "postfiat.release_keyset.v1";
pub const RELEASE_MANIFEST_SCHEMA: &str = "postfiat.release_manifest.v3";
pub const RELEASE_MANIFEST_SIGNING_CONTEXT: &str = "postfiat-l1-v2/release-manifest/v3";
pub const UPGRADE_REPORT_SCHEMA: &str = "postfiat-node-upgrade-report-v1";
pub const UPGRADE_BACKUP_MANIFEST_SCHEMA: &str = "postfiat-node-upgrade-backup-v1";
pub const DEFAULT_RELEASE_CHANNEL_URL: &str =
    "https://releases.postfiat.org/latest/release-manifest.json";
pub const DEFAULT_RELEASE_KEYSET_FILE: &str = "/etc/postfiat/release-keyset.json";
pub const DEFAULT_UPGRADE_DATA_DIR: &str = "/var/lib/postfiat/validator-0";
pub const DEFAULT_SIGNER_GUARD_DIR: &str = "/var/lib/postfiat/signer-guard";
pub const DEFAULT_UPGRADE_BINARY_PATH: &str = "/usr/local/bin/postfiat-node";
pub const DEFAULT_UPGRADE_BACKUP_DIR: &str = "/var/lib/postfiat/backups";
pub const DEFAULT_UPGRADE_SERVICE_UNITS: &str =
    "postfiat-validator-0.service,postfiat-validator-0-rpc.service";
pub const UPGRADE_HEADROOM_BYTES: u64 = 536_870_912; // 512 MiB
pub const INSTALLED_RELEASE_SCHEMA: &str = "postfiat-node-installed-release-v1";
pub const INSTALLED_RELEASE_FILE: &str = "installed-release.json";
pub const UPGRADE_JOURNAL_SCHEMA: &str = "postfiat-node-upgrade-journal-v1";
pub const UPGRADE_JOURNAL_FILE: &str = "upgrade-journal.json";
pub const RELEASE_KEYSET_ROTATION_SCHEMA: &str = "postfiat.release_keyset_rotation.v1";
pub const RELEASE_KEYSET_ROTATION_CONTEXT: &str = "postfiat-l1-v2/release-keyset-rotation/v1";
/// Entries of the data directory that are never backed up and never rolled
/// back: the validator's own vote record, its key file, and runtime state.
/// A vote record is only ever carried forward.
pub const DEFAULT_BACKUP_EXCLUDES: &[&str] = &[
    crate::CONSENSUS_V2_SAFETY_DIR,
    crate::VALIDATOR_KEYS_FILE,
    "runtime",
    "readiness",
];
pub const MAX_RELEASE_MANIFEST_BYTES: usize = 1_048_576; // 1 MiB

// Frozen Contract C2 §3.3 exit codes.
pub const UPGRADE_EXIT_OK: i32 = 0;
pub const UPGRADE_EXIT_INVALID_ARGS: i32 = 2;
pub const UPGRADE_EXIT_SECURITY_REFUSAL: i32 = 3;
pub const UPGRADE_EXIT_PREFLIGHT_FAILED: i32 = 4;
pub const UPGRADE_EXIT_ROLLED_BACK: i32 = 5;
pub const UPGRADE_EXIT_ROLLBACK_FAILED: i32 = 6;
pub const UPGRADE_EXIT_AVAILABLE: i32 = 10;
pub const UPGRADE_EXIT_ACTIVATION_WARNING: i32 = 11;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseKeysetSignerV1 {
    pub signer_id: String,
    pub role: String,
    pub publisher: String,
    pub public_key_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseKeysetV1 {
    pub schema: String,
    pub keyset_id: String,
    pub threshold: usize,
    pub total_signers: usize,
    pub algorithm_id: String,
    pub signing_context: String,
    pub signers: Vec<ReleaseKeysetSignerV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseRolloutV3 {
    pub mode: String,
    pub activation_height: Option<u64>,
    pub advance_warning_blocks: Option<u64>,
    pub halt_if_missed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseCheckpointV3 {
    pub height: u64,
    pub block_hash: String,
    pub state_root: String,
    pub certificate_id: String,
    pub committee_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseBinaryArtifactV3 {
    pub binary_url: String,
    pub binary_sha256: String,
    pub binary_size_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseArtifactsV3 {
    #[serde(rename = "linux-x86_64")]
    pub linux_x86_64: ReleaseBinaryArtifactV3,
    #[serde(rename = "linux-aarch64")]
    pub linux_aarch64: ReleaseBinaryArtifactV3,
    pub systemd_bundle_sha256: String,
    pub docker_profiles_sha256: String,
    pub sbom_sha256: String,
    pub checksums_sha256: String,
    pub swap_circuit_metadata_sha256: String,
    pub private_egress_circuit_metadata_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseSigningPolicyV3 {
    pub keyset_id: String,
    pub threshold: usize,
    pub total_signers: usize,
    pub algorithm_id: String,
    pub signing_context: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseSignatureV3 {
    pub signer_id: String,
    pub publisher: String,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signed_at_unix: u64,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseManifestV3 {
    pub schema: String,
    pub release_id: String,
    /// Grows by at least 1 with every release of a chain; orders releases.
    pub release_sequence: u64,
    pub created_unix: u64,
    pub valid_from_unix: u64,
    pub valid_until_unix: u64,
    pub chain_id: String,
    pub genesis_hash: String,
    pub git_revision: String,
    pub build_profile: String,
    pub build_features: Vec<String>,
    pub protocol_version: u32,
    pub min_compatible_protocol_version: u32,
    pub rpc_schema: String,
    pub rollout: ReleaseRolloutV3,
    pub checkpoint: ReleaseCheckpointV3,
    pub artifacts: ReleaseArtifactsV3,
    pub signing_policy: ReleaseSigningPolicyV3,
    pub signatures: Vec<ReleaseSignatureV3>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReleaseManifestVerification {
    pub release_id: String,
    pub release_sequence: u64,
    pub git_revision: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub rollout_mode: String,
    pub activation_height: Option<u64>,
    pub advance_warning_blocks: Option<u64>,
    pub halt_if_missed: bool,
    pub threshold: usize,
    pub total_signers: usize,
    pub verified_signers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeBackupFileEntry {
    pub relative_path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeBackupManifestV1 {
    pub schema: String,
    pub release_id: String,
    pub created_unix: u64,
    pub source_data_dir: String,
    pub source_binary_path: String,
    pub signer_guard_dir_excluded: String,
    pub excluded_paths: Vec<String>,
    pub prior_binary_sha256: String,
    pub prior_binary_size_bytes: u64,
    pub data_files: Vec<UpgradeBackupFileEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpgradeReportV1 {
    pub schema: String,
    pub check_only: bool,
    pub status: String,
    pub exit_code: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub release_sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target_git_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_release_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_release_sequence: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed_git_revision: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub architecture: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rollout_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub activation_height: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks_until_activation: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub halt_if_missed: Option<bool>,
    pub verified_signers: Vec<String>,
    pub signer_guard_isolated: bool,
    pub data_dir_bytes: u64,
    pub candidate_binary_bytes: u64,
    pub required_free_bytes: u64,
    pub available_free_bytes: u64,
    pub disk_headroom_ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub backup_path: Option<String>,
    pub rolled_back: bool,
    /// Units that are not running at exit. Empty unless the exit code is 6.
    pub units_stopped: Vec<String>,
}

/// What runs now, written after a successful upgrade.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InstalledReleaseV1 {
    pub schema: String,
    pub release_id: String,
    pub release_sequence: u64,
    pub git_revision: String,
    pub installed_unix: u64,
}

/// Written before the services are stopped and removed when the upgrade has
/// ended one way or the other. A journal found at start means an earlier run
/// was interrupted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UpgradeJournalV1 {
    pub schema: String,
    pub release_id: String,
    pub release_sequence: u64,
    pub started_unix: u64,
    /// `stopping`, `backed_up`, `swapped` or `started`.
    pub step: String,
    pub data_dir: String,
    pub binary_path: String,
    pub signer_guard_dir: String,
    /// Set from `backed_up` on.
    pub backup_root: Option<String>,
    pub service_units: Vec<String>,
    pub exclude_paths: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseKeysetRotationSignatureV1 {
    pub signer_id: String,
    pub algorithm_id: String,
    pub signature_hex: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseKeysetRotationV1 {
    pub schema: String,
    pub prior_keyset_id: String,
    pub new_keyset: ReleaseKeysetV1,
    pub signatures: Vec<ReleaseKeysetRotationSignatureV1>,
}

#[derive(Debug, Clone)]
pub struct UpgradeOptions {
    pub check_only: bool,
    pub manifest_file: Option<PathBuf>,
    pub channel_url: Option<String>,
    pub binary_file: Option<PathBuf>,
    pub keyset_file: PathBuf,
    pub data_dir: PathBuf,
    pub signer_guard_dir: PathBuf,
    pub binary_path: PathBuf,
    pub backup_dir: PathBuf,
    pub service_units: Vec<String>,
    pub rpc_host: String,
    pub rpc_port: u16,
    pub health_timeout_ms: u64,
    pub now_unix: Option<u64>,
    pub architecture: Option<String>,
    pub yes: bool,
    pub json: bool,
    /// Further entries of `data_dir` to leave out of the backup and the rollback.
    pub exclude_paths: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UpgradeNodeStatusProbe {
    pub chain_id: String,
    pub genesis_hash: String,
    pub block_height: u64,
    pub git_revision: Option<String>,
    pub signer_mode: Option<String>,
    pub signer_double_sign_protection: Option<bool>,
}

pub trait UpgradeSystemHooks {
    fn stop_units(&self, units: &[String]) -> Result<(), String>;
    fn start_units(&self, units: &[String]) -> Result<(), String>;
    fn probe_node(
        &self,
        host: &str,
        port: u16,
        timeout_ms: u64,
    ) -> Result<UpgradeNodeStatusProbe, String>;
    fn free_disk_bytes(&self, path: &Path) -> Result<u64, String>;
    fn unit_is_active(&self, unit: &str) -> bool;
}

pub struct LiveUpgradeSystemHooks;

impl UpgradeSystemHooks for LiveUpgradeSystemHooks {
    fn stop_units(&self, units: &[String]) -> Result<(), String> {
        run_systemctl("stop", units)
    }

    fn start_units(&self, units: &[String]) -> Result<(), String> {
        run_systemctl("start", units)
    }

    fn probe_node(
        &self,
        host: &str,
        port: u16,
        timeout_ms: u64,
    ) -> Result<UpgradeNodeStatusProbe, String> {
        probe_local_node_rpc(host, port, timeout_ms)
    }

    fn free_disk_bytes(&self, path: &Path) -> Result<u64, String> {
        filesystem_free_bytes(path)
    }

    fn unit_is_active(&self, unit: &str) -> bool {
        Command::new("systemctl")
            .args(["is-active", "--quiet", unit])
            .status()
            .is_ok_and(|status| status.success())
    }
}

fn stopped_units(hooks: &dyn UpgradeSystemHooks, units: &[String]) -> Vec<String> {
    units
        .iter()
        .filter(|unit| !hooks.unit_is_active(unit))
        .cloned()
        .collect()
}

/// The node reports a short revision (8 characters today); the manifest
/// carries the full one. They match when the reported one is a prefix.
pub fn revision_matches(reported: Option<&str>, manifest_revision: &str) -> bool {
    reported.is_some_and(|revision| revision.len() >= 8 && manifest_revision.starts_with(revision))
}

fn write_json_atomically<T: Serialize>(path: &Path, value: &T) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| format!("`{}` has no parent directory", path.display()))?;
    fs::create_dir_all(parent)
        .map_err(|error| format!("create `{}` failed: {error}", parent.display()))?;
    let json = serde_json::to_string_pretty(value)
        .map_err(|error| format!("serialize `{}` failed: {error}", path.display()))?;
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    let write = || -> io::Result<()> {
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(&tmp)?;
        file.write_all(json.as_bytes())?;
        file.write_all(b"\n")?;
        file.sync_all()?;
        fs::rename(&tmp, path)?;
        fsync_dir(parent)
    };
    write().map_err(|error| format!("write `{}` failed: {error}", path.display()))
}

pub fn read_installed_release(backup_dir: &Path) -> Result<Option<InstalledReleaseV1>, String> {
    let path = backup_dir.join(INSTALLED_RELEASE_FILE);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read `{}` failed: {error}", path.display())),
    };
    let record: InstalledReleaseV1 = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "`{}` is not a valid installed-release record: {error}",
            path.display()
        )
    })?;
    if record.schema != INSTALLED_RELEASE_SCHEMA || record.release_sequence == 0 {
        return Err(format!(
            "`{}` is not a valid installed-release record",
            path.display()
        ));
    }
    Ok(Some(record))
}

pub fn read_upgrade_journal(backup_dir: &Path) -> Result<Option<UpgradeJournalV1>, String> {
    let path = backup_dir.join(UPGRADE_JOURNAL_FILE);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("read `{}` failed: {error}", path.display())),
    };
    let journal: UpgradeJournalV1 = serde_json::from_slice(&bytes).map_err(|error| {
        format!(
            "`{}` is not a valid upgrade journal: {error}",
            path.display()
        )
    })?;
    if journal.schema != UPGRADE_JOURNAL_SCHEMA {
        return Err(format!(
            "`{}` is not a valid upgrade journal",
            path.display()
        ));
    }
    Ok(Some(journal))
}

/// Relative entries of the data directory that the backup and the rollback skip.
pub fn backup_excludes(extra: &[String]) -> Result<Vec<PathBuf>, String> {
    let mut excludes = Vec::new();
    for raw in DEFAULT_BACKUP_EXCLUDES
        .iter()
        .map(|s| (*s).to_string())
        .chain(extra.iter().cloned())
    {
        let path = PathBuf::from(&raw);
        if raw.is_empty()
            || path.is_absolute()
            || path
                .components()
                .any(|c| !matches!(c, Component::Normal(_)))
        {
            return Err(format!(
                "--exclude-path `{raw}` must be a relative entry of the data directory without `.` or `..`"
            ));
        }
        if !excludes.contains(&path) {
            excludes.push(path);
        }
    }
    Ok(excludes)
}

fn is_excluded(relative: &Path, excludes: &[PathBuf]) -> bool {
    excludes.iter().any(|exclude| relative.starts_with(exclude))
}

/// Give `dest` the owner, group and mode recorded in `source`.
fn copy_owner_and_mode(source: &fs::Metadata, dest: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let current = fs::metadata(dest)?;
        if current.uid() != source.uid() || current.gid() != source.gid() {
            std::os::unix::fs::chown(dest, Some(source.uid()), Some(source.gid()))?;
        }
        fs::set_permissions(dest, fs::Permissions::from_mode(source.mode() & 0o7777))?;
    }
    #[cfg(not(unix))]
    {
        let _ = (source, dest);
    }
    Ok(())
}

pub fn release_keyset_rotation_signing_bytes(
    new_keyset: &ReleaseKeysetV1,
) -> Result<Vec<u8>, String> {
    let value = serde_json::to_value(new_keyset)
        .map_err(|error| format!("release keyset serialization failed: {error}"))?;
    canonical_json_bytes(&value)
}

pub fn sign_release_keyset_rotation(
    new_keyset: &ReleaseKeysetV1,
    signer_id: &str,
    private_key_hex: &str,
) -> Result<ReleaseKeysetRotationSignatureV1, String> {
    let sk = hex_to_bytes(private_key_hex)
        .map_err(|error| format!("invalid release signer private key hex: {error}"))?;
    let signature = ml_dsa_65_sign_with_context(
        &sk,
        &release_keyset_rotation_signing_bytes(new_keyset)?,
        RELEASE_KEYSET_ROTATION_CONTEXT.as_bytes(),
    )
    .map_err(|error| format!("release keyset rotation ML-DSA-65 sign failed: {error}"))?;
    Ok(ReleaseKeysetRotationSignatureV1 {
        signer_id: signer_id.to_string(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        signature_hex: bytes_to_hex(&signature),
    })
}

/// A new keyset counts only when `threshold` signers of the installed keyset
/// have signed it. Returns the signers that verified.
pub fn verify_release_keyset_rotation(
    rotation: &ReleaseKeysetRotationV1,
    installed: &ReleaseKeysetV1,
) -> Result<Vec<String>, String> {
    validate_release_keyset_v1(installed)?;
    validate_release_keyset_v1(&rotation.new_keyset)?;
    if rotation.schema != RELEASE_KEYSET_ROTATION_SCHEMA {
        return Err(format!(
            "unexpected keyset rotation schema `{}`",
            rotation.schema
        ));
    }
    if rotation.prior_keyset_id != installed.keyset_id {
        return Err(format!(
            "rotation replaces keyset `{}`, but `{}` is installed",
            rotation.prior_keyset_id, installed.keyset_id
        ));
    }
    if rotation.new_keyset.keyset_id == installed.keyset_id {
        return Err("the new keyset must have a new keyset_id".to_string());
    }
    let signed = release_keyset_rotation_signing_bytes(&rotation.new_keyset)?;
    let mut verified = BTreeSet::new();
    for signature in &rotation.signatures {
        let Some(pinned) = installed
            .signers
            .iter()
            .find(|s| s.signer_id == signature.signer_id)
        else {
            return Err(format!(
                "rotation signer `{}` is not in the installed keyset",
                signature.signer_id
            ));
        };
        if !verified.insert(signature.signer_id.clone()) {
            return Err(format!(
                "duplicate rotation signature for `{}`",
                signature.signer_id
            ));
        }
        let public_key = hex_to_bytes(&pinned.public_key_hex)
            .map_err(|error| format!("bad pinned key for `{}`: {error}", pinned.signer_id))?;
        let signature_bytes = hex_to_bytes(&signature.signature_hex)
            .map_err(|error| format!("bad signature hex for `{}`: {error}", signature.signer_id))?;
        if signature.algorithm_id != ML_DSA_65_ALGORITHM
            || !ml_dsa_65_verify_with_context(
                &public_key,
                &signed,
                &signature_bytes,
                RELEASE_KEYSET_ROTATION_CONTEXT.as_bytes(),
            )
        {
            return Err(format!(
                "rotation signature of `{}` does not verify",
                signature.signer_id
            ));
        }
    }
    if verified.len() < installed.threshold {
        return Err(format!(
            "{} rotation signatures verified; the installed keyset requires {}",
            verified.len(),
            installed.threshold
        ));
    }
    Ok(verified.into_iter().collect())
}

/// Verify a rotation against the installed keyset and replace the keyset file.
/// The replaced file is kept beside it as `<keyset-file>.<prior keyset_id>`.
pub fn apply_release_keyset_rotation(
    rotation_file: &Path,
    keyset_file: &Path,
) -> Result<String, String> {
    let installed: ReleaseKeysetV1 = serde_json::from_slice(
        &fs::read(keyset_file)
            .map_err(|error| format!("read `{}` failed: {error}", keyset_file.display()))?,
    )
    .map_err(|error| format!("invalid installed keyset: {error}"))?;
    let rotation: ReleaseKeysetRotationV1 = serde_json::from_slice(
        &fs::read(rotation_file)
            .map_err(|error| format!("read `{}` failed: {error}", rotation_file.display()))?,
    )
    .map_err(|error| format!("invalid keyset rotation: {error}"))?;
    verify_release_keyset_rotation(&rotation, &installed)?;
    let kept = PathBuf::from(format!("{}.{}", keyset_file.display(), installed.keyset_id));
    fs::copy(keyset_file, &kept)
        .map_err(|error| format!("keep `{}` failed: {error}", kept.display()))?;
    write_json_atomically(keyset_file, &rotation.new_keyset)?;
    Ok(rotation.new_keyset.keyset_id)
}

/// Pre-start check of a service: the manifest is signed by the pinned keyset
/// and the binary about to run is the one the manifest names. The validity
/// window bounds accepting an upgrade, not running an installed release, so
/// an expired manifest still passes here.
pub fn verify_installed_release(
    manifest_file: &Path,
    keyset_file: &Path,
    runtime_binary: &Path,
    architecture: &str,
) -> Result<ReleaseManifestVerification, String> {
    let keyset: ReleaseKeysetV1 = serde_json::from_slice(
        &fs::read(keyset_file)
            .map_err(|error| format!("read `{}` failed: {error}", keyset_file.display()))?,
    )
    .map_err(|error| format!("invalid release keyset: {error}"))?;
    let bytes = fs::read(manifest_file)
        .map_err(|error| format!("read `{}` failed: {error}", manifest_file.display()))?;
    if bytes.len() > MAX_RELEASE_MANIFEST_BYTES {
        return Err("release manifest exceeds 1 MiB limit".to_string());
    }
    let manifest: ReleaseManifestV3 = serde_json::from_slice(&bytes)
        .map_err(|error| format!("invalid release manifest: {error}"))?;
    let verification = verify_release_manifest_v3(&manifest, &keyset, manifest.valid_from_unix)?;
    let artifact = match architecture {
        "linux-x86_64" => &manifest.artifacts.linux_x86_64,
        "linux-aarch64" => &manifest.artifacts.linux_aarch64,
        other => return Err(format!("unsupported architecture `{other}`")),
    };
    let (digest, size) = sha256_file_and_size(runtime_binary)
        .map_err(|error| format!("read `{}` failed: {error}", runtime_binary.display()))?;
    if digest != artifact.binary_sha256 || size != artifact.binary_size_bytes {
        return Err(format!(
            "`{}` is not the {architecture} binary of release `{}`",
            runtime_binary.display(),
            manifest.release_id
        ));
    }
    Ok(verification)
}

fn run_systemctl(action: &str, units: &[String]) -> Result<(), String> {
    if units.is_empty() {
        return Ok(());
    }
    let output = Command::new("systemctl")
        .arg(action)
        .args(units)
        .output()
        .map_err(|error| format!("systemctl {action} failed to launch: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("systemctl {action} failed: {}", stderr.trim()));
    }
    Ok(())
}

pub fn host_release_architecture() -> Result<&'static str, String> {
    match std::env::consts::ARCH {
        "x86_64" => Ok("linux-x86_64"),
        "aarch64" => Ok("linux-aarch64"),
        other => Err(format!("unsupported host architecture `{other}`")),
    }
}

pub fn release_manifest_signing_bytes(manifest: &ReleaseManifestV3) -> Result<Vec<u8>, String> {
    let mut unsigned = manifest.clone();
    unsigned.signatures.clear();
    let value = serde_json::to_value(&unsigned)
        .map_err(|error| format!("release manifest serialization failed: {error}"))?;
    canonical_json_bytes(&value)
}

pub fn sign_release_manifest_v3(
    manifest: &ReleaseManifestV3,
    signer_id: &str,
    publisher: &str,
    public_key_hex: &str,
    private_key_hex: &str,
    signed_at_unix: u64,
) -> Result<ReleaseSignatureV3, String> {
    let signing_bytes = release_manifest_signing_bytes(manifest)?;
    let sk = hex_to_bytes(private_key_hex)
        .map_err(|error| format!("invalid release signer private key hex: {error}"))?;
    let sig = ml_dsa_65_sign_with_context(
        &sk,
        &signing_bytes,
        RELEASE_MANIFEST_SIGNING_CONTEXT.as_bytes(),
    )
    .map_err(|error| format!("release manifest ML-DSA-65 sign failed: {error}"))?;
    Ok(ReleaseSignatureV3 {
        signer_id: signer_id.to_string(),
        publisher: publisher.to_string(),
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: public_key_hex.to_string(),
        signed_at_unix,
        signature_hex: bytes_to_hex(&sig),
    })
}

pub fn validate_release_keyset_v1(keyset: &ReleaseKeysetV1) -> Result<(), String> {
    if keyset.schema != RELEASE_KEYSET_SCHEMA {
        return Err(format!(
            "unexpected release keyset schema `{}` (expected `{RELEASE_KEYSET_SCHEMA}`)",
            keyset.schema
        ));
    }
    if keyset.threshold < 2 || keyset.threshold > keyset.total_signers || keyset.total_signers < 3 {
        return Err("release keyset must enforce at least 2-of-3 signing policy".to_string());
    }
    if keyset.algorithm_id != ML_DSA_65_ALGORITHM {
        return Err(format!(
            "unsupported release keyset algorithm `{}`",
            keyset.algorithm_id
        ));
    }
    if keyset.signing_context != RELEASE_MANIFEST_SIGNING_CONTEXT {
        return Err(format!(
            "unexpected release keyset signing_context `{}`",
            keyset.signing_context
        ));
    }
    if keyset.signers.len() != keyset.total_signers {
        return Err(format!(
            "release keyset has {} signers, expected {}",
            keyset.signers.len(),
            keyset.total_signers
        ));
    }
    let mut ids = BTreeSet::new();
    let mut keys = BTreeSet::new();
    for signer in &keyset.signers {
        if signer.signer_id.is_empty() || !ids.insert(signer.signer_id.as_str()) {
            return Err(format!(
                "duplicate or empty signer_id `{}`",
                signer.signer_id
            ));
        }
        if !is_lowercase_hex_len(&signer.public_key_hex, ML_DSA_65_PUBLIC_KEY_BYTES * 2) {
            return Err(format!(
                "signer `{}` has invalid ML-DSA-65 public_key_hex",
                signer.signer_id
            ));
        }
        if !keys.insert(signer.public_key_hex.as_str()) {
            return Err(format!(
                "duplicate public_key_hex in keyset for `{}`",
                signer.signer_id
            ));
        }
        let pk_bytes = hex_to_bytes(&signer.public_key_hex)
            .map_err(|error| format!("signer `{}` bad pubkey hex: {error}", signer.signer_id))?;
        ml_dsa_65_validate_public_key(&pk_bytes).map_err(|error| {
            format!(
                "signer `{}` invalid ML-DSA-65 public key: {error}",
                signer.signer_id
            )
        })?;
    }
    Ok(())
}

pub fn verify_release_manifest_v3(
    manifest: &ReleaseManifestV3,
    keyset: &ReleaseKeysetV1,
    now_unix: u64,
) -> Result<ReleaseManifestVerification, String> {
    validate_release_keyset_v1(keyset)?;
    if manifest.schema != RELEASE_MANIFEST_SCHEMA {
        return Err(format!(
            "unexpected release manifest schema `{}` (expected `{RELEASE_MANIFEST_SCHEMA}`)",
            manifest.schema
        ));
    }
    if manifest.release_id.trim().is_empty() || manifest.chain_id.trim().is_empty() {
        return Err("release_id and chain_id must be non-empty".to_string());
    }
    if manifest.release_sequence == 0 {
        return Err("release manifest release_sequence must be at least 1".to_string());
    }
    if !is_lowercase_hex_len(&manifest.genesis_hash, 96) {
        return Err("release manifest genesis_hash must be 96 lowercase hex chars".to_string());
    }
    if !is_lowercase_hex_len(&manifest.git_revision, 40) {
        return Err("release manifest git_revision must be 40 lowercase hex chars".to_string());
    }
    if manifest.valid_from_unix > manifest.valid_until_unix {
        return Err("release manifest validity window is inverted".to_string());
    }
    if now_unix < manifest.valid_from_unix || now_unix > manifest.valid_until_unix {
        return Err(format!(
            "release manifest validity window [{}, {}] does not cover {now_unix}",
            manifest.valid_from_unix, manifest.valid_until_unix
        ));
    }
    match manifest.rollout.mode.as_str() {
        "rolling" => {
            if manifest.rollout.activation_height.is_some() || manifest.rollout.halt_if_missed {
                return Err(
                    "rolling rollout must have activation_height=null and halt_if_missed=false"
                        .to_string(),
                );
            }
        }
        "activation_height" => {
            if manifest.rollout.activation_height.unwrap_or(0) == 0
                || manifest.rollout.advance_warning_blocks.unwrap_or(0) == 0
                || !manifest.rollout.halt_if_missed
            {
                return Err(
                    "activation_height rollout requires activation_height>=1, advance_warning_blocks>=1, halt_if_missed=true"
                        .to_string(),
                );
            }
        }
        other => {
            return Err(format!("invalid rollout.mode `{other}`"));
        }
    }
    if manifest.checkpoint.height == 0
        || !is_lowercase_hex_len(&manifest.checkpoint.block_hash, 96)
        || !is_lowercase_hex_len(&manifest.checkpoint.state_root, 96)
        || !is_lowercase_hex_len(&manifest.checkpoint.certificate_id, 96)
    {
        return Err("release manifest checkpoint contains invalid hashes or height".to_string());
    }
    for (label, digest) in [
        (
            "linux-x86_64.binary_sha256",
            &manifest.artifacts.linux_x86_64.binary_sha256,
        ),
        (
            "linux-aarch64.binary_sha256",
            &manifest.artifacts.linux_aarch64.binary_sha256,
        ),
        (
            "systemd_bundle_sha256",
            &manifest.artifacts.systemd_bundle_sha256,
        ),
        (
            "docker_profiles_sha256",
            &manifest.artifacts.docker_profiles_sha256,
        ),
        ("sbom_sha256", &manifest.artifacts.sbom_sha256),
        ("checksums_sha256", &manifest.artifacts.checksums_sha256),
        (
            "swap_circuit_metadata_sha256",
            &manifest.artifacts.swap_circuit_metadata_sha256,
        ),
        (
            "private_egress_circuit_metadata_sha256",
            &manifest.artifacts.private_egress_circuit_metadata_sha256,
        ),
    ] {
        if !is_lowercase_hex_len(digest, 64) {
            return Err(format!(
                "artifact digest `{label}` must be 64 lowercase hex chars"
            ));
        }
    }
    if manifest.artifacts.linux_x86_64.binary_size_bytes == 0
        || manifest.artifacts.linux_aarch64.binary_size_bytes == 0
    {
        return Err("artifact binary_size_bytes must be positive".to_string());
    }
    if manifest.signing_policy.keyset_id != keyset.keyset_id
        || manifest.signing_policy.threshold != keyset.threshold
        || manifest.signing_policy.total_signers != keyset.total_signers
        || manifest.signing_policy.algorithm_id != keyset.algorithm_id
        || manifest.signing_policy.signing_context != keyset.signing_context
    {
        return Err("release manifest signing_policy does not match pinned keyset".to_string());
    }
    if manifest.signatures.len() < keyset.threshold {
        return Err(format!(
            "release manifest has {} signatures, requires at least {}",
            manifest.signatures.len(),
            keyset.threshold
        ));
    }

    let pinned_by_id: BTreeMap<&str, &ReleaseKeysetSignerV1> = keyset
        .signers
        .iter()
        .map(|s| (s.signer_id.as_str(), s))
        .collect();
    let canonical = release_manifest_signing_bytes(manifest)?;
    let mut verified_signers = BTreeSet::new();

    for sig in &manifest.signatures {
        let Some(pinned) = pinned_by_id.get(sig.signer_id.as_str()) else {
            return Err(format!(
                "manifest signature signer_id `{}` is not in pinned keyset",
                sig.signer_id
            ));
        };
        if !verified_signers.insert(sig.signer_id.clone()) {
            return Err(format!(
                "duplicate signature for signer_id `{}`",
                sig.signer_id
            ));
        }
        if sig.algorithm_id != ML_DSA_65_ALGORITHM
            || sig.publisher != pinned.publisher
            || sig.public_key_hex != pinned.public_key_hex
        {
            return Err(format!(
                "manifest signature metadata for `{}` does not match pinned keyset",
                sig.signer_id
            ));
        }
        if !is_lowercase_hex_len(&sig.signature_hex, ML_DSA_65_SIGNATURE_BYTES * 2) {
            return Err(format!(
                "manifest signature for `{}` has invalid ML-DSA-65 hex length",
                sig.signer_id
            ));
        }
        let pk_bytes = hex_to_bytes(&sig.public_key_hex)
            .map_err(|error| format!("bad pubkey hex for `{}`: {error}", sig.signer_id))?;
        let sig_bytes = hex_to_bytes(&sig.signature_hex)
            .map_err(|error| format!("bad sig hex for `{}`: {error}", sig.signer_id))?;
        if !ml_dsa_65_verify_with_context(
            &pk_bytes,
            &canonical,
            &sig_bytes,
            RELEASE_MANIFEST_SIGNING_CONTEXT.as_bytes(),
        ) {
            return Err(format!(
                "ML-DSA-65 signature verification failed for signer `{}`",
                sig.signer_id
            ));
        }
    }

    if verified_signers.len() < keyset.threshold {
        return Err(format!(
            "only {} valid release signatures verified; threshold is {}",
            verified_signers.len(),
            keyset.threshold
        ));
    }

    Ok(ReleaseManifestVerification {
        release_id: manifest.release_id.clone(),
        release_sequence: manifest.release_sequence,
        git_revision: manifest.git_revision.clone(),
        chain_id: manifest.chain_id.clone(),
        genesis_hash: manifest.genesis_hash.clone(),
        rollout_mode: manifest.rollout.mode.clone(),
        activation_height: manifest.rollout.activation_height,
        advance_warning_blocks: manifest.rollout.advance_warning_blocks,
        halt_if_missed: manifest.rollout.halt_if_missed,
        threshold: keyset.threshold,
        total_signers: keyset.total_signers,
        verified_signers: verified_signers.into_iter().collect(),
    })
}

/// Normalize a path even if its leaf components do not yet exist: canonicalize
/// the deepest existing ancestor, then lexically resolve remaining components.
pub fn resolve_normalized_path(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()?.join(path)
    };
    let mut existing = absolute.clone();
    let mut tail = Vec::new();
    while !existing.exists() {
        let Some(name) = existing.file_name() else {
            break;
        };
        tail.push(name.to_os_string());
        if !existing.pop() {
            break;
        }
    }
    let mut base = if existing.exists() {
        fs::canonicalize(&existing)?
    } else {
        existing
    };
    for part in tail.into_iter().rev() {
        base.push(part);
    }
    let mut normalized = PathBuf::new();
    for component in base.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            other => normalized.push(other.as_os_str()),
        }
    }
    Ok(normalized)
}

/// Contract C2 §3.2 Step 2 HARD INVARIANT: `--signer-guard-dir` MUST be
/// strictly disjoint from both `--data-dir` and `--backup-dir` (neither equal,
/// nested inside, nor an ancestor).
pub fn verify_signer_guard_isolation(
    signer_guard_dir: &Path,
    data_dir: &Path,
    backup_dir: &Path,
) -> Result<(), String> {
    let guard = resolve_normalized_path(signer_guard_dir)
        .map_err(|error| format!("cannot resolve signer-guard-dir: {error}"))?;
    let data = resolve_normalized_path(data_dir)
        .map_err(|error| format!("cannot resolve data-dir: {error}"))?;
    let backup = resolve_normalized_path(backup_dir)
        .map_err(|error| format!("cannot resolve backup-dir: {error}"))?;

    for (label, other) in [("data-dir", &data), ("backup-dir", &backup)] {
        if guard == *other || guard.starts_with(other) || other.starts_with(&guard) {
            return Err(format!(
                "guard_path_overlap: signer-guard-dir `{}` overlaps with {label} `{}`",
                guard.display(),
                other.display()
            ));
        }
    }
    Ok(())
}

pub fn directory_data_size_bytes(path: &Path) -> io::Result<u64> {
    if !path.exists() {
        return Ok(0);
    }
    let mut total = 0_u64;
    let mut stack = vec![path.to_path_buf()];
    while let Some(current) = stack.pop() {
        let meta = fs::symlink_metadata(&current)?;
        if meta.file_type().is_symlink() {
            continue;
        }
        if meta.is_file() {
            total = total.saturating_add(meta.len());
        } else if meta.is_dir() {
            for entry in fs::read_dir(&current)? {
                let entry = entry?;
                let child_meta = fs::symlink_metadata(entry.path())?;
                if child_meta.file_type().is_symlink() {
                    continue;
                }
                if child_meta.is_dir() {
                    stack.push(entry.path());
                } else if child_meta.is_file() {
                    total = total.saturating_add(child_meta.len());
                }
            }
        }
    }
    Ok(total)
}

#[cfg(unix)]
pub fn filesystem_free_bytes(path: &Path) -> Result<u64, String> {
    let mut target = path.to_path_buf();
    while !target.exists() {
        if !target.pop() {
            target = PathBuf::from("/");
            break;
        }
    }
    let c_path = CString::new(target.to_string_lossy().as_bytes())
        .map_err(|_| "path contains interior NUL byte".to_string())?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let rc = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if rc != 0 {
        return Err(format!(
            "statvfs `{}` failed: {}",
            target.display(),
            io::Error::last_os_error()
        ));
    }
    #[allow(clippy::unnecessary_cast)]
    let avail = stat.f_bavail as u64;
    #[allow(clippy::unnecessary_cast)]
    let frsize = stat.f_frsize as u64;
    Ok(avail.saturating_mul(frsize))
}

#[cfg(not(unix))]
pub fn filesystem_free_bytes(_path: &Path) -> Result<u64, String> {
    Err("filesystem_free_bytes is only supported on Unix platforms".to_string())
}

pub fn sha256_file_and_size(path: &Path) -> io::Result<(String, u64)> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0_u8; 64 * 1024];
    let mut size = 0_u64;
    loop {
        let count = file.read(&mut buf)?;
        if count == 0 {
            break;
        }
        hasher.update(&buf[..count]);
        size = size.saturating_add(count as u64);
    }
    Ok((bytes_to_hex(&hasher.finalize()), size))
}

pub fn fetch_https_bounded(url: &str, max_bytes: usize) -> Result<Vec<u8>, String> {
    if !url.starts_with("https://") {
        return Err(format!(
            "channel URL `{url}` must use https:// (plaintext http is forbidden)"
        ));
    }
    let output = Command::new("curl")
        .args([
            "--proto",
            "=https",
            "--tlsv1.2",
            "--fail",
            "--silent",
            "--show-error",
            "--max-time",
            "30",
            "--max-filesize",
            &max_bytes.to_string(),
            url,
        ])
        .output()
        .map_err(|error| format!("failed to launch curl for `{url}`: {error}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("HTTPS fetch `{url}` failed: {}", stderr.trim()));
    }
    if output.stdout.len() > max_bytes {
        return Err(format!(
            "HTTPS response from `{url}` exceeded {max_bytes}-byte limit"
        ));
    }
    Ok(output.stdout)
}

fn send_single_rpc(
    host: &str,
    port: u16,
    method: &str,
    timeout_ms: u64,
) -> Result<serde_json::Value, String> {
    let addr_text = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]:{port}")
    } else {
        format!("{host}:{port}")
    };
    let address = addr_text
        .to_socket_addrs()
        .map_err(|error| format!("RPC address resolution failed: {error}"))?
        .next()
        .ok_or_else(|| "RPC address resolved to no socket address".to_string())?;
    let timeout = Duration::from_millis(timeout_ms.max(1));
    let started = Instant::now();
    let mut stream = TcpStream::connect_timeout(&address, timeout)
        .map_err(|error| format!("RPC connect `{addr_text}` failed: {error}"))?;
    let req_id = format!("upgrade-{method}");
    let request = RpcRequest::empty(&req_id, method);
    let mut wire =
        serde_json::to_vec(&request).map_err(|error| format!("RPC encode failed: {error}"))?;
    wire.push(b'\n');
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| format!("RPC write timeout failed: {error}"))?;
    stream
        .write_all(&wire)
        .map_err(|error| format!("RPC write failed: {error}"))?;
    let _ = stream.shutdown(std::net::Shutdown::Write);

    let mut raw = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        let remaining = timeout.saturating_sub(started.elapsed());
        if remaining.is_zero() {
            return Err(format!("RPC `{method}` timed out after {timeout_ms}ms"));
        }
        stream
            .set_read_timeout(Some(remaining))
            .map_err(|error| format!("RPC read timeout failed: {error}"))?;
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(n) => {
                let nl = chunk[..n].iter().position(|b| *b == b'\n');
                raw.extend_from_slice(&chunk[..nl.unwrap_or(n)]);
                if raw.len() > 2 * 1024 * 1024 {
                    return Err(format!("RPC `{method}` response exceeded 2 MiB"));
                }
                if nl.is_some() {
                    break;
                }
            }
            Err(error) => return Err(format!("RPC `{method}` read failed: {error}")),
        }
    }
    let response: RpcResponse = serde_json::from_slice(&raw)
        .map_err(|error| format!("RPC `{method}` parse failed: {error}"))?;
    response
        .validate_protocol()
        .map_err(|error| format!("RPC `{method}` protocol error: {error}"))?;
    if response.id != req_id {
        return Err(format!("RPC `{method}` id mismatch"));
    }
    if !response.ok {
        let code = response
            .error
            .as_ref()
            .map_or("rpc_error", |e| e.code.as_str());
        return Err(format!("RPC `{method}` returned ok=false ({code})"));
    }
    response
        .result
        .ok_or_else(|| format!("RPC `{method}` missing result"))
}

pub fn probe_local_node_rpc(
    host: &str,
    port: u16,
    timeout_ms: u64,
) -> Result<UpgradeNodeStatusProbe, String> {
    let status = send_single_rpc(host, port, "status", timeout_ms)?;
    let chain_id = status
        .get("chain_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "status missing chain_id".to_string())?
        .to_string();
    let genesis_hash = status
        .get("genesis_hash")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| "status missing genesis_hash".to_string())?
        .to_string();
    let block_height = status
        .get("block_height")
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| "status missing block_height".to_string())?;
    let git_revision = status
        .get("build_git_revision")
        .or_else(|| status.get("git_revision"))
        .and_then(serde_json::Value::as_str)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    let (signer_mode, signer_double_sign_protection) =
        match send_single_rpc(host, port, "private_readiness", timeout_ms.min(5_000)) {
            Ok(readiness) => {
                let custody = readiness.get("custody");
                let mode = custody
                    .and_then(|c| c.get("validator_signer"))
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                let dsp = custody
                    .and_then(|c| c.get("signer_double_sign_protection"))
                    .and_then(serde_json::Value::as_bool);
                (mode, dsp)
            }
            Err(_) => (None, None),
        };

    Ok(UpgradeNodeStatusProbe {
        chain_id,
        genesis_hash,
        block_height,
        git_revision,
        signer_mode,
        signer_double_sign_protection,
    })
}

fn fsync_dir(dir: &Path) -> io::Result<()> {
    let handle = File::open(dir)?;
    handle.sync_all()
}

/// Atomic binary replacement: write `<dest>.staging.<pid>`, chmod 0755,
/// `fsync`, atomic `rename(2)`, and `fsync` parent directory.
pub fn atomic_install_binary(staged_source: &Path, dest_binary: &Path) -> io::Result<()> {
    let parent = dest_binary.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "destination binary path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;
    let file_name = dest_binary
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid binary filename"))?;
    let staging = parent.join(format!("{file_name}.staging.{}", std::process::id()));
    let mut src = File::open(staged_source)?;
    let mut dst = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&staging)?;
    io::copy(&mut src, &mut dst)?;
    dst.sync_all()?;
    drop(dst);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&staging)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&staging, perms)?;
    }
    fs::rename(&staging, dest_binary)?;
    fsync_dir(parent)?;
    Ok(())
}

/// Copy chain state only: regular files and subdirectories, without symlinks,
/// sockets, `signer_guard_dir` and every excluded entry. Every copy keeps the
/// owner, group and mode of its source. Returns the files with their digests.
fn copy_chain_state_tree(
    source_root: &Path,
    dest_root: &Path,
    canonical_guard_dir: &Path,
    excludes: &[PathBuf],
) -> io::Result<Vec<UpgradeBackupFileEntry>> {
    fs::create_dir_all(dest_root)?;
    copy_owner_and_mode(&fs::metadata(source_root)?, dest_root)?;
    let mut entries = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let src_dir = source_root.join(&rel);
        let dst_dir = dest_root.join(&rel);
        let mut children: Vec<_> = fs::read_dir(&src_dir)?.collect::<io::Result<_>>()?;
        children.sort_by_key(|e| e.file_name());
        for child in children {
            let child_src = child.path();
            let child_rel = rel.join(child.file_name());
            if is_excluded(&child_rel, excludes) {
                continue;
            }
            let normalized_child =
                resolve_normalized_path(&child_src).unwrap_or_else(|_| child_src.clone());
            if normalized_child == canonical_guard_dir
                || normalized_child.starts_with(canonical_guard_dir)
            {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "guard_path_overlap detected while copying chain state",
                ));
            }
            let meta = fs::symlink_metadata(&child_src)?;
            if meta.file_type().is_symlink() {
                continue;
            }
            let child_dst = dst_dir.join(child.file_name());
            if meta.is_dir() {
                fs::create_dir_all(&child_dst)?;
                copy_owner_and_mode(&meta, &child_dst)?;
                stack.push(child_rel);
            } else if meta.is_file() {
                let mut reader = File::open(&child_src)?;
                let mut options = OpenOptions::new();
                options.create(true).truncate(true).write(true);
                #[cfg(unix)]
                {
                    // Never wider than owner-only while the bytes are written.
                    use std::os::unix::fs::OpenOptionsExt;
                    options.mode(0o600);
                }
                let mut writer = options.open(&child_dst)?;
                let mut hasher = Sha256::new();
                let mut buf = [0_u8; 64 * 1024];
                let mut size = 0_u64;
                loop {
                    let n = reader.read(&mut buf)?;
                    if n == 0 {
                        break;
                    }
                    writer.write_all(&buf[..n])?;
                    hasher.update(&buf[..n]);
                    size = size.saturating_add(n as u64);
                }
                writer.sync_all()?;
                drop(writer);
                copy_owner_and_mode(&meta, &child_dst)?;
                entries.push(UpgradeBackupFileEntry {
                    relative_path: child_rel.to_string_lossy().replace('\\', "/"),
                    size_bytes: size,
                    sha256: bytes_to_hex(&hasher.finalize()),
                });
            }
            // Unix domain sockets and FIFOs are deliberately skipped.
        }
    }
    entries.sort_by(|a, b| a.relative_path.cmp(&b.relative_path));
    Ok(entries)
}

fn backup_snapshot_dir(backup_dir: &Path, release_id: &str, now_unix: u64) -> PathBuf {
    let safe_release: String = release_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect();
    backup_dir.join(format!("{safe_release}-pre-{now_unix}"))
}

fn backup_paths(snapshot_dir: &Path) -> (PathBuf, PathBuf) {
    (
        snapshot_dir.join("postfiat-node.prev"),
        snapshot_dir.join("chain-data"),
    )
}

fn create_pre_upgrade_backup(
    release_id: &str,
    now_unix: u64,
    data_dir: &Path,
    binary_path: &Path,
    backup_dir: &Path,
    signer_guard_dir: &Path,
    excludes: &[PathBuf],
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let canonical_guard = resolve_normalized_path(signer_guard_dir)
        .map_err(|error| format!("resolve signer-guard-dir failed: {error}"))?;
    let snapshot_dir = backup_snapshot_dir(backup_dir, release_id, now_unix);
    if snapshot_dir.exists() {
        return Err(format!(
            "backup directory `{}` already exists",
            snapshot_dir.display()
        ));
    }
    let (backup_binary, backup_data_dir) = backup_paths(&snapshot_dir);
    fs::create_dir_all(&snapshot_dir)
        .map_err(|error| format!("create backup dir failed: {error}"))?;
    // The backup belongs to the owner of the data directory and to nobody else.
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        let owner =
            fs::metadata(data_dir).map_err(|error| format!("stat data-dir failed: {error}"))?;
        let snapshot = fs::metadata(&snapshot_dir)
            .map_err(|error| format!("stat backup dir failed: {error}"))?;
        if snapshot.uid() != owner.uid() || snapshot.gid() != owner.gid() {
            std::os::unix::fs::chown(&snapshot_dir, Some(owner.uid()), Some(owner.gid()))
                .map_err(|error| format!("chown backup dir failed: {error}"))?;
        }
        fs::set_permissions(&snapshot_dir, fs::Permissions::from_mode(0o700))
            .map_err(|error| format!("chmod backup dir failed: {error}"))?;
    }

    let data_files = copy_chain_state_tree(data_dir, &backup_data_dir, &canonical_guard, excludes)
        .map_err(|error| format!("chain-state backup failed: {error}"))?;

    let (prior_binary_sha256, prior_binary_size_bytes) = if binary_path.is_file() {
        fs::copy(binary_path, &backup_binary)
            .map_err(|error| format!("prior binary backup failed: {error}"))?;
        let file = File::open(&backup_binary)
            .map_err(|error| format!("open backup binary failed: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("fsync backup binary failed: {error}"))?;
        sha256_file_and_size(&backup_binary)
            .map_err(|error| format!("hash backup binary failed: {error}"))?
    } else {
        return Err(format!(
            "installed binary `{}` does not exist",
            binary_path.display()
        ));
    };

    let manifest = UpgradeBackupManifestV1 {
        schema: UPGRADE_BACKUP_MANIFEST_SCHEMA.to_string(),
        release_id: release_id.to_string(),
        created_unix: now_unix,
        source_data_dir: data_dir.display().to_string(),
        source_binary_path: binary_path.display().to_string(),
        signer_guard_dir_excluded: canonical_guard.display().to_string(),
        excluded_paths: excludes
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
        prior_binary_sha256,
        prior_binary_size_bytes,
        data_files,
    };
    // Written last: a snapshot without this file is not a complete backup.
    write_json_atomically(&snapshot_dir.join("backup-manifest.json"), &manifest)?;
    Ok((snapshot_dir, backup_binary, backup_data_dir))
}

/// Restore the pre-upgrade binary and chain state. The excluded entries (the
/// vote record, the key file, runtime state) are carried over from the
/// directory the failed binary ran in: the old binary starts with every vote
/// the new one cast on record. `signer_guard_dir` is never touched.
fn restore_pre_upgrade_backup(
    backup_binary: &Path,
    backup_data_dir: &Path,
    dest_binary: &Path,
    dest_data_dir: &Path,
    signer_guard_dir: &Path,
    excludes: &[PathBuf],
) -> Result<(), String> {
    verify_signer_guard_isolation(signer_guard_dir, dest_data_dir, backup_data_dir)?;
    let canonical_guard = resolve_normalized_path(signer_guard_dir)
        .map_err(|error| format!("resolve signer-guard-dir failed: {error}"))?;

    // 1. Atomically restore prior binary.
    atomic_install_binary(backup_binary, dest_binary)
        .map_err(|error| format!("rollback binary restore failed: {error}"))?;

    // 2. Stage the restored chain state beside the data directory, then swap.
    let parent = dest_data_dir
        .parent()
        .ok_or_else(|| "data-dir has no parent directory".to_string())?;
    let name = dest_data_dir
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "invalid data-dir name".to_string())?;
    let restore_stage = parent.join(format!(".{name}.rollback-stage"));
    let failed_stash = parent.join(format!(".{name}.failed-upgrade"));
    // A stash beside an existing data directory means an earlier rollback
    // already swapped the chain state in and stopped while carrying entries
    // over. The swap is not repeated: the data directory may already hold the
    // vote record, and it must not be deleted.
    let swap_already_done = failed_stash.exists() && dest_data_dir.exists();
    if !swap_already_done {
        if failed_stash.exists() {
            fs::rename(&failed_stash, dest_data_dir)
                .map_err(|error| format!("rollback could not put the data-dir back: {error}"))?;
        }
        if restore_stage.exists() {
            fs::remove_dir_all(&restore_stage)
                .map_err(|error| format!("rollback could not clear an old stage: {error}"))?;
        }
        copy_chain_state_tree(backup_data_dir, &restore_stage, &canonical_guard, &[])
            .map_err(|error| format!("rollback chain-state stage failed: {error}"))?;
        if dest_data_dir.exists() {
            fs::rename(dest_data_dir, &failed_stash)
                .map_err(|error| format!("rollback move failed data-dir failed: {error}"))?;
        }
        if let Err(error) = fs::rename(&restore_stage, dest_data_dir) {
            if failed_stash.exists() && !dest_data_dir.exists() {
                let _ = fs::rename(&failed_stash, dest_data_dir);
            }
            return Err(format!("rollback swap restored data-dir failed: {error}"));
        }
    }
    // 3. Carry the excluded entries over. Until every one has moved, the stash stays.
    for exclude in excludes {
        let from = failed_stash.join(exclude);
        if fs::symlink_metadata(&from).is_err() {
            continue;
        }
        let to = dest_data_dir.join(exclude);
        if let Some(to_parent) = to.parent() {
            fs::create_dir_all(to_parent).map_err(|error| {
                format!("rollback could not prepare `{}`: {error}", to.display())
            })?;
        }
        fs::rename(&from, &to).map_err(|error| {
            format!(
                "rollback could not carry `{}` over; it is kept in `{}`: {error}",
                exclude.display(),
                failed_stash.display()
            )
        })?;
    }
    fsync_dir(dest_data_dir).map_err(|error| format!("fsync data-dir failed: {error}"))?;
    fsync_dir(parent).map_err(|error| format!("fsync data-dir parent failed: {error}"))?;
    let _ = fs::remove_dir_all(&failed_stash);
    Ok(())
}

fn base_report(check_only: bool) -> UpgradeReportV1 {
    UpgradeReportV1 {
        schema: UPGRADE_REPORT_SCHEMA.to_string(),
        check_only,
        status: "refused".to_string(),
        exit_code: UPGRADE_EXIT_INVALID_ARGS,
        error_code: None,
        message: None,
        release_id: None,
        release_sequence: None,
        target_git_revision: None,
        installed_release_id: None,
        installed_release_sequence: None,
        installed_git_revision: None,
        architecture: None,
        rollout_mode: None,
        activation_height: None,
        blocks_until_activation: None,
        halt_if_missed: None,
        verified_signers: Vec::new(),
        signer_guard_isolated: false,
        data_dir_bytes: 0,
        candidate_binary_bytes: 0,
        required_free_bytes: 0,
        available_free_bytes: 0,
        disk_headroom_ok: false,
        backup_path: None,
        rolled_back: false,
        units_stopped: Vec::new(),
    }
}

pub fn execute_upgrade(
    options: &UpgradeOptions,
    hooks: &dyn UpgradeSystemHooks,
) -> UpgradeReportV1 {
    let mut report = base_report(options.check_only);

    // Validate mutually exclusive manifest sources.
    if options.manifest_file.is_some() && options.channel_url.is_some() {
        report.exit_code = UPGRADE_EXIT_INVALID_ARGS;
        report.error_code = Some("invalid_args".to_string());
        report.message =
            Some("--manifest-file and --channel-url are mutually exclusive".to_string());
        return report;
    }

    let excludes = match backup_excludes(&options.exclude_paths) {
        Ok(excludes) => excludes,
        Err(error) => {
            report.exit_code = UPGRADE_EXIT_INVALID_ARGS;
            report.error_code = Some("invalid_args".to_string());
            report.message = Some(error);
            return report;
        }
    };

    // An earlier run that did not finish comes first. Nothing else is done
    // until its rollback is complete.
    match read_upgrade_journal(&options.backup_dir) {
        Ok(None) => {}
        Ok(Some(journal)) if options.check_only => {
            report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
            report.error_code = Some("interrupted_upgrade".to_string());
            report.release_id = Some(journal.release_id.clone());
            report.message = Some(format!(
                "an upgrade to `{}` was interrupted at step `{}`; run `postfiat-node upgrade` to roll it back",
                journal.release_id, journal.step
            ));
            return report;
        }
        Ok(Some(journal)) => return recover_interrupted_upgrade(&journal, options, hooks, report),
        Err(error) => {
            report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
            report.error_code = Some("upgrade_journal_invalid".to_string());
            report.message = Some(error);
            return report;
        }
    }

    let now_unix = options.now_unix.unwrap_or_else(unix_now);
    let arch = match options.architecture.as_deref() {
        Some(a @ ("linux-x86_64" | "linux-aarch64")) => a.to_string(),
        Some(other) => {
            report.exit_code = UPGRADE_EXIT_INVALID_ARGS;
            report.error_code = Some("unsupported_architecture".to_string());
            report.message = Some(format!("unsupported --arch `{other}`"));
            return report;
        }
        None => match host_release_architecture() {
            Ok(a) => a.to_string(),
            Err(error) => {
                report.exit_code = UPGRADE_EXIT_INVALID_ARGS;
                report.error_code = Some("unsupported_architecture".to_string());
                report.message = Some(error);
                return report;
            }
        },
    };
    report.architecture = Some(arch.clone());

    // Step 1: Load and verify pinned 2-of-3 release keyset & manifest.
    let keyset_raw = match fs::read_to_string(&options.keyset_file) {
        Ok(raw) => raw,
        Err(error) => {
            report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
            report.error_code = Some("release_keyset_unreadable".to_string());
            report.message = Some(format!(
                "failed to read release keyset `{}`: {error}",
                options.keyset_file.display()
            ));
            return report;
        }
    };
    let keyset: ReleaseKeysetV1 = match serde_json::from_str(&keyset_raw) {
        Ok(k) => k,
        Err(error) => {
            report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
            report.error_code = Some("release_keyset_invalid".to_string());
            report.message = Some(format!("invalid release keyset JSON: {error}"));
            return report;
        }
    };

    let manifest_bytes = if let Some(path) = &options.manifest_file {
        match fs::read(path) {
            Ok(bytes) if bytes.len() <= MAX_RELEASE_MANIFEST_BYTES => bytes,
            Ok(_) => {
                report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
                report.error_code = Some("manifest_too_large".to_string());
                report.message = Some("release manifest exceeds 1 MiB limit".to_string());
                return report;
            }
            Err(error) => {
                report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
                report.error_code = Some("manifest_unreadable".to_string());
                report.message = Some(format!(
                    "failed to read manifest `{}`: {error}",
                    path.display()
                ));
                return report;
            }
        }
    } else {
        let url = options
            .channel_url
            .as_deref()
            .unwrap_or(DEFAULT_RELEASE_CHANNEL_URL);
        match fetch_https_bounded(url, MAX_RELEASE_MANIFEST_BYTES) {
            Ok(bytes) => bytes,
            Err(error) => {
                report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
                report.error_code = Some("channel_fetch_failed".to_string());
                report.message = Some(error);
                return report;
            }
        }
    };

    let manifest: ReleaseManifestV3 = match serde_json::from_slice(&manifest_bytes) {
        Ok(m) => m,
        Err(error) => {
            report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
            report.error_code = Some("manifest_invalid".to_string());
            report.message = Some(format!("invalid release manifest JSON: {error}"));
            return report;
        }
    };

    let verification = match verify_release_manifest_v3(&manifest, &keyset, now_unix) {
        Ok(v) => v,
        Err(error) => {
            report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
            report.error_code = Some("manifest_signature_invalid".to_string());
            report.message = Some(error);
            return report;
        }
    };

    report.release_id = Some(verification.release_id.clone());
    report.release_sequence = Some(verification.release_sequence);
    report.target_git_revision = Some(verification.git_revision.clone());

    // Releases have an order. An older one is refused; there is no flag to skip this.
    let installed = match read_installed_release(&options.backup_dir) {
        Ok(installed) => installed,
        Err(error) => {
            report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
            report.error_code = Some("installed_release_record_invalid".to_string());
            report.message = Some(error);
            return report;
        }
    };
    if let Some(installed) = &installed {
        report.installed_release_id = Some(installed.release_id.clone());
        report.installed_release_sequence = Some(installed.release_sequence);
        if manifest.release_sequence < installed.release_sequence {
            report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
            report.error_code = Some("release_downgrade".to_string());
            report.message = Some(format!(
                "release `{}` (sequence {}) is older than the installed `{}` (sequence {})",
                manifest.release_id,
                manifest.release_sequence,
                installed.release_id,
                installed.release_sequence
            ));
            return report;
        }
        if manifest.release_sequence == installed.release_sequence {
            if manifest.release_id != installed.release_id {
                report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
                report.error_code = Some("release_sequence_conflict".to_string());
                report.message = Some(format!(
                    "release `{}` has the sequence {} of the installed `{}`",
                    manifest.release_id, manifest.release_sequence, installed.release_id
                ));
                return report;
            }
            report.status = "up_to_date".to_string();
            report.exit_code = UPGRADE_EXIT_OK;
            report.message = Some(format!("already at release `{}`", manifest.release_id));
            return report;
        }
    }
    report.rollout_mode = Some(verification.rollout_mode.clone());
    report.activation_height = verification.activation_height;
    report.halt_if_missed = Some(verification.halt_if_missed);
    report.verified_signers = verification.verified_signers.clone();

    // Probe local node (chain_id, genesis_hash, current height & git_revision).
    let pre_probe = {
        match hooks.probe_node(&options.rpc_host, options.rpc_port, 5_000) {
            Ok(probe) => {
                if probe.chain_id != manifest.chain_id
                    || probe.genesis_hash != manifest.genesis_hash
                {
                    report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
                    report.error_code = Some("chain_identity_mismatch".to_string());
                    report.message = Some(format!(
                        "manifest chain `{}`/{} does not match local node `{}`/{}",
                        manifest.chain_id,
                        manifest.genesis_hash,
                        probe.chain_id,
                        probe.genesis_hash
                    ));
                    return report;
                }
                report.installed_git_revision = probe.git_revision.clone();
                if let Some(activation) = manifest.rollout.activation_height {
                    report.blocks_until_activation =
                        Some(activation.saturating_sub(probe.block_height));
                }
                Some(probe)
            }
            Err(error) => {
                if !options.check_only {
                    report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
                    report.error_code = Some("pre_upgrade_rpc_unhealthy".to_string());
                    report.message = Some(format!(
                        "pre-upgrade local RPC health probe failed: {error}"
                    ));
                    return report;
                }
                None
            }
        }
    };

    // Select architecture artifact and verify candidate binary when provided or applying.
    let artifact = if arch == "linux-aarch64" {
        &manifest.artifacts.linux_aarch64
    } else {
        &manifest.artifacts.linux_x86_64
    };
    report.candidate_binary_bytes = artifact.binary_size_bytes;

    // Step 2: Double-sign guard isolation check (HARD INVARIANT).
    if let Err(error) = verify_signer_guard_isolation(
        &options.signer_guard_dir,
        &options.data_dir,
        &options.backup_dir,
    ) {
        report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
        report.error_code = Some("guard_path_overlap".to_string());
        report.message = Some(error);
        return report;
    }
    report.signer_guard_isolated = true;

    // Step 3: Disk-space preflight (`free >= 2 * D + B + 512 MiB`).
    let data_bytes = match directory_data_size_bytes(&options.data_dir) {
        Ok(size) => size,
        Err(error) => {
            report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
            report.error_code = Some("data_dir_inspect_failed".to_string());
            report.message = Some(format!(
                "failed to measure data-dir `{}`: {error}",
                options.data_dir.display()
            ));
            return report;
        }
    };
    let required_free = data_bytes
        .saturating_mul(2)
        .saturating_add(artifact.binary_size_bytes)
        .saturating_add(UPGRADE_HEADROOM_BYTES);
    let available_free = match hooks.free_disk_bytes(&options.backup_dir) {
        Ok(bytes) => bytes,
        Err(error) => {
            report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
            report.error_code = Some("disk_check_failed".to_string());
            report.message = Some(error);
            return report;
        }
    };
    report.data_dir_bytes = data_bytes;
    report.required_free_bytes = required_free;
    report.available_free_bytes = available_free;
    report.disk_headroom_ok = available_free >= required_free;
    if !report.disk_headroom_ok {
        report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
        report.error_code = Some("insufficient_disk_space".to_string());
        report.message = Some(format!(
            "insufficient_disk_space: free {available_free} bytes < required {required_free} bytes (2x data_dir {data_bytes} + binary {} + 512 MiB)",
            artifact.binary_size_bytes
        ));
        return report;
    }

    // Verify local candidate binary checksum if supplied in --check mode too.
    let mut downloaded_temp: Option<PathBuf> = None;
    let candidate_path = if let Some(local_bin) = &options.binary_file {
        match sha256_file_and_size(local_bin) {
            Ok((digest, size)) => {
                if digest != artifact.binary_sha256 || size != artifact.binary_size_bytes {
                    report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
                    report.error_code = Some("binary_checksum_mismatch".to_string());
                    report.message = Some(format!(
                        "candidate binary checksum/size mismatch: got {digest} ({size}B), expected {} ({}B)",
                        artifact.binary_sha256, artifact.binary_size_bytes
                    ));
                    return report;
                }
                Some(local_bin.clone())
            }
            Err(error) => {
                report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
                report.error_code = Some("binary_unreadable".to_string());
                report.message = Some(format!(
                    "failed to read candidate binary `{}`: {error}",
                    local_bin.display()
                ));
                return report;
            }
        }
    } else if !options.check_only {
        // Download candidate binary over bounded HTTPS and verify digest.
        let max_bin = usize::try_from(artifact.binary_size_bytes.saturating_add(1024))
            .unwrap_or(256 * 1024 * 1024);
        let bytes = match fetch_https_bounded(&artifact.binary_url, max_bin) {
            Ok(b) => b,
            Err(error) => {
                report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
                report.error_code = Some("binary_download_failed".to_string());
                report.message = Some(error);
                return report;
            }
        };
        let digest = crate::validator_uptime::sha256_hex(&bytes);
        if digest != artifact.binary_sha256 || bytes.len() as u64 != artifact.binary_size_bytes {
            report.exit_code = UPGRADE_EXIT_SECURITY_REFUSAL;
            report.error_code = Some("binary_checksum_mismatch".to_string());
            report.message = Some(format!(
                "downloaded binary checksum/size mismatch: got {digest} ({}B), expected {} ({}B)",
                bytes.len(),
                artifact.binary_sha256,
                artifact.binary_size_bytes
            ));
            return report;
        }
        let temp_path = options.backup_dir.join(format!(
            ".postfiat-upgrade-candidate-{}.bin",
            std::process::id()
        ));
        if let Err(error) =
            fs::create_dir_all(&options.backup_dir).and_then(|()| fs::write(&temp_path, &bytes))
        {
            report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
            report.error_code = Some("binary_stage_failed".to_string());
            report.message = Some(format!("failed to stage downloaded binary: {error}"));
            return report;
        }
        downloaded_temp = Some(temp_path.clone());
        Some(temp_path)
    } else {
        None
    };

    // Step 4: `--check` read-only exit codes (0, 10, 11).
    if options.check_only {
        let already_at_release = pre_probe
            .as_ref()
            .is_some_and(|p| revision_matches(p.git_revision.as_deref(), &manifest.git_revision))
            || options
                .binary_path
                .is_file()
                .then(|| sha256_file_and_size(&options.binary_path).ok())
                .flatten()
                .is_some_and(|(sha, sz)| {
                    sha == artifact.binary_sha256 && sz == artifact.binary_size_bytes
                });

        if already_at_release {
            report.status = "up_to_date".to_string();
            report.exit_code = UPGRADE_EXIT_OK;
            report.message = Some(format!("already at release `{}`", manifest.release_id));
            return report;
        }

        let in_activation_warning = manifest.rollout.mode == "activation_height"
            && manifest
                .rollout
                .activation_height
                .zip(manifest.rollout.advance_warning_blocks)
                .is_some_and(|(act_h, warn_blocks)| {
                    let current_h = pre_probe.as_ref().map_or(0, |p| p.block_height);
                    act_h.saturating_sub(current_h) <= warn_blocks
                });

        if in_activation_warning {
            report.status = "activation_imminent".to_string();
            report.exit_code = UPGRADE_EXIT_ACTIVATION_WARNING;
            report.message = Some(format!(
                "activation_height {} is within advance_warning_blocks",
                manifest.rollout.activation_height.unwrap_or(0)
            ));
        } else {
            report.status = "upgrade_available".to_string();
            report.exit_code = UPGRADE_EXIT_AVAILABLE;
            report.message = Some(format!(
                "verified {} upgrade `{}` is available",
                manifest.rollout.mode, manifest.release_id
            ));
        }
        return report;
    }

    // Apply mode requires `--yes` when stdin is non-interactive.
    #[cfg(unix)]
    let stdin_is_tty = unsafe { libc::isatty(libc::STDIN_FILENO) != 0 };
    #[cfg(not(unix))]
    let stdin_is_tty = false;
    if !options.yes && !stdin_is_tty {
        if let Some(tmp) = downloaded_temp {
            let _ = fs::remove_file(tmp);
        }
        report.exit_code = UPGRADE_EXIT_INVALID_ARGS;
        report.error_code = Some("confirmation_required".to_string());
        report.message = Some(
            "non-interactive upgrade requires --yes (or run with --check for dry-run)".to_string(),
        );
        return report;
    }

    let staged_binary = match candidate_path {
        Some(p) => p,
        None => {
            report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
            report.error_code = Some("missing_candidate_binary".to_string());
            report.message = Some("no candidate binary staged".to_string());
            return report;
        }
    };

    let pre_height = pre_probe.as_ref().map_or(0, |p| p.block_height);
    let had_socket_signer = pre_probe
        .as_ref()
        .and_then(|p| p.signer_mode.as_deref())
        .is_some_and(|mode| mode == "socket");

    // Step 5: Journal, then graceful stop.
    let journal_path = options.backup_dir.join(UPGRADE_JOURNAL_FILE);
    let mut journal = UpgradeJournalV1 {
        schema: UPGRADE_JOURNAL_SCHEMA.to_string(),
        release_id: manifest.release_id.clone(),
        release_sequence: manifest.release_sequence,
        started_unix: now_unix,
        step: "stopping".to_string(),
        data_dir: options.data_dir.display().to_string(),
        binary_path: options.binary_path.display().to_string(),
        signer_guard_dir: options.signer_guard_dir.display().to_string(),
        backup_root: None,
        service_units: options.service_units.clone(),
        exclude_paths: excludes
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect(),
    };
    let discard_candidate = |downloaded: &Option<PathBuf>| {
        if let Some(tmp) = downloaded {
            let _ = fs::remove_file(tmp);
        }
    };
    if let Err(error) = write_json_atomically(&journal_path, &journal) {
        discard_candidate(&downloaded_temp);
        report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
        report.error_code = Some("journal_write_failed".to_string());
        report.message = Some(error);
        return report;
    }
    // Nothing of the installation has changed yet: a failure here ends with
    // the units started again and the journal removed.
    let abort_before_swap = |report: &mut UpgradeReportV1, code: &str, error: String| {
        let start = hooks.start_units(&options.service_units);
        let stopped = stopped_units(hooks, &options.service_units);
        let _ = fs::remove_file(&journal_path);
        report.error_code = Some(code.to_string());
        if start.is_ok() && stopped.is_empty() {
            report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
            report.message = Some(error);
        } else {
            report.status = "rollback_failed".to_string();
            report.exit_code = UPGRADE_EXIT_ROLLBACK_FAILED;
            report.units_stopped = stopped;
            report.message = Some(format!(
                "{error}; the units could not all be started again ({})",
                start
                    .err()
                    .unwrap_or_else(|| "see units_stopped".to_string())
            ));
        }
    };
    if let Err(error) = hooks.stop_units(&options.service_units) {
        discard_candidate(&downloaded_temp);
        abort_before_swap(&mut report, "service_stop_failed", error);
        return report;
    }

    // Step 6: Chain-state backup + prior binary.
    let (backup_root, backup_binary, backup_data_dir) = match create_pre_upgrade_backup(
        &manifest.release_id,
        now_unix,
        &options.data_dir,
        &options.binary_path,
        &options.backup_dir,
        &options.signer_guard_dir,
        &excludes,
    ) {
        Ok(paths) => paths,
        Err(error) => {
            discard_candidate(&downloaded_temp);
            let partial = backup_snapshot_dir(&options.backup_dir, &manifest.release_id, now_unix);
            if !partial.join("backup-manifest.json").exists() {
                let _ = fs::remove_dir_all(&partial);
            }
            abort_before_swap(&mut report, "backup_failed", error);
            return report;
        }
    };
    report.backup_path = Some(backup_root.display().to_string());
    journal.step = "backed_up".to_string();
    journal.backup_root = Some(backup_root.display().to_string());
    if let Err(error) = write_json_atomically(&journal_path, &journal) {
        discard_candidate(&downloaded_temp);
        abort_before_swap(&mut report, "journal_write_failed", error);
        return report;
    }

    let rollback = |cause: &str, report: &mut UpgradeReportV1| {
        rollback_and_verify(
            hooks,
            &options.service_units,
            &backup_binary,
            &backup_data_dir,
            &options.binary_path,
            &options.data_dir,
            &options.signer_guard_dir,
            &excludes,
            (&options.rpc_host, options.rpc_port),
            &journal_path,
            cause,
            report,
        )
    };

    // Step 7: Atomic binary swap.
    let swap_result = atomic_install_binary(&staged_binary, &options.binary_path);
    discard_candidate(&downloaded_temp);
    if let Err(error) = swap_result {
        return rollback(&format!("atomic binary swap failed: {error}"), &mut report);
    }
    journal.step = "swapped".to_string();
    if let Err(error) = write_json_atomically(&journal_path, &journal) {
        return rollback(&error, &mut report);
    }

    // Step 8: Start services and enforce post-upgrade health gate.
    if let Err(error) = hooks.start_units(&options.service_units) {
        return rollback(
            &format!("post-upgrade service start failed: {error}"),
            &mut report,
        );
    }
    journal.step = "started".to_string();
    if let Err(error) = write_json_atomically(&journal_path, &journal) {
        return rollback(&error, &mut report);
    }

    let deadline = Instant::now() + Duration::from_millis(options.health_timeout_ms.max(100));
    let mut last_health_err = "post-upgrade health timeout".to_string();
    let mut healthy = false;
    while Instant::now() <= deadline {
        match hooks.probe_node(&options.rpc_host, options.rpc_port, 2_000) {
            Ok(probe) => {
                let rev_ok =
                    revision_matches(probe.git_revision.as_deref(), &manifest.git_revision);
                let height_ok = probe.block_height >= pre_height;
                let signer_ok =
                    if had_socket_signer || probe.signer_mode.as_deref() == Some("socket") {
                        probe.signer_mode.as_deref() == Some("socket")
                            && probe.signer_double_sign_protection == Some(true)
                    } else {
                        true
                    };
                let stopped = stopped_units(hooks, &options.service_units);
                if rev_ok && height_ok && signer_ok && stopped.is_empty() {
                    report.installed_git_revision = probe.git_revision;
                    healthy = true;
                    break;
                }
                last_health_err = format!(
                    "health gate mismatch: rev_ok={rev_ok} (node reports {:?}), height_ok={height_ok} ({}<{pre_height}), signer_ok={signer_ok}, units_not_running={stopped:?}",
                    probe.git_revision, probe.block_height
                );
            }
            Err(error) => {
                last_health_err = error;
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    if !healthy {
        return rollback(&last_health_err, &mut report);
    }

    let record = InstalledReleaseV1 {
        schema: INSTALLED_RELEASE_SCHEMA.to_string(),
        release_id: manifest.release_id.clone(),
        release_sequence: manifest.release_sequence,
        git_revision: manifest.git_revision.clone(),
        installed_unix: now_unix,
    };
    let recorded = write_json_atomically(&options.backup_dir.join(INSTALLED_RELEASE_FILE), &record);
    let _ = fs::remove_file(&journal_path);
    report.installed_release_id = Some(record.release_id.clone());
    report.installed_release_sequence = Some(record.release_sequence);
    report.status = "upgraded".to_string();
    report.exit_code = UPGRADE_EXIT_OK;
    report.message = Some(match recorded {
        Ok(()) => format!(
            "successfully upgraded to `{}` (git {})",
            manifest.release_id, manifest.git_revision
        ),
        Err(error) => format!(
            "upgraded to `{}`, but the installed-release record could not be written ({error}); the next upgrade cannot check its order until it is",
            manifest.release_id
        ),
    });
    report
}

/// Finish what an interrupted run started: put the prior binary and chain
/// state back if they were touched, start the units, and remove the journal.
fn recover_interrupted_upgrade(
    journal: &UpgradeJournalV1,
    options: &UpgradeOptions,
    hooks: &dyn UpgradeSystemHooks,
    mut report: UpgradeReportV1,
) -> UpgradeReportV1 {
    let journal_path = options.backup_dir.join(UPGRADE_JOURNAL_FILE);
    report.release_id = Some(journal.release_id.clone());
    report.release_sequence = Some(journal.release_sequence);
    let cause = format!(
        "an earlier upgrade to `{}` was interrupted at step `{}`",
        journal.release_id, journal.step
    );
    let excludes = match backup_excludes(&journal.exclude_paths) {
        Ok(excludes) => excludes,
        Err(error) => {
            report.exit_code = UPGRADE_EXIT_ROLLBACK_FAILED;
            report.status = "rollback_failed".to_string();
            report.error_code = Some("upgrade_journal_invalid".to_string());
            report.message = Some(error);
            return report;
        }
    };
    let complete_backup = journal
        .backup_root
        .as_ref()
        .map(PathBuf::from)
        .filter(|root| root.join("backup-manifest.json").is_file());
    match complete_backup {
        Some(root) => {
            report.backup_path = Some(root.display().to_string());
            let (backup_binary, backup_data_dir) = backup_paths(&root);
            rollback_and_verify(
                hooks,
                &journal.service_units,
                &backup_binary,
                &backup_data_dir,
                Path::new(&journal.binary_path),
                Path::new(&journal.data_dir),
                Path::new(&journal.signer_guard_dir),
                &excludes,
                (&options.rpc_host, options.rpc_port),
                &journal_path,
                &cause,
                &mut report,
            )
        }
        None => {
            // Interrupted before the backup was complete: the installation is
            // unchanged. Remove the partial backup and start the units.
            let partial = backup_snapshot_dir(
                &options.backup_dir,
                &journal.release_id,
                journal.started_unix,
            );
            if !partial.join("backup-manifest.json").exists() {
                let _ = fs::remove_dir_all(&partial);
            }
            let start = hooks.start_units(&journal.service_units);
            let health =
                start.and_then(|()| hooks.probe_node(&options.rpc_host, options.rpc_port, 5_000));
            let stopped = stopped_units(hooks, &journal.service_units);
            match health {
                Ok(_) if stopped.is_empty() => {
                    let _ = fs::remove_file(&journal_path);
                    report.status = "rolled_back".to_string();
                    report.exit_code = UPGRADE_EXIT_ROLLED_BACK;
                    report.error_code = Some("rolled_back".to_string());
                    report.message = Some(format!(
                        "{cause}; nothing had been changed, the units are running again"
                    ));
                }
                other => {
                    report.status = "rollback_failed".to_string();
                    report.exit_code = UPGRADE_EXIT_ROLLBACK_FAILED;
                    report.error_code = Some("rollback_failed".to_string());
                    report.units_stopped = stopped;
                    report.message = Some(format!(
                        "CRITICAL: {cause} and the units did not come back ({})",
                        other
                            .err()
                            .unwrap_or_else(|| "see units_stopped".to_string())
                    ));
                }
            }
            report
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rollback_and_verify(
    hooks: &dyn UpgradeSystemHooks,
    service_units: &[String],
    backup_binary: &Path,
    backup_data_dir: &Path,
    binary_path: &Path,
    data_dir: &Path,
    signer_guard_dir: &Path,
    excludes: &[PathBuf],
    rpc: (&str, u16),
    journal_path: &Path,
    cause: &str,
    report: &mut UpgradeReportV1,
) -> UpgradeReportV1 {
    let _ = hooks.stop_units(service_units);
    let restore = restore_pre_upgrade_backup(
        backup_binary,
        backup_data_dir,
        binary_path,
        data_dir,
        signer_guard_dir,
        excludes,
    );
    // The units are started whether or not the restore worked: a validator
    // left stopped helps nobody, and the report names what is not running.
    let start = hooks.start_units(service_units);
    let health = restore
        .clone()
        .and_then(|()| start.clone())
        .and_then(|()| hooks.probe_node(rpc.0, rpc.1, 5_000).map(|_| ()));
    let stopped = stopped_units(hooks, service_units);

    match health {
        Ok(()) if stopped.is_empty() => {
            let _ = fs::remove_file(journal_path);
            report.status = "rolled_back".to_string();
            report.exit_code = UPGRADE_EXIT_ROLLED_BACK;
            report.error_code = Some("rolled_back".to_string());
            report.rolled_back = true;
            report.message = Some(format!(
                "upgrade failed ({cause}); the prior binary and chain state are back, the vote record was carried forward, signer-guard untouched"
            ));
        }
        other => {
            // The journal stays: the next run tries the rollback again.
            report.status = "rollback_failed".to_string();
            report.exit_code = UPGRADE_EXIT_ROLLBACK_FAILED;
            report.error_code = Some("rollback_failed".to_string());
            report.rolled_back = false;
            report.units_stopped = stopped;
            report.message = Some(format!(
                "CRITICAL: upgrade failed ({cause}) AND rollback failed ({})",
                other
                    .err()
                    .unwrap_or_else(|| "units are not running".to_string())
            ));
        }
    }
    report.clone()
}

pub fn format_upgrade_human_report(report: &UpgradeReportV1) -> String {
    let mut lines = Vec::new();
    lines.push(format!(
        "postfiat-node upgrade [{}]: status={} exit={}",
        if report.check_only { "check" } else { "apply" },
        report.status,
        report.exit_code
    ));
    if let Some(rel) = &report.release_id {
        lines.push(format!(
            "  release: {} (git={}, arch={})",
            rel,
            report.target_git_revision.as_deref().unwrap_or("unknown"),
            report.architecture.as_deref().unwrap_or("unknown")
        ));
    }
    if let Some(mode) = &report.rollout_mode {
        let act = report
            .activation_height
            .map(|h| format!(" activation_height={h}"))
            .unwrap_or_default();
        let rem = report
            .blocks_until_activation
            .map(|b| format!(" remaining_blocks={b}"))
            .unwrap_or_default();
        lines.push(format!(
            "  rollout: mode={mode}{act}{rem} halt_if_missed={}",
            report.halt_if_missed.unwrap_or(false)
        ));
    }
    if !report.verified_signers.is_empty() {
        lines.push(format!(
            "  signatures: {} verified ({})",
            report.verified_signers.len(),
            report.verified_signers.join(", ")
        ));
    }
    lines.push(format!(
        "  signer_guard_isolated: {}  disk_headroom_ok: {} (free={}B required={}B data={}B)",
        report.signer_guard_isolated,
        report.disk_headroom_ok,
        report.available_free_bytes,
        report.required_free_bytes,
        report.data_dir_bytes
    ));
    if let Some(backup) = &report.backup_path {
        lines.push(format!("  backup: {backup}"));
    }
    if !report.units_stopped.is_empty() {
        lines.push(format!(
            "  UNITS NOT RUNNING: {}",
            report.units_stopped.join(", ")
        ));
    }
    if let Some(code) = &report.error_code {
        lines.push(format!("  error_code: {code}"));
    }
    if let Some(msg) = &report.message {
        lines.push(format!("  detail: {msg}"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{address_from_public_key, ml_dsa_65_keygen};
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;

    struct TestTempDir(PathBuf);
    impl TestTempDir {
        fn new(label: &str) -> Self {
            static SEQ: AtomicUsize = AtomicUsize::new(0);
            let dir = std::env::temp_dir().join(format!(
                "postfiat-upgrade-test-{label}-{}-{}",
                std::process::id(),
                SEQ.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&dir).expect("mkdir");
            Self(dir)
        }
    }
    impl Drop for TestTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    const CHAIN: &str = "postfiat-testnet-1";
    const TARGET_REV: &str = "1111111111111111111111111111111111111111";
    const NOW: u64 = 1_795_651_600;

    /// A stand-in for systemd and the node. The node it models reports an
    /// 8-character revision, as the real one does, and the revision follows the
    /// binary that is installed when the units start.
    struct MockHooks {
        installed_bin: PathBuf,
        data_dir: PathBuf,
        free_bytes: u64,
        height: u64,
        /// The new binary starts but never answers its health probe.
        new_binary_unhealthy: bool,
        /// The new binary casts a vote before it is found unhealthy.
        new_binary_votes: bool,
        /// Nothing answers after a rollback either.
        dead_after_rollback: bool,
        /// A unit that systemd reports as not running whatever is done.
        dead_unit: Option<String>,
        running: Mutex<bool>,
        log: Mutex<Vec<String>>,
    }

    impl MockHooks {
        fn new(installed_bin: &Path, data_dir: &Path) -> Self {
            Self {
                installed_bin: installed_bin.to_path_buf(),
                data_dir: data_dir.to_path_buf(),
                free_bytes: 10 * 1024 * 1024 * 1024,
                height: 432_000,
                new_binary_unhealthy: false,
                new_binary_votes: false,
                dead_after_rollback: false,
                dead_unit: None,
                running: Mutex::new(true),
                log: Mutex::new(Vec::new()),
            }
        }
        fn new_binary_installed(&self) -> bool {
            fs::read(&self.installed_bin).is_ok_and(|bytes| bytes == b"NEW_BINARY_BYTES_V3")
        }
        fn log(&self) -> Vec<String> {
            self.log.lock().expect("log").clone()
        }
    }

    impl UpgradeSystemHooks for MockHooks {
        fn stop_units(&self, _units: &[String]) -> Result<(), String> {
            *self.running.lock().expect("running") = false;
            self.log.lock().expect("log").push("stop".to_string());
            Ok(())
        }
        fn start_units(&self, _units: &[String]) -> Result<(), String> {
            *self.running.lock().expect("running") = true;
            self.log.lock().expect("log").push("start".to_string());
            if self.new_binary_installed() {
                // The new binary writes chain state as soon as it runs.
                fs::write(self.data_dir.join("ledger.dat"), b"REWRITTEN_BY_NEW_BINARY")
                    .expect("ledger");
            }
            if self.new_binary_installed() && self.new_binary_votes {
                let safety = self.data_dir.join(crate::CONSENSUS_V2_SAFETY_DIR);
                fs::create_dir_all(&safety).expect("safety dir");
                fs::write(safety.join("height-432001.json"), b"VOTED_BY_NEW_BINARY").expect("vote");
            }
            Ok(())
        }
        fn probe_node(
            &self,
            _host: &str,
            _port: u16,
            _timeout_ms: u64,
        ) -> Result<UpgradeNodeStatusProbe, String> {
            if !*self.running.lock().expect("running") {
                return Err("connection refused".to_string());
            }
            let new = self.new_binary_installed();
            if new && self.new_binary_unhealthy {
                return Err("post-upgrade probe failed".to_string());
            }
            if !new
                && self.dead_after_rollback
                && self.log().iter().filter(|l| *l == "start").count() > 0
            {
                return Err("node failed to restart".to_string());
            }
            Ok(UpgradeNodeStatusProbe {
                chain_id: CHAIN.to_string(),
                genesis_hash: "aa".repeat(48),
                block_height: self.height + u64::from(new),
                git_revision: Some(if new {
                    TARGET_REV[..8].to_string()
                } else {
                    "00000000".to_string()
                }),
                signer_mode: Some("socket".to_string()),
                signer_double_sign_protection: Some(true),
            })
        }
        fn free_disk_bytes(&self, _path: &Path) -> Result<u64, String> {
            Ok(self.free_bytes)
        }
        fn unit_is_active(&self, unit: &str) -> bool {
            *self.running.lock().expect("running") && self.dead_unit.as_deref() != Some(unit)
        }
    }

    struct Fixture {
        _tmp: TestTempDir,
        root: PathBuf,
        keys: Vec<(String, String, String, String)>,
        keyset: ReleaseKeysetV1,
        options: UpgradeOptions,
    }

    fn rolling() -> ReleaseRolloutV3 {
        ReleaseRolloutV3 {
            mode: "rolling".to_string(),
            activation_height: None,
            advance_warning_blocks: None,
            halt_if_missed: false,
        }
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let tmp = TestTempDir::new(label);
            let root = tmp.0.clone();
            let mut keys = Vec::new();
            let mut signers = Vec::new();
            for (id, role) in [
                ("release-lead-1", "release_lead"),
                ("independent-builder-2", "independent_builder"),
                ("security-custodian-3", "security_custodian"),
            ] {
                let kp = ml_dsa_65_keygen().expect("keygen");
                let pk_hex = bytes_to_hex(&kp.public_key);
                let publisher = address_from_public_key(&kp.public_key);
                signers.push(ReleaseKeysetSignerV1 {
                    signer_id: id.to_string(),
                    role: role.to_string(),
                    publisher: publisher.clone(),
                    public_key_hex: pk_hex.clone(),
                });
                keys.push((
                    id.to_string(),
                    publisher,
                    pk_hex,
                    bytes_to_hex(&kp.private_key),
                ));
            }
            let keyset = ReleaseKeysetV1 {
                schema: RELEASE_KEYSET_SCHEMA.to_string(),
                keyset_id: "postfiat-release-keyset-2026-v1".to_string(),
                threshold: 2,
                total_signers: 3,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                signing_context: RELEASE_MANIFEST_SIGNING_CONTEXT.to_string(),
                signers,
            };
            let keyset_file = root.join("release-keyset.json");
            fs::write(&keyset_file, serde_json::to_string_pretty(&keyset).unwrap()).unwrap();
            let candidate = root.join("candidate-postfiat-node");
            fs::write(&candidate, b"NEW_BINARY_BYTES_V3").unwrap();
            let installed = root.join("installed-postfiat-node");
            fs::write(&installed, b"OLD_BINARY_BYTES_V2").unwrap();

            // A validator's data directory as it is on the fleet today: chain
            // state, the vote record, the key file and runtime state together.
            let data_dir = root.join("data");
            fs::create_dir_all(data_dir.join("blocks")).unwrap();
            fs::write(data_dir.join("ledger.dat"), b"CHAIN_STATE_V2").unwrap();
            fs::write(data_dir.join("blocks/000001.json"), b"BLOCK_1").unwrap();
            fs::create_dir_all(data_dir.join(crate::CONSENSUS_V2_SAFETY_DIR)).unwrap();
            fs::write(
                data_dir
                    .join(crate::CONSENSUS_V2_SAFETY_DIR)
                    .join("height-432000.json"),
                b"VOTED_BY_OLD_BINARY",
            )
            .unwrap();
            fs::write(
                data_dir.join(crate::VALIDATOR_KEYS_FILE),
                b"SECRET_VALIDATOR_KEY",
            )
            .unwrap();
            fs::create_dir_all(data_dir.join("runtime")).unwrap();
            fs::write(data_dir.join("runtime/spool"), b"RUNTIME").unwrap();
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&data_dir, fs::Permissions::from_mode(0o710)).unwrap();
                fs::set_permissions(
                    data_dir.join("ledger.dat"),
                    fs::Permissions::from_mode(0o600),
                )
                .unwrap();
                fs::set_permissions(data_dir.join("blocks"), fs::Permissions::from_mode(0o700))
                    .unwrap();
                fs::set_permissions(
                    data_dir.join("blocks/000001.json"),
                    fs::Permissions::from_mode(0o640),
                )
                .unwrap();
            }
            let guard_dir = root.join("signer-guard");
            fs::create_dir_all(&guard_dir).unwrap();
            fs::write(
                guard_dir.join("guard.state"),
                b"MONOTONIC_GUARD_FLOOR_432000",
            )
            .unwrap();
            let backup_dir = root.join("backups");
            fs::create_dir_all(&backup_dir).unwrap();

            let options = UpgradeOptions {
                check_only: false,
                manifest_file: None,
                channel_url: None,
                binary_file: Some(candidate),
                keyset_file,
                data_dir,
                signer_guard_dir: guard_dir,
                binary_path: installed,
                backup_dir,
                service_units: vec![
                    "postfiat-validator-0.service".to_string(),
                    "postfiat-validator-0-rpc.service".to_string(),
                ],
                rpc_host: "127.0.0.1".to_string(),
                rpc_port: 27650,
                health_timeout_ms: 250,
                now_unix: Some(NOW),
                architecture: Some("linux-x86_64".to_string()),
                yes: true,
                json: true,
                exclude_paths: Vec::new(),
            };
            let mut fixture = Self {
                _tmp: tmp,
                root,
                keys,
                keyset,
                options,
            };
            fixture.options.manifest_file =
                Some(fixture.manifest("v0.3.0-testnet", 7, rolling(), 2));
            fixture
        }

        fn manifest(
            &self,
            release_id: &str,
            sequence: u64,
            rollout: ReleaseRolloutV3,
            signatures: usize,
        ) -> PathBuf {
            let (sha, size) =
                sha256_file_and_size(self.options.binary_file.as_ref().unwrap()).unwrap();
            let artifact = |arch: &str| ReleaseBinaryArtifactV3 {
                binary_url: format!(
                    "https://releases.postfiat.org/{release_id}/postfiat-node-{arch}"
                ),
                binary_sha256: sha.clone(),
                binary_size_bytes: size,
            };
            let mut manifest = ReleaseManifestV3 {
                schema: RELEASE_MANIFEST_SCHEMA.to_string(),
                release_id: release_id.to_string(),
                release_sequence: sequence,
                created_unix: 1_795_651_200,
                valid_from_unix: 1_795_651_200,
                valid_until_unix: 1_803_427_200,
                chain_id: CHAIN.to_string(),
                genesis_hash: "aa".repeat(48),
                git_revision: TARGET_REV.to_string(),
                build_profile: "release".to_string(),
                build_features: vec![
                    "privacy".to_string(),
                    "rpc".to_string(),
                    "transport".to_string(),
                ],
                protocol_version: 2,
                min_compatible_protocol_version: 1,
                rpc_schema: "postfiat-local-rpc-v1".to_string(),
                rollout,
                checkpoint: ReleaseCheckpointV3 {
                    height: 432_000,
                    block_hash: "bb".repeat(48),
                    state_root: "cc".repeat(48),
                    certificate_id: "dd".repeat(48),
                    committee_epoch: 12,
                },
                artifacts: ReleaseArtifactsV3 {
                    linux_x86_64: artifact("linux-x86_64"),
                    linux_aarch64: artifact("linux-aarch64"),
                    systemd_bundle_sha256: "22".repeat(32),
                    docker_profiles_sha256: "33".repeat(32),
                    sbom_sha256: "44".repeat(32),
                    checksums_sha256: "55".repeat(32),
                    swap_circuit_metadata_sha256: "66".repeat(32),
                    private_egress_circuit_metadata_sha256: "77".repeat(32),
                },
                signing_policy: ReleaseSigningPolicyV3 {
                    keyset_id: self.keyset.keyset_id.clone(),
                    threshold: 2,
                    total_signers: 3,
                    algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                    signing_context: RELEASE_MANIFEST_SIGNING_CONTEXT.to_string(),
                },
                signatures: Vec::new(),
            };
            for (id, publisher, pk, sk) in self.keys.iter().take(signatures) {
                let sig = sign_release_manifest_v3(&manifest, id, publisher, pk, sk, 1_795_651_260)
                    .expect("sign");
                manifest.signatures.push(sig);
            }
            let path = self.root.join(format!(
                "release-manifest-{release_id}-{sequence}-{signatures}.json"
            ));
            fs::write(&path, serde_json::to_string_pretty(&manifest).unwrap()).unwrap();
            path
        }

        fn hooks(&self) -> MockHooks {
            MockHooks::new(&self.options.binary_path, &self.options.data_dir)
        }
        fn data(&self, relative: &str) -> Vec<u8> {
            fs::read(self.options.data_dir.join(relative))
                .unwrap_or_else(|error| panic!("{relative}: {error}"))
        }
        fn journal(&self) -> PathBuf {
            self.options.backup_dir.join(UPGRADE_JOURNAL_FILE)
        }
        fn guard(&self) -> Vec<u8> {
            fs::read(self.options.signer_guard_dir.join("guard.state")).unwrap()
        }
    }

    fn mode(path: &Path) -> u32 {
        use std::os::unix::fs::PermissionsExt;
        fs::metadata(path).unwrap().permissions().mode() & 0o7777
    }

    fn files_under(root: &Path) -> Vec<String> {
        let mut out = Vec::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(dir) = stack.pop() {
            for entry in fs::read_dir(dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    out.push(
                        path.strip_prefix(root)
                            .unwrap()
                            .to_string_lossy()
                            .into_owned(),
                    );
                }
            }
        }
        out.sort();
        out
    }

    #[test]
    fn the_node_reports_a_short_revision_and_the_health_gate_accepts_it() {
        // What the build script puts into the binary: `git rev-parse --short=8`.
        assert_eq!(
            env!("POSTFIAT_BUILD_GIT_REV").len().min(8),
            env!("POSTFIAT_BUILD_GIT_REV").len()
        );
        assert!(revision_matches(Some("11111111"), TARGET_REV));
        assert!(revision_matches(Some(TARGET_REV), TARGET_REV));
        assert!(
            !revision_matches(Some("1111111"), TARGET_REV),
            "shorter than 8 proves too little"
        );
        assert!(!revision_matches(Some("11111112"), TARGET_REV));
        assert!(!revision_matches(Some("unknown"), TARGET_REV));
        assert!(
            !revision_matches(None, TARGET_REV),
            "a node that names no revision is not healthy"
        );

        let fixture = Fixture::new("short-rev");
        let hooks = fixture.hooks();
        let report = execute_upgrade(&fixture.options, &hooks);
        assert_eq!(report.exit_code, UPGRADE_EXIT_OK, "{report:?}");
        assert_eq!(report.status, "upgraded");
        assert_eq!(report.installed_git_revision.as_deref(), Some("11111111"));
        assert_eq!(
            fs::read(&fixture.options.binary_path).unwrap(),
            b"NEW_BINARY_BYTES_V3"
        );
        assert_eq!(hooks.log(), ["stop", "start"]);
        assert!(
            !fixture.journal().exists(),
            "the journal is removed at the end"
        );
        let record = read_installed_release(&fixture.options.backup_dir)
            .unwrap()
            .expect("record");
        assert_eq!(
            (record.release_id.as_str(), record.release_sequence),
            ("v0.3.0-testnet", 7)
        );
        assert_eq!(fixture.guard(), b"MONOTONIC_GUARD_FLOOR_432000");
    }

    #[test]
    fn the_backup_holds_no_key_and_no_vote_record_and_keeps_owner_and_mode() {
        let fixture = Fixture::new("backup");
        let hooks = fixture.hooks();
        let report = execute_upgrade(&fixture.options, &hooks);
        assert_eq!(report.exit_code, UPGRADE_EXIT_OK, "{report:?}");
        let backup = PathBuf::from(report.backup_path.expect("backup path"));
        assert_eq!(mode(&backup), 0o700, "the backup is the owner's alone");
        assert_eq!(
            files_under(&backup.join("chain-data")),
            ["blocks/000001.json", "ledger.dat"],
            "chain state only: no key file, no vote record, no runtime state"
        );
        for (relative, expected) in [
            ("", 0o710),
            ("ledger.dat", 0o600),
            ("blocks", 0o700),
            ("blocks/000001.json", 0o640),
        ] {
            assert_eq!(
                mode(&backup.join("chain-data").join(relative)),
                expected,
                "{relative}"
            );
        }
        let manifest: UpgradeBackupManifestV1 =
            serde_json::from_slice(&fs::read(backup.join("backup-manifest.json")).unwrap())
                .unwrap();
        assert!(manifest
            .excluded_paths
            .contains(&crate::CONSENSUS_V2_SAFETY_DIR.to_string()));
        assert!(manifest
            .excluded_paths
            .contains(&crate::VALIDATOR_KEYS_FILE.to_string()));
        use std::os::unix::fs::MetadataExt;
        assert_eq!(
            fs::metadata(backup.join("chain-data/ledger.dat"))
                .unwrap()
                .uid(),
            fs::metadata(fixture.options.data_dir.join("ledger.dat"))
                .unwrap()
                .uid()
        );
        // A further exclusion is honoured; one that leaves the data directory is refused.
        let extra = Fixture::new("backup-extra");
        let mut options = extra.options.clone();
        options.exclude_paths = vec!["blocks".to_string()];
        let report = execute_upgrade(&options, &extra.hooks());
        assert_eq!(
            files_under(&PathBuf::from(report.backup_path.unwrap()).join("chain-data")),
            ["ledger.dat"]
        );
        options.exclude_paths = vec!["../elsewhere".to_string()];
        assert_eq!(
            execute_upgrade(&options, &extra.hooks()).exit_code,
            UPGRADE_EXIT_INVALID_ARGS
        );
    }

    #[test]
    fn a_rollback_carries_the_vote_record_forward_and_restores_owner_and_mode() {
        let fixture = Fixture::new("rollback");
        let mut hooks = fixture.hooks();
        hooks.new_binary_unhealthy = true;
        hooks.new_binary_votes = true;
        let report = {
            let report = execute_upgrade(&fixture.options, &hooks);
            assert_eq!(report.exit_code, UPGRADE_EXIT_ROLLED_BACK, "{report:?}");
            report
        };
        assert!(report.rolled_back && report.units_stopped.is_empty());
        assert_eq!(
            fs::read(&fixture.options.binary_path).unwrap(),
            b"OLD_BINARY_BYTES_V2"
        );
        assert_eq!(fixture.data("ledger.dat"), b"CHAIN_STATE_V2");
        // The vote the new binary cast is on record for the old binary: it
        // cannot sign a second, different vote for that height.
        let safety = crate::CONSENSUS_V2_SAFETY_DIR;
        assert_eq!(
            fixture.data(&format!("{safety}/height-432001.json")),
            b"VOTED_BY_NEW_BINARY"
        );
        assert_eq!(
            fixture.data(&format!("{safety}/height-432000.json")),
            b"VOTED_BY_OLD_BINARY"
        );
        assert_eq!(
            fixture.data(crate::VALIDATOR_KEYS_FILE),
            b"SECRET_VALIDATOR_KEY"
        );
        assert_eq!(fixture.data("runtime/spool"), b"RUNTIME");
        // The service user can start the node: nothing changed owner or mode.
        for (relative, expected) in [
            ("", 0o710),
            ("ledger.dat", 0o600),
            ("blocks", 0o700),
            ("blocks/000001.json", 0o640),
        ] {
            assert_eq!(
                mode(&fixture.options.data_dir.join(relative)),
                expected,
                "{relative}"
            );
        }
        assert_eq!(hooks.log(), ["stop", "start", "stop", "start"]);
        assert!(!fixture.journal().exists());
        assert!(read_installed_release(&fixture.options.backup_dir)
            .unwrap()
            .is_none());
        assert_eq!(fixture.guard(), b"MONOTONIC_GUARD_FLOOR_432000");
        let parent = fixture.options.data_dir.parent().unwrap();
        assert!(
            !parent.join(".data.failed-upgrade").exists()
                && !parent.join(".data.rollback-stage").exists()
        );
    }

    #[test]
    fn a_failed_rollback_names_the_stopped_units_and_keeps_the_journal() {
        let fixture = Fixture::new("rollback-failed");
        let mut hooks = fixture.hooks();
        hooks.new_binary_unhealthy = true;
        hooks.dead_after_rollback = true;
        let report = execute_upgrade(&fixture.options, &hooks);
        assert_eq!(report.exit_code, UPGRADE_EXIT_ROLLBACK_FAILED, "{report:?}");
        assert_eq!(report.status, "rollback_failed");
        assert!(fixture.journal().exists(), "the next run tries again");

        // A unit that will not start is named, even when the node answers.
        let fixture = Fixture::new("dead-unit");
        let mut hooks = fixture.hooks();
        hooks.new_binary_unhealthy = true;
        hooks.dead_unit = Some("postfiat-validator-0.service".to_string());
        let report = execute_upgrade(&fixture.options, &hooks);
        assert_eq!(report.exit_code, UPGRADE_EXIT_ROLLBACK_FAILED, "{report:?}");
        assert_eq!(report.units_stopped, ["postfiat-validator-0.service"]);
        assert!(format_upgrade_human_report(&report)
            .contains("UNITS NOT RUNNING: postfiat-validator-0.service"));
        // And a new binary whose RPC answers while the validator unit is down is not healthy.
        let fixture = Fixture::new("dead-unit-new");
        let mut hooks = fixture.hooks();
        hooks.dead_unit = Some("postfiat-validator-0.service".to_string());
        assert_ne!(
            execute_upgrade(&fixture.options, &hooks).exit_code,
            UPGRADE_EXIT_OK
        );
    }

    #[test]
    fn an_older_release_is_refused_and_the_same_release_is_up_to_date() {
        let fixture = Fixture::new("order");
        let hooks = fixture.hooks();
        assert_eq!(
            execute_upgrade(&fixture.options, &hooks).exit_code,
            UPGRADE_EXIT_OK
        );
        let log_after_install = hooks.log();

        let mut options = fixture.options.clone();
        for check_only in [true, false] {
            options.check_only = check_only;
            // Validly signed, inside its validity window, and older.
            options.manifest_file = Some(fixture.manifest("v0.2.9-testnet", 6, rolling(), 2));
            let report = execute_upgrade(&options, &hooks);
            assert_eq!(
                report.exit_code, UPGRADE_EXIT_SECURITY_REFUSAL,
                "{report:?}"
            );
            assert_eq!(report.error_code.as_deref(), Some("release_downgrade"));
            assert_eq!(report.installed_release_sequence, Some(7));
            // The same sequence under another name.
            options.manifest_file = Some(fixture.manifest("v0.3.0-other", 7, rolling(), 2));
            let report = execute_upgrade(&options, &hooks);
            assert_eq!(
                report.error_code.as_deref(),
                Some("release_sequence_conflict")
            );
            // The installed release again.
            options.manifest_file = Some(fixture.manifest("v0.3.0-testnet", 7, rolling(), 2));
            let report = execute_upgrade(&options, &hooks);
            assert_eq!(
                (report.exit_code, report.status.as_str()),
                (UPGRADE_EXIT_OK, "up_to_date")
            );
        }
        assert_eq!(
            hooks.log(),
            log_after_install,
            "a refused release stops no service"
        );
        assert_eq!(
            fs::read(&fixture.options.binary_path).unwrap(),
            b"NEW_BINARY_BYTES_V3"
        );
        // A sequence of 0 is not a release.
        options.manifest_file = Some(fixture.manifest("v0-zero", 0, rolling(), 2));
        assert_eq!(
            execute_upgrade(&options, &hooks).exit_code,
            UPGRADE_EXIT_SECURITY_REFUSAL
        );
        // A damaged record is a refusal, not a fresh start.
        fs::write(
            fixture.options.backup_dir.join(INSTALLED_RELEASE_FILE),
            b"{}",
        )
        .unwrap();
        options.manifest_file = Some(fixture.manifest("v0.4.0-testnet", 8, rolling(), 2));
        let report = execute_upgrade(&options, &hooks);
        assert_eq!(
            report.error_code.as_deref(),
            Some("installed_release_record_invalid")
        );
    }

    #[test]
    fn check_reports_and_refusals() {
        let fixture = Fixture::new("check");
        let mut hooks = fixture.hooks();
        hooks.height = 480_000;
        let mut options = fixture.options.clone();
        options.check_only = true;
        let report = execute_upgrade(&options, &hooks);
        assert_eq!(
            (report.exit_code, report.status.as_str()),
            (UPGRADE_EXIT_AVAILABLE, "upgrade_available")
        );
        assert_eq!(report.release_sequence, Some(7));
        assert_eq!(report.verified_signers.len(), 2);

        // Inside the warning window of an activation height (500_000 - 480_000 <= 28_800).
        options.manifest_file = Some(fixture.manifest(
            "v0.3.1-testnet",
            8,
            ReleaseRolloutV3 {
                mode: "activation_height".to_string(),
                activation_height: Some(500_000),
                advance_warning_blocks: Some(28_800),
                halt_if_missed: true,
            },
            2,
        ));
        let report = execute_upgrade(&options, &hooks);
        assert_eq!(
            (report.exit_code, report.status.as_str()),
            (UPGRADE_EXIT_ACTIVATION_WARNING, "activation_imminent")
        );
        assert_eq!(report.blocks_until_activation, Some(20_000));

        // One signature is not a release.
        options.manifest_file = Some(fixture.manifest("v0.3.2-testnet", 9, rolling(), 1));
        let report = execute_upgrade(&options, &hooks);
        assert_eq!(report.exit_code, UPGRADE_EXIT_SECURITY_REFUSAL);
        assert_eq!(report.status, "refused");

        options.manifest_file = fixture.options.manifest_file.clone();
        let mut overlap = options.clone();
        overlap.signer_guard_dir = options.data_dir.join("nested-guard");
        assert_eq!(
            execute_upgrade(&overlap, &hooks).error_code.as_deref(),
            Some("guard_path_overlap")
        );
        hooks.free_bytes = 1024;
        assert_eq!(
            execute_upgrade(&options, &hooks).error_code.as_deref(),
            Some("insufficient_disk_space")
        );
        assert!(hooks.log().is_empty(), "--check stops and starts nothing");

        // Another chain is refused in apply mode, and there is no flag to skip the check.
        let other = Fixture::new("other-chain");
        struct OtherChain(MockHooks);
        impl UpgradeSystemHooks for OtherChain {
            fn stop_units(&self, units: &[String]) -> Result<(), String> {
                self.0.stop_units(units)
            }
            fn start_units(&self, units: &[String]) -> Result<(), String> {
                self.0.start_units(units)
            }
            fn probe_node(
                &self,
                host: &str,
                port: u16,
                timeout_ms: u64,
            ) -> Result<UpgradeNodeStatusProbe, String> {
                let mut probe = self.0.probe_node(host, port, timeout_ms)?;
                probe.chain_id = "some-other-chain".to_string();
                Ok(probe)
            }
            fn free_disk_bytes(&self, path: &Path) -> Result<u64, String> {
                self.0.free_disk_bytes(path)
            }
            fn unit_is_active(&self, unit: &str) -> bool {
                self.0.unit_is_active(unit)
            }
        }
        let hooks = OtherChain(other.hooks());
        let report = execute_upgrade(&other.options, &hooks);
        assert_eq!(
            report.error_code.as_deref(),
            Some("chain_identity_mismatch")
        );
        assert!(hooks.0.log().is_empty());
    }

    #[test]
    fn an_interrupted_upgrade_is_rolled_back_before_anything_else() {
        // Interrupted after the swap: the new binary is in place, the journal says so.
        let fixture = Fixture::new("interrupted");
        let mut hooks = fixture.hooks();
        hooks.new_binary_unhealthy = true;
        hooks.new_binary_votes = true;
        hooks.dead_after_rollback = true;
        let first = execute_upgrade(&fixture.options, &hooks);
        assert_eq!(first.exit_code, UPGRADE_EXIT_ROLLBACK_FAILED, "{first:?}");
        // Put the machine into the state a power cut after the swap leaves: new
        // binary installed, journal at `swapped`, units down.
        fs::write(&fixture.options.binary_path, b"NEW_BINARY_BYTES_V3").unwrap();
        fs::write(
            fixture.options.data_dir.join("ledger.dat"),
            b"REWRITTEN_BY_NEW_BINARY",
        )
        .unwrap();
        let mut journal = read_upgrade_journal(&fixture.options.backup_dir)
            .unwrap()
            .expect("journal");
        journal.step = "swapped".to_string();
        write_json_atomically(&fixture.journal(), &journal).unwrap();

        let recover = fixture.hooks();
        *recover.running.lock().unwrap() = false;
        let mut check = fixture.options.clone();
        check.check_only = true;
        let report = execute_upgrade(&check, &recover);
        assert_eq!(report.exit_code, UPGRADE_EXIT_PREFLIGHT_FAILED);
        assert_eq!(report.error_code.as_deref(), Some("interrupted_upgrade"));
        assert!(recover.log().is_empty(), "--check changes nothing");

        // Any run finishes the rollback first, whatever manifest it was given.
        let mut next = fixture.options.clone();
        next.manifest_file = Some(fixture.manifest("v0.4.0-testnet", 8, rolling(), 2));
        let report = execute_upgrade(&next, &recover);
        assert_eq!(report.exit_code, UPGRADE_EXIT_ROLLED_BACK, "{report:?}");
        assert_eq!(report.release_id.as_deref(), Some("v0.3.0-testnet"));
        assert_eq!(
            fs::read(&fixture.options.binary_path).unwrap(),
            b"OLD_BINARY_BYTES_V2"
        );
        assert_eq!(fixture.data("ledger.dat"), b"CHAIN_STATE_V2");
        assert_eq!(
            fixture.data(&format!(
                "{}/height-432001.json",
                crate::CONSENSUS_V2_SAFETY_DIR
            )),
            b"VOTED_BY_NEW_BINARY"
        );
        assert_eq!(
            fixture.data(crate::VALIDATOR_KEYS_FILE),
            b"SECRET_VALIDATOR_KEY"
        );
        assert!(!fixture.journal().exists());
        // With the journal gone the next run is an ordinary upgrade.
        assert_eq!(
            execute_upgrade(&next, &fixture.hooks()).exit_code,
            UPGRADE_EXIT_OK
        );

        // Interrupted before the backup was complete: nothing changed, the units are started.
        let early = Fixture::new("interrupted-early");
        write_json_atomically(
            &early.journal(),
            &UpgradeJournalV1 {
                schema: UPGRADE_JOURNAL_SCHEMA.to_string(),
                release_id: "v0.3.0-testnet".to_string(),
                release_sequence: 7,
                started_unix: NOW,
                step: "stopping".to_string(),
                data_dir: early.options.data_dir.display().to_string(),
                binary_path: early.options.binary_path.display().to_string(),
                signer_guard_dir: early.options.signer_guard_dir.display().to_string(),
                backup_root: None,
                service_units: early.options.service_units.clone(),
                exclude_paths: Vec::new(),
            },
        )
        .unwrap();
        let partial = backup_snapshot_dir(&early.options.backup_dir, "v0.3.0-testnet", NOW);
        fs::create_dir_all(partial.join("chain-data")).unwrap();
        let hooks = early.hooks();
        *hooks.running.lock().unwrap() = false;
        let report = execute_upgrade(&early.options, &hooks);
        assert_eq!(report.exit_code, UPGRADE_EXIT_ROLLED_BACK, "{report:?}");
        assert_eq!(hooks.log(), ["start"]);
        assert!(!partial.exists() && !early.journal().exists());
        assert_eq!(
            fs::read(&early.options.binary_path).unwrap(),
            b"OLD_BINARY_BYTES_V2"
        );
        assert_eq!(early.data("ledger.dat"), b"CHAIN_STATE_V2");
    }

    #[test]
    fn a_rollback_that_stopped_while_carrying_entries_over_never_deletes_the_vote_record() {
        let fixture = Fixture::new("carry");
        let report = execute_upgrade(&fixture.options, &{
            let mut hooks = fixture.hooks();
            hooks.new_binary_unhealthy = true;
            hooks.new_binary_votes = true;
            hooks
        });
        assert_eq!(report.exit_code, UPGRADE_EXIT_ROLLED_BACK, "{report:?}");
        // Rebuild the state of a rollback that died after the swap and after
        // moving the vote record, with the key file still in the stash.
        let data_dir = &fixture.options.data_dir;
        let stash = data_dir.parent().unwrap().join(".data.failed-upgrade");
        fs::create_dir_all(&stash).unwrap();
        fs::rename(
            data_dir.join(crate::VALIDATOR_KEYS_FILE),
            stash.join(crate::VALIDATOR_KEYS_FILE),
        )
        .unwrap();
        let backup = PathBuf::from(report.backup_path.unwrap());
        let (backup_binary, backup_data) = backup_paths(&backup);
        restore_pre_upgrade_backup(
            &backup_binary,
            &backup_data,
            &fixture.options.binary_path,
            data_dir,
            &fixture.options.signer_guard_dir,
            &backup_excludes(&[]).unwrap(),
        )
        .expect("second rollback");
        assert_eq!(
            fixture.data(&format!(
                "{}/height-432001.json",
                crate::CONSENSUS_V2_SAFETY_DIR
            )),
            b"VOTED_BY_NEW_BINARY",
            "the record already carried over is still there"
        );
        assert_eq!(
            fixture.data(crate::VALIDATOR_KEYS_FILE),
            b"SECRET_VALIDATOR_KEY"
        );
        assert!(!stash.exists());
    }

    #[test]
    fn a_keyset_rotation_needs_two_signers_of_the_keyset_it_replaces() {
        let fixture = Fixture::new("rotation");
        let mut new_keyset = fixture.keyset.clone();
        new_keyset.keyset_id = "postfiat-release-keyset-2027-v1".to_string();
        let replacement = ml_dsa_65_keygen().expect("keygen");
        new_keyset.signers[2].public_key_hex = bytes_to_hex(&replacement.public_key);
        new_keyset.signers[2].publisher = address_from_public_key(&replacement.public_key);
        let sign = |index: usize, keyset: &ReleaseKeysetV1| {
            sign_release_keyset_rotation(keyset, &fixture.keys[index].0, &fixture.keys[index].3)
                .expect("sign")
        };
        let rotation =
            |signatures: Vec<ReleaseKeysetRotationSignatureV1>| ReleaseKeysetRotationV1 {
                schema: RELEASE_KEYSET_ROTATION_SCHEMA.to_string(),
                prior_keyset_id: fixture.keyset.keyset_id.clone(),
                new_keyset: new_keyset.clone(),
                signatures,
            };
        let good = rotation(vec![sign(0, &new_keyset), sign(2, &new_keyset)]);
        assert_eq!(
            verify_release_keyset_rotation(&good, &fixture.keyset).expect("two prior signers"),
            ["release-lead-1", "security-custodian-3"]
        );
        // One signer, the same signer twice, a signature over another keyset, the new key signing for itself.
        assert!(verify_release_keyset_rotation(
            &rotation(vec![sign(0, &new_keyset)]),
            &fixture.keyset
        )
        .is_err());
        assert!(verify_release_keyset_rotation(
            &rotation(vec![sign(0, &new_keyset), sign(0, &new_keyset)]),
            &fixture.keyset
        )
        .is_err());
        assert!(verify_release_keyset_rotation(
            &rotation(vec![sign(0, &fixture.keyset), sign(1, &new_keyset)]),
            &fixture.keyset
        )
        .is_err());
        let self_signed = sign_release_keyset_rotation(
            &new_keyset,
            "security-custodian-3",
            &bytes_to_hex(&replacement.private_key),
        )
        .unwrap();
        assert!(verify_release_keyset_rotation(
            &rotation(vec![sign(0, &new_keyset), self_signed]),
            &fixture.keyset
        )
        .is_err());
        let mut wrong_prior = good.clone();
        wrong_prior.prior_keyset_id = "some-other-keyset".to_string();
        assert!(verify_release_keyset_rotation(&wrong_prior, &fixture.keyset).is_err());
        let mut same_id = good.clone();
        same_id.new_keyset.keyset_id = fixture.keyset.keyset_id.clone();
        assert!(verify_release_keyset_rotation(&same_id, &fixture.keyset).is_err());

        // Applied: the keyset file is replaced and the old one kept beside it.
        let rotation_file = fixture.root.join("rotation.json");
        fs::write(&rotation_file, serde_json::to_vec(&good).unwrap()).unwrap();
        let keyset_file = &fixture.options.keyset_file;
        assert_eq!(
            apply_release_keyset_rotation(&rotation_file, keyset_file).unwrap(),
            "postfiat-release-keyset-2027-v1"
        );
        let installed: ReleaseKeysetV1 =
            serde_json::from_slice(&fs::read(keyset_file).unwrap()).unwrap();
        assert_eq!(installed, new_keyset);
        assert!(PathBuf::from(format!(
            "{}.postfiat-release-keyset-2026-v1",
            keyset_file.display()
        ))
        .is_file());
        // The same rotation cannot be applied twice, and a manifest under the old keyset is now refused.
        assert!(apply_release_keyset_rotation(&rotation_file, keyset_file).is_err());
        let report = execute_upgrade(&fixture.options, &fixture.hooks());
        assert_eq!(
            report.exit_code, UPGRADE_EXIT_SECURITY_REFUSAL,
            "{report:?}"
        );
    }

    #[test]
    fn the_pre_start_check_ties_the_running_binary_to_a_signed_manifest() {
        let fixture = Fixture::new("prestart");
        let manifest = fixture.options.manifest_file.clone().unwrap();
        let keyset = &fixture.options.keyset_file;
        let candidate = fixture.options.binary_file.clone().unwrap();
        let verified = verify_installed_release(&manifest, keyset, &candidate, "linux-x86_64")
            .expect("the release binary");
        assert_eq!(
            (verified.release_id.as_str(), verified.release_sequence),
            ("v0.3.0-testnet", 7)
        );
        // Another binary, an unsigned manifest, an unknown architecture.
        assert!(verify_installed_release(
            &manifest,
            keyset,
            &fixture.options.binary_path,
            "linux-x86_64"
        )
        .is_err());
        let one_signature = fixture.manifest("v0.3.0-testnet", 7, rolling(), 1);
        assert!(
            verify_installed_release(&one_signature, keyset, &candidate, "linux-x86_64").is_err()
        );
        assert!(verify_installed_release(&manifest, keyset, &candidate, "linux-riscv").is_err());
        // The validity window bounds accepting an upgrade, not running a release:
        // a node must still start after its manifest has expired.
        let parsed: ReleaseManifestV3 =
            serde_json::from_slice(&fs::read(&manifest).unwrap()).unwrap();
        assert!(
            verify_release_manifest_v3(&parsed, &fixture.keyset, parsed.valid_until_unix + 1)
                .is_err()
        );
        assert!(verify_installed_release(&manifest, keyset, &candidate, "linux-x86_64").is_ok());
    }
}
