// Genesis external lanes: native PFT reserved at genesis for a claim that
// lives outside PFTL (the ERC-20 PFT on Base). Design:
// `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md` sections 5
// and 5.1. Genesis pins the lane and its release policy; `LedgerState` carries
// the live record. Nothing releases from a lane until the release operation
// (`external_lane_release_v1`, milestone P2.4) exists.

pub const EXTERNAL_LANE_ID_MAX_BYTES: usize = 32;
pub const EXTERNAL_LANE_RELEASE_CAP_BPS_MAX: u32 = 10_000;
const EXTERNAL_LANE_EVM_ADDRESS_HEX_LEN: usize = 40;
const EXTERNAL_LANE_CODE_HASH_HEX_LEN: usize = 64;

/// What the release operation of P2.4 verifies a proven Lock against.
/// Shape-checked here; the values are pinned at genesis and never change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisExternalLanePolicy {
    /// EVM chain id of the source chain (Base mainnet is 8453).
    pub chain_id: u64,
    pub lock_contract_address: String,
    pub token_address: String,
    pub token_runtime_code_hash: String,
    pub lock_runtime_code_hash: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisExternalLane {
    pub id: String,
    pub reserved_atoms: u64,
    pub release_window_blocks: u64,
    pub release_cap_bps: u32,
    pub policy: GenesisExternalLanePolicy,
}

impl GenesisExternalLanePolicy {
    pub fn validate(&self) -> Result<(), String> {
        if self.chain_id == 0 {
            return Err("external lane policy chain_id must be nonzero".to_string());
        }
        validate_lane_hex(
            "lock_contract_address",
            &self.lock_contract_address,
            EXTERNAL_LANE_EVM_ADDRESS_HEX_LEN,
        )?;
        validate_lane_hex(
            "token_address",
            &self.token_address,
            EXTERNAL_LANE_EVM_ADDRESS_HEX_LEN,
        )?;
        validate_lane_hex(
            "token_runtime_code_hash",
            &self.token_runtime_code_hash,
            EXTERNAL_LANE_CODE_HASH_HEX_LEN,
        )?;
        validate_lane_hex(
            "lock_runtime_code_hash",
            &self.lock_runtime_code_hash,
            EXTERNAL_LANE_CODE_HASH_HEX_LEN,
        )
    }
}

impl GenesisExternalLane {
    pub fn validate(&self) -> Result<(), String> {
        validate_external_lane_id(&self.id)?;
        if self.reserved_atoms == 0 {
            return Err(format!(
                "external lane `{}` reserved_atoms must be positive",
                self.id
            ));
        }
        if self.release_window_blocks == 0 {
            return Err(format!(
                "external lane `{}` release_window_blocks must be positive",
                self.id
            ));
        }
        if !(1..=EXTERNAL_LANE_RELEASE_CAP_BPS_MAX).contains(&self.release_cap_bps) {
            return Err(format!(
                "external lane `{}` release_cap_bps must be between 1 and {EXTERNAL_LANE_RELEASE_CAP_BPS_MAX}",
                self.id
            ));
        }
        self.policy
            .validate()
            .map_err(|error| format!("external lane `{}`: {error}", self.id))
    }
}

/// Validates a genesis lane list: non-empty, unique ids, and a reserved total
/// strictly below the native supply so the faucet keeps a positive balance.
pub fn validate_genesis_external_lanes(
    lanes: &[GenesisExternalLane],
    native_supply_atoms: u64,
) -> Result<u64, String> {
    if lanes.is_empty() {
        return Err("genesis external_lanes must be absent or non-empty".to_string());
    }
    let mut ids = BTreeSet::new();
    let mut reserved_total = 0_u64;
    for lane in lanes {
        lane.validate()?;
        if !ids.insert(lane.id.as_str()) {
            return Err(format!("duplicate genesis external lane id `{}`", lane.id));
        }
        reserved_total = reserved_total
            .checked_add(lane.reserved_atoms)
            .ok_or_else(|| "genesis external lane reserved total overflows".to_string())?;
    }
    if reserved_total >= native_supply_atoms {
        return Err(format!(
            "genesis external lanes reserve {reserved_total} atoms, which must be below the native supply {native_supply_atoms}"
        ));
    }
    Ok(reserved_total)
}

fn validate_external_lane_id(id: &str) -> Result<(), String> {
    let valid = !id.is_empty()
        && id.len() <= EXTERNAL_LANE_ID_MAX_BYTES
        && id.as_bytes()[0].is_ascii_lowercase()
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-');
    if valid {
        Ok(())
    } else {
        Err(format!(
            "external lane id `{id}` must be 1-{EXTERNAL_LANE_ID_MAX_BYTES} bytes of [a-z0-9-] starting with a letter"
        ))
    }
}

fn validate_lane_hex(field: &str, value: &str, digits: usize) -> Result<(), String> {
    let valid = value.len() == digits + 2
        && value.starts_with("0x")
        && value[2..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte));
    if valid {
        Ok(())
    } else {
        Err(format!(
            "external lane policy {field} must be 0x followed by {digits} lower-case hex digits"
        ))
    }
}

/// Live record of a genesis external lane. Its remainder
/// (`reserved_atoms - released_atoms`) is native supply that is not in
/// circulation; the release operation of P2.4 is the only writer of the
/// release fields.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalLaneState {
    pub lane_id: String,
    pub reserved_atoms: u64,
    pub released_atoms: u64,
    #[serde(default)]
    pub released_lock_nonces: BTreeSet<u64>,
    pub window_index: u64,
    pub window_released_atoms: u64,
}

impl ExternalLaneState {
    pub fn genesis(lane: &GenesisExternalLane) -> Self {
        Self {
            lane_id: lane.id.clone(),
            reserved_atoms: lane.reserved_atoms,
            released_atoms: 0,
            released_lock_nonces: BTreeSet::new(),
            window_index: 0,
            window_released_atoms: 0,
        }
    }

    pub fn remainder_atoms(&self) -> Result<u64, String> {
        self.reserved_atoms
            .checked_sub(self.released_atoms)
            .ok_or_else(|| {
                format!(
                    "external lane `{}` released {} atoms above its reserve {}",
                    self.lane_id, self.released_atoms, self.reserved_atoms
                )
            })
    }

    pub fn validate(&self) -> Result<(), String> {
        validate_external_lane_id(&self.lane_id)?;
        if self.reserved_atoms == 0 {
            return Err(format!(
                "external lane `{}` reserved_atoms must be positive",
                self.lane_id
            ));
        }
        self.remainder_atoms()?;
        if self.window_released_atoms > self.released_atoms {
            return Err(format!(
                "external lane `{}` window release exceeds total release",
                self.lane_id
            ));
        }
        if self.released_atoms > 0 && self.released_lock_nonces.is_empty() {
            return Err(format!(
                "external lane `{}` released atoms without a Lock nonce",
                self.lane_id
            ));
        }
        Ok(())
    }

    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(self).map_err(|error| format!("encode external lane state: {error}"))
    }
}
