use super::*;
use postfiat_types::{PfUsdcBeaconFinalityStateV1, PfUsdcEthereumIngressPublicValuesV1,
    PFUSDC_BEACON_FINALITY_STATE_SCHEMA_V1};

fn honest_witness() -> EthIngressWitnessV1 {
    let mut raw: serde_json::Value = serde_json::from_str(include_str!(
        "../tests/fixtures/mainnet-deposit.json")).unwrap();
    let inputs: ProofInputs = serde_json::from_value(raw["helios"].clone()).unwrap();
    let profile: postfiat_types::VaultBridgeRouteProfileV1 = serde_json::from_str(
        include_str!("../tests/fixtures/mainnet-route.json")).unwrap();
    raw["beacon_anchor"] = serde_json::json!({
        "schema": PFUSDC_BEACON_FINALITY_STATE_SCHEMA_V1,
        "route_profile_hash": profile.profile_hash().unwrap(), "route_epoch": profile.route_epoch,
        "source_chain_id": 1, "vault_address": raw["evidence"]["vault_address"],
        "token_address": raw["evidence"]["token_address"],
        "route_binding": raw["evidence"]["route_binding"],
        "finalized_beacon_root": hex32(inputs.store.finalized_header.beacon().tree_hash_root()),
        "finalized_slot": inputs.store.finalized_header.beacon().slot,
        "current_sync_committee_root": hex32(inputs.store.current_sync_committee.tree_hash_root()),
    });
    serde_json::from_value(raw).unwrap()
}

#[test]
fn honest_header_with_swapped_committee_is_rejected() {
    let mut witness = honest_witness();
    let honest_header = witness.beacon_anchor.finalized_beacon_root.clone();
    // Use a different, well-formed committee already present in the public
    // fixture. The attack keeps the honest header and replaces its committee.
    let replacement = witness.helios.updates.iter()
        .map(|update| update.next_sync_committee()).next().unwrap().clone();
    assert_ne!(replacement.tree_hash_root(), witness.helios.store.current_sync_committee.tree_hash_root());
    witness.helios.store.current_sync_committee = replacement;
    assert_eq!(hex32(witness.helios.store.finalized_header.beacon().tree_hash_root()), honest_header);
    let error = verify_witness(&witness).unwrap_err();
    assert!(error.contains("sync committee does not match pinned beacon anchor"), "{error}");
}

#[test]
fn honest_proof_advances_header_and_authenticated_committee() {
    let witness = honest_witness();
    // Execute the real guest verifier, including BLS and MPT checks, on an
    // archived mainnet deposit. This is native verification, not SP1 proving.
    let values = verify_witness(&witness).unwrap();
    let bytes = serde_cbor::to_vec(&values).unwrap();
    let decoded = PfUsdcEthereumIngressPublicValuesV1::from_canonical_bytes(&bytes).unwrap();
    let anchor = witness.beacon_anchor;
    let advanced = PfUsdcBeaconFinalityStateV1::advance_for_deposit(
        &[anchor.clone()], &anchor.route_profile_hash, &decoded).unwrap();
    assert!(advanced.finalized_slot > anchor.finalized_slot);
    assert_eq!(advanced.current_sync_committee_root, decoded.current_sync_committee_root);
    assert_eq!(advanced.finalized_beacon_root, decoded.finalized_beacon_root);
    assert_ne!(anchor.state_commitment_bytes().unwrap(), advanced.state_commitment_bytes().unwrap());
    let mut substituted = anchor.clone();
    substituted.current_sync_committee_root = "ff".repeat(32);
    assert_ne!(anchor.state_commitment_bytes().unwrap(), substituted.state_commitment_bytes().unwrap());
    assert!(PfUsdcBeaconFinalityStateV1::advance_for_deposit(
        &[substituted], &anchor.route_profile_hash, &decoded).is_err());
}

#[test]
fn no_anchor_rejects_an_honest_proof() {
    let witness = honest_witness();
    let values = verify_witness(&witness).unwrap();
    let decoded = PfUsdcEthereumIngressPublicValuesV1::from_canonical_bytes(
        &serde_cbor::to_vec(&values).unwrap()).unwrap();
    let error = PfUsdcBeaconFinalityStateV1::advance_for_deposit(
        &[], &witness.beacon_anchor.route_profile_hash, &decoded).unwrap_err();
    assert_eq!(error.0, "pfusdc_beacon_finality_state_missing");
}

#[test]
fn witness_bookkeeping_cannot_supply_a_committee_or_finality_shortcut() {
    let mut witness = honest_witness();
    let baseline = verify_witness(&witness).unwrap();
    witness.helios.store.current_max_active_participants = u64::MAX;
    witness.helios.store.previous_max_active_participants = u64::MAX;
    witness.helios.store.next_sync_committee = Some(Default::default());
    assert_eq!(verify_witness(&witness).unwrap(), baseline);
}

#[test]
fn wrong_epoch_anchor_cannot_admit_a_deposit_and_order_does_not_matter() {
    let witness = honest_witness();
    let values = verify_witness(&witness).unwrap();
    let decoded = PfUsdcEthereumIngressPublicValuesV1::from_canonical_bytes(
        &serde_cbor::to_vec(&values).unwrap()).unwrap();
    let anchor = witness.beacon_anchor;
    let mut wrong_epoch = anchor.clone();
    wrong_epoch.route_epoch += 1;
    let error = PfUsdcBeaconFinalityStateV1::advance_for_deposit(
        &[wrong_epoch.clone()], &anchor.route_profile_hash, &decoded).unwrap_err();
    assert_eq!(error.0, "pfusdc_beacon_finality_state_missing");
    let first = PfUsdcBeaconFinalityStateV1::advance_for_deposit(
        &[wrong_epoch.clone(), anchor.clone()], &anchor.route_profile_hash, &decoded).unwrap();
    let last = PfUsdcBeaconFinalityStateV1::advance_for_deposit(
        &[anchor.clone(), wrong_epoch.clone()], &anchor.route_profile_hash, &decoded).unwrap();
    assert_eq!(first, last);
    let mut wrong_values = decoded.clone();
    wrong_values.route_epoch = wrong_epoch.route_epoch;
    let error = PfUsdcBeaconFinalityStateV1::advance_for_deposit(
        &[wrong_epoch], &anchor.route_profile_hash, &wrong_values).unwrap_err();
    assert_eq!(error.0, "pfusdc_beacon_route_epoch_invalid");
    let error = PfUsdcBeaconFinalityStateV1::advance_for_deposit(
        &[anchor.clone(), anchor.clone()], &anchor.route_profile_hash, &decoded).unwrap_err();
    assert_eq!(error.0, "pfusdc_beacon_finality_state_ambiguous");
}
