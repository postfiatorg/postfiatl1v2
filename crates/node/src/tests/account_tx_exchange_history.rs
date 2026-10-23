//! Exchange-facing account history: `payment_v2` memo contents and
//! shielded-withdrawal credits through `tx`, the `account_tx` archive scan,
//! the disk index, and the index rebuild that backfills older indexes.

use super::*;

const DEPOSIT_TAG_TEXT: &str = "customer-4242";
const NON_UTF8_MEMO_HEX: &str = "ff00fe";
const PAYMENT_RECIPIENT: &str = "pf0000000000000000000000000000000000000001";
const WITHDRAW_RECIPIENT: &str = "pf0000000000000000000000000000000000000002";
const EGRESS_RECIPIENT: &str = "pf0000000000000000000000000000000000000003";
const WITHDRAW_AMOUNT: u64 = 2_500_000;
const WITHDRAW_FEE: u64 = 7;
const EGRESS_AMOUNT: u64 = 900;

fn text_hex(text: &str) -> String {
    bytes_to_hex(text.as_bytes())
}

/// A deposit-tag memo (all three fields UTF-8) and a binary memo whose data is
/// not UTF-8 and whose type and format are absent.
fn exchange_memos() -> Vec<PaymentMemo> {
    vec![
        PaymentMemo {
            memo_type: text_hex("deposit_tag"),
            memo_format: text_hex("text/plain"),
            memo_data: text_hex(DEPOSIT_TAG_TEXT),
        },
        PaymentMemo {
            memo_type: String::new(),
            memo_format: String::new(),
            memo_data: NON_UTF8_MEMO_HEX.to_string(),
        },
    ]
}

fn signed_payment_v2_from_faucet(data_dir: &Path, memos: Vec<PaymentMemo>) -> SignedPaymentV2 {
    let store = NodeStore::new(data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let ledger = store.read_ledger().expect("ledger");
    let key_file = read_transfer_key_file(data_dir, None).expect("faucet key");
    let sequence = ledger
        .account(&key_file.address)
        .expect("faucet account")
        .sequence
        + 1;
    let private_key = hex_to_bytes(&key_file.private_key_hex).expect("private key bytes");
    let mut fee = MIN_TRANSFER_FEE;
    loop {
        let unsigned = UnsignedPaymentV2 {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
            transaction_kind: PAYMENT_V2_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            from: key_file.address.clone(),
            to: PAYMENT_RECIPIENT.to_string(),
            amount: ACCOUNT_RESERVE,
            fee,
            sequence,
            memos: memos.clone(),
        };
        let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign");
        let signed = SignedPaymentV2 {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: key_file.public_key_hex.clone(),
            signature_hex: bytes_to_hex(&signature),
        };
        let minimum = postfiat_execution::minimum_payment_v2_fee_for_ledger(&ledger, &signed);
        if fee >= minimum {
            signed.validate().expect("signed payment validates");
            return signed;
        }
        fee = minimum;
    }
}

/// Height 1: one finalized `payment_v2` from the faucet carrying
/// `exchange_memos()`.
fn payment_v2_memo_chain(label: &str) -> (PathBuf, SignedPaymentV2, Receipt) {
    let data_dir = unique_test_dir(label);
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    let signed = signed_payment_v2_from_faucet(&data_dir, exchange_memos());
    submit_signed_payment_v2_json_to_mempool(SignedPaymentV2JsonSubmitOptions {
        data_dir: data_dir.clone(),
        signed_payment_v2_json: serde_json::to_string(&signed).expect("payment json"),
    })
    .expect("admit payment v2");
    let batch_file = data_dir.join("payment-v2-batch.json");
    create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 10,
    })
    .expect("create payment v2 batch");
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply payment v2 batch");
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted, "{receipts:?}");
    assert_eq!(receipts[0].tx_id, payment_v2_tx_id(&signed));
    (data_dir, signed, receipts[0].clone())
}

