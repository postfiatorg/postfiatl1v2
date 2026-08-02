use postfiat_types::NavReservePublicValuesV1;
use sha3::{Digest, Sha3_384};

const PUBLIC_VALUES_HASH_DOMAIN_V1: &str = "postfiat.nav_reserve_public_values_hash.v1";
const SUBSCRIPTION_COMPOSITE_SOURCE_ROOT_DOMAIN_V1: &str =
    "postfiat.nav_reserve_subscription_composite_source_root.v1";

/// Derive the exact consensus source root and total assets for a reserve proof
/// plus a PFTL-accounted NAV-subscription overlay.
///
/// This host-side construction is shared by validator execution and the public
/// packet builder. It deliberately does not live in `postfiat-types`, which is
/// linked into the immutable SP1 guest and must not change merely because a
/// packet-construction helper changes.
pub fn nav_reserve_subscription_composite_source_root_v1(
    values: &NavReservePublicValuesV1,
    overlay_source_root: &str,
    overlay_value: u64,
) -> Result<(String, u64), String> {
    values.validate()?;
    validate_fixed_lower_hex("subscription_overlay_source_root", overlay_source_root, 48)?;
    if overlay_value == 0 {
        return Err("subscription overlay value must be nonzero".to_string());
    }
    let total = values
        .verified_net_assets
        .checked_add(overlay_value)
        .ok_or_else(|| "subscription overlay value overflows verified net assets".to_string())?;
    let encoded = values.encode()?;
    let public_values_hash = hash_hex_domain(PUBLIC_VALUES_HASH_DOMAIN_V1, &encoded);
    let preimage = format!(
        "asset_id={}\nprofile_id={}\nsource_manifest_hash={}\nproof_source_observation_root={}\npublic_values_hash={}\nproof_verified_net_assets={}\nsubscription_overlay_source_root={}\nsubscription_overlay_value={}\ntotal_verified_net_assets={}\n",
        values.nav_asset_id,
        values.proof_profile_id,
        values.source_manifest_hash,
        values.source_observation_root,
        public_values_hash,
        values.verified_net_assets,
        overlay_source_root,
        overlay_value,
        total,
    );
    Ok((
        hash_hex_domain(
            SUBSCRIPTION_COMPOSITE_SOURCE_ROOT_DOMAIN_V1,
            preimage.as_bytes(),
        ),
        total,
    ))
}

fn validate_fixed_lower_hex(field: &str, value: &str, bytes: usize) -> Result<(), String> {
    if value.len() != bytes.saturating_mul(2)
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(format!(
            "{field} must be exactly {bytes} bytes of lowercase hex"
        ));
    }
    Ok(())
}

fn hash_hex_domain(domain: &str, bytes: &[u8]) -> String {
    let mut hasher = Sha3_384::new();
    hasher.update(domain.as_bytes());
    hasher.update([0u8]);
    hasher.update(bytes);
    bytes_to_hex(&hasher.finalize())
}

fn bytes_to_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len().saturating_mul(2));
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_types::{NavReserveTrustCountsV1, NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1};

    fn fixture() -> NavReservePublicValuesV1 {
        NavReservePublicValuesV1 {
            schema: NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string(),
            pftl_genesis_hash: "01".repeat(48),
            nav_asset_id: "02".repeat(48),
            proof_profile_id: "03".repeat(48),
            valuation_policy_hash: "04".repeat(32),
            source_manifest_hash: "05".repeat(48),
            valuation_unit_id: "06".repeat(48),
            valuation_scale: 1_000_000,
            observation_epoch: 7,
            observation_not_before: 40,
            observation_not_after: 44,
            source_observation_root: "07".repeat(48),
            gross_assets: 1_200_000,
            total_liabilities: 200_000,
            verified_net_assets: 1_000_000,
            cryptographically_verified_value: 600_000,
            attested_value: 400_000,
            controlled_value: 0,
            source_count: 3,
            quantity_trust_counts: NavReserveTrustCountsV1 {
                cryptographic: 2,
                attested: 1,
                controlled: 0,
            },
            valuation_trust_counts: NavReserveTrustCountsV1 {
                cryptographic: 1,
                attested: 2,
                controlled: 0,
            },
            quantity_trust_root: "08".repeat(48),
            valuation_trust_root: "09".repeat(48),
            source_disclosure_root: "0a".repeat(48),
        }
    }

    #[test]
    fn subscription_composite_root_has_stable_vector_and_bounds() {
        let value = fixture();
        let overlay_root = "0b".repeat(48);
        let (root, total) =
            nav_reserve_subscription_composite_source_root_v1(&value, &overlay_root, 75)
                .expect("valid subscription overlay");
        assert_eq!(total, 1_000_075);
        assert_eq!(
            root,
            "de231528d015f5f4b7290b59837e343d13fc4b392dcd0beab8d87ba2959470c2173a413ae94b2c53dc2a16af21063ab4"
        );
        assert!(
            nav_reserve_subscription_composite_source_root_v1(&value, &overlay_root, 0,).is_err()
        );
        assert!(nav_reserve_subscription_composite_source_root_v1(&value, "0b", 75).is_err());

        let mut overflowing = value;
        overflowing.gross_assets = u64::MAX;
        overflowing.total_liabilities = 0;
        overflowing.verified_net_assets = u64::MAX;
        overflowing.cryptographically_verified_value = u64::MAX;
        overflowing.attested_value = 0;
        assert!(
            nav_reserve_subscription_composite_source_root_v1(&overflowing, &overlay_root, 1,)
                .is_err()
        );
    }
}
