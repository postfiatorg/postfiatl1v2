use super::*;
use std::fs;
use std::io::Read;

pub const PFTL_SWAP_QUOTE_SCHEMA_V1: &str = "postfiat.pftl_swap.quote.v1";
pub const PFTL_SWAP_INTENT_SCHEMA_V1: &str = "postfiat.pftl_swap.intent.v1";
pub const PFTL_SWAP_SIGNED_INTENT_SCHEMA_V1: &str = "postfiat.pftl_swap.signed_intent.v1";
pub const PFTL_SWAP_JOURNAL_SCHEMA_V1: &str = "postfiat.pftl_swap.journal.v1";
pub const PFTL_SWAP_QUOTE_STORE_SCHEMA_V1: &str = "postfiat.pftl_swap.quote_store.v1";
pub const PFTL_SWAP_INTENT_SIGNATURE_CONTEXT_V1: &[u8] = b"postfiat.pftl_swap.intent.v1";

const PFTL_SWAP_MAX_ID_BYTES: usize = 128;
const PFTL_SWAP_MAX_REFERENCE_BYTES: usize = 256;
const PFTL_SWAP_MAX_JOURNAL_ENTRIES: usize = 4_096;
const PFTL_SWAP_MAX_JOURNAL_TRANSITIONS: usize = 64;
const PFTL_SWAP_MAX_TIMING_STAGES: usize = 64;
const PFTL_SWAP_MAX_REASON_BYTES: usize = 256;
const PFTL_SWAP_MAX_DURABLE_FILE_BYTES: usize = 32 << 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PftlSwapDirection {
    Issue,
    Redeem,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PftlSwapOutputMode {
    Private,
    Transparent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlSwapQuoteRequestV1 {
    pub direction: PftlSwapDirection,
    pub nav_amount_atoms: u64,
    pub output_mode: PftlSwapOutputMode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PftlSwapQuoteOptions {
    pub data_dir: PathBuf,
    pub route_id: String,
    pub request: PftlSwapQuoteRequestV1,
    pub quote_ttl_blocks: u64,
    pub maximum_fee_atoms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlSwapQuoteV1 {
    pub schema: String,
    pub quote_id: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub route_id: String,
    pub direction: PftlSwapDirection,
    pub output_mode: PftlSwapOutputMode,
    pub nav_amount_atoms: u64,
    pub input_asset_id: String,
    pub input_amount_atoms: u64,
    pub output_asset_id: String,
    pub output_amount_atoms: u64,
    pub base_settlement_atoms: u64,
    pub spread_atoms: u64,
    pub maximum_fee_atoms: u64,
    pub route_epoch: u64,
    pub policy_epoch: u64,
    pub policy_hash: String,
    pub pricing_nav_epoch: u64,
    pub pricing_reserve_packet_hash: String,
    pub quote_height: u64,
    pub quote_block_id: String,
    pub state_root: String,
    pub orchard_root: String,
    pub route_state_hash: String,
    pub expiry_height: u64,
    pub created_at_unix_ms: u64,
}

impl PftlSwapQuoteV1 {
    pub fn validate(&self) -> io::Result<()> {
        if self.schema != PFTL_SWAP_QUOTE_SCHEMA_V1
            || !pftl_swap_bounded_id(&self.chain_id)
            || !pftl_swap_bounded_id(&self.route_id)
            || !pftl_swap_lower_hex(&self.quote_id, 96)
            || !pftl_swap_lower_hex(&self.genesis_hash, 96)
            || !pftl_swap_lower_hex(&self.input_asset_id, 96)
            || !pftl_swap_lower_hex(&self.output_asset_id, 96)
            || !pftl_swap_lower_hex(&self.policy_hash, 96)
            || !pftl_swap_lower_hex(&self.pricing_reserve_packet_hash, 96)
            || !pftl_swap_lower_hex(&self.quote_block_id, 96)
            || !pftl_swap_lower_hex(&self.state_root, 96)
            || !pftl_swap_lower_hex(&self.orchard_root, 64)
            || !pftl_swap_lower_hex(&self.route_state_hash, 96)
            || self.nav_amount_atoms == 0
            || self.input_amount_atoms == 0
            || self.output_amount_atoms == 0
            || self.base_settlement_atoms == 0
            || self.maximum_fee_atoms == 0
            || self.expiry_height <= self.quote_height
            || self.created_at_unix_ms == 0
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PFTL swap quote has invalid bounded fields",
            ));
        }
        let expected = pftl_swap_quote_id(self)?;
        if self.quote_id != expected {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PFTL swap quote id mismatch",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlSwapQuoteStoreV1 {
    pub schema: String,
    pub quotes: BTreeMap<String, PftlSwapQuoteV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlSwapStateIdentityV1 {
    pub height: u64,
    pub block_id: String,
    pub state_root: String,
    pub orchard_root: String,
}

impl Default for PftlSwapQuoteStoreV1 {
    fn default() -> Self {
        Self {
            schema: PFTL_SWAP_QUOTE_STORE_SCHEMA_V1.to_string(),
            quotes: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlSwapIntentV1 {
    pub schema: String,
    pub chain_id: String,
    pub genesis_hash: String,
    pub protocol_version: u32,
    pub principal: String,
    pub controlled_wallet_id: String,
    pub route_id: String,
    pub direction: PftlSwapDirection,
    pub output_mode: PftlSwapOutputMode,
    pub input_reference: String,
    pub input_amount_atoms: u64,
    pub minimum_output_amount_atoms: u64,
    pub maximum_fee_atoms: u64,
    pub quote_id: String,
    pub pricing_nav_epoch: u64,
    pub policy_hash: String,
    pub expiry_height: u64,
    pub idempotency_key: String,
}

impl PftlSwapIntentV1 {
    pub fn signing_bytes(&self) -> io::Result<Vec<u8>> {
        self.validate_bounded_fields()?;
        serde_json::to_vec(self).map_err(invalid_data)
    }

    fn validate_bounded_fields(&self) -> io::Result<()> {
        if self.schema != PFTL_SWAP_INTENT_SCHEMA_V1
            || !pftl_swap_postfiat_address(&self.principal)
            || !pftl_swap_bounded_id(&self.chain_id)
            || !pftl_swap_lower_hex(&self.genesis_hash, 96)
            || !pftl_swap_bounded_id(&self.controlled_wallet_id)
            || !pftl_swap_bounded_id(&self.route_id)
            || !pftl_swap_bounded_reference(&self.input_reference)
            || !pftl_swap_bounded_id(&self.idempotency_key)
            || !self
                .idempotency_key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
            || self.input_amount_atoms == 0
            || self.minimum_output_amount_atoms == 0
            || self.maximum_fee_atoms == 0
            || !pftl_swap_lower_hex(&self.quote_id, 96)
            || !pftl_swap_lower_hex(&self.policy_hash, 96)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "PFTL swap intent has invalid bounded fields",
            ));
        }
        Ok(())
    }

    pub fn validate_against_quote(
        &self,
        quote: &PftlSwapQuoteV1,
        expected_wallet_id: &str,
        execution_height: u64,
    ) -> io::Result<()> {
        self.validate_bounded_fields()?;
        quote.validate()?;
        let expected_input = quote.input_amount_atoms;
        if self.chain_id != quote.chain_id
            || self.genesis_hash != quote.genesis_hash
            || self.protocol_version != quote.protocol_version
            || self.controlled_wallet_id != expected_wallet_id
            || self.route_id != quote.route_id
            || self.direction != quote.direction
            || self.output_mode != quote.output_mode
            || self.input_amount_atoms != expected_input
            || self.minimum_output_amount_atoms > quote.output_amount_atoms
            || self.maximum_fee_atoms < quote.maximum_fee_atoms
            || self.quote_id != quote.quote_id
            || self.pricing_nav_epoch != quote.pricing_nav_epoch
            || self.policy_hash != quote.policy_hash
            || self.expiry_height != quote.expiry_height
            || execution_height > self.expiry_height
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "signed PFTL swap intent does not match the quote or execution limits",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SignedPftlSwapIntentV1 {
    pub schema: String,
    pub intent: PftlSwapIntentV1,
    pub algorithm_id: String,
    pub public_key_hex: String,
    pub signature_hex: String,
}

impl SignedPftlSwapIntentV1 {
    pub fn verify(&self) -> io::Result<()> {
        if self.schema != PFTL_SWAP_SIGNED_INTENT_SCHEMA_V1
            || self.algorithm_id != ML_DSA_65_ALGORITHM
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "PFTL swap intent signature metadata is invalid",
            ));
        }
        let public_key = hex_to_bytes(&self.public_key_hex).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid intent public key: {error}"),
            )
        })?;
        let signature = hex_to_bytes(&self.signature_hex).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid intent signature: {error}"),
            )
        })?;
        ml_dsa_65_validate_public_key(&public_key).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid intent public key: {error}"),
            )
        })?;
        if address_from_public_key(&public_key) != self.intent.principal
            || !ml_dsa_65_verify_with_context(
                &public_key,
                &self.intent.signing_bytes()?,
                &signature,
                PFTL_SWAP_INTENT_SIGNATURE_CONTEXT_V1,
            )
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "PFTL swap intent signature verification failed",
            ));
        }
        Ok(())
    }

    pub fn intent_hash(&self) -> io::Result<String> {
        Ok(hash_hex(
            "postfiat.pftl_swap.signed_intent.v1",
            &serde_json::to_vec(self).map_err(invalid_data)?,
        ))
    }
}

