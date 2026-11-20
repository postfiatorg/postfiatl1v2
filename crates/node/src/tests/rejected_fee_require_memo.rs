use super::*;
use postfiat_types::{
    AccountSetOperation, PaymentMemo, UnsignedPaymentV2, UnsignedTransfer,
    ACCOUNT_FLAG_REQUIRE_MEMO, ACCOUNT_SET_TRANSACTION_KIND,
};

const ACTIVATION_HEIGHT: u32 = 20;

fn activation_amendment(kind: &str, height: u32) -> GovernanceAmendment {
    GovernanceAmendment {
        amendment_id: format!("{kind}-{height}"),
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "00".repeat(48),
        protocol_version: 1,
        instance_id: "fee-memo-instance".to_string(),
        proposal_id: "fee-memo-proposal".to_string(),
        certificate_id: "fee-memo-certificate".to_string(),
        proposer: "validator-0".to_string(),
        validators: vec!["validator-0".to_string()],
        quorum: 1,
        kind: kind.to_string(),
        value: height,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        support: vec!["validator-0".to_string()],
        votes: Vec::new(),
        signed_authorizations: Vec::new(),
    }
}

fn governed(kinds: &[&str]) -> GovernanceState {
    let mut governance = GovernanceState::new(1);
    for kind in kinds {
        governance.apply(activation_amendment(kind, ACTIVATION_HEIGHT));
    }
    governance
}

fn key(seed: u8) -> MlDsa65KeyPair {
    postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[seed; 32])
}

fn signed_transfer_for(
    genesis: &Genesis,
    key_pair: &MlDsa65KeyPair,
    to: &str,
    amount: u64,
    fee: u64,
    sequence: u64,
) -> SignedTransfer {
    let unsigned = UnsignedTransfer {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from: address_from_public_key(&key_pair.public_key),
        to: to.to_string(),
        amount,
        fee,
        sequence,
    };
    let signature =
        ml_dsa_65_sign(&key_pair.private_key, &unsigned.signing_bytes()).expect("sign transfer");
    SignedTransfer {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&key_pair.public_key),
        signature_hex: bytes_to_hex(&signature),
    }
}

fn signed_payment_for(
    genesis: &Genesis,
    key_pair: &MlDsa65KeyPair,
    to: &str,
    amount: u64,
    fee: u64,
    sequence: u64,
    memos: Vec<PaymentMemo>,
) -> SignedPaymentV2 {
    let unsigned = UnsignedPaymentV2 {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: PAYMENT_V2_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from: address_from_public_key(&key_pair.public_key),
        to: to.to_string(),
        amount,
        fee,
        sequence,
        memos,
    };
    let signature =
        ml_dsa_65_sign(&key_pair.private_key, &unsigned.signing_bytes()).expect("sign payment");
    SignedPaymentV2 {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&key_pair.public_key),
        signature_hex: bytes_to_hex(&signature),
    }
}

fn deposit_memo() -> PaymentMemo {
    PaymentMemo {
        memo_type: "6465706f736974".to_string(),
        memo_format: String::new(),
        memo_data: "637573742d3432".to_string(),
    }
}

struct BatchFixture {
    genesis: Genesis,
    ledger: LedgerState,
    sender_key: MlDsa65KeyPair,
    sender: String,
    exchange: String,
}

fn batch_fixture() -> BatchFixture {
    let genesis = Genesis::new("postfiat-local");
    let sender_key = key(21);
    let exchange_key = key(22);
    let sender = address_from_public_key(&sender_key.public_key);
    let exchange = address_from_public_key(&exchange_key.public_key);
    let ledger = LedgerState::new(vec![
        Account::new(sender.clone(), 1_000, Some(bytes_to_hex(&sender_key.public_key))),
        Account::new(
            exchange.clone(),
            1_000,
            Some(bytes_to_hex(&exchange_key.public_key)),
        ),
    ]);
    BatchFixture {
        genesis,
        ledger,
        sender_key,
        sender,
        exchange,
    }
}

