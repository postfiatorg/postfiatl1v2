// Genesis external lanes: native PFT reserved at genesis for a claim that
// lives outside PFTL (the ERC-20 PFT on Base). Design:
// `docs/specs/pft-erc20-base-bridge-phase2-design-20261026.md` sections 5
// and 5.1. Genesis pins the lane and its release policy; `LedgerState` carries
// the live record, and `external_lane_release_v1` (milestone P2.4) is its only
// writer: it moves atoms from the lane remainder to a PFTL account against a
// proven Base Lock, once per Lock nonce and within the per-window cap.

pub const EXTERNAL_LANE_ID_MAX_BYTES: usize = 32;
pub const EXTERNAL_LANE_RELEASE_CAP_BPS_MAX: u32 = 10_000;
/// Lanes one genesis may configure: execution rules carry each lane's
/// governed release cap in a fixed table of this size.
pub const EXTERNAL_LANE_MAX_LANES: usize = 8;
/// Hard ceiling on the release records one lane stores: under 400 bytes of
/// JSON each, so under 400 MB, and one SHA3-384 step each when a replay base
/// is checked. It is far above any expected release volume (each record costs
/// a Base Lock and a PFTL fee) but below `reserved_atoms` over the lock
/// minimum for any realistic slice; pruning the records behind the finalized
/// checkpoint (P2.5) keeps a lane below it.
pub const EXTERNAL_LANE_MAX_RELEASE_RECORDS: usize = 1_000_000;
const EXTERNAL_LANE_EVM_ADDRESS_HEX_LEN: usize = 40;
const EXTERNAL_LANE_CODE_HASH_HEX_LEN: usize = 64;
/// A `FastSwapCommitteeV1` root is 48 bytes (SHA3-384 over the roster).
const EXTERNAL_LANE_COMMITTEE_ROOT_HEX_LEN: usize = 96;
/// Base mainnet and Base Sepolia: the two source chains a lane may name.
pub const EXTERNAL_LANE_BASE_MAINNET_CHAIN_ID: u64 = 8453;
pub const EXTERNAL_LANE_BASE_SEPOLIA_CHAIN_ID: u64 = 84_532;
pub const EXTERNAL_LANE_SOURCE_CHAIN_IDS: [u64; 2] = [
    EXTERNAL_LANE_BASE_MAINNET_CHAIN_ID,
    EXTERNAL_LANE_BASE_SEPOLIA_CHAIN_ID,
];
/// Governed amendment kinds of a lane, `<prefix>:<lane_id>`: the per-window
/// release cap (design section 5.1) and the committee epoch move (section
/// 4.1). The cap amendment's value is the new `release_cap_bps`, 0 to
/// [`EXTERNAL_LANE_RELEASE_CAP_BPS_MAX`]; the committee epoch move is not
/// built and is refused with [`EXTERNAL_LANE_AMENDMENT_NOT_SUPPORTED`].
pub const GOVERNANCE_KIND_EXTERNAL_LANE_RELEASE_CAP_PREFIX_V1: &str =
    "external_lane_release_cap_v1";
pub const GOVERNANCE_KIND_EXTERNAL_LANE_COMMITTEE_EPOCH_PREFIX_V1: &str =
    "external_lane_committee_epoch_v1";
pub const EXTERNAL_LANE_AMENDMENT_NOT_SUPPORTED: &str = "external_lane_amendment_not_supported";
pub const EXTERNAL_LANE_RELEASE_CAP_INVALID: &str = "invalid_external_lane_release_cap";
/// A cap amendment that raises the cap takes effect this many windows after
/// it commits (design section 5.1).
pub const EXTERNAL_LANE_RELEASE_CAP_RAISE_DELAY_WINDOWS: u64 = 7;

/// The cap amendment kind of `lane_id`.
pub fn external_lane_release_cap_amendment_kind(lane_id: &str) -> String {
    format!("{GOVERNANCE_KIND_EXTERNAL_LANE_RELEASE_CAP_PREFIX_V1}:{lane_id}")
}