pub fn sign_pftl_swap_intent_with_key_file(
    key_file_path: &Path,
    intent: PftlSwapIntentV1,
) -> io::Result<SignedPftlSwapIntentV1> {
    let DevKeyFile {
        algorithm_id,
        address,
        public_key_hex,
        private_key_hex,
    } = read_key_file(key_file_path)?;
    if intent.principal != address {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "PFTL swap intent principal does not match the signing key",
        ));
    }
    let private_key_hex = Zeroizing::new(private_key_hex);
    let private_key = Zeroizing::new(hex_to_bytes(private_key_hex.as_str()).map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("PFTL swap signing key has invalid private key hex: {error}"),
        )
    })?);
    let signature = ml_dsa_65_sign_with_context(
        &private_key,
        &intent.signing_bytes()?,
        PFTL_SWAP_INTENT_SIGNATURE_CONTEXT_V1,
    )
    .map_err(invalid_data)?;
    let signed = SignedPftlSwapIntentV1 {
        schema: PFTL_SWAP_SIGNED_INTENT_SCHEMA_V1.to_string(),
        intent,
        algorithm_id,
        public_key_hex,
        signature_hex: bytes_to_hex(&signature),
    };
    signed.verify()?;
    Ok(signed)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum PftlSwapJournalState {
    Journaled,
    Proving,
    Prepared,
    Published,
    Committed,
    Rejected,
    FailedPrepublish,
    InterruptedPrepublish,
}

impl PftlSwapJournalState {
    fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Committed | Self::Rejected | Self::FailedPrepublish
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlSwapJournalTransition {
    pub state: PftlSwapJournalState,
    pub at_unix_ms: u64,
    #[serde(default)]
    pub at_monotonic_ns: u64,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlSwapTimingV1 {
    pub schema: String,
    pub recorded_at_unix_ms: u64,
    pub stages_ns: BTreeMap<String, u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlSwapJournalEntry {
    pub swap_id: String,
    pub idempotency_key: String,
    pub intent_hash: String,
    pub quote_id: String,
    pub principal: String,
    pub controlled_wallet_id: String,
    pub input_reference_hash: String,
    pub direction: PftlSwapDirection,
    pub input_amount_atoms: u64,
    pub minimum_output_amount_atoms: u64,
    pub state: PftlSwapJournalState,
    pub batch_hash: Option<String>,
    pub committed_height: Option<u64>,
    pub certificate_ref: Option<String>,
    pub transitions: Vec<PftlSwapJournalTransition>,
    #[serde(default)]
    pub timing: Option<PftlSwapTimingV1>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PftlSwapJournalV1 {
    pub schema: String,
    pub entries: BTreeMap<String, PftlSwapJournalEntry>,
}

impl Default for PftlSwapJournalV1 {
    fn default() -> Self {
        Self {
            schema: PFTL_SWAP_JOURNAL_SCHEMA_V1.to_string(),
            entries: BTreeMap::new(),
        }
    }
}

pub fn build_pftl_swap_quote(options: PftlSwapQuoteOptions) -> io::Result<PftlSwapQuoteV1> {
    if !pftl_swap_bounded_id(&options.route_id)
        || options.quote_ttl_blocks == 0
        || options.maximum_fee_atoms == 0
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "quote TTL and maximum fee must be nonzero",
        ));
    }
    let store = NodeStore::new(&options.data_dir);
    let genesis = store.read_genesis()?;
    let ledger = store.read_ledger()?;
    let shielded = store.read_shielded()?;
    let bridge = store.read_bridge()?;
    let governance = store.read_governance()?;
    let ordered_batches = store.read_ordered_batches()?;
    let tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let execution_height = tip
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "block height overflow"))?;
    let route = ledger
        .pftl_uniswap_routes
        .iter()
        .find(|route| route.route_id == options.route_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PFTL swap route missing"))?;
    route.validate().map_err(|error| {
        io::Error::new(
            io::ErrorKind::InvalidData,
            format!("invalid PFTL swap route: {error}"),
        )
    })?;
    if route.paused || !route.live_value_enabled {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "PFTL swap route is not live",
        ));
    }
    let v2 = route
        .v2
        .as_ref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "PFTL swap route is not v2"))?;
    let policy = &v2.primary_market_policy;
    let nav_amount = options.request.nav_amount_atoms;
    if nav_amount < policy.min_order_atoms
        || nav_amount > policy.max_order_atoms
        || execution_height < policy.valid_from_height
        || execution_height > policy.expires_at_height
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap amount or execution height is outside policy bounds",
        ));
    }
    let used_capacity = match options.request.direction {
        PftlSwapDirection::Issue => v2.issue_capacity_used_atoms,
        PftlSwapDirection::Redeem => v2.redeem_capacity_used_atoms,
    };
    let capacity = match options.request.direction {
        PftlSwapDirection::Issue => policy.issue_capacity_atoms,
        PftlSwapDirection::Redeem => policy.redeem_capacity_atoms,
    };
    if used_capacity
        .checked_add(nav_amount)
        .is_none_or(|after| after > capacity)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap exceeds remaining route capacity",
        ));
    }
    let native_definition = ledger
        .asset_definition(&route.native_nav_asset_id)
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "NAV asset definition missing")
        })?;
    let settlement_definition = ledger
        .asset_definition(&route.settlement_asset_id)
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                "settlement asset definition missing",
            )
        })?;
    let nav_asset = ledger
        .nav_asset(&route.native_nav_asset_id)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "pricing NAV asset missing"))?;
    let settlement_nav_asset = ledger
        .nav_asset(&route.settlement_asset_id)
        .ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "settlement NAV asset missing")
        })?;
    if nav_asset.finalized_epoch != policy.pricing_nav_epoch
        || nav_asset.finalized_reserve_packet_hash != policy.pricing_reserve_packet_hash
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL swap policy is not pinned to the finalized NAV asset",
        ));
    }
    let nav_fresh_until = nav_asset
        .finalized_at_height
        .checked_add(policy.max_nav_age_blocks)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "NAV freshness overflow"))?;
    if execution_height > nav_fresh_until {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap NAV mark is stale",
        ));
    }
    let base_settlement_atoms = required_vault_bridge_settlement_atoms(
        nav_amount,
        native_definition.precision,
        nav_asset.nav_per_unit,
        &nav_asset.valuation_unit,
        &settlement_nav_asset.valuation_unit,
        settlement_definition.precision,
    )
    .map_err(|(code, message)| {
        io::Error::new(io::ErrorKind::InvalidData, format!("{code}: {message}"))
    })?;
    let (input_asset_id, input_amount_atoms, output_asset_id, output_amount_atoms, spread_atoms) =
        match options.request.direction {
            PftlSwapDirection::Issue => {
                let due = pftl_swap_mul_div_ceil(
                    base_settlement_atoms,
                    policy.issue_multiplier_bps,
                    postfiat_types::PFTL_UNISWAP_BPS_DENOMINATOR,
                )?;
                (
                    route.settlement_asset_id.clone(),
                    due,
                    route.native_nav_asset_id.clone(),
                    nav_amount,
                    due.checked_sub(base_settlement_atoms).ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "issue spread underflow")
                    })?,
                )
            }
            PftlSwapDirection::Redeem => {
                let output = pftl_swap_mul_div_floor(
                    base_settlement_atoms,
                    policy.redeem_multiplier_bps,
                    postfiat_types::PFTL_UNISWAP_BPS_DENOMINATOR,
                )?;
                (
                    route.native_nav_asset_id.clone(),
                    nav_amount,
                    route.settlement_asset_id.clone(),
                    output,
                    base_settlement_atoms.checked_sub(output).ok_or_else(|| {
                        io::Error::new(io::ErrorKind::InvalidData, "redeem spread underflow")
                    })?,
                )
            }
        };
    let orchard_root = shielded
        .orchard
        .as_ref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Orchard pool missing"))
        .and_then(orchard_pool_current_root)?;
    let state_root = replicated_state_root(
        &genesis,
        &governance,
        &ledger,
        &ordered_batches,
        &shielded,
        &bridge,
    )?;
    let ttl_expiry = tip
        .height
        .checked_add(options.quote_ttl_blocks)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "quote expiry overflow"))?;
    let expiry_height = ttl_expiry
        .min(policy.expires_at_height)
        .min(nav_fresh_until);
    if expiry_height <= tip.height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap quote would already be expired",
        ));
    }
    let mut quote = PftlSwapQuoteV1 {
        schema: PFTL_SWAP_QUOTE_SCHEMA_V1.to_string(),
        quote_id: String::new(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        route_id: route.route_id.clone(),
        direction: options.request.direction,
        output_mode: options.request.output_mode,
        nav_amount_atoms: nav_amount,
        input_asset_id,
        input_amount_atoms,
        output_asset_id,
        output_amount_atoms,
        base_settlement_atoms,
        spread_atoms,
        maximum_fee_atoms: options.maximum_fee_atoms,
        route_epoch: v2.route_epoch,
        policy_epoch: policy.policy_epoch,
        policy_hash: policy.policy_hash.clone(),
        pricing_nav_epoch: policy.pricing_nav_epoch,
        pricing_reserve_packet_hash: policy.pricing_reserve_packet_hash.clone(),
        quote_height: tip.height,
        quote_block_id: tip.block_hash,
        state_root,
        orchard_root,
        route_state_hash: pftl_uniswap_route_state_hash(route),
        expiry_height,
        created_at_unix_ms: pftl_swap_now_unix_ms()?,
    };
    quote.quote_id = pftl_swap_quote_id(&quote)?;
    quote.validate()?;
    Ok(quote)
}

