use postfiat_crypto_provider::ml_dsa_65_verify_with_context;
use postfiat_types::{
    EthereumCheckpointCertificateV1, EthereumFinalizedCheckpointV1, FastSwapCodecError,
    FastSwapCommitteeV1, FastSwapOpaqueHashV1, ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum EthereumCheckpointVerificationError {
    Codec(FastSwapCodecError),
    CommitteeMismatch,
    BelowQuorum,
    UnknownValidator,
    InvalidVoteSignature,
}

impl From<FastSwapCodecError> for EthereumCheckpointVerificationError {
    fn from(value: FastSwapCodecError) -> Self {
        Self::Codec(value)
    }
}

pub fn verify_ethereum_checkpoint_certificate<'a>(
    committee: &FastSwapCommitteeV1,
    certificate: &'a EthereumCheckpointCertificateV1,
) -> Result<
    (&'a EthereumFinalizedCheckpointV1, FastSwapOpaqueHashV1),
    EthereumCheckpointVerificationError,
> {
    committee.validate()?;
    certificate.validate_canonical_order()?;
    let checkpoint = &certificate.checkpoint;
    if checkpoint.pftl_domain != committee.domain.chain
        || checkpoint.authority_epoch != committee.domain.committee_epoch
        || checkpoint.committee_root != committee.domain.committee_root
    {
        return Err(EthereumCheckpointVerificationError::CommitteeMismatch);
    }
    if certificate.votes.len() < usize::from(committee.domain.quorum) {
        return Err(EthereumCheckpointVerificationError::BelowQuorum);
    }
    for vote in &certificate.votes {
        let validator = committee
            .validators
            .iter()
            .find(|validator| validator.validator_id == vote.validator_id)
            .ok_or(EthereumCheckpointVerificationError::UnknownValidator)?;
        if !ml_dsa_65_verify_with_context(
            &validator.public_key,
            &vote.signing_bytes(checkpoint)?,
            &vote.signature,
            ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
        ) {
            return Err(EthereumCheckpointVerificationError::InvalidVoteSignature);
        }
    }
    Ok((checkpoint, checkpoint.digest()?))
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_crypto_provider::{
        ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context_seed, MlDsa65KeyPair,
    };
    use postfiat_types::{
        EthereumCheckpointVoteV1, FastSwapChainDomainV1, FastSwapCommitteeDomainV1,
        FastSwapCommitteeRootV1, FastSwapValidatorV1, FASTSWAP_SCHEMA_VERSION_V1,
    };

    fn committee() -> (FastSwapCommitteeV1, Vec<MlDsa65KeyPair>) {
        let keys = (0_u8..4)
            .map(|index| ml_dsa_65_keygen_from_seed(&[index + 1; 32]))
            .collect::<Vec<_>>();
        let validators = keys
            .iter()
            .enumerate()
            .map(|(index, key)| FastSwapValidatorV1 {
                validator_id: format!("validator-{index}"),
                public_key: key.public_key.clone(),
            })
            .collect::<Vec<_>>();
        let mut committee = FastSwapCommitteeV1 {
            domain: FastSwapCommitteeDomainV1 {
                chain: FastSwapChainDomainV1 {
                    chain_id: "postfiat-checkpoint-test".to_string(),
                    genesis_hash: FastSwapOpaqueHashV1([0x11; 48]),
                    protocol_version: 1,
                },
                fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
                committee_epoch: 7,
                committee_root: FastSwapCommitteeRootV1::ZERO,
                validator_count: 4,
                quorum: 3,
            },
            validators,
        };
        committee.domain.committee_root = committee.computed_root().expect("committee root");
        (committee, keys)
    }

    fn checkpoint(committee: &FastSwapCommitteeV1) -> EthereumFinalizedCheckpointV1 {
        EthereumFinalizedCheckpointV1 {
            schema_version: postfiat_types::ETHEREUM_CHECKPOINT_SCHEMA_V1,
            pftl_domain: committee.domain.chain.clone(),
            route_id: "pftl-uniswap-a651".to_string(),
            route_config_digest: FastSwapOpaqueHashV1([0x22; 48]),
            ethereum_chain_id: 1,
            block_number: 100,
            block_hash: [0x33; 32],
            receipts_root: [0x44; 32],
            observed_head_number: 112,
            minimum_confirmations: 12,
            authority_epoch: committee.domain.committee_epoch,
            committee_root: committee.domain.committee_root,
            handoff_controller: [0x55; 20],
            wrapped_navcoin_token: [0x66; 20],
            handoff_controller_code_hash: [0x77; 32],
            wrapped_navcoin_code_hash: [0x88; 32],
        }
    }

    fn signed_certificate(
        committee: &FastSwapCommitteeV1,
        keys: &[MlDsa65KeyPair],
        signer_count: usize,
    ) -> EthereumCheckpointCertificateV1 {
        signed_certificate_for_checkpoint(keys, checkpoint(committee), signer_count)
    }

    fn signed_certificate_for_checkpoint(
        keys: &[MlDsa65KeyPair],
        checkpoint: EthereumFinalizedCheckpointV1,
        signer_count: usize,
    ) -> EthereumCheckpointCertificateV1 {
        let votes = keys
            .iter()
            .enumerate()
            .take(signer_count)
            .map(|(index, key)| {
                let mut vote = EthereumCheckpointVoteV1 {
                    validator_id: format!("validator-{index}"),
                    signature: vec![1],
                };
                vote.signature = ml_dsa_65_sign_with_context_seed(
                    &key.private_key,
                    &vote.signing_bytes(&checkpoint).expect("vote bytes"),
                    ETHEREUM_CHECKPOINT_VOTE_CONTEXT_V1,
                    &[0x90 + u8::try_from(index).expect("test index"); 32],
                )
                .expect("checkpoint vote");
                vote
            })
            .collect();
        EthereumCheckpointCertificateV1 { checkpoint, votes }
    }

    #[test]
    fn verifies_distinct_exact_committee_quorum_and_field_binding() {
        let (committee, keys) = committee();
        let certificate = signed_certificate(&committee, &keys, 3);

        let (verified, digest) =
            verify_ethereum_checkpoint_certificate(&committee, &certificate).expect("certificate");

        assert_eq!(verified, &certificate.checkpoint);
        assert_eq!(digest, certificate.checkpoint.digest().expect("digest"));

        let under_quorum = signed_certificate(&committee, &keys, 2);
        assert_eq!(
            verify_ethereum_checkpoint_certificate(&committee, &under_quorum),
            Err(EthereumCheckpointVerificationError::BelowQuorum)
        );

        let mut duplicate = certificate.clone();
        duplicate.votes[1].validator_id = duplicate.votes[0].validator_id.clone();
        assert!(matches!(
            verify_ethereum_checkpoint_certificate(&committee, &duplicate),
            Err(EthereumCheckpointVerificationError::Codec(
                FastSwapCodecError::NonCanonical("ethereum checkpoint vote order")
            ))
        ));

        let mut altered = certificate.clone();
        altered.checkpoint.receipts_root[0] ^= 1;
        assert_eq!(
            verify_ethereum_checkpoint_certificate(&committee, &altered),
            Err(EthereumCheckpointVerificationError::InvalidVoteSignature)
        );

        let mut wrong_epoch = certificate;
        wrong_epoch.checkpoint.authority_epoch += 1;
        assert_eq!(
            verify_ethereum_checkpoint_certificate(&committee, &wrong_epoch),
            Err(EthereumCheckpointVerificationError::CommitteeMismatch)
        );
    }

    #[test]
    fn rejects_partitioned_alternate_checkpoint_and_unvoted_reorg() {
        let (committee, keys) = committee();
        let canonical = checkpoint(&committee);

        let mut partitioned_alternate = canonical.clone();
        partitioned_alternate.block_hash[0] ^= 1;
        partitioned_alternate.receipts_root[0] ^= 1;
        let minority_certificate =
            signed_certificate_for_checkpoint(&keys, partitioned_alternate, 2);
        assert_eq!(
            verify_ethereum_checkpoint_certificate(&committee, &minority_certificate),
            Err(EthereumCheckpointVerificationError::BelowQuorum)
        );

        let mut unvoted_reorg = signed_certificate_for_checkpoint(&keys, canonical, 3);
        unvoted_reorg.checkpoint.block_hash[0] ^= 1;
        unvoted_reorg.checkpoint.receipts_root[0] ^= 1;
        assert_eq!(
            verify_ethereum_checkpoint_certificate(&committee, &unvoted_reorg),
            Err(EthereumCheckpointVerificationError::InvalidVoteSignature)
        );
    }
}
