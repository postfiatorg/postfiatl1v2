fn sample_external_lane(id: &str, reserved_atoms: u64) -> GenesisExternalLane {
    GenesisExternalLane {
        id: id.to_string(),
        reserved_atoms,
        release_window_blocks: 86_400,
        release_cap_bps: 200,
        policy: GenesisExternalLanePolicy {
            chain_id: 8453,
            lock_contract_address: format!("0x{}", "1a".repeat(20)),
            token_address: format!("0x{}", "2b".repeat(20)),
            token_runtime_code_hash: format!("0x{}", "3c".repeat(32)),
            lock_runtime_code_hash: format!("0x{}", "4d".repeat(32)),
            committee_epoch: 2,
            committee_root: format!("0x{}", "5e".repeat(32)),
            confirmation_depth: 64,
        },
    }
}

fn genesis_with_lanes(lanes: Vec<GenesisExternalLane>) -> Genesis {
    let mut genesis = Genesis::new("postfiat-local");
    genesis.external_lanes = Some(lanes);
    genesis
}

fn genesis_json_hash(genesis: &Genesis) -> String {
    let json = genesis.to_json().expect("serialize genesis");
    let mut hasher = Sha3_384::new();
    hasher.update(b"postfiat.genesis.v1");
    hasher.update([0_u8]);
    hasher.update(json.as_bytes());
    hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn genesis_without_lanes_hashes_as_before() {
    let genesis = Genesis::new("postfiat-local");
    assert_eq!(genesis.external_lanes, None);
    let json = genesis.to_json().expect("serialize genesis");
    assert!(!json.contains("external_lanes"));
    assert_eq!(
        json,
        "{\n  \"chain_id\": \"postfiat-local\",\n  \"protocol_version\": 1,\n  \"validator_count\": 1,\n  \"native_supply_atoms\": 1000000000,\n  \"replicated_state_v2_activation_height\": 0\n}\n"
    );
    // The canonical vector of `genesis_hash_has_stable_canonical_test_vector`
    // (crates/execution/src/core_asset_execution_tests.rs), recomputed here.
    assert_eq!(
        genesis_json_hash(&genesis),
        "16843cd2ee2a22a98063f2269d1a06279d33598d07331e32cbf838ee9798c9219c61bc0f6509871ec74c5760277d889f"
    );
    // The WAN devnet genesis file parses and re-serialises byte for byte.
    let devnet =
        "{\n  \"chain_id\": \"postfiat-wan-devnet\",\n  \"protocol_version\": 1,\n  \"validator_count\": 6\n}\n";
    let parsed = Genesis::from_json(devnet).expect("parse devnet genesis");
    assert_eq!(parsed.to_json().expect("serialize devnet genesis"), devnet);
    assert_eq!(
        genesis_json_hash(&parsed),
        "231b1cfb63439c23bdcc3f7ea2f7f3ce7a53f9abffef8f720f47421b575f16e7f2d9ad5e61298207be2e9ce08743f870"
    );
}

#[test]
fn ledger_state_without_lanes_serializes_as_before() {
    let ledger = LedgerState::new(Vec::new());
    let json = serde_json::to_string(&ledger).expect("serialize ledger");
    assert_eq!(json, "{\"accounts\":[]}");
}

#[test]
fn genesis_external_lane_reserves_the_configured_amount() {
    let genesis = genesis_with_lanes(vec![
        sample_external_lane("base-erc20", 250_000_000),
        sample_external_lane("base-erc20-b", 50_000_000),
    ]);
    genesis.validate().expect("valid lanes");
    let parsed = Genesis::from_json(&genesis.to_json().expect("serialize")).expect("parse");
    assert_eq!(parsed, genesis);
    assert_eq!(parsed.external_lane_reserved_atoms(), Ok(300_000_000));
    let states = parsed.genesis_external_lane_states();
    assert_eq!(states.len(), 2);
    assert_eq!(states[0].lane_id, "base-erc20");
    assert_eq!(states[0].reserved_atoms, 250_000_000);
    assert_eq!(states[0].released_atoms, 0);
    assert!(states[0].released_lock_nonces.is_empty());
    assert_eq!(
        (states[0].window_index, states[0].window_released_atoms),
        (0, 0)
    );
    assert_eq!(states[1].remainder_atoms(), Ok(50_000_000));
}

#[test]
fn genesis_rejects_lanes_above_native_supply() {
    let at_supply = genesis_with_lanes(vec![sample_external_lane(
        "base-erc20",
        GENESIS_NATIVE_SUPPLY_ATOMS,
    )]);
    assert!(at_supply
        .validate()
        .unwrap_err()
        .contains("below the native supply"));
    let split = genesis_with_lanes(vec![
        sample_external_lane("a", GENESIS_NATIVE_SUPPLY_ATOMS / 2),
        sample_external_lane("b", GENESIS_NATIVE_SUPPLY_ATOMS / 2),
    ]);
    assert!(split
        .validate()
        .unwrap_err()
        .contains("below the native supply"));
    let overflow = genesis_with_lanes(vec![
        sample_external_lane("a", u64::MAX),
        sample_external_lane("b", 1),
    ]);
    assert!(overflow.validate().unwrap_err().contains("overflows"));
    let just_below = genesis_with_lanes(vec![sample_external_lane(
        "base-erc20",
        GENESIS_NATIVE_SUPPLY_ATOMS - 1,
    )]);
    just_below.validate().expect("one atom left for the faucet");
}

#[test]
fn genesis_faucet_holds_supply_minus_lanes() {
    assert_eq!(
        Genesis::new("postfiat-local").genesis_faucet_balance_atoms(),
        Ok(GENESIS_NATIVE_SUPPLY_ATOMS)
    );
    let genesis = genesis_with_lanes(vec![
        sample_external_lane("base-erc20", 400_000_000),
        sample_external_lane("other", 100_000_000),
    ]);
    assert_eq!(
        genesis.genesis_faucet_balance_atoms(),
        Ok(GENESIS_NATIVE_SUPPLY_ATOMS - 500_000_000)
    );
}

#[test]
fn genesis_rejects_unknown_lane_fields() {
    let genesis = genesis_with_lanes(vec![sample_external_lane("base-erc20", 1)]);
    let json = genesis.to_json().expect("serialize");
    let lane_extra = json.replacen(
        "\"release_cap_bps\": 200,",
        "\"release_cap_bps\": 200,\n      \"mint_authority\": \"x\",",
        1,
    );
    assert_ne!(lane_extra, json);
    assert!(Genesis::from_json(&lane_extra)
        .unwrap_err()
        .to_string()
        .contains("unknown field `mint_authority`"));
    let policy_extra = json.replacen(
        "\"chain_id\": 8453,",
        "\"chain_id\": 8453,\n        \"admin\": \"x\",",
        1,
    );
    assert_ne!(policy_extra, json);
    assert!(Genesis::from_json(&policy_extra)
        .unwrap_err()
        .to_string()
        .contains("unknown field `admin`"));
    let state_extra = r#"{"lane_id":"a","reserved_atoms":1,"released_atoms":0,"window_index":0,"window_released_atoms":0,"owner":"x"}"#;
    assert!(serde_json::from_str::<ExternalLaneState>(state_extra).is_err());
}

#[test]
fn genesis_rejects_release_cap_outside_one_to_ten_thousand_bps() {
    for (bps, ok) in [(0, false), (1, true), (10_000, true), (10_001, false)] {
        let mut lane = sample_external_lane("base-erc20", 1);
        lane.release_cap_bps = bps;
        let result = genesis_with_lanes(vec![lane]).validate();
        assert_eq!(result.is_ok(), ok, "release_cap_bps {bps}: {result:?}");
    }
}

#[test]
fn genesis_rejects_malformed_external_lanes() {
    let empty = genesis_with_lanes(Vec::new());
    assert!(empty
        .validate()
        .unwrap_err()
        .contains("absent or non-empty"));
    let duplicate = genesis_with_lanes(vec![
        sample_external_lane("base-erc20", 1),
        sample_external_lane("base-erc20", 2),
    ]);
    assert!(duplicate.validate().unwrap_err().contains("duplicate"));
    let mut cases: Vec<(GenesisExternalLane, &str)> = Vec::new();
    for id in ["", "Base", "9lane", "lane_1", &"a".repeat(33)] {
        cases.push((sample_external_lane(id, 1), "external lane id"));
    }
    let mut lane = sample_external_lane("a", 0);
    cases.push((lane.clone(), "reserved_atoms"));
    lane.reserved_atoms = 1;
    lane.release_window_blocks = 0;
    cases.push((lane, "release_window_blocks"));
    let mut lane = sample_external_lane("a", 1);
    lane.policy.chain_id = 0;
    cases.push((lane, "chain_id"));
    let mut lane = sample_external_lane("a", 1);
    lane.policy.token_address = format!("0x{}", "AB".repeat(20));
    cases.push((lane, "token_address"));
    let mut lane = sample_external_lane("a", 1);
    lane.policy.lock_contract_address = "1a".repeat(21);
    cases.push((lane, "lock_contract_address"));
    let mut lane = sample_external_lane("a", 1);
    lane.policy.lock_runtime_code_hash = format!("0x{}", "4d".repeat(20));
    cases.push((lane, "lock_runtime_code_hash"));
    let mut lane = sample_external_lane("a", 1);
    lane.policy.token_runtime_code_hash = format!("0x{}", "zz".repeat(32));
    cases.push((lane, "token_runtime_code_hash"));
    for (lane, needle) in cases {
        let error = genesis_with_lanes(vec![lane.clone()])
            .validate()
            .unwrap_err();
        assert!(error.contains(needle), "{lane:?}: {error}");
    }
}

#[test]
fn external_lane_state_rejects_release_above_reserve() {
    let mut state = ExternalLaneState::genesis(&sample_external_lane("a", 10));
    state.validate().expect("genesis lane state");
    state.released_atoms = 11;
    state.released_lock_nonces.insert(1);
    assert!(state.remainder_atoms().is_err());
    assert!(state.state_commitment_bytes().is_err());
    state.released_atoms = 4;
    state.window_released_atoms = 5;
    assert!(state.validate().is_err());
}

fn lane_policy_json() -> serde_json::Value {
    serde_json::to_value(&sample_external_lane("base-erc20", 1).policy).expect("policy json")
}

#[test]
fn genesis_external_lane_policy_requires_proof_policy_pins() {
    for field in ["committee_epoch", "committee_root", "confirmation_depth"] {
        let mut policy = lane_policy_json();
        policy.as_object_mut().expect("object").remove(field);
        let error = serde_json::from_value::<GenesisExternalLanePolicy>(policy)
            .unwrap_err()
            .to_string();
        assert!(error.contains(&format!("missing field `{field}`")), "{error}");
    }
    let mut policy = lane_policy_json();
    policy["committee_signers"] = serde_json::json!(5);
    assert!(serde_json::from_value::<GenesisExternalLanePolicy>(policy)
        .unwrap_err()
        .to_string()
        .contains("unknown field `committee_signers`"));
    let mut policy = lane_policy_json();
    policy["committee_epoch"] = serde_json::json!("2");
    assert!(serde_json::from_value::<GenesisExternalLanePolicy>(policy).is_err());
    let mut policy = lane_policy_json();
    policy["confirmation_depth"] = serde_json::json!(-1);
    assert!(serde_json::from_value::<GenesisExternalLanePolicy>(policy).is_err());

    let mut cases: Vec<(GenesisExternalLanePolicy, &str)> = Vec::new();
    for root in [
        String::new(),
        "5e".repeat(32),
        format!("0x{}", "5E".repeat(32)),
        format!("0x{}", "5e".repeat(31)),
        format!("0x{}", "5e".repeat(48)),
        format!("0x{}g", "5e".repeat(31)),
    ] {
        let mut policy = sample_external_lane("a", 1).policy;
        policy.committee_root = root;
        cases.push((policy, "committee_root"));
    }
    let mut policy = sample_external_lane("a", 1).policy;
    policy.confirmation_depth = 0;
    cases.push((policy, "confirmation_depth"));
    for (policy, needle) in cases {
        let error = policy.validate().unwrap_err();
        assert!(error.contains(needle), "{policy:?}: {error}");
    }
    // chain_id is a parameter: Base Sepolia validates like Base mainnet.
    let mut sepolia = sample_external_lane("a", 1);
    sepolia.policy.chain_id = 84_532;
    genesis_with_lanes(vec![sepolia]).validate().expect("Base Sepolia lane");
}

#[test]
fn genesis_external_lane_docs_example_parses_and_validates() {
    let page = include_str!("../../../docs/architecture/genesis-external-lanes.md");
    let example = page
        .split("```json\n")
        .nth(1)
        .and_then(|rest| rest.split("```").next())
        .expect("docs page has a JSON example");
    let genesis = Genesis::from_json(example).expect("docs example parses");
    genesis.validate().expect("docs example validates");
    let policy = &genesis.external_lanes()[0].policy;
    assert_eq!(policy.chain_id, 8453);
    assert!(policy.committee_epoch > 0 && policy.confirmation_depth > 0);
}

#[test]
fn deployed_devnet_genesis_bytes_and_hash_are_unchanged() {
    // The genesis of the deployed WAN devnet `postfiat-wan-devnet-2`, kept as
    // crates/node/testdata/pfeth-reserve-replay/genesis.json; its hash is the
    // one pinned by crates/node/tests/fixtures/private-actions-v1/manifest.json.
    let deployed = include_str!("../../node/testdata/pfeth-reserve-replay/genesis.json");
    let genesis = Genesis::from_json(deployed).expect("parse deployed devnet genesis");
    assert_eq!(genesis.chain_id, "postfiat-wan-devnet-2");
    assert_eq!(genesis.external_lanes, None);
    genesis.validate().expect("deployed devnet genesis validates");
    assert_eq!(genesis.to_json().expect("serialize devnet genesis"), deployed);
    assert_eq!(
        genesis_json_hash(&genesis),
        "ce22ca8c932da0998b484483a09647138a30e0bf44408dd49a8d6d452787ad25521aff3ed334da07e150a7233a3e90a9"
    );
    assert_eq!(genesis.genesis_faucet_balance_atoms(), Ok(GENESIS_NATIVE_SUPPLY_ATOMS));
}
