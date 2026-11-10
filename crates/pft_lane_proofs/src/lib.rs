//! The lane export proof statement (design note, "Base half, step 3"). It
//! calls the pfUSDC finality and checkpoint checks unchanged and copies none of
//! their code.

use postfiat_ordering_fast::consensus_v2_block_ref_with_external_lane_export_root;
use postfiat_pfusdc_proofs::{verify_checkpoint_witness_v1, verify_pftl_finality_segment_v1};
use postfiat_types::{
    external_lane_export_egress_public_values, ConsensusV2Domain, ExternalLaneExportCheckpointV1,
    ExternalLaneExportInclusionV1, ExternalLaneExportProofWitnessV1,
    PfUsdcCheckpointProofWitnessV1, PfUsdcCheckpointPublicValuesV1, PfUsdcEgressPublicValuesV1,
    CONSENSUS_V2_DOMAIN_SCHEMA,
};

/// Proves one lane export and returns exactly
/// `external_lane_export_egress_public_values` for it:
/// (i) PFTL finality of the witness block from the prior checkpoint, by the
/// pfUSDC finality-segment check; (ii) the export's block is that block: its
/// consensus v2 block id, recomputed with the header's export root, is the
/// finalized id, and its height is the packet's; (iii) the packet's leaf is in
/// that root at the claimed index under the claimed count; (iv) the lane
/// policy, from the genesis whose hash the finality domain signed, gives the
/// route profile hash and asset id; (v) the accepted
/// transaction id is the packet's. An export in an older block is refused.
pub fn verify_external_lane_export_witness_v1(
    witness: &ExternalLaneExportProofWitnessV1,
) -> Result<PfUsdcEgressPublicValuesV1, String> {
    witness.validate_bounds()?;
    let header = &witness.block.header;
    let packet = &witness.packet;

    // (i) finality, unchanged.
    let segment = verify_pftl_finality_segment_v1(
        &witness.chain_id,
        &witness.genesis_hash,
        witness.protocol_version,
        &witness.prior_checkpoint_block_id,
        &witness.finality_ancestry,
        &witness.block,
        witness.committee_epoch,
        &witness.committee,
    )?;
    let committed = &segment.committed_block;

    // (ii) the export's block is the finalized block.
    let export_root = header
        .external_lane_export_root
        .as_deref()
        .ok_or_else(|| "export block carries no external lane export root".to_string())?;
    if committed.external_lane_export_root.as_deref() != Some(export_root) {
        return Err("finalized block does not commit the header's export root".to_string());
    }
    if header.height != packet.exported_at_height {
        return Err(
            "export is not in the proven block; an older export block is not supported in v1"
                .to_string(),
        );
    }
    let domain = ConsensusV2Domain {
        schema: CONSENSUS_V2_DOMAIN_SCHEMA.to_string(),
        chain_id: witness.chain_id.clone(),
        genesis_hash: witness.genesis_hash.clone(),
        protocol_version: witness.protocol_version,
        committee_epoch: witness.committee_epoch,
        committee_root: segment.committee_root.clone(),
    };
    let recomputed = consensus_v2_block_ref_with_external_lane_export_root(
        &domain,
        header.height,
        header.parent_hash.clone(),
        committed.payload_hash.clone(),
        header.state_root.clone(),
        header.bridge_exit_root.clone(),
        header.pftl_uniswap_receipt_root.clone(),
        export_root,
    )
    .map_err(|error| format!("export block id: {error}"))?;
    if recomputed.block_id != committed.block_id || recomputed.block_id != header.block_hash {
        return Err("export block fields do not hash to the finalized block id".to_string());
    }

    // (iv) the lane policy is the one of the genesis the finality domain
    // signed: same hash, chain id and protocol version.
    let pins = witness
        .genesis
        .external_lane_verifier_pins(&packet.lane_id)?;
    if pins.pftl_genesis_hash != witness.genesis_hash
        || pins.pftl_chain_id != witness.chain_id
        || witness.genesis.protocol_version != witness.protocol_version
    {
        return Err("witness genesis is not the genesis the finality check proved".to_string());
    }
    let lane = witness
        .genesis
        .external_lanes
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|lane| lane.id == packet.lane_id)
        .ok_or_else(|| "export packet names a lane the genesis does not have".to_string())?;

    // (iii) membership of the recomputed leaf.
    let leaf = packet.export_root_leaf(&witness.chain_id)?;
    if witness.export_root_proof.leaf != hex(&leaf) {
        return Err("export inclusion proves another leaf".to_string());
    }
    postfiat_types::verify_external_lane_export_root_proof_v1(
        export_root,
        &witness.export_root_proof,
    )?;

    // (v) the transaction is a receipt of the block.
    if !witness
        .block
        .receipt_ids
        .iter()
        .any(|id| id == &packet.tx_id)
    {
        return Err("export transaction is not a receipt of the proven block".to_string());
    }

    let checkpoint = ExternalLaneExportCheckpointV1 {
        pftl_chain_id: witness.chain_id.clone(),
        pftl_genesis_hash: witness.genesis_hash.clone(),
        pftl_protocol_version: witness.protocol_version,
        prior_checkpoint_block_id: witness.prior_checkpoint_block_id.clone(),
        committee_epoch: witness.committee_epoch,
        committee_root: segment.committee_root.clone(),
        committee_transition_commitment: segment.transition_start_root.clone(),
        finalized_block_height: committed.height,
        finalized_block_view: header.view,
        finalized_block_id: committed.block_id.clone(),
        finalized_parent_block_id: committed.parent_block_id.clone(),
        finalized_state_root: committed.state_root.clone(),
        export: ExternalLaneExportInclusionV1 {
            block_id: committed.block_id.clone(),
            block_height: header.height,
            export_root: export_root.to_string(),
            proof: witness.export_root_proof.clone(),
        },
    };
    let values = external_lane_export_egress_public_values(lane, packet, &checkpoint)?;

    // (iv) and (v), and the checkpoint fields, field by field against what
    // the finality check returned.
    let expected = [
        ("pftl_chain_id", &values.pftl_chain_id, &witness.chain_id),
        (
            "pftl_genesis_hash",
            &values.pftl_genesis_hash,
            &witness.genesis_hash,
        ),
        (
            "prior_checkpoint_block_id",
            &values.prior_checkpoint_block_id,
            &witness.prior_checkpoint_block_id,
        ),
        (
            "resulting_checkpoint_block_id",
            &values.resulting_checkpoint_block_id,
            &committed.block_id,
        ),
        (
            "committee_root",
            &values.committee_root,
            &segment.committee_root,
        ),
        (
            "committee_transition_commitment",
            &values.committee_transition_commitment,
            &segment.transition_start_root,
        ),
        (
            "finalized_block_id",
            &values.finalized_block_id,
            &committed.block_id,
        ),
        (
            "finalized_parent_block_id",
            &values.finalized_parent_block_id,
            &committed.parent_block_id,
        ),
        (
            "finalized_state_root",
            &values.finalized_state_root,
            &committed.state_root,
        ),
        (
            "route_profile_hash",
            &values.route_profile_hash,
            &pins.route_profile_hash,
        ),
        ("asset_id", &values.asset_id, &pins.asset_id),
        (
            "accepted_receipt_id",
            &values.accepted_receipt_id,
            &packet.tx_id,
        ),
    ];
    for (field, actual, wanted) in expected {
        if actual != wanted {
            return Err(format!("public value {field} does not match the proof"));
        }
    }
    if values.pftl_protocol_version != witness.protocol_version
        || values.committee_epoch != witness.committee_epoch
        || values.finalized_block_height != committed.height
        || values.finalized_block_view != header.view
        || values.withdrawal_finalized_height != committed.height
        || values.bridge_exit_root != export_root
        || values.exit_leaf_index != witness.export_root_proof.leaf_index
    {
        return Err("public values do not match the proven block".to_string());
    }
    Ok(values)
}

/// The checkpoint advance, by the pfUSDC checkpoint check unchanged.
pub fn verify_external_lane_checkpoint_witness_v1(
    witness: &PfUsdcCheckpointProofWitnessV1,
) -> Result<PfUsdcCheckpointPublicValuesV1, String> {
    verify_checkpoint_witness_v1(witness)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

#[cfg(test)]
mod tests;
