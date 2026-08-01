use postfiat_types::{
    NavReserveTrustCountsV1, NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1,
    NAV_RESERVE_PUBLIC_VALUES_V1_BYTES, NAV_SP1_PROOF_ENCODING_GROTH16,
};

#[test]
fn nav_reserve_proof_status_is_bounded_provider_neutral_and_trust_explicit() {
    let data_dir = unique_test_dir("postfiat-nav-reserve-proof-status");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-reserve-status-test".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init reserve status test");
    let store = NodeStore::new(&data_dir);
    let mut ledger = store.read_ledger().expect("read reserve status ledger");
    let asset_id = "11".repeat(48);
    let profile = NavProofProfile::new(
        "pfissuer",
        NAV_PROFILE_VERIFIER_SP1_GROTH16,
        "manifest-driven-reserves",
        20,
        1,
        100,
        0,
        0,
        0,
        0,
        "22".repeat(32),
        format!("0x{}", "33".repeat(32)),
        NAV_SP1_PROOF_ENCODING_GROTH16,
        1024,
        NAV_RESERVE_PUBLIC_VALUES_V1_BYTES as u64,
    )
    .expect("SP1 base profile")
    .with_nav_reserve_bindings(
        NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1,
        "44".repeat(48),
        "55".repeat(48),
        8,
        false,
    )
    .expect("provider-neutral reserve profile");
    ledger.nav_assets.push(
        NavTrackedAsset::new(
            asset_id.clone(),
            "pfissuer",
            "pfoperator",
            profile.profile_id.clone(),
            "USD",
            "pfredeemer",
        )
        .expect("tracked NAV asset"),
    );
    ledger.nav_proof_profiles.push(profile.clone());

    for epoch in 1..=18u64 {
        let packet_hash = format!("{epoch:096x}");
        let mut packet = NavReservePacket::new(
            asset_id.clone(),
            "pfissuer",
            "pfoperator",
            epoch,
            1_000_000,
            100,
            100,
            profile.profile_id.clone(),
            "66".repeat(48),
            "77".repeat(48),
            packet_hash,
        )
        .expect("reserve packet");
        packet.submitted_at_height = epoch;
        packet.public_values_schema = NAV_RESERVE_PUBLIC_VALUES_SCHEMA_V1.to_string();
        packet.source_manifest_hash = profile.source_manifest_hash.clone();
        packet.valuation_unit_id = profile.valuation_unit_id.clone();
        packet.observation_not_before = epoch;
        packet.observation_not_after = epoch;
        packet.proof_verified_net_assets = 90;
        packet.consensus_overlay_value = 10;
        packet.gross_assets = 100;
        packet.total_liabilities = 10;
        packet.cryptographically_verified_value = 60;
        packet.attested_value = 30;
        packet.controlled_value = 0;
        packet.source_count = 2;
        packet.quantity_trust_counts = NavReserveTrustCountsV1 {
            cryptographic: 1,
            attested: 1,
            controlled: 0,
        };
        packet.valuation_trust_counts = NavReserveTrustCountsV1 {
            cryptographic: 0,
            attested: 2,
            controlled: 0,
        };
        packet.quantity_trust_root = "88".repeat(48);
        packet.valuation_trust_root = "99".repeat(48);
        packet.source_disclosure_root = "aa".repeat(48);
        packet.sp1_proof_bytes = vec![1, 2, 3];
        packet.sp1_public_values = vec![4, 5, 6];
        packet.validate().expect("derived reserve packet");
        ledger.nav_reserve_packets.push(packet);
    }
    store.write_ledger(&ledger).expect("write reserve status ledger");
    assert_eq!(
        NodeStore::new(&data_dir)
            .read_ledger()
            .expect("restart-read successor reserve ledger"),
        ledger,
        "all successor profile and packet fields must survive restart"
    );

    let report = nav_reserve_proof_status(NavReserveProofStatusOptions {
        data_dir: data_dir.clone(),
        asset_id: asset_id.clone(),
    })
    .expect("reserve proof status");
    assert_eq!(report.schema, "postfiat.nav_reserve_proof_status.v1");
    assert!(report.found);
    assert_eq!(report.asset_id, asset_id);
    assert_eq!(report.active_profile.as_ref(), Some(&profile));
    assert_eq!(report.packets.len(), 16);
    assert_eq!(report.packets[0].epoch, 18);
    assert_eq!(report.packets[15].epoch, 3);
    assert_eq!(report.packets[0].quantity_trust_counts.cryptographic, 1);
    assert_eq!(report.packets[0].valuation_trust_counts.attested, 2);
    assert_eq!(report.packets[0].controlled_value, 0);
    let encoded = serde_json::to_string(&report).expect("serialize reserve status");
    assert!(!encoded.contains("sp1_proof_bytes"));
    assert!(!encoded.contains("sp1_public_values"));

    let missing = nav_reserve_proof_status(NavReserveProofStatusOptions {
        data_dir: data_dir.clone(),
        asset_id: "ff".repeat(48),
    })
    .expect("missing reserve proof status");
    assert!(!missing.found);
    assert!(missing.active_profile.is_none());
    assert!(missing.packets.is_empty());
    fs::remove_dir_all(data_dir).expect("remove reserve status fixture");
}
