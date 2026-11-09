// Regression tests for postfiatorg/postfiatl1v2#193.
//
// `transfer_fee_quote` and `mempool-submit-transfer` take the sequence of the
// quoted transfer from the mempool, so they must price it on the ledger that
// sequence executes on: after the executable mempool
// (`ledger_after_executable_mempool`), as mempool admission, block execution
// and the other quotes (`asset_fee_quote`, `escrow_fee_quote`,
// `nft_fee_quote`, `offer_fee_quote`) do. Before the fix they priced it on the
// committed ledger, so `recipient_exists`, `state_expansion_fee`,
// `minimum_fee`, `sender_balance` and `sender_meets_reserve_after_transfer`
// ignored the pending entries. The control test pins the empty-mempool case.
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
/// and the fee `mempool-submit-transfer` declares must not price the account
/// creation again: the receipt charges no `TRANSFER_ACCOUNT_CREATION_FEE`.
#[test]
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

/// The quote must count the sender's pending debit: it reports
/// `sender_meets_reserve_after_transfer: false` for a transfer that admission
/// refuses. With the declared fee priced on the same ledger (22: the pending
/// transfer creates the recipient) admission refuses with
/// `below_account_reserve`; before the fix the declared 32 made it
/// `insufficient_funds`.
#[test]
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
        refused.to_string().contains("below_account_reserve"),
        "{refused}"
    );

    // Expected: the quote agrees with admission.
    assert_eq!(quote.sender_balance, remaining - AMOUNT - first.transfer.unsigned.fee);
    assert_eq!(quote.minimum_fee, 22, "sequence 2 creates the recipient");
    assert_eq!(
        quote.sender_balance_after_amount_and_fee,
        Some(quote.sender_balance - AMOUNT - 22)
    );
    assert!(
        !quote.sender_meets_reserve_after_transfer,
        "quote: sender_balance {} after {:?}, meets reserve; admission: {refused}",
        quote.sender_balance, quote.sender_balance_after_amount_and_fee
    );

    fs::remove_dir_all(data_dir).expect("cleanup");
}

/// Another sender's pending `payment_v2` creates the recipient. A plain
/// transfer runs before every pending `payment_v2` in admission and in the
/// block, so for it the recipient does not exist yet: the quote and the fee
/// `mempool-submit-transfer` declares must include the creation fee. A memo
/// quote runs after the pending `payment_v2` entries and sees the recipient.
#[test]
fn transfer_fee_quote_runs_only_the_entries_before_a_plain_transfer() {
    let data_dir = init_devnet("postfiat-transfer-fee-quote-family-order");
    let genesis = NodeStore::new(&data_dir).read_genesis().expect("genesis");
    let faucet = read_transfer_key_file(&data_dir, None).expect("faucet key");
    let sender = create_dev_key_file().expect("sender key");
    let sender_key = data_dir.join("sender.key.json");
    fs::write(
        &sender_key,
        serde_json::to_string_pretty(&sender).expect("key json"),
    )
    .expect("write sender key");
    set_private_file_permissions(&sender_key).expect("key permissions");
    submit_faucet_transfer(&data_dir, &sender.address, 100_000).expect("fund sender");
    assert!(apply_pending(&data_dir, 1)[0].accepted);

    // The faucet's pending payment_v2 creates SINK.
    let memo = || PaymentMemo {
        memo_type: "74616700".to_string(),
        memo_format: String::new(),
        memo_data: "2a".to_string(),
    };
    let memo_quote = |from: &str| {
        transfer_fee_quote(TransferFeeQuoteOptions {
            data_dir: data_dir.clone(),
            from: from.to_string(),
            to: SINK.to_string(),
            amount: AMOUNT,
            sequence: None,
            memo_type: Some(memo().memo_type),
            memo_format: Some(memo().memo_format),
            memo_data: Some(memo().memo_data),
        })
        .expect("memo quote")
    };
    let faucet_quote = memo_quote(&faucet.address);
    let unsigned = UnsignedPaymentV2 {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: PAYMENT_V2_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from: faucet.address.clone(),
        to: SINK.to_string(),
        amount: AMOUNT,
        fee: faucet_quote.minimum_fee,
        sequence: faucet_quote.sequence,
        memos: vec![memo()],
    };
    let private_key = hex_to_bytes(&faucet.private_key_hex).expect("faucet private key");
    let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign");
    let payment = SignedPaymentV2 {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: faucet.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    };
    submit_signed_payment_v2_json_to_mempool(SignedPaymentV2JsonSubmitOptions {
        data_dir: data_dir.clone(),
        signed_payment_v2_json: serde_json::to_string(&payment).expect("payment json"),
    })
    .expect("pending payment_v2 creating the recipient");

    // A plain transfer from the other sender runs before that payment_v2.
    let quote = transfer_fee_quote(TransferFeeQuoteOptions {
        data_dir: data_dir.clone(),
        from: sender.address.clone(),
        to: SINK.to_string(),
        amount: AMOUNT,
        sequence: None,
        memo_type: None,
        memo_format: None,
        memo_data: None,
    })
    .expect("plain quote");
    assert!(!quote.recipient_exists, "{quote:?}");
    assert!(quote.will_create_recipient_account);
    assert_eq!(quote.state_expansion_fee, TRANSFER_ACCOUNT_CREATION_FEE);

    let submitted = submit_transfer_to_mempool(TransferOptions {
        data_dir: data_dir.clone(),
        key_file: Some(sender_key.clone()),
        to: SINK.to_string(),
        amount: AMOUNT,
    })
    .expect("admission accepts the plain transfer at the quoted fee");
    assert_eq!(submitted.transfer.unsigned.fee, quote.minimum_fee);

    // A memo payment runs after the pending payment_v2 entries: the recipient exists.
    let sender_memo_quote = memo_quote(&sender.address);
    assert!(sender_memo_quote.recipient_exists, "{sender_memo_quote:?}");
    assert_eq!(sender_memo_quote.state_expansion_fee, 0);

    let receipts = apply_pending(&data_dir, 10);
    assert_eq!(receipts.len(), 2, "{receipts:?}");
    assert!(receipts.iter().all(|receipt| receipt.accepted), "{receipts:?}");
    let transfer_receipt = receipts
        .iter()
        .find(|receipt| receipt.tx_id == submitted.tx_id)
        .expect("transfer receipt");
    assert_eq!(
        (transfer_receipt.state_expansion_fee, transfer_receipt.minimum_fee),
        (quote.state_expansion_fee, quote.minimum_fee),
        "the plain transfer created the recipient, as quoted"
    );

    fs::remove_dir_all(data_dir).expect("cleanup");
}

