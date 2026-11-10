// The Base half of Phase 3: what the Base finality verifier
// (`PFTLFinalityVerifierV1`) pins for a lane, and the public values it decodes
// for one `external_lane_export_v1` packet.

pub const EXTERNAL_LANE_ROUTE_PROFILE_SCHEMA_V1: &str = "postfiat.external_lane.route_profile.v1";
pub const EXTERNAL_LANE_ROUTE_PROFILE_HASH_DOMAIN_V1: &str =
    "postfiat.external_lane.route_profile_hash.v1";
pub const EXTERNAL_LANE_ASSET_ID_DOMAIN_V1: &str = "postfiat.external_lane.asset_id.v1";
pub const EXTERNAL_LANE_EXPORT_EVM_DIGEST_DOMAIN_V1: &str =
    "postfiat.external_lane.export_evm_digest.v1";
/// The asset a lane carries: native PFT, as `issued_asset_id` names an asset
/// by code and version.
pub const EXTERNAL_LANE_ASSET_CODE: &str = "PFT";
pub const EXTERNAL_LANE_ASSET_VERSION: u32 = 1;
/// A lane's route is fixed by genesis and has no governed successor, so it has
/// exactly one epoch, the first.
pub const EXTERNAL_LANE_ROUTE_EPOCH_V1: u32 = 1;

/// The route a Base verifier pins for a lane: the fields of
/// `VaultBridgeRouteProfileV1` that genesis fixes for the lane, in its order
/// and preimage format. The verifier kind, program key and proof bounds are
/// deployment values of the verifier, not route fields of a lane.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalLaneRouteProfileV1 {
    pub schema: String,
    pub route_id: String,
    pub asset_id: String,
    pub source_chain_id: u64,
    pub vault_address: String,
    pub vault_runtime_code_hash: String,
    pub token_address: String,
    pub token_runtime_code_hash: String,
    pub route_epoch: u32,
}

impl ExternalLaneRouteProfileV1 {
    /// The shapes the preimage relies on: the schema, the lane id, a 48-byte
    /// asset id, a Base chain id, lower-case `0x` addresses and code hashes,
    /// and a nonzero epoch.
    pub fn validate(&self) -> Result<(), String> {
        if self.schema != EXTERNAL_LANE_ROUTE_PROFILE_SCHEMA_V1 {
            return Err("external lane route profile schema mismatch".to_string());
        }
        validate_external_lane_id(&self.route_id)?;
        validate_lower_hex_len("external_lane_route_profile.asset_id", &self.asset_id, 96)?;
        if !EXTERNAL_LANE_SOURCE_CHAIN_IDS.contains(&self.source_chain_id) {
            return Err("external lane route profile source chain id is not Base".to_string());
        }
        validate_lane_hex(
            "vault_address",
            &self.vault_address,
            EXTERNAL_LANE_EVM_ADDRESS_HEX_LEN,
        )?;
        validate_lane_hex(
            "token_address",
            &self.token_address,
            EXTERNAL_LANE_EVM_ADDRESS_HEX_LEN,
        )?;
        validate_lane_hex(
            "vault_runtime_code_hash",
            &self.vault_runtime_code_hash,
            EXTERNAL_LANE_CODE_HASH_HEX_LEN,
        )?;
        validate_lane_hex(
            "token_runtime_code_hash",
            &self.token_runtime_code_hash,
            EXTERNAL_LANE_CODE_HASH_HEX_LEN,
        )?;
        if self.route_epoch == 0 {
            return Err("external lane route profile route_epoch must be nonzero".to_string());
        }
        Ok(())
    }

    pub fn canonical_preimage(&self) -> Result<String, String> {
        self.validate()?;
        Ok(format!(
            "schema={}\nroute_id={}\nasset_id={}\nsource_chain_id={}\nvault_address={}\nvault_runtime_code_hash={}\ntoken_address={}\ntoken_runtime_code_hash={}\nroute_epoch={}\n",
            self.schema,
            self.route_id,
            self.asset_id,
            self.source_chain_id,
            self.vault_address,
            self.vault_runtime_code_hash,
            self.token_address,
            self.token_runtime_code_hash,
            self.route_epoch,
        ))
    }

