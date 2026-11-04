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
            committee_root: format!("0x{}", "5e".repeat(48)),
            confirmation_depth: 64,
        },
    }
}

/// A lane on its own Base source: genesis allows one lane per lock contract.
fn sample_external_lane_on(id: &str, reserved_atoms: u64, lock_byte: &str) -> GenesisExternalLane {
    let mut lane = sample_external_lane(id, reserved_atoms);
    lane.policy.lock_contract_address = format!("0x{}", lock_byte.repeat(20));
    lane
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
        sample_external_lane_on("base-erc20-b", 50_000_000, "7b"),
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
fn genesis_rejects_lanes_at_or_above_native_supply() {
    let half = GENESIS_NATIVE_SUPPLY_ATOMS / 2;
    let at_or_above = [
        // One lane, sum == supply.
        vec![sample_external_lane("base-erc20", GENESIS_NATIVE_SUPPLY_ATOMS)],
        // Two lanes, sum == supply.
        vec![
            sample_external_lane("a", half),
            sample_external_lane_on("b", GENESIS_NATIVE_SUPPLY_ATOMS - half, "7b"),
        ],
        // Two lanes, ordinary sum == supply + 1 (far from u64 overflow).
        vec![
            sample_external_lane("a", half),
            sample_external_lane_on("b", GENESIS_NATIVE_SUPPLY_ATOMS - half + 1, "7b"),
        ],
    ];
    for lanes in at_or_above {
        let total: u64 = lanes.iter().map(|lane| lane.reserved_atoms).sum();
        assert!(total >= GENESIS_NATIVE_SUPPLY_ATOMS);
        let error = genesis_with_lanes(lanes).validate().unwrap_err();
        assert_eq!(
            error,
            format!(
                "genesis external lanes reserve {total} atoms, which must be below the native supply {GENESIS_NATIVE_SUPPLY_ATOMS}"
            )
        );
    }
    let overflow = genesis_with_lanes(vec![
        sample_external_lane("a", u64::MAX),
        sample_external_lane_on("b", 1, "7b"),
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
        "5e".repeat(48),
        format!("0x{}", "5E".repeat(48)),
        format!("0x{}", "5e".repeat(47)),
        // 32 bytes, the width P2.2 allowed: no committee root has it.
        format!("0x{}", "5e".repeat(32)),
        format!("0x{}g", "5e".repeat(47)),
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

fn sample_release(nonce: u64, amount_atoms: u64, height: u64) -> ExternalLaneReleaseRecord {
    ExternalLaneReleaseRecord {
        lock_nonce: nonce,
        source_chain_id: 8453,
        lock_contract_address: format!("0x{}", "1a".repeat(20)),
        amount_atoms,
        recipient: "pftl-recipient".to_string(),
        source_block_number: 100,
        source_block_hash: format!("0x{}", "6f".repeat(32)),
        released_at_height: height,
    }
}

/// A lane of 10,000 atoms at 200 bps: a window of 100 blocks may release 200.
fn release_test_lane() -> GenesisExternalLane {
    let mut lane = sample_external_lane("base-erc20", 10_000);
    lane.release_window_blocks = 100;
    lane
}

#[test]
fn genesis_external_lane_source_must_be_base_mainnet_or_sepolia() {
    for chain_id in [8453, 84_532] {
        let mut lane = sample_external_lane("a", 1);
        lane.policy.chain_id = chain_id;
        lane.validate().expect("allowed Base source");
    }
    for chain_id in [1, 10, 11_155_111, 8454] {
        let mut lane = sample_external_lane("a", 1);
        lane.policy.chain_id = chain_id;
        let error = lane.validate().unwrap_err();
        assert!(error.contains("must be Base mainnet 8453 or Base Sepolia 84532"), "{error}");
    }
}

#[test]
fn external_lane_window_cap_rounds_down() {
    let lane = release_test_lane();
    assert_eq!(lane.window_cap_atoms(), 200);
    let mut odd = sample_external_lane("a", 149);
    odd.release_cap_bps = 100;
    assert_eq!(odd.window_cap_atoms(), 1);
    assert_eq!(lane.window_index_at(99), 0);
    assert_eq!(lane.window_index_at(100), 1);
}

#[test]
fn external_lane_release_credits_the_lane_record_once() {
    let lane = release_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    state
        .apply_release(&lane, sample_release(0, 150, 5))
        .expect("first release");
    assert_eq!(state.released_atoms, 150);
    assert_eq!(state.remainder_atoms().expect("remainder"), 9_850);
    assert_eq!(state.released_lock_nonces, [0_u64].into_iter().collect());
    assert_eq!((state.window_index, state.window_released_atoms), (0, 150));
    state.validate_against_genesis(&lane).expect("recorded release");

    let before = state.clone();
    let (code, _) = state
        .apply_release(&lane, sample_release(0, 1, 6))
        .unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_NONCE_ALREADY_RELEASED);
    assert_eq!(state, before, "a replayed nonce changes nothing");
}

#[test]
fn external_lane_release_refuses_amount_past_window_cap_without_mutation() {
    let lane = release_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    state
        .apply_release(&lane, sample_release(0, 150, 5))
        .expect("first release");
    let before = state.clone();
    let (code, message) = state
        .apply_release(&lane, sample_release(1, 51, 99))
        .unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_WINDOW_CAP_EXCEEDED, "{message}");
    assert_eq!(state, before);
    // The same nonce fits in the next window: the refusal consumed nothing.
    state
        .apply_release(&lane, sample_release(1, 51, 100))
        .expect("later window");
    assert_eq!((state.window_index, state.window_released_atoms), (1, 51));
    state.validate_against_genesis(&lane).expect("two windows");
}

#[test]
fn external_lane_release_refuses_past_the_record_ceiling_without_mutation() {
    let lane = release_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    state
        .apply_release_within(&lane, sample_release(0, 10, 5), lane.window_cap_atoms(), 2)
        .expect("first record");
    state
        .apply_release_within(&lane, sample_release(1, 10, 6), lane.window_cap_atoms(), 2)
        .expect("second record");
    let before = state.clone();
    let (code, message) = state
        .apply_release_within(&lane, sample_release(2, 10, 7), lane.window_cap_atoms(), 2)
        .unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_RELEASE_RECORDS_FULL, "{message}");
    assert_eq!(state, before, "a refused release leaves the lane untouched");
    state
        .apply_release(&lane, sample_release(2, 10, 7))
        .expect("far below the ceiling");
    state.validate_against_genesis(&lane).expect("three records");
}

#[test]
fn external_lane_release_rejects_amount_above_lane_remainder() {
    let mut lane = release_test_lane();
    lane.release_cap_bps = 10_000;
    let mut state = ExternalLaneState::genesis(&lane);
    state
        .apply_release(&lane, sample_release(0, 9_990, 1))
        .expect("most of the lane");
    let before = state.clone();
    let (code, _) = state
        .apply_release(&lane, sample_release(1, 11, 200))
        .unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_REMAINDER_EXCEEDED);
    assert_eq!(state, before);
    state
        .apply_release(&lane, sample_release(1, 10, 200))
        .expect("exactly the remainder");
    assert_eq!(state.remainder_atoms().expect("remainder"), 0);
}

