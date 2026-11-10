use super::*;
use postfiat_types::PfUsdcEgressPublicValuesV1;
use std::fs;

/// One lane export's proof input (design note, "Base half, step 3").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExternalLaneExportProofInputOptions {
    pub data_dir: PathBuf,
    pub lane_id: String,
    pub export_nonce: Option<u64>,
    pub tx_id: Option<String>,
    /// The Base verifier's latest accepted checkpoint block id.
    pub prior_checkpoint_block_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExternalLaneExportProofInput {
    pub witness: postfiat_types::ExternalLaneExportProofWitnessV1,
    /// The guest's stdin item: `ExternalLaneExportProgramInputV1::Withdrawal`
    /// as CBOR.
    #[serde(skip)]
    pub program_input_cbor: Vec<u8>,
    pub expected_public_values: PfUsdcEgressPublicValuesV1,
    /// `canonical_bytes_without_commitment()` of the expected values, hex: the
    /// bytes the guest commits and `verifyAndConsume` decodes.
    pub expected_public_values_hex: String,
    /// Whether the block's packets came from an archive replay (pruned).
    pub rebuilt_from_archive: bool,
}

fn invalid_input(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message.into())
}

fn invalid_data(message: impl Into<String>) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.into())
}

/// Builds the witness of one export from its finalized block and checks it
/// with the program's own statement before returning it.
pub fn external_lane_export_proof_input(
    options: ExternalLaneExportProofInputOptions,
) -> io::Result<ExternalLaneExportProofInput> {
    validate_lower_hex_len(
        "prior_checkpoint_block_id",
        &options.prior_checkpoint_block_id,
        96,
    )
    .map_err(invalid_input)?;
    if options.export_nonce.is_some() == options.tx_id.is_some() {
        return Err(invalid_input(
            "name the export by exactly one of --export-nonce and --tx-id",
        ));
    }
    if let Some(tx_id) = options.tx_id.as_deref() {
        validate_lower_hex_len("tx_id", tx_id, 96).map_err(invalid_input)?;
    }
    let store = NodeStore::new(options.data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let ledger = store.read_ledger()?;
    let receipts = store.read_receipts()?;
    let blocks = store.read_blocks()?;
    let lane = genesis
        .external_lanes
        .as_deref()
        .unwrap_or_default()
        .iter()
        .find(|lane| lane.id == options.lane_id)
        .cloned()
        .ok_or_else(|| {
            invalid_input(format!(
                "genesis has no external lane `{}`",
                options.lane_id
            ))
        })?;
    if !lane.policy.exports_enabled {
        return Err(invalid_input(format!(
            "external lane `{}` has exports off",
            lane.id
        )));
    }
    let activation =
        postfiat_types::external_lane_export_root_activation_height(&genesis, &governance)
            .ok_or_else(|| {
                invalid_data("the external lane export root is not active on this chain")
            })?;
    let state = ledger
        .external_lanes
        .iter()
        .find(|state| state.lane_id == lane.id)
        .ok_or_else(|| invalid_data(format!("external lane `{}` has no state", lane.id)))?;
    let matches = |packet: &postfiat_types::ExternalLaneExportPacket| match (
        options.export_nonce,
        options.tx_id.as_deref(),
    ) {
        (Some(nonce), _) => packet.lane_id == lane.id && packet.export_nonce == nonce,
        (None, Some(tx_id)) => packet.lane_id == lane.id && packet.tx_id == tx_id,
        (None, None) => false,
    };

    // The export's height: from the stored packet, else from the finalized
    // block holding the transaction or, for a pruned nonce, from the replay.
    let stored = state.exports.iter().find(|packet| matches(packet)).cloned();
    let pruned_nonce = options
        .export_nonce
        .is_some_and(|nonce| nonce < state.pruned_export_count);
    let pruned_tx_height = options
        .tx_id
        .as_deref()
        .filter(|_| state.pruned_export_count > 0)
        .and_then(|tx_id| {
            blocks
                .blocks
                .iter()
                .find(|block| block.receipt_ids.iter().any(|id| id == tx_id))
                .map(|block| block.header.height)
        });
    if stored.is_none() && !pruned_nonce && pruned_tx_height.is_none() {
        if options
            .export_nonce
            .is_some_and(|nonce| nonce >= state.next_export_nonce)
        {
            return Err(invalid_input(format!(
                "external lane `{}` has no export nonce {} (next is {}); an unexecuted export is not final yet",
                lane.id,
                options.export_nonce.unwrap_or_default(),
                state.next_export_nonce
            )));
        }
        return Err(invalid_input(
            "unknown export: no finalized block holds this transaction id; an export still in the mempool is not final yet",
        ));
    }

    // The block's packets in receipt order: from the lane's stored records, or
    // rebuilt by replaying the archive when the records are pruned.
    // Another lane may have pruned its share of the block: then the stored
    // packets miss the header root and the archive replay rebuilds them.
    let stored = match stored {
        Some(packet) => {
            let block = finalized_block_at(&blocks, packet.exported_at_height)?;
            let block_receipts = receipts_for_block(&receipts, &block.receipt_ids)?;
            let packets = external_lane_export_packets_for_block(
                &ledger,
                &block_receipts,
                packet.exported_at_height,
            )?;
            let root = export_root_of(&genesis, &packets)?;
            (Some(root) == block.header.external_lane_export_root).then_some((packet, packets))
        }
        None => None,
    };
    let rebuilt_from_archive = stored.is_none();
    let (packet, block_packets) = match stored {
        Some(found) => found,
        None => replay_export_block(&store, &genesis, &blocks, &receipts, &matches).map_err(
            |error| {
                invalid_data(format!(
                    "the export's block packets are pruned from the lane records and the archive replay failed ({error}); \
                     run this command on a data directory that keeps the batch archive from genesis, \
                     or from a history checkpoint below the export's height"
                ))
            },
        )?,
    };
    let height = packet.exported_at_height;
    if height < activation {
        return Err(invalid_input(format!(
            "export at height {height} is before the export root activation height {activation}"
        )));
    }
    let block = finalized_block_at(&blocks, height)?;
    let export_root = block
        .header
        .external_lane_export_root
        .clone()
        .ok_or_else(|| {
            invalid_data(format!(
                "block {height} carries no external lane export root"
            ))
        })?;
    if block.header.consensus_v2_commit.is_none() {
        return Err(invalid_data(format!(
            "block {height} has no consensus v2 commit; the export is not final yet"
        )));
    }
    let leaves = export_leaves(&genesis, &block_packets)?;
    let leaf_index = block_packets
        .iter()
        .position(|candidate| candidate == &packet)
        .ok_or_else(|| invalid_data("the export is not among its block's packets"))?;
    if export_root_of(&genesis, &block_packets)? != export_root {
        return Err(invalid_data(format!(
            "block {height}'s stored export packets do not reproduce its header root"
        )));
    }
    let export_root_proof = postfiat_types::external_lane_export_root_proof_v1(&leaves, leaf_index)
        .map_err(invalid_data)?;
    let segment = pfusdc_export_finality_segment(
        &store,
        &genesis,
        &governance,
        &blocks,
        options.prior_checkpoint_block_id,
        &block,
    )?;
    let witness = postfiat_types::ExternalLaneExportProofWitnessV1 {
        schema: postfiat_types::EXTERNAL_LANE_EXPORT_PROOF_WITNESS_SCHEMA_V1.to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        prior_checkpoint_block_id: segment.prior_checkpoint_block_id,
        finality_ancestry: segment.finality_ancestry,
        block,
        committee_epoch: segment.committee_epoch,
        committee: segment.committee,
        genesis: genesis.clone(),
        packet,
        export_root_proof,
    };
    let expected_public_values = postfiat_pft_lane_proofs::verify_external_lane_export_witness_v1(
        &witness,
    )
    .map_err(|error| {
        invalid_data(format!(
            "the built witness does not prove the export: {error}"
        ))
    })?;
    let bytes = expected_public_values
        .canonical_bytes_without_commitment()
        .map_err(invalid_data)?;
    let program_input_cbor = serde_cbor::to_vec(
        &postfiat_types::ExternalLaneExportProgramInputV1::Withdrawal(witness.clone()),
    )
    .map_err(|error| invalid_data(format!("encode program input: {error}")))?;
    Ok(ExternalLaneExportProofInput {
        witness,
        program_input_cbor,
        expected_public_values,
        expected_public_values_hex: bytes_to_hex(&bytes),
        rebuilt_from_archive,
    })
}

fn export_leaves(
    genesis: &Genesis,
    packets: &[postfiat_types::ExternalLaneExportPacket],
) -> io::Result<Vec<[u8; 48]>> {
    packets
        .iter()
        .map(|packet| packet.export_root_leaf(&genesis.chain_id))
        .collect::<Result<Vec<_>, _>>()
        .map_err(invalid_data)
}

fn export_root_of(
    genesis: &Genesis,
    packets: &[postfiat_types::ExternalLaneExportPacket],
) -> io::Result<String> {
    postfiat_types::external_lane_export_root_v1(&export_leaves(genesis, packets)?)
        .map_err(invalid_data)
}

fn finalized_block_at(blocks: &BlockLog, height: u64) -> io::Result<BlockRecord> {
    blocks
        .blocks
        .iter()
        .find(|block| block.header.height == height)
        .cloned()
        .ok_or_else(|| invalid_data(format!("export block {height} is not final yet")))
}

type ExportBlock = (
    postfiat_types::ExternalLaneExportPacket,
    Vec<postfiat_types::ExternalLaneExportPacket>,
);

/// Replays the archive and keeps the packets of the block holding the export.
fn replay_export_block(
    store: &NodeStore,
    genesis: &Genesis,
    blocks: &BlockLog,
    receipts: &[Receipt],
    matches: &dyn Fn(&postfiat_types::ExternalLaneExportPacket) -> bool,
) -> io::Result<ExportBlock> {
    let archive = store.read_batch_archive()?;
    let mut found = None;
    verify_replayed_blocks_with_exports(
        store,
        genesis,
        blocks,
        &archive,
        receipts,
        &mut |_, packets| {
            if found.is_none() {
                if let Some(packet) = packets.iter().find(|packet| matches(packet)).cloned() {
                    found = Some((packet, packets));
                }
            }
        },
    )?;
    found.ok_or_else(|| invalid_data("the archive replay holds no such export"))
}

/// Writes `input.cbor` and `expected-public-values.json` under `output_dir`.
pub fn write_external_lane_export_proof_input(
    input: &ExternalLaneExportProofInput,
    output_dir: &Path,
) -> io::Result<()> {
    fs::create_dir_all(output_dir)?;
    fs::write(output_dir.join("input.cbor"), &input.program_input_cbor)?;
    let json = serde_json::to_string_pretty(input)
        .map_err(|error| invalid_data(format!("encode proof input: {error}")))?;
    fs::write(output_dir.join("expected-public-values.json"), json)
}
