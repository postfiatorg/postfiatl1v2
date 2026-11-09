// Reproduction for postfiatorg/postfiatl1v2#198.
//
// The `account_tx` row for an `escrow_finish` lists the escrow owner as
// `from` and carries the signed envelope's `fee` and `sequence`. A finish is
// signed by the recipient (`EscrowTransactionOperation::source_matches`), who
// pays the fee and whose sequence advances; the owner's balance and sequence
// do not move. The row contract (docs/rpc/account-history.md) says `fee` left
// `from` and `sequence` was consumed from it.
//
// The control (the escrow_create row) passes on main. The `#[ignore]` test
// states the expected attribution and fails on main. Run it with
// `cargo test -p postfiat-node --lib account_tx_escrow_finish_row -- --include-ignored`.
use super::*;

const ESCROW_AMOUNT: u64 = ACCOUNT_RESERVE + 25;

struct Devnet {
    data_dir: PathBuf,
    genesis: Genesis,
    store: NodeStore,
}

struct Holder {
    address: String,
    public_key_hex: String,
    private_key_hex: String,
}

/// Fresh devnet. Block 1 funds a recipient so it can sign and pay for the
/// finish. Returns the devnet, the owner (the faucet) and the recipient.
fn init_devnet(prefix: &str) -> (Devnet, Holder, Holder) {
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
    let owner = Holder {
        address: faucet.address.clone(),
        public_key_hex: faucet.public_key_hex.clone(),
        private_key_hex: faucet.private_key_hex.clone(),
    };
    let key = ml_dsa_65_keygen().expect("recipient keygen");
    let recipient = Holder {
        address: address_from_public_key(&key.public_key),
        public_key_hex: bytes_to_hex(&key.public_key),
        private_key_hex: bytes_to_hex(&key.private_key),
    };
    let devnet = Devnet {
        data_dir,
        genesis,
        store,
    };

    let ledger = devnet.store.read_ledger().expect("ledger");
    let transfer = build_signed_transfer_for_key(
        &devnet.genesis,
        &ledger,
        &faucet,
        recipient.address.clone(),
        ACCOUNT_RESERVE + 700,
        1,
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
    assert!(receipts[0].accepted, "{receipts:?}");
    (devnet, owner, recipient)
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

/// Quote, sign as `signer` with the quoted fee and sequence, admit, and apply
/// in its own block. Returns the signed transaction.
fn run_escrow(
    devnet: &Devnet,
    signer: &Holder,
    kind: &str,
    operation: EscrowTransactionOperation,
    label: &str,
) -> SignedEscrowTransaction {
    let quote = escrow_fee_quote(EscrowFeeQuoteOptions {
        data_dir: devnet.data_dir.clone(),
        source: signer.address.clone(),
        operation_json: serde_json::to_string(&operation).expect("escrow op json"),
        sequence: None,
    })
    .expect("escrow fee quote");
    let ledger = devnet.store.read_ledger().expect("ledger");
    let signed = signed_escrow_transaction_for_test(
        &devnet.genesis,
        &ledger,
        &signer.address,
        &signer.public_key_hex,
        &signer.private_key_hex,
        kind,
        quote.sequence,
        operation,
    );
    assert_eq!(signed.unsigned.fee, quote.minimum_fee);
    submit_signed_escrow_transaction_json_to_mempool(SignedEscrowTransactionJsonSubmitOptions {
        data_dir: devnet.data_dir.clone(),
        signed_escrow_transaction_json: serde_json::to_string(&signed).expect("signed escrow json"),
    })
    .expect("admit escrow transaction");
    let receipts = apply_pending(devnet, label);
    assert_eq!(receipts.len(), 1, "{receipts:?}");
    assert!(receipts[0].accepted, "{receipts:?}");
    signed
}

/// Block 2: the owner creates a native escrow to the recipient.
fn create_escrow(
    devnet: &Devnet,
    owner: &Holder,
    recipient: &Holder,
) -> (SignedEscrowTransaction, String) {
    let signed = run_escrow(
        devnet,
        owner,
        ESCROW_CREATE_TRANSACTION_KIND,
        EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
            owner: owner.address.clone(),
            recipient: recipient.address.clone(),
            asset_id: postfiat_execution::NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            amount: ESCROW_AMOUNT,
            condition: "repro-secret".to_string(),
            finish_after: 0,
            cancel_after: 50,
        }),
        "create",
    );
    let escrow_id = postfiat_types::escrow_id(
        &devnet.genesis.chain_id,
        &owner.address,
        signed.unsigned.sequence,
    )
    .expect("escrow id");
    (signed, escrow_id)
}