#[test]
fn external_lane_state_requires_release_fields_to_match_recorded_releases() {
    let lane = release_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    state
        .apply_release(&lane, sample_release(3, 40, 10))
        .expect("release");
    state
        .apply_release(&lane, sample_release(7, 60, 20))
        .expect("release");
    state.validate_against_genesis(&lane).expect("consistent");

    let refuse = |edit: &dyn Fn(&mut ExternalLaneState), needle: &str| {
        let mut tampered = state.clone();
        edit(&mut tampered);
        let error = tampered.validate_against_genesis(&lane).unwrap_err();
        assert!(error.contains(needle), "{needle}: {error}");
    };
    // Atoms released with no record behind them.
    refuse(
        &|lane| {
            lane.released_atoms += 5;
            lane.window_released_atoms += 5;
        },
        "recorded releases sum to 100",
    );
    // A nonce added to the set without a record, or a record dropped.
    refuse(&|lane| {
        lane.released_lock_nonces.insert(9);
    }, "differ from its recorded releases");
    refuse(&|lane| {
        lane.releases.pop();
    }, "differ from its recorded releases");
    // A record duplicated, an amount raised, a foreign source.
    refuse(&|lane| {
        let first = lane.releases[0].clone();
        lane.releases.push(first);
    }, "more than once");
    refuse(&|lane| lane.releases[0].amount_atoms += 1, "recorded releases sum to 101");
    refuse(&|lane| lane.releases[0].source_chain_id = 84_532, "source genesis did not configure");
    refuse(
        &|lane| lane.releases[1].lock_contract_address = format!("0x{}", "99".repeat(20)),
        "source genesis did not configure",
    );
    // Window fields that do not follow from the recorded heights.
    refuse(&|lane| lane.window_released_atoms = 60, "recorded releases give window 0 with 100");
    refuse(&|lane| lane.window_index = 1, "recorded releases give window 0 with 100");
    // Reserve changed against genesis.
    refuse(&|lane| lane.reserved_atoms += 1, "but genesis reserves 10000");
}