/// Rebuilds a quote from the current replicated state and requires every
/// state-derived field to remain byte-for-byte identical. This is deliberately
/// strict: any intervening block changes the pinned execution anchor and
/// requires the caller to obtain a fresh quote.
pub fn revalidate_pftl_swap_quote_current(
    data_dir: &Path,
    quote: &PftlSwapQuoteV1,
) -> io::Result<()> {
    quote.validate()?;
    let quote_ttl_blocks = quote
        .expiry_height
        .checked_sub(quote.quote_height)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "quote expiry underflow"))?;
    let mut current = build_pftl_swap_quote(PftlSwapQuoteOptions {
        data_dir: data_dir.to_path_buf(),
        route_id: quote.route_id.clone(),
        request: PftlSwapQuoteRequestV1 {
            direction: quote.direction,
            nav_amount_atoms: quote.nav_amount_atoms,
            output_mode: quote.output_mode,
        },
        quote_ttl_blocks,
        maximum_fee_atoms: quote.maximum_fee_atoms,
    })?;
    current.created_at_unix_ms = quote.created_at_unix_ms;
    current.quote_id = quote.quote_id.clone();
    if current != *quote {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "PFTL swap quote no longer matches the exact current state",
        ));
    }
    Ok(())
}

/// Rechecks the quote's governed economics and capacity at the current
/// execution height without requiring the chain tip to remain frozen since
/// quote creation. Orchard/tip identity is pinned separately around proof
/// construction by `capture_pftl_swap_state_identity`.
pub fn revalidate_pftl_swap_quote_for_execution(
    data_dir: &Path,
    quote: &PftlSwapQuoteV1,
) -> io::Result<()> {
    quote.validate()?;
    let current_identity = capture_pftl_swap_state_identity(data_dir)?;
    let next_height = current_identity
        .height
        .checked_add(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "height overflow"))?;
    if next_height > quote.expiry_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap quote is expired at the next execution height",
        ));
    }
    let remaining_ttl = quote
        .expiry_height
        .checked_sub(current_identity.height)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "quote expiry underflow"))?;
    let current = build_pftl_swap_quote(PftlSwapQuoteOptions {
        data_dir: data_dir.to_path_buf(),
        route_id: quote.route_id.clone(),
        request: PftlSwapQuoteRequestV1 {
            direction: quote.direction,
            nav_amount_atoms: quote.nav_amount_atoms,
            output_mode: quote.output_mode,
        },
        quote_ttl_blocks: remaining_ttl,
        maximum_fee_atoms: quote.maximum_fee_atoms,
    })?;
    let economics_match = current.chain_id == quote.chain_id
        && current.genesis_hash == quote.genesis_hash
        && current.protocol_version == quote.protocol_version
        && current.route_id == quote.route_id
        && current.direction == quote.direction
        && current.output_mode == quote.output_mode
        && current.nav_amount_atoms == quote.nav_amount_atoms
        && current.input_asset_id == quote.input_asset_id
        && current.input_amount_atoms == quote.input_amount_atoms
        && current.output_asset_id == quote.output_asset_id
        && current.output_amount_atoms == quote.output_amount_atoms
        && current.base_settlement_atoms == quote.base_settlement_atoms
        && current.spread_atoms == quote.spread_atoms
        && current.maximum_fee_atoms == quote.maximum_fee_atoms
        && current.route_epoch == quote.route_epoch
        && current.policy_epoch == quote.policy_epoch
        && current.policy_hash == quote.policy_hash
        && current.pricing_nav_epoch == quote.pricing_nav_epoch
        && current.pricing_reserve_packet_hash == quote.pricing_reserve_packet_hash
        && current.expiry_height == quote.expiry_height;
    if !economics_match {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "PFTL swap quote economics or governed policy changed before execution",
        ));
    }
    Ok(())
}