    /// SHA3-384 over the domain tag, a zero byte and the preimage: 48 bytes,
    /// as `VaultBridgeRouteProfileV1::profile_hash`. Validates first.
    pub fn profile_hash(&self) -> Result<String, String> {
        Ok(hash_hex_domain(
            EXTERNAL_LANE_ROUTE_PROFILE_HASH_DOMAIN_V1,
            self.canonical_preimage()?.as_bytes(),
        ))
    }
}

impl GenesisExternalLane {
    /// Lower level: the caller names the chain id. Deployment code must use
    /// `Genesis::external_lane_verifier_pins`, which takes it from the genesis.
    ///
    /// The lane's PFT asset id: `issued_asset_id`'s construction (PFTL chain
    /// id, then the asset's issuer, code and version) with the lane's genesis
    /// binding to Base in the issuer's place, under its own domain.
    pub fn pft_asset_id(&self, pftl_chain_id: &str) -> Result<String, String> {
        validate_chain_id(pftl_chain_id)?;
        self.validate()?;
        let preimage = format!(
            "chain_id={pftl_chain_id}\nlane_id={}\nsource_chain_id={}\nlock_contract_address={}\ntoken_address={}\ncode_bytes={}\ncode={EXTERNAL_LANE_ASSET_CODE}\nversion={EXTERNAL_LANE_ASSET_VERSION}\n",
            self.id,
            self.policy.chain_id,
            self.policy.lock_contract_address,
            self.policy.token_address,
            EXTERNAL_LANE_ASSET_CODE.len(),
        );
        Ok(hash_hex_domain(
            EXTERNAL_LANE_ASSET_ID_DOMAIN_V1,
            preimage.as_bytes(),
        ))
    }

    /// The lane's route profile, every field from genesis. Lower level, as
    /// `pft_asset_id`; deployment code uses `Genesis::external_lane_verifier_pins`.
    pub fn route_profile(&self, pftl_chain_id: &str) -> Result<ExternalLaneRouteProfileV1, String> {
        Ok(ExternalLaneRouteProfileV1 {
            schema: EXTERNAL_LANE_ROUTE_PROFILE_SCHEMA_V1.to_string(),
            route_id: self.id.clone(),
            asset_id: self.pft_asset_id(pftl_chain_id)?,
            source_chain_id: self.policy.chain_id,
            vault_address: self.policy.lock_contract_address.clone(),
            vault_runtime_code_hash: self.policy.lock_runtime_code_hash.clone(),
            token_address: self.policy.token_address.clone(),
            token_runtime_code_hash: self.policy.token_runtime_code_hash.clone(),
            route_epoch: EXTERNAL_LANE_ROUTE_EPOCH_V1,
        })
    }
}

/// A finalized PFTL checkpoint at or above the export's block, as the egress
/// public values carry it. Every hash is 96 lower-case hex digits WITHOUT `0x`,
/// as in `PfUsdcEgressPublicValuesV1` (the lane state's own roots carry `0x`;
/// none of them is an input here). `export` places the packet in the
/// `external_lane_export_root` of the block that accepted it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalLaneExportCheckpointV1 {
    pub pftl_chain_id: String,
    pub pftl_genesis_hash: String,
    pub pftl_protocol_version: u32,
    pub prior_checkpoint_block_id: String,
    pub committee_epoch: u64,
    pub committee_root: String,
    pub committee_transition_commitment: String,
    pub finalized_block_height: u64,
    pub finalized_block_view: u64,
    pub finalized_block_id: String,
    pub finalized_parent_block_id: String,
    pub finalized_state_root: String,
    pub export: ExternalLaneExportInclusionV1,
}

/// The export's block: its height, the header's `external_lane_export_root`
/// and the packet leaf's membership proof under it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalLaneExportInclusionV1 {
    pub block_height: u64,
    pub export_root: String,
    pub proof: ExternalLaneExportRootProofV1,
}

impl ExternalLaneExportInclusionV1 {
    /// The inclusion of the only export of a block.
    pub fn single(packet: &ExternalLaneExportPacket, chain_id: &str) -> Result<Self, String> {
        let leaf = packet.export_root_leaf(chain_id)?;
        Ok(Self {
            block_height: packet.exported_at_height,
            export_root: external_lane_export_root_v1(&[leaf])?,
            proof: external_lane_export_root_proof_v1(&[leaf], 0)?,
        })
    }
}