fn rows_at(devnet: &Devnet, address: &str, height: u64) -> Vec<AccountTxRow> {
    rebuild_account_tx_index(AccountTxIndexOptions {
        data_dir: devnet.data_dir.clone(),
    })
    .expect("rebuild account_tx index");
    account_tx(AccountTxQueryOptions {
        data_dir: devnet.data_dir.clone(),
        address: address.to_string(),
        from_height: Some(height),
        to_height: Some(height),
        limit: Some(10),
    })
    .expect("account_tx")
    .rows
}

/// Control: the create row is listed under the owner, who signed it, paid its
/// fee and consumed its sequence.
#[test]
fn account_tx_escrow_create_row_names_the_signer_as_from() {
    let (devnet, owner, recipient) = init_devnet("postfiat-account-tx-escrow-create-row");
    let (signed_create, escrow_id) = create_escrow(&devnet, &owner, &recipient);

    let rows = rows_at(&devnet, &owner.address, 2);
    assert_eq!(rows.len(), 1, "{rows:?}");
    let row = &rows[0];
    assert_eq!(row.transaction_kind, ESCROW_CREATE_TRANSACTION_KIND);
    assert_eq!(row.escrow_id.as_deref(), Some(escrow_id.as_str()));
    assert_eq!(
        (
            row.from_address.as_str(),
            row.to_address.as_str(),
            row.amount,
            row.fee,
            row.sequence
        ),
        (
            owner.address.as_str(),
            recipient.address.as_str(),
            ESCROW_AMOUNT,
            signed_create.unsigned.fee,
            signed_create.unsigned.sequence
        )
    );
    fs::remove_dir_all(devnet.data_dir).expect("cleanup");
}

/// The recipient signs the finish, pays its fee and consumes its sequence; the
/// ledger shows the owner untouched. The row lists `from: owner` with the
/// recipient's fee and sequence, in the owner's history.
#[test]
#[ignore = "reproduction for #198: the escrow_finish row attributes the signer's fee and sequence to the owner"]
fn account_tx_escrow_finish_row_names_the_signer_as_from() {
    let (devnet, owner, recipient) = init_devnet("postfiat-account-tx-escrow-finish-row");
    let (_, escrow_id) = create_escrow(&devnet, &owner, &recipient);
    let ledger = devnet.store.read_ledger().expect("ledger");
    let owner_before = ledger.account(&owner.address).expect("owner").clone();
    let recipient_before = ledger
        .account(&recipient.address)
        .expect("recipient")
        .clone();

    // Block 3: the recipient finishes the escrow.
    let signed_finish = run_escrow(
        &devnet,
        &recipient,
        ESCROW_FINISH_TRANSACTION_KIND,
        EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
            escrow_id: escrow_id.clone(),
            owner: owner.address.clone(),
            recipient: recipient.address.clone(),
            fulfillment: "repro-secret".to_string(),
        }),
        "finish",
    );
    assert_eq!(signed_finish.unsigned.source, recipient.address);

    let ledger = devnet.store.read_ledger().expect("ledger");
    let owner_after = ledger.account(&owner.address).expect("owner").clone();
    let recipient_after = ledger
        .account(&recipient.address)
        .expect("recipient")
        .clone();
    assert_eq!(
        (owner_after.balance, owner_after.sequence),
        (owner_before.balance, owner_before.sequence),
        "the owner paid nothing and signed nothing"
    );
    assert_eq!(
        (recipient_after.balance, recipient_after.sequence),
        (
            recipient_before.balance + ESCROW_AMOUNT - signed_finish.unsigned.fee,
            recipient_before.sequence + 1
        ),
        "the recipient paid the fee and consumed the sequence"
    );

    let owner_rows = rows_at(&devnet, &owner.address, 3);
    let recipient_rows = rows_at(&devnet, &recipient.address, 3);
    assert_eq!(owner_rows.len(), 1, "{owner_rows:?}");
    assert_eq!(
        recipient_rows, owner_rows,
        "one row, listed for both parties"
    );
    let row = &owner_rows[0];
    assert_eq!(row.transaction_kind, ESCROW_FINISH_TRANSACTION_KIND);
    assert_eq!(row.escrow_id.as_deref(), Some(escrow_id.as_str()));
    assert_eq!(
        (row.fee, row.sequence),
        (signed_finish.unsigned.fee, signed_finish.unsigned.sequence),
        "the row carries the recipient's fee and sequence"
    );
    assert_eq!(
        row.from_address, recipient.address,
        "`from` must be the account that paid `fee` and consumed `sequence`: row from={} to={} amount={} fee={} sequence={}; owner balance {} -> {}, recipient balance {} -> {}",
        row.from_address,
        row.to_address,
        row.amount,
        row.fee,
        row.sequence,
        owner_before.balance,
        owner_after.balance,
        recipient_before.balance,
        recipient_after.balance,
    );
    fs::remove_dir_all(devnet.data_dir).expect("cleanup");
}