/// The lane a cap amendment kind names, `None` for any other kind.
pub fn external_lane_release_cap_amendment_lane(kind: &str) -> Option<&str> {
    kind.strip_prefix(GOVERNANCE_KIND_EXTERNAL_LANE_RELEASE_CAP_PREFIX_V1)?
        .strip_prefix(':')
}

/// The refusal of a governed lane amendment, `None` when it is accepted or
/// is of any other kind: the committee epoch move is not supported, and a
/// cap amendment must name a well-formed lane id and a value of at most
/// 10,000 basis points.
pub fn external_lane_amendment_refusal(kind: &str, value: u32) -> Option<(&'static str, String)> {
    let family = kind.split(':').next()?;
    if family == GOVERNANCE_KIND_EXTERNAL_LANE_COMMITTEE_EPOCH_PREFIX_V1 {
        return Some((
            EXTERNAL_LANE_AMENDMENT_NOT_SUPPORTED,
            format!("governed external lane amendment `{family}` is not supported yet"),
        ));
    }
    if family != GOVERNANCE_KIND_EXTERNAL_LANE_RELEASE_CAP_PREFIX_V1 {
        return None;
    }
    let lane_id = external_lane_release_cap_amendment_lane(kind).unwrap_or_default();
    if let Err(error) = validate_external_lane_id(lane_id) {
        return Some((
            EXTERNAL_LANE_RELEASE_CAP_INVALID,
            format!("release cap amendment `{kind}` must name one lane: {error}"),
        ));
    }
    (value > EXTERNAL_LANE_RELEASE_CAP_BPS_MAX).then(|| {
        (
            EXTERNAL_LANE_RELEASE_CAP_INVALID,
            format!(
                "release cap amendment value {value} must be between 0 and {EXTERNAL_LANE_RELEASE_CAP_BPS_MAX} basis points"
            ),
        )
    })
}

const EXTERNAL_LANE_ROUTE_CONFIG_DOMAIN: &[u8] = b"postfiat.external_lane.route_config.v1";
const EXTERNAL_LANE_RELEASE_HISTORY_DOMAIN: &[u8] = b"postfiat.external_lane.release_history.v1";
/// A release history root is 48 bytes (SHA3-384).
const EXTERNAL_LANE_RELEASE_HISTORY_ROOT_HEX_LEN: usize = 96;

// Refusal codes of `external_lane_release_v1`. A refused release changes no
// state and consumes no Lock nonce, so it can be retried.
pub const EXTERNAL_LANE_NOT_CONFIGURED: &str = "external_lane_not_configured";
pub const EXTERNAL_LANE_NONCE_ALREADY_RELEASED: &str = "external_lane_nonce_already_released";
pub const EXTERNAL_LANE_REMAINDER_EXCEEDED: &str = "external_lane_remainder_exceeded";
pub const EXTERNAL_LANE_WINDOW_CAP_EXCEEDED: &str = "external_lane_window_cap_exceeded";
pub const EXTERNAL_LANE_WINDOW_REGRESSED: &str = "external_lane_window_regressed";
pub const EXTERNAL_LANE_SOURCE_MISMATCH: &str = "external_lane_source_mismatch";
pub const EXTERNAL_LANE_PROOF_INVALID: &str = "external_lane_proof_invalid";
pub const EXTERNAL_LANE_CHECKPOINT_MISMATCH: &str = "external_lane_checkpoint_mismatch";
pub const EXTERNAL_LANE_LOCK_MISMATCH: &str = "external_lane_lock_mismatch";
pub const EXTERNAL_LANE_COMMITTEE_MISSING: &str = "external_lane_committee_missing";
pub const EXTERNAL_LANE_RELEASE_RECORDS_FULL: &str = "external_lane_release_records_full";