fn assert_exchange_memo_views(row: &AccountTxRow) {
    assert_eq!(row.transaction_kind, PAYMENT_V2_TRANSACTION_KIND);
    // The hash and count fields stay for compatibility next to the contents.
    assert_eq!(row.memo_count, Some(2));
    assert!(row.memo_hash.as_deref().is_some_and(|hash| hash.len() == 96));
    let memos = row.memos.as_ref().expect("payment_v2 row carries memos");
    assert_eq!(
        memos,
        &vec![
            PaymentMemoView {
                memo_type: text_hex("deposit_tag"),
                memo_format: text_hex("text/plain"),
                memo_data: text_hex(DEPOSIT_TAG_TEXT),
                memo_type_text: Some("deposit_tag".to_string()),
                memo_format_text: Some("text/plain".to_string()),
                memo_data_text: Some(DEPOSIT_TAG_TEXT.to_string()),
            },
            PaymentMemoView {
                memo_type: String::new(),
                memo_format: String::new(),
                memo_data: NON_UTF8_MEMO_HEX.to_string(),
                memo_type_text: None,
                memo_format_text: None,
                memo_data_text: None,
            },
        ]
    );
}

fn validate_rpc_result<T: serde::Serialize>(
    result: &T,
    kind: postfiat_rpc_sdk::RpcResponseKind,
) -> serde_json::Value {
    let response = postfiat_rpc_sdk::success_response("exchange-history", result, Vec::new())
        .expect("rpc success response");
    postfiat_rpc_sdk::validate_response_kind(&response, kind).expect("rpc response validates");
    response.result.expect("rpc result")
}

fn history(data_dir: &Path, address: &str) -> AccountTxReport {
    account_tx(AccountTxQueryOptions {
        data_dir: data_dir.to_path_buf(),
        address: address.to_string(),
        from_height: None,
        to_height: None,
        limit: Some(50),
    })
    .expect("account tx")
}

fn finality(data_dir: &Path, tx_id: &str) -> TxFinalityReport {
    tx_finality(TxFinalityQueryOptions {
        data_dir: data_dir.to_path_buf(),
        tx_id: tx_id.to_string(),
        audit_block_log: false,
    })
    .expect("tx finality")
}

#[test]
fn payment_v2_memo_contents_flow_through_tx_scan_and_index() {
    let (data_dir, signed, receipt) = payment_v2_memo_chain("postfiat-account-tx-memo-contents");

    let tx = finality(&data_dir, &receipt.tx_id);
    assert_eq!(tx.transaction_rows.len(), 1);
    let tx_row = &tx.transaction_rows[0];
    assert_eq!(tx_row.tx_id, receipt.tx_id);
    assert_eq!(tx_row.to_address, PAYMENT_RECIPIENT);
    assert_eq!(tx_row.amount, signed.unsigned.amount);
    assert_exchange_memo_views(tx_row);
    let tx_json = validate_rpc_result(&tx, postfiat_rpc_sdk::RpcResponseKind::Tx);
    assert_eq!(
        tx_json["transaction_rows"][0]["memos"][0]["memo_data_text"],
        DEPOSIT_TAG_TEXT
    );
    assert!(tx_json["transaction_rows"][0]["memos"][1]
        .get("memo_data_text")
        .is_none());

    let scan = history(&data_dir, PAYMENT_RECIPIENT);
    assert!(!scan.index_used);
    assert_eq!(scan.rows, tx.transaction_rows);
    rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("build account tx index");
    let indexed = history(&data_dir, PAYMENT_RECIPIENT);
    assert!(indexed.index_used);
    assert_eq!(indexed.rows, scan.rows, "index and scan disagree on memo rows");
    let indexed_json = validate_rpc_result(&indexed, postfiat_rpc_sdk::RpcResponseKind::AccountTx);
    assert_eq!(indexed_json["rows"][0]["memos"][0]["memo_type_text"], "deposit_tag");

    // The sender's history lists the same row, memos included.
    let sender_rows = history(&data_dir, &signed.unsigned.from).rows;
    assert_eq!(sender_rows, indexed.rows);

    fs::remove_dir_all(data_dir).expect("remove test data dir");
}

struct ShieldedWithdrawalBlock {
    height: u64,
    batch_id: String,
    withdraw_tx_id: String,
    egress_tx_id: String,
    rejected_tx_id: String,
    asset_id: String,
}

fn shielded_test_id(label: &str) -> String {
    hash_hex("postfiat.test.account_tx_shielded", label.as_bytes())
}