fn live_and_replayed(
    fixture: &BatchFixture,
    ledger: &LedgerState,
    batch: &TransactionBatch,
    governance: &GovernanceState,
    height: u64,
) -> (Vec<Receipt>, LedgerState) {
    let compatibility =
        asset_execution_compatibility_for_genesis_and_governance(&fixture.genesis, governance);
    let mut live_ledger = ledger.clone();
    let live = execute_transparent_batch(
        &fixture.genesis,
        governance,
        &mut live_ledger,
        batch,
        height,
        compatibility,
    );
    let mut block = dummy_block_record(height);
    block.receipt_ids = live.iter().map(|receipt| receipt.tx_id.clone()).collect();
    block.header.receipt_count = live.len() as u64;
    let mut replay_ledger = ledger.clone();
    let replayed = execute_transparent_batch_for_archive_replay(
        &fixture.genesis,
        &mut replay_ledger,
        batch,
        &block,
        governance,
        &[],
    )
    .expect("archive replay");
    assert_eq!(
        serde_json::to_vec(&replayed).unwrap(),
        serde_json::to_vec(&live).unwrap(),
        "archive replay must reproduce the live receipts byte-for-byte"
    );
    assert_eq!(replay_ledger, live_ledger);
    let live_before: u128 = ledger.accounts.iter().map(|a| u128::from(a.balance)).sum();
    let live_after: u128 = live_ledger
        .accounts
        .iter()
        .map(|a| u128::from(a.balance))
        .sum();
    verify_native_pft_transition(height, live_before, live_after, &live)
        .expect("native supply conservation with fee-claimed receipts");
    (live, live_ledger)
}

#[test]
fn transparent_batch_claims_rejected_fees_only_from_the_governed_height() {
    let fixture = batch_fixture();
    // Overdrawn transfer (effect-level) plus a future-sequence payment
    // (sequence failure) in one certified batch.
    let overdrawn = signed_transfer_for(
        &fixture.genesis,
        &fixture.sender_key,
        &fixture.exchange,
        50_000,
        40,
        1,
    );
    let future_sequence = signed_payment_for(
        &fixture.genesis,
        &fixture.sender_key,
        &fixture.exchange,
        5,
        40,
        9,
        vec![deposit_memo()],
    );
    let mut batch = TransactionBatch::new("fee-claim-batch", vec![overdrawn.clone()]);
    batch.payments_v2.push(future_sequence);

    let governance = governed(&[GOVERNANCE_KIND_REJECTED_TX_FEE_V2_ACTIVATION_HEIGHT]);
    assert_eq!(
        governance.rejected_tx_fee_v2_activation_height(),
        Some(u64::from(ACTIVATION_HEIGHT))
    );

    // Pre-activation (and with no amendment at all) the batch is free and
    // its receipts are identical to the receipts of the pre-change executor.
    for (governance, height) in [
        (GovernanceState::new(1), u64::from(ACTIVATION_HEIGHT)),
        (governance.clone(), u64::from(ACTIVATION_HEIGHT) - 1),
    ] {
        let (receipts, ledger) =
            live_and_replayed(&fixture, &fixture.ledger, &batch, &governance, height);
        let mut legacy_ledger = fixture.ledger.clone();
        let legacy = vec![
            execute_transfer(&fixture.genesis, &mut legacy_ledger, &batch.transactions[0]),
            postfiat_execution::execute_payment_v2(
                &fixture.genesis,
                &mut legacy_ledger,
                &batch.payments_v2[0],
            ),
        ];
        assert_eq!(
            serde_json::to_vec(&receipts).unwrap(),
            serde_json::to_vec(&legacy).unwrap()
        );
        assert_eq!(ledger, fixture.ledger);
        assert_eq!(legacy_ledger, fixture.ledger);
    }

    let (receipts, ledger) = live_and_replayed(
        &fixture,
        &fixture.ledger,
        &batch,
        &governance,
        u64::from(ACTIVATION_HEIGHT),
    );
    assert_eq!(receipts[0].code, "tec_insufficient_funds");
    assert_eq!(receipts[0].fee_burned, 40);
    assert_eq!(receipts[1].code, "bad_sequence");
    assert_eq!(receipts[1].fee_burned, 0);
    let sender = ledger.account(&fixture.sender).expect("sender");
    assert_eq!(sender.balance, 960);
    assert_eq!(sender.sequence, 1);
    assert_eq!(
        ledger.account(&fixture.exchange).expect("exchange").balance,
        1_000
    );
}