#[test]
fn external_lane_recorded_window_above_cap_is_refused() {
    let lane = release_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    // Two records that fit the remainder but together pass one window's cap.
    for (nonce, height) in [(0, 10), (1, 20)] {
        let record = sample_release(nonce, 150, height);
        state.released_atoms += record.amount_atoms;
        state.released_lock_nonces.insert(nonce);
        state.releases.push(record);
    }
    state.window_released_atoms = 300;
    state.validate().expect("internally consistent");
    let error = state.validate_against_genesis(&lane).unwrap_err();
    assert!(error.contains("above the cap 200"), "{error}");
}

#[test]
fn unreleased_external_lane_state_encodes_without_releases() {
    let state = ExternalLaneState::genesis(&release_test_lane());
    let json = serde_json::to_string(&state).expect("encode");
    assert!(!json.contains("releases\""), "{json}");
    assert_eq!(
        json,
        "{\"lane_id\":\"base-erc20\",\"reserved_atoms\":10000,\"released_atoms\":0,\"released_lock_nonces\":[],\"window_index\":0,\"window_released_atoms\":0}"
    );
}

fn sample_release_operation() -> ExternalLaneReleaseOperationV1 {
    let checkpoint = EthereumFinalizedCheckpointV1 {
        schema_version: ETHEREUM_CHECKPOINT_SCHEMA_V1,
        pftl_domain: FastSwapChainDomainV1 {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: FastSwapOpaqueHashV1([7; 48]),
            protocol_version: 1,
        },
        route_id: "base-erc20".to_string(),
        route_config_digest: FastSwapOpaqueHashV1([8; 48]),
        ethereum_chain_id: 8453,
        block_number: 10,
        block_hash: [9; 32],
        receipts_root: [10; 32],
        observed_head_number: 80,
        minimum_confirmations: 64,
        authority_epoch: 2,
        committee_root: FastSwapCommitteeRootV1([11; 48]),
        handoff_controller: [0x1a; 20],
        wrapped_navcoin_token: [0x2b; 20],
        handoff_controller_code_hash: [0x4d; 32],
        wrapped_navcoin_code_hash: [0x3c; 32],
    };
    ExternalLaneReleaseOperationV1 {
        submitter: "pf1111111111111111111111111111111111111111".to_string(),
        lane_id: "base-erc20".to_string(),
        source_chain_id: 8453,
        lock_contract_address: format!("0x{}", "1a".repeat(20)),
        lock_nonce: 0,
        amount_atoms: 5,
        recipient: "pf2222222222222222222222222222222222222222".to_string(),
        proof: EthereumExternalEventProofV1 {
            checkpoint_certificate: EthereumCheckpointCertificateV1 {
                checkpoint,
                votes: vec![EthereumCheckpointVoteV1 {
                    validator_id: "validator-0".to_string(),
                    signature: vec![1],
                }],
            },
            receipt_proof: EthereumReceiptProofV1 {
                transaction_index: 0,
                receipt_rlp: vec![0xc0],
                proof_nodes_rlp: vec![vec![0xc0]],
            },
            log_index: 0,
        },
    }
}

type OperationEdit = fn(&mut ExternalLaneReleaseOperationV1);

#[test]
fn external_lane_release_operation_validates_its_lock_key() {
    let operation = sample_release_operation();
    operation.validate().expect("valid release operation");
    let wrapped = AssetTransactionOperation::ExternalLaneReleaseV1(operation.clone());
    assert_eq!(wrapped.transaction_kind(), EXTERNAL_LANE_RELEASE_TRANSACTION_KIND_V1);
    let json = serde_json::to_value(&wrapped).expect("encode");
    assert_eq!(json["operation"], "external_lane_release_v1");
    let decoded: AssetTransactionOperation = serde_json::from_value(json.clone()).expect("decode");
    assert_eq!(decoded, wrapped);
    let mut unknown = json;
    unknown["relayer_override"] = serde_json::json!("x");
    assert!(serde_json::from_value::<AssetTransactionOperation>(unknown).is_err());

    let cases: [(OperationEdit, &str); 6] = [
        (|op| op.source_chain_id = 1, "source_chain_id"),
        (|op| op.lock_contract_address = "1a".repeat(20), "lock_contract_address"),
        (|op| op.amount_atoms = 0, "amount_atoms"),
        (|op| op.recipient = String::new(), "recipient"),
        (|op| op.lane_id = "Base".to_string(), "external lane id"),
        (|op| op.proof.checkpoint_certificate.votes.clear(), "proof"),
    ];
    for (edit, needle) in cases {
        let mut operation = sample_release_operation();
        edit(&mut operation);
        let error = operation.validate().unwrap_err();
        assert!(error.contains(needle), "{needle}: {error}");
    }
}

