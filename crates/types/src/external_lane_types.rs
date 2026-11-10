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
/// minimum for any realistic slice. Pruning the records behind the finalized
/// height keeps the records a lane stores at most this many: only the
/// releases of the retention window and of the lane's open window stay stored.
pub const EXTERNAL_LANE_MAX_RELEASE_RECORDS: usize = 1_000_000;
/// Release windows a record stays stored behind the finalized height before
/// it is pruned. One window is this change's own retention, not a rule of
/// design section 5.2; it is justified by 5.2's loss bound of one window's
/// cap, which assumes the watcher sees each release and the zero-cap
/// amendment lands before the next window releases. A record becomes
/// prunable one window after its height and is removed by the next
/// qualifying release.
pub const EXTERNAL_LANE_RELEASE_RETENTION_WINDOWS: u64 = 1;
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
/// Governed amendment kinds of a lane: the per-window release cap,
/// `<prefix>:<lane_id>` (design section 5.1), whose value is the new
/// `release_cap_bps`, 0 to [`EXTERNAL_LANE_RELEASE_CAP_BPS_MAX`]; and the
/// committee epoch move, `<prefix>:<lane_id>:<binding>` (section 4.1), whose
/// value is the new committee epoch and whose binding
/// ([`external_lane_committee_binding`]) hashes the lane, that epoch and the
/// registered committee root, which does not fit the `u32` value.
pub const GOVERNANCE_KIND_EXTERNAL_LANE_RELEASE_CAP_PREFIX_V1: &str =
    "external_lane_release_cap_v1";
pub const GOVERNANCE_KIND_EXTERNAL_LANE_COMMITTEE_EPOCH_PREFIX_V1: &str =
    "external_lane_committee_epoch_v1";
pub const EXTERNAL_LANE_RELEASE_CAP_INVALID: &str = "invalid_external_lane_release_cap";
/// A cap amendment that raises the cap takes effect this many windows after
/// it commits (design section 5.1).
pub const EXTERNAL_LANE_RELEASE_CAP_RAISE_DELAY_WINDOWS: u64 = 7;
// Refusal codes of the committee epoch move.
pub const EXTERNAL_LANE_COMMITTEE_MOVE_INVALID: &str = "invalid_external_lane_committee_move";
pub const EXTERNAL_LANE_COMMITTEE_BINDING_MISMATCH: &str =
    "external_lane_committee_binding_mismatch";
pub const EXTERNAL_LANE_COMMITTEE_NOT_CURRENT: &str = "external_lane_committee_not_current";
pub const EXTERNAL_LANE_COMMITTEE_EPOCH_NOT_ADVANCED: &str =
    "external_lane_committee_epoch_not_advanced";
pub const EXTERNAL_LANE_COMMITTEE_LEDGER_UNAVAILABLE: &str =
    "external_lane_committee_ledger_unavailable";
/// A release whose checkpoint names a committee other than the lane's active
/// one at the release's height.
pub const EXTERNAL_LANE_COMMITTEE_INACTIVE: &str = "external_lane_committee_inactive";
const EXTERNAL_LANE_COMMITTEE_BINDING_DOMAIN: &[u8] = b"postfiat.external_lane.committee_move.v1";

