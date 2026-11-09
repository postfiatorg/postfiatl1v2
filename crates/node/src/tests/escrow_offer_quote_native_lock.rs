// Regression tests for postfiatorg/postfiatl1v2#197.
//
// Execution locks the native PFT an `escrow_create` escrows
// (`apply_escrow_operation`) or an `offer_create` sells (`lock_offer_sell_side`)
// after taking the fee, and refuses with `insufficient_funds` or
// `below_account_reserve` when the lock breaks the reserve. `escrow_fee_quote`
// and `offer_fee_quote` count that lock in `sender_meets_reserve_after_fee`
// (escrow) and `sender_meets_reserve_after_fee_and_reserve` (offer) and report
// the balance after it in `sender_balance_after_amount_and_fee` and
// `sender_balance_after_amount_fee_and_reserve`. Before the fix they counted
// only the fee (and the offer object reserve).
//
// The controls pin the exact boundary; the other tests are one atom past it
// (or above the balance), where the quote and admission must both refuse.
use super::*;

const FUNDING: u64 = ACCOUNT_RESERVE + 700;

struct Devnet {
    data_dir: PathBuf,
    genesis: Genesis,
    store: NodeStore,
    faucet: DevKeyFile,
}

fn init_devnet(prefix: &str) -> Devnet {
    let data_dir = unique_test_dir(prefix);
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let faucet = read_transfer_key_file(&data_dir, None).expect("faucet key");
    Devnet {
        data_dir,
        genesis,
        store,
        faucet,
    }
}

struct Holder {
    address: String,
    public_key_hex: String,
    private_key_hex: String,
}

fn keygen_holder() -> Holder {
    let key = ml_dsa_65_keygen().expect("holder keygen");
    Holder {
        address: address_from_public_key(&key.public_key),
        public_key_hex: bytes_to_hex(&key.public_key),
        private_key_hex: bytes_to_hex(&key.private_key),
    }
}

/// Faucet transfer of `amount` to `to`, applied in its own block.
fn fund(devnet: &Devnet, to: &str, amount: u64, faucet_sequence: u64) {
    let ledger = devnet.store.read_ledger().expect("ledger");
    let transfer = build_signed_transfer_for_key(
        &devnet.genesis,
        &ledger,
        &devnet.faucet,
        to.to_string(),
        amount,
        faucet_sequence,
    )
    .expect("fund transfer");
    let batch = build_transaction_batch(&mempool_batch_domain(&devnet.genesis), vec![transfer])
        .expect("fund batch")
        .batch;
    let batch_file = devnet.data_dir.join("fund-batch.json");
    write_batch_file(&batch_file, &batch).expect("write fund batch");
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: devnet.data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply fund batch");
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    assert!(receipts[0].accepted, "{receipts:?}");
}

fn apply_pending(devnet: &Devnet, label: &str) -> Vec<Receipt> {
    let batch_file = devnet.data_dir.join(format!("{label}-mempool-batch.json"));
    create_mempool_batch(MempoolBatchOptions {
        data_dir: devnet.data_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 8,
    })
    .expect("mempool batch");
    apply_batch(ApplyBatchOptions {
        data_dir: devnet.data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply mempool batch")
}

// ---------------------------------------------------------------------------
// escrow_create
// ---------------------------------------------------------------------------

fn native_escrow_create(owner: &str, recipient: &str, amount: u64) -> EscrowTransactionOperation {
    EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
        owner: owner.to_string(),
        recipient: recipient.to_string(),
        asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
        amount,
        condition: "repro-197".to_string(),
        finish_after: 0,
        cancel_after: 50,
    })
}

fn quote_escrow(
    devnet: &Devnet,
    source: &str,
    operation: &EscrowTransactionOperation,
) -> EscrowFeeQuoteReport {
    escrow_fee_quote(EscrowFeeQuoteOptions {
        data_dir: devnet.data_dir.clone(),
        source: source.to_string(),
        operation_json: serde_json::to_string(operation).expect("escrow op json"),
        sequence: None,
    })
    .expect("escrow fee quote")
}

/// Quote a native escrow_create from `holder`, then size the amount so the
/// holder keeps exactly `ACCOUNT_RESERVE + slack` after amount and fee.
fn sized_native_escrow_create(
    devnet: &Devnet,
    holder: &Holder,
    recipient: &str,
    slack_above_reserve: i64,
) -> (EscrowTransactionOperation, EscrowFeeQuoteReport) {
    let probe = native_escrow_create(&holder.address, recipient, 1);
    let probe_quote = quote_escrow(devnet, &holder.address, &probe);
    let amount = probe_quote.sender_balance as i64
        - probe_quote.minimum_fee as i64
        - ACCOUNT_RESERVE as i64
        - slack_above_reserve;
    let operation = native_escrow_create(&holder.address, recipient, amount as u64);
    let quote = quote_escrow(devnet, &holder.address, &operation);
    assert_eq!(
        quote.minimum_fee, probe_quote.minimum_fee,
        "fee changed with the amount digits"
    );
    (operation, quote)
}