/// Appends one finalized shielded block to the stored history, as an ordered
/// commit would leave it: the block, its archived `ShieldedActionBatch`, and
/// one receipt per action. Actions: 0 an accepted native PFT
/// `orchard_withdraw_v1` to `WITHDRAW_RECIPIENT`; 1 a shielded-to-shielded
/// `orchard_action_v1` (no transparent effect, no row); 2 an accepted
/// issued-asset `asset_orchard_private_egress_v1` to `EGRESS_RECIPIENT`; 3 a
/// rejected `orchard_withdraw_v1` to `WITHDRAW_RECIPIENT`. The index reads only
/// stored data, so the Orchard proofs are not needed here; the end-to-end
/// Orchard withdraw test checks the same rows against real execution.
fn append_shielded_withdrawal_block(data_dir: &Path) -> ShieldedWithdrawalBlock {
    let store = NodeStore::new(data_dir);
    let tip = store
        .read_blocks()
        .expect("blocks")
        .blocks
        .last()
        .cloned()
        .expect("chain has a block");
    let asset_id = shielded_test_id("issued-asset");
    let withdraw = |amount: u64| {
        ShieldedAction::OrchardWithdrawV1(OrchardWithdrawActionPayload {
            action_json: serde_json::json!({ "pool_id": ORCHARD_DEFAULT_POOL_ID }).to_string(),
            to: WITHDRAW_RECIPIENT.to_string(),
            amount,
            fee: WITHDRAW_FEE,
            policy_id: ORCHARD_WITHDRAW_POLICY_ID.to_string(),
            disclosure_hash: shielded_test_id("disclosure"),
        })
    };
    let egress = ShieldedAction::AssetOrchardPrivateEgressV1(AssetOrchardPrivateEgressActionPayload {
        version: 1,
        schema: "asset-orchard-private-egress-v1".to_string(),
        pool_id: ASSET_ORCHARD_POOL_ID_V1.to_string(),
        to: EGRESS_RECIPIENT.to_string(),
        asset_id: asset_id.clone(),
        amount: EGRESS_AMOUNT,
        fee: 0,
        policy_id: "policy".to_string(),
        disclosure_hash: shielded_test_id("egress-disclosure"),
        proof_system_id: "proof-system".to_string(),
        circuit_id: "circuit".to_string(),
        pool_domain: "domain".to_string(),
        anchor: "00".repeat(32),
        nullifier: "11".repeat(32),
        randomized_verification_key: "22".repeat(32),
        asset_tag_lo: 0,
        asset_tag_hi: 0,
        exit_binding_hash: shielded_test_id("exit-binding"),
        proof: "33".repeat(8),
        spend_authorization_signature: "44".repeat(64),
    });
    let private_transfer = ShieldedAction::OrchardV1(OrchardActionPayload {
        action_json: serde_json::json!({ "pool_id": ORCHARD_DEFAULT_POOL_ID }).to_string(),
    });
    let batch_id = shielded_test_id("batch");
    let batch = ShieldedActionBatch::new(
        batch_id.clone(),
        vec![withdraw(WITHDRAW_AMOUNT), private_transfer, egress, withdraw(WITHDRAW_AMOUNT + 1)],
    );
    let receipts = vec![
        Receipt::accepted(shielded_test_id("withdraw"), "Orchard withdraw applied"),
        Receipt::accepted(shielded_test_id("private-transfer"), "Orchard action applied"),
        Receipt::accepted(shielded_test_id("egress"), "private egress applied"),
        Receipt::rejected(
            shielded_test_id("rejected-withdraw"),
            "orchard_withdraw_exceeds_pool",
            "Orchard withdraw exit exceeds issued pool value",
        ),
    ];
    let mut block = tip.clone();
    block.header.height = tip.header.height + 1;
    block.header.parent_hash = tip.header.block_hash.clone();
    block.header.batch_kind = BATCH_KIND_SHIELDED.to_string();
    block.header.batch_id = batch_id.clone();
    block.header.receipt_count = receipts.len() as u64;
    block.header.block_hash = shielded_test_id("block");
    block.receipt_ids = receipts.iter().map(|receipt| receipt.tx_id.clone()).collect();
    block.fastpay_pre_state_effects = Vec::new();
    store
        .append_batch_archive_entry(BatchArchiveEntry {
            batch_kind: BATCH_KIND_SHIELDED.to_string(),
            batch_id: batch_id.clone(),
            payload_hash: String::new(),
            payload_json: serde_json::to_string(&batch).expect("shielded batch json"),
        })
        .expect("archive shielded batch");
    for receipt in &receipts {
        store.append_receipt(receipt.clone()).expect("append receipt");
    }
    store.append_block(block.clone()).expect("append shielded block");
    assert_eq!(
        store.read_blocks().expect("blocks").blocks.last(),
        Some(&block)
    );
    ShieldedWithdrawalBlock {
        height: block.header.height,
        batch_id,
        withdraw_tx_id: receipts[0].tx_id.clone(),
        egress_tx_id: receipts[2].tx_id.clone(),
        rejected_tx_id: receipts[3].tx_id.clone(),
        asset_id,
    }
}