/// A confirmed FastPay unwrap that is not yet in a block credits a transparent
/// account in the ledger admission starts from (`read_fastpay_ledger`) and not
/// in the committed ledger. Every fee quote must see that credit: the account
/// exists with the unwrapped amount for each quote that names it.
#[test]
fn fee_quotes_start_from_the_ledger_admission_uses() {
    let (root, dirs, owner) = super::fastpay_payment_safety::active_fastpay_fixture();
    let unwrapped_to = "pf00000000000000000000000000000000000000a9".to_string();
    let store = NodeStore::new(&dirs[0]);
    let committed = store.read_ledger().expect("committed ledger");
    let committee = &committed.fastpay_recovery_committees[0];
    let mut order = postfiat_types::OwnedUnwrapOrderV3 {
        domain: committee.certificate_domain(),
        recovery: postfiat_types::FastPayOrderRecoveryV1 {
            schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.into(),
            committee_epoch: 1,
            lock_id: "00".repeat(48),
            valid_from_height: 1,
            expires_at_height: 10,
            recovery_closes_at_height: 20,
        },
        inputs: vec![postfiat_types::OwnedObjectRef {
            id: "active-fastpay-input".into(),
            version: 1,
        }],
        to_address: unwrapped_to.clone(),
        amount: 99,
        fee: 1,
        asset: "PFT".into(),
        nonce: 43,
        memos: Vec::new(),
    };
    order.recovery.lock_id = postfiat_types::fastpay_unwrap_lock_id_v1(&order);
    let owner_signature_hex = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &owner.private_key,
            &postfiat_execution::owned_unwrap_v3_signing_bytes(&order),
            postfiat_execution::OWNED_UNWRAP_CONTEXT_V3,
        )
        .expect("owner signature"),
    );
    let keys = read_validator_key_file(&dirs[0].join(VALIDATOR_KEYS_FILE)).expect("keys");
    let votes = keys
        .validators
        .iter()
        .take(5)
        .map(|key| postfiat_types::OwnedUnwrapVote {
            validator_id: key.node_id.clone(),
            signature_hex: bytes_to_hex(
                &ml_dsa_65_sign_with_context(
                    &hex_to_bytes(&key.private_key_hex).expect("validator key"),
                    &postfiat_execution::owned_unwrap_v3_signing_bytes(&order),
                    postfiat_execution::OWNED_UNWRAP_CONTEXT_V3,
                )
                .expect("vote signature"),
            ),
        })
        .collect();
    let certificate = postfiat_types::OwnedUnwrapCertificateV3 {
        order,
        owner_pubkey_hex: bytes_to_hex(&owner.public_key),
        owner_signature_hex,
        votes,
    };
    owned_unwrap_apply_v3(
        NodeOptions {
            data_dir: dirs[0].clone(),
        },
        &serde_json::to_string(&certificate).expect("certificate json"),
        "validator-0",
    )
    .expect("apply the certified FastPay unwrap");

    // The unwrap is confirmed but not committed.
    assert!(store
        .read_ledger()
        .expect("committed ledger")
        .account(&unwrapped_to)
        .is_none());
    let admission = admission_base_ledger(&store).expect("admission ledger");
    assert_eq!(
        admission.account(&unwrapped_to).expect("unwrapped account").balance,
        99
    );

    // The transfer quote sees the account the unwrap created, as a recipient
    // and as a sender.
    let faucet = read_transfer_key_file(&dirs[0], None).expect("faucet key");
    let transfer_quote = |from: &str, to: &str| {
        transfer_fee_quote(TransferFeeQuoteOptions {
            data_dir: dirs[0].clone(),
            from: from.to_string(),
            to: to.to_string(),
            amount: 10,
            sequence: None,
            memo_type: None,
            memo_format: None,
            memo_data: None,
        })
    };
    let to_unwrapped = transfer_quote(&faucet.address, &unwrapped_to).expect("transfer quote");
    assert!(to_unwrapped.recipient_exists, "{to_unwrapped:?}");
    assert_eq!(to_unwrapped.state_expansion_fee, 0);
    let from_unwrapped = transfer_quote(&unwrapped_to, &faucet.address)
        .expect("transfer quote from the unwrapped account");
    assert_eq!(from_unwrapped.sender_balance, 99);

    // The other four quotes report the unwrapped account's balance.
    let escrow = escrow_fee_quote(EscrowFeeQuoteOptions {
        data_dir: dirs[0].clone(),
        source: unwrapped_to.clone(),
        operation_json: serde_json::to_string(&EscrowTransactionOperation::EscrowCreate(
            EscrowCreateOperation {
                owner: unwrapped_to.clone(),
                recipient: faucet.address.clone(),
                asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
                amount: 1,
                condition: "unwrap-quote".to_string(),
                finish_after: 0,
                cancel_after: 50,
            },
        ))
        .expect("escrow json"),
        sequence: None,
    })
    .expect("escrow quote");
    assert_eq!(escrow.sender_balance, 99);
    let asset = asset_fee_quote(AssetFeeQuoteOptions {
        data_dir: dirs[0].clone(),
        source: unwrapped_to.clone(),
        operation_json: serde_json::to_string(&AssetTransactionOperation::AssetCreate(
            AssetCreateOperation {
                issuer: unwrapped_to.clone(),
                code: "UNW".to_string(),
                version: 1,
                precision: 0,
                display_name: "unwrap quote asset".to_string(),
                max_supply: Some(10),
                requires_authorization: false,
                freeze_enabled: false,
                clawback_enabled: false,
            },
        ))
        .expect("asset json"),
        sequence: None,
    })
    .expect("asset quote");
    assert_eq!(asset.sender_balance, 99);
    let nft = nft_fee_quote(NftFeeQuoteOptions {
        data_dir: dirs[0].clone(),
        source: unwrapped_to.clone(),
        operation_json: serde_json::json!({
            "operation": "nft_mint",
            "issuer": unwrapped_to,
            "collection_id": "unwrap-quote",
            "serial": 1,
            "owner": unwrapped_to,
            "metadata_hash": "ab".repeat(32)
        })
        .to_string(),
        sequence: None,
    })
    .expect("nft quote");
    assert_eq!(nft.sender_balance, 99);
    let offer = offer_fee_quote(OfferFeeQuoteOptions {
        data_dir: dirs[0].clone(),
        source: unwrapped_to.clone(),
        operation_json: serde_json::to_string(&OfferTransactionOperation::OfferCreate(
            OfferCreateOperation {
                owner: unwrapped_to.clone(),
                taker_gets_asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
                taker_gets_amount: 1,
                taker_pays_asset_id: "cd".repeat(48),
                taker_pays_amount: 1,
                expiration_height: 0,
            },
        ))
        .expect("offer json"),
        sequence: None,
    })
    .expect("offer quote");
    assert_eq!(offer.sender_balance, 99);

    // Atomic swaps are not active in this fixture, so the swap quote stops at
    // its activation check. It reads its base ledger first: with a FastPay
    // journal admission's ledger rejects, the quote fails on that journal.
    let journal_path = dirs[0].join(FASTPAY_SPECULATIVE_JOURNAL_FILE);
    let mut journal: serde_json::Value =
        serde_json::from_slice(&fs::read(&journal_path).expect("journal")).expect("journal json");
    journal["effects"][0]["retained_tip"]["height"] = 1_000.into();
    atomic_write(&journal_path, serde_json::to_vec(&journal).expect("journal bytes"))
        .expect("write journal");
    let leg = |owner: &str, recipient: &str| AtomicSwapQuoteLegInput {
        owner: owner.to_string(),
        recipient: recipient.to_string(),
        issuer: String::new(),
        asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
        amount: 1,
    };
    let swap_error = atomic_swap_fee_quote(AtomicSwapFeeQuoteOptions {
        data_dir: dirs[0].clone(),
        rfq_hash: "11".repeat(48),
        market_envelope_hash: "22".repeat(48),
        nav_epoch: 1,
        expires_at_height: 50,
        swap_nonce: "33".repeat(16),
        leg_0: leg(&unwrapped_to, &faucet.address),
        leg_1: leg(&faucet.address, &unwrapped_to),
    })
    .expect_err("the swap quote reads the rejected FastPay journal");
    assert!(
        swap_error.to_string().contains("FastPay journal"),
        "{swap_error}"
    );

    fs::remove_dir_all(root).expect("cleanup");
}