/// What the release operation of P2.4 verifies a proven Lock against.
/// Shape-checked here; the values are pinned at genesis and the policy cannot
/// gain a field later without a schema change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenesisExternalLanePolicy {
    /// EVM chain id of the source chain: a parameter, so a test network such
    /// as Base Sepolia (84532) is configurable; Base mainnet is 8453.
    pub chain_id: u64,
    pub lock_contract_address: String,
    pub token_address: String,
    pub token_runtime_code_hash: String,
    pub lock_runtime_code_hash: String,
    /// Bridge committee epoch whose checkpoints prove a Lock.
    pub committee_epoch: u64,
    /// Root of that committee (`FastSwapCommitteeV1::computed_root`): `0x`
    /// and 96 lower-case hex digits.
    pub committee_root: String,
    /// Source-chain confirmations a Lock needs before it can be proven.
    pub confirmation_depth: u64,
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
        if !EXTERNAL_LANE_SOURCE_CHAIN_IDS.contains(&self.chain_id) {
            return Err(format!(
                "external lane policy chain_id {} must be Base mainnet {EXTERNAL_LANE_BASE_MAINNET_CHAIN_ID} or Base Sepolia {EXTERNAL_LANE_BASE_SEPOLIA_CHAIN_ID}",
                self.chain_id
            ));
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
        )?;
        validate_lane_hex(
            "committee_root",
            &self.committee_root,
            EXTERNAL_LANE_COMMITTEE_ROOT_HEX_LEN,
        )?;
        if self.confirmation_depth == 0 {
            return Err("external lane policy confirmation_depth must be positive".to_string());
        }
        Ok(())
    }
}

/// A committed cap amendment of one lane: its id, value and the height at
/// which governance applied it, in commit order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLaneReleaseCapAmendment {
    pub amendment_id: String,
    pub cap_bps: u32,
    pub committed_height: u64,
}

/// A cap set by an amendment and the height from which it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLaneReleaseCapChange {
    pub amendment_id: String,
    pub cap_bps: u32,
    pub effective_height: u64,
}

/// The cap in force at a height, the amendment that set it (`None` while
/// the genesis cap holds), and a committed raise still in its delay.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLaneReleaseCapSchedule {
    pub active_cap_bps: u32,
    pub active: Option<ExternalLaneReleaseCapChange>,
    pub pending: Option<ExternalLaneReleaseCapChange>,
}

/// The cap of one lane for every height from its last committed cap
/// amendment on: `cap_bps`, or `pending_cap_bps` from
/// `pending_from_height`. `Copy`, so execution rules can carry it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalLaneReleaseCapRule {
    pub cap_bps: u32,
    pub pending_cap_bps: u32,
    pub pending_from_height: u64,
}

impl ExternalLaneReleaseCapRule {
    pub fn cap_bps_at(&self, height: u64) -> u32 {
        if height >= self.pending_from_height {
            self.pending_cap_bps
        } else {
            self.cap_bps
        }
    }
}

/// The governed release cap of each genesis lane, by its index in the
/// genesis list; a lane without a committed cap amendment keeps its genesis
/// cap. Built from governance, carried by the execution rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalLaneReleaseCaps {
    rules: [Option<ExternalLaneReleaseCapRule>; EXTERNAL_LANE_MAX_LANES],
}

impl Default for ExternalLaneReleaseCaps {
    fn default() -> Self {
        Self::genesis()
    }
}

impl ExternalLaneReleaseCaps {
    /// Every lane at its genesis cap.
    pub const fn genesis() -> Self {
        Self {
            rules: [None; EXTERNAL_LANE_MAX_LANES],
        }
    }

    /// The table for `lanes` under `amendments_of(lane_id)`, each lane's
    /// committed cap amendments in commit order.
    pub fn from_amendments(
        lanes: &[GenesisExternalLane],
        amendments_of: impl Fn(&str) -> Vec<ExternalLaneReleaseCapAmendment>,
    ) -> Self {
        let mut caps = Self::genesis();
        for (slot, lane) in caps.rules.iter_mut().zip(lanes) {
            let amendments = amendments_of(&lane.id);
            if !amendments.is_empty() {
                *slot = Some(lane.release_cap_rule(&amendments));
            }
        }
        caps
    }