#[test]
fn genesis_rejects_two_lanes_on_one_base_source() {
    let mut other = sample_external_lane("b", 10);
    let first = sample_external_lane("a", 10);
    let error = validate_genesis_external_lanes(&[first.clone(), other.clone()], 1_000).unwrap_err();
    assert!(
        error.contains("genesis external lanes `a` and `b` both name Base source 8453"),
        "{error}"
    );
    // A different lock contract or chain is a different source.
    other.policy.lock_contract_address = format!("0x{}", "99".repeat(20));
    validate_genesis_external_lanes(&[first.clone(), other.clone()], 1_000).expect("other contract");
    other.policy.lock_contract_address = first.policy.lock_contract_address.clone();
    other.policy.chain_id = 84_532;
    validate_genesis_external_lanes(&[first, other], 1_000).expect("other chain");
}

#[test]
fn external_lane_release_below_the_recorded_window_is_refused() {
    let lane = release_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    state
        .apply_release(&lane, sample_release(0, 200, 250))
        .expect("window 2 at its cap");
    let before = state.clone();
    let (code, message) = state
        .apply_release(&lane, sample_release(1, 200, 150))
        .unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_WINDOW_REGRESSED, "{message}");
    assert!(message.contains("release window 1 is below its recorded window 2"), "{message}");
    assert_eq!(state, before);
}

#[test]
fn external_lane_commitment_is_independent_of_release_records() {
    let lane = sample_external_lane("base-erc20", 1_000_000_000);
    let mut state = ExternalLaneState::genesis(&lane);
    state
        .apply_release(&lane, sample_release(0, 7, 1))
        .expect("first release");
    let one = state.state_commitment_bytes().expect("commit one");
    for nonce in 1..200 {
        state
            .apply_release(&lane, sample_release(nonce, 7, 1 + nonce))
            .expect("release");
    }
    let many = state.state_commitment_bytes().expect("commit many");
    // Only the design's fields grow: the nonce set, and the released and
    // window totals by their digits. No release record is in the bytes.
    let nonce_growth = serde_json::to_vec(&state.released_lock_nonces).expect("nonces").len()
        - serde_json::to_vec(&[0_u64]).expect("one nonce").len();
    let total_digits = 2 * (state.released_atoms.to_string().len() - 1);
    assert_eq!(many.len() - one.len(), nonce_growth + total_digits);
    let mut without_records = state.clone();
    without_records.releases.clear();
    assert_eq!(without_records.state_commitment_bytes().expect("commit"), many);
    assert!(!String::from_utf8(many).expect("utf8").contains("pftl-recipient"));
}

#[test]
fn external_lane_release_history_root_binds_every_record() {
    let lane = release_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    assert_eq!(state.release_history_root, None);
    state
        .apply_release(&lane, sample_release(3, 40, 10))
        .expect("release");
    let first_root = state.release_history_root.clone().expect("root");
    assert_eq!(first_root.len(), 2 + 96);
    state
        .apply_release(&lane, sample_release(7, 60, 20))
        .expect("release");
    assert_ne!(state.release_history_root.as_deref(), Some(first_root.as_str()));
    state.validate_against_genesis(&lane).expect("consistent");
    // A record changed without changing the totals: only the root sees it.
    let mut tampered = state.clone();
    tampered.releases[1].recipient = "someone-else".to_string();
    assert_eq!(
        tampered.state_commitment_bytes().expect("bounded commitment"),
        state.state_commitment_bytes().expect("bounded commitment")
    );
    let error = tampered.validate_against_genesis(&lane).unwrap_err();
    assert!(error.contains("release history root"), "{error}");
}

fn cap_amendment(id: &str, cap_bps: u32, committed_height: u64) -> ExternalLaneReleaseCapAmendment {
    ExternalLaneReleaseCapAmendment {
        amendment_id: id.to_string(),
        cap_bps,
        committed_height,
    }
}

/// The cap in force at `height` (window 100 blocks, genesis cap 200 bps).
fn cap_at(amendments: &[ExternalLaneReleaseCapAmendment], height: u64) -> u32 {
    release_test_lane()
        .release_cap_schedule_at(amendments, height)
        .active_cap_bps
}

