// Reproduction for postfiatorg/postfiatl1v2#193.
//
// `transfer_fee_quote` and `mempool-submit-transfer` take the sequence of the
// quoted transfer from the mempool, so they describe the transfer that runs
// after the sender's pending entries, but they price it against the committed
// ledger: `recipient_exists`, `state_expansion_fee`, `minimum_fee`,
// `sender_balance` and `sender_meets_reserve_after_transfer` ignore the
// pending entries. The other quotes (`asset_fee_quote`, `escrow_fee_quote`,
// `nft_fee_quote`, `offer_fee_quote`), mempool admission and block execution
// all use `ledger_after_executable_mempool`.
//
// The two `#[ignore]` tests state the expected behaviour and fail on main; the
// control test passes on main. Run them with
// `cargo test -p postfiat-node transfer_fee_quote_pending_recipient -- --include-ignored`.
use super::*;

const RECIPIENT: &str = "pf0000000000000000000000000000000000000002";
const SINK: &str = "pf0000000000000000000000000000000000000003";
const AMOUNT: u64 = 1_000;

fn init_devnet(prefix: &str) -> PathBuf {
    let data_dir = unique_test_dir(prefix);
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    data_dir
}

fn submit_faucet_transfer(data_dir: &Path, to: &str, amount: u64) -> io::Result<MempoolEntry> {
    submit_transfer_to_mempool(TransferOptions {
        data_dir: data_dir.to_path_buf(),
        key_file: None,
        to: to.to_string(),
        amount,
    })
}

fn quote_faucet_transfer(data_dir: &Path, from: &str) -> TransferFeeQuoteReport {
    transfer_fee_quote(TransferFeeQuoteOptions {
        data_dir: data_dir.to_path_buf(),
        from: from.to_string(),
        to: RECIPIENT.to_string(),
        amount: AMOUNT,
        sequence: None,
        memo_type: None,
        memo_format: None,
        memo_data: None,
    })
    .expect("transfer fee quote")
}

fn apply_pending(data_dir: &Path, max_transactions: usize) -> Vec<Receipt> {
    let batch_file = data_dir.join(format!("pending-{max_transactions}.batch.json"));
    create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file: batch_file.clone(),
        max_transactions,
    })
    .expect("mempool batch");
    apply_batch(ApplyBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply batch")
}

/// Control: with an empty mempool the quote, the transfer that
/// `mempool-submit-transfer` signs and the receipt agree.
#[test]
fn transfer_fee_quote_matches_the_receipt_with_an_empty_mempool() {
    let data_dir = init_devnet("postfiat-transfer-fee-quote-control");
    let faucet = read_transfer_key_file(&data_dir, None).expect("faucet key");

    let quote = quote_faucet_transfer(&data_dir, &faucet.address);
    assert_eq!(quote.sequence, 1);
    assert_eq!(quote.mempool_pending_for_sender, 0);
    assert!(!quote.recipient_exists);
    assert!(quote.will_create_recipient_account);
    assert_eq!(quote.state_expansion_fee, TRANSFER_ACCOUNT_CREATION_FEE);

    let pending = submit_faucet_transfer(&data_dir, RECIPIENT, AMOUNT).expect("pending transfer");
    assert_eq!(pending.transfer.unsigned.sequence, 1);
    assert_eq!(pending.transfer.unsigned.fee, quote.minimum_fee);

    let receipts = apply_pending(&data_dir, 1);
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    let receipt = &receipts[0];
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(receipt.fee_charged, quote.minimum_fee);
    assert_eq!(
        (receipt.state_expansion_fee, receipt.minimum_fee),
        (quote.state_expansion_fee, quote.minimum_fee),
        "quote for sequence 1 vs its receipt"
    );

    fs::remove_dir_all(data_dir).expect("cleanup");
}

