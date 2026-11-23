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
    if height < record.authorized_height || height >= record.profile.expires_at_height {
        return Err("Solana exit descriptor is outside its authorized interval".into());
    }
    Ok((record, amendment))
}
