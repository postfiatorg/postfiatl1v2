pub const FX_FIX_PACKET_VERSION_V1: u16 = 1;
pub const FX_FIX_PACKET_SCHEMA_V1: &str = "postfiat.fx_fix.packet.v1";
pub const FX_FIX_PACKET_HASH_DOMAIN_V1: &str = "postfiat.fx_fix.packet_hash.v1";
pub const FX_FIX_RESERVATION_ID_DOMAIN_V1: &str = "postfiat.fx_fix.reservation_id.v1";
pub const FX_FIX_PACKET_HASH_HEX_LEN: usize = 96;
pub const FX_FIX_ACTION_BINDING_HASH_HEX_LEN: usize = 128;
pub const FX_FIX_RESERVATION_ID_HEX_LEN: usize = 96;
pub const FX_FIX_RESERVATION_NONCE_HEX_LEN: usize = 96;
pub const FX_FIX_WALLET_INTENT_HASH_HEX_LEN: usize = 96;
pub const FX_FIX_SOURCE_COMMITMENT_HEX_LEN: usize = 96;
pub const FX_FIX_GOVERNANCE_POLICY_HASH_HEX_LEN: usize = 96;
pub const FX_FIX_RESERVATION_STATE_ACTIVE: &str = "active";
pub const FX_FIX_RESERVATION_STATE_RELEASED: &str = "released";
pub const FX_FIX_RESERVATION_STATE_FILLED: &str = "filled";
pub const MAX_FX_FIX_STATES: usize = 128;
pub const MAX_FX_FIX_RESERVATIONS: usize = 1_024;
pub const MAX_ACTIVE_FX_FIX_RESERVATIONS_PER_FIX: usize = 64;
pub const MAX_FX_FIX_FILLS: u32 = 1_024;

/// A public, consensus-finalized fixed-rate policy used by a private
/// Asset-Orchard swap. The v1 circuit keeps values private; therefore the
/// hard capacity boundary is a bounded fill count plus the facility's
/// nullifier-protected private inventory. Atom capacities are public quote
/// metadata and become directly decrementable only in a future circuit that
/// proves a capacity transition.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxFixPacketV1 {
    pub version: u16,
    pub schema: String,
    pub operator: String,
    pub base_asset_id: String,
    pub quote_asset_id: String,
    pub epoch: u64,
    pub ratio_numerator: u64,
    pub ratio_denominator: u64,
    pub band_bps: u16,
    pub fee_bps: u16,
    pub valid_from_height: u64,
    pub expires_at_height: u64,
    pub minimum_base_atoms: u64,
    pub capacity_base_atoms: u64,
    pub capacity_quote_atoms: u64,
    pub max_fills: u32,
    pub source_label: String,
    pub source_observation_commitment: String,
    pub governance_policy_hash: String,
    pub previous_fix_hash: Option<String>,
    pub packet_hash: String,
}