#[test]
fn transparent_batch_enforces_require_memo_after_activation() {
    let fixture = batch_fixture();
    let exchange_key = key(22);
    let flag_batch = |sequence: u64, set_flags: u32, clear_flags: u32| {
        let unsigned = postfiat_types::UnsignedAssetTransaction {
            chain_id: fixture.genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&fixture.genesis),
            protocol_version: fixture.genesis.protocol_version,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            transaction_kind: ACCOUNT_SET_TRANSACTION_KIND.to_string(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            source: fixture.exchange.clone(),
            fee: 40,
            sequence,
            operation: AssetTransactionOperation::AccountSet(AccountSetOperation {
                account: fixture.exchange.clone(),
                set_flags,
                clear_flags,
            }),
        };
        let signature = ml_dsa_65_sign(&exchange_key.private_key, &unsigned.signing_bytes())
            .expect("sign account_set");
        let mut batch = TransactionBatch::new("account-set-batch", Vec::new());
        batch.asset_transactions.push(SignedAssetTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: bytes_to_hex(&exchange_key.public_key),
            signature_hex: bytes_to_hex(&signature),
        });
        batch
    };
    let governance = governed(&[
        GOVERNANCE_KIND_REJECTED_TX_FEE_V2_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_REQUIRE_MEMO_V2_ACTIVATION_HEIGHT,
    ]);
    let height = u64::from(ACTIVATION_HEIGHT);

    // Before activation account_set is disabled and free.
    let (receipts, ledger) = live_and_replayed(
        &fixture,
        &fixture.ledger,
        &flag_batch(1, ACCOUNT_FLAG_REQUIRE_MEMO, 0),
        &governance,
        height - 1,
    );
    assert_eq!(receipts[0].code, "account_set_not_active");
    assert_eq!(ledger, fixture.ledger);

    // After activation the owner sets the flag.
    let (receipts, flagged) = live_and_replayed(
        &fixture,
        &fixture.ledger,
        &flag_batch(1, ACCOUNT_FLAG_REQUIRE_MEMO, 0),
        &governance,
        height,
    );
    assert!(receipts[0].accepted, "{receipts:?}");
    assert!(flagged.account(&fixture.exchange).unwrap().requires_memo());

    // A memo-less deposit is rejected and its fee claimed; a memo deposit lands.
    let memo_less = signed_payment_for(
        &fixture.genesis,
        &fixture.sender_key,
        &fixture.exchange,
        25,
        40,
        1,
        Vec::new(),
    );
    let mut batch = TransactionBatch::new("memo-less-deposit", Vec::new());
    batch.payments_v2.push(memo_less);
    let (receipts, after) = live_and_replayed(&fixture, &flagged, &batch, &governance, height);
    assert_eq!(receipts[0].code, "tec_destination_memo_required");
    assert_eq!(after.account(&fixture.exchange).unwrap().balance, flagged.account(&fixture.exchange).unwrap().balance);
    assert_eq!(after.account(&fixture.sender).unwrap().balance, 960);

    let with_memo = signed_payment_for(
        &fixture.genesis,
        &fixture.sender_key,
        &fixture.exchange,
        25,
        40,
        1,
        vec![deposit_memo()],
    );
    let mut batch = TransactionBatch::new("memo-deposit", Vec::new());
    batch.payments_v2.push(with_memo);
    let (receipts, after) = live_and_replayed(&fixture, &flagged, &batch, &governance, height);
    assert!(receipts[0].accepted, "{receipts:?}");
    assert_eq!(
        after.account(&fixture.exchange).unwrap().balance,
        flagged.account(&fixture.exchange).unwrap().balance + 25
    );

    // The owner clears the flag; memo-less deposits are accepted again.
    let (receipts, cleared) = live_and_replayed(
        &fixture,
        &flagged,
        &flag_batch(2, 0, ACCOUNT_FLAG_REQUIRE_MEMO),
        &governance,
        height,
    );
    assert!(receipts[0].accepted, "{receipts:?}");
    assert_eq!(cleared.account(&fixture.exchange).unwrap().flags, 0);
    let transfer = signed_transfer_for(
        &fixture.genesis,
        &fixture.sender_key,
        &fixture.exchange,
        25,
        40,
        1,
    );
    let batch = TransactionBatch::new("plain-transfer", vec![transfer]);
    let (receipts, _) = live_and_replayed(&fixture, &cleared, &batch, &governance, height);
    assert!(receipts[0].accepted, "{receipts:?}");
}

