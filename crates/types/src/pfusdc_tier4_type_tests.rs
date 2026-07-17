fn tier4_exit_leaf(seed: u8) -> BridgeExitLeafV1 {
    BridgeExitLeafV1 {
        schema: BRIDGE_EXIT_LEAF_SCHEMA_V1.to_string(),
        route_epoch: 7,
        asset_id: format!("{seed:02x}").repeat(48),
        burn_tx_id: "11".repeat(48),
        withdrawal_id: "22".repeat(48),
        source_bucket_id: "33".repeat(48),
        amount_atoms: 1_000_000,
        recipient: "0x4444444444444444444444444444444444444444".to_string(),
        destination_hash: "55".repeat(48),
        evidence_root: "66".repeat(48),
        finalized_height: 1213,
        accepted_receipt_id: "77".repeat(48),
        accepted_receipt_code: BRIDGE_EXIT_ACCEPTED_RECEIPT_CODE.to_string(),
        withdrawal_packet_hash: "88".repeat(48),
        withdrawal_packet_evm_digest: "99".repeat(32),
    }
}

#[test]
fn bridge_exit_leaf_is_canonical_and_every_money_field_is_bound() {
    let leaf = tier4_exit_leaf(0xaa);
    leaf.validate().expect("valid leaf");
    let original = leaf.commitment().expect("leaf commitment");
    assert_eq!(original.len(), 96);

    macro_rules! changed {
        ($field:ident, $value:expr) => {{
            let mut changed = leaf.clone();
            changed.$field = $value;
            assert_ne!(changed.commitment().expect("changed commitment"), original);
        }};
    }
    changed!(route_epoch, 8);
    changed!(asset_id, "ab".repeat(48));
    changed!(burn_tx_id, "12".repeat(48));
    changed!(withdrawal_id, "23".repeat(48));
    changed!(source_bucket_id, "34".repeat(48));
    changed!(amount_atoms, 1_000_001);
    changed!(recipient, "0x4545454545454545454545454545454545454545".to_string());
    changed!(destination_hash, "56".repeat(48));
    changed!(evidence_root, "67".repeat(48));
    changed!(finalized_height, 1214);
    changed!(accepted_receipt_id, "78".repeat(48));
    changed!(withdrawal_packet_hash, "89".repeat(48));
    changed!(withdrawal_packet_evm_digest, "9a".repeat(32));

    let mut rejected = leaf;
    rejected.accepted_receipt_code = "rejected".to_string();
    assert!(rejected.validate().is_err());
}

#[test]
fn bridge_exit_merkle_root_is_ordered_bounded_and_has_fixed_empty_value() {
    let first = tier4_exit_leaf(0xaa);
    let second = tier4_exit_leaf(0xbb);
    let empty = bridge_exit_merkle_root_v1(&[]).expect("empty root");
    assert_eq!(empty, bridge_exit_empty_root_v1());
    assert_eq!(empty.len(), 96);

    let ordered = bridge_exit_merkle_root_v1(&[first.clone(), second.clone()])
        .expect("ordered root");
    let reversed = bridge_exit_merkle_root_v1(&[second, first]).expect("reversed root");
    assert_ne!(ordered, reversed, "leaf order must be consensus-visible");
}

#[test]
fn bridge_exit_merkle_proof_binds_leaf_position_and_every_sibling() {
    let leaves = vec![tier4_exit_leaf(0xaa), tier4_exit_leaf(0xbb), tier4_exit_leaf(0xcc)];
    let root = bridge_exit_merkle_root_v1(&leaves).expect("root");
    let proof = bridge_exit_merkle_proof_v1(&leaves, 2).expect("proof");
    verify_bridge_exit_merkle_proof_v1(&root, &proof).expect("valid proof");

    let mut wrong_index = proof.clone();
    wrong_index.leaf_index = 1;
    assert!(verify_bridge_exit_merkle_proof_v1(&root, &wrong_index).is_err());

    let mut wrong_sibling = proof.clone();
    wrong_sibling.siblings[0] = "ff".repeat(48);
    assert!(verify_bridge_exit_merkle_proof_v1(&root, &wrong_sibling).is_err());

    let mut wrong_leaf = proof;
    wrong_leaf.leaf.amount_atoms += 1;
    assert!(verify_bridge_exit_merkle_proof_v1(&root, &wrong_leaf).is_err());
}