/// The 32-byte digest the verifier returns and the lock emits as
/// `packetDigest`: Keccak-256 over the domain tag, a zero byte and the
/// 48-byte packet digest.
pub fn external_lane_export_evm_digest(
    packet: &ExternalLaneExportPacket,
) -> Result<String, String> {
    Ok(pfusdc_keccak_commitment(
        EXTERNAL_LANE_EXPORT_EVM_DIGEST_DOMAIN_V1,
        &packet.digest()?,
    ))
}

fn lane_hex_without_prefix(field: &str, value: &str) -> Result<String, String> {
    let digits = value
        .strip_prefix("0x")
        .ok_or_else(|| format!("{field} must be 0x-prefixed"))?;
    Ok(digits.to_string())
}

/// Every field of the verifier's `DecodedEgress` for one export packet, as the
/// egress public values the verifier decodes (field table: design note,
/// "Base half, step 1").
pub fn external_lane_export_egress_public_values(
    lane: &GenesisExternalLane,
    packet: &ExternalLaneExportPacket,
    checkpoint: &ExternalLaneExportCheckpointV1,
) -> Result<PfUsdcEgressPublicValuesV1, String> {
    lane.validate()?;
    packet.validate()?;
    if !lane.policy.exports_enabled {
        return Err(format!("external lane `{}` has no exports", lane.id));
    }
    if packet.lane_id != lane.id {
        return Err("export packet names another lane".to_string());
    }
    if packet.base_recipient == lane.policy.lock_contract_address
        || packet.base_recipient == lane.policy.token_address
        || packet.base_recipient == format!("0x{}", "0".repeat(40))
    {
        return Err(
            "export recipient is the lane's lock, its token or the zero address".to_string(),
        );
    }
    if packet.exported_at_height == 0
        || packet.exported_at_height > checkpoint.finalized_block_height
    {
        return Err("export is not at or below the finalized checkpoint".to_string());
    }
    validate_lower_hex_len("external_lane_export.tx_id", &packet.tx_id, 96)?;
    let chain_id = checkpoint.pftl_chain_id.as_str();
    let export = &checkpoint.export;
    if export.block_height != packet.exported_at_height {
        return Err("export inclusion names another block".to_string());
    }
    if export.proof.leaf != bytes_to_hex(&packet.export_root_leaf(chain_id)?) {
        return Err("export inclusion proves another leaf".to_string());
    }
    verify_external_lane_export_root_proof_v1(&export.export_root, &export.proof)?;
    let route = lane.route_profile(chain_id)?;
    let digest = bytes_to_hex(&packet.digest()?);
    let burn_tx_id = bytes_to_hex(&packet.burn_tx_commitment(chain_id));
    let withdrawal_id = bytes_to_hex(&packet.withdrawal_id_commitment(chain_id)?);
    let route_epoch = u64::from(route.route_epoch);
    let proof_nullifier = pfusdc_egress_proof_nullifier_v1(
        route_epoch,
        &burn_tx_id,
        &withdrawal_id,
        &checkpoint.finalized_block_id,
    )?;
    let none = "00".repeat(48);
    let mut values = PfUsdcEgressPublicValuesV1 {
        schema: PFUSDC_EGRESS_PUBLIC_VALUES_SCHEMA_V1.to_string(),
        proof_program_version: 1,
        pftl_chain_id: chain_id.to_string(),
        pftl_genesis_hash: checkpoint.pftl_genesis_hash.clone(),
        pftl_protocol_version: checkpoint.pftl_protocol_version,
        route_profile_hash: route.profile_hash()?,
        route_epoch,
        prior_checkpoint_block_id: checkpoint.prior_checkpoint_block_id.clone(),
        resulting_checkpoint_block_id: checkpoint.finalized_block_id.clone(),
        committee_epoch: checkpoint.committee_epoch,
        committee_root: checkpoint.committee_root.clone(),
        committee_transition_commitment: checkpoint.committee_transition_commitment.clone(),
        finalized_block_height: checkpoint.finalized_block_height,
        finalized_block_view: checkpoint.finalized_block_view,
        finalized_block_id: checkpoint.finalized_block_id.clone(),
        finalized_parent_block_id: checkpoint.finalized_parent_block_id.clone(),
        finalized_state_root: checkpoint.finalized_state_root.clone(),
        bridge_exit_root: export.export_root.clone(),
        exit_leaf_index: export.proof.leaf_index,
        exit_leaf_commitment: export.proof.leaf.clone(),
        accepted_receipt_id: packet.tx_id.clone(),
        accepted_receipt_code: BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE.to_string(),
        asset_id: route.asset_id.clone(),
        burn_tx_id,
        withdrawal_id,
        source_bucket_id: bytes_to_hex(&lane.route_config_digest()?),
        amount_atoms: packet.amount_atoms,
        recipient: packet.base_recipient.clone(),
        destination_hash: none.clone(),
        evidence_root: none,
        withdrawal_finalized_height: export.block_height,
        arbitrum_chain_id: lane.policy.chain_id,
        vault_address: route.vault_address,
        vault_runtime_code_hash: lane_hex_without_prefix(
            "lock_runtime_code_hash",
            &route.vault_runtime_code_hash,
        )?,
        token_address: route.token_address,
        token_runtime_code_hash: lane_hex_without_prefix(
            "token_runtime_code_hash",
            &route.token_runtime_code_hash,
        )?,
        withdrawal_packet_digest: external_lane_export_evm_digest(packet)?,
        withdrawal_packet_hash: digest,
        proof_nullifier,
        public_values_commitment: String::new(),
    };
    values.seal()?;
    values.validate()?;
    Ok(values)
}

