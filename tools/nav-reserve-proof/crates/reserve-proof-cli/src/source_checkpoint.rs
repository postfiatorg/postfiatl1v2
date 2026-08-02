use std::{fs, path::PathBuf};

use anyhow::{Context, Result};
use clap::Subcommand;
use reserve_proof_types::bft_checkpoint::{
    BftCheckpointCommitteeV1, BftSourceCheckpointCertificateV1, BftSourceCheckpointV1,
    BftSourceCheckpointVoteV1, MAX_BFT_CHECKPOINT_VALIDATORS,
};
use reserve_proof_types::MAX_WITNESS_BYTES;

use crate::{read_json, write_new};

#[derive(Debug, Subcommand)]
pub enum SourceCheckpointCommand {
    /// Emit the exact bytes one governed validator signs with the public
    /// source-checkpoint ML-DSA context.
    VoteStatement {
        #[arg(long)]
        checkpoint: PathBuf,
        #[arg(long)]
        validator_id: String,
        #[arg(long)]
        output: PathBuf,
    },
    /// Assemble independently produced vote files into a verified certificate.
    Assemble {
        #[arg(long)]
        committee: PathBuf,
        #[arg(long)]
        checkpoint: PathBuf,
        #[arg(long, required = true, num_args = 1..)]
        vote: Vec<PathBuf>,
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify a complete checkpoint certificate without writing state.
    Validate {
        #[arg(long)]
        certificate: PathBuf,
    },
}

pub fn run(command: SourceCheckpointCommand) -> Result<()> {
    match command {
        SourceCheckpointCommand::VoteStatement {
            checkpoint,
            validator_id,
            output,
        } => vote_statement(checkpoint, validator_id, output),
        SourceCheckpointCommand::Assemble {
            committee,
            checkpoint,
            vote,
            output,
        } => assemble(committee, checkpoint, vote, output),
        SourceCheckpointCommand::Validate { certificate } => validate(certificate),
    }
}

fn vote_statement(checkpoint_path: PathBuf, validator_id: String, output: PathBuf) -> Result<()> {
    let checkpoint: BftSourceCheckpointV1 = read_json(&checkpoint_path)?;
    let statement = checkpoint
        .vote_signing_statement(&validator_id)
        .map_err(anyhow::Error::msg)?;
    write_new(&output, &statement)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_source_checkpoint_vote_statement.v1",
            "checkpoint": checkpoint_path,
            "validator_id": validator_id,
            "output": output,
            "statement_bytes": statement.len(),
            "statement_hex": hex::encode(statement),
        }))?
    );
    Ok(())
}

fn assemble(
    committee_path: PathBuf,
    checkpoint_path: PathBuf,
    vote_paths: Vec<PathBuf>,
    output: PathBuf,
) -> Result<()> {
    anyhow::ensure!(
        vote_paths.len() <= MAX_BFT_CHECKPOINT_VALIDATORS,
        "vote file count exceeds {MAX_BFT_CHECKPOINT_VALIDATORS}"
    );
    let committee: BftCheckpointCommitteeV1 = read_json(&committee_path)?;
    let checkpoint: BftSourceCheckpointV1 = read_json(&checkpoint_path)?;
    let mut votes = vote_paths
        .iter()
        .map(|path| {
            let metadata = fs::metadata(path)
                .with_context(|| format!("stat checkpoint vote {}", path.display()))?;
            anyhow::ensure!(
                metadata.is_file(),
                "checkpoint vote is not a regular file: {}",
                path.display()
            );
            read_json::<BftSourceCheckpointVoteV1>(path)
        })
        .collect::<Result<Vec<_>>>()?;
    votes.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
    let certificate = build_certificate(committee, checkpoint, votes)?;
    write_new(&output, &serde_json::to_vec_pretty(&certificate)?)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_source_checkpoint_certificate_assembly.v1",
            "valid": true,
            "output": output,
            "checkpoint_kind": certificate.checkpoint.checkpoint_kind,
            "source_domain": certificate.checkpoint.source_domain,
            "source_height": certificate.checkpoint.source_height,
            "committee_epoch": certificate.committee.epoch,
            "committee_root": certificate.checkpoint.committee_root,
            "vote_count": certificate.votes.len(),
            "quorum": certificate.committee.quorum,
        }))?
    );
    Ok(())
}

fn validate(certificate_path: PathBuf) -> Result<()> {
    let certificate: BftSourceCheckpointCertificateV1 = read_json(&certificate_path)?;
    certificate.verify().map_err(anyhow::Error::msg)?;
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "schema": "postfiat.reserve_source_checkpoint_certificate_validation.v1",
            "valid": true,
            "certificate": certificate_path,
            "checkpoint_kind": certificate.checkpoint.checkpoint_kind,
            "source_domain": certificate.checkpoint.source_domain,
            "source_height": certificate.checkpoint.source_height,
            "committee_epoch": certificate.committee.epoch,
            "committee_root": certificate.checkpoint.committee_root,
            "vote_count": certificate.votes.len(),
            "quorum": certificate.committee.quorum,
        }))?
    );
    Ok(())
}

