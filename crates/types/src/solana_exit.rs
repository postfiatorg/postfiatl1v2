//! The frozen mainnet checker binds a single immutable descriptor and source bucket.
//! Governance signs the descriptor through a route-profile activation. The registered exit record
//! is excluded from ingress profile selection.
use super::*;

pub const BUCKET: &str = "4c045e289eaaa2c7ae7592b96ec0eeabe9fa0cfdd032783d359409dcc08d9dd0dfdd7cc8d483d6a88c12dc713522d2cc";
pub const POLICY: &str = "5bb5a3d99616945cb7330c07a51c47c0453ff54f6c622f44f18c5b505cc9b13f179a6796a537119a482d788537bd96c7";
pub const PROFILE_HASH: &str = "37e3c05d8e353d93d56819bfdc3b10f314e5ae298cf43a34cefac477d06893953ceca391165ec3bc977c5e5b273b4046";
pub const FAMILY: &str = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b";

const RELEASES: &[(&str, &str, Option<u64>)] = &[(
    "postfiat-wan-devnet-2",
    "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9",
    None, // Release sets the agreed future height here.
)];

pub fn release_height(chain: &str, genesis: &str) -> Option<u64> {
    RELEASES
        .iter()
        .find(|(c, g, _)| *c == chain && *g == genesis)?
        .2
}

pub fn descriptor() -> VaultBridgeRouteProfileV1 {
    serde_json::from_str(include_str!("solana_exit_profile.json"))
        .expect("checked release descriptor")
}

pub fn matches_bucket(bucket: &VaultBridgeBucketState) -> bool {
    bucket.bucket_id == BUCKET
        && bucket.policy_hash == POLICY
        && bucket.asset_id == FAMILY
        && vault_bridge_bucket_id(FAMILY, &bucket.source_domain, POLICY).as_deref() == Ok(BUCKET)
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

pub fn is_decision(kind: &str) -> bool {
    matches!(kind, OPEN | CLOSE | RETIRE) || kind.starts_with(PAID)
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