#[test]
fn mempool_admission_rejects_memo_less_payment_to_flagged_account_without_charging() {
    let data_dir = unique_test_dir("postfiat-require-memo-mempool");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-require-memo-mempool".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init require-memo store");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let mut governance = store.read_governance().expect("governance");
    governance.apply(activation_amendment(GOVERNANCE_KIND_REQUIRE_MEMO_V2_ACTIVATION_HEIGHT, 1));
    store.write_governance(&governance).expect("write governance");
    let sender_key = key(31);
    let sender = address_from_public_key(&sender_key.public_key);
    let exchange = address_from_public_key(&key(32).public_key);
    let mut ledger = store.read_ledger().expect("ledger");
    ledger.accounts.push(Account::new(sender.clone(), 1_000, None));
    let mut exchange_account = Account::new(exchange.clone(), 1_000, None);
    exchange_account.flags = ACCOUNT_FLAG_REQUIRE_MEMO;
    ledger.accounts.push(exchange_account);
    store.write_ledger(&ledger).expect("write ledger");

    let memo_less = signed_payment_for(&genesis, &sender_key, &exchange, 25, 40, 1, Vec::new());
    let error = submit_signed_payment_v2_json_to_mempool(SignedPaymentV2JsonSubmitOptions {
        data_dir: data_dir.clone(),
        signed_payment_v2_json: serde_json::to_string(&memo_less).unwrap(),
    })
    .expect_err("memo-less payment to a flagged account admitted");
    assert!(error.to_string().contains("destination_memo_required"), "{error}");
    assert!(store.read_mempool().expect("mempool").is_empty());
    assert_eq!(store.read_ledger().expect("ledger"), ledger);

    let with_memo =
        signed_payment_for(&genesis, &sender_key, &exchange, 25, 40, 1, vec![deposit_memo()]);
    submit_signed_payment_v2_json_to_mempool(SignedPaymentV2JsonSubmitOptions {
        data_dir: data_dir.clone(),
        signed_payment_v2_json: serde_json::to_string(&with_memo).unwrap(),
    })
    .expect("memo payment admitted");
    std::fs::remove_dir_all(data_dir).expect("remove require-memo store");
}

#[test]
fn new_activation_kinds_must_be_scheduled_after_the_amendment_block() {
    for (kind, code) in [
        (
            GOVERNANCE_KIND_REJECTED_TX_FEE_V2_ACTIVATION_HEIGHT,
            "invalid_rejected_tx_fee_v2_activation_height",
        ),
        (
            GOVERNANCE_KIND_REQUIRE_MEMO_V2_ACTIVATION_HEIGHT,
            "invalid_require_memo_v2_activation_height",
        ),
    ] {
        let amendment = activation_amendment(kind, 2);
        assert!(governance_amendment_lifecycle_rejection(&amendment, 1).is_none());
        assert_eq!(
            governance_amendment_lifecycle_rejection(&amendment, 2).map(|rejection| rejection.0),
            Some(code)
        );
        let mut governance = GovernanceState::new(1);
        apply_governance_amendment_with_lifecycle_records(&mut governance, amendment, "batch", 1);
        assert_eq!(governance.amendment_activation_records.len(), 1);
        assert_eq!(governance_amendment_current_value(&governance, kind), 2);
    }
}

#[test]
fn recipient_failures_claim_fees_with_both_gates_and_replay_identically() {
    let fixture = batch_fixture();
    let address = postfiat_crypto_provider::checksum_address(&fixture.exchange).unwrap();
    let mut bad_checksum = address.as_bytes().to_vec();
    let index = bad_checksum.iter().enumerate().skip(2)
        .find(|(_, byte)| byte.is_ascii_alphabetic()).unwrap().0;
    bad_checksum[index] ^= 0x20;
    let bad_checksum = String::from_utf8(bad_checksum).unwrap();
    let governance = governed(&[
        GOVERNANCE_KIND_RECIPIENT_FORMAT_V2_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_REJECTED_TX_FEE_V2_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_REQUIRE_MEMO_V2_ACTIVATION_HEIGHT,
    ]);
    for (destination, code) in [
        ("pf-malformed", "invalid_recipient_address"),
        (address.as_str(), "noncanonical_recipient_address"),
        (bad_checksum.as_str(), "invalid_recipient_checksum"),
    ] {
        let transfer = signed_transfer_for(
            &fixture.genesis, &fixture.sender_key, destination, 25, 40, 1,
        );
        let batch = TransactionBatch::new("recipient-fee-integration", vec![transfer]);
        let (receipts, ledger) = live_and_replayed(
            &fixture, &fixture.ledger, &batch, &governance, u64::from(ACTIVATION_HEIGHT),
        );
        assert_eq!(receipts[0].code, format!("tec_{code}"));
        assert_eq!(receipts[0].fee_burned, 40);
        assert_eq!(ledger.account(&fixture.sender).unwrap().sequence, 1);
        assert_eq!(ledger.account(&fixture.sender).unwrap().balance, 960);
        assert!(ledger.account(destination).is_none());
    }
}

