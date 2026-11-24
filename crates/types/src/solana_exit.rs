//! The frozen mainnet checker binds a single immutable descriptor. From the first release
//! (`RELEASES`) only the revision-8 bucket can exit, and every decision is unanimous. From the
//! second release (`SECOND_RELEASES`) every Solana-source bucket of the family in the shared
//! custody can exit (each burn still debits its own series and bucket), close, paid and a
//! re-opening need the registry threshold, owners must be wallet keys and a governed minimum
//! applies. Unset, the second release is off at every height. Governance signs the descriptor
//! through a route-profile activation. The registered exit record is excluded from ingress
//! profile selection.
use super::*;

pub const BUCKET: &str = "4c045e289eaaa2c7ae7592b96ec0eeabe9fa0cfdd032783d359409dcc08d9dd0dfdd7cc8d483d6a88c12dc713522d2cc";
pub const SERIES: &str = "e90627502bea5857aefd003d80af518f0ccb44ad2be8098ec6db2025ccaf8390e8bef31e7538b398a3c94f019d7b3b0f";
pub const POLICY: &str = "5bb5a3d99616945cb7330c07a51c47c0453ff54f6c622f44f18c5b505cc9b13f179a6796a537119a482d788537bd96c7";
pub const PROFILE_HASH: &str = "37e3c05d8e353d93d56819bfdc3b10f314e5ae298cf43a34cefac477d06893953ceca391165ec3bc977c5e5b273b4046";
pub const FAMILY: &str = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b";

const RELEASES: &[(&str, &str, Option<u64>)] = &[(
    "postfiat-wan-devnet-2",
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9",
    Some(1841), // Agreed WAN release height.
)];

pub fn release_height(chain: &str, genesis: &str) -> Option<u64> {
    RELEASES
        .iter()
        .find(|(c, g, _)| *c == chain && *g == genesis)?
        .2
}

/// The second Solana exit release. Unset means off: the build then follows the first release
/// at every height, byte for byte.
const SECOND_RELEASES: &[(&str, &str, Option<u64>)] = &[(
    "postfiat-wan-devnet-2",
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9",
    None, // Governance agrees this height once every validator runs the second release.
)];

#[cfg(any(test, feature = "test-second-release"))]
thread_local! {
    static SECOND_RELEASE_FOR_TESTS: std::cell::Cell<Option<u64>> = const { std::cell::Cell::new(None) };
}

/// Sets a second release height for every chain of this thread (tests only). The first
/// release still applies where `RELEASES` sets it.
#[cfg(any(test, feature = "test-second-release"))]
pub fn set_second_release_for_tests(height: Option<u64>) {
    SECOND_RELEASE_FOR_TESTS.with(|h| h.set(height));
}

pub fn second_release_height(chain: &str, genesis: &str) -> Option<u64> {
    SECOND_RELEASES
        .iter()
        .find(|(c, g, _)| *c == chain && *g == genesis)?
        .2
}

/// Whether the second release's rules apply at `height`. It never applies before the first.
pub fn second_release_active(chain: &str, genesis: &str, height: u64) -> bool {
    let first = release_height(chain, genesis);
    #[cfg(any(test, feature = "test-second-release"))]
    if let Some(second) = SECOND_RELEASE_FOR_TESTS.with(|h| h.get()) {
        return height >= second && first.is_none_or(|f| height >= f);
    }
    first.is_some_and(|f| height >= f)
        && second_release_height(chain, genesis).is_some_and(|second| height >= second)
}

pub fn descriptor() -> VaultBridgeRouteProfileV1 {
    serde_json::from_str(include_str!("solana_exit_profile.json"))
        .expect("checked release descriptor")
}

/// Source domain of the shared Solana custody. Every ingress revision of the route keeps the
/// genesis, vault program and mint, so all their buckets share it. `BUCKET` and `POLICY` remain
/// the revision-8 values of the trusted checkpoint's ingress profile.
pub const SOURCE_DOMAIN: &str = "spl:45296998a6f8e2a784db5d9f95e18fc23f70441a1039446801089879b08c7ef0:e2a6fbfcf1e65a0f3e08d400c97d913e3e4eca81f84c814efa0bbcada24aac8b:c6fa7af3bedbad3a3d65f36aabc97431b1bbe4c2d2f6e0e47ca60203452f5d61";