    /// The cap of the lane at `lane_index` at `height`.
    pub fn cap_bps_at(&self, lane_index: usize, lane: &GenesisExternalLane, height: u64) -> u32 {
        self.rules
            .get(lane_index)
            .copied()
            .flatten()
            .map_or(lane.release_cap_bps, |rule| rule.cap_bps_at(height))
    }
}

impl GenesisExternalLane {
    /// Atoms one window may release: `reserved_atoms × release_cap_bps /
    /// 10_000`, rounded down.
    pub fn window_cap_atoms(&self) -> u64 {
        self.window_cap_atoms_for_bps(self.release_cap_bps)
    }

    /// Atoms one window may release under a cap of `cap_bps`.
    pub fn window_cap_atoms_for_bps(&self, cap_bps: u32) -> u64 {
        let cap = u128::from(self.reserved_atoms) * u128::from(cap_bps)
            / u128::from(EXTERNAL_LANE_RELEASE_CAP_BPS_MAX);
        u64::try_from(cap).unwrap_or(u64::MAX)
    }

    /// The cap in force at `height` under the lane's committed cap
    /// amendments, in commit order (design section 5.1). Amendments
    /// committed above `height` are not yet known there. An amendment at or
    /// below the cap in force when it commits applies from its commit height;
    /// one above it applies `EXTERNAL_LANE_RELEASE_CAP_RAISE_DELAY_WINDOWS`
    /// windows of `release_window_blocks` later. Each committed amendment
    /// replaces a raise still in its delay, so a zero cap committed during a
    /// pending raise holds; ties at one height follow commit order.
    pub fn release_cap_schedule_at(
        &self,
        amendments: &[ExternalLaneReleaseCapAmendment],
        height: u64,
    ) -> ExternalLaneReleaseCapSchedule {
        let delay = self
            .release_window_blocks
            .saturating_mul(EXTERNAL_LANE_RELEASE_CAP_RAISE_DELAY_WINDOWS);
        let mut active_cap_bps = self.release_cap_bps;
        let mut active = None;
        let mut pending: Option<ExternalLaneReleaseCapChange> = None;
        for amendment in amendments
            .iter()
            .filter(|amendment| amendment.committed_height <= height)
        {
            if let Some(raise) = pending.take() {
                if raise.effective_height <= amendment.committed_height {
                    active_cap_bps = raise.cap_bps;
                    active = Some(raise);
                }
            }
            let lowers = amendment.cap_bps <= active_cap_bps;
            let change = ExternalLaneReleaseCapChange {
                amendment_id: amendment.amendment_id.clone(),
                cap_bps: amendment.cap_bps,
                effective_height: if lowers {
                    amendment.committed_height
                } else {
                    amendment.committed_height.saturating_add(delay)
                },
            };
            if lowers {
                active_cap_bps = change.cap_bps;
                active = Some(change);
            } else {
                pending = Some(change);
            }
        }
        if let Some(raise) = pending.take_if(|raise| raise.effective_height <= height) {
            active_cap_bps = raise.cap_bps;
            active = Some(raise);
        }
        ExternalLaneReleaseCapSchedule {
            active_cap_bps,
            active,
            pending,
        }
    }

    /// The cap for every height from the last committed amendment on, which
    /// is every height a block can still execute at.
    pub fn release_cap_rule(
        &self,
        amendments: &[ExternalLaneReleaseCapAmendment],
    ) -> ExternalLaneReleaseCapRule {
        let last = amendments
            .iter()
            .map(|amendment| amendment.committed_height)
            .max()
            .unwrap_or(0);
        let schedule = self.release_cap_schedule_at(amendments, last);
        let (pending_cap_bps, pending_from_height) = schedule
            .pending
            .map_or((schedule.active_cap_bps, u64::MAX), |raise| {
                (raise.cap_bps, raise.effective_height)
            });
        ExternalLaneReleaseCapRule {
            cap_bps: schedule.active_cap_bps,
            pending_cap_bps,
            pending_from_height,
        }
    }