#[test]
fn issued_payment_v2_uses_recipient_validation_and_exposes_memos() {
    let fixture = batch_fixture();
    let unsigned = postfiat_types::UnsignedAssetTransaction {
        chain_id: fixture.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&fixture.genesis),
        protocol_version: fixture.genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: postfiat_types::ISSUED_PAYMENT_V2_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: fixture.sender.clone(),
        fee: 40,
        sequence: 1,
        operation: AssetTransactionOperation::IssuedPaymentV2(postfiat_types::IssuedPaymentV2Operation {
            from: fixture.sender.clone(), to: "pf-malformed".to_string(),
            issuer: fixture.sender.clone(), asset_id: "ab".repeat(48), amount: 25,
            memos: vec![deposit_memo()],
        }),
    };
    let signature = ml_dsa_65_sign(&fixture.sender_key.private_key, &unsigned.signing_bytes()).unwrap();
    let transaction = SignedAssetTransaction {
        unsigned, algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: bytes_to_hex(&fixture.sender_key.public_key), signature_hex: bytes_to_hex(&signature),
    };
    assert_eq!(
        destination_address_rejection(&asset_transaction_destination_addresses(&transaction)).unwrap().0,
        "invalid_recipient_address",
    );
    let mut batch = TransactionBatch::new("issued-memo-integration", Vec::new());
    batch.asset_transactions.push(transaction);
    let governance = governed(&[
        GOVERNANCE_KIND_RECIPIENT_FORMAT_V2_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_REJECTED_TX_FEE_V2_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_REQUIRE_MEMO_V2_ACTIVATION_HEIGHT,
    ]);
    let (receipts, ledger) = live_and_replayed(
        &fixture, &fixture.ledger, &batch, &governance, u64::from(ACTIVATION_HEIGHT),
    );
    assert_eq!(receipts[0].code, "tec_invalid_recipient_address");
    assert_eq!(ledger.account(&fixture.sender).unwrap().balance, 960);
    assert!(ledger.trustlines.is_empty());

    let mut block = dummy_block_record(u64::from(ACTIVATION_HEIGHT));
    block.header.batch_kind = BATCH_KIND_TRANSPARENT.to_string();
    block.receipt_ids = receipts.iter().map(|receipt| receipt.tx_id.clone()).collect();
    let archive = BatchArchiveEntry {
        batch_kind: BATCH_KIND_TRANSPARENT.to_string(), batch_id: block.header.batch_id.clone(),
        payload_hash: "00".repeat(48),
        payload_json: serde_json::to_string(&batch).unwrap(),
    };
    let by_tx = receipts.iter().map(|receipt| (receipt.tx_id.as_str(), receipt)).collect();
    let rows = account_tx_rows_for_archived_block(&block, &archive, &by_tx).unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].memo_count, Some(1));
    assert_eq!(rows[0].memos.as_ref().unwrap()[0].memo_data_text.as_deref(), Some("cust-42"));
}

fn fixture_root(fixture: &BatchFixture, governance: &GovernanceState, ledger: &LedgerState) -> String {
    replicated_state_root(
        &fixture.genesis,
        governance,
        ledger,
        &Vec::<String>::new(),
        &ShieldedState::empty(),
        &BridgeState::empty(),
    )
    .expect("state root")
}