fn build_certificate(
    committee: BftCheckpointCommitteeV1,
    checkpoint: BftSourceCheckpointV1,
    votes: Vec<BftSourceCheckpointVoteV1>,
) -> Result<BftSourceCheckpointCertificateV1> {
    let certificate = BftSourceCheckpointCertificateV1 {
        committee,
        checkpoint,
        votes,
    };
    certificate.verify().map_err(anyhow::Error::msg)?;
    Ok(certificate)
}

pub(crate) fn fuzz_external_input(data: &[u8]) {
    if data.len() > MAX_WITNESS_BYTES {
        return;
    }
    if let Ok(committee) = serde_json::from_slice::<BftCheckpointCommitteeV1>(data) {
        let _ = committee.validate();
        let _ = committee.root();
    }
    if let Ok(checkpoint) = serde_json::from_slice::<BftSourceCheckpointV1>(data) {
        let _ = checkpoint.canonical_bytes();
        let _ = checkpoint.vote_signing_statement("fuzz-validator");
    }
    if let Ok(certificate) = serde_json::from_slice::<BftSourceCheckpointCertificateV1>(data) {
        let _ = certificate.verify();
    }
    let _ = serde_json::from_slice::<BftSourceCheckpointVoteV1>(data);
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloy_primitives::B256;
    use postfiat_crypto_provider::{ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed};
    use reserve_proof_types::bft_checkpoint::{
        BftCheckpointValidatorV1, BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
    };

    fn fixture() -> (
        BftCheckpointCommitteeV1,
        BftSourceCheckpointV1,
        Vec<BftSourceCheckpointVoteV1>,
    ) {
        let keys = (0u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 1; 32]))
            .collect::<Vec<_>>();
        let committee = BftCheckpointCommitteeV1 {
            epoch: 12,
            quorum: 3,
            validators: keys
                .iter()
                .enumerate()
                .map(|(index, key)| BftCheckpointValidatorV1 {
                    validator_id: format!("validator-{index}"),
                    public_key: key.public_key.clone(),
                })
                .collect(),
        };
        let checkpoint = BftSourceCheckpointV1 {
            pftl_genesis_hash: "11".repeat(48),
            checkpoint_kind: "external-head-v1".to_string(),
            source_domain: "example:mainnet".to_string(),
            source_height: 1_000,
            source_timestamp_ms: 1_785_000_000_000,
            source_block_hash: B256::repeat_byte(0x22),
            source_state_commitment: B256::repeat_byte(0x33),
            observed_source_head: 1_012,
            minimum_depth: 12,
            pftl_observation_height: 500,
            committee_epoch: committee.epoch,
            committee_root: committee.root().unwrap(),
        };
        let votes = keys
            .iter()
            .take(3)
            .enumerate()
            .map(|(index, key)| {
                let validator_id = format!("validator-{index}");
                let statement = checkpoint.vote_signing_statement(&validator_id).unwrap();
                BftSourceCheckpointVoteV1 {
                    validator_id,
                    signature: ml_dsa_65_sign_with_context_seed(
                        &key.private_key,
                        &statement,
                        BFT_SOURCE_CHECKPOINT_SIGNATURE_CONTEXT_V1,
                        &[0x70 + index as u8; 32],
                    )
                    .unwrap(),
                }
            })
            .collect();
        (committee, checkpoint, votes)
    }

    #[test]
    fn assembly_sorts_and_verifies_independent_votes() {
        let (committee, checkpoint, mut votes) = fixture();
        votes.reverse();
        votes.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
        let certificate = build_certificate(committee, checkpoint, votes).unwrap();
        assert_eq!(certificate.votes.len(), 3);
        assert!(certificate.verify().is_ok());
    }

    #[test]
    fn assembly_rejects_duplicate_invalid_and_foreign_votes() {
        let (committee, checkpoint, votes) = fixture();
        let mut duplicate = votes.clone();
        duplicate[1] = duplicate[0].clone();
        assert!(build_certificate(committee.clone(), checkpoint.clone(), duplicate).is_err());

        let mut invalid = votes.clone();
        invalid[0].signature[0] ^= 1;
        assert!(build_certificate(committee.clone(), checkpoint.clone(), invalid).is_err());

        let mut foreign = votes;
        foreign[0].validator_id = "validator-9".to_string();
        foreign.sort_by(|left, right| left.validator_id.cmp(&right.validator_id));
        assert!(build_certificate(committee, checkpoint, foreign).is_err());
    }
}