pub fn capture_pftl_swap_state_identity(data_dir: &Path) -> io::Result<PftlSwapStateIdentityV1> {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis()?;
    let governance = store.read_governance()?;
    let ledger = store.read_ledger()?;
    let ordered_batches = store.read_ordered_batches()?;
    let shielded = store.read_shielded()?;
    let bridge = store.read_bridge()?;
    let tip = read_chain_tip_or_reconstruct_for_genesis(&store, &genesis)?;
    let orchard_root = shielded
        .orchard
        .as_ref()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Orchard pool missing"))
        .and_then(orchard_pool_current_root)?;
    Ok(PftlSwapStateIdentityV1 {
        height: tip.height,
        block_id: tip.block_hash,
        state_root: replicated_state_root(
            &genesis,
            &governance,
            &ledger,
            &ordered_batches,
            &shielded,
            &bridge,
        )?,
        orchard_root,
    })
}

pub fn load_pftl_swap_quote_store(path: &Path) -> io::Result<PftlSwapQuoteStoreV1> {
    if !path.exists() {
        return Ok(PftlSwapQuoteStoreV1::default());
    }
    validate_private_file_permissions(path, "PFTL swap quote store")?;
    let store: PftlSwapQuoteStoreV1 =
        serde_json::from_slice(&read_pftl_swap_bounded_file(path)?).map_err(invalid_data)?;
    validate_pftl_swap_quote_store(&store)?;
    Ok(store)
}

pub fn store_pftl_swap_quote(
    path: &Path,
    quote: &PftlSwapQuoteV1,
    current_height: u64,
) -> io::Result<()> {
    quote.validate()?;
    if quote.expiry_height < current_height {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap quote is already expired",
        ));
    }
    let mut store = load_pftl_swap_quote_store(path)?;
    store
        .quotes
        .retain(|_, existing| existing.expiry_height >= current_height);
    if store.quotes.len() >= PFTL_SWAP_MAX_JOURNAL_ENTRIES
        && !store.quotes.contains_key(&quote.quote_id)
    {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "PFTL swap quote store has reached its bounded capacity",
        ));
    }
    if let Some(existing) = store.quotes.get(&quote.quote_id) {
        if existing != quote {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "PFTL swap quote id is bound to different quote bytes",
            ));
        }
        return Ok(());
    }
    store.quotes.insert(quote.quote_id.clone(), quote.clone());
    persist_pftl_swap_quote_store(path, &store)
}

pub fn find_pftl_swap_quote(path: &Path, quote_id: &str) -> io::Result<PftlSwapQuoteV1> {
    if !pftl_swap_lower_hex(quote_id, 96) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap quote id must be 48-byte lowercase hex",
        ));
    }
    load_pftl_swap_quote_store(path)?
        .quotes
        .get(quote_id)
        .cloned()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "PFTL swap quote is unknown"))
}

fn persist_pftl_swap_quote_store(path: &Path, store: &PftlSwapQuoteStoreV1) -> io::Result<()> {
    validate_pftl_swap_quote_store(store)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        set_pftl_swap_private_directory_permissions(parent)?;
    }
    let mut text = serde_json::to_string_pretty(store).map_err(invalid_data)?;
    text.push('\n');
    atomic_write(path, text)?;
    set_private_file_permissions(path)
}

fn validate_pftl_swap_quote_store(store: &PftlSwapQuoteStoreV1) -> io::Result<()> {
    if store.schema != PFTL_SWAP_QUOTE_STORE_SCHEMA_V1
        || store.quotes.len() > PFTL_SWAP_MAX_JOURNAL_ENTRIES
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL swap quote store schema or capacity is invalid",
        ));
    }
    for (quote_id, quote) in &store.quotes {
        quote.validate()?;
        if quote_id != &quote.quote_id {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PFTL swap quote store key mismatch",
            ));
        }
    }
    Ok(())
}

pub fn load_pftl_swap_journal(path: &Path) -> io::Result<PftlSwapJournalV1> {
    if !path.exists() {
        return Ok(PftlSwapJournalV1::default());
    }
    validate_private_file_permissions(path, "PFTL swap journal")?;
    let journal: PftlSwapJournalV1 =
        serde_json::from_slice(&read_pftl_swap_bounded_file(path)?).map_err(invalid_data)?;
    if journal.schema != PFTL_SWAP_JOURNAL_SCHEMA_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL swap journal schema mismatch",
        ));
    }
    validate_pftl_swap_journal(&journal)?;
    Ok(journal)
}

pub fn persist_pftl_swap_journal(path: &Path, journal: &PftlSwapJournalV1) -> io::Result<()> {
    if journal.schema != PFTL_SWAP_JOURNAL_SCHEMA_V1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap journal schema mismatch",
        ));
    }
    validate_pftl_swap_journal(journal)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
        set_pftl_swap_private_directory_permissions(parent)?;
    }
    let mut text = serde_json::to_string_pretty(journal).map_err(invalid_data)?;
    text.push('\n');
    atomic_write(path, text)?;
    set_private_file_permissions(path)
}

#[cfg(unix)]
fn set_pftl_swap_private_directory_permissions(path: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn set_pftl_swap_private_directory_permissions(_path: &Path) -> io::Result<()> {
    Ok(())
}

pub fn journal_pftl_swap_intent(
    path: &Path,
    quote: &PftlSwapQuoteV1,
    signed_intent: &SignedPftlSwapIntentV1,
) -> io::Result<(PftlSwapJournalEntry, bool)> {
    signed_intent.verify()?;
    journal_verified_pftl_swap_intent(path, quote, signed_intent)
}

pub fn find_pftl_swap_intent_replay(
    path: &Path,
    signed_intent: &SignedPftlSwapIntentV1,
) -> io::Result<Option<PftlSwapJournalEntry>> {
    signed_intent.verify()?;
    let Some(existing) = load_pftl_swap_journal(path)?
        .entries
        .get(&signed_intent.intent.idempotency_key)
        .cloned()
    else {
        return Ok(None);
    };
    if existing.intent_hash != signed_intent.intent_hash()? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "idempotency key is already bound to a different signed intent",
        ));
    }
    Ok(Some(existing))
}

fn journal_verified_pftl_swap_intent(
    path: &Path,
    quote: &PftlSwapQuoteV1,
    signed_intent: &SignedPftlSwapIntentV1,
) -> io::Result<(PftlSwapJournalEntry, bool)> {
    let intent_hash = signed_intent.intent_hash()?;
    let key = signed_intent.intent.idempotency_key.clone();
    let mut journal = load_pftl_swap_journal(path)?;
    if let Some(existing) = journal.entries.get(&key) {
        if existing.intent_hash == intent_hash {
            return Ok((existing.clone(), true));
        }
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            "idempotency key is already bound to a different signed intent",
        ));
    }
    let input_reference_hash = hash_hex(
        "postfiat.pftl_swap.input_reference.v1",
        signed_intent.intent.input_reference.as_bytes(),
    );
    if journal.entries.values().any(|entry| {
        !entry.state.is_terminal() && entry.input_reference_hash == input_reference_hash
    }) {
        return Err(io::Error::new(
            io::ErrorKind::WouldBlock,
            "PFTL swap input is already reserved by an active intent",
        ));
    }
    if journal.entries.len() >= PFTL_SWAP_MAX_JOURNAL_ENTRIES {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "PFTL swap journal has reached its bounded capacity",
        ));
    }
    let now = pftl_swap_now_unix_ms()?;
    let swap_id = hash_hex(
        "postfiat.pftl_swap.swap_id.v1",
        format!("{key}:{intent_hash}:{}", quote.quote_id).as_bytes(),
    );
    let entry = PftlSwapJournalEntry {
        swap_id,
        idempotency_key: key.clone(),
        intent_hash,
        quote_id: quote.quote_id.clone(),
        principal: signed_intent.intent.principal.clone(),
        controlled_wallet_id: signed_intent.intent.controlled_wallet_id.clone(),
        input_reference_hash,
        direction: signed_intent.intent.direction,
        input_amount_atoms: signed_intent.intent.input_amount_atoms,
        minimum_output_amount_atoms: signed_intent.intent.minimum_output_amount_atoms,
        state: PftlSwapJournalState::Journaled,
        batch_hash: None,
        committed_height: None,
        certificate_ref: None,
        transitions: vec![PftlSwapJournalTransition {
            state: PftlSwapJournalState::Journaled,
            at_unix_ms: now,
            at_monotonic_ns: pftl_swap_now_monotonic_ns()?,
            reason: None,
        }],
        timing: None,
    };
    journal.entries.insert(key, entry.clone());
    persist_pftl_swap_journal(path, &journal)?;
    Ok((entry, false))
}