/// The exact bytes the verifier's `verifyAndConsume` decodes.
pub fn external_lane_export_egress_public_values_bytes(
    lane: &GenesisExternalLane,
    packet: &ExternalLaneExportPacket,
    checkpoint: &ExternalLaneExportCheckpointV1,
) -> Result<Vec<u8>, String> {
    external_lane_export_egress_public_values(lane, packet, checkpoint)?
        .canonical_bytes_without_commitment()
}

pub const EXTERNAL_LANE_EXPORT_ROOT_LEAF_DOMAIN_V1: &str =
    "postfiat.external_lane.export_root_leaf.v1";

impl ExternalLaneExportPacket {
    /// The packet's leaf in its block's `external_lane_export_root`: SHA3-384
    /// over the leaf domain, then each of the PFTL chain id, the lane id, the
    /// export nonce (8 bytes big-endian) and the packet digest (48 bytes),
    /// each after a zero byte.
    pub fn export_root_leaf(&self, chain_id: &str) -> Result<[u8; 48], String> {
        let digest = self.digest()?;
        let mut hasher = Sha3_384::new();
        hasher.update(EXTERNAL_LANE_EXPORT_ROOT_LEAF_DOMAIN_V1.as_bytes());
        for part in [
            chain_id.as_bytes(),
            self.lane_id.as_bytes(),
            &self.export_nonce.to_be_bytes(),
            &digest,
        ] {
            hasher.update([0_u8]);
            hasher.update(part);
        }
        Ok(hasher.finalize().into())
    }
}

/// The header root over a block's export leaves, in execution order across
/// all lanes: the bridge-exit tree (`bridge_exit_tree_root_v1`), whose empty
/// value is `bridge_exit_empty_root_v1()`. 96 lower-case hex digits.
pub fn external_lane_export_root_v1(leaves: &[[u8; 48]]) -> Result<String, String> {
    bridge_exit_tree_root_v1(&leaves.iter().map(|leaf| leaf.to_vec()).collect::<Vec<_>>())
}

/// Membership of one export leaf in its block's export root.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalLaneExportRootProofV1 {
    /// The leaf, 96 lower-case hex digits.
    pub leaf: String,
    pub leaf_index: u64,
    pub leaf_count: u64,
    pub siblings: Vec<String>,
}