fn sign_escrow(
    devnet: &Devnet,
    holder: &Holder,
    sequence: u64,
    operation: &EscrowTransactionOperation,
) -> SignedEscrowTransaction {
    let ledger = devnet.store.read_ledger().expect("ledger");
    signed_escrow_transaction_for_test(
        &devnet.genesis,
        &ledger,
        &holder.address,
        &holder.public_key_hex,
        &holder.private_key_hex,
        ESCROW_CREATE_TRANSACTION_KIND,
        sequence,
        operation.clone(),
    )
}

fn submit_escrow(
    devnet: &Devnet,
    signed: &SignedEscrowTransaction,
) -> io::Result<MempoolEscrowTransactionEntry> {
    submit_signed_escrow_transaction_json_to_mempool(SignedEscrowTransactionJsonSubmitOptions {
        data_dir: devnet.data_dir.clone(),
        signed_escrow_transaction_json: serde_json::to_string(signed).expect("signed escrow json"),
    })
}

fn escrow_amount(operation: &EscrowTransactionOperation) -> u64 {
    match operation {
        EscrowTransactionOperation::EscrowCreate(operation) => operation.amount,
        _ => unreachable!("escrow_create only"),
    }
}

/// Control: the holder locks everything above reserve + fee. The quote says
/// the reserve is met, admission accepts, the block accepts, and the holder
/// keeps exactly ACCOUNT_RESERVE.
#[test]
fn escrow_create_quote_and_admission_agree_at_exactly_the_reserve() {
    let devnet = init_devnet("postfiat-escrow-quote-native-lock-control");
    let holder = keygen_holder();
    let recipient = keygen_holder();
    fund(&devnet, &holder.address, FUNDING, 1);

    let (operation, quote) = sized_native_escrow_create(&devnet, &holder, &recipient.address, 0);
    assert_eq!(quote.sender_balance, FUNDING);
    assert!(quote.sender_meets_reserve_after_fee, "{quote:?}");
    assert_eq!(quote.sender_balance_after_amount_and_fee, Some(ACCOUNT_RESERVE));
    let signed = sign_escrow(&devnet, &holder, quote.sequence, &operation);
    assert_eq!(signed.unsigned.fee, quote.minimum_fee);

    submit_escrow(&devnet, &signed).expect("admission accepts the boundary escrow");
    let receipts = apply_pending(&devnet, "escrow-boundary");
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    assert!(receipts[0].accepted, "{receipts:?}");
    let ledger = devnet.store.read_ledger().expect("ledger");
    assert_eq!(
        ledger.account(&holder.address).expect("holder").balance,
        ACCOUNT_RESERVE
    );

    fs::remove_dir_all(devnet.data_dir).expect("cleanup");
}

/// One atom more than the holder can lock. The quote reports the reserve
/// missed, one atom below it after the lock, and admission refuses the
/// transaction signed with the quote's fee and sequence with
/// `below_account_reserve`.
#[test]
fn escrow_create_quote_and_admission_refuse_one_atom_past_the_reserve() {
    let devnet = init_devnet("postfiat-escrow-quote-native-lock-plus-one");
    let holder = keygen_holder();
    let recipient = keygen_holder();
    fund(&devnet, &holder.address, FUNDING, 1);

    let (operation, quote) = sized_native_escrow_create(&devnet, &holder, &recipient.address, -1);
    assert!(!quote.sender_meets_reserve_after_fee, "{quote:?}");
    assert_eq!(
        quote.sender_balance_after_amount_and_fee,
        Some(ACCOUNT_RESERVE - 1)
    );
    assert_eq!(
        quote.sender_balance_after_fee,
        Some(quote.sender_balance - quote.minimum_fee),
        "the after-fee field keeps its meaning"
    );
    let signed = sign_escrow(&devnet, &holder, quote.sequence, &operation);
    assert_eq!(signed.unsigned.fee, quote.minimum_fee);

    let refused = submit_escrow(&devnet, &signed).expect_err("admission refuses");
    assert!(
        refused.to_string().contains("below_account_reserve"),
        "amount {}: {refused}",
        escrow_amount(&operation)
    );
    fs::remove_dir_all(devnet.data_dir).expect("cleanup");
}

