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
            VaultBridgeRouteReport::select_beacon_anchor(std::slice::from_ref(&state), &"ff".repeat(48), 11)
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
    #[test]
    fn b23_rpc_real_report_reaches_python_live_beacon_anchor() {
        use std::io::Write;
        use std::process::{Command, Stdio};
        let mut profile: postfiat_types::VaultBridgeRouteProfileV1 =
            serde_json::from_str(include_str!(
                "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/mainnet-route.json"
            ))
            .unwrap();
        profile.vault_address = anchor().vault_address;
        profile.route_epoch = 11;
        profile.verifier_policy_hash =
            "44a402a3df72ee8da6f23737becef04fd0ce55e683706e929606a6aea1f05b25".into();
        profile.verifier_program_vkey =
            "0x00d8cbf06d5640adde0fef863b3c689c4ab3442b0dc91d28ac4cc5f9c9b2ccbb".into();
        let profile_hash = profile.profile_hash().unwrap();
        let binding =
            postfiat_types::vault_bridge_route_binding(&profile_hash, profile.route_epoch).unwrap();
        let mut state = anchor();
        state.route_profile_hash = profile_hash.clone();
        state.route_binding = binding.clone();
        let credited = postfiat_types::pfusdc_source_series_id(
            "postfiat-wan-devnet-2",
            &profile.asset_id,
            1,
            &profile.vault_address,
            &profile.token_address,
            profile.route_epoch,
            &profile_hash,
        )
        .unwrap();
        let report = VaultBridgeRouteReport {
            schema: "postfiat.vault_bridge.route_report.v1".into(),
            chain_id: "postfiat-wan-devnet-2".into(),
            genesis_hash: String::new(),
            current_height: 400,
            profile,
            profile_hash: profile_hash.clone(),
            route_binding: binding,
            governance_amendment_id: String::new(),
            governance_activation_height: 318,
            governance_route_epoch: 11,
            nav: super::super::VaultBridgeNavReport::Route {
                nav_profile_id: String::new(),
                nav_profile_source_class: String::new(),
                nav_profile_verifier_kind: "sp1-groth16".into(),
                nav_profile_policy_hash: profile_hash.clone(),
            },
            route_trust_class: "TRUSTLESS_FINALITY".into(),
            live_value_enabled: true,
            active: true,
            burns_open: None,
            beacon_finality_anchor: VaultBridgeRouteReport::select_beacon_anchor(
                &[state],
                &profile_hash,
                11,
            )
            .unwrap(),
        };
        let mut child = Command::new("python3")
            .arg(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../scripts/a666-wallet-eth-bridge-stage-tests.py"
            ))
            .arg("--report-stdin")
            .arg(credited)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(&serde_json::to_vec(&report).unwrap())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        println!("{}", String::from_utf8_lossy(&output.stdout));
    }
}

#[cfg(test)]
#[path = "b23_ingress_legacy_tests.rs"]
mod legacy_tests;

#[cfg(test)]
#[path = "b23_ingress_release4_tests.rs"]
mod release4_tests;