/// The binding a committee move names: SHA3-384 over a domain tag, the lane
/// id, the epoch and the committee root, in lowercase hex.
pub fn external_lane_committee_binding(lane_id: &str, epoch: u64, root: &[u8; 48]) -> String {
    let mut hasher = Sha3_384::new();
    hasher.update(EXTERNAL_LANE_COMMITTEE_BINDING_DOMAIN);
    hasher.update([0_u8]);
    hasher.update(lane_id.as_bytes());
    hasher.update([0_u8]);
    hasher.update(epoch.to_be_bytes());
    hasher.update(root);
    let digest: [u8; 48] = hasher.finalize().into();
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The committee move kind of `lane_id` to the committee of `epoch` and
/// `root`; its amendment value is `epoch`.
pub fn external_lane_committee_amendment_kind(lane_id: &str, epoch: u64, root: &[u8; 48]) -> String {
    format!(
        "{GOVERNANCE_KIND_EXTERNAL_LANE_COMMITTEE_EPOCH_PREFIX_V1}:{lane_id}:{}",
        external_lane_committee_binding(lane_id, epoch, root)
    )
}

/// The lane and binding a committee move kind names, `None` for any other
/// kind.
pub fn external_lane_committee_amendment_parts(kind: &str) -> Option<(&str, &str)> {
    kind.strip_prefix(GOVERNANCE_KIND_EXTERNAL_LANE_COMMITTEE_EPOCH_PREFIX_V1)?
        .strip_prefix(':')?
        .split_once(':')
}

/// The cap amendment kind of `lane_id`.
pub fn external_lane_release_cap_amendment_kind(lane_id: &str) -> String {
    format!("{GOVERNANCE_KIND_EXTERNAL_LANE_RELEASE_CAP_PREFIX_V1}:{lane_id}")
}

/// The lane a cap amendment kind names, `None` for any other kind.
pub fn external_lane_release_cap_amendment_lane(kind: &str) -> Option<&str> {
    kind.strip_prefix(GOVERNANCE_KIND_EXTERNAL_LANE_RELEASE_CAP_PREFIX_V1)?
        .strip_prefix(':')
}

/// The refusal of a governed lane amendment's shape, `None` when it is
/// well formed or of any other kind: a cap amendment names one lane id and a
/// value of at most 10,000 basis points; a committee move names one lane id
/// and a lowercase 48-byte binding. Whether a committee move binds a
/// registered committee is checked against the ledger
/// ([`external_lane_committee_move_refusal`]).
pub fn external_lane_amendment_refusal(kind: &str, value: u32) -> Option<(&'static str, String)> {
    let family = kind.split(':').next()?;
    if family == GOVERNANCE_KIND_EXTERNAL_LANE_COMMITTEE_EPOCH_PREFIX_V1 {
        let well_formed = external_lane_committee_amendment_parts(kind).is_some_and(
            |(lane_id, binding)| {
                validate_external_lane_id(lane_id).is_ok()
                    && binding.len() == EXTERNAL_LANE_COMMITTEE_ROOT_HEX_LEN
                    && binding
                        .bytes()
                        .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            },
        );
        return (!well_formed).then(|| {
            (
                EXTERNAL_LANE_COMMITTEE_MOVE_INVALID,
                format!(
                    "committee move `{kind}` must name one lane and a lowercase 48-byte binding of its epoch and committee root"
                ),
            )
        });
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

/// A cap amendment for a lane id genesis does not configure.
pub const EXTERNAL_LANE_RELEASE_CAP_LANE_NOT_CONFIGURED: &str =
    "external_lane_release_cap_lane_not_configured";
/// A cap amendment not declared by the active validator set at the
/// registry threshold.
pub const EXTERNAL_LANE_RELEASE_CAP_AUTHORITY_INVALID: &str =
    "external_lane_release_cap_authority_invalid";
/// A cap amendment whose activation height is not its proposal slot.
pub const EXTERNAL_LANE_RELEASE_CAP_SLOT_INVALID: &str = "external_lane_release_cap_slot_invalid";
/// A committee move not declared by the active validator set at the
/// registry threshold.
pub const EXTERNAL_LANE_COMMITTEE_MOVE_AUTHORITY_INVALID: &str =
    "external_lane_committee_move_authority_invalid";
/// A committee move whose activation height is not its proposal slot.
pub const EXTERNAL_LANE_COMMITTEE_MOVE_SLOT_INVALID: &str =
    "external_lane_committee_move_slot_invalid";

/// The governed lane amendment `kind` names, `None` for any other kind:
/// its lane, its label, and its lane, authority and slot refusal codes.
fn governed_lane_amendment(
    kind: &str,
) -> Option<(&str, &'static str, [&'static str; 3])> {
    if let Some(lane_id) = external_lane_release_cap_amendment_lane(kind) {
        return Some((
            lane_id,
            "release cap amendment",
            [
                EXTERNAL_LANE_RELEASE_CAP_LANE_NOT_CONFIGURED,
                EXTERNAL_LANE_RELEASE_CAP_AUTHORITY_INVALID,
                EXTERNAL_LANE_RELEASE_CAP_SLOT_INVALID,
            ],
        ));
    }
    let (lane_id, _) = external_lane_committee_amendment_parts(kind)?;
    Some((
        lane_id,
        "committee move",
        [
            EXTERNAL_LANE_NOT_CONFIGURED,
            EXTERNAL_LANE_COMMITTEE_MOVE_AUTHORITY_INVALID,
            EXTERNAL_LANE_COMMITTEE_MOVE_SLOT_INVALID,
        ],
    ))
}

/// Validators of `validator_count` whose signatures a governed lane
/// amendment needs: the registry threshold ⌊2n/3⌋ + 1 (design sections 4.1,
/// 5.1 and 5.2).
pub fn external_lane_amendment_threshold(validator_count: usize) -> usize {
    validator_count.saturating_mul(2) / 3 + 1
}

/// The refusal of a cap amendment or committee move for a lane none of
/// `lane_ids` names, `None` for a configured lane or any other kind.
pub fn external_lane_amendment_lane_refusal<'a>(
    kind: &str,
    mut lane_ids: impl Iterator<Item = &'a str>,
) -> Option<(&'static str, String)> {
    let (lane_id, label, [code, _, _]) = governed_lane_amendment(kind)?;
    (!lane_ids.any(|configured| configured == lane_id)).then(|| {
        (
            code,
            format!("{label} `{kind}` names a lane genesis does not configure"),
        )
    })
}

/// The refusal of a cap amendment's or committee move's authority, `None`
/// for any other kind: it must be declared by `active_validators` (its own
/// set when the caller has no registry), with a quorum and support of at
/// least the registry threshold.
pub fn external_lane_amendment_authority_refusal(
    amendment: &crate::GovernanceAmendment,
    active_validators: Option<&[String]>,
) -> Option<(&'static str, String)> {
    let (_, label, [_, code, _]) = governed_lane_amendment(&amendment.kind)?;
    let validators = active_validators.unwrap_or(&amendment.validators);
    let threshold = external_lane_amendment_threshold(validators.len());
    if amendment.validators != validators {
        return Some((code, format!("{label} must declare the active validator set")));
    }
    // `support.len()` alone is compared with the threshold: distinct in-set
    // support is enforced by the set equality above and by the
    // signed-authorization verification at admission.
    (amendment.quorum < threshold || amendment.support.len() < threshold).then(|| {
        (
            code,
            format!(
                "{label} needs quorum and support of at least the registry threshold {threshold} of {}",
                validators.len()
            ),
        )
    })
}

/// The refusal of a cap amendment's or committee move's lifecycle, `None`
/// for any other kind: it carries its proposal slot as its activation
/// height (`slot` when the caller knows it) and is not paused. The slot
/// gives each request its own id, so a repeated request is a new amendment.
pub fn external_lane_amendment_slot_refusal(
    amendment: &crate::GovernanceAmendment,
    slot: Option<u64>,
) -> Option<(&'static str, String)> {
    let (_, label, [_, _, code]) = governed_lane_amendment(&amendment.kind)?;
    (amendment.paused
        || amendment.activation_height == 0
        || slot.is_some_and(|slot| amendment.activation_height != slot))
    .then(|| {
        (
            code,
            format!("{label} must name its proposal slot as its activation height and not be paused"),
        )
    })
}

const EXTERNAL_LANE_ROUTE_CONFIG_DOMAIN: &[u8] = b"postfiat.external_lane.route_config.v1";
const EXTERNAL_LANE_RELEASE_HISTORY_DOMAIN: &[u8] = b"postfiat.external_lane.release_history.v1";
/// Domain tags of an export packet: its digest, the hash chain over a lane's
/// packets, and the two commitments the Base lock consumes.
pub const EXTERNAL_LANE_EXPORT_PACKET_DOMAIN: &[u8] = b"postfiat.external_lane.export_packet.v1";
pub const EXTERNAL_LANE_EXPORT_HISTORY_DOMAIN: &[u8] = b"postfiat.external_lane.export_history.v1";
pub const EXTERNAL_LANE_EXPORT_WITHDRAWAL_ID_DOMAIN: &[u8] =
    b"postfiat.external_lane.export_withdrawal_id.v1";
pub const EXTERNAL_LANE_EXPORT_BURN_TX_DOMAIN: &[u8] = b"postfiat.external_lane.export_burn_tx.v1";
/// A release history root is 48 bytes (SHA3-384).
const EXTERNAL_LANE_RELEASE_HISTORY_ROOT_HEX_LEN: usize = 96;

// Refusal codes of `external_lane_release_v1`. A refused release changes no
// state and consumes no Lock nonce, so it can be retried.
pub const EXTERNAL_LANE_NOT_CONFIGURED: &str = "external_lane_not_configured";
/// `external_lane_export_v1` is recognised and refused until the export
/// accounting and the Base release path are built
/// (`docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md`).
pub const EXTERNAL_LANE_EXPORT_NOT_SUPPORTED: &str = "external_lane_export_not_supported";
/// An export whose Base recipient is the lane's own lock contract, which a
/// release can never pay.
pub const EXTERNAL_LANE_EXPORT_RECIPIENT_IS_LOCK: &str = "external_lane_export_recipient_is_lock";
/// `external_lane_export_v1` before block headers carry the external-lane
/// export root: an export accepted then could never be proven on Base.
pub const EXTERNAL_LANE_EXPORT_ROOT_NOT_ACTIVE: &str = "external_lane_export_root_not_active";
/// An export while the lane's active release cap is zero: the governed halt
/// stops exports as it stops releases.
pub const EXTERNAL_LANE_EXPORT_HALTED: &str = "external_lane_export_halted";
/// An export that passes every rule but was executed without governed caps
/// (the bare `execute_asset_transaction` entry): it fails closed, since the
/// genesis cap it saw may no longer be the active one.
pub const EXTERNAL_LANE_EXPORT_NEEDS_GOVERNED_CAPS: &str =
    "external_lane_export_needs_governed_caps";
/// A verified release executed without governed caps and committees: it
/// fails closed, as an export does.
pub const EXTERNAL_LANE_RELEASE_NEEDS_GOVERNED_CAPS: &str =
    "external_lane_release_needs_governed_caps";
/// A verified release executed with governed caps but without the governed
/// committees: the committee it was checked against may have moved.
pub const EXTERNAL_LANE_RELEASE_NEEDS_GOVERNED_COMMITTEES: &str =
    "external_lane_release_needs_governed_committees";
/// An export above `released_atoms - exported_atoms`.
pub const EXTERNAL_LANE_EXPORT_ABOVE_RELEASED: &str = "external_lane_export_above_released";
pub const EXTERNAL_LANE_EXPORT_NONCE_EXHAUSTED: &str = "external_lane_export_nonce_exhausted";
pub const EXTERNAL_LANE_EXPORT_RECORDS_FULL: &str = "external_lane_export_records_full";
/// A lane whose `exported_atoms` exceed its `released_atoms`: a corrupt
/// record no rule can produce.
pub const EXTERNAL_LANE_STATE_CORRUPT: &str = "external_lane_state_corrupt";
/// A release whose `released_atoms + amount` overflows u64.
pub const EXTERNAL_LANE_COUNTER_OVERFLOW: &str = "external_lane_counter_overflow";
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
    /// Whether `external_lane_export_v1` is applied. Absent reads as false,
    /// and false is not encoded, so a lane without exports keeps its genesis
    /// JSON, its genesis hash and its route-config digest.
    #[serde(default, skip_serializing_if = "exports_enabled_is_false")]
    pub exports_enabled: bool,
}