impl FxFixPacketV1 {
    pub fn canonical_hash(&self) -> String {
        let preimage = format!(
            "version={}\nschema={}\noperator={}\nbase_asset_id={}\nquote_asset_id={}\nepoch={}\nratio_numerator={}\nratio_denominator={}\nband_bps={}\nfee_bps={}\nvalid_from_height={}\nexpires_at_height={}\nminimum_base_atoms={}\ncapacity_base_atoms={}\ncapacity_quote_atoms={}\nmax_fills={}\nsource_label={}\nsource_observation_commitment={}\ngovernance_policy_hash={}\nprevious_fix_hash={}\n",
            self.version,
            self.schema,
            self.operator,
            self.base_asset_id,
            self.quote_asset_id,
            self.epoch,
            self.ratio_numerator,
            self.ratio_denominator,
            self.band_bps,
            self.fee_bps,
            self.valid_from_height,
            self.expires_at_height,
            self.minimum_base_atoms,
            self.capacity_base_atoms,
            self.capacity_quote_atoms,
            self.max_fills,
            self.source_label,
            self.source_observation_commitment,
            self.governance_policy_hash,
            self.previous_fix_hash.as_deref().unwrap_or("none"),
        );
        let mut hasher = Sha3_384::new();
        Digest::update(&mut hasher, FX_FIX_PACKET_HASH_DOMAIN_V1.as_bytes());
        Digest::update(&mut hasher, [0]);
        Digest::update(&mut hasher, preimage.as_bytes());
        bytes_to_lower_hex(&hasher.finalize())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.version != FX_FIX_PACKET_VERSION_V1 {
            return Err(format!(
                "fx_fix_packet.version must equal {FX_FIX_PACKET_VERSION_V1}"
            ));
        }
        if self.schema != FX_FIX_PACKET_SCHEMA_V1 {
            return Err(format!(
                "fx_fix_packet.schema must equal {FX_FIX_PACKET_SCHEMA_V1}"
            ));
        }
        validate_postfiat_address("fx_fix_packet.operator", &self.operator)?;
        validate_lower_hex_len(
            "fx_fix_packet.base_asset_id",
            &self.base_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "fx_fix_packet.quote_asset_id",
            &self.quote_asset_id,
            ISSUED_ASSET_ID_HEX_LEN,
        )?;
        if self.base_asset_id == self.quote_asset_id {
            return Err("fx_fix_packet asset pair must be distinct".to_string());
        }
        if self.epoch == 0 {
            return Err("fx_fix_packet.epoch must be nonzero".to_string());
        }
        if self.ratio_numerator == 0 || self.ratio_denominator == 0 {
            return Err("fx_fix_packet ratio terms must be nonzero".to_string());
        }
        if self.band_bps > 10_000 || self.fee_bps > 10_000 {
            return Err("fx_fix_packet band_bps and fee_bps must not exceed 10000".to_string());
        }
        if self.valid_from_height > self.expires_at_height {
            return Err(
                "fx_fix_packet.valid_from_height must be <= expires_at_height".to_string(),
            );
        }
        if self.expires_at_height == 0 {
            return Err("fx_fix_packet.expires_at_height must be nonzero".to_string());
        }
        if self.minimum_base_atoms == 0
            || self.minimum_base_atoms > self.capacity_base_atoms
        {
            return Err(
                "fx_fix_packet.minimum_base_atoms must be in 1..=capacity_base_atoms"
                    .to_string(),
            );
        }
        if self.capacity_base_atoms == 0 || self.capacity_quote_atoms == 0 {
            return Err("fx_fix_packet capacities must be nonzero".to_string());
        }
        if self.max_fills == 0 || self.max_fills > MAX_FX_FIX_FILLS {
            return Err(format!(
                "fx_fix_packet.max_fills must be in 1..={MAX_FX_FIX_FILLS}"
            ));
        }
        validate_text_field("fx_fix_packet.source_label", &self.source_label)?;
        validate_lower_hex_len(
            "fx_fix_packet.source_observation_commitment",
            &self.source_observation_commitment,
            FX_FIX_SOURCE_COMMITMENT_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "fx_fix_packet.governance_policy_hash",
            &self.governance_policy_hash,
            FX_FIX_GOVERNANCE_POLICY_HASH_HEX_LEN,
        )?;
        if let Some(previous_fix_hash) = self.previous_fix_hash.as_deref() {
            validate_lower_hex_len(
                "fx_fix_packet.previous_fix_hash",
                previous_fix_hash,
                FX_FIX_PACKET_HASH_HEX_LEN,
            )?;
        }
        if self.epoch == 1 && self.previous_fix_hash.is_some() {
            return Err("fx_fix_packet epoch 1 cannot name a previous fix".to_string());
        }
        if self.epoch > 1 && self.previous_fix_hash.is_none() {
            return Err("fx_fix_packet epoch greater than 1 requires previous_fix_hash".to_string());
        }
        validate_lower_hex_len(
            "fx_fix_packet.packet_hash",
            &self.packet_hash,
            FX_FIX_PACKET_HASH_HEX_LEN,
        )?;
        if self.packet_hash != self.canonical_hash() {
            return Err("fx_fix_packet.packet_hash does not match canonical fields".to_string());
        }
        Ok(())
    }

