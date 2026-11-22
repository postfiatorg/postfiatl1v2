use anyhow::{ensure, Result};
use postfiat_execution::{
    derive_nav_reserve_subscription_composite_source_root_v1, NavSubscriptionReserveOverlay,
};
use postfiat_types::NavReservePublicValuesV1;

pub fn compose(
    values: &NavReservePublicValuesV1,
    overlay: Option<&NavSubscriptionReserveOverlay>,
    supply: u64,
) -> Result<serde_json::Value> {
    values.validate().map_err(anyhow::Error::msg)?;
    ensure!(supply > 0, "zero circulating supply");
    let (root, total) = match overlay {
        Some(o) => derive_nav_reserve_subscription_composite_source_root_v1(
            values,
            &o.source_root,
            o.value_nav_units,
        )
        .map_err(anyhow::Error::msg)?,
        None => (
            values.source_observation_root.clone(),
            values.verified_net_assets,
        ),
    };
    let price =
        u64::try_from(u128::from(total) * u128::from(values.valuation_scale) / u128::from(supply))?;
    Ok(
        serde_json::json!({"source_root":root,"verified_net_assets":total,
        "proof_verified_net_assets":values.verified_net_assets,"attested_value":values.attested_value,
        "overlay_value":overlay.map_or(0,|o|o.value_nav_units),"overlay_source_root":overlay.map(|o|&o.source_root),
        "supply_atoms":supply,"nav_per_unit":price}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use postfiat_types::{NavReserveTrustCountsV1, NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1};
    fn values() -> NavReservePublicValuesV1 {
        let counts = NavReserveTrustCountsV1 {
            cryptographic: 0,
            attested: 1,
            controlled: 0,
        };
        NavReservePublicValuesV1 {
            schema: NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.into(),
            pftl_genesis_hash: "01".repeat(48),
            nav_asset_id: "02".repeat(48),
            proof_profile_id: "03".repeat(48),
            valuation_policy_hash: "04".repeat(32),
            source_manifest_hash: "05".repeat(48),
            valuation_unit_id: "06".repeat(48),
            valuation_scale: 1_000_000,
            observation_epoch: 3,
            observation_not_before: 40,
            observation_not_after: 40,
            source_observation_root: "07".repeat(48),
            gross_assets: 420244150000,
            total_liabilities: 0,
            verified_net_assets: 420244150000,
            cryptographically_verified_value: 0,
            attested_value: 420244150000,
            controlled_value: 0,
            source_count: 1,
            quantity_trust_counts: counts,
            valuation_trust_counts: counts,
            quantity_trust_root: "08".repeat(48),
            valuation_trust_root: "09".repeat(48),
            source_disclosure_root: "0a".repeat(48),
        }
    }
    #[test]
    fn zero_overlay_keeps_original_proof_amount_and_source_root() {
        let v = values();
        let before = v.encode().unwrap();
        let p = compose(&v, None, 419230350000).unwrap();
        assert_eq!(p["verified_net_assets"], v.verified_net_assets);
        assert_eq!(p["source_root"], v.source_observation_root);
        assert_eq!(p["nav_per_unit"], 1002418);
        assert_eq!(before, v.encode().unwrap());
    }
    #[test]
    fn nonzero_overlay_uses_consensus_composite_without_changing_attestation() {
        let v = values();
        let before = v.encode().unwrap();
        let o = NavSubscriptionReserveOverlay {
            value_nav_units: 37000000,
            source_root: "0b".repeat(48),
        };
        let p = compose(&v, Some(&o), 419230350000).unwrap();
        let (root, total) = derive_nav_reserve_subscription_composite_source_root_v1(
            &v,
            &o.source_root,
            o.value_nav_units,
        )
        .unwrap();
        assert_eq!(total, 420281150000);
        assert_eq!(p["verified_net_assets"], total);
        assert_eq!(p["source_root"], root);
        assert_ne!(p["source_root"], v.source_observation_root);
        assert_eq!(p["attested_value"], 420244150000_u64);
        assert_eq!(before, v.encode().unwrap());
        let changed = NavSubscriptionReserveOverlay {
            value_nav_units: 38000000,
            source_root: o.source_root,
        };
        assert_ne!(
            p["source_root"],
            compose(&v, Some(&changed), 419230350000).unwrap()["source_root"]
        );
    }
}