fn exports_enabled_is_false(enabled: &bool) -> bool {
    !*enabled
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
    /// committed cap amendments in commit order. Genesis parse refuses more
    /// than [`EXTERNAL_LANE_MAX_LANES`] lanes; a lane past the table is
    /// halted by [`Self::cap_bps_at`] rather than left at its genesis cap.
    pub fn from_amendments(
        lanes: &[GenesisExternalLane],
        amendments_of: impl Fn(&str) -> Vec<ExternalLaneReleaseCapAmendment>,
    ) -> Self {
        debug_assert!(
            lanes.len() <= EXTERNAL_LANE_MAX_LANES,
            "{} external lanes exceed the cap table of {EXTERNAL_LANE_MAX_LANES}",
            lanes.len()
        );
        let mut caps = Self::genesis();
        for (slot, lane) in caps.rules.iter_mut().zip(lanes) {
            let amendments = amendments_of(&lane.id);
            if !amendments.is_empty() {
                *slot = Some(lane.release_cap_rule(&amendments));
            }
        }
        caps
    }

    /// The cap of the lane at `lane_index` at `height`; 0 for an index the
    /// table cannot hold, so such a lane releases nothing.
    pub fn cap_bps_at(&self, lane_index: usize, lane: &GenesisExternalLane, height: u64) -> u32 {
        self.rules.get(lane_index).map_or(0, |rule| {
            rule.map_or(lane.release_cap_bps, |rule| rule.cap_bps_at(height))
        })
    }
}

/// A committed committee move of one lane: its id, the new epoch, the
/// binding its kind names and the height governance applied it, in commit
/// order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLaneCommitteeAmendment {
    pub amendment_id: String,
    pub committee_epoch: u64,
    pub binding: String,
    pub committed_height: u64,
}

/// A committee set by a move and the height from which it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLaneCommitteeChange {
    pub amendment_id: String,
    pub committee_epoch: u64,
    pub binding: String,
    pub effective_height: u64,
}

/// The committee epoch in force at a height and the move that set it
/// (`None` while the genesis policy's committee holds).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExternalLaneCommitteeSchedule {
    pub committee_epoch: u64,
    pub active: Option<ExternalLaneCommitteeChange>,
}

/// Which registered committee a lane's checkpoints must carry: the genesis
/// policy's, or the one a move bound by epoch and binding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExternalLaneCommitteeSelector {
    Genesis,
    Moved { epoch: u64, binding: [u8; 48] },
}

impl ExternalLaneCommitteeSelector {
    pub fn matches(&self, lane: &GenesisExternalLane, committee: &FastSwapCommitteeV1) -> bool {
        let epoch = committee.domain.committee_epoch;
        let root = &committee.domain.committee_root.0;
        match self {
            Self::Genesis => {
                epoch == lane.policy.committee_epoch
                    && lane_hex_bytes::<48>(&lane.policy.committee_root).as_ref() == Some(root)
            }
            Self::Moved {
                epoch: moved,
                binding,
            } => {
                epoch == *moved
                    && lane_hex_bytes::<48>(&external_lane_committee_binding(&lane.id, epoch, root))
                        .as_ref()
                        == Some(binding)
            }
        }
    }

    /// The committee epoch a release record names: `None` for the genesis
    /// committee, so a lane that never moved records as before.
    pub fn recorded_epoch(&self) -> Option<u64> {
        match self {
            Self::Genesis => None,
            Self::Moved { epoch, .. } => Some(*epoch),
        }
    }
}

/// The committee of one lane for every height from its last committed move
/// on: `before`, then `after` from `from_height`. `Copy`, so execution rules
/// can carry it. As the cap rule (`release_cap_rule`), it is correct for
/// every height from the last committed move on, which is every height a
/// block can still execute at; earlier heights read
/// `GenesisExternalLane::committee_schedule_at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalLaneCommitteeRule {
    pub before: ExternalLaneCommitteeSelector,
    pub after: ExternalLaneCommitteeSelector,
    pub from_height: u64,
}

/// The governed committee of each genesis lane, by its index in the genesis
/// list; a lane without a committed move keeps its genesis committee.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalLaneCommittees {
    rules: [Option<ExternalLaneCommitteeRule>; EXTERNAL_LANE_MAX_LANES],
}

impl Default for ExternalLaneCommittees {
    fn default() -> Self {
        Self::genesis()
    }
}

impl ExternalLaneCommittees {
    /// Every lane on its genesis committee.
    pub const fn genesis() -> Self {
        Self {
            rules: [None; EXTERNAL_LANE_MAX_LANES],
        }
    }