    /// Block headers carry no time, so a window is a run of
    /// `release_window_blocks` heights.
    pub fn window_index_at(&self, height: u64) -> u64 {
        height / self.release_window_blocks.max(1)
    }

    /// Digest a checkpoint names as its `route_config_digest` for this lane:
    /// SHA3-384 over a domain tag and the lane's genesis JSON.
    pub fn route_config_digest(&self) -> Result<[u8; 48], String> {
        let encoded = serde_json::to_vec(self)
            .map_err(|error| format!("encode external lane `{}`: {error}", self.id))?;
        let mut hasher = Sha3_384::new();
        hasher.update(EXTERNAL_LANE_ROUTE_CONFIG_DOMAIN);
        hasher.update([0_u8]);
        hasher.update(encoded);
        Ok(hasher.finalize().into())
    }

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

/// Validates a genesis lane list: non-empty, unique ids, one lane per Base
/// source (chain id and lock contract, so a Lock is released by one lane
/// only), and a reserved total strictly below the native supply so the faucet
/// keeps a positive balance.
pub fn validate_genesis_external_lanes(
    lanes: &[GenesisExternalLane],
    native_supply_atoms: u64,
) -> Result<u64, String> {
    if lanes.is_empty() {
        return Err("genesis external_lanes must be absent or non-empty".to_string());
    }
    if lanes.len() > EXTERNAL_LANE_MAX_LANES {
        return Err(format!(
            "genesis configures {} external lanes, above the maximum {EXTERNAL_LANE_MAX_LANES}",
            lanes.len()
        ));
    }
    let mut ids = BTreeSet::new();
    let mut sources = BTreeMap::new();
    let mut reserved_total = 0_u64;
    for lane in lanes {
        lane.validate()?;
        if !ids.insert(lane.id.as_str()) {
            return Err(format!("duplicate genesis external lane id `{}`", lane.id));
        }
        let source = (
            lane.policy.chain_id,
            lane.policy.lock_contract_address.as_str(),
        );
        if let Some(first) = sources.insert(source, lane.id.as_str()) {
            return Err(format!(
                "genesis external lanes `{first}` and `{}` both name Base source {} lock contract {}",
                lane.id, source.0, source.1
            ));
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

/// One applied release, keyed by the Base Lock it consumed: the source chain,
/// the lock contract and the Lock nonce, with the proven amount and recipient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalLaneReleaseRecord {
    pub lock_nonce: u64,
    pub source_chain_id: u64,
    pub lock_contract_address: String,
    pub amount_atoms: u64,
    pub recipient: String,
    /// The Base block of the committee checkpoint that proved the Lock.
    pub source_block_number: u64,
    pub source_block_hash: String,
    pub released_at_height: u64,
}

impl ExternalLaneReleaseRecord {
    pub fn validate(&self) -> Result<(), String> {
        if self.amount_atoms == 0 {
            return Err(format!(
                "external lane release of Lock nonce {} has zero amount",
                self.lock_nonce
            ));
        }
        if self.recipient.is_empty() {
            return Err(format!(
                "external lane release of Lock nonce {} has no recipient",
                self.lock_nonce
            ));
        }
        validate_lane_hex(
            "release lock_contract_address",
            &self.lock_contract_address,
            EXTERNAL_LANE_EVM_ADDRESS_HEX_LEN,
        )?;
        validate_lane_hex(
            "release source_block_hash",
            &self.source_block_hash,
            EXTERNAL_LANE_CODE_HASH_HEX_LEN,
        )
    }
}

/// Live record of a genesis external lane. Its remainder
/// (`reserved_atoms - released_atoms`) is native supply that is not in
/// circulation; `external_lane_release_v1` is the only writer of the release
/// fields, and every released atom has a record in `releases`.
///
/// The state commitment covers the release fields and
/// `release_history_root`, a SHA3-384 hash chain over the records in
/// application order; the records themselves stay in the stored ledger for
/// reads and replay and are checked against the root when a replay base is
/// loaded, so the per-block commitment does not grow with them.
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
    /// Hash chain over `releases`. Omitted while there is none, so an
    /// unreleased lane encodes and commits exactly as it did in P2.2.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_history_root: Option<String>,
    /// Release history in application order; not in the state commitment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub releases: Vec<ExternalLaneReleaseRecord>,
}

/// The committed part of an `ExternalLaneState`: every field but the records.
#[derive(Serialize)]
struct ExternalLaneCommitment<'a> {
    lane_id: &'a str,
    reserved_atoms: u64,
    released_atoms: u64,
    released_lock_nonces: &'a BTreeSet<u64>,
    window_index: u64,
    window_released_atoms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    release_history_root: Option<&'a str>,
}

/// The release history root after `record`: SHA3-384 over a domain tag, the
/// previous root (empty for the first record) and the record's JSON.
pub fn external_lane_release_history_root(
    previous: Option<&str>,
    record: &ExternalLaneReleaseRecord,
) -> Result<String, String> {
    let encoded = serde_json::to_vec(record)
        .map_err(|error| format!("encode external lane release: {error}"))?;
    let mut hasher = Sha3_384::new();
    hasher.update(EXTERNAL_LANE_RELEASE_HISTORY_DOMAIN);
    hasher.update([0_u8]);
    hasher.update(previous.unwrap_or_default().as_bytes());
    hasher.update([0_u8]);
    hasher.update(encoded);
    let digest: [u8; 48] = hasher.finalize().into();
    Ok(format!(
        "0x{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    ))
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
            release_history_root: None,
            releases: Vec::new(),
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
        if self.released_lock_nonces.len() as u64 > self.released_atoms {
            return Err(format!(
                "external lane `{}` has more Lock nonces than released atoms",
                self.lane_id
            ));
        }
        if let Some(root) = &self.release_history_root {
            validate_lane_hex(
                "release_history_root",
                root,
                EXTERNAL_LANE_RELEASE_HISTORY_ROOT_HEX_LEN,
            )?;
        }
        Ok(())
    }

    /// The release fields must be exactly what the recorded releases add up
    /// to: the nonce set is the records' nonces, each once, and
    /// `released_atoms` is the sum of their amounts. Walks every record, so
    /// it runs when a replay base is loaded, not per block.
    fn validate_recorded_releases(&self) -> Result<(), String> {
        let mut nonces = BTreeSet::new();
        let mut released = 0_u64;
        for record in &self.releases {
            record
                .validate()
                .map_err(|error| format!("external lane `{}`: {error}", self.lane_id))?;
            if !nonces.insert(record.lock_nonce) {
                return Err(format!(
                    "external lane `{}` records Lock nonce {} more than once",
                    self.lane_id, record.lock_nonce
                ));
            }
            released = released.checked_add(record.amount_atoms).ok_or_else(|| {
                format!("external lane `{}` recorded releases overflow", self.lane_id)
            })?;
        }
        if nonces != self.released_lock_nonces {
            return Err(format!(
                "external lane `{}` released Lock nonces {:?} differ from its recorded releases {:?}",
                self.lane_id, self.released_lock_nonces, nonces
            ));
        }
        if released != self.released_atoms {
            return Err(format!(
                "external lane `{}` has released_atoms {}, but its recorded releases sum to {released}",
                self.lane_id, self.released_atoms
            ));
        }
        Ok(())
    }

    /// Checks the record against its genesis lane: same id and reserve,
    /// releases only from the lane's configured source, and window fields that
    /// follow from the recorded heights, no window above the cap.
    pub fn validate_against_genesis(&self, lane: &GenesisExternalLane) -> Result<(), String> {
        self.validate()?;
        self.validate_recorded_releases()?;
        if self.lane_id != lane.id {
            return Err(format!(
                "external lane `{}` checked against genesis lane `{}`",
                self.lane_id, lane.id
            ));
        }
        if self.reserved_atoms != lane.reserved_atoms {
            return Err(format!(
                "external lane `{}` reserves {} atoms, but genesis reserves {}",
                self.lane_id, self.reserved_atoms, lane.reserved_atoms
            ));
        }
        let cap = lane.window_cap_atoms();
        let mut per_window = BTreeMap::<u64, u64>::new();
        let mut previous_height = 0_u64;
        let mut root = None;
        for record in &self.releases {
            root = Some(external_lane_release_history_root(root.as_deref(), record)?);
            if record.source_chain_id != lane.policy.chain_id
                || record.lock_contract_address != lane.policy.lock_contract_address
            {
                return Err(format!(
                    "external lane `{}` records Lock nonce {} from a source genesis did not configure",
                    self.lane_id, record.lock_nonce
                ));
            }
            if record.released_at_height < previous_height {
                return Err(format!(
                    "external lane `{}` records releases out of height order",
                    self.lane_id
                ));
            }
            previous_height = record.released_at_height;
            let window = per_window
                .entry(lane.window_index_at(record.released_at_height))
                .or_default();
            *window = window.saturating_add(record.amount_atoms);
            if *window > cap {
                return Err(format!(
                    "external lane `{}` records {} atoms in one window, above the cap {cap}",
                    self.lane_id, window
                ));
            }
        }
        let (window_index, window_released) = per_window
            .last_key_value()
            .map(|(index, released)| (*index, *released))
            .unwrap_or((0, 0));
        if self.window_index != window_index || self.window_released_atoms != window_released {
            return Err(format!(
                "external lane `{}` has window {} with {} atoms released, but its recorded releases give window {window_index} with {window_released}",
                self.lane_id, self.window_index, self.window_released_atoms
            ));
        }
        if self.release_history_root != root {
            return Err(format!(
                "external lane `{}` release history root {:?} does not match its recorded releases {:?}",
                self.lane_id, self.release_history_root, root
            ));
        }
        Ok(())
    }

    /// The release rules, in the design's order: a nonce is paid once, the
    /// amount fits the remainder, and the window stays within its cap. Returns
    /// a refusal code and message; nothing is mutated.
    pub fn check_release(
        &self,
        lane: &GenesisExternalLane,
        lock_nonce: u64,
        amount_atoms: u64,
        height: u64,
    ) -> Result<(), (&'static str, String)> {
        self.check_release_capped(lane, lock_nonce, amount_atoms, height, lane.window_cap_atoms())
    }

    /// [`Self::check_release`] under a window cap of `cap_atoms`, the cap a
    /// governed amendment set for the release's height.
    pub fn check_release_capped(
        &self,
        lane: &GenesisExternalLane,
        lock_nonce: u64,
        amount_atoms: u64,
        height: u64,
        cap_atoms: u64,
    ) -> Result<(), (&'static str, String)> {
        self.check_release_within(
            lane,
            lock_nonce,
            amount_atoms,
            height,
            cap_atoms,
            EXTERNAL_LANE_MAX_RELEASE_RECORDS,
        )
    }

    fn check_release_within(
        &self,
        lane: &GenesisExternalLane,
        lock_nonce: u64,
        amount_atoms: u64,
        height: u64,
        cap: u64,
        max_records: usize,
    ) -> Result<(), (&'static str, String)> {
        if self.released_lock_nonces.contains(&lock_nonce) {
            return Err((
                EXTERNAL_LANE_NONCE_ALREADY_RELEASED,
                format!(
                    "external lane `{}` already released Lock nonce {lock_nonce}",
                    self.lane_id
                ),
            ));
        }
        let remainder = self
            .remainder_atoms()
            .map_err(|error| (EXTERNAL_LANE_REMAINDER_EXCEEDED, error))?;
        if amount_atoms > remainder {
            return Err((
                EXTERNAL_LANE_REMAINDER_EXCEEDED,
                format!(
                    "external lane `{}` holds {remainder} atoms, below the release of {amount_atoms}",
                    self.lane_id
                ),
            ));
        }
        let window_index = lane.window_index_at(height);
        if window_index < self.window_index {
            return Err((
                EXTERNAL_LANE_WINDOW_REGRESSED,
                format!(
                    "external lane `{}` release window {window_index} is below its recorded window {}",
                    self.lane_id, self.window_index
                ),
            ));
        }
        let window_released = if window_index == self.window_index {
            self.window_released_atoms
        } else {
            0
        };
        if window_released
            .checked_add(amount_atoms)
            .is_none_or(|total| total > cap)
        {
            return Err((
                EXTERNAL_LANE_WINDOW_CAP_EXCEEDED,
                format!(
                    "external lane `{}` window {window_index} has released {window_released} of its cap {cap}; {amount_atoms} more would exceed it",
                    self.lane_id
                ),
            ));
        }
        if self.releases.len() >= max_records {
            return Err((
                EXTERNAL_LANE_RELEASE_RECORDS_FULL,
                format!(
                    "external lane `{}` stores {} release records, the ceiling {max_records}",
                    self.lane_id,
                    self.releases.len()
                ),
            ));
        }
        Ok(())
    }

    /// Applies a checked release: records it, adds the nonce and the amount,
    /// and moves the window. The caller credits the recipient. Every fallible
    /// step runs before the first mutation, so a refusal leaves the lane as it
    /// was.
    pub fn apply_release(
        &mut self,
        lane: &GenesisExternalLane,
        record: ExternalLaneReleaseRecord,
    ) -> Result<(), (&'static str, String)> {
        self.apply_release_capped(lane, record, lane.window_cap_atoms())
    }

    /// [`Self::apply_release`] under a window cap of `cap_atoms`.
    pub fn apply_release_capped(
        &mut self,
        lane: &GenesisExternalLane,
        record: ExternalLaneReleaseRecord,
        cap_atoms: u64,
    ) -> Result<(), (&'static str, String)> {
        self.apply_release_within(lane, record, cap_atoms, EXTERNAL_LANE_MAX_RELEASE_RECORDS)
    }

    fn apply_release_within(
        &mut self,
        lane: &GenesisExternalLane,
        record: ExternalLaneReleaseRecord,
        cap: u64,
        max_records: usize,
    ) -> Result<(), (&'static str, String)> {
        self.check_release_within(
            lane,
            record.lock_nonce,
            record.amount_atoms,
            record.released_at_height,
            cap,
            max_records,
        )?;
        let root = external_lane_release_history_root(self.release_history_root.as_deref(), &record)
            .map_err(|error| (EXTERNAL_LANE_PROOF_INVALID, error))?;
        let window_index = lane.window_index_at(record.released_at_height);
        if window_index != self.window_index {
            self.window_index = window_index;
            self.window_released_atoms = 0;
        }
        self.window_released_atoms += record.amount_atoms;
        self.released_atoms += record.amount_atoms;
        self.released_lock_nonces.insert(record.lock_nonce);
        self.release_history_root = Some(root);
        self.releases.push(record);
        Ok(())
    }

    /// The committed fields, without the release records: bounded per
    /// release by one nonce, whatever the records hold.
    pub fn state_commitment_bytes(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        serde_json::to_vec(&ExternalLaneCommitment {
            lane_id: &self.lane_id,
            reserved_atoms: self.reserved_atoms,
            released_atoms: self.released_atoms,
            released_lock_nonces: &self.released_lock_nonces,
            window_index: self.window_index,
            window_released_atoms: self.window_released_atoms,
            release_history_root: self.release_history_root.as_deref(),
        })
        .map_err(|error| format!("encode external lane state: {error}"))
    }
}
