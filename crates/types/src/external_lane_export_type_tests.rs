// `external_lane_export_v1` accounting on the lane record (design
// `docs/specs/pft-erc20-base-bridge-phase3-export-design-20261105.md`
// sections 4 and 7).

/// A lane of 100 atoms whose window may release all of them, with exports.
fn export_test_lane() -> GenesisExternalLane {
    let mut lane = sample_external_lane("base-erc20", 100);
    lane.release_window_blocks = 100;
    lane.release_cap_bps = 10_000;
    lane.policy.exports_enabled = true;
    lane
}

fn sample_export(nonce: u64, amount_atoms: u64, height: u64) -> ExternalLaneExportPacket {
    ExternalLaneExportPacket {
        lane_id: "base-erc20".to_string(),
        export_nonce: nonce,
        sender: "pftl-sender".to_string(),
        base_recipient: format!("0x{}", "2c".repeat(20)),
        amount_atoms,
        exported_at_height: height,
        tx_id: format!("tx-{nonce}"),
    }
}

fn released_export_lane(released: u64) -> (GenesisExternalLane, ExternalLaneState) {
    let lane = export_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    state
        .apply_release(&lane, sample_release(0, released, 1))
        .expect("release");
    (lane, state)
}

#[test]
fn external_lane_export_with_no_releases_yet_is_refused() {
    let lane = export_test_lane();
    let mut state = ExternalLaneState::genesis(&lane);
    let before = state.clone();
    let (code, _) = state.apply_export(&lane, sample_export(0, 1, 2)).unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_EXPORT_ABOVE_RELEASED);
    assert_eq!(state, before);
}

#[test]
fn external_lane_export_of_exactly_released_minus_exported_is_accepted() {
    let (lane, mut state) = released_export_lane(60);
    state.apply_export(&lane, sample_export(0, 25, 2)).expect("first export");
    state
        .apply_export(&lane, sample_export(1, 35, 3))
        .expect("export of exactly released - exported");
    assert_eq!((state.exported_atoms, state.next_export_nonce), (60, 2));
    assert_eq!(state.remainder_atoms().expect("remainder"), 100);
    state.validate_against_genesis(&lane).expect("recorded exports");
}

#[test]
fn external_lane_export_one_atom_above_released_minus_exported_is_refused() {
    let (lane, mut state) = released_export_lane(60);
    state.apply_export(&lane, sample_export(0, 25, 2)).expect("first export");
    let before = state.clone();
    let (code, _) = state.apply_export(&lane, sample_export(1, 36, 3)).unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_EXPORT_ABOVE_RELEASED);
    assert_eq!(state, before);
}

#[test]
fn external_lane_release_after_a_full_cycle_has_the_right_remainder() {
    // Reserved 100, released 100, exported 100: the next 1-atom release
    // would underflow in the form reserved - released + exported.
    let (lane, mut state) = released_export_lane(100);
    state.apply_export(&lane, sample_export(0, 100, 2)).expect("full export");
    assert_eq!(state.remainder_atoms().expect("remainder"), 100);
    state
        .apply_release(&lane, sample_release(1, 1, 150))
        .expect("release after a full cycle");
    assert_eq!(state.released_atoms, 101);
    assert_eq!(state.remainder_atoms().expect("remainder"), 99);
    // The net form still bounds a release by the reserve.
    let (code, _) = state
        .check_release(&lane, 2, 100, 150)
        .unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_REMAINDER_EXCEEDED);
    state.validate_against_genesis(&lane).expect("cycle replays");
}

#[test]
fn external_lane_export_of_u64_max_is_refused_without_overflow() {
    let (lane, mut state) = released_export_lane(60);
    let before = state.clone();
    let (code, _) = state
        .apply_export(&lane, sample_export(0, u64::MAX, 2))
        .unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_EXPORT_ABOVE_RELEASED);
    assert_eq!(state, before);
    // A release counter at the top of u64 refuses rather than wraps.
    state.released_atoms = u64::MAX;
    state.exported_atoms = u64::MAX - 1;
    let (code, _) = state.check_release(&lane, 9, 2, 2).unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_COUNTER_OVERFLOW);
}

#[test]
fn external_lane_export_with_an_exhausted_nonce_is_refused() {
    let (lane, mut state) = released_export_lane(60);
    state.next_export_nonce = u64::MAX;
    let before = state.clone();
    let (code, _) = state
        .apply_export(&lane, sample_export(u64::MAX, 1, 2))
        .unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_EXPORT_NONCE_EXHAUSTED);
    assert_eq!(state, before);
}

#[test]
fn external_lane_exported_above_released_is_a_corrupt_state() {
    let (lane, mut state) = released_export_lane(10);
    state.exported_atoms = 11;
    assert!(state.remainder_atoms().is_err());
    assert!(state.validate().is_err());
    let (code, _) = state.check_export(&lane, 1, 2).unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_STATE_CORRUPT);
    let (code, _) = state.check_release(&lane, 1, 1, 2).unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_STATE_CORRUPT);
}

#[test]
fn external_lane_commitment_changes_with_exported_atoms_only() {
    let (lane, mut state) = released_export_lane(60);
    let unexported = state.state_commitment_bytes().expect("commit");
    // A lane without exports commits no export field.
    assert!(!String::from_utf8(unexported.clone()).expect("utf8").contains("export"));
    assert_eq!(state.state_commitment_bytes().expect("commit"), unexported);
    state.apply_export(&lane, sample_export(0, 5, 2)).expect("export");
    let exported = state.state_commitment_bytes().expect("commit");
    assert_ne!(exported, unexported);
    assert_eq!(state.state_commitment_bytes().expect("commit"), exported);
    // The packets are not in the bytes.
    let mut without_packets = state.clone();
    without_packets.exports.clear();
    assert_eq!(without_packets.state_commitment_bytes().expect("commit"), exported);
    state.exported_atoms = 6;
    assert_ne!(state.state_commitment_bytes().expect("commit"), exported);
}

