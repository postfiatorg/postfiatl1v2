// The lane export proof program's witness and input (design note, "Base half,
// step 3"). The program has its own key: the pfUSDC egress program binds the
// bridge-exit root and a burn receipt, which a lane export does not have.

pub const EXTERNAL_LANE_EXPORT_PROOF_WITNESS_SCHEMA_V1: &str =
    "postfiat.external_lane.export_proof_witness.v1";

/// Everything the lane export program needs to prove one export: PFTL finality
/// of the export's own block from the Base verifier's accepted checkpoint, the
/// block's fields (its header carries `external_lane_export_root`), the
/// chain's genesis (whose hash the finality domain signs, so it fixes the
/// lane's policy), the packet and the packet leaf's membership proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalLaneExportProofWitnessV1 {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub prior_checkpoint_block_id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub finality_ancestry: Vec<PfUsdcEgressFinalityStepV1>,
    /// The export's block, which is the proven (finalized) block.
    pub block: BlockRecord,
    pub committee_epoch: u64,
    pub committee: Vec<ValidatorRegistryEntry>,
    /// The chain's genesis: its hash must be `genesis_hash`, and its lane
    /// `packet.lane_id` gives the route profile and asset id.
    pub genesis: Genesis,
    pub packet: ExternalLaneExportPacket,
    pub export_root_proof: ExternalLaneExportRootProofV1,
}

impl ExternalLaneExportProofWitnessV1 {
    /// The pfUSDC egress witness's bounds (committee, ancestry) and the lane
    /// tree's depth for at most 4,096 leaves.
    pub fn validate_bounds(&self) -> Result<(), String> {
        if self.schema != EXTERNAL_LANE_EXPORT_PROOF_WITNESS_SCHEMA_V1 {
            return Err("external lane export proof witness schema mismatch".to_string());
        }
        pfusdc_validate_text("external_lane_export_witness.chain_id", &self.chain_id)?;
        validate_lower_hex_len(
            "external_lane_export_witness.genesis_hash",
            &self.genesis_hash,
            96,
        )?;
        validate_lower_hex_len(
            "external_lane_export_witness.prior_checkpoint_block_id",
            &self.prior_checkpoint_block_id,
            96,
        )?;
        if self.protocol_version == 0
            || self.committee_epoch == 0
            || self.committee.is_empty()
            || self.committee.len() > 64
        {
            return Err("external lane export proof witness bounds are invalid".to_string());
        }
        if self.finality_ancestry.len() > PFUSDC_EGRESS_MAX_ANCESTRY_BLOCKS_V1 {
            return Err("external lane export finality ancestry exceeds the v1 bound".to_string());
        }
        validate_finality_steps_v1(&self.finality_ancestry)?;
        self.packet.validate()?;
        if self.export_root_proof.siblings.len() > 12 {
            return Err("external lane export proof exceeds bounded depth".to_string());
        }
        Ok(())
    }
}

/// The lane export program's versioned CBOR input: one export, or a checkpoint
/// advance that the pfUSDC checkpoint statement proves unchanged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", content = "witness", rename_all = "snake_case")]
pub enum ExternalLaneExportProgramInputV1 {
    Withdrawal(ExternalLaneExportProofWitnessV1),
    Checkpoint(PfUsdcCheckpointProofWitnessV1),
}
