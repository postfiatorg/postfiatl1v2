// The Base half of Phase 3: what the Base finality verifier
// (`PFTLFinalityVerifierV1`) pins for a lane.

pub const EXTERNAL_LANE_ROUTE_PROFILE_SCHEMA_V1: &str = "postfiat.external_lane.route_profile.v1";
pub const EXTERNAL_LANE_ROUTE_PROFILE_HASH_DOMAIN_V1: &str =
    "postfiat.external_lane.route_profile_hash.v1";
pub const EXTERNAL_LANE_ASSET_ID_DOMAIN_V1: &str = "postfiat.external_lane.asset_id.v1";
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
    pub fn canonical_preimage(&self) -> String {
        format!(
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
        )
    }

    /// SHA3-384 over the domain tag, a zero byte and the preimage: 48 bytes,
    /// as `VaultBridgeRouteProfileV1::profile_hash`.
    pub fn profile_hash(&self) -> String {
        hash_hex_domain(
            EXTERNAL_LANE_ROUTE_PROFILE_HASH_DOMAIN_V1,
            self.canonical_preimage().as_bytes(),
        )
    }
}

impl GenesisExternalLane {
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

    /// The lane's route profile, every field from genesis.
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