pub fn authorize_and_journal_pftl_swap_intent(
    path: &Path,
    data_dir: &Path,
    quote: &PftlSwapQuoteV1,
    signed_intent: &SignedPftlSwapIntentV1,
    expected_wallet_id: &str,
    execution_height: u64,
) -> io::Result<(PftlSwapJournalEntry, bool)> {
    signed_intent.verify()?;
    if let Some(existing) = find_pftl_swap_intent_replay(path, signed_intent)? {
        return Ok((existing, true));
    }
    signed_intent
        .intent
        .validate_against_quote(quote, expected_wallet_id, execution_height)?;
    revalidate_pftl_swap_quote_for_execution(data_dir, quote)?;
    journal_verified_pftl_swap_intent(path, quote, signed_intent)
}

pub fn transition_pftl_swap_journal_entry(
    path: &Path,
    idempotency_key: &str,
    next: PftlSwapJournalState,
    batch_hash: Option<String>,
    committed_height: Option<u64>,
    certificate_ref: Option<String>,
    reason: Option<String>,
) -> io::Result<PftlSwapJournalEntry> {
    if reason
        .as_ref()
        .is_some_and(|value| value.is_empty() || value.len() > PFTL_SWAP_MAX_REASON_BYTES)
        || batch_hash
            .as_ref()
            .is_some_and(|value| !pftl_swap_lower_hex(value, 96))
        || certificate_ref
            .as_ref()
            .is_some_and(|value| !pftl_swap_bounded_id(value))
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap journal transition has invalid bounded fields",
        ));
    }
    let mut journal = load_pftl_swap_journal(path)?;
    let entry = journal.entries.get_mut(idempotency_key).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "PFTL swap journal entry missing")
    })?;
    if entry.state == next {
        if batch_hash
            .as_ref()
            .is_some_and(|value| entry.batch_hash.as_ref() != Some(value))
            || committed_height.is_some_and(|value| entry.committed_height != Some(value))
            || certificate_ref
                .as_ref()
                .is_some_and(|value| entry.certificate_ref.as_ref() != Some(value))
        {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "idempotent journal transition conflicts with durable state",
            ));
        }
        return Ok(entry.clone());
    }
    if !pftl_swap_transition_allowed(entry.state, next) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "invalid PFTL swap journal transition {:?} -> {:?}",
                entry.state, next
            ),
        ));
    }
    if entry.transitions.len() >= PFTL_SWAP_MAX_JOURNAL_TRANSITIONS {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "PFTL swap journal transition history has reached its bounded capacity",
        ));
    }
    if matches!(
        next,
        PftlSwapJournalState::Proving | PftlSwapJournalState::Prepared
    ) && entry.transitions.len() >= PFTL_SWAP_MAX_JOURNAL_TRANSITIONS.saturating_sub(2)
    {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "PFTL swap journal reserves final transitions for interruption and resolution",
        ));
    }
    if next == PftlSwapJournalState::Published
        && entry.transitions.len() >= PFTL_SWAP_MAX_JOURNAL_TRANSITIONS.saturating_sub(1)
    {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "PFTL swap journal reserves its final transition for consensus resolution",
        ));
    }
    entry.state = next;
    if let Some(batch_hash) = batch_hash {
        entry.batch_hash = Some(batch_hash);
    }
    if let Some(height) = committed_height {
        entry.committed_height = Some(height);
    }
    if let Some(certificate_ref) = certificate_ref {
        entry.certificate_ref = Some(certificate_ref);
    }
    if matches!(
        next,
        PftlSwapJournalState::Prepared
            | PftlSwapJournalState::Published
            | PftlSwapJournalState::Committed
            | PftlSwapJournalState::Rejected
    ) && entry.batch_hash.is_none()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "prepared or post-publication journal state requires a batch hash",
        ));
    }
    if next == PftlSwapJournalState::Committed
        && (entry.committed_height.is_none() || entry.certificate_ref.is_none())
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "committed journal state requires height and certificate reference",
        ));
    }
    if matches!(
        next,
        PftlSwapJournalState::Rejected
            | PftlSwapJournalState::FailedPrepublish
            | PftlSwapJournalState::InterruptedPrepublish
    ) && reason.is_none()
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "failed or interrupted journal transition requires a bounded reason",
        ));
    }
    entry.transitions.push(PftlSwapJournalTransition {
        state: next,
        at_unix_ms: pftl_swap_now_unix_ms()?,
        at_monotonic_ns: pftl_swap_now_monotonic_ns()?,
        reason,
    });
    let updated = entry.clone();
    persist_pftl_swap_journal(path, &journal)?;
    Ok(updated)
}

pub fn record_pftl_swap_stage_timings(
    path: &Path,
    idempotency_key: &str,
    stages_ns: &BTreeMap<String, u64>,
) -> io::Result<PftlSwapJournalEntry> {
    if stages_ns.is_empty() || stages_ns.len() > PFTL_SWAP_MAX_TIMING_STAGES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap timing update has an invalid stage count",
        ));
    }
    if stages_ns
        .iter()
        .any(|(stage, elapsed_ns)| !pftl_swap_timing_stage(stage) || *elapsed_ns == 0)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "PFTL swap timing update has invalid bounded fields",
        ));
    }
    let mut journal = load_pftl_swap_journal(path)?;
    let entry = journal.entries.get_mut(idempotency_key).ok_or_else(|| {
        io::Error::new(io::ErrorKind::NotFound, "PFTL swap journal entry missing")
    })?;
    let timing = entry.timing.get_or_insert_with(|| PftlSwapTimingV1 {
        schema: "postfiat.pftl_swap.timing.v1".to_string(),
        recorded_at_unix_ms: 0,
        stages_ns: BTreeMap::new(),
    });
    if timing.stages_ns.len().saturating_add(stages_ns.len()) > PFTL_SWAP_MAX_TIMING_STAGES {
        return Err(io::Error::new(
            io::ErrorKind::StorageFull,
            "PFTL swap timing history has reached its bounded capacity",
        ));
    }
    let mut changed = false;
    for (stage, elapsed_ns) in stages_ns {
        match timing.stages_ns.get(stage) {
            Some(existing) if existing != elapsed_ns => {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "PFTL swap timing stage is already recorded differently",
                ));
            }
            Some(_) => {}
            None => {
                timing.stages_ns.insert(stage.clone(), *elapsed_ns);
                changed = true;
            }
        }
    }
    if !changed {
        return Ok(entry.clone());
    }
    timing.recorded_at_unix_ms = pftl_swap_now_unix_ms()?;
    let updated = entry.clone();
    persist_pftl_swap_journal(path, &journal)?;
    Ok(updated)
}