fn assert_shielded_withdrawal_rows(
    shielded: &ShieldedWithdrawalBlock,
    withdraw_rows: &[AccountTxRow],
    egress_rows: &[AccountTxRow],
) {
    let shielded_withdraw_rows = withdraw_rows
        .iter()
        .filter(|row| row.batch_kind == BATCH_KIND_SHIELDED)
        .collect::<Vec<_>>();
    assert_eq!(shielded_withdraw_rows.len(), 2, "{withdraw_rows:?}");
    let credit = shielded_withdraw_rows[0];
    assert_eq!(credit.tx_id, shielded.withdraw_tx_id);
    assert_eq!(credit.block_height, shielded.height);
    assert_eq!(credit.batch_id, shielded.batch_id);
    assert_eq!(credit.transaction_index, 0);
    assert_eq!(credit.transaction_kind, "orchard_withdraw_v1");
    assert_eq!(
        credit.tx_role.as_deref(),
        Some(ACCOUNT_TX_ROLE_SHIELDED_WITHDRAWAL)
    );
    assert_eq!(credit.from_address, ORCHARD_DEFAULT_POOL_ID);
    assert_eq!(credit.to_address, WITHDRAW_RECIPIENT);
    assert_eq!(credit.amount, WITHDRAW_AMOUNT);
    assert_eq!(credit.fee, WITHDRAW_FEE);
    assert_eq!(credit.sequence, 0);
    assert_eq!(credit.asset_id, None, "native PFT rows omit asset_id");
    assert_eq!(credit.memos, None);
    assert_eq!(credit.accepted, Some(true));
    assert_eq!(credit.receipt_code.as_deref(), Some("accepted"));

    let rejected = shielded_withdraw_rows[1];
    assert_eq!(rejected.tx_id, shielded.rejected_tx_id);
    assert_eq!(rejected.transaction_index, 3);
    assert_eq!(rejected.accepted, Some(false));
    assert_eq!(
        rejected.receipt_code.as_deref(),
        Some("orchard_withdraw_exceeds_pool")
    );

    assert_eq!(egress_rows.len(), 1, "{egress_rows:?}");
    let egress = &egress_rows[0];
    assert_eq!(egress.tx_id, shielded.egress_tx_id);
    assert_eq!(egress.batch_kind, BATCH_KIND_SHIELDED);
    assert_eq!(egress.transaction_index, 2);
    assert_eq!(egress.transaction_kind, "asset_orchard_private_egress_v1");
    assert_eq!(
        egress.tx_role.as_deref(),
        Some(ACCOUNT_TX_ROLE_SHIELDED_WITHDRAWAL)
    );
    assert_eq!(egress.from_address, ASSET_ORCHARD_POOL_ID_V1);
    assert_eq!(egress.to_address, EGRESS_RECIPIENT);
    assert_eq!(egress.amount, EGRESS_AMOUNT);
    assert_eq!(egress.asset_id.as_deref(), Some(shielded.asset_id.as_str()));
    assert_eq!(egress.accepted, Some(true));
}

