use postfiat_crypto_provider::{
    bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context, MlDsa65KeyPair,
};
use postfiat_ordering_fast::{
    certify_consensus_v2_votes, consensus_v2_block_ref_with_bridge_exit_root, consensus_v2_domain,
    consensus_v2_proposal_signing_bytes, consensus_v2_vote_signing_bytes, leader_for_view,
    ConsensusV2Validator, ConsensusV2ValidatorSet, CONSENSUS_V2_PROPOSAL_CONTEXT,
    CONSENSUS_V2_VOTE_CONTEXT,
};
use postfiat_types::*;

use super::verify_egress_witness_v1;

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

fn route_profile() -> VaultBridgeRouteProfileRecordV1 {
    let profile = VaultBridgeRouteProfileV1 {
        schema: VAULT_BRIDGE_ROUTE_PROFILE_SCHEMA_V1.to_string(),
        route_id: "arbitrum-pfusdc-tier4".to_string(),
        asset_id: "21".repeat(48),
        source_chain_id: 42_161,
        vault_address: "0x1111111111111111111111111111111111111111".to_string(),
        vault_runtime_code_hash: format!("0x{}", "22".repeat(32)),
        token_address: "0x3333333333333333333333333333333333333333".to_string(),
        token_runtime_code_hash: format!("0x{}", "44".repeat(32)),
        route_epoch: 7,
        verifier_kind: NAV_PROFILE_VERIFIER_SP1_ARBITRUM_FINALITY_V1.to_string(),
        evidence_tier: VAULT_BRIDGE_EVIDENCE_TIER_RECEIPT_PROVEN.to_string(),
        verifier_policy_hash: "55".repeat(32),
        verifier_program_vkey: format!("0x{}", "66".repeat(32)),
        verifier_proof_encoding: NAV_SP1_PROOF_ENCODING_GROTH16.to_string(),
        max_proof_bytes: 1_000_000,
        max_public_values_bytes: 32_768,
        max_snapshot_age_blocks: 100,
        challenge_window_blocks: 6,
        max_epoch_gap_blocks: 1_000,
        settle_deadline_blocks: 1_000,
        min_challenge_bond: 1,
        min_attestations: 0,
        minimum_confirmations: 0,
        activation_height: 1,
        expires_at_height: 10_000,
    };
    VaultBridgeRouteProfileRecordV1 {
        schema: VAULT_BRIDGE_ROUTE_PROFILE_RECORD_SCHEMA_V1.to_string(),
        profile_hash: profile.profile_hash().expect("profile hash"),
        profile,
        governance_amendment_id: "tier4-route-amendment".to_string(),
        authorized_height: 1,
    }
}