pub fn recover_pftl_swap_journal(path: &Path) -> io::Result<PftlSwapJournalV1> {
    let mut journal = load_pftl_swap_journal(path)?;
    let now = pftl_swap_now_unix_ms()?;
    let mut changed = false;
    for entry in journal.entries.values_mut() {
        if matches!(
            entry.state,
            PftlSwapJournalState::Journaled
                | PftlSwapJournalState::Proving
                | PftlSwapJournalState::Prepared
        ) {
            if entry.transitions.len() >= PFTL_SWAP_MAX_JOURNAL_TRANSITIONS {
                return Err(io::Error::new(
                    io::ErrorKind::StorageFull,
                    "PFTL swap journal has no transition capacity for crash recovery",
                ));
            }
            entry.state = PftlSwapJournalState::InterruptedPrepublish;
            entry.transitions.push(PftlSwapJournalTransition {
                state: PftlSwapJournalState::InterruptedPrepublish,
                at_unix_ms: now,
                at_monotonic_ns: pftl_swap_now_monotonic_ns()?,
                reason: Some("daemon restarted before publication".to_string()),
            });
            changed = true;
        }
    }
    if changed {
        persist_pftl_swap_journal(path, &journal)?;
    }
    Ok(journal)
}

fn pftl_swap_transition_allowed(current: PftlSwapJournalState, next: PftlSwapJournalState) -> bool {
    use PftlSwapJournalState::*;
    current == next
        || matches!(
            (current, next),
            (Journaled, Proving)
                | (Journaled, FailedPrepublish)
                | (Journaled, InterruptedPrepublish)
                | (Proving, Prepared)
                | (Proving, FailedPrepublish)
                | (Proving, InterruptedPrepublish)
                | (Prepared, Published)
                | (Prepared, FailedPrepublish)
                | (Prepared, InterruptedPrepublish)
                | (InterruptedPrepublish, Proving)
                | (InterruptedPrepublish, FailedPrepublish)
                | (Published, Committed)
                | (Published, Rejected)
        )
}

fn pftl_swap_quote_id(quote: &PftlSwapQuoteV1) -> io::Result<String> {
    let mut unsigned = quote.clone();
    unsigned.quote_id.clear();
    Ok(hash_hex(
        "postfiat.pftl_swap.quote.v1",
        &serde_json::to_vec(&unsigned).map_err(invalid_data)?,
    ))
}

fn validate_pftl_swap_journal(journal: &PftlSwapJournalV1) -> io::Result<()> {
    if journal.entries.len() > PFTL_SWAP_MAX_JOURNAL_ENTRIES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL swap journal exceeds its bounded capacity",
        ));
    }
    for (key, entry) in &journal.entries {
        if key != &entry.idempotency_key
            || !pftl_swap_bounded_id(key)
            || !pftl_swap_lower_hex(&entry.swap_id, 96)
            || !pftl_swap_lower_hex(&entry.intent_hash, 96)
            || !pftl_swap_lower_hex(&entry.quote_id, 96)
            || !pftl_swap_postfiat_address(&entry.principal)
            || !pftl_swap_bounded_id(&entry.controlled_wallet_id)
            || !pftl_swap_lower_hex(&entry.input_reference_hash, 96)
            || entry.input_amount_atoms == 0
            || entry.minimum_output_amount_atoms == 0
            || entry.transitions.is_empty()
            || entry.transitions.len() > PFTL_SWAP_MAX_JOURNAL_TRANSITIONS
            || entry.transitions.last().map(|transition| transition.state) != Some(entry.state)
            || matches!(
                entry.state,
                PftlSwapJournalState::Prepared
                    | PftlSwapJournalState::Published
                    | PftlSwapJournalState::Committed
                    | PftlSwapJournalState::Rejected
            ) && entry.batch_hash.is_none()
            || entry.state == PftlSwapJournalState::Committed
                && (entry.committed_height.is_none() || entry.certificate_ref.is_none())
            || entry
                .batch_hash
                .as_ref()
                .is_some_and(|value| !pftl_swap_lower_hex(value, 96))
            || entry
                .certificate_ref
                .as_ref()
                .is_some_and(|value| !pftl_swap_bounded_id(value))
            || entry.transitions.iter().any(|transition| {
                transition.at_unix_ms == 0
                    || transition.reason.as_ref().is_some_and(|reason| {
                        reason.is_empty() || reason.len() > PFTL_SWAP_MAX_REASON_BYTES
                    })
            })
            || entry.timing.as_ref().is_some_and(|timing| {
                timing.schema != "postfiat.pftl_swap.timing.v1"
                    || timing.recorded_at_unix_ms == 0
                    || timing.stages_ns.is_empty()
                    || timing.stages_ns.len() > PFTL_SWAP_MAX_TIMING_STAGES
                    || timing.stages_ns.iter().any(|(stage, elapsed_ns)| {
                        !pftl_swap_timing_stage(stage) || *elapsed_ns == 0
                    })
            })
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "PFTL swap journal contains invalid bounded fields",
            ));
        }
    }
    Ok(())
}

fn pftl_swap_bounded_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= PFTL_SWAP_MAX_ID_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn pftl_swap_timing_stage(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn read_pftl_swap_bounded_file(path: &Path) -> io::Result<Vec<u8>> {
    let mut file = fs::File::open(path)?;
    if file.metadata()?.len() > PFTL_SWAP_MAX_DURABLE_FILE_BYTES as u64 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL swap durable file exceeds its bounded capacity",
        ));
    }
    let mut bytes = Vec::new();
    Read::by_ref(&mut file)
        .take(PFTL_SWAP_MAX_DURABLE_FILE_BYTES.saturating_add(1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > PFTL_SWAP_MAX_DURABLE_FILE_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "PFTL swap durable file exceeds its bounded capacity",
        ));
    }
    Ok(bytes)
}

fn pftl_swap_bounded_reference(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= PFTL_SWAP_MAX_REFERENCE_BYTES
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn pftl_swap_lower_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
}

fn pftl_swap_postfiat_address(value: &str) -> bool {
    value
        .strip_prefix("pf")
        .is_some_and(|payload| pftl_swap_lower_hex(payload, 40))
}

fn pftl_swap_mul_div_ceil(value: u64, multiplier: u32, denominator: u32) -> io::Result<u64> {
    let numerator = u128::from(value)
        .checked_mul(u128::from(multiplier))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "quote overflow"))?;
    let denominator = u128::from(denominator);
    let result = numerator
        .checked_add(denominator - 1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "quote overflow"))?
        / denominator;
    u64::try_from(result)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "quote exceeds u64"))
}

fn pftl_swap_mul_div_floor(value: u64, multiplier: u32, denominator: u32) -> io::Result<u64> {
    let result = u128::from(value)
        .checked_mul(u128::from(multiplier))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "quote overflow"))?
        / u128::from(denominator);
    u64::try_from(result)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "quote exceeds u64"))
}

fn pftl_swap_now_unix_ms() -> io::Result<u64> {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(invalid_data)?
        .as_millis();
    u64::try_from(millis)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "clock exceeds u64"))
}

