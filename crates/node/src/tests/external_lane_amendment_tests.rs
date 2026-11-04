//! Governed changes to a genesis external lane (design sections 4.1 and 5.1):
//! the release cap amendment is accepted with its value range checked, and
//! the committee epoch move is refused with a typed code until it is built.

use super::*;

const CAP_KIND: &str = "external_lane_release_cap_v1:base-erc20";
const LANE_KINDS: [&str; 1] = ["external_lane_committee_epoch_v1:base-erc20"];
const NOT_SUPPORTED: &str = "external_lane_amendment_not_supported";
const INVALID_CAP: &str = "invalid_external_lane_release_cap";

fn lane_amendment(genesis: &Genesis, kind: &str, signed: bool) -> GovernanceAmendment {
    lane_amendment_with_value(genesis, kind, 100, signed)
}

fn lane_amendment_with_value(
    genesis: &Genesis,
    kind: &str,
    value: u32,
    signed: bool,
) -> GovernanceAmendment {
    let validators = local_validator_ids(4).expect("validators");
    let signed_authorizations = if signed {
        validators
            .iter()
            .map(|validator| SignedGovernanceAuthorizationV2 {
                schema: SIGNED_GOVERNANCE_AUTHORIZATION_SCHEMA_V2.to_string(),
                validator: validator.clone(),
                vote_id: format!("{validator}-vote"),
                old_registry_root: "00".repeat(48),
                committee_epoch: 0,
                proposal_slot: 1,
                expires_at_height: 20,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                signature_hex: "00".repeat(32),
            })
            .collect()
    } else {
        Vec::new()
    };
    GovernanceAmendment {
        amendment_id: format!("lane-amendment-{kind}"),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        instance_id: "lane-amendment-instance".to_string(),
        proposal_id: "lane-amendment-proposal".to_string(),
        certificate_id: "lane-amendment-certificate".to_string(),
        proposer: validators[0].clone(),
        validators: validators.clone(),
        quorum: 3,
        kind: kind.to_string(),
        value,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        support: validators,
        votes: Vec::new(),
        signed_authorizations,
    }
}

/// A four-validator node whose genesis configures the lane `base-erc20`.
fn lane_amendment_node(label: &str) -> PathBuf {
    let data_dir = unique_test_dir(label);
    let genesis = super::genesis_external_lane_tests::lane_genesis("external-lane-amendment", 4);
    init_with_genesis(data_dir.clone(), "validator-0".to_string(), genesis).expect("init");
    data_dir
}

#[test]
fn external_lane_amendments_are_refused_at_ratification_with_a_typed_code() {
    let data_dir = lane_amendment_node("external-lane-amendment-ratify");
    let store = NodeStore::new(&data_dir);
    let before = store.read_governance().expect("governance");
    let validators = local_validator_ids(4).expect("validators");
    for kind in LANE_KINDS {
        let amendment_file = data_dir.join("lane-amendment.json");
        let error = ratify_governance(RatifyGovernanceOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            kind: kind.to_string(),
            value: 100,
            activation_height: 0,
            veto_until_height: 0,
            paused: false,
            amendment_file: amendment_file.clone(),
        })
        .expect_err("external lane amendment ratified");
        assert!(error.to_string().starts_with(NOT_SUPPORTED), "{kind}: {error}");
        assert!(!amendment_file.exists(), "{kind}: amendment file written");
    }
    assert_eq!(store.read_governance().expect("governance"), before);
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn signed_external_lane_amendments_are_refused_before_their_signatures_are_read() {
    let data_dir = lane_amendment_node("external-lane-amendment-signed");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let governance = store.read_governance().expect("governance");
    let registry =
        read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE)).expect("registry");
    for kind in LANE_KINDS {
        for signed in [false, true] {
            let amendment = lane_amendment(&genesis, kind, signed);
            let batch = GovernanceActionBatch::new("lane-amendment", vec![amendment.clone()]);
            let error =
                verify_live_signed_governance_batch(&genesis, &governance, &registry, &batch, 1)
                    .expect_err("external lane amendment admitted");
            assert!(error.to_string().starts_with(NOT_SUPPORTED), "{kind}: {error}");

            let (code, message) = governance_amendment_lifecycle_rejection(&amendment, 10)
                .expect("external lane amendment applied");
            assert_eq!(code, NOT_SUPPORTED, "{kind}: {message}");
        }
    }
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn only_the_lane_amendment_families_are_refused_as_not_supported() {
    let genesis = Genesis::new_with_validator_count("external-lane-amendment", 4);
    for kind in [
        "external_lane_release_cap_v1x:base-erc20",
        CAP_KIND,
        GOVERNANCE_KIND_RECIPIENT_FORMAT_ACTIVATION_HEIGHT,
    ] {
        let rejection =
            governance_amendment_lifecycle_rejection(&lane_amendment(&genesis, kind, true), 10);
        assert!(
            rejection.as_ref().is_none_or(|(code, _)| *code != NOT_SUPPORTED),
            "{kind}: {rejection:?}"
        );
    }
}