#[test]
fn release_cap_without_amendments_is_the_genesis_cap() {
    let lane = release_test_lane();
    let schedule = lane.release_cap_schedule_at(&[], 1_000_000);
    assert_eq!(schedule.active_cap_bps, 200);
    assert_eq!((schedule.active, schedule.pending), (None, None));
    assert_eq!(lane.window_cap_atoms_for_bps(schedule.active_cap_bps), 200);
    assert_eq!(lane.window_cap_atoms_for_bps(0), 0);
}

#[test]
fn lower_release_cap_applies_from_its_activation_height() {
    let lower = [cap_amendment("lower", 50, 250)];
    assert_eq!(cap_at(&lower, 249), 200);
    assert_eq!(cap_at(&lower, 250), 50);
    assert_eq!(cap_at(&lower, 251), 50);
    let schedule = release_test_lane().release_cap_schedule_at(&lower, 250);
    assert_eq!(
        schedule.active,
        Some(ExternalLaneReleaseCapChange {
            amendment_id: "lower".to_string(),
            cap_bps: 50,
            effective_height: 250,
        })
    );
    assert_eq!(schedule.pending, None);
    // A zero cap halts every window from its activation height.
    let zero = [cap_amendment("zero", 0, 250)];
    assert_eq!(cap_at(&zero, 250), 0);
    assert_eq!(cap_at(&zero, 10_000), 0);
}

#[test]
fn higher_release_cap_applies_seven_windows_after_it_commits() {
    // Committed at 250, the raise takes effect at 250 + 7 × 100 = 950.
    let raise = [cap_amendment("raise", 500, 250)];
    assert_eq!(EXTERNAL_LANE_RELEASE_CAP_RAISE_DELAY_WINDOWS, 7);
    assert_eq!(cap_at(&raise, 250), 200);
    assert_eq!(cap_at(&raise, 949), 200);
    assert_eq!(cap_at(&raise, 950), 500);
    let lane = release_test_lane();
    // The window before the delay ends still holds the old cap.
    assert_eq!(lane.window_index_at(949), 9);
    let pending = lane.release_cap_schedule_at(&raise, 949);
    assert_eq!((pending.active_cap_bps, pending.active), (200, None));
    assert_eq!(
        pending.pending,
        Some(ExternalLaneReleaseCapChange {
            amendment_id: "raise".to_string(),
            cap_bps: 500,
            effective_height: 950,
        })
    );
    let applied = lane.release_cap_schedule_at(&raise, 950);
    assert_eq!(applied.active_cap_bps, 500);
    assert_eq!(applied.active.map(|change| change.effective_height), Some(950));
    assert_eq!(applied.pending, None);
    // Before the amendment commits, nothing of it shows.
    assert_eq!(lane.release_cap_schedule_at(&raise, 249).pending, None);
}

#[test]
fn latest_applied_release_cap_amendment_wins() {
    // Two lowers in one window: the later one holds from its height.
    let two_lowers = [cap_amendment("a", 150, 210), cap_amendment("b", 100, 260)];
    assert_eq!(cap_at(&two_lowers, 209), 200);
    assert_eq!(cap_at(&two_lowers, 210), 150);
    assert_eq!(cap_at(&two_lowers, 260), 100);
    // A later amendment back up is a raise against the cap in force.
    let lower_then_raise = [cap_amendment("a", 50, 210), cap_amendment("b", 150, 260)];
    assert_eq!(cap_at(&lower_then_raise, 959), 50);
    assert_eq!(cap_at(&lower_then_raise, 960), 150);
    // Ties at one height follow commit order.
    let tie = [cap_amendment("a", 50, 300), cap_amendment("b", 120, 300)];
    assert_eq!(cap_at(&tie, 300), 50, "b raises against a, so it waits");
    assert_eq!(cap_at(&tie, 1_000), 120);
    let tie = [cap_amendment("a", 120, 300), cap_amendment("b", 50, 300)];
    assert_eq!(cap_at(&tie, 300), 50);
    assert_eq!(cap_at(&tie, 1_000), 50);
}

