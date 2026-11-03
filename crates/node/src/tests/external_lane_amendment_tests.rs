//! Governed changes to a genesis external lane (design sections 4.1 and 5.1)
//! are recognised and refused with a typed code until they are built.

use super::*;

const LANE_KINDS: [&str; 2] = [
    "external_lane_release_cap_v1:base-erc20",
    "external_lane_committee_epoch_v1:base-erc20",
];
const NOT_SUPPORTED: &str = "external_lane_amendment_not_supported";

fn lane_amendment(genesis: &Genesis, kind: &str, signed: bool) -> GovernanceAmendment {
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
        value: 100,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        support: validators,
        votes: Vec::new(),
        signed_authorizations,
    }
}

fn lane_amendment_node(label: &str) -> PathBuf {
    let data_dir = unique_test_dir(label);
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "external-lane-amendment".into(),
        node_id: "validator-0".into(),
        validator_count: 4,
    })
    .expect("init");
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
