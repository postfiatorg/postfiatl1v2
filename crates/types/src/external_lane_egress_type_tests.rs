const EXTERNAL_LANE_ROUTE_PROFILE_VECTORS: &str =
    include_str!("../testdata/external-lane-route-profile-vectors.json");
fn keccak_commitment_hex(hex48: &str) -> String {
    format!(
        "0x{}",
        bytes_to_hex(&Keccak256::digest(pfusdc_decode_hex(hex48).expect("hex")))
    )
}

#[test]
fn external_lane_route_profile_and_asset_id_match_the_shared_vectors() {
    let vectors: serde_json::Value =
        serde_json::from_str(EXTERNAL_LANE_ROUTE_PROFILE_VECTORS).expect("vector file");
    for (field, domain) in [
        ("asset_id", EXTERNAL_LANE_ASSET_ID_DOMAIN_V1),
        (
            "route_profile_hash",
            EXTERNAL_LANE_ROUTE_PROFILE_HASH_DOMAIN_V1,
        ),
        (
            "route_profile_schema",
            EXTERNAL_LANE_ROUTE_PROFILE_SCHEMA_V1,
        ),
    ] {
        assert_eq!(vectors["domains"][field], domain);
    }
    let vectors = vectors["vectors"].as_array().expect("vectors");
    assert_eq!(vectors.len(), 2);
    for vector in vectors {
        let lane: GenesisExternalLane =
            serde_json::from_value(vector["lane"].clone()).expect("lane");
        let chain_id = vector["pftl_chain_id"].as_str().expect("chain id");
        let asset_id = lane.pft_asset_id(chain_id).expect("asset id");
        let profile = lane.route_profile(chain_id).expect("profile");
        assert_eq!(asset_id, vector["asset_id"]);
        assert_eq!(profile.asset_id, asset_id);
        assert_eq!(
            profile.canonical_preimage(),
            vector["route_profile_preimage"]
        );
        assert_eq!(profile.profile_hash(), vector["route_profile_hash"]);
        assert_eq!(u64::from(profile.route_epoch), vector["route_epoch"]);
        assert_eq!(
            keccak_commitment_hex(&asset_id),
            vector["verifier_asset_id_commitment"]
        );
        assert_eq!(
            keccak_commitment_hex(&profile.profile_hash()),
            vector["verifier_route_profile_hash_commitment"]
        );
    }
    // The rehearsal lane of 2026-11-06, as its genesis file pins it.
    let rehearsal: Genesis = serde_json::from_str(include_str!(
        "../../../docs/review/base-sepolia-lock-rehearsal-20261106/genesis.json"
    ))
    .expect("rehearsal genesis");
    let lane = &rehearsal.external_lanes.as_ref().expect("lanes")[0];
    assert_eq!(lane.policy.chain_id, EXTERNAL_LANE_BASE_SEPOLIA_CHAIN_ID);
    assert_eq!(
        lane.pft_asset_id(&rehearsal.chain_id).expect("asset id"),
        vectors[0]["asset_id"]
    );
}

#[test]
fn external_lane_route_profile_and_asset_id_bind_each_genesis_field() {
    let lane = sample_external_lane("base-erc20", 1_000);
    let asset_id = lane.pft_asset_id("postfiat-local").expect("asset id");
    let hash = lane
        .route_profile("postfiat-local")
        .expect("profile")
        .profile_hash();
    let changes: [fn(&mut GenesisExternalLane); 6] = [
        |lane| lane.id = "base-erc21".to_string(),
        |lane| lane.policy.chain_id = EXTERNAL_LANE_BASE_SEPOLIA_CHAIN_ID,
        |lane| lane.policy.lock_contract_address = format!("0x{}", "1b".repeat(20)),
        |lane| lane.policy.token_address = format!("0x{}", "2c".repeat(20)),
        |lane| lane.policy.lock_runtime_code_hash = format!("0x{}", "4e".repeat(32)),
        |lane| lane.policy.token_runtime_code_hash = format!("0x{}", "3d".repeat(32)),
    ];
    for (index, change) in changes.iter().enumerate() {
        let mut changed = lane.clone();
        change(&mut changed);
        let profile = changed.route_profile("postfiat-local").expect("profile");
        assert_ne!(profile.profile_hash(), hash, "change {index}");
        // The code hashes are route fields, not asset fields.
        assert_eq!(profile.asset_id == asset_id, index >= 4, "change {index}");
    }
    assert_ne!(
        lane.pft_asset_id("postfiat-other").expect("asset id"),
        asset_id
    );
    // Release-side fields are not part of either value.
    let mut caps = lane.clone();
    caps.release_cap_bps = 100;
    caps.policy.exports_enabled = true;
    assert_eq!(
        caps.route_profile("postfiat-local")
            .expect("profile")
            .profile_hash(),
        hash
    );
    assert!(lane.pft_asset_id("bad\nchain").is_err());
}
