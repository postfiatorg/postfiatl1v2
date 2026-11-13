use crate::*;
use postfiat_types::{PfUsdcBeaconFinalityStateV1, VaultBridgeRouteProfileRecordV1};

// Real rejected block 1209 payload/receipt from OPS's public RPC archive.
// Public context is a read-only extract of validator-0: genesis, public registry,
// authority records and the unchanged epoch-10 PFUSDC NAV registry entries.
const BATCH: &str = include_str!(
    "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/block-1209-governance.json"
);
const RECEIPT: &str = include_str!(
    "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/block-1209-receipt.json"
);
const CONTEXT: &str = include_str!(
    "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/block-1209-public-context.json"
);

fn context() -> (Genesis, GovernanceState, ValidatorRegistry, LedgerState) {
    let raw: serde_json::Value = serde_json::from_str(CONTEXT).unwrap();
    let mut ledger = LedgerState::new(vec![]);
    ledger
        .nav_assets
        .push(serde_json::from_value(raw["nav_asset"].clone()).unwrap());
    ledger
        .nav_proof_profiles
        .push(serde_json::from_value(raw["nav_profile"].clone()).unwrap());
    assert!(raw["beacon_finality_states"].as_array().unwrap().is_empty());
    (
        serde_json::from_value(raw["genesis"].clone()).unwrap(),
        serde_json::from_value(raw["governance"].clone()).unwrap(),
        serde_json::from_value(raw["registry"].clone()).unwrap(),
        ledger,
    )
}

#[test]
fn b23_legacy_bootstrap_deserializes_and_preserves_base_batch_bytes_and_hashes() {
    let batch: GovernanceActionBatch = serde_json::from_str(BATCH).unwrap();
    assert_eq!(serde_json::to_string(&batch).unwrap(), BATCH);
    let activation = &batch.vault_bridge_route_profile_activations[0];
    let anchor = activation.beacon_finality_bootstrap.as_ref().unwrap();
    assert!(anchor.current_sync_committee_root.is_empty());
    let raw: serde_json::Value = serde_json::from_str(BATCH).unwrap();
    let old = &raw["vault_bridge_route_profile_activations"][0]["beacon_finality_bootstrap"];
    let decoded: PfUsdcBeaconFinalityStateV1 = serde_json::from_value(old.clone()).unwrap();
    assert_eq!(serde_json::to_value(&decoded).unwrap(), *old);
    assert!(anchor.validate().is_err());
    assert!(anchor.state_commitment_bytes().is_err());
    assert_eq!(
        postfiat_types::vault_bridge_beacon_route_amendment_kind_v2(&activation.profile, anchor)
            .unwrap(),
        activation.amendment.kind
    );
    let mut substituted = activation.clone();
    substituted
        .beacon_finality_bootstrap
        .as_mut()
        .unwrap()
        .current_sync_committee_root = "ab".repeat(32);
    assert!(
        substituted.validate_for_replay().is_err(),
        "legacy signatures cannot authorize an added committee"
    );
    let (genesis, governance, registry, _) = context();
    assert_eq!(crate::storage_commit::batch_archive_payload_hash(&genesis, "governance", &batch.batch_id, BATCH).unwrap(), "d9645d45c821a566c1cd38397d716e30e7cbdb553d05939943e6f839d3d329b62ebc4d31246a693e52d0cce465bd13a6");
    crate::consensus_artifacts::verify_archived_governance_action_batch_id(&genesis, &batch)
        .unwrap();
    crate::consensus_artifacts::verify_live_signed_governance_batch(
        &genesis,
        &governance,
        &registry,
        &batch,
        1209,
    )
    .unwrap();
    // Staging a new activation remains forbidden, even though archived identity
    // and all six existing ML-DSA authorizations remain verifiable.
    assert!(
        crate::consensus_artifacts::verify_governance_action_batch_id(&genesis, &batch).is_err()
    );
}

