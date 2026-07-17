use super::*;

pub const PFUSDC_EGRESS_WITNESS_SCHEMA_V1: &str = "postfiat.pfusdc.egress_witness.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PfUsdcEgressWitnessOptions {
    pub data_dir: PathBuf,
    pub withdrawal_id: String,
}

pub fn pfusdc_egress_witness(
    options: PfUsdcEgressWitnessOptions,
) -> io::Result<PfUsdcEgressWitnessV1> {
    validate_lower_hex_len("withdrawal_id", &options.withdrawal_id, 96)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
    let store = NodeStore::new(options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let ledger = store.read_ledger()?;
    let receipts = store.read_receipts()?;
    let blocks = store.read_blocks()?;

    let redemption = ledger
        .vault_bridge_redemptions
        .iter()
        .find(|redemption| redemption.withdrawal_packet.withdrawal_id == options.withdrawal_id)
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "withdrawal ID is not finalized"))?;
    let block = blocks
        .blocks
        .iter()
        .find(|block| block.header.height == redemption.created_at_height)
        .cloned()
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::NotFound, "withdrawal block is unavailable")
        })?;
    let expected_root = block.header.bridge_exit_root.as_deref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "withdrawal block predates the Tier-4 bridge-exit-root encoding",
        )
    })?;
    let activation_height =
        bridge_exit_root_activation_height_for_chain(&governance).ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "bridge exit root is not governed",
            )
        })?;
    if block.header.height < activation_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "withdrawal block is before bridge-exit-root activation",
        ));
    }
    let block_receipts = receipts_for_block(&receipts, &block.receipt_ids)?;
    let receipt = block_receipts
        .iter()
        .find(|receipt| receipt.tx_id == redemption.withdrawal_packet.burn_tx_id)
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "burn receipt is absent"))?;
    if !receipt.accepted || receipt.code != BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "burn receipt is not literal accepted",
        ));
    }

    let leaves =
        bridge_exit_leaves_for_block(&governance, &ledger, &block_receipts, block.header.height)?;
    let leaf_index = leaves
        .iter()
        .position(|leaf| leaf.withdrawal_id == options.withdrawal_id)
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "withdrawal exit leaf is absent")
        })?;
    let merkle_proof = postfiat_types::bridge_exit_merkle_proof_v1(&leaves, leaf_index)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    postfiat_types::verify_bridge_exit_merkle_proof_v1(expected_root, &merkle_proof)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;

    let bucket = ledger
        .vault_bridge_bucket_states
        .iter()
        .find(|bucket| bucket.bucket_id == redemption.bucket_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "withdrawal bucket is absent"))?;
    let route_profile = governance
        .authorized_vault_bridge_route_profile(&redemption.asset_id, &bucket.policy_hash)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?
        .clone();
    let commit = block.header.consensus_v2_commit.as_ref().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            "Tier-4 egress witness requires a consensus-v2 commit",
        )
    })?;
    if commit.proposal.block.bridge_exit_root.as_deref() != Some(expected_root) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "consensus-v2 commit does not bind the block bridge-exit root",
        ));
    }
    let committee_validators = active_validator_ids(&governance)?;
    let validator_registry =
        read_validator_registry_file(&store.data_dir().join(VALIDATOR_REGISTRY_FILE))?;
    let committee_root = validator_registry_root(&validator_registry, &committee_validators)?;
    if committee_root != commit.proposal.domain.committee_root {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "current validator registry does not reproduce the finalized committee root",
        ));
    }

    Ok(PfUsdcEgressWitnessV1 {
        schema: PFUSDC_EGRESS_WITNESS_SCHEMA_V1.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        bridge_exit_root_activation_height: activation_height,
        route_profile,
        block,
        receipt,
        merkle_proof,
        withdrawal_packet: redemption.withdrawal_packet,
        withdrawal_packet_hash: redemption.withdrawal_packet_hash,
        withdrawal_packet_evm_digest: redemption.withdrawal_packet_evm_digest,
        committee_validators,
        validator_registry,
    })
}
