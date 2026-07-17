#[test]
fn atomic_swap_batch_fields_preserve_legacy_json_when_empty() {
    let batch = TransactionBatch::new("legacy-batch", Vec::new());
    assert_eq!(
        serde_json::to_string(&batch).expect("serialize legacy batch"),
        r#"{"batch_id":"legacy-batch","transactions":[]}"#
    );
    let parsed: TransactionBatch =
        serde_json::from_str(r#"{"batch_id":"legacy-batch","transactions":[]}"#)
            .expect("parse legacy batch");
    assert!(parsed.atomic_swap_transactions.is_empty());

    let mempool = MempoolState::empty();
    assert_eq!(
        serde_json::to_string(&mempool).expect("serialize legacy mempool"),
        r#"{"pending":[]}"#
    );
    let parsed: MempoolState =
        serde_json::from_str(r#"{"pending":[]}"#).expect("parse legacy mempool");
    assert!(parsed.pending_atomic_swaps.is_empty());
}

#[test]
fn atomic_swap_batch_and_mempool_bind_both_owner_sequences() {
    let transaction = atomic_swap_fixture();
    let owner_0 = transaction.unsigned.leg_0.owner.clone();
    let owner_1 = transaction.unsigned.leg_1.owner.clone();
    let sequence_0 = transaction.unsigned.leg_0.sequence;
    let sequence_1 = transaction.unsigned.leg_1.sequence;
    let batch = TransactionBatch::new_with_atomic_swap_transactions(
        "atomic-batch",
        Vec::new(),
        Vec::new(),
        Vec::new(),
        vec![transaction.clone()],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    );
    assert_eq!(batch.transaction_count(), 1);
    assert!(!batch.is_empty());
    let json = serde_json::to_string(&batch).expect("serialize atomic batch");
    assert!(json.contains(r#""atomic_swap_transactions""#));

    let mut mempool = MempoolState::empty();
    mempool
        .pending_atomic_swaps
        .push(MempoolAtomicSwapEntry::new("atomic-tx", transaction));
    assert_eq!(mempool.len(), 1);
    assert!(mempool.has_sender_sequence(&owner_0, sequence_0));
    assert!(mempool.has_sender_sequence(&owner_1, sequence_1));
    assert!(!mempool.has_sender_sequence(&owner_0, sequence_0 + 1));
    assert!(!mempool.has_sender_sequence(&owner_1, sequence_1 + 1));
}