fn ratify_cap(data_dir: &Path, kind: &str, value: u32) -> io::Result<GovernanceAmendment> {
    let amendment_file = data_dir.join(format!("cap-{value}.json"));
    ratify_governance(RatifyGovernanceOptions {
        data_dir: data_dir.to_path_buf(),
        validators: local_validator_ids(4).expect("validators"),
        support: local_validator_ids(4).expect("validators"),
        kind: kind.to_string(),
        value,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        amendment_file,
    })
}

#[test]
fn release_cap_amendment_ratifies_within_its_value_range() {
    let data_dir = lane_amendment_node("external-lane-cap-ratify");
    for value in [0, 200, 10_000] {
        let amendment = ratify_cap(&data_dir, CAP_KIND, value).expect("cap amendment ratified");
        assert_eq!((amendment.kind.as_str(), amendment.value), (CAP_KIND, value));
    }
    for (kind, value) in [
        (CAP_KIND, 10_001),
        (CAP_KIND, u32::MAX),
        ("external_lane_release_cap_v1", 100),
        ("external_lane_release_cap_v1:", 100),
        ("external_lane_release_cap_v1:Base-ERC20", 100),
        ("external_lane_release_cap_v1:base-erc20:extra", 100),
    ] {
        let error = ratify_cap(&data_dir, kind, value).expect_err("invalid cap ratified");
        assert!(error.to_string().contains(INVALID_CAP), "{kind} {value}: {error}");
    }
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn release_cap_amendment_passes_admission_and_execution_checks_within_range() {
    let data_dir = lane_amendment_node("external-lane-cap-signed");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let governance = store.read_governance().expect("governance");
    let registry =
        read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE)).expect("registry");
    for value in [0, 10_000] {
        let amendment = lane_amendment_with_value(&genesis, CAP_KIND, value, false);
        assert_eq!(governance_amendment_lifecycle_rejection(&amendment, 10), None);
        let batch = GovernanceActionBatch::new("lane-cap", vec![amendment]);
        if let Err(error) =
            verify_live_signed_governance_batch(&genesis, &governance, &registry, &batch, 1)
        {
            let error = error.to_string();
            assert!(
                !error.contains(INVALID_CAP) && !error.contains(NOT_SUPPORTED),
                "{value}: {error}"
            );
        }
    }
    for signed in [false, true] {
        let amendment = lane_amendment_with_value(&genesis, CAP_KIND, 10_001, signed);
        let batch = GovernanceActionBatch::new("lane-cap", vec![amendment.clone()]);
        let error = verify_live_signed_governance_batch(&genesis, &governance, &registry, &batch, 1)
            .expect_err("out-of-range cap admitted");
        assert!(error.to_string().contains(INVALID_CAP), "{error}");
        let (code, _) = governance_amendment_lifecycle_rejection(&amendment, 10)
            .expect("out-of-range cap applied");
        assert_eq!(code, INVALID_CAP);
    }
    let _ = fs::remove_dir_all(data_dir);
}

const LANE_NOT_CONFIGURED: &str = "external_lane_release_cap_lane_not_configured";

#[test]
fn release_cap_amendment_for_a_lane_genesis_does_not_configure_is_refused() {
    let data_dir = lane_amendment_node("external-lane-cap-unknown-lane");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let mut governance = store.read_governance().expect("governance");
    let mut ledger = store.read_ledger().expect("ledger");
    let registry =
        read_validator_registry_file(&data_dir.join(VALIDATOR_REGISTRY_FILE)).expect("registry");
    let kind = "external_lane_release_cap_v1:not-a-lane";

    let error = ratify_cap(&data_dir, kind, 0).expect_err("unknown lane ratified");
    assert!(error.to_string().starts_with(LANE_NOT_CONFIGURED), "{error}");

    let amendment = lane_amendment_with_value(&genesis, kind, 0, false);
    let batch = GovernanceActionBatch::new("lane-cap-unknown", vec![amendment]);
    let error = verify_live_signed_governance_batch(&genesis, &governance, &registry, &batch, 1)
        .expect_err("unknown lane admitted");
    assert!(error.to_string().contains(LANE_NOT_CONFIGURED), "{error}");

    let receipts = crate::execution_actions::execute_governance_batch(
        &mut governance,
        Some(&mut ledger),
        &batch,
        1,
    );
    assert!(!receipts[0].accepted, "{receipts:?}");
    assert_eq!(receipts[0].code, LANE_NOT_CONFIGURED, "{receipts:?}");
    assert!(governance.external_lane_release_cap_amendments("not-a-lane").is_empty());
    let _ = fs::remove_dir_all(data_dir);
}
