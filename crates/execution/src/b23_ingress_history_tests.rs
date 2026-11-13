use super::*;
use postfiat_types::NAV_SP1_PROOF_ENCODING_GROTH16;

// History reading for Ethereum ingress deposits made before the anchored
// beacon route: block 319 of postfiat-wan-devnet-2 carries an accepted deposit
// whose proof commits the 26-field public values.

const H319_FIXTURE: &str = include_str!("../tests/fixtures/ethereum_ingress_h319_legacy_deposit.json");
/// Verifying key of the pre-anchor Ethereum-finality guest that proved block 319.
const H319_PROGRAM_VKEY: &str = "0x00a9f8f037da18dd1aa5a7b0f478df0c7c9fae411ee62b339baf48dc2505076e";

struct H319Deposit {
    evidence: VaultBridgeDepositEvidence,
    evidence_root: String,
    policy_hash: String,
    source_proof_hash: String,
    source_public_values_hash: String,
    proof: Vec<u8>,
    public_values: Vec<u8>,
}

fn decode_hex(text: &str) -> Result<Vec<u8>, std::num::ParseIntError> {
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&text[index..index + 2], 16))
        .collect()
}

fn h319_deposit() -> H319Deposit {
    let fixture: serde_json::Value = serde_json::from_str(H319_FIXTURE).expect("fixture");
    let text = |key: &str| fixture[key].as_str().expect(key).to_string();
    H319Deposit {
        evidence: serde_json::from_value(fixture["evidence"].clone()).expect("evidence"),
        evidence_root: text("evidence_root"),
        policy_hash: text("policy_hash"),
        source_proof_hash: text("source_proof_hash"),
        source_public_values_hash: text("source_public_values_hash"),
        proof: decode_hex(&text("source_proof_hex")).expect("proof hex"),
        public_values: decode_hex(&text("source_public_values_hex")).expect("values hex"),
    }
}

fn h319_profile(deposit: &H319Deposit, manifest_hash: &str) -> NavProofProfile {
    NavProofProfile::new(
        "operator",
        NAV_PROFILE_VERIFIER_SP1_GROTH16,
        format!("vault_bridge:{}", deposit.evidence.source_domain()),
        7_200,
        1,
        7_200,
        7_200,
        0,
        0,
        0,
        manifest_hash,
        H319_PROGRAM_VKEY,
        NAV_SP1_PROOF_ENCODING_GROTH16,
        DEFAULT_MAX_NAV_SP1_PROOF_BYTES,
        DEFAULT_MAX_NAV_SP1_PUBLIC_VALUES_BYTES,
    )
    .expect("SP1 profile")
    .with_vault_bridge_route_policy_hash(deposit.policy_hash.clone())
    .expect("route-bound profile")
}

fn h319_admit(
    deposit: &H319Deposit,
    profile: &NavProofProfile,
) -> Result<Option<VerifiedIngressPublicValues>, (&'static str, String)> {
    let genesis = Genesis::new("postfiat-wan-devnet-2");
    ensure_vault_bridge_deposit_source_proof(VaultBridgeDepositSourceProof {
        genesis: Some(&genesis),
        profile,
        evidence: &deposit.evidence,
        evidence_root: &deposit.evidence_root,
        policy_hash: &deposit.policy_hash,
        source_proof_kind: SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1,
        source_proof_hash: &deposit.source_proof_hash,
        source_public_values_hash: &deposit.source_public_values_hash,
        source_proof_bytes: &deposit.proof,
        source_public_values: &deposit.public_values,
        fast_ingress_verifier: None,
    })
}

fn h319_anchor(policy_hash: &str) -> postfiat_types::PfUsdcBeaconFinalityStateV1 {
    postfiat_types::PfUsdcBeaconFinalityStateV1 {
        schema: postfiat_types::PFUSDC_BEACON_FINALITY_STATE_SCHEMA_V1.to_string(),
        route_profile_hash: policy_hash.to_string(),
        route_epoch: 11,
        source_chain_id: 1,
        vault_address: "0x8583409ddbac984ec195dfa06a21103d92403c1e".to_string(),
        token_address: "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string(),
        route_binding: "11".repeat(32),
        finalized_beacon_root: "22".repeat(32),
        finalized_slot: 1,
        current_sync_committee_root: "33".repeat(32),
    }
}

#[test]
fn h319_legacy_deposit_is_admitted_on_its_unanchored_route_as_main_did() {
    let deposit = h319_deposit();
    // The anchored layout cannot read it: this is the replay failure at 319.
    let current = postfiat_types::PfUsdcEthereumIngressPublicValuesV1::from_canonical_bytes(
        &deposit.public_values,
    )
    .expect_err("26-field values do not decode as the anchored layout");
    assert!(current.contains("missing field `route_epoch`"), "{current}");
    let legacy = postfiat_types::PfUsdcEthereumIngressPublicValuesLegacyV1::from_canonical_bytes(
        &deposit.public_values,
    )
    .expect("main's 26-field decoder reads block 319");
    let profile = h319_profile(&deposit, &legacy.manifest_hash);
    let verified = h319_admit(&deposit, &profile).expect("block 319 deposit is admitted");
    let Some(VerifiedIngressPublicValues::EthereumUnanchored(values)) = verified else {
        panic!("expected the unanchored legacy path, got {verified:?}");
    };
    assert_eq!(values.deposit_nullifier, legacy.deposit_nullifier);
    assert_eq!(values.deposit_id, deposit.evidence.deposit_id);
    // No anchor existed for this route at 319, nor for any other route profile.
    ensure_ethereum_ingress_route_unanchored(&[], &deposit.policy_hash).expect("no anchors");
    ensure_ethereum_ingress_route_unanchored(&[h319_anchor(&"ab".repeat(48))], &deposit.policy_hash)
        .expect("an anchor on another route profile does not gate this one");
}

#[test]
fn legacy_layout_deposit_on_an_anchored_route_is_refused() {
    let deposit = h319_deposit();
    let refused = ensure_ethereum_ingress_route_unanchored(
        &[h319_anchor(&deposit.policy_hash)],
        &deposit.policy_hash,
    )
    .expect_err("anchored routes require the 29-field values");
    assert_eq!(refused.0, "ethereum_ingress_legacy_public_values_on_anchored_route");
}
