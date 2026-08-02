//! Validator-local source-checkpoint voting.
//!
//! This module deliberately does not expose an arbitrary checkpoint signing
//! command. Source adapters call it only after independently reproducing the
//! governed source state from a validator-selected RPC endpoint.

use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

#[cfg(unix)]
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};

use anyhow::{Context, Result};
use clap::Args;
use postfiat_crypto_provider::{
    ml_dsa_65_sign_with_context, ml_dsa_65_verify_with_context, ML_DSA_65_ALGORITHM,
};
use reserve_proof_types::bft_checkpoint::{
    BftCheckpointCommitteeV1, BftSourceCheckpointV1, BftSourceCheckpointVoteV1,
    BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use zeroize::Zeroizing;

use crate::{read_json, write_new};

const SIGNING_STATE_SCHEMA_V1: &str = "postfiat.reserve_source_checkpoint_signing_state.v1";
const MAX_VALIDATOR_KEY_FILE_BYTES: u64 = 512 * 1024;
static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Optional atomic vote output for a source adapter's checkpoint operation.
/// Either all four fields are present or all are absent.
#[derive(Clone, Debug, Default, Args)]
pub struct CheckpointSigningArgs {
    /// Validator ID in the governed source-checkpoint committee.
    #[arg(long, requires_all = ["validator_key_file", "signing_state_dir", "vote_output"])]
    pub validator_id: Option<String>,
    /// Permission-restricted PFTL validator key file. It is read locally only.
    #[arg(long, requires_all = ["validator_id", "signing_state_dir", "vote_output"])]
    pub validator_key_file: Option<PathBuf>,
    /// Durable validator-local directory used to reject equivocation.
    #[arg(long, requires_all = ["validator_id", "validator_key_file", "vote_output"])]
    pub signing_state_dir: Option<PathBuf>,
    /// New public vote file. Existing files are never overwritten.
    #[arg(long, requires_all = ["validator_id", "validator_key_file", "signing_state_dir"])]
    pub vote_output: Option<PathBuf>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatorKeyFile {
    validators: Vec<ValidatorKeyRecord>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ValidatorKeyRecord {
    node_id: String,
    algorithm_id: String,
    public_key_hex: String,
    private_key_hex: Zeroizing<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SigningStateV1 {
    schema: String,
    validator_id: String,
    checkpoint: BftSourceCheckpointV1,
    vote: Option<BftSourceCheckpointVoteV1>,
}

pub(crate) fn maybe_sign_reproduced_checkpoint(
    checkpoint: &BftSourceCheckpointV1,
    committee: &BftCheckpointCommitteeV1,
    args: &CheckpointSigningArgs,
) -> Result<()> {
    let Some(validator_id) = args.validator_id.as_deref() else {
        anyhow::ensure!(
            args.validator_key_file.is_none()
                && args.signing_state_dir.is_none()
                && args.vote_output.is_none(),
            "checkpoint signing options must be supplied together"
        );
        return Ok(());
    };
    let key_path = args
        .validator_key_file
        .as_deref()
        .context("validator key file is required for checkpoint signing")?;
    let state_dir = args
        .signing_state_dir
        .as_deref()
        .context("signing state directory is required for checkpoint signing")?;
    let vote_output = args
        .vote_output
        .as_deref()
        .context("vote output is required for checkpoint signing")?;

    committee.validate().map_err(anyhow::Error::msg)?;
    let committee_root = committee.root().map_err(anyhow::Error::msg)?;
    anyhow::ensure!(
        checkpoint.committee_epoch == committee.epoch
            && checkpoint.committee_root == committee_root,
        "reproduced checkpoint is not bound to the governed committee"
    );
    checkpoint.canonical_bytes().map_err(anyhow::Error::msg)?;
    let validator = committee
        .validators
        .iter()
        .find(|candidate| candidate.validator_id == validator_id)
        .context("checkpoint signer is not in the governed committee")?;

    validate_private_key_file_permissions(key_path)?;
    let metadata = fs::metadata(key_path)
        .with_context(|| format!("stat validator key file {}", key_path.display()))?;
    anyhow::ensure!(
        metadata.is_file(),
        "validator key path is not a regular file"
    );
    anyhow::ensure!(
        metadata.len() <= MAX_VALIDATOR_KEY_FILE_BYTES,
        "validator key file exceeds {MAX_VALIDATOR_KEY_FILE_BYTES} bytes"
    );
    let key_file_bytes = Zeroizing::new(
        fs::read(key_path)
            .with_context(|| format!("read validator key file {}", key_path.display()))?,
    );
    let key_file: ValidatorKeyFile = serde_json::from_slice(&key_file_bytes)
        .with_context(|| format!("parse validator key file {}", key_path.display()))?;
    let mut matching = key_file
        .validators
        .iter()
        .filter(|record| record.node_id == validator_id);
    let key = matching.next().context("validator key record is missing")?;
    anyhow::ensure!(matching.next().is_none(), "duplicate validator key record");
    anyhow::ensure!(
        key.algorithm_id == ML_DSA_65_ALGORITHM,
        "validator key algorithm is not ML-DSA-65"
    );
    let public_key = decode_lower_hex("validator public key", &key.public_key_hex)?;
    anyhow::ensure!(
        public_key == validator.public_key,
        "validator key does not match the governed committee"
    );

    let state_path = signing_state_path(state_dir, checkpoint, validator_id)?;
    let intended = SigningStateV1 {
        schema: SIGNING_STATE_SCHEMA_V1.to_string(),
        validator_id: validator_id.to_string(),
        checkpoint: checkpoint.clone(),
        vote: None,
    };
    let mut state = load_or_create_state(&state_path, &intended)?;
    validate_state(&state, checkpoint, validator_id, &public_key)?;
    let vote = if let Some(vote) = state.vote.clone() {
        vote
    } else {
        let statement = checkpoint
            .vote_signing_statement(validator_id)
            .map_err(anyhow::Error::msg)?;
        let private_key = Zeroizing::new(decode_lower_hex(
            "validator private key",
            key.private_key_hex.as_str(),
        )?);
        let signature = ml_dsa_65_sign_with_context(
            &private_key,
            &statement,
            BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
        )
        .map_err(anyhow::Error::msg)?;
        let vote = BftSourceCheckpointVoteV1 {
            validator_id: validator_id.to_string(),
            signature,
        };
        anyhow::ensure!(
            ml_dsa_65_verify_with_context(
                &public_key,
                &statement,
                &vote.signature,
                BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
            ),
            "new checkpoint vote failed local verification"
        );
        state.vote = Some(vote.clone());
        write_private_json_replace(&state_path, &state)?;
        vote
    };
    write_new(vote_output, &serde_json::to_vec_pretty(&vote)?)?;
    Ok(())
}

fn signing_state_path(
    state_dir: &Path,
    checkpoint: &BftSourceCheckpointV1,
    validator_id: &str,
) -> Result<PathBuf> {
    let mut hasher = Sha256::new();
    hasher.update(b"postfiat.reserve-source-checkpoint.signing-slot.v1");
    hasher.update((checkpoint.pftl_genesis_hash.len() as u64).to_be_bytes());
    hasher.update(checkpoint.pftl_genesis_hash.as_bytes());
    hasher.update((checkpoint.checkpoint_kind.len() as u64).to_be_bytes());
    hasher.update(checkpoint.checkpoint_kind.as_bytes());
    hasher.update((checkpoint.source_domain.len() as u64).to_be_bytes());
    hasher.update(checkpoint.source_domain.as_bytes());
    hasher.update(checkpoint.committee_epoch.to_be_bytes());
    hasher.update(checkpoint.source_height.to_be_bytes());
    hasher.update((validator_id.len() as u64).to_be_bytes());
    hasher.update(validator_id.as_bytes());
    Ok(state_dir.join(format!("{}.json", hex::encode(hasher.finalize()))))
}

fn load_or_create_state(path: &Path, intended: &SigningStateV1) -> Result<SigningStateV1> {
    if path.exists() {
        return read_json(path);
    }
    let mut bytes = serde_json::to_vec_pretty(intended)?;
    bytes.push(b'\n');
    match create_once(path, &bytes) {
        Ok(()) => Ok(intended.clone()),
        Err(error)
            if error
                .downcast_ref::<std::io::Error>()
                .is_some_and(|io| io.kind() == std::io::ErrorKind::AlreadyExists) =>
        {
            read_json(path)
        }
        Err(error) => Err(error),
    }
}

fn validate_state(
    state: &SigningStateV1,
    checkpoint: &BftSourceCheckpointV1,
    validator_id: &str,
    public_key: &[u8],
) -> Result<()> {
    anyhow::ensure!(
        state.schema == SIGNING_STATE_SCHEMA_V1 && state.validator_id == validator_id,
        "checkpoint signing state has the wrong domain"
    );
    anyhow::ensure!(
        state.checkpoint == *checkpoint,
        "validator already recorded a conflicting checkpoint at this source, epoch, and height"
    );
    if let Some(vote) = &state.vote {
        let statement = checkpoint
            .vote_signing_statement(validator_id)
            .map_err(anyhow::Error::msg)?;
        anyhow::ensure!(
            vote.validator_id == validator_id
                && ml_dsa_65_verify_with_context(
                    public_key,
                    &statement,
                    &vote.signature,
                    BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
                ),
            "persisted checkpoint vote does not verify"
        );
    }
    Ok(())
}

fn create_once(path: &Path, contents: &[u8]) -> Result<()> {
    let parent = path.parent().context("signing state path has no parent")?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(
        ".source-checkpoint-{}-{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temp)?;
    if let Err(error) = file.write_all(contents).and_then(|()| file.sync_all()) {
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    drop(file);
    match fs::hard_link(&temp, path) {
        Ok(()) => {
            fs::remove_file(&temp)?;
            set_private_permissions(path)?;
            sync_directory(parent)?;
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error.into())
        }
    }
}

fn write_private_json_replace(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("signing state path has no parent")?;
    let temp = parent.join(format!(
        ".source-checkpoint-vote-{}-{}.tmp",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut file = options.open(&temp)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    drop(file);
    fs::rename(&temp, path)?;
    set_private_permissions(path)?;
    sync_directory(parent)?;
    Ok(())
}

fn validate_private_key_file_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    {
        let mode = fs::metadata(path)?.permissions().mode() & 0o777;
        anyhow::ensure!(
            mode & 0o077 == 0,
            "validator key file must not be accessible by group or other users"
        );
    }
    Ok(())
}

fn set_private_permissions(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::set_permissions(path, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn sync_directory(path: &Path) -> Result<()> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    Ok(())
}

fn decode_lower_hex(label: &str, value: &str) -> Result<Vec<u8>> {
    anyhow::ensure!(
        value.len() % 2 == 0
            && value
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)),
        "{label} is not canonical lowercase hex"
    );
    hex::decode(value).with_context(|| format!("decode {label}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use postfiat_crypto_provider::ml_dsa_65_keygen_from_seed;

    fn test_dir(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "postfiat-reserve-checkpoint-{label}-{}-{}",
            std::process::id(),
            TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
        ))
    }

    #[test]
    fn signs_only_governed_key_and_rejects_same_slot_equivocation() {
        let root = test_dir("vote");
        fs::create_dir_all(&root).unwrap();
        let key = ml_dsa_65_keygen_from_seed(&[7; 32]);
        let key_path = root.join("validator-key.json");
        fs::write(
            &key_path,
            serde_json::to_vec(&serde_json::json!({
                "validators": [{
                    "node_id": "validator-0",
                    "algorithm_id": ML_DSA_65_ALGORITHM,
                    "public_key_hex": hex::encode(&key.public_key),
                    "private_key_hex": hex::encode(&key.private_key),
                }]
            }))
            .unwrap(),
        )
        .unwrap();
        #[cfg(unix)]
        fs::set_permissions(&key_path, fs::Permissions::from_mode(0o600)).unwrap();

        let committee = BftCheckpointCommitteeV1 {
            epoch: 4,
            quorum: 1,
            validators: vec![
                reserve_proof_types::bft_checkpoint::BftCheckpointValidatorV1 {
                    validator_id: "validator-0".to_string(),
                    public_key: key.public_key.clone(),
                },
            ],
        };
        let checkpoint = BftSourceCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            checkpoint_kind: "evm-state-root-v1".to_string(),
            source_domain: "ethereum:1".to_string(),
            source_height: 100,
            source_timestamp_ms: 1_700_000_000_000,
            source_block_hash: B256::repeat_byte(0x22),
            source_state_commitment: B256::repeat_byte(0x33),
            observed_source_head: 112,
            minimum_depth: 12,
            pftl_observation_height: 200,
            committee_epoch: committee.epoch,
            committee_root: committee.root().unwrap(),
        };
        let args = |vote: &str| CheckpointSigningArgs {
            validator_id: Some("validator-0".to_string()),
            validator_key_file: Some(key_path.clone()),
            signing_state_dir: Some(root.join("state")),
            vote_output: Some(root.join(vote)),
        };
        maybe_sign_reproduced_checkpoint(&checkpoint, &committee, &args("vote-1.json")).unwrap();
        maybe_sign_reproduced_checkpoint(&checkpoint, &committee, &args("vote-2.json")).unwrap();
        let vote_one: BftSourceCheckpointVoteV1 = read_json(&root.join("vote-1.json")).unwrap();
        let vote_two: BftSourceCheckpointVoteV1 = read_json(&root.join("vote-2.json")).unwrap();
        assert_eq!(vote_one, vote_two);

        let mut conflicting = checkpoint;
        conflicting.source_block_hash = B256::repeat_byte(0x44);
        let error = maybe_sign_reproduced_checkpoint(
            &conflicting,
            &committee,
            &args("conflicting-vote.json"),
        )
        .unwrap_err();
        assert!(error.to_string().contains("conflicting checkpoint"));
        assert!(!root.join("conflicting-vote.json").exists());
        fs::remove_dir_all(root).unwrap();
    }
}