#[test]
fn b23_legacy_block_1209_replays_identical_rejection_receipt_bytes() {
    let batch: GovernanceActionBatch = serde_json::from_str(BATCH).unwrap();
    let expected: Receipt = serde_json::from_str(RECEIPT).unwrap();
    assert!(!expected.accepted);
    assert_eq!(expected.code, "vault_bridge_route_profile_rejected");
    for archived in [true, false] {
        let (genesis, mut governance, _, mut ledger) = context();
        let before_ledger = ledger.clone();
        let before_governance = governance.clone();
        let receipts = if archived {
            crate::execution_actions::execute_archived_governance_batch(
                &mut governance,
                Some(&mut ledger),
                Some(&genesis),
                &batch,
                1209,
            )
        } else {
            crate::execution_actions::execute_governance_batch(
                &mut governance,
                Some(&mut ledger),
                Some(&genesis),
                &batch,
                1209,
            )
        };
        assert_eq!(receipts.len(), 1);
        assert_eq!(
            serde_json::to_vec(&receipts[0]).unwrap(),
            serde_json::to_vec(&expected).unwrap()
        );
        assert_eq!(ledger, before_ledger);
        assert_eq!(governance, before_governance);
    }
}

#[test]
fn b23_legacy_anchor_cannot_activate_even_after_nav_profile_matches() {
    let batch: GovernanceActionBatch = serde_json::from_str(BATCH).unwrap();
    let activation = &batch.vault_bridge_route_profile_activations[0];
    assert!(activation.validate().is_err());
    assert!(VaultBridgeRouteProfileRecordV1::new_for_replay(activation, 1209).is_err());
    for archived in [true, false] {
        let (genesis, mut governance, _, mut ledger) = context();
        let hash = activation.profile.profile_hash().unwrap();
        ledger.nav_proof_profiles[0].vault_bridge_route_policy_hash = hash.clone();
        crate::execution_actions::validate_vault_bridge_route_profile_against_ledger(
            &ledger,
            &activation.profile,
            &hash,
        )
        .unwrap();
        let before = ledger.clone();
        let receipts = if archived {
            crate::execution_actions::execute_archived_governance_batch(
                &mut governance,
                Some(&mut ledger),
                Some(&genesis),
                &batch,
                1209,
            )
        } else {
            crate::execution_actions::execute_governance_batch(
                &mut governance,
                Some(&mut ledger),
                Some(&genesis),
                &batch,
                1209,
            )
        };
        assert!(!receipts[0].accepted);
        assert!(
            receipts[0].message.contains("current_sync_committee_root"),
            "{}",
            receipts[0].message
        );
        assert_eq!(ledger, before);
        assert!(ledger.beacon_finality_states.is_empty());
        assert!(governance.vault_bridge_route_profiles.is_empty());
    }
}

#[test]
#[ignore = "requires read-only live disk audit inputs via B23_LEGACY_DISK_INPUTS"]
fn b23_legacy_live_disk_copies_deserialize() {
    let path = std::env::var("B23_LEGACY_DISK_INPUTS").unwrap();
    let records: Vec<serde_json::Value> =
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert!(!records.is_empty());
    for record in &records {
        let value = record["value"].clone();
        let state = if value.get("batch_id").is_some() {
            let batch: GovernanceActionBatch = serde_json::from_value(value).unwrap();
            let activation = &batch.vault_bridge_route_profile_activations[0];
            let state = activation.beacon_finality_bootstrap.clone().unwrap();
            let historical_kind = postfiat_types::vault_bridge_beacon_route_amendment_kind_v2(
                &activation.profile,
                &state,
            )
            .unwrap();
            if activation.amendment.kind.contains(":beacon-bootstrap-v2:") {
                assert_eq!(historical_kind, activation.amendment.kind);
            } else {
                // Earlier abandoned staging attempts already lacked bound
                // bootstrap authorization. Deserialization must not bless them.
                assert!(activation.validate_for_replay().is_err());
                println!(
                    "pre-bootstrap-binding staging input remains unauthorized: {}",
                    record["path"]
                );
            }
            assert!(activation.validate().is_err());
            state
        } else {
            serde_json::from_value::<PfUsdcBeaconFinalityStateV1>(value).unwrap()
        };
        assert!(state.current_sync_committee_root.is_empty());
        assert!(state.validate().is_err());
    }
    println!(
        "{} public live-disk legacy inputs decode and remain inadmissible",
        records.len()
    );
}