#[test]
fn lower_release_cap_after_a_pending_raise_cancels_the_raise() {
    // The raise commits at 250 (effective 950); a zero cap commits at 400,
    // before the raise took effect, and holds past 950.
    let amendments = [cap_amendment("raise", 500, 250), cap_amendment("halt", 0, 400)];
    assert_eq!(cap_at(&amendments, 399), 200);
    assert_eq!(cap_at(&amendments, 400), 0);
    assert_eq!(cap_at(&amendments, 950), 0);
    let schedule = release_test_lane().release_cap_schedule_at(&amendments, 950);
    assert_eq!(schedule.active.map(|change| change.amendment_id), Some("halt".to_string()));
    assert_eq!(schedule.pending, None);
    // A second raise while one is pending replaces it and restarts the delay.
    let two_raises = [cap_amendment("r1", 500, 250), cap_amendment("r2", 300, 400)];
    assert_eq!(cap_at(&two_raises, 950), 200);
    assert_eq!(cap_at(&two_raises, 1_099), 200);
    assert_eq!(cap_at(&two_raises, 1_100), 300);
    // A raise already in effect is not cancelled by a later amendment.
    let applied_then_lower = [cap_amendment("r1", 500, 250), cap_amendment("l", 300, 960)];
    assert_eq!(cap_at(&applied_then_lower, 959), 500);
    assert_eq!(cap_at(&applied_then_lower, 960), 300);
}

#[test]
fn release_cap_rule_matches_the_schedule_past_the_last_commit() {
    let lane = release_test_lane();
    let amendments = [cap_amendment("lower", 50, 210), cap_amendment("raise", 400, 300)];
    let rule = lane.release_cap_rule(&amendments);
    for height in [300, 999, 1_000, 5_000] {
        assert_eq!(
            rule.cap_bps_at(height),
            lane.release_cap_schedule_at(&amendments, height).active_cap_bps,
            "{height}"
        );
    }
    assert_eq!(rule.cap_bps_at(999), 50);
    assert_eq!(rule.cap_bps_at(1_000), 400);
    assert_eq!(lane.release_cap_rule(&[]).cap_bps_at(0), 200);
}

#[test]
fn genesis_refuses_more_external_lanes_than_the_cap_table_holds() {
    let lanes = |count: usize| {
        (0..count)
            .map(|index| sample_external_lane_on(&format!("lane-{index}"), 1_000, &format!("{:02x}", 0x10 + index)))
            .collect::<Vec<_>>()
    };
    validate_genesis_external_lanes(&lanes(EXTERNAL_LANE_MAX_LANES), GENESIS_NATIVE_SUPPLY_ATOMS)
        .expect("eight lanes");
    let error =
        validate_genesis_external_lanes(&lanes(EXTERNAL_LANE_MAX_LANES + 1), GENESIS_NATIVE_SUPPLY_ATOMS)
            .unwrap_err();
    assert!(error.contains("above the maximum 8"), "{error}");
}

#[test]
fn recorded_windows_are_checked_against_the_cap_active_at_their_heights() {
    let lane = release_test_lane();
    // Lowered to 50 bps (50 atoms) at 120; raised to 400 bps (400 atoms) at
    // 300, effective at 1_000.
    let amendments = [cap_amendment("lower", 50, 120), cap_amendment("raise", 400, 300)];
    let mut state = ExternalLaneState::genesis(&lane);
    state
        .apply_release_capped(&lane, sample_release(0, 150, 110), 200)
        .expect("genesis cap before the lowering");
    state
        .apply_release_capped(&lane, sample_release(1, 50, 250), 50)
        .expect("lowered cap");
    state
        .apply_release_capped(&lane, sample_release(2, 400, 1_000), 400)
        .expect("raised cap after the delay");
    state
        .validate_against_genesis_with_caps(&lane, &amendments)
        .expect("each window within the cap active at its heights");
    // Without the raise the window of height 1_000 is above the cap.
    let error = state
        .validate_against_genesis_with_caps(&lane, &amendments[..1])
        .unwrap_err();
    assert!(error.contains("above the cap 50"), "{error}");
    // A window above the lowered cap is refused even though the genesis cap holds it.
    let mut over = ExternalLaneState::genesis(&lane);
    over.apply_release_capped(&lane, sample_release(0, 60, 130), 200)
        .expect("written past the lowered cap");
    over.validate_against_genesis(&lane).expect("within the genesis cap");
    let error = over
        .validate_against_genesis_with_caps(&lane, &amendments)
        .unwrap_err();
    assert!(error.contains("above the cap 50"), "{error}");
    // A lowering in the middle of a window: releases before it count under the old cap.
    let mid = [cap_amendment("mid", 50, 150)];
    let mut split = ExternalLaneState::genesis(&lane);
    split
        .apply_release_capped(&lane, sample_release(0, 100, 140), 200)
        .expect("before the lowering");
    split
        .validate_against_genesis_with_caps(&lane, &mid)
        .expect("released before the lowering");
    split
        .apply_release_capped(&lane, sample_release(1, 1, 150), 200)
        .expect("forged past the lowered cap");
    assert!(split.validate_against_genesis_with_caps(&lane, &mid).is_err());
}