fn pftl_swap_now_monotonic_ns() -> io::Result<u64> {
    let mut value = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    let result = unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut value) };
    if result != 0 {
        return Err(io::Error::last_os_error());
    }
    let seconds = u64::try_from(value.tv_sec)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative monotonic clock"))?;
    let nanos = u64::try_from(value.tv_nsec)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "negative monotonic clock"))?;
    seconds
        .checked_mul(1_000_000_000)
        .and_then(|base| base.checked_add(nanos))
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "monotonic clock overflow"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn swap_test_dir(label: &str) -> PathBuf {
        let suffix = hash_hex(
            "postfiat.pftl_swap.test_dir.v1",
            format!(
                "{label}:{}:{}",
                std::process::id(),
                pftl_swap_now_unix_ms().expect("test clock")
            )
            .as_bytes(),
        );
        std::env::temp_dir().join(format!("postfiat-{label}-{}", &suffix[..16]))
    }

    fn signed_fixture(idempotency_key: &str) -> SignedPftlSwapIntentV1 {
        let keypair = ml_dsa_65_keygen_from_seed(&[7_u8; 32]);
        let principal = address_from_public_key(&keypair.public_key);
        let intent = PftlSwapIntentV1 {
            schema: PFTL_SWAP_INTENT_SCHEMA_V1.to_string(),
            chain_id: "postfiat-test".to_string(),
            genesis_hash: "11".repeat(48),
            protocol_version: 1,
            principal,
            controlled_wallet_id: "wallet-1".to_string(),
            route_id: "route-1".to_string(),
            direction: PftlSwapDirection::Issue,
            output_mode: PftlSwapOutputMode::Private,
            input_reference: "account:sequence:9".to_string(),
            input_amount_atoms: 1_005_000,
            minimum_output_amount_atoms: 1_000_000,
            maximum_fee_atoms: 100,
            quote_id: "22".repeat(48),
            pricing_nav_epoch: 3,
            policy_hash: "33".repeat(48),
            expiry_height: 99,
            idempotency_key: idempotency_key.to_string(),
        };
        let signature = ml_dsa_65_sign_with_context(
            &keypair.private_key,
            &intent.signing_bytes().expect("signing bytes"),
            PFTL_SWAP_INTENT_SIGNATURE_CONTEXT_V1,
        )
        .expect("sign intent");
        SignedPftlSwapIntentV1 {
            schema: PFTL_SWAP_SIGNED_INTENT_SCHEMA_V1.to_string(),
            intent,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&keypair.public_key),
            signature_hex: bytes_to_hex(&signature),
        }
    }

    fn quote_fixture() -> PftlSwapQuoteV1 {
        let mut quote = PftlSwapQuoteV1 {
            schema: PFTL_SWAP_QUOTE_SCHEMA_V1.to_string(),
            quote_id: String::new(),
            chain_id: "postfiat-test".to_string(),
            genesis_hash: "11".repeat(48),
            protocol_version: 1,
            route_id: "route-1".to_string(),
            direction: PftlSwapDirection::Issue,
            output_mode: PftlSwapOutputMode::Private,
            nav_amount_atoms: 1_000_000,
            input_asset_id: "44".repeat(48),
            input_amount_atoms: 1_005_000,
            output_asset_id: "55".repeat(48),
            output_amount_atoms: 1_000_000,
            base_settlement_atoms: 1_000_000,
            spread_atoms: 5_000,
            maximum_fee_atoms: 100,
            route_epoch: 2,
            policy_epoch: 3,
            policy_hash: "33".repeat(48),
            pricing_nav_epoch: 3,
            pricing_reserve_packet_hash: "66".repeat(48),
            quote_height: 90,
            quote_block_id: "77".repeat(48),
            state_root: "88".repeat(48),
            orchard_root: "99".repeat(32),
            route_state_hash: "aa".repeat(48),
            expiry_height: 99,
            created_at_unix_ms: 1,
        };
        quote.quote_id = pftl_swap_quote_id(&quote).expect("quote id");
        quote
    }

    #[test]
    fn signed_intent_key_file_helper_binds_the_principal_and_zeroizes_key_bytes() {
        let dir = swap_test_dir("pftl-swap-intent-key-file");
        fs::create_dir_all(&dir).expect("create key directory");
        let key_path = dir.join("wallet-key.json");
        let keypair = ml_dsa_65_keygen_from_seed(&[19_u8; 32]);
        let key_file = DevKeyFile {
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            address: address_from_public_key(&keypair.public_key),
            public_key_hex: bytes_to_hex(&keypair.public_key),
            private_key_hex: bytes_to_hex(&keypair.private_key),
        };
        write_key_file(&key_path, &key_file).expect("write signing key");

        let mut intent = signed_fixture("key-file-signing").intent;
        intent.principal = key_file.address.clone();
        let signed = sign_pftl_swap_intent_with_key_file(&key_path, intent.clone())
            .expect("sign intent from key file");
        signed.verify().expect("verify signed intent");
        assert_eq!(signed.intent, intent);

        let mut mismatched = intent;
        mismatched.principal =
            address_from_public_key(&ml_dsa_65_keygen_from_seed(&[20_u8; 32]).public_key);
        assert_eq!(
            sign_pftl_swap_intent_with_key_file(&key_path, mismatched)
                .expect_err("mismatched principal must fail")
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::remove_dir_all(dir).expect("remove key directory");
    }

    #[test]
    fn signed_intent_binds_quote_and_detects_tampering() {
        let quote = quote_fixture();
        let mut signed = signed_fixture("intent-1");
        signed.intent.quote_id = quote.quote_id.clone();
        let keypair = ml_dsa_65_keygen_from_seed(&[7_u8; 32]);
        signed.signature_hex = bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &keypair.private_key,
                &signed.intent.signing_bytes().expect("signing bytes"),
                PFTL_SWAP_INTENT_SIGNATURE_CONTEXT_V1,
            )
            .expect("sign intent"),
        );
        signed.verify().expect("verify signed intent");
        signed
            .intent
            .validate_against_quote(&quote, "wallet-1", 91)
            .expect("intent matches quote");

        signed.intent.minimum_output_amount_atoms += 1;
        assert!(signed.verify().is_err());
    }

    #[test]
    fn quote_and_intent_reject_noncanonical_identifiers() {
        let mut quote = quote_fixture();
        quote.policy_hash = "AA".repeat(48);
        quote.quote_id = pftl_swap_quote_id(&quote).expect("tampered quote id");
        assert!(quote.validate().is_err());

        let mut signed = signed_fixture("intent-safe");
        signed.intent.idempotency_key = "intent/unsafe".to_string();
        assert!(signed.intent.signing_bytes().is_err());
        signed.intent.idempotency_key = "intent-safe".to_string();
        signed.intent.principal = String::new();
        assert!(signed.intent.signing_bytes().is_err());
    }

    #[test]
    fn journal_is_idempotent_and_reserves_inputs() {
        let root = swap_test_dir("pftl-swap-journal");
        let path = root.join("swap-journal.json");
        let quote = quote_fixture();
        let mut signed = signed_fixture("intent-1");
        signed.intent.quote_id = quote.quote_id.clone();
        let keypair = ml_dsa_65_keygen_from_seed(&[7_u8; 32]);
        signed.signature_hex = bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &keypair.private_key,
                &signed.intent.signing_bytes().expect("signing bytes"),
                PFTL_SWAP_INTENT_SIGNATURE_CONTEXT_V1,
            )
            .expect("sign intent"),
        );

        let (first, replayed) =
            journal_pftl_swap_intent(&path, &quote, &signed).expect("journal intent");
        assert!(!replayed);
        let (second, replayed) =
            journal_pftl_swap_intent(&path, &quote, &signed).expect("idempotent retry");
        assert!(replayed);
        assert_eq!(first, second);
        assert_eq!(
            find_pftl_swap_intent_replay(&path, &signed).expect("lookup exact replay"),
            Some(first.clone())
        );

        let mut conflicting_key = signed.clone();
        conflicting_key.intent.minimum_output_amount_atoms -= 1;
        conflicting_key.signature_hex = bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &keypair.private_key,
                &conflicting_key
                    .intent
                    .signing_bytes()
                    .expect("conflicting replay bytes"),
                PFTL_SWAP_INTENT_SIGNATURE_CONTEXT_V1,
            )
            .expect("sign conflicting replay"),
        );
        assert_eq!(
            find_pftl_swap_intent_replay(&path, &conflicting_key)
                .expect_err("same key with different signed intent must fail")
                .kind(),
            io::ErrorKind::AlreadyExists,
        );

        let mut conflicting = signed_fixture("intent-2");
        conflicting.intent.quote_id = quote.quote_id.clone();
        conflicting.signature_hex = bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &keypair.private_key,
                &conflicting
                    .intent
                    .signing_bytes()
                    .expect("conflicting signing bytes"),
                PFTL_SWAP_INTENT_SIGNATURE_CONTEXT_V1,
            )
            .expect("sign conflicting intent"),
        );
        assert_eq!(
            journal_pftl_swap_intent(&path, &quote, &conflicting)
                .expect_err("input reservation must conflict")
                .kind(),
            io::ErrorKind::WouldBlock,
        );

        let recovered = recover_pftl_swap_journal(&path).expect("recover journal");
        assert_eq!(
            recovered.entries["intent-1"].state,
            PftlSwapJournalState::InterruptedPrepublish
        );
        transition_pftl_swap_journal_entry(
            &path,
            "intent-1",
            PftlSwapJournalState::Proving,
            None,
            None,
            None,
            None,
        )
        .expect("resume proving");
        let replay = transition_pftl_swap_journal_entry(
            &path,
            "intent-1",
            PftlSwapJournalState::Proving,
            None,
            None,
            None,
            None,
        )
        .expect("same transition is idempotent");
        assert_eq!(replay.transitions.len(), 3);
        assert_eq!(
            transition_pftl_swap_journal_entry(
                &path,
                "intent-1",
                PftlSwapJournalState::Prepared,
                None,
                None,
                None,
                None,
            )
            .expect_err("prepared state requires batch hash")
            .kind(),
            io::ErrorKind::InvalidInput,
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn journal_records_bounded_machine_timings_idempotently() {
        let root = swap_test_dir("pftl-swap-journal-timing");
        let path = root.join("swap-journal.json");
        let quote = quote_fixture();
        let mut signed = signed_fixture("timed-intent");
        signed.intent.quote_id = quote.quote_id.clone();
        let keypair = ml_dsa_65_keygen_from_seed(&[7_u8; 32]);
        signed.signature_hex = bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &keypair.private_key,
                &signed.intent.signing_bytes().expect("timed signing bytes"),
                PFTL_SWAP_INTENT_SIGNATURE_CONTEXT_V1,
            )
            .expect("sign timed intent"),
        );
        journal_pftl_swap_intent(&path, &quote, &signed).expect("journal timed intent");
        let stages = BTreeMap::from([
            ("attempt_1_primary_output_validity".to_string(), 11_u64),
            ("attempt_1_primary_outer_proof".to_string(), 17_u64),
        ]);
        let first =
            record_pftl_swap_stage_timings(&path, "timed-intent", &stages).expect("record timings");
        assert_eq!(
            first
                .timing
                .as_ref()
                .expect("timing")
                .stages_ns
                .get("attempt_1_primary_outer_proof"),
            Some(&17)
        );
        let replay =
            record_pftl_swap_stage_timings(&path, "timed-intent", &stages).expect("timing replay");
        assert_eq!(first.timing, replay.timing);
        let conflict = BTreeMap::from([("attempt_1_primary_outer_proof".to_string(), 18_u64)]);
        assert_eq!(
            record_pftl_swap_stage_timings(&path, "timed-intent", &conflict)
                .expect_err("conflicting timing must fail")
                .kind(),
            io::ErrorKind::AlreadyExists,
        );
        assert!(first
            .transitions
            .iter()
            .all(|transition| transition.at_monotonic_ns > 0));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn journal_reserves_crash_and_terminal_transition_capacity() {
        let root = swap_test_dir("pftl-swap-journal-capacity");
        let path = root.join("swap-journal.json");
        let quote = quote_fixture();
        let mut signed = signed_fixture("capacity-intent");
        signed.intent.quote_id = quote.quote_id.clone();
        let keypair = ml_dsa_65_keygen_from_seed(&[7_u8; 32]);
        signed.signature_hex = bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &keypair.private_key,
                &signed
                    .intent
                    .signing_bytes()
                    .expect("capacity intent bytes"),
                PFTL_SWAP_INTENT_SIGNATURE_CONTEXT_V1,
            )
            .expect("sign capacity intent"),
        );
        journal_pftl_swap_intent(&path, &quote, &signed).expect("journal capacity intent");
        recover_pftl_swap_journal(&path).expect("initial interruption");

        loop {
            let entry = load_pftl_swap_journal(&path)
                .expect("load capacity journal")
                .entries["capacity-intent"]
                .clone();
            if entry.transitions.len() >= PFTL_SWAP_MAX_JOURNAL_TRANSITIONS.saturating_sub(2) {
                assert_eq!(entry.state, PftlSwapJournalState::InterruptedPrepublish);
                break;
            }
            transition_pftl_swap_journal_entry(
                &path,
                "capacity-intent",
                PftlSwapJournalState::Proving,
                None,
                None,
                None,
                None,
            )
            .expect("resume before capacity");
            transition_pftl_swap_journal_entry(
                &path,
                "capacity-intent",
                PftlSwapJournalState::InterruptedPrepublish,
                None,
                None,
                None,
                Some("bounded retry interruption".to_string()),
            )
            .expect("interrupt before capacity");
        }
        assert_eq!(
            transition_pftl_swap_journal_entry(
                &path,
                "capacity-intent",
                PftlSwapJournalState::Proving,
                None,
                None,
                None,
                None,
            )
            .expect_err("proving must preserve recovery capacity")
            .kind(),
            io::ErrorKind::StorageFull,
        );
        let terminal = transition_pftl_swap_journal_entry(
            &path,
            "capacity-intent",
            PftlSwapJournalState::FailedPrepublish,
            None,
            None,
            None,
            Some("retry capacity exhausted".to_string()),
        )
        .expect("terminal failure retains capacity");
        assert_eq!(terminal.state, PftlSwapJournalState::FailedPrepublish);
        assert!(terminal.transitions.len() <= PFTL_SWAP_MAX_JOURNAL_TRANSITIONS);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn quote_store_is_durable_bounded_and_exact() {
        let root = swap_test_dir("pftl-swap-quote-store");
        let path = root.join("quotes.json");
        let quote = quote_fixture();
        store_pftl_swap_quote(&path, &quote, quote.quote_height).expect("store quote");
        assert_eq!(
            find_pftl_swap_quote(&path, &quote.quote_id).expect("find quote"),
            quote
        );

        let mut mismatched = quote.clone();
        mismatched.created_at_unix_ms += 1;
        assert_eq!(
            store_pftl_swap_quote(&path, &mismatched, quote.quote_height)
                .expect_err("same id with different bytes must fail")
                .kind(),
            io::ErrorKind::InvalidData,
        );

        assert_eq!(
            store_pftl_swap_quote(&path, &quote, quote.expiry_height + 1)
                .expect_err("expired quote must not be stored")
                .kind(),
            io::ErrorKind::InvalidInput,
        );
        let _ = fs::remove_dir_all(root);
    }
}