#[test]
fn account_tx_lists_shielded_withdrawal_credits_through_tx_scan_and_index() {
    let (data_dir, _, _) = payment_v2_memo_chain("postfiat-account-tx-shielded-withdrawal");
    let shielded = append_shielded_withdrawal_block(&data_dir);

    let withdraw_scan = history(&data_dir, WITHDRAW_RECIPIENT);
    let egress_scan = history(&data_dir, EGRESS_RECIPIENT);
    assert!(!withdraw_scan.index_used);
    assert_eq!(withdraw_scan.archive_lookup_count, 2);
    assert_shielded_withdrawal_rows(&shielded, &withdraw_scan.rows, &egress_scan.rows);
    validate_rpc_result(&withdraw_scan, postfiat_rpc_sdk::RpcResponseKind::AccountTx);
    validate_rpc_result(&egress_scan, postfiat_rpc_sdk::RpcResponseKind::AccountTx);

    let build = rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("build account tx index");
    assert!(build.index_usable && build.disk_index_usable);
    assert_eq!(build.indexed_block_count, 2);
    assert_eq!(build.indexed_row_count, 4);
    let withdraw_indexed = history(&data_dir, WITHDRAW_RECIPIENT);
    let egress_indexed = history(&data_dir, EGRESS_RECIPIENT);
    assert!(withdraw_indexed.index_used && egress_indexed.index_used);
    assert_eq!(withdraw_indexed.rows, withdraw_scan.rows);
    assert_eq!(egress_indexed.rows, egress_scan.rows);
    validate_rpc_result(&withdraw_indexed, postfiat_rpc_sdk::RpcResponseKind::AccountTx);

    let tx = finality(&data_dir, &shielded.withdraw_tx_id);
    assert_eq!(tx.block.header.batch_kind, BATCH_KIND_SHIELDED);
    assert_eq!(tx.transaction_rows, vec![withdraw_scan.rows[0].clone()]);
    let tx_json = validate_rpc_result(&tx, postfiat_rpc_sdk::RpcResponseKind::Tx);
    assert_eq!(tx_json["transaction_rows"][0]["tx_role"], "shielded_withdrawal");
    // The shielded-to-shielded action is in the block but affects no
    // transparent account, so its `tx` carries no rows.
    assert!(finality(&data_dir, &shielded_test_id("private-transfer"))
        .transaction_rows
        .is_empty());

    fs::remove_dir_all(data_dir).expect("remove test data dir");
}