/// The lane of `release_test_lane` after one release per height, nonce `i`
/// for the `i`-th height, 10 atoms each.
fn lane_after_releases(lane: &GenesisExternalLane, heights: &[u64]) -> ExternalLaneState {
    let mut state = ExternalLaneState::genesis(lane);
    for (nonce, height) in heights.iter().enumerate() {
        state
            .apply_release(lane, sample_release(nonce as u64, 10, *height))
            .expect("release");
    }
    state
}

fn unpruned_history_root(heights: &[u64]) -> Option<String> {
    heights.iter().enumerate().fold(None, |root, (nonce, height)| {
        Some(
            external_lane_release_history_root(
                root.as_deref(),
                &sample_release(nonce as u64, 10, *height),
            )
            .expect("root"),
        )
    })
}

#[test]
fn external_lane_release_prunes_records_behind_the_finalized_height_at_the_boundary() {
    // Window 100 blocks, retention one window: a release at height H prunes
    // the records released at or below H - 100.
    let lane = release_test_lane();
    let kept = lane_after_releases(&lane, &[5, 104]);
    assert_eq!(kept.releases.len(), 2, "height 5 is above 104 - 100");
    let pruned = lane_after_releases(&lane, &[5, 105]);
    assert_eq!(pruned.releases.len(), 1, "height 5 is at 105 - 100");
    assert_eq!(pruned.releases[0].lock_nonce, 1);
    // The root, the counters and the nonce set keep the pruned release.
    assert_eq!(pruned.release_history_root, unpruned_history_root(&[5, 105]));
    assert_eq!(pruned.released_atoms, 20);
    assert_eq!(pruned.released_lock_nonces, [0_u64, 1].into_iter().collect());
    assert_eq!(pruned.pruned_release_count, 1);
    assert_eq!(pruned.pruned_released_atoms, 10);
    assert_eq!(pruned.pruned_release_history_root, unpruned_history_root(&[5]));
    assert_eq!(pruned.release_count(), 2);
    pruned.validate_against_genesis(&lane).expect("pruned lane");
    kept.validate_against_genesis(&lane).expect("unpruned lane");
    // The pruned nonce stays paid.
    let mut replay = pruned.clone();
    let (code, _) = replay
        .apply_release(&lane, sample_release(0, 10, 106))
        .unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_NONCE_ALREADY_RELEASED);
    assert_eq!(replay, pruned);
}

#[test]
fn external_lane_release_keeps_the_open_window_records() {
    // Three releases in window 1, then one in window 2 more than a window
    // later: the open window's records stay until a later window opens.
    let lane = release_test_lane();
    let state = lane_after_releases(&lane, &[100, 150, 199]);
    assert_eq!(state.releases.len(), 3);
    assert_eq!((state.window_index, state.window_released_atoms), (1, 30));
    let state = lane_after_releases(&lane, &[100, 150, 199, 298]);
    assert_eq!(state.releases.len(), 2, "heights 100 and 150 are at or below 298 - 100");
    assert_eq!((state.window_index, state.window_released_atoms), (2, 10));
    state.validate_against_genesis(&lane).expect("pruned lane");
}

#[test]
fn external_lane_releases_past_the_record_ceiling_with_pruning_are_admitted() {
    // A ceiling of 2 records: one release per window prunes the older ones,
    // so the ceiling is never reached.
    let lane = release_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    for nonce in 0..20_u64 {
        state
            .apply_release_within(&lane, sample_release(nonce, 10, 5 + nonce * 100), lane.window_cap_atoms(), 2)
            .expect("pruning keeps the lane below the ceiling");
        assert!(state.releases.len() <= 2);
    }
    assert_eq!(state.released_atoms, 200);
    assert_eq!(state.released_lock_nonces.len(), 20);
    assert_eq!(state.release_count(), 20);
    state.validate_against_genesis(&lane).expect("pruned lane");
}

#[test]
fn external_lane_release_of_zero_atoms_is_refused_before_a_release_can_prune_it() {
    // A zero-amount release followed by a release one window later that would
    // prune it: the writer's own state must still pass the lane rule.
    let lane = release_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    let zero = state.apply_release(&lane, sample_release(0, 0, 5));
    state
        .apply_release(&lane, sample_release(1, 10, 105))
        .expect("positive release");
    state
        .validate_against_genesis(&lane)
        .expect("the writer's own state passes the lane rule");
    let (code, error) = zero.unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_PROOF_INVALID);
    assert!(error.contains("zero amount"), "{error}");
    let (code, _) = state.check_release(&lane, 2, 0, 106).unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_PROOF_INVALID);
    assert_eq!((state.release_count(), state.pruned_release_count), (1, 0));
    assert_eq!(state.released_lock_nonces, [1_u64].into_iter().collect());
}