/// An amount above the whole balance: the quote reports the reserve missed
/// and no balance after the lock; admission refuses with `insufficient_funds`.
#[test]
fn escrow_create_quote_and_admission_refuse_above_the_balance() {
    let devnet = init_devnet("postfiat-escrow-quote-native-lock-above-balance");
    let holder = keygen_holder();
    let recipient = keygen_holder();
    fund(&devnet, &holder.address, FUNDING, 1);

    let operation = native_escrow_create(&holder.address, &recipient.address, FUNDING * 10);
    let quote = quote_escrow(&devnet, &holder.address, &operation);
    assert!(!quote.sender_meets_reserve_after_fee, "{quote:?}");
    assert_eq!(quote.sender_balance_after_amount_and_fee, None);
    let signed = sign_escrow(&devnet, &holder, quote.sequence, &operation);

    let refused = submit_escrow(&devnet, &signed).expect_err("admission refuses");
    assert!(
        refused.to_string().contains("insufficient_funds"),
        "amount {}: {refused}",
        escrow_amount(&operation)
    );
    fs::remove_dir_all(devnet.data_dir).expect("cleanup");
}

// ---------------------------------------------------------------------------
// offer_create with a native sell side
// ---------------------------------------------------------------------------

/// Faucet-issued asset plus a trustline for `holder`, so the holder can sell
/// native PFT for it.
fn setup_dex_asset(devnet: &Devnet, holder: &Holder) -> String {
    let ledger = devnet.store.read_ledger().expect("ledger");
    let faucet_sequence = ledger
        .account(&devnet.faucet.address)
        .expect("faucet")
        .sequence
        + 1;
    let create_asset = signed_asset_transaction_for_test(
        &devnet.genesis,
        &ledger,
        &devnet.faucet.address,
        &devnet.faucet.public_key_hex,
        &devnet.faucet.private_key_hex,
        ASSET_CREATE_TRANSACTION_KIND,
        faucet_sequence,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: devnet.faucet.address.clone(),
            code: "RPR".to_string(),
            version: 1,
            precision: 0,
            display_name: "repro 197 asset".to_string(),
            max_supply: Some(1_000),
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
        }),
    );
    let mut dry_run = ledger.clone();
    assert!(execute_asset_transaction(&devnet.genesis, &mut dry_run, &create_asset, 1).accepted);
    let asset_id = dry_run.asset_definitions[0].asset_id.clone();
    let trust_set = signed_asset_transaction_for_test(
        &devnet.genesis,
        &dry_run,
        &holder.address,
        &holder.public_key_hex,
        &holder.private_key_hex,
        TRUST_SET_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: holder.address.clone(),
            issuer: devnet.faucet.address.clone(),
            asset_id: asset_id.clone(),
            limit: 500,
            authorized: false,
            frozen: false,
            reserve_paid: postfiat_execution::TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    let batch = postfiat_mempool_dag::build_mixed_transaction_batch_with_assets(
        &mempool_batch_domain(&devnet.genesis),
        Vec::new(),
        Vec::new(),
        vec![create_asset, trust_set],
    )
    .expect("asset setup batch")
    .batch;
    let batch_file = devnet.data_dir.join("dex-asset-setup-batch.json");
    write_batch_file(&batch_file, &batch).expect("write asset setup batch");
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: devnet.data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply asset setup");
    assert!(
        receipts.iter().all(|receipt| receipt.accepted),
        "{receipts:?}"
    );
    asset_id
}

fn native_sell_offer(
    owner: &str,
    asset_id: &str,
    taker_gets_amount: u64,
) -> OfferTransactionOperation {
    OfferTransactionOperation::OfferCreate(OfferCreateOperation {
        owner: owner.to_string(),
        taker_gets_asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
        taker_gets_amount,
        taker_pays_asset_id: asset_id.to_string(),
        taker_pays_amount: 1,
        expiration_height: 0,
    })
}

fn quote_offer(
    devnet: &Devnet,
    source: &str,
    operation: &OfferTransactionOperation,
) -> OfferFeeQuoteReport {
    offer_fee_quote(OfferFeeQuoteOptions {
        data_dir: devnet.data_dir.clone(),
        source: source.to_string(),
        operation_json: serde_json::to_string(operation).expect("offer op json"),
        sequence: None,
    })
    .expect("offer fee quote")
}

/// Size the native sell amount so the owner keeps `ACCOUNT_RESERVE + slack`
/// after fee, offer object reserve and the locked sell side.
fn sized_native_sell_offer(
    devnet: &Devnet,
    holder: &Holder,
    asset_id: &str,
    slack_above_reserve: i64,
) -> (OfferTransactionOperation, OfferFeeQuoteReport) {
    let probe_quote = quote_offer(
        devnet,
        &holder.address,
        &native_sell_offer(&holder.address, asset_id, 1),
    );
    assert!(probe_quote.will_create_residual_offer, "{probe_quote:?}");
    let amount = probe_quote.sender_balance as i64
        - probe_quote.minimum_fee as i64
        - probe_quote.offer_object_reserve as i64
        - ACCOUNT_RESERVE as i64
        - slack_above_reserve;
    let operation = native_sell_offer(&holder.address, asset_id, amount as u64);
    let quote = quote_offer(devnet, &holder.address, &operation);
    assert_eq!(
        quote.minimum_fee, probe_quote.minimum_fee,
        "fee changed with the amount digits"
    );
    (operation, quote)
}