/// Release 6.4 has not applied the original three kinds since the 2026-11-12
/// rollout, so with only those kinds committed (as at height 1153 on the live
/// chain) every rule stays off at any later height. The v2 kinds switch each
/// rule on from their own height and not before.
#[test]
fn original_activation_kinds_leave_the_rules_off_until_the_v2_kinds() {
    let fixture = batch_fixture();
    let original = governed(&[
        GOVERNANCE_KIND_RECIPIENT_FORMAT_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_REJECTED_TX_FEE_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_REQUIRE_MEMO_ACTIVATION_HEIGHT,
    ]);
    let v2_height = ACTIVATION_HEIGHT + 500;
    let mut with_v2 = original.clone();
    for kind in [
        GOVERNANCE_KIND_RECIPIENT_FORMAT_V2_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_REJECTED_TX_FEE_V2_ACTIVATION_HEIGHT,
        GOVERNANCE_KIND_REQUIRE_MEMO_V2_ACTIVATION_HEIGHT,
    ] {
        with_v2.apply(activation_amendment(kind, v2_height));
    }
    let v2_height = u64::from(v2_height);
    let noncanonical = postfiat_crypto_provider::checksum_address(&fixture.exchange).unwrap();
    let mut flagged = fixture.ledger.clone();
    flagged
        .accounts
        .iter_mut()
        .find(|account| account.address == fixture.exchange)
        .unwrap()
        .flags = ACCOUNT_FLAG_REQUIRE_MEMO;
    let overdrawn =
        signed_transfer_for(&fixture.genesis, &fixture.sender_key, &fixture.exchange, 50_000, 40, 1);
    let to_noncanonical =
        signed_transfer_for(&fixture.genesis, &fixture.sender_key, &noncanonical, 25, 40, 1);
    let memo_less =
        signed_payment_for(&fixture.genesis, &fixture.sender_key, &fixture.exchange, 25, 40, 1, Vec::new());

    // Off: original kinds at any later height, and v2 kinds one block early.
    for (governance, height) in [
        (&original, u64::from(ACTIVATION_HEIGHT) + 1_000),
        (&with_v2, v2_height - 1),
    ] {
        let batch = TransactionBatch::new("fee-off", vec![overdrawn.clone()]);
        let (receipts, ledger) = live_and_replayed(&fixture, &fixture.ledger, &batch, governance, height);
        let mut legacy_ledger = fixture.ledger.clone();
        let legacy = execute_transfer(&fixture.genesis, &mut legacy_ledger, &overdrawn);
        assert_eq!(serde_json::to_vec(&receipts).unwrap(), serde_json::to_vec(&vec![legacy]).unwrap());
        assert_eq!(receipts[0].fee_burned, 0);
        assert_eq!(ledger, fixture.ledger);
        assert_eq!(
            fixture_root(&fixture, governance, &ledger),
            fixture_root(&fixture, governance, &fixture.ledger)
        );

        let batch = TransactionBatch::new("recipient-off", vec![to_noncanonical.clone()]);
        let (receipts, ledger) = live_and_replayed(&fixture, &fixture.ledger, &batch, governance, height);
        let mut legacy_ledger = fixture.ledger.clone();
        let legacy = execute_transfer(&fixture.genesis, &mut legacy_ledger, &to_noncanonical);
        assert!(legacy.accepted, "{legacy:?}");
        assert_eq!(serde_json::to_vec(&receipts).unwrap(), serde_json::to_vec(&vec![legacy]).unwrap());
        assert_eq!(ledger, legacy_ledger);

        let mut batch = TransactionBatch::new("memo-off", Vec::new());
        batch.payments_v2.push(memo_less.clone());
        let (receipts, ledger) = live_and_replayed(&fixture, &flagged, &batch, governance, height);
        let mut legacy_ledger = flagged.clone();
        let legacy =
            postfiat_execution::execute_payment_v2(&fixture.genesis, &mut legacy_ledger, &memo_less);
        assert!(legacy.accepted, "{legacy:?}");
        assert_eq!(serde_json::to_vec(&receipts).unwrap(), serde_json::to_vec(&vec![legacy]).unwrap());
        assert_eq!(ledger, legacy_ledger);
    }

    // On from the v2 height.
    let batch = TransactionBatch::new("fee-on", vec![overdrawn]);
    let (receipts, ledger) = live_and_replayed(&fixture, &fixture.ledger, &batch, &with_v2, v2_height);
    assert_eq!(receipts[0].code, "tec_insufficient_funds");
    assert_eq!(receipts[0].fee_burned, 40);
    assert_eq!(ledger.account(&fixture.sender).unwrap().balance, 960);
    let batch = TransactionBatch::new("recipient-on", vec![to_noncanonical]);
    let (receipts, _) = live_and_replayed(&fixture, &fixture.ledger, &batch, &with_v2, v2_height);
    assert_eq!(receipts[0].code, "tec_noncanonical_recipient_address");
    let mut batch = TransactionBatch::new("memo-on", Vec::new());
    batch.payments_v2.push(memo_less);
    let (receipts, _) = live_and_replayed(&fixture, &flagged, &batch, &with_v2, v2_height);
    assert_eq!(receipts[0].code, "tec_destination_memo_required");
}
