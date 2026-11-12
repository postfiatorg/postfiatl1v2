use super::VaultBridgeRouteReport;
use postfiat_types::PfUsdcBeaconFinalityStateV1;
use std::io;

impl VaultBridgeRouteReport {
    pub(crate) fn select_beacon_anchor(
        states: &[PfUsdcBeaconFinalityStateV1],
        profile_hash: &str,
        route_epoch: u64,
    ) -> io::Result<Option<PfUsdcBeaconFinalityStateV1>> {
        let mut matches = states.iter().filter(|state| {
            state.route_profile_hash == profile_hash && state.route_epoch == route_epoch
        });
        let anchor = matches.next();
        if matches.next().is_some() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "duplicate beacon anchor key",
            ));
        }
        if let Some(anchor) = anchor {
            anchor
                .validate()
                .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
        }
        Ok(anchor.cloned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anchor() -> PfUsdcBeaconFinalityStateV1 {
        PfUsdcBeaconFinalityStateV1 {
            schema: postfiat_types::PFUSDC_BEACON_FINALITY_STATE_SCHEMA_V1.into(),
            route_profile_hash: "ab".repeat(48),
            route_epoch: 11,
            source_chain_id: 1,
            vault_address: "0xe7a432a28b20621a70e845c7f202004cf98e7a2c".into(),
            token_address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".into(),
            route_binding: "12".repeat(32),
            finalized_beacon_root: "34".repeat(32),
            finalized_slot: 100,
            current_sync_committee_root: "56".repeat(32),
        }
    }

    #[test]
    fn b23_rpc_anchor_matches_profile_and_epoch_and_fails_on_duplicate() {
        let state = anchor();
        let mut old = state.clone();
        old.route_epoch = 10;
        assert_eq!(
            VaultBridgeRouteReport::select_beacon_anchor(&[], &state.route_profile_hash, 11)
                .unwrap(),
            None
        );
        assert_eq!(
            VaultBridgeRouteReport::select_beacon_anchor(
                &[old.clone()],
                &state.route_profile_hash,
                11
            )
            .unwrap(),
            None
        );
        assert_eq!(
            VaultBridgeRouteReport::select_beacon_anchor(&[state.clone()], &"ff".repeat(48), 11)
                .unwrap(),
            None
        );
        for states in [vec![old.clone(), state.clone()], vec![state.clone(), old]] {
            assert_eq!(
                VaultBridgeRouteReport::select_beacon_anchor(
                    &states,
                    &state.route_profile_hash,
                    11
                )
                .unwrap(),
                Some(state.clone())
            );
        }
        assert!(VaultBridgeRouteReport::select_beacon_anchor(
            &[state.clone(), state.clone()],
            &state.route_profile_hash,
            11
        )
        .is_err());
    }

    #[test]
    fn b23_rpc_absent_anchor_serializes_as_explicit_null() {
        let profile: serde_json::Value = serde_json::from_str(include_str!(
            "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/mainnet-route.json"
        ))
        .unwrap();
        let report: VaultBridgeRouteReport = serde_json::from_value(serde_json::json!({
            "schema": "postfiat.vault_bridge.route_report.v1", "chain_id": "test", "genesis_hash": "",
            "current_height": 1, "profile": profile, "profile_hash": "ab".repeat(48), "route_binding": "",
            "governance_amendment_id": "", "governance_activation_height": 0, "governance_route_epoch": 11,
            "nav_profile_id": "", "nav_profile_source_class": "", "nav_profile_verifier_kind": "sp1-groth16",
            "nav_profile_policy_hash": "", "route_trust_class": "TRUSTLESS_FINALITY", "live_value_enabled": false,
            "active": true
        })).unwrap();
        let encoded = serde_json::to_value(report).unwrap();
        assert!(encoded
            .as_object()
            .unwrap()
            .contains_key("beacon_finality_anchor"));
        assert!(encoded["beacon_finality_anchor"].is_null());
    }
}