    pub fn quote_atoms_for_base(&self, base_atoms: u64) -> Result<(u64, bool), String> {
        if base_atoms < self.minimum_base_atoms || base_atoms > self.capacity_base_atoms {
            return Err(format!(
                "base_atoms must be in {}..={} for this fix",
                self.minimum_base_atoms, self.capacity_base_atoms
            ));
        }
        let scaled = u128::from(base_atoms)
            .checked_mul(u128::from(self.ratio_numerator))
            .ok_or_else(|| "fx fix quote multiplication overflow".to_string())?;
        let denominator = u128::from(self.ratio_denominator);
        let quote_atoms_u128 = scaled / denominator;
        let quote_atoms = u64::try_from(quote_atoms_u128)
            .map_err(|_| "fx fix quote exceeds u64".to_string())?;
        if quote_atoms == 0 || quote_atoms > self.capacity_quote_atoms {
            return Err("computed quote amount is outside the fix capacity".to_string());
        }
        Ok((quote_atoms, scaled % denominator == 0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxFixStateV1 {
    pub packet: FxFixPacketV1,
    pub paused: bool,
    pub fill_count: u32,
    pub registered_at_height: u64,
    pub last_updated_height: u64,
}

impl FxFixStateV1 {
    pub fn validate(&self) -> Result<(), String> {
        self.packet.validate()?;
        if self.fill_count > self.packet.max_fills {
            return Err("fx_fix_state.fill_count exceeds packet max_fills".to_string());
        }
        if self.last_updated_height < self.registered_at_height {
            return Err(
                "fx_fix_state.last_updated_height precedes registered_at_height".to_string(),
            );
        }
        Ok(())
    }

    pub fn accepts_height(&self, height: u64) -> bool {
        !self.paused
            && height >= self.packet.valid_from_height
            && height <= self.packet.expires_at_height
            && self.fill_count < self.packet.max_fills
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FxFixReservationV1 {
    pub reservation_id: String,
    pub fix_packet_hash: String,
    pub operator: String,
    pub action_binding_hash: String,
    pub base_atoms: u64,
    pub quote_atoms: u64,
    pub wallet_intent_hash: String,
    pub reservation_nonce: String,
    pub created_at_height: u64,
    pub expires_at_height: u64,
    pub state: String,
    pub terminal_at_height: u64,
}

impl FxFixReservationV1 {
    pub fn validate(&self) -> Result<(), String> {
        validate_lower_hex_len(
            "fx_fix_reservation.reservation_id",
            &self.reservation_id,
            FX_FIX_RESERVATION_ID_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "fx_fix_reservation.fix_packet_hash",
            &self.fix_packet_hash,
            FX_FIX_PACKET_HASH_HEX_LEN,
        )?;
        validate_postfiat_address("fx_fix_reservation.operator", &self.operator)?;
        validate_lower_hex_len(
            "fx_fix_reservation.action_binding_hash",
            &self.action_binding_hash,
            FX_FIX_ACTION_BINDING_HASH_HEX_LEN,
        )?;
        if self.base_atoms == 0 {
            return Err("fx_fix_reservation.base_atoms must be nonzero".to_string());
        }
        if self.quote_atoms == 0 {
            return Err("fx_fix_reservation.quote_atoms must be nonzero".to_string());
        }
        validate_lower_hex_len(
            "fx_fix_reservation.wallet_intent_hash",
            &self.wallet_intent_hash,
            FX_FIX_WALLET_INTENT_HASH_HEX_LEN,
        )?;
        validate_lower_hex_len(
            "fx_fix_reservation.reservation_nonce",
            &self.reservation_nonce,
            FX_FIX_RESERVATION_NONCE_HEX_LEN,
        )?;
        if self.expires_at_height < self.created_at_height {
            return Err(
                "fx_fix_reservation.expires_at_height precedes created_at_height".to_string(),
            );
        }
        if !matches!(
            self.state.as_str(),
            FX_FIX_RESERVATION_STATE_ACTIVE
                | FX_FIX_RESERVATION_STATE_RELEASED
                | FX_FIX_RESERVATION_STATE_FILLED
        ) {
            return Err("fx_fix_reservation.state is unsupported".to_string());
        }
        if self.state == FX_FIX_RESERVATION_STATE_ACTIVE && self.terminal_at_height != 0 {
            return Err("active fx fix reservation cannot have terminal_at_height".to_string());
        }
        if self.state != FX_FIX_RESERVATION_STATE_ACTIVE && self.terminal_at_height == 0 {
            return Err("terminal fx fix reservation requires terminal_at_height".to_string());
        }
        Ok(())
    }

    pub fn active_at(&self, height: u64) -> bool {
        self.state == FX_FIX_RESERVATION_STATE_ACTIVE && height <= self.expires_at_height
    }
}

pub fn fx_fix_reservation_id(
    fix_packet_hash: &str,
    operator: &str,
    action_binding_hash: &str,
    base_atoms: u64,
    quote_atoms: u64,
    wallet_intent_hash: &str,
    reservation_nonce: &str,
) -> Result<String, String> {
    validate_lower_hex_len(
        "fx_fix_reservation.fix_packet_hash",
        fix_packet_hash,
        FX_FIX_PACKET_HASH_HEX_LEN,
    )?;
    validate_postfiat_address("fx_fix_reservation.operator", operator)?;
    validate_lower_hex_len(
        "fx_fix_reservation.action_binding_hash",
        action_binding_hash,
        FX_FIX_ACTION_BINDING_HASH_HEX_LEN,
    )?;
    if base_atoms == 0 {
        return Err("fx_fix_reservation.base_atoms must be nonzero".to_string());
    }
    if quote_atoms == 0 {
        return Err("fx_fix_reservation.quote_atoms must be nonzero".to_string());
    }
    validate_lower_hex_len(
        "fx_fix_reservation.wallet_intent_hash",
        wallet_intent_hash,
        FX_FIX_WALLET_INTENT_HASH_HEX_LEN,
    )?;
    validate_lower_hex_len(
        "fx_fix_reservation.reservation_nonce",
        reservation_nonce,
        FX_FIX_RESERVATION_NONCE_HEX_LEN,
    )?;
    let preimage = format!(
        "fix_packet_hash={fix_packet_hash}\noperator={operator}\naction_binding_hash={action_binding_hash}\nbase_atoms={base_atoms}\nquote_atoms={quote_atoms}\nwallet_intent_hash={wallet_intent_hash}\nreservation_nonce={reservation_nonce}\n"
    );
    let mut hasher = Sha3_384::new();
    Digest::update(&mut hasher, FX_FIX_RESERVATION_ID_DOMAIN_V1.as_bytes());
    Digest::update(&mut hasher, [0]);
    Digest::update(&mut hasher, preimage.as_bytes());
    Ok(bytes_to_lower_hex(&hasher.finalize()))
}