#[test]
fn external_lane_export_packet_matches_the_shared_vectors() {
    let vectors: serde_json::Value = serde_json::from_str(include_str!(
        "../testdata/external-lane-export-packet-vectors.json"
    ))
    .expect("vector file");
    let hex = |digest: [u8; 48]| lane_digest_hex(digest);
    for vector in vectors["vectors"].as_array().expect("vectors") {
        let packet: ExternalLaneExportPacket =
            serde_json::from_value(vector["packet"].clone()).expect("packet");
        let chain_id = vector["chain_id"].as_str().expect("chain id");
        assert_eq!(hex(packet.digest().expect("digest")), vector["packet_digest"]);
        assert_eq!(
            hex(packet.withdrawal_id_commitment(chain_id).expect("withdrawal id")),
            vector["withdrawal_id_commitment"]
        );
        assert_eq!(hex(packet.burn_tx_commitment(chain_id)), vector["burn_tx_commitment"]);
        assert_eq!(
            external_lane_export_history_root(None, &packet).expect("root"),
            vector["export_history_root_from_empty"]
        );
    }
    for (field, domain) in [
        ("packet", EXTERNAL_LANE_EXPORT_PACKET_DOMAIN),
        ("history", EXTERNAL_LANE_EXPORT_HISTORY_DOMAIN),
        ("withdrawal_id", EXTERNAL_LANE_EXPORT_WITHDRAWAL_ID_DOMAIN),
        ("burn_tx", EXTERNAL_LANE_EXPORT_BURN_TX_DOMAIN),
    ] {
        assert_eq!(vectors["domains"][field], std::str::from_utf8(domain).expect("tag"));
    }
}

#[test]
fn external_lane_policy_reads_an_absent_export_switch_as_false() {
    let mut lane = sample_external_lane("base-erc20", 1_000);
    let json = serde_json::to_string(&lane).expect("lane json");
    assert!(!json.contains("exports_enabled"), "false is not encoded");
    let parsed: GenesisExternalLane = serde_json::from_str(&json).expect("parse");
    assert!(!parsed.policy.exports_enabled);
    let digest = lane.route_config_digest().expect("digest");
    lane.policy.exports_enabled = true;
    let json = serde_json::to_string(&lane).expect("lane json");
    assert!(json.contains("\"exports_enabled\":true"));
    let parsed: GenesisExternalLane = serde_json::from_str(&json).expect("parse");
    assert!(parsed.policy.exports_enabled);
    assert_ne!(lane.route_config_digest().expect("digest"), digest);
}

type ExportTamper = (&'static str, fn(&mut ExternalLaneState));

#[test]
fn external_lane_record_refuses_exports_genesis_did_not_enable_and_tampered_packets() {
    let (lane, mut state) = released_export_lane(60);
    state.apply_export(&lane, sample_export(0, 5, 2)).expect("export");
    state.apply_export(&lane, sample_export(1, 7, 3)).expect("export");
    state.validate_against_genesis(&lane).expect("recorded exports");
    let mut disabled = lane.clone();
    disabled.policy.exports_enabled = false;
    assert!(state
        .validate_against_genesis(&disabled)
        .unwrap_err()
        .contains("does not enable"));
    let tampers: [ExportTamper; 4] = [
        ("atoms", |state| state.exported_atoms += 1),
        ("nonce", |state| state.next_export_nonce += 1),
        ("amount", |state| state.exports[1].amount_atoms += 1),
        ("removed", |state| {
            state.exports.remove(0);
        }),
    ];
    for (label, tamper) in tampers {
        let mut tampered = state.clone();
        tamper(&mut tampered);
        assert!(tampered.validate_against_genesis(&lane).is_err(), "{label}");
    }
}

#[test]
fn external_lane_export_packets_prune_and_bound_like_release_records() {
    let (lane, mut state) = released_export_lane(60);
    // Window of 100 blocks, one retained window: the packet at height 2 is
    // prunable from height 102 in window 1.
    state.apply_export(&lane, sample_export(0, 1, 2)).expect("export");
    state.apply_export(&lane, sample_export(1, 1, 101)).expect("export");
    assert_eq!(state.exports.len(), 2, "height 101 keeps the packet at 2");
    state.apply_export(&lane, sample_export(2, 1, 102)).expect("export");
    assert_eq!(state.pruned_export_count, 1);
    assert_eq!(state.pruned_exported_atoms, 1);
    assert_eq!(state.export_count(), 3);
    assert_eq!(state.exports[0].export_nonce, 1);
    state.validate_against_genesis(&lane).expect("pruned exports");
    // The stored ceiling refuses without mutation.
    let before = state.clone();
    let (code, _) = state
        .apply_export_within(&lane, sample_export(3, 1, 103), 2)
        .unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_EXPORT_RECORDS_FULL);
    assert_eq!(state, before);
    // A packet must take the lane's next nonce.
    let (code, _) = state.apply_export(&lane, sample_export(7, 1, 103)).unwrap_err();
    assert_eq!(code, EXTERNAL_LANE_STATE_CORRUPT);
    assert_eq!(state, before);
}