#[test]
fn account_tx_index_rebuild_backfills_memos_and_shielded_withdrawals_over_a_v1_index() {
    let (data_dir, _, receipt) = payment_v2_memo_chain("postfiat-account-tx-v1-backfill");
    let shielded = append_shielded_withdrawal_block(&data_dir);
    rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("build account tx index");
    let expected_payment_rows = history(&data_dir, PAYMENT_RECIPIENT).rows;
    let expected_withdraw_rows = history(&data_dir, WITHDRAW_RECIPIENT).rows;
    let expected_egress_rows = history(&data_dir, EGRESS_RECIPIENT).rows;

    // Rewrite the cache as the pre-upgrade binary left it at the same tip:
    // v1 schemas, no memo contents, and no shielded rows.
    let index_path = account_tx_index_path(&data_dir);
    let mut legacy = read_account_tx_index_file(&index_path).expect("read index");
    legacy.schema = "postfiat-account-tx-index-v1".to_string();
    for rows in legacy.accounts.values_mut() {
        rows.retain(|row| row.batch_kind != BATCH_KIND_SHIELDED);
        for row in rows.iter_mut() {
            row.memos = None;
        }
    }
    legacy.accounts.retain(|_, rows| !rows.is_empty());
    legacy.account_count = legacy.accounts.len() as u64;
    legacy.indexed_row_count = 1;
    write_account_tx_index_file(&index_path, &legacy).expect("write v1 index");
    let disk_path = account_tx_disk_index_path(&data_dir);
    let mut legacy_disk = read_account_tx_disk_index_file(&disk_path).expect("read disk index");
    legacy_disk.schema = "postfiat-account-tx-disk-index-v1".to_string();
    write_account_tx_disk_index_file(&disk_path, &legacy_disk).expect("write v1 disk index");

    let status = account_tx_index_status(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("index status");
    assert!(status.index_present && !status.index_usable);
    assert_eq!(
        status.reason.as_deref(),
        Some("unsupported account_tx index schema `postfiat-account-tx-index-v1`")
    );
    assert!(status.disk_index_present && !status.disk_index_usable);

    // Until the rebuild runs, reads fall back to the archive scan, which
    // already returns the v2 rows.
    let before_rebuild = history(&data_dir, WITHDRAW_RECIPIENT);
    assert!(!before_rebuild.index_used);
    assert_eq!(before_rebuild.rows, expected_withdraw_rows);

    // The rebuild does not extend a v1 index: it re-reads every retained
    // block, so historical memos and shielded withdrawals are backfilled.
    let build = rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("rebuild over v1 index");
    assert!(build.index_usable && build.disk_index_usable);
    assert_eq!(build.indexed_row_count, 4);
    let payment = history(&data_dir, PAYMENT_RECIPIENT);
    let withdraw = history(&data_dir, WITHDRAW_RECIPIENT);
    let egress = history(&data_dir, EGRESS_RECIPIENT);
    assert!(payment.index_used && withdraw.index_used && egress.index_used);
    assert_eq!(payment.rows, expected_payment_rows);
    assert_eq!(payment.rows[0].tx_id, receipt.tx_id);
    assert_exchange_memo_views(&payment.rows[0]);
    assert_eq!(withdraw.rows, expected_withdraw_rows);
    assert_eq!(egress.rows, expected_egress_rows);
    assert_shielded_withdrawal_rows(&shielded, &withdraw.rows, &egress.rows);
    let rebuilt = read_account_tx_index_file(&index_path).expect("read rebuilt index");
    assert_eq!(rebuilt.schema, "postfiat-account-tx-index-v2");

    fs::remove_dir_all(data_dir).expect("remove test data dir");
}

/// Appends a block of `batch_kind` that links `receipt` (appended to the
/// receipt log) and reuses `batch_id`'s archive entry when it has one.
fn append_block_linking_receipt(
    data_dir: &Path,
    batch_kind: &str,
    batch_id: &str,
    receipt: Receipt,
    label: &str,
) -> u64 {
    let store = NodeStore::new(data_dir);
    let tip = store
        .read_blocks()
        .expect("blocks")
        .blocks
        .last()
        .cloned()
        .expect("chain has a block");
    let mut block = tip.clone();
    block.header.height = tip.header.height + 1;
    block.header.parent_hash = tip.header.block_hash.clone();
    block.header.batch_kind = batch_kind.to_string();
    block.header.batch_id = batch_id.to_string();
    block.header.receipt_count = 1;
    block.header.block_hash = shielded_test_id(label);
    block.receipt_ids = vec![receipt.tx_id.clone()];
    block.fastpay_pre_state_effects = Vec::new();
    store.append_receipt(receipt).expect("append receipt");
    store.append_block(block.clone()).expect("append block");
    block.header.height
}

// A governance receipt's tx_id is the amendment id, so resubmitting the same
// signed amendment in a later governance batch repeats it in the receipt log
// (postfiat-wan-devnet-2: heights 64/66 and 940/941/945). The history index,
// the archive scan and `tx` must accept exactly that repeat.
#[test]
fn repeated_governance_receipt_ids_build_the_index_and_answer_account_tx_and_tx() {
    let (data_dir, signed, receipt) = payment_v2_memo_chain("repeated-governance-receipt");
    let amendment_id = shielded_test_id("repeated-amendment");
    let rejected = Receipt::rejected(
        amendment_id.clone(),
        "vault_bridge_route_profile_rejected",
        "vault bridge route does not exactly match the active NAV proof profile",
    );
    let accepted = Receipt::accepted(amendment_id.clone(), "vault bridge route profile activated");
    append_block_linking_receipt(
        &data_dir,
        BATCH_KIND_GOVERNANCE,
        &shielded_test_id("governance-batch-1"),
        rejected.clone(),
        "governance-block-1",
    );
    let second = append_block_linking_receipt(
        &data_dir,
        BATCH_KIND_GOVERNANCE,
        &shielded_test_id("governance-batch-2"),
        accepted.clone(),
        "governance-block-2",
    );
    // The transactional store keeps one receipt per linking block
    // (`receipts_in_block_order`); write that receipt log.
    let store = NodeStore::new(&data_dir);
    store
        .write_receipts(&[receipt.clone(), rejected.clone(), accepted.clone()])
        .expect("write block-ordered receipt log");
    let receipts = store.read_receipts().expect("receipts");
    assert_eq!(receipts.iter().filter(|r| r.tx_id == amendment_id).count(), 2);

    // Archive scan, then the index build and indexed reads.
    let scanned = history(&data_dir, PAYMENT_RECIPIENT);
    assert!(!scanned.index_used);
    assert_eq!(scanned.rows.len(), 1);
    assert_eq!(scanned.rows[0].tx_id, receipt.tx_id);
    assert_exchange_memo_views(&scanned.rows[0]);
    let build = rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .expect("index build with repeated governance receipt ids");
    assert!(build.index_usable && build.disk_index_usable);
    let indexed = history(&data_dir, PAYMENT_RECIPIENT);
    assert!(indexed.index_used);
    assert_eq!(indexed.rows, scanned.rows);
    let sender = history(&data_dir, &signed.unsigned.from);
    assert!(sender.index_used);
    assert_eq!(sender.rows.len(), 1);

    // `tx` of the repeated id answers with its latest receipt and block; the
    // payment's own `tx` is unchanged.
    let governance = finality(&data_dir, &amendment_id);
    assert_eq!(governance.receipt, accepted);
    assert_eq!(governance.block.header.height, second);
    assert_eq!(governance.receipt_index, 0);
    let payment = finality(&data_dir, &receipt.tx_id);
    assert_eq!(payment.receipt, receipt);
    assert_eq!(payment.transaction_rows.len(), 1);

    // The legacy receipt log keeps one terminal receipt per id
    // (`merge_appended_receipt`); `tx` answers the same way from it.
    store
        .write_receipts(&[receipt.clone(), accepted.clone()])
        .expect("write terminal receipt log");
    let terminal = finality(&data_dir, &amendment_id);
    assert_eq!(terminal.receipt, accepted);
    assert_eq!(terminal.block.header.height, second);
    assert_eq!(history(&data_dir, PAYMENT_RECIPIENT).rows, scanned.rows);

    fs::remove_dir_all(data_dir).expect("remove test data dir");
}

// An ordinary transaction's tx_id must stay unique in the receipt log; a
// repeat (here the payment linked again by a later transparent block, and an
// id linked by one governance and one transparent block) is still refused.
#[test]
fn repeated_ordinary_receipt_ids_are_still_rejected() {
    let (data_dir, _signed, receipt) = payment_v2_memo_chain("repeated-ordinary-receipt");
    let payment_block = NodeStore::new(&data_dir)
        .read_blocks()
        .expect("blocks")
        .blocks
        .last()
        .cloned()
        .expect("payment block");
    append_block_linking_receipt(
        &data_dir,
        BATCH_KIND_TRANSPARENT,
        &payment_block.header.batch_id,
        receipt.clone(),
        "repeated-transparent-block",
    );
    NodeStore::new(&data_dir)
        .write_receipts(&[receipt.clone(), receipt.clone()])
        .expect("write block-ordered receipt log");
    let duplicate = |error: io::Error| error.to_string();
    let scan = account_tx(AccountTxQueryOptions {
        data_dir: data_dir.clone(),
        address: PAYMENT_RECIPIENT.to_string(),
        from_height: None,
        to_height: None,
        limit: Some(50),
    })
    .map_err(duplicate)
    .expect_err("archive scan refuses a repeated ordinary tx_id");
    assert!(scan.contains("receipt log contains duplicate tx_id"), "{scan}");
    let build = rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(duplicate)
    .expect_err("index build refuses a repeated ordinary tx_id");
    assert!(build.contains("receipt log contains duplicate tx_id"), "{build}");
    let tx = tx_finality(TxFinalityQueryOptions {
        data_dir: data_dir.clone(),
        tx_id: receipt.tx_id.clone(),
        audit_block_log: false,
    })
    .map_err(duplicate)
    .expect_err("tx refuses a repeated ordinary tx_id");
    assert!(tx.contains("has multiple receipts"), "{tx}");
    fs::remove_dir_all(&data_dir).expect("remove test data dir");

    // Mixed: one governance link and one transparent link of the same id.
    let (data_dir, _signed, payment) = payment_v2_memo_chain("repeated-mixed-receipt");
    let mixed_id = shielded_test_id("mixed-id");
    let governance = Receipt::accepted(mixed_id.clone(), "vault bridge route profile activated");
    let transparent = Receipt::accepted(mixed_id, "accepted");
    append_block_linking_receipt(
        &data_dir,
        BATCH_KIND_GOVERNANCE,
        &shielded_test_id("mixed-governance-batch"),
        governance.clone(),
        "mixed-governance-block",
    );
    append_block_linking_receipt(
        &data_dir,
        BATCH_KIND_TRANSPARENT,
        &shielded_test_id("mixed-transparent-batch"),
        transparent.clone(),
        "mixed-transparent-block",
    );
    NodeStore::new(&data_dir)
        .write_receipts(&[payment, governance, transparent])
        .expect("write block-ordered receipt log");
    let build = rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: data_dir.clone(),
    })
    .map_err(duplicate)
    .expect_err("index build refuses an id repeated outside governance blocks");
    assert!(build.contains("receipt log contains duplicate tx_id"), "{build}");
    fs::remove_dir_all(data_dir).expect("remove test data dir");
}