    /// The table for `lanes` under `amendments_of(lane_id)`, each lane's
    /// committed committee moves in commit order.
    pub fn from_amendments(
        lanes: &[GenesisExternalLane],
        amendments_of: impl Fn(&str) -> Vec<ExternalLaneCommitteeAmendment>,
    ) -> Self {
        let mut committees = Self::genesis();
        for (slot, lane) in committees.rules.iter_mut().zip(lanes) {
            let amendments = amendments_of(&lane.id);
            if !amendments.is_empty() {
                *slot = Some(lane.committee_rule(&amendments));
            }
        }
        committees
    }

    /// The committee the lane at `lane_index` requires at `height`.
    pub fn selector_at(&self, lane_index: usize, height: u64) -> ExternalLaneCommitteeSelector {
        self.rules
            .get(lane_index)
            .copied()
            .flatten()
            .map_or(ExternalLaneCommitteeSelector::Genesis, |rule| {
                if height >= rule.from_height {
                    rule.after
                } else {
                    rule.before
                }
            })
    }
}

/// Whether `kind` is of the committee move family, well formed or not.
pub fn is_external_lane_committee_amendment_kind(kind: &str) -> bool {
    kind.split(':').next() == Some(GOVERNANCE_KIND_EXTERNAL_LANE_COMMITTEE_EPOCH_PREFIX_V1)
}

/// The refusal of a committee move whose epoch does not advance the lane's
/// committee (design section 4.1), `None` for any other kind: the lane must
/// be one of `genesis_lanes`, and the epoch must be above that lane's
/// genesis policy epoch and every epoch `committed` moved it to. It needs no
/// ledger, so signed-batch admission checks it too.
pub fn external_lane_committee_epoch_refusal(
    genesis_lanes: &[GenesisExternalLane],
    committed: &[ExternalLaneCommitteeAmendment],
    kind: &str,
    value: u32,
) -> Option<(&'static str, String)> {
    if !is_external_lane_committee_amendment_kind(kind) {
        return None;
    }
    if let Some(refusal) = external_lane_amendment_refusal(kind, value) {
        return Some(refusal);
    }
    let (lane_id, _) = external_lane_committee_amendment_parts(kind)?;
    let Some(lane) = genesis_lanes.iter().find(|lane| lane.id == lane_id) else {
        return Some((
            EXTERNAL_LANE_NOT_CONFIGURED,
            format!("committee move names external lane `{lane_id}`, which genesis never configured"),
        ));
    };
    let epoch = u64::from(value);
    let previous = committed
        .iter()
        .map(|amendment| amendment.committee_epoch)
        .fold(lane.policy.committee_epoch, u64::max);
    (epoch <= previous).then(|| {
        (
            EXTERNAL_LANE_COMMITTEE_EPOCH_NOT_ADVANCED,
            format!(
                "external lane `{lane_id}` already names committee epoch {previous}; a move must advance it"
            ),
        )
    })
}

/// The refusal of a committee move against the ledger, the genesis lanes and
/// the lane's committed moves (design section 4.1), `None` for any other
/// kind; a kind of the family without one well-formed lane id and binding is
/// refused: the lane must be configured; the kind's binding must hash the
/// lane, the value's epoch and the root of the registered committee of that
/// epoch; that committee must be the newest registered and not fenced; and
/// the epoch must advance (`external_lane_committee_epoch_refusal`).
pub fn external_lane_committee_move_refusal(
    ledger: &LedgerState,
    genesis_lanes: &[GenesisExternalLane],
    committed: &[ExternalLaneCommitteeAmendment],
    kind: &str,
    value: u32,
) -> Option<(&'static str, String)> {
    if !is_external_lane_committee_amendment_kind(kind) {
        return None;
    }
    if let Some(refusal) = external_lane_amendment_refusal(kind, value) {
        return Some(refusal);
    }
    let (lane_id, binding) = external_lane_committee_amendment_parts(kind)?;
    if !ledger.external_lanes.iter().any(|lane| lane.lane_id == lane_id) {
        return Some((
            EXTERNAL_LANE_NOT_CONFIGURED,
            format!("committee move names external lane `{lane_id}`, which genesis never configured"),
        ));
    }
    let epoch = u64::from(value);
    let Some(committee) = ledger.fastswap_committees.iter().find(|committee| {
        committee.domain.committee_epoch == epoch
            && external_lane_committee_binding(lane_id, epoch, &committee.domain.committee_root.0)
                == binding
    }) else {
        return Some((
            EXTERNAL_LANE_COMMITTEE_BINDING_MISMATCH,
            format!(
                "committee move of external lane `{lane_id}` binds no registered committee of epoch {epoch}"
            ),
        ));
    };
    let current = committee.validate().is_ok()
        && ledger.fastswap_committees.last() == Some(committee)
        && !ledger
            .fast_lane_prepare_fences
            .iter()
            .any(|fence| fence.committee_epoch == epoch);
    if !current {
        return Some((
            EXTERNAL_LANE_COMMITTEE_NOT_CURRENT,
            format!("committee epoch {epoch} must be the newest registered committee and not fenced"),
        ));
    }
    external_lane_committee_epoch_refusal(genesis_lanes, committed, kind, value)
}