fn sign_offer(
    devnet: &Devnet,
    holder: &Holder,
    sequence: u64,
    operation: &OfferTransactionOperation,
) -> SignedOfferTransaction {
    let ledger = devnet.store.read_ledger().expect("ledger");
    signed_offer_transaction_for_test(
        &devnet.genesis,
        &ledger,
        &holder.address,
        &holder.public_key_hex,
        &holder.private_key_hex,
        OFFER_CREATE_TRANSACTION_KIND,
        sequence,
        operation.clone(),
    )
}

fn submit_offer(
    devnet: &Devnet,
    signed: &SignedOfferTransaction,
) -> io::Result<MempoolOfferTransactionEntry> {
    submit_signed_offer_transaction_json_to_mempool(SignedOfferTransactionJsonSubmitOptions {
        data_dir: devnet.data_dir.clone(),
        signed_offer_transaction_json: serde_json::to_string(signed).expect("signed offer json"),
    })
}

fn offer_sell_amount(operation: &OfferTransactionOperation) -> u64 {
    match operation {
        OfferTransactionOperation::OfferCreate(operation) => operation.taker_gets_amount,
        _ => unreachable!("offer_create only"),
    }
}

/// Control: selling everything above fee + object reserve + account reserve
/// is quoted as fine, admitted and executed; the owner keeps exactly
/// ACCOUNT_RESERVE.
#[test]
fn offer_create_quote_and_admission_agree_at_exactly_the_reserve() {
    let devnet = init_devnet("postfiat-offer-quote-native-lock-control");
    let holder = keygen_holder();
    fund(&devnet, &holder.address, FUNDING, 1);
    let asset_id = setup_dex_asset(&devnet, &holder);

    let (operation, quote) = sized_native_sell_offer(&devnet, &holder, &asset_id, 0);
    assert!(
        quote.sender_meets_reserve_after_fee_and_reserve,
        "{quote:?}"
    );
    assert_eq!(
        quote.sender_balance_after_amount_fee_and_reserve,
        Some(ACCOUNT_RESERVE)
    );
    let signed = sign_offer(&devnet, &holder, quote.sequence, &operation);
    assert_eq!(signed.unsigned.fee, quote.minimum_fee);
    submit_offer(&devnet, &signed).expect("admission accepts the boundary offer");
    let receipts = apply_pending(&devnet, "offer-boundary");
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    assert!(receipts[0].accepted, "{receipts:?}");
    let ledger = devnet.store.read_ledger().expect("ledger");
    assert_eq!(
        ledger.account(&holder.address).expect("holder").balance,
        ACCOUNT_RESERVE
    );
    fs::remove_dir_all(devnet.data_dir).expect("cleanup");
}

/// One atom more on the native sell side: the quote reports the reserve
/// missed, one atom below it after the lock and the object reserve, and
/// admission refuses with `below_account_reserve`.
#[test]
fn offer_create_quote_and_admission_refuse_one_atom_past_the_reserve() {
    let devnet = init_devnet("postfiat-offer-quote-native-lock-plus-one");
    let holder = keygen_holder();
    fund(&devnet, &holder.address, FUNDING, 1);
    let asset_id = setup_dex_asset(&devnet, &holder);

    let (operation, quote) = sized_native_sell_offer(&devnet, &holder, &asset_id, -1);
    assert!(
        !quote.sender_meets_reserve_after_fee_and_reserve,
        "{quote:?}"
    );
    assert_eq!(
        quote.sender_balance_after_amount_fee_and_reserve,
        Some(ACCOUNT_RESERVE - 1)
    );
    assert_eq!(
        quote.sender_balance_after_fee_and_reserve,
        Some(quote.sender_balance - quote.minimum_fee - quote.offer_object_reserve),
        "the after-fee-and-reserve field keeps its meaning"
    );
    let signed = sign_offer(&devnet, &holder, quote.sequence, &operation);
    let refused = submit_offer(&devnet, &signed).expect_err("admission refuses");
    assert!(
        refused.to_string().contains("below_account_reserve"),
        "taker_gets_amount {}: {refused}",
        offer_sell_amount(&operation)
    );
    fs::remove_dir_all(devnet.data_dir).expect("cleanup");
}
