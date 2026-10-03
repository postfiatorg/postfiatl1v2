use super::*;

fn account_tx_window(data_dir: &Path, address: &str, limit: usize) -> AccountTxReport {
    account_tx(AccountTxQueryOptions {
        data_dir: data_dir.to_path_buf(),
        address: address.to_string(),
        from_height: Some(1),
        to_height: Some(1),
        limit: Some(limit),
    })
    .expect("account tx window")
}

fn assert_truncation_matches_omitted_rows(data_dir: &Path, sender: &str, recipient: &str) {
    // The recipient has exactly one row: a limit of one returns all of it.
    let exact = account_tx_window(data_dir, recipient, 1);
    assert_eq!(exact.row_count, 1);
    assert!(!exact.truncated, "exact-limit window reported truncated");

    // The sender has two rows: a limit of one omits one, a limit of two none.
    let short = account_tx_window(data_dir, sender, 1);
    assert_eq!(short.row_count, 1);
    assert!(short.truncated, "omitted row was not reported");
    let full = account_tx_window(data_dir, sender, 2);
    assert_eq!(full.row_count, 2);
    assert!(!full.truncated, "exact-limit window reported truncated");
}

#[test]
fn account_tx_truncated_only_when_rows_are_omitted() {
    let data_dir = unique_test_dir("postfiat-account-tx-truncation");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    run_once(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("run once");
    let mut pending = Vec::new();
    for to in [
        "pfrecipient000000000000000000000000000001",
        "pfrecipient000000000000000000000000000002",
    ] {
        pending.push(
            submit_transfer_to_mempool(TransferOptions {
                data_dir: data_dir.clone(),
                key_file: None,
                to: to.to_string(),
                amount: ACCOUNT_RESERVE,
            })
            .expect("submit transfer"),
        );
    }
    let batch_file = data_dir.join("mempool-batch.json");
    create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 10,
    })
    .expect("create mempool batch");
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply batch");
    assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
    let sender = pending[0].transfer.unsigned.from.clone();
    let recipient = pending[1].transfer.unsigned.to.clone();

    assert!(!account_tx_window(&data_dir, &recipient, 1).index_used);
    assert_truncation_matches_omitted_rows(&data_dir, &sender, &recipient);

    rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("build account tx index");
    assert!(account_tx_window(&data_dir, &recipient, 1).index_used);
    assert_truncation_matches_omitted_rows(&data_dir, &sender, &recipient);

    fs::remove_dir_all(data_dir).expect("remove test data dir");
}