/// A pending transfer creates the recipient. The quote for the next sequence
/// still prices the account creation, and `mempool-submit-transfer` declares
/// that fee; the receipt shows the node required `TRANSFER_ACCOUNT_CREATION_FEE`
/// less.
#[test]
#[ignore = "reproduction for #193: the quote prices a recipient a pending transfer already creates"]
fn transfer_fee_quote_sees_the_recipient_a_pending_transfer_creates() {
    let data_dir = init_devnet("postfiat-transfer-fee-quote-pending-recipient");
    let faucet = read_transfer_key_file(&data_dir, None).expect("faucet key");

    let first = submit_faucet_transfer(&data_dir, RECIPIENT, AMOUNT).expect("first pending transfer");
    assert_eq!(first.transfer.unsigned.sequence, 1);

    let quote = quote_faucet_transfer(&data_dir, &faucet.address);
    assert_eq!(quote.sequence, 2, "the quote takes its sequence from the mempool");
    assert_eq!(quote.mempool_pending_for_sender, 1);

    let second = submit_faucet_transfer(&data_dir, RECIPIENT, AMOUNT).expect("second pending transfer");
    assert_eq!(second.transfer.unsigned.sequence, 2);

    let receipts = apply_pending(&data_dir, 2);
    assert_eq!(receipts.len(), 2, "{receipts:?}");
    assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
    let receipt = &receipts[1];
    assert_eq!(receipt.tx_id, second.tx_id);
    assert_eq!(receipt.state_expansion_fee, 0, "sequence 1 created the recipient");

    // Expected: the quote for sequence 2 describes the ledger sequence 2 runs on.
    assert_eq!(
        (quote.state_expansion_fee, quote.minimum_fee),
        (receipt.state_expansion_fee, receipt.minimum_fee),
        "quote for sequence 2 vs its receipt"
    );
    assert!(quote.recipient_exists);
    assert!(!quote.will_create_recipient_account);
    assert_eq!(
        second.transfer.unsigned.fee, receipt.minimum_fee,
        "mempool-submit-transfer declared more than the receipt's minimum fee"
    );

    fs::remove_dir_all(data_dir).expect("cleanup");
}

/// The quote reports the committed balance and `sender_meets_reserve_after_transfer: true`
/// for a transfer that admission refuses because of the sender's pending debit.
#[test]
#[ignore = "reproduction for #193: the quote ignores the sender's pending debit"]
fn transfer_fee_quote_counts_the_pending_debit_of_the_sender() {
    let data_dir = init_devnet("postfiat-transfer-fee-quote-pending-debit");
    let faucet = read_transfer_key_file(&data_dir, None).expect("faucet key");
    let store = NodeStore::new(&data_dir);

    // Leave the faucet about 2 * AMOUNT + 62 atoms: enough for one transfer
    // of AMOUNT plus its fee (22 + the 10 atom creation fee), not for two.
    let faucet_balance = store
        .read_ledger()
        .expect("ledger")
        .account(&faucet.address)
        .expect("faucet account")
        .balance;
    let drain = submit_faucet_transfer(&data_dir, SINK, faucet_balance - 2 * AMOUNT - 62 - 32)
        .expect("drain transfer");
    let drain_receipts = apply_pending(&data_dir, 1);
    assert!(drain_receipts[0].accepted, "{drain_receipts:?}");
    let remaining = store
        .read_ledger()
        .expect("drained ledger")
        .account(&faucet.address)
        .expect("faucet account")
        .balance;
    assert_eq!(
        remaining,
        2 * AMOUNT + 62 + 32 - drain.transfer.unsigned.fee,
        "faucet balance after the drain"
    );
    // One transfer of AMOUNT with the 32 atom fee fits; a second one does not,
    // even at the 22 atom fee of an existing recipient.
    assert!(remaining >= AMOUNT + 32 + ACCOUNT_RESERVE, "one transfer must fit");
    assert!(remaining < 2 * AMOUNT + 32 + 22 + ACCOUNT_RESERVE, "two must not");

    let first = submit_faucet_transfer(&data_dir, RECIPIENT, AMOUNT).expect("first pending transfer");
    assert_eq!(first.transfer.unsigned.sequence, 2);

    let quote = quote_faucet_transfer(&data_dir, &faucet.address);
    assert_eq!(quote.sequence, 3, "the quote takes its sequence from the mempool");
    assert_eq!(quote.mempool_pending_for_sender, 1);

    let refused = submit_faucet_transfer(&data_dir, RECIPIENT, AMOUNT)
        .expect_err("admission refuses the second transfer");
    assert!(
        refused.to_string().contains("insufficient_funds"),
        "{refused}"
    );

    // Expected: the quote agrees with admission.
    assert!(
        !quote.sender_meets_reserve_after_transfer,
        "quote: sender_balance {} after {:?}, meets reserve; admission: {refused}",
        quote.sender_balance, quote.sender_balance_after_amount_and_fee
    );

    fs::remove_dir_all(data_dir).expect("cleanup");
}