/// A bucket the exit covers. First release: the revision-8 bucket only. Second release: any
/// bucket of the family in the shared custody, derived from its own ingress policy.
pub fn matches_bucket(bucket: &VaultBridgeBucketState, second: bool) -> bool {
    if !second {
        return bucket.bucket_id == BUCKET
            && bucket.policy_hash == POLICY
            && bucket.asset_id == FAMILY
            && vault_bridge_bucket_id(FAMILY, &bucket.source_domain, POLICY).as_deref()
                == Ok(BUCKET);
    }
    bucket.asset_id == FAMILY
        && bucket.source_domain == SOURCE_DOMAIN
        && vault_bridge_bucket_id(FAMILY, SOURCE_DOMAIN, &bucket.policy_hash).as_deref()
            == Ok(bucket.bucket_id.as_str())
}

pub fn registered(
    governance: &GovernanceState,
) -> Option<(&VaultBridgeRouteProfileRecordV1, &GovernanceAmendment)> {
    if !governance
        .vault_bridge_route_profiles
        .iter()
        .any(|r| r.profile_hash == PROFILE_HASH)
    {
        return None;
    }
    let r = governance
        .authorized_vault_bridge_route_profile(FAMILY, PROFILE_HASH)
        .ok()?;
    let a = governance
        .amendments
        .iter()
        .find(|a| a.amendment_id == r.governance_amendment_id)?;
    let release = release_height(&a.chain_id, &a.genesis_hash)?;
    (r.authorized_height >= release && !a.paused).then_some((r, a))
}

pub fn active(
    g: &GovernanceState,
    height: u64,
) -> Result<(&VaultBridgeRouteProfileRecordV1, &GovernanceAmendment), String> {
    let (record, amendment) = registered(g).ok_or("Solana exit descriptor is not governed")?;
    if retirement_height(g).is_some_and(|h| height >= h) {
        return Err("Solana exit descriptor has been retired".into());
    }
    if height < record.authorized_height || height >= record.profile.expires_at_height {
        return Err("Solana exit descriptor is outside its authorized interval".into());
    }
    Ok((record, amendment))
}

pub const OPEN: &str = "solana_exit_r8_open";
pub const SETTLEMENT_TRUST_CLASS: &str = "VALIDATOR_ATTESTED_SETTLEMENT";
pub const RETIRE: &str = "solana_exit_r8_retire";
pub const CLOSE: &str = "solana_exit_r8_close";
/// The frozen revision-8 tag-3 binding uses the finalized WAN block at 1753.
/// The route profile's activation height is a different field.
pub const TRUSTED_CHECKPOINT_HEIGHT: u64 = 1753;
pub const PAID: &str = "solana_exit_r8_paid:";
/// `solana_exit_min_withdrawal:<atoms>` sets the smallest Solana exit burn.
pub const MIN_WITHDRAWAL: &str = "solana_exit_min_withdrawal:";
/// 0.30 USDC. Each payout leaves the relayer about 4,162,080 lamports of permanent
/// rent (burn marker 897,840, checkpoint PDA 1,224,960, recipient ATA 2,039,280),
/// about 0.29 USDC at 70.54 USD/SOL; a smaller burn costs the relayer more than it moves.
pub const DEFAULT_MIN_WITHDRAWAL_ATOMS: u64 = 300_000;

/// Signatures a Solana exit decision needs from the declared full active set.
/// Closing burns, settling a payout and setting the minimum withdrawal (and a
/// future refund) use the registry threshold of every threshold amendment, so
/// one lost validator cannot block them. Opening and retirement stay unanimous on purpose: opening starts
/// irreversible burns and freezes the committee, and retirement gives up the
/// protection of every holder whose burn the frozen checker must still prove.
/// A re-opening after a close uses the close quorum (`required_quorum`).
/// The first release needs every validator for every decision.
pub fn decision_quorum(kind: &str, validator_count: usize, second: bool) -> usize {
    if second && (kind == CLOSE || kind.starts_with(PAID) || kind.starts_with(MIN_WITHDRAWAL)) {
        external_lane_amendment_threshold(validator_count)
    } else {
        validator_count
    }
}

/// Signatures a decision needs in the current state. A re-opening after a close
/// needs only the close quorum: the first opening was unanimous, and otherwise a
/// close by five while one validator is lost could never be undone, freezing every
/// holder (the committee cannot change while Solana pfUSDC is outstanding).
pub fn required_quorum(kind: &str, reopening: bool, validator_count: usize, second: bool) -> usize {
    if kind == OPEN && reopening {
        decision_quorum(CLOSE, validator_count, second)
    } else {
        decision_quorum(kind, validator_count, second)
    }
}

