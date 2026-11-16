// Local-only synthetic signer fixture; adapted from pfusdc_proofs tests.
fn committee() -> (ConsensusV2ValidatorSet, Vec<MlDsa65KeyPair>) {
    let keys = (0..6)
        .map(|index| ml_dsa_65_keygen_from_seed(&[index as u8 + 1; 32]))
        .collect::<Vec<_>>();
    let validators = keys
        .iter()
        .enumerate()
        .map(|(index, key)| ConsensusV2Validator {
            validator_id: format!("validator-{index}"),
            public_key_hex: bytes_to_hex(&key.public_key),
        })
        .collect();
    (
        ConsensusV2ValidatorSet::try_new(validators).expect("committee"),
        keys,
    )
}

fn empty_signature(validator: &ConsensusV2Validator) -> ConsensusV2Signature {
    ConsensusV2Signature {
        algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
        signer: validator.validator_id.clone(),
        public_key_hex: validator.public_key_hex.clone(),
        signature_hex: "00".to_string(),
    }
}

fn signed_proposal(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    keys: &[MlDsa65KeyPair],
    round: ConsensusV2Round,
    block: ConsensusV2BlockRef,
) -> ConsensusV2Proposal {
    let proposer =
        leader_for_view(&validators.validator_ids(), round.height, round.view).expect("leader");
    let index = validators
        .validators
        .iter()
        .position(|validator| validator.validator_id == proposer)
        .expect("proposer index");
    let mut proposal = ConsensusV2Proposal {
        schema: CONSENSUS_V2_PROPOSAL_SCHEMA.to_string(),
        domain: domain.clone(),
        round,
        block,
        valid_qc: None,
        timeout_certificate_id: None,
        proposer,
        signature: empty_signature(&validators.validators[index]),
    };
    let message = consensus_v2_proposal_signing_bytes(&proposal).expect("proposal bytes");
    proposal.signature.signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &keys[index].private_key,
            &message,
            CONSENSUS_V2_PROPOSAL_CONTEXT,
        )
        .expect("proposal signature"),
    );
    proposal
}

fn signed_votes(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    keys: &[MlDsa65KeyPair],
    round: ConsensusV2Round,
    phase: ConsensusV2Phase,
    block: &ConsensusV2BlockRef,
) -> Vec<ConsensusV2Vote> {
    validators
        .validators
        .iter()
        .zip(keys)
        .take(validators.quorum)
        .map(|(validator, key)| {
            let mut vote = ConsensusV2Vote {
                schema: CONSENSUS_V2_VOTE_SCHEMA.to_string(),
                domain: domain.clone(),
                round,
                phase,
                block: Some(block.clone()),
                validator: validator.validator_id.clone(),
                signature: empty_signature(validator),
            };
            let message = consensus_v2_vote_signing_bytes(&vote).expect("vote bytes");
            vote.signature.signature_hex = bytes_to_hex(
                &ml_dsa_65_sign_with_context(&key.private_key, &message, CONSENSUS_V2_VOTE_CONTEXT)
                    .expect("vote signature"),
            );
            vote
        })
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn signed_block_record(
    domain: &ConsensusV2Domain,
    validators: &ConsensusV2ValidatorSet,
    keys: &[MlDsa65KeyPair],
    height: u64,
    parent: String,
    payload_hash: String,
    state_root: String,
    bridge_exit_root: Option<String>,
    batch_kind: &str,
    batch_id: String,
    receipt_ids: Vec<String>,
) -> BlockRecord {
    let block_ref = match bridge_exit_root.clone() {
        Some(root) => consensus_v2_block_ref_with_bridge_exit_root(
            domain,
            height,
            parent,
            payload_hash,
            state_root,
            root,
        ),
        None => consensus_v2_block_ref(domain, height, parent, payload_hash, state_root),
    }
    .expect("block ref");
    let round = ConsensusV2Round { height, view: 0 };
    let proposal = signed_proposal(domain, validators, keys, round, block_ref.clone());
    let prepare_qc = certify_consensus_v2_votes(
        domain,
        validators,
        round,
        ConsensusV2Phase::Prepare,
        Some(block_ref.clone()),
        signed_votes(
            domain,
            validators,
            keys,
            round,
            ConsensusV2Phase::Prepare,
            &block_ref,
        ),
    )
    .expect("prepare QC");
    let precommit_qc = certify_consensus_v2_votes(
        domain,
        validators,
        round,
        ConsensusV2Phase::Precommit,
        Some(block_ref.clone()),
        signed_votes(
            domain,
            validators,
            keys,
            round,
            ConsensusV2Phase::Precommit,
            &block_ref,
        ),
    )
    .expect("precommit QC");
    BlockRecord {
        header: BlockHeader {
            height,
            view: 0,
            parent_hash: block_ref.parent_block_id.clone(),
            proposer: proposal.proposer.clone(),
            batch_kind: batch_kind.to_string(),
            batch_id,
            state_root: block_ref.state_root.clone(),
            bridge_exit_root,
            pftl_uniswap_receipt_root: None,
            receipt_count: receipt_ids.len() as u64,
            certificate_id: "dd".repeat(48),
            certificate: BlockCertificate {
                validators: Vec::new(),
                quorum: 0,
                registry_root: String::new(),
                votes: Vec::new(),
            },
            consensus_v2_commit: Some(ConsensusV2Commit {
                schema: CONSENSUS_V2_COMMIT_SCHEMA.to_string(),
                proposal,
                prior_qcs: Vec::new(),
                timeout_certificate: None,
                prepare_qc,
                precommit_qc,
            }),
            block_hash: block_ref.block_id,
        },
        receipt_ids,
        fastpay_pre_state_effects: Vec::new(),
    }
}