pub fn external_lane_export_root_proof_v1(
    leaves: &[[u8; 48]],
    leaf_index: usize,
) -> Result<ExternalLaneExportRootProofV1, String> {
    let commitments = leaves.iter().map(|leaf| leaf.to_vec()).collect::<Vec<_>>();
    let siblings = bridge_exit_tree_siblings_v1(&commitments, leaf_index)?;
    Ok(ExternalLaneExportRootProofV1 {
        leaf: bytes_to_hex(&leaves[leaf_index]),
        leaf_index: leaf_index as u64,
        leaf_count: leaves.len() as u64,
        siblings,
    })
}

pub fn verify_external_lane_export_root_proof_v1(
    expected_root: &str,
    proof: &ExternalLaneExportRootProofV1,
) -> Result<(), String> {
    validate_lower_hex_len("external_lane_export_root_proof.leaf", &proof.leaf, 96)?;
    verify_bridge_exit_tree_path_v1(
        expected_root,
        &pfusdc_decode_hex(&proof.leaf)?,
        proof.leaf_index,
        proof.leaf_count,
        &proof.siblings,
    )
}

/// The external-lane export-root activation height of a chain: its genesis
/// value, else the earliest unpaused governed one.
pub fn external_lane_export_root_activation_height(
    genesis: &Genesis,
    governance: &GovernanceState,
) -> Option<u64> {
    genesis
        .external_lane_export_root_activation_height
        .or_else(|| governance.external_lane_export_root_activation_height())
}

pub fn external_lane_export_root_active_at(
    genesis: &Genesis,
    governance: &GovernanceState,
    height: u64,
) -> bool {
    external_lane_export_root_activation_height(genesis, governance)
        .is_some_and(|activation| height >= activation)
}

/// Every value a Base verifier and its deployment pin for one lane, all from
/// one genesis. Hashes are 96 lower-case hex digits; Keccak-256 commitments are
/// `0x` and 64 digits.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalLaneVerifierPinsV1 {
    pub lane_id: String,
    pub pftl_chain_id: String,
    pub asset_id: String,
    pub route_profile: ExternalLaneRouteProfileV1,
    pub route_profile_hash: String,
    pub asset_id_commitment: String,
    pub route_profile_hash_commitment: String,
    pub pftl_chain_id_hash: String,
    pub pftl_genesis_hash: String,
    pub pftl_genesis_hash_commitment: String,
}

fn keccak_hex(bytes: &[u8]) -> String {
    format!("0x{}", bytes_to_hex(&Keccak256::digest(bytes)))
}

impl Genesis {
    /// The verifier pins of lane `lane_id`, from this genesis only: its chain
    /// id, its hash (`postfiat.genesis.v1` over its JSON) and the lane's
    /// policy. Fails closed on an invalid genesis or an unknown lane.
    pub fn external_lane_verifier_pins(
        &self,
        lane_id: &str,
    ) -> Result<ExternalLaneVerifierPinsV1, String> {
        self.validate()?;
        let lane = self
            .external_lanes
            .as_deref()
            .unwrap_or_default()
            .iter()
            .find(|lane| lane.id == lane_id)
            .ok_or_else(|| format!("genesis has no external lane `{lane_id}`"))?;
        let route_profile = lane.route_profile(&self.chain_id)?;
        let route_profile_hash = route_profile.profile_hash()?;
        let genesis_hash = hash_hex_domain(
            "postfiat.genesis.v1",
            self.to_json()
                .map_err(|error| format!("encode genesis: {error}"))?
                .as_bytes(),
        );
        Ok(ExternalLaneVerifierPinsV1 {
            lane_id: lane.id.clone(),
            pftl_chain_id: self.chain_id.clone(),
            asset_id: route_profile.asset_id.clone(),
            asset_id_commitment: keccak_hex(&pfusdc_decode_hex(&route_profile.asset_id)?),
            route_profile_hash_commitment: keccak_hex(&pfusdc_decode_hex(&route_profile_hash)?),
            route_profile,
            route_profile_hash,
            pftl_chain_id_hash: keccak_hex(self.chain_id.as_bytes()),
            pftl_genesis_hash_commitment: keccak_hex(&pfusdc_decode_hex(&genesis_hash)?),
            pftl_genesis_hash: genesis_hash,
        })
    }
}
