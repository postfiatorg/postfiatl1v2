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
    pub target_git_revision: Option<String>,
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
    pub skip_rpc_preflight: bool,
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

/// Copy chain state only (regular files and subdirectories, skipping symlinks
/// and sockets, and strictly excluding `signer_guard_dir`), returning file
/// entries with SHA-256 digests.
fn copy_chain_state_tree(
    source_root: &Path,
    dest_root: &Path,
    canonical_guard_dir: &Path,
) -> io::Result<Vec<UpgradeBackupFileEntry>> {
    fs::create_dir_all(dest_root)?;
    let mut entries = Vec::new();
    let mut stack = vec![PathBuf::new()];
    while let Some(rel) = stack.pop() {
        let src_dir = if rel.as_os_str().is_empty() {
            source_root.to_path_buf()
        } else {
            source_root.join(&rel)
        };
        let dst_dir = if rel.as_os_str().is_empty() {
            dest_root.to_path_buf()
        } else {
            dest_root.join(&rel)
        };
        fs::create_dir_all(&dst_dir)?;
        let mut children: Vec<_> = fs::read_dir(&src_dir)?.collect::<io::Result<_>>()?;
        children.sort_by_key(|e| e.file_name());
        for child in children {
            let child_src = child.path();
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
            let child_rel = if rel.as_os_str().is_empty() {
                PathBuf::from(child.file_name())
            } else {
                rel.join(child.file_name())
            };
            if meta.is_dir() {
                stack.push(child_rel);
            } else if meta.is_file() {
                let child_dst = dest_root.join(&child_rel);
                let mut reader = File::open(&child_src)?;
                let mut writer = OpenOptions::new()
                    .create(true)
                    .truncate(true)
                    .write(true)
                    .open(&child_dst)?;
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

fn create_pre_upgrade_backup(
    release_id: &str,
    now_unix: u64,
    data_dir: &Path,
    binary_path: &Path,
    backup_dir: &Path,
    signer_guard_dir: &Path,
) -> Result<(PathBuf, PathBuf, PathBuf), String> {
    let canonical_guard = resolve_normalized_path(signer_guard_dir)
        .map_err(|error| format!("resolve signer-guard-dir failed: {error}"))?;
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
    let snapshot_dir = backup_dir.join(format!("{safe_release}-pre-{now_unix}"));
    if snapshot_dir.exists() {
        return Err(format!(
            "backup directory `{}` already exists",
            snapshot_dir.display()
        ));
    }
    let backup_data_dir = snapshot_dir.join("chain-data");
    let backup_binary = snapshot_dir.join("postfiat-node.prev");
    fs::create_dir_all(&snapshot_dir)
        .map_err(|error| format!("create backup dir failed: {error}"))?;

    let data_files = if data_dir.exists() {
        copy_chain_state_tree(data_dir, &backup_data_dir, &canonical_guard)
            .map_err(|error| format!("chain-state backup failed: {error}"))?
    } else {
        fs::create_dir_all(&backup_data_dir)
            .map_err(|error| format!("create empty chain-data backup failed: {error}"))?;
        Vec::new()
    };

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
        prior_binary_sha256,
        prior_binary_size_bytes,
        data_files,
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)
        .map_err(|error| format!("backup manifest serialize failed: {error}"))?;
    let manifest_path = snapshot_dir.join("backup-manifest.json");
    fs::write(&manifest_path, format!("{manifest_json}\n"))
        .map_err(|error| format!("write backup-manifest.json failed: {error}"))?;
    fsync_dir(&snapshot_dir).map_err(|error| format!("fsync backup dir failed: {error}"))?;
    Ok((snapshot_dir, backup_binary, backup_data_dir))
}

/// Restore the pre-upgrade binary and chain-state directory together atomically,
/// NEVER touching `signer_guard_dir`.
fn restore_pre_upgrade_backup(
    backup_binary: &Path,
    backup_data_dir: &Path,
    dest_binary: &Path,
    dest_data_dir: &Path,
    signer_guard_dir: &Path,
) -> Result<(), String> {
    verify_signer_guard_isolation(signer_guard_dir, dest_data_dir, backup_data_dir)?;
    let canonical_guard = resolve_normalized_path(signer_guard_dir)
        .map_err(|error| format!("resolve signer-guard-dir failed: {error}"))?;

    // 1. Atomically restore prior binary.
    atomic_install_binary(backup_binary, dest_binary)
        .map_err(|error| format!("rollback binary restore failed: {error}"))?;

    // 2. Stage restored chain-data beside dest_data_dir, then swap cleanly.
    let parent = dest_data_dir
        .parent()
        .ok_or_else(|| "data-dir has no parent directory".to_string())?;
    let name = dest_data_dir
        .file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| "invalid data-dir name".to_string())?;
    let pid = std::process::id();
    let restore_stage = parent.join(format!(".{name}.rollback-stage.{pid}"));
    let failed_stash = parent.join(format!(".{name}.failed-upgrade.{pid}"));
    if restore_stage.exists() {
        let _ = fs::remove_dir_all(&restore_stage);
    }
    if failed_stash.exists() {
        let _ = fs::remove_dir_all(&failed_stash);
    }
    copy_chain_state_tree(backup_data_dir, &restore_stage, &canonical_guard)
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
    let _ = fs::remove_dir_all(&failed_stash);
    let _ = fsync_dir(parent);
    Ok(())
}

fn base_report(check_only: bool) -> UpgradeReportV1 {
    UpgradeReportV1 {
        schema: UPGRADE_REPORT_SCHEMA.to_string(),
        check_only,
        status: "failed".to_string(),
        exit_code: UPGRADE_EXIT_INVALID_ARGS,
        error_code: None,
        message: None,
        release_id: None,
        target_git_revision: None,
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
    report.target_git_revision = Some(verification.git_revision.clone());
    report.rollout_mode = Some(verification.rollout_mode.clone());
    report.activation_height = verification.activation_height;
    report.halt_if_missed = Some(verification.halt_if_missed);
    report.verified_signers = verification.verified_signers.clone();

    // Probe local node (chain_id, genesis_hash, current height & git_revision).
    let pre_probe = if options.skip_rpc_preflight {
        None
    } else {
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
            .and_then(|p| p.git_revision.as_deref())
            .is_some_and(|rev| rev == manifest.git_revision)
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
            report.status = "activation_warning".to_string();
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

    // Step 5: Graceful service stop.
    if let Err(error) = hooks.stop_units(&options.service_units) {
        if let Some(tmp) = downloaded_temp {
            let _ = fs::remove_file(tmp);
        }
        report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
        report.error_code = Some("service_stop_failed".to_string());
        report.message = Some(error);
        return report;
    }

    // Step 6: Chain-state-only backup + prior binary.
    let (backup_root, backup_binary, backup_data_dir) = match create_pre_upgrade_backup(
        &manifest.release_id,
        now_unix,
        &options.data_dir,
        &options.binary_path,
        &options.backup_dir,
        &options.signer_guard_dir,
    ) {
        Ok(paths) => paths,
        Err(error) => {
            let _ = hooks.start_units(&options.service_units);
            if let Some(tmp) = downloaded_temp {
                let _ = fs::remove_file(tmp);
            }
            report.exit_code = UPGRADE_EXIT_PREFLIGHT_FAILED;
            report.error_code = Some("backup_failed".to_string());
            report.message = Some(error);
            return report;
        }
    };
    report.backup_path = Some(backup_root.display().to_string());

    // Step 7: Atomic binary swap.
    let swap_result = atomic_install_binary(&staged_binary, &options.binary_path);
    if let Some(tmp) = downloaded_temp {
        let _ = fs::remove_file(tmp);
    }
    if let Err(error) = swap_result {
        let rb = rollback_and_verify(
            hooks,
            options,
            &backup_binary,
            &backup_data_dir,
            &format!("atomic binary swap failed: {error}"),
            &mut report,
        );
        return rb;
    }

    // Step 8: Start services and enforce post-upgrade health gate.
    if let Err(error) = hooks.start_units(&options.service_units) {
        return rollback_and_verify(
            hooks,
            options,
            &backup_binary,
            &backup_data_dir,
            &format!("post-upgrade service start failed: {error}"),
            &mut report,
        );
    }

    let deadline = Instant::now() + Duration::from_millis(options.health_timeout_ms.max(100));
    let mut last_health_err = "post-upgrade health timeout".to_string();
    let mut healthy = false;
    while Instant::now() <= deadline {
        match hooks.probe_node(&options.rpc_host, options.rpc_port, 2_000) {
            Ok(probe) => {
                let rev_ok = probe
                    .git_revision
                    .as_deref()
                    .is_none_or(|rev| rev == manifest.git_revision);
                let height_ok = probe.block_height >= pre_height;
                let signer_ok =
                    if had_socket_signer || probe.signer_mode.as_deref() == Some("socket") {
                        probe.signer_mode.as_deref() == Some("socket")
                            && probe.signer_double_sign_protection == Some(true)
                    } else {
                        true
                    };
                if rev_ok && height_ok && signer_ok {
                    report.installed_git_revision = probe.git_revision;
                    healthy = true;
                    break;
                }
                last_health_err = format!(
                    "health gate mismatch: rev_ok={rev_ok}, height_ok={height_ok} ({}<{pre_height}), signer_ok={signer_ok}",
                    probe.block_height
                );
            }
            Err(error) => {
                last_health_err = error;
            }
        }
        thread::sleep(Duration::from_millis(100));
    }

    if !healthy {
        return rollback_and_verify(
            hooks,
            options,
            &backup_binary,
            &backup_data_dir,
            &last_health_err,
            &mut report,
        );
    }

    report.status = "upgraded".to_string();
    report.exit_code = UPGRADE_EXIT_OK;
    report.message = Some(format!(
        "successfully upgraded to `{}` (git {})",
        manifest.release_id, manifest.git_revision
    ));
    report
}

fn rollback_and_verify(
    hooks: &dyn UpgradeSystemHooks,
    options: &UpgradeOptions,
    backup_binary: &Path,
    backup_data_dir: &Path,
    cause: &str,
    report: &mut UpgradeReportV1,
) -> UpgradeReportV1 {
    let _ = hooks.stop_units(&options.service_units);
    let restore = restore_pre_upgrade_backup(
        backup_binary,
        backup_data_dir,
        &options.binary_path,
        &options.data_dir,
        &options.signer_guard_dir,
    );
    let start = restore
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|()| hooks.start_units(&options.service_units));
    let health = start
        .as_ref()
        .map_err(Clone::clone)
        .and_then(|()| hooks.probe_node(&options.rpc_host, options.rpc_port, 5_000));

    match health {
        Ok(_) => {
            report.status = "rolled_back".to_string();
            report.exit_code = UPGRADE_EXIT_ROLLED_BACK;
            report.error_code = Some("rolled_back".to_string());
            report.rolled_back = true;
            report.message = Some(format!(
                "upgrade health check failed ({cause}); atomic binary+chain-state rollback succeeded (signer-guard untouched)"
            ));
        }
        Err(rb_error) => {
            report.status = "rollback_failed".to_string();
            report.exit_code = UPGRADE_EXIT_ROLLBACK_FAILED;
            report.error_code = Some("rollback_failed".to_string());
            report.rolled_back = false;
            report.message = Some(format!(
                "CRITICAL: upgrade failed ({cause}) AND rollback failed ({rb_error})"
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
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

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

    struct MockHooks {
        free_bytes: u64,
        fail_post_upgrade: AtomicBool,
        probe_calls: AtomicUsize,
        target_rev: String,
        prior_rev: String,
        block_height: u64,
        fail_rollback_health: bool,
    }

    impl UpgradeSystemHooks for MockHooks {
        fn stop_units(&self, _units: &[String]) -> Result<(), String> {
            Ok(())
        }
        fn start_units(&self, _units: &[String]) -> Result<(), String> {
            Ok(())
        }
        fn probe_node(
            &self,
            _host: &str,
            _port: u16,
            _timeout_ms: u64,
        ) -> Result<UpgradeNodeStatusProbe, String> {
            let call = self.probe_calls.fetch_add(1, Ordering::Relaxed);
            if call == 0 {
                return Ok(UpgradeNodeStatusProbe {
                    chain_id: "postfiat-testnet-1".to_string(),
                    genesis_hash: "aa".repeat(48),
                    block_height: self.block_height,
                    git_revision: Some(self.prior_rev.clone()),
                    signer_mode: Some("socket".to_string()),
                    signer_double_sign_protection: Some(true),
                });
            }
            if self.fail_post_upgrade.load(Ordering::Relaxed) {
                if self.fail_rollback_health {
                    return Err("node failed to restart".to_string());
                }
                // After rollback, let the final rollback health check pass.
                if call >= 3 {
                    return Ok(UpgradeNodeStatusProbe {
                        chain_id: "postfiat-testnet-1".to_string(),
                        genesis_hash: "aa".repeat(48),
                        block_height: self.block_height,
                        git_revision: Some(self.prior_rev.clone()),
                        signer_mode: Some("socket".to_string()),
                        signer_double_sign_protection: Some(true),
                    });
                }
                return Err("post-upgrade probe failed".to_string());
            }
            Ok(UpgradeNodeStatusProbe {
                chain_id: "postfiat-testnet-1".to_string(),
                genesis_hash: "aa".repeat(48),
                block_height: self.block_height + 1,
                git_revision: Some(self.target_rev.clone()),
                signer_mode: Some("socket".to_string()),
                signer_double_sign_protection: Some(true),
            })
        }
        fn free_disk_bytes(&self, _path: &Path) -> Result<u64, String> {
            Ok(self.free_bytes)
        }
    }

    fn build_signed_fixture(
        root: &Path,
        rollout: ReleaseRolloutV3,
        signers_count: usize,
    ) -> (PathBuf, PathBuf, PathBuf, PathBuf, PathBuf, PathBuf, String) {
        let mut keypairs = Vec::new();
        let mut signers = Vec::new();
        for (id, role) in [
            ("release-lead-1", "release_lead"),
            ("independent-builder-2", "independent_builder"),
            ("security-custodian-3", "security_custodian"),
        ] {
            let kp = ml_dsa_65_keygen().expect("keygen");
            let pk_hex = bytes_to_hex(&kp.public_key);
            let sk_hex = bytes_to_hex(&kp.private_key);
            let publisher = address_from_public_key(&kp.public_key);
            signers.push(ReleaseKeysetSignerV1 {
                signer_id: id.to_string(),
                role: role.to_string(),
                publisher: publisher.clone(),
                public_key_hex: pk_hex.clone(),
            });
            keypairs.push((id.to_string(), publisher, pk_hex, sk_hex));
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

        let candidate_bin = root.join("candidate-postfiat-node");
        fs::write(&candidate_bin, b"NEW_BINARY_BYTES_V3").unwrap();
        let (bin_sha, bin_size) = sha256_file_and_size(&candidate_bin).unwrap();

        let installed_bin = root.join("installed-postfiat-node");
        fs::write(&installed_bin, b"OLD_BINARY_BYTES_V2").unwrap();

        let data_dir = root.join("data");
        fs::create_dir_all(&data_dir).unwrap();
        fs::write(data_dir.join("ledger.dat"), b"CHAIN_STATE_V2").unwrap();

        let guard_dir = root.join("signer-guard");
        fs::create_dir_all(&guard_dir).unwrap();
        fs::write(
            guard_dir.join("guard.state"),
            b"MONOTONIC_GUARD_FLOOR_432000",
        )
        .unwrap();

        let backup_dir = root.join("backups");
        fs::create_dir_all(&backup_dir).unwrap();

        let target_rev = "11".repeat(20);
        let mut manifest = ReleaseManifestV3 {
            schema: RELEASE_MANIFEST_SCHEMA.to_string(),
            release_id: "v0.3.0-testnet".to_string(),
            created_unix: 1_795_651_200,
            valid_from_unix: 1_795_651_200,
            valid_until_unix: 1_803_427_200,
            chain_id: "postfiat-testnet-1".to_string(),
            genesis_hash: "aa".repeat(48),
            git_revision: target_rev.clone(),
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
                linux_x86_64: ReleaseBinaryArtifactV3 {
                    binary_url:
                        "https://releases.postfiat.org/v0.3.0-testnet/postfiat-node-linux-x86_64"
                            .to_string(),
                    binary_sha256: bin_sha.clone(),
                    binary_size_bytes: bin_size,
                },
                linux_aarch64: ReleaseBinaryArtifactV3 {
                    binary_url:
                        "https://releases.postfiat.org/v0.3.0-testnet/postfiat-node-linux-aarch64"
                            .to_string(),
                    binary_sha256: bin_sha,
                    binary_size_bytes: bin_size,
                },
                systemd_bundle_sha256: "22".repeat(32),
                docker_profiles_sha256: "33".repeat(32),
                sbom_sha256: "44".repeat(32),
                checksums_sha256: "55".repeat(32),
                swap_circuit_metadata_sha256: "66".repeat(32),
                private_egress_circuit_metadata_sha256: "77".repeat(32),
            },
            signing_policy: ReleaseSigningPolicyV3 {
                keyset_id: "postfiat-release-keyset-2026-v1".to_string(),
                threshold: 2,
                total_signers: 3,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                signing_context: RELEASE_MANIFEST_SIGNING_CONTEXT.to_string(),
            },
            signatures: Vec::new(),
        };
        for (id, publisher, pk, sk) in keypairs.iter().take(signers_count) {
            let sig = sign_release_manifest_v3(&manifest, id, publisher, pk, sk, 1_795_651_260)
                .expect("sign");
            manifest.signatures.push(sig);
        }
        let manifest_file = root.join("release-manifest.json");
        fs::write(
            &manifest_file,
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        (
            manifest_file,
            keyset_file,
            candidate_bin,
            installed_bin,
            data_dir,
            guard_dir,
            target_rev,
        )
    }

    #[test]
    fn upgrade_check_reports_exit_10_and_exit_11_and_refuses_single_signature() {
        let tmp = TestTempDir::new("check");
        let (manifest_file, keyset_file, candidate_bin, installed_bin, data_dir, guard_dir, rev) =
            build_signed_fixture(
                &tmp.0,
                ReleaseRolloutV3 {
                    mode: "activation_height".to_string(),
                    activation_height: Some(500_000),
                    advance_warning_blocks: Some(28_800),
                    halt_if_missed: true,
                },
                2,
            );
        let backup_dir = tmp.0.join("backups");

        // Inside activation warning window (500_000 - 480_000 = 20_000 <= 28_800) => exit 11
        let hooks = MockHooks {
            free_bytes: 10 * 1024 * 1024 * 1024,
            fail_post_upgrade: AtomicBool::new(false),
            probe_calls: AtomicUsize::new(0),
            target_rev: rev.clone(),
            prior_rev: "00".repeat(20),
            block_height: 480_000,
            fail_rollback_health: false,
        };
        let opts = UpgradeOptions {
            check_only: true,
            manifest_file: Some(manifest_file.clone()),
            channel_url: None,
            binary_file: Some(candidate_bin.clone()),
            keyset_file: keyset_file.clone(),
            data_dir: data_dir.clone(),
            signer_guard_dir: guard_dir.clone(),
            binary_path: installed_bin.clone(),
            backup_dir: backup_dir.clone(),
            service_units: Vec::new(),
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: 27650,
            health_timeout_ms: 500,
            now_unix: Some(1_795_651_500),
            architecture: Some("linux-x86_64".to_string()),
            yes: true,
            json: true,
            skip_rpc_preflight: false,
        };
        let report = execute_upgrade(&opts, &hooks);
        assert_eq!(
            report.exit_code, UPGRADE_EXIT_ACTIVATION_WARNING,
            "{report:?}"
        );
        assert_eq!(report.verified_signers.len(), 2);

        // Guard overlap must refuse with exit 3.
        let mut overlap = opts.clone();
        overlap.signer_guard_dir = data_dir.join("nested-guard");
        let report = execute_upgrade(&overlap, &hooks);
        assert_eq!(report.exit_code, UPGRADE_EXIT_SECURITY_REFUSAL);
        assert_eq!(report.error_code.as_deref(), Some("guard_path_overlap"));

        // Insufficient disk space must refuse with exit 4.
        let low_disk = MockHooks {
            free_bytes: 1024,
            fail_post_upgrade: AtomicBool::new(false),
            probe_calls: AtomicUsize::new(0),
            target_rev: rev,
            prior_rev: "00".repeat(20),
            block_height: 400_000,
            fail_rollback_health: false,
        };
        let report = execute_upgrade(&opts, &low_disk);
        assert_eq!(report.exit_code, UPGRADE_EXIT_PREFLIGHT_FAILED);
        assert_eq!(
            report.error_code.as_deref(),
            Some("insufficient_disk_space")
        );

        // Single signature (1-of-3) must be refused with exit 3.
        let tmp_bad = TestTempDir::new("one-sig");
        let (m1, k1, b1, inst1, d1, g1, _) = build_signed_fixture(
            &tmp_bad.0,
            ReleaseRolloutV3 {
                mode: "rolling".to_string(),
                activation_height: None,
                advance_warning_blocks: None,
                halt_if_missed: false,
            },
            1,
        );
        let bad_opts = UpgradeOptions {
            manifest_file: Some(m1),
            keyset_file: k1,
            binary_file: Some(b1),
            binary_path: inst1,
            data_dir: d1,
            signer_guard_dir: g1,
            backup_dir: tmp_bad.0.join("backups"),
            ..opts
        };
        let report = execute_upgrade(&bad_opts, &hooks);
        assert_eq!(report.exit_code, UPGRADE_EXIT_SECURITY_REFUSAL);
    }

    #[test]
    fn upgrade_apply_and_rollback_preserve_signer_guard_untouched() {
        let tmp = TestTempDir::new("apply-rollback");
        let (manifest_file, keyset_file, candidate_bin, installed_bin, data_dir, guard_dir, rev) =
            build_signed_fixture(
                &tmp.0,
                ReleaseRolloutV3 {
                    mode: "rolling".to_string(),
                    activation_height: None,
                    advance_warning_blocks: None,
                    halt_if_missed: false,
                },
                2,
            );
        let backup_dir = tmp.0.join("backups");
        let guard_file = guard_dir.join("guard.state");

        // 1. Failing post-upgrade health triggers atomic rollback (exit 5), guard untouched.
        let failing_hooks = MockHooks {
            free_bytes: 10 * 1024 * 1024 * 1024,
            fail_post_upgrade: AtomicBool::new(true),
            probe_calls: AtomicUsize::new(0),
            target_rev: rev.clone(),
            prior_rev: "00".repeat(20),
            block_height: 432_000,
            fail_rollback_health: false,
        };
        let opts = UpgradeOptions {
            check_only: false,
            manifest_file: Some(manifest_file.clone()),
            channel_url: None,
            binary_file: Some(candidate_bin.clone()),
            keyset_file: keyset_file.clone(),
            data_dir: data_dir.clone(),
            signer_guard_dir: guard_dir.clone(),
            binary_path: installed_bin.clone(),
            backup_dir: backup_dir.clone(),
            service_units: Vec::new(),
            rpc_host: "127.0.0.1".to_string(),
            rpc_port: 27650,
            health_timeout_ms: 250,
            now_unix: Some(1_795_651_600),
            architecture: Some("linux-x86_64".to_string()),
            yes: true,
            json: true,
            skip_rpc_preflight: false,
        };
        let rb_report = execute_upgrade(&opts, &failing_hooks);
        assert_eq!(
            rb_report.exit_code, UPGRADE_EXIT_ROLLED_BACK,
            "{rb_report:?}"
        );
        assert!(rb_report.rolled_back);
        assert_eq!(fs::read(&installed_bin).unwrap(), b"OLD_BINARY_BYTES_V2");
        assert_eq!(
            fs::read(&guard_file).unwrap(),
            b"MONOTONIC_GUARD_FLOOR_432000"
        );

        // 2. Healthy apply swaps binary atomically (exit 0), guard still untouched.
        let ok_hooks = MockHooks {
            free_bytes: 10 * 1024 * 1024 * 1024,
            fail_post_upgrade: AtomicBool::new(false),
            probe_calls: AtomicUsize::new(0),
            target_rev: rev,
            prior_rev: "00".repeat(20),
            block_height: 432_000,
            fail_rollback_health: false,
        };
        let mut opts2 = opts;
        opts2.now_unix = Some(1_795_651_700);
        let ok_report = execute_upgrade(&opts2, &ok_hooks);
        assert_eq!(ok_report.exit_code, UPGRADE_EXIT_OK, "{ok_report:?}");
        assert_eq!(fs::read(&installed_bin).unwrap(), b"NEW_BINARY_BYTES_V3");
        assert_eq!(
            fs::read(&guard_file).unwrap(),
            b"MONOTONIC_GUARD_FLOOR_432000"
        );
    }
}