/// The lowest quorum any state admits, for the stateless domain check.
pub fn signer_floor(kind: &str, validator_count: usize, second: bool) -> usize {
    required_quorum(kind, kind == OPEN, validator_count, second)
}

/// Whether burns were closed since the descriptor's registration, so that an
/// opening now is a re-opening.
pub fn closed_since_registration(g: &GovernanceState) -> bool {
    let Some((descriptor, _)) = registered(g) else {
        return false;
    };
    g.amendment_activation_records.iter().any(|a| {
        a.kind == CLOSE
            && a.value == 1
            && a.activated_height > descriptor.authorized_height
            && a.activation_height == a.activated_height
    })
}

/// The minimum-withdrawal kind exists from the second release only; before it the kind is
/// handled exactly as the first release handles any other kind.
pub fn is_decision(kind: &str, second: bool) -> bool {
    matches!(kind, OPEN | CLOSE | RETIRE)
        || kind.starts_with(PAID)
        || (second && kind.starts_with(MIN_WITHDRAWAL))
}

/// The amount of a minimum-withdrawal decision: canonical decimal atoms, at least 1.
pub fn minimum_withdrawal(kind: &str) -> Result<u64, String> {
    let text = kind
        .strip_prefix(MIN_WITHDRAWAL)
        .ok_or("expected Solana minimum withdrawal amendment")?;
    match text.parse::<u64>() {
        Ok(atoms) if atoms > 0 && atoms.to_string() == text => Ok(atoms),
        _ => Err("invalid Solana minimum withdrawal atoms".into()),
    }
}

/// The latest governed minimum after the descriptor's registration, or the default.
pub fn min_withdrawal_atoms(g: &GovernanceState) -> u64 {
    let Some((descriptor, _)) = registered(g) else {
        return DEFAULT_MIN_WITHDRAWAL_ATOMS;
    };
    g.amendment_activation_records
        .iter()
        .rev()
        .filter(|a| {
            a.value == 1
                && a.activated_height > descriptor.authorized_height
                && a.activation_height == a.activated_height
        })
        .find_map(|a| minimum_withdrawal(&a.kind).ok())
        .unwrap_or(DEFAULT_MIN_WITHDRAWAL_ATOMS)
}

/// The packet hash commits to the burn, amount and source bucket. The burn ID
/// commits to the signed operation, including the full 32-byte destination.
/// Solana transaction signatures use their raw 64 bytes encoded as lowercase hex.
pub struct FinalizedPayout<'a> {
    pub packet_hash: &'a str,
    pub slot: u64,
    pub index: u16,
    pub signature_hex: &'a str,
}

pub fn payout_fields(kind: &str) -> Result<FinalizedPayout<'_>, String> {
    let mut fields = kind
        .strip_prefix(PAID)
        .ok_or("expected Solana payout amendment")?
        .split(':');
    let packet = fields.next().ok_or("missing withdrawal packet hash")?;
    let slot = fields.next().ok_or("missing finalized Solana slot")?;
    let ordinal = fields.next().ok_or("missing payout index")?;
    let index = ordinal.parse::<u16>().map_err(|_| "invalid payout index")?;
    let signature = fields
        .next()
        .ok_or("missing Solana transaction signature")?;
    validate_lower_hex_len("withdrawal packet hash", packet, 96)?;
    validate_lower_hex_len("Solana transaction signature", signature, 128)?;
    let number = slot
        .parse::<u64>()
        .map_err(|_| "invalid finalized Solana slot")?;
    if fields.next().is_some()
        || ordinal != index.to_string()
        || number == 0
        || number.to_string() != slot
        || signature.bytes().all(|b| b == b'0')
    {
        return Err("invalid finalized Solana payout reference".into());
    }
    Ok(FinalizedPayout {
        packet_hash: packet,
        slot: number,
        index,
        signature_hex: signature,
    })
}

pub fn retirement_height(g: &GovernanceState) -> Option<u64> {
    g.amendment_activation_records
        .iter()
        .find(|r| r.kind == RETIRE && r.value == 1 && r.activated_height == r.activation_height)
        .map(|r| r.activated_height)
}

pub fn open_height(g: &GovernanceState) -> Option<u64> {
    let (descriptor, _) = registered(g)?;
    g.amendment_activation_records
        .iter()
        .rev()
        .find(|a| {
            matches!(a.kind.as_str(), OPEN | CLOSE | RETIRE)
                && a.value == 1
                && a.activated_height > descriptor.authorized_height
                && a.activation_height == a.activated_height
        })
        .filter(|a| a.kind == OPEN)
        .map(|a| a.activated_height)
}