fn lane_hex_bytes<const N: usize>(value: &str) -> Option<[u8; N]> {
    let digits = value.strip_prefix("0x").unwrap_or(value);
    if digits.len() != N * 2 {
        return None;
    }
    let mut bytes = [0_u8; N];
    for (index, byte) in bytes.iter_mut().enumerate() {
        *byte = u8::from_str_radix(digits.get(index * 2..index * 2 + 2)?, 16).ok()?;
    }
    Some(bytes)
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

    /// The committee in force at `height` under the lane's committed
    /// committee moves, in commit order (design section 4.1): each move
    /// holds from the block after it commits, and the latest one in force
    /// wins.
    pub fn committee_schedule_at(
        &self,
        amendments: &[ExternalLaneCommitteeAmendment],
        height: u64,
    ) -> ExternalLaneCommitteeSchedule {
        let active = amendments
            .iter()
            .rev()
            .find(|amendment| amendment.committed_height.saturating_add(1) <= height)
            .map(|amendment| ExternalLaneCommitteeChange {
                amendment_id: amendment.amendment_id.clone(),
                committee_epoch: amendment.committee_epoch,
                binding: amendment.binding.clone(),
                effective_height: amendment.committed_height.saturating_add(1),
            });
        ExternalLaneCommitteeSchedule {
            committee_epoch: active
                .as_ref()
                .map_or(self.policy.committee_epoch, |change| change.committee_epoch),
            active,
        }
    }

    /// The committee for every height from the last committed move on.
    pub fn committee_rule(
        &self,
        amendments: &[ExternalLaneCommitteeAmendment],
    ) -> ExternalLaneCommitteeRule {
        let last = amendments
            .iter()
            .map(|amendment| amendment.committed_height)
            .max()
            .unwrap_or(0);
        let selector = |schedule: ExternalLaneCommitteeSchedule| {
            schedule.active.map_or(ExternalLaneCommitteeSelector::Genesis, |change| {
                ExternalLaneCommitteeSelector::Moved {
                    epoch: change.committee_epoch,
                    // Fail closed: a committed binding is 96 lowercase hex
                    // digits (`external_lane_amendment_refusal`); one that did
                    // not parse would become the all-zero binding, which no
                    // committee root hashes to, so every release is refused.
                    binding: lane_hex_bytes::<48>(&change.binding).unwrap_or([0; 48]),
                }
            })
        };
        ExternalLaneCommitteeRule {
            before: selector(self.committee_schedule_at(amendments, last)),
            after: selector(self.committee_schedule_at(amendments, last.saturating_add(1))),
            from_height: last.saturating_add(1),
        }
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
        // A cap of zero is a governed halt, which only an amendment may set
        // (design 5.1). Rounding must not produce one at genesis: the lane would
        // accept every release proof and refuse every release; only a governed
        // cap amendment can raise it, and a raise waits seven windows (design
        // 5.1).
        if self.window_cap_atoms() == 0 {
            return Err(format!(
                "external lane `{}` reserves {} atoms at {} bps, so its window cap rounds down to 0 atoms, so the lane could release nothing",
                self.id, self.reserved_atoms, self.release_cap_bps
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
    /// The committee epoch a governed move named when this release was
    /// proven; absent while the lane's genesis committee held, so a lane
    /// that never moved records as before.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub committee_epoch: Option<u64>,
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

/// One accepted `external_lane_export_v1`: the packet a Base release pays.
/// Every field is one PFTL owns; the verifier's route profile and asset id
/// are not part of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExternalLaneExportPacket {
    pub lane_id: String,
    pub export_nonce: u64,
    pub sender: String,
    pub base_recipient: String,
    pub amount_atoms: u64,
    pub exported_at_height: u64,
    /// The accepted transaction's id.
    pub tx_id: String,
}

fn lane_digest_hex(digest: [u8; 48]) -> String {
    format!(
        "0x{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

impl ExternalLaneExportPacket {
    /// SHA3-384 over `EXTERNAL_LANE_EXPORT_PACKET_DOMAIN`, a zero byte and
    /// the packet's JSON, as a release record is hashed.
    pub fn digest(&self) -> Result<[u8; 48], String> {
        let encoded = serde_json::to_vec(self)
            .map_err(|error| format!("encode external lane export packet: {error}"))?;
        let mut hasher = Sha3_384::new();
        hasher.update(EXTERNAL_LANE_EXPORT_PACKET_DOMAIN);
        hasher.update([0_u8]);
        hasher.update(encoded);
        Ok(hasher.finalize().into())
    }

    /// SHA3-384 over `EXTERNAL_LANE_EXPORT_WITHDRAWAL_ID_DOMAIN`, then each
    /// of the chain id, the lane id, the export nonce (8 bytes big-endian)
    /// and the packet digest (48 bytes), each after a zero byte.
    pub fn withdrawal_id_commitment(&self, chain_id: &str) -> Result<[u8; 48], String> {
        let digest = self.digest()?;
        let mut hasher = Sha3_384::new();
        hasher.update(EXTERNAL_LANE_EXPORT_WITHDRAWAL_ID_DOMAIN);
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

    /// SHA3-384 over `EXTERNAL_LANE_EXPORT_BURN_TX_DOMAIN`, then each of the
    /// chain id, the lane id and the accepted transaction id, each after a
    /// zero byte.
    pub fn burn_tx_commitment(&self, chain_id: &str) -> [u8; 48] {
        let mut hasher = Sha3_384::new();
        hasher.update(EXTERNAL_LANE_EXPORT_BURN_TX_DOMAIN);
        for part in [chain_id, &self.lane_id, &self.tx_id] {
            hasher.update([0_u8]);
            hasher.update(part.as_bytes());
        }
        hasher.finalize().into()
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.amount_atoms == 0 {
            return Err(format!(
                "external lane export nonce {} has zero amount",
                self.export_nonce
            ));
        }
        if self.sender.is_empty() || self.tx_id.is_empty() {
            return Err(format!(
                "external lane export nonce {} has no sender or transaction id",
                self.export_nonce
            ));
        }
        validate_lane_hex(
            "export base_recipient",
            &self.base_recipient,
            EXTERNAL_LANE_EVM_ADDRESS_HEX_LEN,
        )
    }
}

/// The export history root after `packet`: SHA3-384 over a domain tag, the
/// previous root (empty for the first packet) and the packet's digest.
pub fn external_lane_export_history_root(
    previous: Option<&str>,
    packet: &ExternalLaneExportPacket,
) -> Result<String, String> {
    let mut hasher = Sha3_384::new();
    hasher.update(EXTERNAL_LANE_EXPORT_HISTORY_DOMAIN);
    hasher.update([0_u8]);
    hasher.update(previous.unwrap_or_default().as_bytes());
    hasher.update([0_u8]);
    hasher.update(packet.digest()?);
    Ok(lane_digest_hex(hasher.finalize().into()))
}

/// Live record of a genesis external lane. Its remainder
/// (`reserved_atoms - (released_atoms - exported_atoms)`) is native supply
/// that is not in circulation; `external_lane_release_v1` is the only writer
/// of the release fields and `external_lane_export_v1` of the export fields,
/// and every released atom has a stored record in `releases` or is in
/// the committed pruned aggregate (`pruned_released_atoms`) and root
/// (`pruned_release_history_root`).
///
/// The state commitment covers the release fields, the pruned fields and
/// `release_history_root`, a SHA3-384 hash chain over the records in
/// application order; the stored records stay in the ledger for reads and
/// replay and, when a replay base is loaded, are chained from the pruned root
/// (or from nothing when unpruned) to `release_history_root`, so the per-block
/// commitment does not grow with them.
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
    /// Records pruned behind the finalized height: how many, the atoms they
    /// released, and the history root after the last of them, from which the
    /// stored `releases` chain to `release_history_root`. Omitted while
    /// nothing is pruned, so an unpruned lane encodes and commits as before.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pruned_release_count: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pruned_released_atoms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pruned_release_history_root: Option<String>,
    /// Release history in application order, after the pruned records; not
    /// in the state commitment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub releases: Vec<ExternalLaneReleaseRecord>,
    /// Atoms exported back to the lane and the next export nonce. Omitted
    /// while zero, so a lane without exports encodes and commits as before.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub exported_atoms: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub next_export_nonce: u64,
    /// Hash chain over the export packets, and the pruned packets' count,
    /// atoms and root, as for releases.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub export_history_root: Option<String>,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pruned_export_count: u64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub pruned_exported_atoms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pruned_export_history_root: Option<String>,
    /// Export packets in nonce order, after the pruned ones; not in the
    /// state commitment.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub exports: Vec<ExternalLaneExportPacket>,
}

fn is_zero(value: &u64) -> bool {
    *value == 0
}

/// What pruning at a finalized height removes: the leading records, their
/// atoms and the history root after the last of them.
struct ExternalLaneReleasePrune {
    count: usize,
    atoms: u64,
    root: Option<String>,
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
    #[serde(skip_serializing_if = "is_zero")]
    pruned_release_count: u64,
    #[serde(skip_serializing_if = "is_zero")]
    pruned_released_atoms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pruned_release_history_root: Option<&'a str>,
    #[serde(skip_serializing_if = "is_zero")]
    exported_atoms: u64,
    #[serde(skip_serializing_if = "is_zero")]
    next_export_nonce: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    export_history_root: Option<&'a str>,
    #[serde(skip_serializing_if = "is_zero")]
    pruned_export_count: u64,
    #[serde(skip_serializing_if = "is_zero")]
    pruned_exported_atoms: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pruned_export_history_root: Option<&'a str>,
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
            pruned_release_count: 0,
            pruned_released_atoms: 0,
            pruned_release_history_root: None,
            releases: Vec::new(),
            exported_atoms: 0,
            next_export_nonce: 0,
            export_history_root: None,
            pruned_export_count: 0,
            pruned_exported_atoms: 0,
            pruned_export_history_root: None,
            exports: Vec::new(),
        }
    }

    /// Every release ever applied, pruned or stored; `None` when the sum
    /// does not fit in u64, which no valid record reaches.
    pub fn release_count(&self) -> Option<u64> {
        self.pruned_release_count
            .checked_add(u64::try_from(self.releases.len()).ok()?)
    }

    /// The leading records that pruning at `finalized_height` removes: those
    /// released at or below `finalized_height` minus the retention window and
    /// in a window before `open_window`. The records retained are the
    /// retention window's and the open window's, which the cap accounting and
    /// the window check still need. Records are in height order, so the
    /// prunable ones are a prefix.
    fn release_prune(
        &self,
        lane: &GenesisExternalLane,
        finalized_height: u64,
        open_window: u64,
    ) -> Result<ExternalLaneReleasePrune, String> {
        let retention = lane
            .release_window_blocks
            .max(1)
            .saturating_mul(EXTERNAL_LANE_RELEASE_RETENTION_WINDOWS);
        let count = finalized_height.checked_sub(retention).map_or(0, |cutoff| {
            self.releases
                .iter()
                .take_while(|record| {
                    record.released_at_height <= cutoff
                        && lane.window_index_at(record.released_at_height) < open_window
                })
                .count()
        });
        let mut root = self.pruned_release_history_root.clone();
        let mut atoms = self.pruned_released_atoms;
        for record in &self.releases[..count] {
            root = Some(external_lane_release_history_root(root.as_deref(), record)?);
            atoms = atoms.checked_add(record.amount_atoms).ok_or_else(|| {
                format!("external lane `{}` pruned releases overflow", self.lane_id)
            })?;
        }
        Ok(ExternalLaneReleasePrune { count, atoms, root })
    }

    /// `released_atoms - exported_atoms`: what has left the lane and not
    /// come back. Exports never exceed releases, so a failure is a corrupt
    /// record.
    pub fn net_released_atoms(&self) -> Result<u64, (&'static str, String)> {
        self.released_atoms
            .checked_sub(self.exported_atoms)
            .ok_or_else(|| {
                (
                    EXTERNAL_LANE_STATE_CORRUPT,
                    format!(
                        "external lane `{}` exported {} atoms above its {} released",
                        self.lane_id, self.exported_atoms, self.released_atoms
                    ),
                )
            })
    }

    /// `reserved - (released - exported)`, in that net form: the form
    /// `reserved - released + exported` underflows after a full cycle.
    pub fn remainder_atoms(&self) -> Result<u64, String> {
        let net_released = self.net_released_atoms().map_err(|(_, error)| error)?;
        self.reserved_atoms
            .checked_sub(net_released)
            .ok_or_else(|| {
                format!(
                    "external lane `{}` released {net_released} net atoms above its reserve {}",
                    self.lane_id, self.reserved_atoms
                )
            })
    }

    /// Every export ever applied, pruned or stored; `None` when the sum
    /// does not fit in u64, which no valid record reaches.
    pub fn export_count(&self) -> Option<u64> {
        self.pruned_export_count
            .checked_add(u64::try_from(self.exports.len()).ok()?)
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
        if let Some(root) = &self.pruned_release_history_root {
            validate_lane_hex(
                "pruned_release_history_root",
                root,
                EXTERNAL_LANE_RELEASE_HISTORY_ROOT_HEX_LEN,
            )?;
        }
        for (field, root) in [
            ("export_history_root", &self.export_history_root),
            ("pruned_export_history_root", &self.pruned_export_history_root),
        ] {
            if let Some(root) = root {
                validate_lane_hex(field, root, EXTERNAL_LANE_RELEASE_HISTORY_ROOT_HEX_LEN)?;
            }
        }
        if self.next_export_nonce < self.pruned_export_count
            || (self.exported_atoms > 0) != (self.next_export_nonce > 0)
        {
            return Err(format!(
                "external lane `{}` has inconsistent export counters",
                self.lane_id
            ));
        }
        Ok(())
    }

    /// The export fields must be what the packets add up to: nonces in
    /// order from the pruned count to `next_export_nonce`, and
    /// `exported_atoms` their sum. Walks every packet, so it runs when a
    /// replay base is loaded, not per block.
    fn validate_recorded_exports(&self) -> Result<(), String> {
        let consistent = if self.pruned_export_count == 0 {
            self.pruned_exported_atoms == 0 && self.pruned_export_history_root.is_none()
        } else {
            // Pruning keeps the open window's packets, so a pruned lane still
            // stores its latest packet, as `validate_recorded_releases` requires.
            self.pruned_export_history_root.is_some() && !self.exports.is_empty()
        };
        if !consistent || self.export_count() != Some(self.next_export_nonce) {
            return Err(format!(
                "external lane `{}` has inconsistent export records",
                self.lane_id
            ));
        }
        let mut exported = self.pruned_exported_atoms;
        let mut previous_height = 0_u64;
        let mut root = self.pruned_export_history_root.clone();
        for (offset, packet) in self.exports.iter().enumerate() {
            packet
                .validate()
                .map_err(|error| format!("external lane `{}`: {error}", self.lane_id))?;
            let expected_nonce = self
                .pruned_export_count
                .checked_add(offset as u64)
                .ok_or_else(|| {
                    format!(
                        "external lane `{}` has inconsistent export records",
                        self.lane_id
                    )
                })?;
            if packet.lane_id != self.lane_id
                || packet.export_nonce != expected_nonce
                || packet.exported_at_height < previous_height
            {
                return Err(format!(
                    "external lane `{}` records export nonce {} out of order",
                    self.lane_id, packet.export_nonce
                ));
            }
            previous_height = packet.exported_at_height;
            exported = exported.checked_add(packet.amount_atoms).ok_or_else(|| {
                format!("external lane `{}` recorded exports overflow", self.lane_id)
            })?;
            root = Some(external_lane_export_history_root(root.as_deref(), packet)?);
        }
        if exported != self.exported_atoms || root != self.export_history_root {
            return Err(format!(
                "external lane `{}` has exported_atoms {} and export root {:?}, but its packets give {exported} and {root:?}",
                self.lane_id, self.exported_atoms, self.export_history_root
            ));
        }
        Ok(())
    }

    /// The release fields must be exactly what the recorded releases add up
    /// to: the nonce set is the records' nonces, each once, and
    /// `released_atoms` is the sum of their amounts. Walks every record, so
    /// it runs when a replay base is loaded, not per block.
    fn validate_recorded_releases(&self) -> Result<(), String> {
        let pruned = self.pruned_release_count;
        let pruned_fields_consistent = if pruned == 0 {
            self.pruned_released_atoms == 0 && self.pruned_release_history_root.is_none()
        } else {
            // Pruning keeps the open window's records, so a pruned lane still
            // stores its latest release. The pruned atoms are checked by the
            // exact sum below, the pruned count by the nonce count and the
            // pruned root by the chain to `release_history_root`.
            self.pruned_release_history_root.is_some()
                && !self.releases.is_empty()
        };
        if !pruned_fields_consistent {
            return Err(format!(
                "external lane `{}` has inconsistent pruned release fields",
                self.lane_id
            ));
        }
        let mut nonces = BTreeSet::new();
        let mut released = self.pruned_released_atoms;
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
        if pruned == 0 && nonces != self.released_lock_nonces {
            return Err(format!(
                "external lane `{}` released Lock nonces {:?} differ from its recorded releases {:?}",
                self.lane_id, self.released_lock_nonces, nonces
            ));
        }
        // Replay protection never prunes: the set keeps every paid nonce, one
        // per release, the stored ones among them.
        if pruned > 0
            && (!nonces.is_subset(&self.released_lock_nonces)
                || u64::try_from(self.released_lock_nonces.len()).ok() != self.release_count())
        {
            return Err(format!(
                "external lane `{}` released Lock nonces ({}) differ from its recorded releases ({} stored, {pruned} pruned)",
                self.lane_id,
                self.released_lock_nonces.len(),
                nonces.len()
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
        self.validate_against_genesis_with_caps(lane, &[])
    }

    /// [`Self::validate_against_genesis`] with each window checked against
    /// the cap active at the height of each of its releases, under the lane's
    /// committed cap amendments.
    pub fn validate_against_genesis_with_caps(
        &self,
        lane: &GenesisExternalLane,
        cap_amendments: &[ExternalLaneReleaseCapAmendment],
    ) -> Result<(), String> {
        self.validate_against_genesis_governed(lane, cap_amendments, &[])
    }

    /// [`Self::validate_against_genesis_with_caps`] with each stored release
    /// also naming the committee active at its height under the lane's
    /// committed committee moves.
    pub fn validate_against_genesis_governed(
        &self,
        lane: &GenesisExternalLane,
        cap_amendments: &[ExternalLaneReleaseCapAmendment],
        committee_amendments: &[ExternalLaneCommitteeAmendment],
    ) -> Result<(), String> {

        self.validate()?;
        self.validate_recorded_releases()?;
        self.validate_recorded_exports()?;
        if !lane.policy.exports_enabled && self.next_export_nonce > 0 {
            return Err(format!(
                "external lane `{}` records exports, but genesis does not enable them",
                self.lane_id
            ));
        }
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
        let mut per_window = BTreeMap::<u64, u64>::new();
        let mut previous_height = 0_u64;
        let mut root = self.pruned_release_history_root.clone();
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
            let committee_epoch = lane
                .committee_schedule_at(committee_amendments, record.released_at_height)
                .active
                .map(|change| change.committee_epoch);
            if record.committee_epoch != committee_epoch {
                return Err(format!(
                    "external lane `{}` records Lock nonce {} under committee epoch {:?}, but the committee active at height {} is epoch {:?} (`None` for the genesis committee)",
                    self.lane_id, record.lock_nonce, record.committee_epoch, record.released_at_height, committee_epoch
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
            let cap = lane.window_cap_atoms_for_bps(
                lane.release_cap_schedule_at(cap_amendments, record.released_at_height)
                    .active_cap_bps,
            );
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

    /// The lane-state release rules only, in order: the amount is positive, a
    /// nonce is paid once, the amount fits the remainder, the window does not
    /// regress, the window stays within its cap, the pruning calculation holds,
    /// and the lane stays under the release record ceiling. Returns a refusal
    /// code and message; nothing is mutated. The proof is checked before this
    /// by `verify_external_lane_lock`, and the recipient's balance overflow and
    /// account reserve after it by `apply_external_lane_release`
    /// (`crates/execution/src/external_lane_release.rs`).
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
        // The operation's own validation refuses a zero amount first; the
        // lane refuses it too, so no API caller can store a zero record.
        if amount_atoms == 0 {
            return Err((
                EXTERNAL_LANE_PROOF_INVALID,
                format!(
                    "external lane `{}` release of Lock nonce {lock_nonce} has zero amount",
                    self.lane_id
                ),
            ));
        }
        self.net_released_atoms()?;
        if self.released_lock_nonces.contains(&lock_nonce) {
            return Err((
                EXTERNAL_LANE_NONCE_ALREADY_RELEASED,
                format!(
                    "external lane `{}` already released Lock nonce {lock_nonce}",
                    self.lane_id
                ),
            ));
        }
        // Net form of design section 4: the new released total, less the
        // exported atoms, stays within the reserve.
        let released = self.released_atoms.checked_add(amount_atoms).ok_or_else(|| {
            (
                EXTERNAL_LANE_COUNTER_OVERFLOW,
                format!(
                    "external lane `{}` released atoms overflow with {amount_atoms} more",
                    self.lane_id
                ),
            )
        })?;
        let net_released = released.checked_sub(self.exported_atoms).ok_or_else(|| {
            (
                EXTERNAL_LANE_STATE_CORRUPT,
                format!("external lane `{}` exported above its releases", self.lane_id),
            )
        })?;
        if net_released > self.reserved_atoms {
            let remainder = self
                .remainder_atoms()
                .map_err(|error| (EXTERNAL_LANE_STATE_CORRUPT, error))?;
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
        // The release prunes before it stores, so the ceiling counts the
        // records that stay.
        let prunable = self
            .release_prune(lane, height, window_index)
            .map_err(|error| (EXTERNAL_LANE_PROOF_INVALID, error))?
            .count;
        let stored = self.releases.len() - prunable;
        if stored >= max_records {
            return Err((
                EXTERNAL_LANE_RELEASE_RECORDS_FULL,
                format!(
                    "external lane `{}` stores {stored} release records, the ceiling {max_records}",
                    self.lane_id
                ),
            ));
        }
        Ok(())
    }

    /// Applies a checked release: records it, adds the nonce and the amount,
    /// moves the window, and prunes the records behind the finalized height.
    /// The node has no rollback path for a committed PFTL block, so the
    /// release's own height is the finalized height; a release is the only writer of records, so the
    /// stored count can only cross the ceiling here. The caller credits the
    /// recipient. Every fallible step runs before the first mutation, so a
    /// refusal leaves the lane as it was.
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
        let prune = self
            .release_prune(lane, record.released_at_height, window_index)
            .map_err(|error| (EXTERNAL_LANE_PROOF_INVALID, error))?;
        if window_index != self.window_index {
            self.window_index = window_index;
            self.window_released_atoms = 0;
        }
        self.window_released_atoms += record.amount_atoms;
        self.released_atoms += record.amount_atoms;
        self.released_lock_nonces.insert(record.lock_nonce);
        self.release_history_root = Some(root);
        if prune.count > 0 {
            self.releases.drain(..prune.count);
            self.pruned_release_count += prune.count as u64;
            self.pruned_released_atoms = prune.atoms;
            self.pruned_release_history_root = prune.root;
        }
        self.releases.push(record);
        Ok(())
    }

    /// The export rules of design section 4, after the halt the caller
    /// checks: the amount fits `released - exported`, the export nonce can
    /// advance, and the stored packets stay under the ceiling. Nothing is
    /// mutated.
    pub fn check_export(
        &self,
        lane: &GenesisExternalLane,
        amount_atoms: u64,
        height: u64,
    ) -> Result<(), (&'static str, String)> {
        self.check_export_within(lane, amount_atoms, height, EXTERNAL_LANE_MAX_RELEASE_RECORDS)
    }

    fn check_export_within(
        &self,
        lane: &GenesisExternalLane,
        amount_atoms: u64,
        height: u64,
        max_records: usize,
    ) -> Result<(), (&'static str, String)> {
        let exportable = self.net_released_atoms()?;
        if amount_atoms == 0 || amount_atoms > exportable {
            return Err((
                EXTERNAL_LANE_EXPORT_ABOVE_RELEASED,
                format!(
                    "external lane `{}` can take back {exportable} released atoms, not {amount_atoms}",
                    self.lane_id
                ),
            ));
        }
        if self.next_export_nonce.checked_add(1).is_none() {
            return Err((
                EXTERNAL_LANE_EXPORT_NONCE_EXHAUSTED,
                format!("external lane `{}` export nonce is exhausted", self.lane_id),
            ));
        }
        let prunable = self.export_prune(lane, height)?.count;
        let stored = self.exports.len() - prunable;
        if stored >= max_records {
            return Err((
                EXTERNAL_LANE_EXPORT_RECORDS_FULL,
                format!(
                    "external lane `{}` stores {stored} export packets, the ceiling {max_records}",
                    self.lane_id
                ),
            ));
        }
        Ok(())
    }

    /// The leading packets pruning at `height` removes, by the release
    /// rule: at or below `height` minus the retention window and in a window
    /// before the one `height` falls in.
    fn export_prune(
        &self,
        lane: &GenesisExternalLane,
        height: u64,
    ) -> Result<ExternalLaneReleasePrune, (&'static str, String)> {
        let retention = lane
            .release_window_blocks
            .max(1)
            .saturating_mul(EXTERNAL_LANE_RELEASE_RETENTION_WINDOWS);
        let open_window = lane.window_index_at(height);
        let count = height.checked_sub(retention).map_or(0, |cutoff| {
            self.exports
                .iter()
                .take_while(|packet| {
                    packet.exported_at_height <= cutoff
                        && lane.window_index_at(packet.exported_at_height) < open_window
                })
                .count()
        });
        let mut root = self.pruned_export_history_root.clone();
        let mut atoms = self.pruned_exported_atoms;
        for packet in &self.exports[..count] {
            root = Some(
                external_lane_export_history_root(root.as_deref(), packet)
                    .map_err(|error| (EXTERNAL_LANE_STATE_CORRUPT, error))?,
            );
            atoms = atoms.checked_add(packet.amount_atoms).ok_or_else(|| {
                (
                    EXTERNAL_LANE_STATE_CORRUPT,
                    format!("external lane `{}` pruned exports overflow", self.lane_id),
                )
            })?;
        }
        Ok(ExternalLaneReleasePrune { count, atoms, root })
    }

    /// Applies a checked export: the packet takes `next_export_nonce`,
    /// `exported_atoms` grows by its amount, the nonce advances and the
    /// packets behind the window are pruned. The caller checks the halt and
    /// debits the sender. Every fallible step runs before the first
    /// mutation, so a refusal leaves the lane as it was.
    pub fn apply_export(
        &mut self,
        lane: &GenesisExternalLane,
        packet: ExternalLaneExportPacket,
    ) -> Result<(), (&'static str, String)> {
        self.apply_export_within(lane, packet, EXTERNAL_LANE_MAX_RELEASE_RECORDS)
    }

    fn apply_export_within(
        &mut self,
        lane: &GenesisExternalLane,
        packet: ExternalLaneExportPacket,
        max_records: usize,
    ) -> Result<(), (&'static str, String)> {
        self.check_export_within(lane, packet.amount_atoms, packet.exported_at_height, max_records)?;
        if packet.lane_id != self.lane_id || packet.export_nonce != self.next_export_nonce {
            return Err((
                EXTERNAL_LANE_STATE_CORRUPT,
                format!(
                    "external lane `{}` export packet must take nonce {}",
                    self.lane_id, self.next_export_nonce
                ),
            ));
        }
        let root = external_lane_export_history_root(self.export_history_root.as_deref(), &packet)
            .map_err(|error| (EXTERNAL_LANE_STATE_CORRUPT, error))?;
        let prune = self.export_prune(lane, packet.exported_at_height)?;
        // `amount <= released - exported` holds, so neither step can wrap.
        self.exported_atoms += packet.amount_atoms;
        self.next_export_nonce += 1;
        self.export_history_root = Some(root);
        if prune.count > 0 {
            self.exports.drain(..prune.count);
            self.pruned_export_count += prune.count as u64;
            self.pruned_exported_atoms = prune.atoms;
            self.pruned_export_history_root = prune.root;
        }
        self.exports.push(packet);
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
            pruned_release_count: self.pruned_release_count,
            pruned_released_atoms: self.pruned_released_atoms,
            pruned_release_history_root: self.pruned_release_history_root.as_deref(),
            exported_atoms: self.exported_atoms,
            next_export_nonce: self.next_export_nonce,
            export_history_root: self.export_history_root.as_deref(),
            pruned_export_count: self.pruned_export_count,
            pruned_exported_atoms: self.pruned_exported_atoms,
            pruned_export_history_root: self.pruned_export_history_root.as_deref(),
        })
        .map_err(|error| format!("encode external lane state: {error}"))
    }
}