#[test]
fn pruned_external_lane_refuses_forged_records_tampered_roots_and_removed_ids() {
    let lane = release_test_lane();
    let state = lane_after_releases(&lane, &[5, 105, 106]);
    assert_eq!(state.pruned_release_count, 1);
    state.validate_against_genesis(&lane).expect("pruned lane");
    let refuse = |edit: &dyn Fn(&mut ExternalLaneState), needle: &str| {
        let mut tampered = state.clone();
        edit(&mut tampered);
        let error = tampered.validate_against_genesis(&lane).unwrap_err();
        assert!(error.contains(needle), "{needle}: {error}");
    };
    refuse(
        &|lane| lane.releases[0].recipient = "pftl-forged".to_string(),
        "does not match its recorded releases",
    );
    refuse(
        &|lane| lane.release_history_root = unpruned_history_root(&[5]),
        "does not match its recorded releases",
    );
    refuse(
        &|lane| lane.pruned_release_history_root = unpruned_history_root(&[105]),
        "does not match its recorded releases",
    );
    refuse(
        &|lane| {
            lane.released_lock_nonces.remove(&0);
        },
        "differ from its recorded releases",
    );
    // A pruned nonce swapped for another keeps the count; the lane alone
    // cannot see it, but the nonce set is committed, so the state root moves
    // and the checkpoint and snapshot root checks refuse it.
    let mut swapped = state.clone();
    swapped.released_lock_nonces.remove(&0);
    swapped.released_lock_nonces.insert(9);
    assert_ne!(
        swapped.state_commitment_bytes().expect("commit"),
        state.state_commitment_bytes().expect("commit")
    );
    refuse(&|lane| lane.pruned_released_atoms += 1, "recorded releases sum to 31");
    refuse(&|lane| lane.pruned_release_count = 0, "inconsistent pruned release fields");
    refuse(
        &|lane| {
            lane.releases.clear();
            lane.pruned_release_count = 3;
        },
        "inconsistent pruned release fields",
    );
}

#[test]
fn pruned_external_lane_commits_its_pruned_fields() {
    let lane = release_test_lane();
    let unpruned = lane_after_releases(&lane, &[5, 104]);
    let bytes = String::from_utf8(unpruned.state_commitment_bytes().expect("commit")).expect("utf8");
    assert!(!bytes.contains("pruned"), "an unpruned lane commits as before: {bytes}");
    let pruned = lane_after_releases(&lane, &[5, 105]);
    let bytes = String::from_utf8(pruned.state_commitment_bytes().expect("commit")).expect("utf8");
    assert!(bytes.contains("\"pruned_release_count\":1"), "{bytes}");
    let json = serde_json::to_string(&pruned).expect("json");
    let decoded: ExternalLaneState = serde_json::from_str(&json).expect("decode");
    assert_eq!(decoded, pruned);
}

#[test]
fn cap_table_halts_a_lane_it_cannot_hold() {
    let lane = release_test_lane();
    let caps = ExternalLaneReleaseCaps::genesis();
    assert_eq!(caps.cap_bps_at(EXTERNAL_LANE_MAX_LANES - 1, &lane, 10), 200);
    assert_eq!(caps.cap_bps_at(EXTERNAL_LANE_MAX_LANES, &lane, 10), 0);
}

#[cfg(debug_assertions)]
#[test]
#[should_panic(expected = "exceed the cap table")]
fn cap_table_refuses_more_lanes_than_it_holds() {
    let lanes = (0..=EXTERNAL_LANE_MAX_LANES)
        .map(|index| sample_external_lane_on(&format!("lane-{index}"), 1_000, &format!("{:02x}", 0x10 + index)))
        .collect::<Vec<_>>();
    let _ = ExternalLaneReleaseCaps::from_amendments(&lanes, |_| Vec::new());
}

#[test]
fn cap_amendment_for_a_lane_genesis_does_not_configure_is_refused() {
    let lanes = ["base-erc20"];
    let refusal = |kind: &str| external_lane_release_cap_lane_refusal(kind, lanes.into_iter()).map(|(code, _)| code);
    assert_eq!(
        refusal(&external_lane_release_cap_amendment_kind("not-a-lane")),
        Some(EXTERNAL_LANE_RELEASE_CAP_LANE_NOT_CONFIGURED)
    );
    assert_eq!(refusal(&external_lane_release_cap_amendment_kind("base-erc20")), None);
    assert_eq!(refusal("orchard_pool_pause"), None);
}