fn fixture() -> PfUsdcEgressProofWitnessV1 {
    let chain_id = "postfiat-tier4-test".to_string();
    let genesis_hash = "ab".repeat(48);
    let protocol_version = 2;
    let committee_epoch = 7;
    let (validators, keys) = committee();
    let domain = consensus_v2_domain(
        chain_id.clone(),
        genesis_hash.clone(),
        protocol_version,
        committee_epoch,
        &validators,
    );
    let packet = VaultBridgeWithdrawalPacket {
        pftl_chain_id: 1,
        source_chain_id: 42_161,
        vault_address: "0x1111111111111111111111111111111111111111".to_string(),
        token_address: "0x3333333333333333333333333333333333333333".to_string(),
        vault_bridge_asset_id: "21".repeat(48),
        burn_tx_id: "11".repeat(48),
        withdrawal_id: "12".repeat(48),
        recipient: "0x7777777777777777777777777777777777777777".to_string(),
        amount_atoms: 1_000_000,
        source_bucket_id: "13".repeat(48),
        destination_hash: "14".repeat(48),
        finalized_height: 10,
        evidence_root: "15".repeat(48),
    };
    let packet_hash = vault_bridge_withdrawal_packet_hash(&packet).expect("packet hash");
    let packet_digest = vault_bridge_withdrawal_packet_evm_digest(&packet).expect("packet digest");
    let leaf = BridgeExitLeafV1 {
        schema: BRIDGE_EXIT_LEAF_SCHEMA_V1.to_string(),
        route_epoch: 7,
        asset_id: packet.vault_bridge_asset_id.clone(),
        burn_tx_id: packet.burn_tx_id.clone(),
        withdrawal_id: packet.withdrawal_id.clone(),
        source_bucket_id: packet.source_bucket_id.clone(),
        amount_atoms: packet.amount_atoms,
        recipient: packet.recipient.clone(),
        destination_hash: packet.destination_hash.clone(),
        evidence_root: packet.evidence_root.clone(),
        finalized_height: packet.finalized_height,
        accepted_receipt_id: packet.burn_tx_id.clone(),
        accepted_receipt_code: BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE.to_string(),
        withdrawal_packet_hash: packet_hash.clone(),
        withdrawal_packet_evm_digest: packet_digest.clone(),
    };
    let leaves = vec![leaf];
    let exit_root = bridge_exit_merkle_root_v1(&leaves).expect("exit root");
    let merkle_proof = bridge_exit_merkle_proof_v1(&leaves, 0).expect("Merkle proof");
    let block_ref = consensus_v2_block_ref_with_bridge_exit_root(
        &domain,
        10,
        "aa".repeat(48),
        "bb".repeat(48),
        "cc".repeat(48),
        exit_root.clone(),
    )
    .expect("block ref");
    let round = ConsensusV2Round {
        height: 10,
        view: 0,
    };
    let proposal = signed_proposal(&domain, &validators, &keys, round, block_ref.clone());
    let prepare_qc = certify_consensus_v2_votes(
        &domain,
        &validators,
        round,
        ConsensusV2Phase::Prepare,
        Some(block_ref.clone()),
        signed_votes(
            &domain,
            &validators,
            &keys,
            round,
            ConsensusV2Phase::Prepare,
            &block_ref,
        ),
    )
    .expect("prepare QC");
    let precommit_qc = certify_consensus_v2_votes(
        &domain,
        &validators,
        round,
        ConsensusV2Phase::Precommit,
        Some(block_ref.clone()),
        signed_votes(
            &domain,
            &validators,
            &keys,
            round,
            ConsensusV2Phase::Precommit,
            &block_ref,
        ),
    )
    .expect("precommit QC");
    let commit = ConsensusV2Commit {
        schema: CONSENSUS_V2_COMMIT_SCHEMA.to_string(),
        proposal,
        prior_qcs: Vec::new(),
        timeout_certificate: None,
        prepare_qc,
        precommit_qc,
    };
    let block = BlockRecord {
        header: BlockHeader {
            height: block_ref.height,
            view: round.view,
            parent_hash: block_ref.parent_block_id.clone(),
            proposer: commit.proposal.proposer.clone(),
            batch_kind: "transactions".to_string(),
            batch_id: "bb".repeat(48),
            state_root: block_ref.state_root.clone(),
            bridge_exit_root: Some(exit_root),
            receipt_count: 1,
            certificate_id: "dd".repeat(48),
            certificate: BlockCertificate {
                validators: Vec::new(),
                quorum: 0,
                registry_root: String::new(),
                votes: Vec::new(),
            },
            consensus_v2_commit: Some(commit),
            block_hash: block_ref.block_id.clone(),
        },
        receipt_ids: vec![packet.burn_tx_id.clone()],
        fastpay_pre_state_effects: Vec::new(),
    };
    let committee = validators
        .validators
        .iter()
        .map(|validator| ValidatorRegistryEntry {
            node_id: validator.validator_id.clone(),
            algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: validator.public_key_hex.clone(),
            active: true,
        })
        .collect();
    PfUsdcEgressProofWitnessV1 {
        schema: PFUSDC_EGRESS_PROOF_WITNESS_SCHEMA_V1.to_string(),
        chain_id,
        genesis_hash,
        protocol_version,
        bridge_exit_root_activation_height: 1,
        prior_checkpoint_block_id: block_ref.parent_block_id,
        route_profile: route_profile(),
        block,
        receipt: Receipt::accepted(packet.burn_tx_id.clone(), "accepted burn"),
        merkle_proof,
        withdrawal_packet: packet,
        withdrawal_packet_hash: packet_hash,
        withdrawal_packet_evm_digest: packet_digest,
        committee_epoch,
        committee,
    }
}

#[test]
fn exact_finalized_egress_witness_accepts_and_binds_every_boundary() {
    let witness = fixture();
    let values = verify_egress_witness_v1(&witness).expect("valid egress witness");
    assert_eq!(
        values.accepted_receipt_code,
        BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE
    );
    assert_eq!(values.finalized_block_id, witness.block.header.block_hash);
    assert_eq!(values.amount_atoms, witness.withdrawal_packet.amount_atoms);

    let mut wrong_header_id = witness.clone();
    wrong_header_id.block.header.block_hash = "ee".repeat(48);
    assert!(verify_egress_witness_v1(&wrong_header_id).is_err());

    let mut rejected = witness.clone();
    rejected.receipt =
        Receipt::rejected(witness.receipt.tx_id.clone(), "rejected", "rejected burn");
    assert!(verify_egress_witness_v1(&rejected).is_err());

    let mut bad_path = witness.clone();
    bad_path.merkle_proof.leaf.amount_atoms += 1;
    assert!(verify_egress_witness_v1(&bad_path).is_err());

    let mut wrong_chain = witness.clone();
    wrong_chain.chain_id = "foreign-chain".to_string();
    assert!(verify_egress_witness_v1(&wrong_chain).is_err());

    let mut duplicate_validator = witness.clone();
    duplicate_validator
        .committee
        .push(witness.committee[0].clone());
    assert!(verify_egress_witness_v1(&duplicate_validator).is_err());

    let mut under_quorum = witness;
    under_quorum
        .block
        .header
        .consensus_v2_commit
        .as_mut()
        .expect("commit")
        .precommit_qc
        .votes
        .pop();
    assert!(verify_egress_witness_v1(&under_quorum).is_err());
}
