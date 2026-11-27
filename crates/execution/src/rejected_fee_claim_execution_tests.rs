// Rejected-transaction fee claim (`tec_` receipts) and the require-memo
// account flag. Both rules are governed by activation heights carried in
// `AssetExecutionCompatibility`; these tests drive the execution entry points
// exactly as the transparent batch executor does (execute, then claim).

const FEE_CLAIM_HEIGHT: u64 = 50;

fn fee_claim_rules(activation_height: Option<u64>) -> AssetExecutionCompatibility {
    AssetExecutionCompatibility::strict().with_rejected_tx_fee_activation_height(activation_height)
}

fn memo_and_fee_claim_rules(height: u64) -> AssetExecutionCompatibility {
    fee_claim_rules(Some(height)).with_require_memo_activation_height(Some(height))
}

fn transfer_in_block(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transfer: &SignedTransfer,
    block_height: u64,
    rules: AssetExecutionCompatibility,
) -> Receipt {
    let receipt = execute_transfer_with_compatibility(genesis, ledger, transfer, block_height, rules);
    claim_rejected_transfer_fee(genesis, ledger, transfer, receipt, block_height, rules)
}

fn payment_v2_in_block(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    payment: &SignedPaymentV2,
    block_height: u64,
    rules: AssetExecutionCompatibility,
) -> Receipt {
    let receipt =
        execute_payment_v2_with_compatibility(genesis, ledger, payment, block_height, rules);
    claim_rejected_payment_v2_fee(genesis, ledger, payment, receipt, block_height, rules)
}

fn asset_in_block(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    transaction: &SignedAssetTransaction,
    block_height: u64,
    rules: AssetExecutionCompatibility,
) -> Receipt {
    let receipt = execute_asset_transaction_with_compatibility(
        genesis,
        ledger,
        transaction,
        block_height,
        rules,
    );
    claim_rejected_asset_transaction_fee(genesis, ledger, transaction, receipt, block_height, rules)
}

fn seeded_key(seed: u8) -> MlDsa65KeyPair {
    ml_dsa_65_keygen_from_seed(&[seed; 32])
}

fn receipt_bytes(receipt: &Receipt) -> Vec<u8> {
    serde_json::to_vec(receipt).expect("serialize receipt")
}

struct FeeClaimFixture {
    genesis: Genesis,
    ledger: LedgerState,
    sender_key: MlDsa65KeyPair,
    sender: String,
    recipient: String,
}

/// Sender with an unbound key (first spend binds it) and a funded recipient.
fn fee_claim_fixture(sender_balance: u64) -> FeeClaimFixture {
    let genesis = Genesis::new("postfiat-local");
    let sender_key = seeded_key(7);
    let recipient_key = seeded_key(8);
    let sender = address_from_public_key(&sender_key.public_key);
    let recipient = address_from_public_key(&recipient_key.public_key);
    let ledger = LedgerState::new(vec![
        Account::new(sender.clone(), sender_balance, None),
        Account::new(
            recipient.clone(),
            1_000,
            Some(bytes_to_hex(&recipient_key.public_key)),
        ),
    ]);
    FeeClaimFixture {
        genesis,
        ledger,
        sender_key,
        sender,
        recipient,
    }
}

fn set_require_memo(ledger: &mut LedgerState, address: &str) {
    ledger.account_mut(address).expect("flagged account").flags =
        postfiat_types::ACCOUNT_FLAG_REQUIRE_MEMO;
}

#[test]
fn pre_activation_rejections_and_acceptances_replay_byte_for_byte() {
    let fixture = fee_claim_fixture(100);
    let unfunded = signed_transfer_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        10_000,
        1,
    );
    let funded = signed_transfer_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        5,
        1,
    );
    let memo_less = signed_payment_v2_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        10_000,
        1,
        Vec::new(),
    );
    // A recipient flag can only be set after require-memo activation, but even
    // a flag present in state must not change pre-activation execution.
    let mut flagged_ledger = fixture.ledger.clone();
    set_require_memo(&mut flagged_ledger, &fixture.recipient);
    let memo_less_funded = signed_payment_v2_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        5,
        1,
        Vec::new(),
    );

    for rules in [
        AssetExecutionCompatibility::strict(),
        fee_claim_rules(None),
        // Ratified but not yet reached.
        memo_and_fee_claim_rules(FEE_CLAIM_HEIGHT + 1),
    ] {
        for transfer in [&unfunded, &funded] {
            let mut legacy_ledger = fixture.ledger.clone();
            let legacy = execute_transfer(&fixture.genesis, &mut legacy_ledger, transfer);
            let mut governed_ledger = fixture.ledger.clone();
            let governed = transfer_in_block(
                &fixture.genesis,
                &mut governed_ledger,
                transfer,
                FEE_CLAIM_HEIGHT,
                rules,
            );
            assert_eq!(receipt_bytes(&governed), receipt_bytes(&legacy));
            assert_eq!(governed_ledger, legacy_ledger);
        }
        let mut legacy_ledger = fixture.ledger.clone();
        let legacy = execute_payment_v2(&fixture.genesis, &mut legacy_ledger, &memo_less);
        let mut governed_ledger = fixture.ledger.clone();
        let governed = payment_v2_in_block(
            &fixture.genesis,
            &mut governed_ledger,
            &memo_less,
            FEE_CLAIM_HEIGHT,
            rules,
        );
        assert_eq!(legacy.code, "insufficient_funds");
        assert_eq!(receipt_bytes(&governed), receipt_bytes(&legacy));
        assert_eq!(governed_ledger, fixture.ledger);
        assert_eq!(governed_ledger, legacy_ledger);

        let mut legacy_ledger = flagged_ledger.clone();
        let legacy = execute_payment_v2(&fixture.genesis, &mut legacy_ledger, &memo_less_funded);
        let mut governed_ledger = flagged_ledger.clone();
        let governed = payment_v2_in_block(
            &fixture.genesis,
            &mut governed_ledger,
            &memo_less_funded,
            FEE_CLAIM_HEIGHT,
            rules,
        );
        assert!(legacy.accepted, "{legacy:?}");
        assert_eq!(receipt_bytes(&governed), receipt_bytes(&legacy));
        assert_eq!(governed_ledger, legacy_ledger);
    }
}

#[test]
fn post_activation_effect_rejection_claims_fee_burns_it_and_consumes_sequence() {
    let fixture = fee_claim_fixture(100);
    let transfer = signed_transfer_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        10_000,
        1,
    );
    let fee = transfer.unsigned.fee;
    let mut ledger = fixture.ledger.clone();
    let supply_before: u64 = ledger.accounts.iter().map(|account| account.balance).sum();

    let receipt = transfer_in_block(
        &fixture.genesis,
        &mut ledger,
        &transfer,
        FEE_CLAIM_HEIGHT,
        fee_claim_rules(Some(FEE_CLAIM_HEIGHT)),
    );

    assert!(!receipt.accepted, "{receipt:?}");
    assert_eq!(receipt.code, "tec_insufficient_funds");
    assert!(receipt.is_fee_claimed());
    assert_eq!(receipt.tx_id, transfer_tx_id(&transfer));
    assert_eq!(receipt.fee_charged, fee);
    assert_eq!(receipt.fee_burned, fee);
    assert_eq!(receipt.minimum_fee, minimum_transfer_fee_for_ledger(&fixture.ledger, &transfer));
    assert_eq!(receipt.account_reserve, ACCOUNT_RESERVE);
    let sender = ledger.account(&fixture.sender).expect("sender");
    assert_eq!(sender.balance, 100 - fee);
    assert_eq!(sender.sequence, 1);
    assert_eq!(
        sender.public_key_hex.as_deref(),
        Some(transfer.public_key_hex.as_str())
    );
    assert_eq!(ledger.account(&fixture.recipient).expect("recipient").balance, 1_000);
    let supply_after: u64 = ledger.accounts.iter().map(|account| account.balance).sum();
    assert_eq!(supply_before - supply_after, receipt.fee_burned);
    assert!(ledger.account(FEE_COLLECTOR_ADDRESS).is_none());

    // The sequence is consumed: the same transaction cannot be charged twice.
    let after_claim = ledger.clone();
    let replay = transfer_in_block(
        &fixture.genesis,
        &mut ledger,
        &transfer,
        FEE_CLAIM_HEIGHT + 1,
        fee_claim_rules(Some(FEE_CLAIM_HEIGHT)),
    );
    assert_eq!(replay.code, "bad_sequence");
    assert_eq!(replay.fee_charged, 0);
    assert_eq!(ledger, after_claim);

    // The next sequence executes normally.
    let next = signed_transfer_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        5,
        2,
    );
    let accepted = transfer_in_block(
        &fixture.genesis,
        &mut ledger,
        &next,
        FEE_CLAIM_HEIGHT + 1,
        fee_claim_rules(Some(FEE_CLAIM_HEIGHT)),
    );
    assert!(accepted.accepted, "{accepted:?}");
}

#[test]
fn post_activation_authentication_sequence_and_fee_failures_are_never_charged() {
    let fixture = fee_claim_fixture(100);
    let rules = fee_claim_rules(Some(FEE_CLAIM_HEIGHT));
    let valid = signed_transfer_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        10_000,
        1,
    );

    let mut bad_signature = valid.clone();
    let mut signature = hex_to_bytes(&bad_signature.signature_hex).expect("signature hex");
    signature[0] ^= 0x01;
    bad_signature.signature_hex = bytes_to_hex(&signature);

    let mut wrong_key = valid.clone();
    wrong_key.public_key_hex = bytes_to_hex(&seeded_key(9).public_key);

    let future_sequence = signed_transfer_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        10_000,
        2,
    );
    let fee_too_low = signed_transfer(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        10_000,
        1,
        1,
    );
    let other_chain = Genesis::new("postfiat-other");
    let wrong_chain = signed_transfer_with_minimum_fee(
        &other_chain,
        &fixture.sender_key,
        fixture.recipient.clone(),
        10_000,
        1,
    );
    let mut malformed = valid.clone();
    malformed.signature_hex = "zz".to_string();

    for (transfer, expected_code) in [
        (&bad_signature, "bad_signature"),
        (&wrong_key, "sender_mismatch"),
        (&future_sequence, "bad_sequence"),
        (&fee_too_low, "fee_too_low"),
        (&wrong_chain, "wrong_chain"),
        (&malformed, "bad_transfer_envelope"),
    ] {
        let mut ledger = fixture.ledger.clone();
        let receipt =
            transfer_in_block(&fixture.genesis, &mut ledger, transfer, FEE_CLAIM_HEIGHT, rules);
        assert_eq!(receipt.code, expected_code, "{receipt:?}");
        assert!(!receipt.is_fee_claimed());
        assert_eq!(receipt.fee_charged, 0);
        assert_eq!(receipt.fee_burned, 0);
        assert_eq!(ledger, fixture.ledger, "{expected_code} must not mutate");
    }

    // A bound key that differs from the signing key is an authentication
    // failure, not an effect failure.
    let mut bound_ledger = fixture.ledger.clone();
    bound_ledger.account_mut(&fixture.sender).expect("sender").public_key_hex =
        Some(bytes_to_hex(&seeded_key(10).public_key));
    let before = bound_ledger.clone();
    let receipt = transfer_in_block(&fixture.genesis, &mut bound_ledger, &valid, FEE_CLAIM_HEIGHT, rules);
    assert_eq!(receipt.code, "sender_key_mismatch");
    assert_eq!(bound_ledger, before);

    // Fee affordability: the sender cannot pay the fee and keep the reserve.
    let poor = fee_claim_fixture(valid.unsigned.fee + ACCOUNT_RESERVE - 1);
    let mut ledger = poor.ledger.clone();
    let receipt = transfer_in_block(&poor.genesis, &mut ledger, &valid, FEE_CLAIM_HEIGHT, rules);
    assert_eq!(receipt.code, "insufficient_funds");
    assert!(!receipt.is_fee_claimed());
    assert_eq!(ledger, poor.ledger);

    // Exactly affordable: fee is claimed and the reserve is kept.
    let exact = fee_claim_fixture(valid.unsigned.fee + ACCOUNT_RESERVE);
    let mut ledger = exact.ledger.clone();
    let receipt = transfer_in_block(&exact.genesis, &mut ledger, &valid, FEE_CLAIM_HEIGHT, rules);
    assert_eq!(receipt.code, "tec_insufficient_funds");
    assert_eq!(
        ledger.account(&exact.sender).expect("sender").balance,
        ACCOUNT_RESERVE
    );
}

#[test]
fn asset_escrow_nft_offer_and_atomic_swap_effect_rejections_claim_fees() {
    let fixture = fee_claim_fixture(1_000);
    let rules = fee_claim_rules(Some(FEE_CLAIM_HEIGHT));

    let missing_asset = signed_asset_transaction_with_minimum_fee(
        &fixture.genesis,
        &fixture.ledger,
        &fixture.sender_key,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: fixture.sender.clone(),
            to: fixture.recipient.clone(),
            issuer: fixture.recipient.clone(),
            asset_id: "ab".repeat(48),
            amount: 5,
        }),
    );
    let mut ledger = fixture.ledger.clone();
    let receipt = asset_in_block(&fixture.genesis, &mut ledger, &missing_asset, FEE_CLAIM_HEIGHT, rules);
    assert!(receipt.is_fee_claimed(), "{receipt:?}");
    assert_eq!(receipt.fee_burned, missing_asset.unsigned.fee);
    let sender = ledger.account(&fixture.sender).expect("sender");
    assert_eq!(sender.balance, 1_000 - missing_asset.unsigned.fee);
    assert_eq!(sender.sequence, 1);
    assert!(ledger.trustlines.is_empty());

    // Pre-activation the same asset rejection is free.
    let mut ledger = fixture.ledger.clone();
    let receipt = asset_in_block(
        &fixture.genesis,
        &mut ledger,
        &missing_asset,
        FEE_CLAIM_HEIGHT - 1,
        rules,
    );
    assert!(!receipt.is_fee_claimed());
    assert_eq!(ledger, fixture.ledger);

    let escrow_cancel = signed_escrow_transaction_with_minimum_fee(
        &fixture.genesis,
        &fixture.ledger,
        &fixture.sender_key,
        ESCROW_CANCEL_TRANSACTION_KIND,
        1,
        EscrowTransactionOperation::EscrowCancel(EscrowCancelOperation {
            escrow_id: "cd".repeat(48),
            owner: fixture.sender.clone(),
        }),
    );
    let mut ledger = fixture.ledger.clone();
    let receipt = execute_escrow_transaction(&fixture.genesis, &mut ledger, &escrow_cancel, FEE_CLAIM_HEIGHT);
    let receipt = claim_rejected_escrow_transaction_fee(
        &fixture.genesis,
        &mut ledger,
        &escrow_cancel,
        receipt,
        FEE_CLAIM_HEIGHT,
        rules,
    );
    assert!(receipt.is_fee_claimed(), "{receipt:?}");
    assert_eq!(
        ledger.account(&fixture.sender).expect("sender").balance,
        1_000 - escrow_cancel.unsigned.fee
    );

    let nft_burn = signed_nft_transaction_with_minimum_fee(
        &fixture.genesis,
        &fixture.ledger,
        &fixture.sender_key,
        NFT_BURN_TRANSACTION_KIND,
        1,
        NftTransactionOperation::NftBurn(NftBurnOperation {
            nft_id: "ef".repeat(48),
            owner: fixture.sender.clone(),
            issuer: String::new(),
        }),
    );
    let mut ledger = fixture.ledger.clone();
    let receipt = execute_nft_transaction(&fixture.genesis, &mut ledger, &nft_burn);
    let receipt = claim_rejected_nft_transaction_fee(
        &fixture.genesis,
        &mut ledger,
        &nft_burn,
        receipt,
        FEE_CLAIM_HEIGHT,
        rules,
    );
    assert!(receipt.is_fee_claimed(), "{receipt:?}");
    assert_eq!(ledger.account(&fixture.sender).expect("sender").sequence, 1);

    let offer_cancel = signed_offer_transaction_with_minimum_fee(
        &fixture.genesis,
        &fixture.ledger,
        &fixture.sender_key,
        OFFER_CANCEL_TRANSACTION_KIND,
        1,
        OfferTransactionOperation::OfferCancel(OfferCancelOperation {
            offer_id: "12".repeat(48),
            owner: fixture.sender.clone(),
        }),
        FEE_CLAIM_HEIGHT,
    );
    let mut ledger = fixture.ledger.clone();
    let receipt =
        execute_offer_transaction(&fixture.genesis, &mut ledger, &offer_cancel, FEE_CLAIM_HEIGHT);
    let receipt = claim_rejected_offer_transaction_fee(
        &fixture.genesis,
        &mut ledger,
        &offer_cancel,
        receipt,
        FEE_CLAIM_HEIGHT,
        rules,
    );
    assert!(receipt.is_fee_claimed(), "{receipt:?}");
    assert_eq!(receipt.fee_burned, offer_cancel.unsigned.fee);

    // Atomic swap: both owners signed; both legs pay and consume sequence.
    let atomic = atomic_execution_fixture();
    let mut wrong_envelope = atomic.transaction.clone();
    wrong_envelope.unsigned.market_envelope_hash = "ff".repeat(48);
    resign_atomic_swap(&mut wrong_envelope, &atomic.owner_0_key, &atomic.owner_1_key);
    let swap_rules = rules;
    let mut ledger = atomic.ledger.clone();
    let receipt = execute_atomic_swap_transaction_with_compatibility(
        &atomic.genesis,
        &mut ledger,
        &wrong_envelope,
        FEE_CLAIM_HEIGHT,
        swap_rules,
    );
    assert_eq!(receipt.code, "wrong_market_envelope");
    let receipt = claim_rejected_atomic_swap_fee(
        &atomic.genesis,
        &mut ledger,
        &wrong_envelope,
        receipt,
        FEE_CLAIM_HEIGHT,
        swap_rules,
    );
    assert_eq!(receipt.code, "tec_wrong_market_envelope");
    let leg_0 = &wrong_envelope.unsigned.leg_0;
    let leg_1 = &wrong_envelope.unsigned.leg_1;
    assert_eq!(receipt.fee_burned, leg_0.fee + leg_1.fee);
    for leg in [leg_0, leg_1] {
        let owner = ledger.account(&leg.owner).expect("swap owner");
        assert_eq!(owner.balance, 10_000 - leg.fee);
        assert_eq!(owner.sequence, leg.sequence);
        assert_eq!(
            atomic_line_balance(&ledger, &leg.owner, &leg.asset_id),
            atomic_line_balance(&atomic.ledger, &leg.owner, &leg.asset_id)
        );
    }
    assert_eq!(receipt.atomic_swap_legs.as_ref().map(Vec::len), Some(2));

    // An expired swap is never charged.
    let mut ledger = atomic.ledger.clone();
    let expired_height = wrong_envelope.unsigned.expires_at_height + 1;
    let receipt = execute_atomic_swap_transaction_with_compatibility(
        &atomic.genesis,
        &mut ledger,
        &wrong_envelope,
        expired_height,
        swap_rules,
    );
    let receipt = claim_rejected_atomic_swap_fee(
        &atomic.genesis,
        &mut ledger,
        &wrong_envelope,
        receipt,
        expired_height,
        swap_rules,
    );
    assert_eq!(receipt.code, "swap_expired");
    assert_eq!(ledger, atomic.ledger);
}

fn account_set(
    fixture: &FeeClaimFixture,
    ledger: &LedgerState,
    sequence: u64,
    set_flags: u32,
    clear_flags: u32,
) -> SignedAssetTransaction {
    signed_asset_transaction_with_minimum_fee(
        &fixture.genesis,
        ledger,
        &fixture.sender_key,
        postfiat_types::ACCOUNT_SET_TRANSACTION_KIND,
        sequence,
        AssetTransactionOperation::AccountSet(postfiat_types::AccountSetOperation {
            account: fixture.sender.clone(),
            set_flags,
            clear_flags,
        }),
    )
}

#[test]
fn account_set_sets_and_clears_require_memo_only_after_activation() {
    let fixture = fee_claim_fixture(1_000);
    let flag = postfiat_types::ACCOUNT_FLAG_REQUIRE_MEMO;
    let mut ledger = fixture.ledger.clone();

    // Before require-memo activation the transaction kind is disabled, and a
    // disabled feature is never fee-claimed even when fee claims are active.
    let early = account_set(&fixture, &ledger, 1, flag, 0);
    let receipt = asset_in_block(
        &fixture.genesis,
        &mut ledger,
        &early,
        FEE_CLAIM_HEIGHT,
        fee_claim_rules(Some(FEE_CLAIM_HEIGHT))
            .with_require_memo_activation_height(Some(FEE_CLAIM_HEIGHT + 1)),
    );
    assert_eq!(receipt.code, "account_set_not_active");
    assert_eq!(ledger, fixture.ledger);

    let rules = memo_and_fee_claim_rules(FEE_CLAIM_HEIGHT);
    let set = account_set(&fixture, &ledger, 1, flag, 0);
    let receipt = asset_in_block(&fixture.genesis, &mut ledger, &set, FEE_CLAIM_HEIGHT, rules);
    assert!(receipt.accepted, "{receipt:?}");
    let sender = ledger.account(&fixture.sender).expect("sender");
    assert_eq!(sender.flags, flag);
    assert!(sender.requires_memo());
    assert_eq!(sender.sequence, 1);
    assert!(serde_json::to_string(sender).unwrap().contains("\"flags\":1"));

    let clear = account_set(&fixture, &ledger, 2, 0, flag);
    let receipt = asset_in_block(&fixture.genesis, &mut ledger, &clear, FEE_CLAIM_HEIGHT, rules);
    assert!(receipt.accepted, "{receipt:?}");
    let sender = ledger.account(&fixture.sender).expect("sender");
    assert_eq!(sender.flags, 0);
    assert!(!serde_json::to_string(sender).unwrap().contains("flags"));

    // Malformed masks are envelope errors (never charged).
    for (set_flags, clear_flags) in [(0, 0), (0x2, 0), (0, 0x8000_0000), (flag, flag)] {
        let operation = postfiat_types::AccountSetOperation {
            account: fixture.sender.clone(),
            set_flags,
            clear_flags,
        };
        assert!(operation.validate().is_err(), "{set_flags:#x}/{clear_flags:#x}");
    }

    // Another account cannot change the owner's flags.
    let other = postfiat_types::AccountSetOperation {
        account: fixture.recipient.clone(),
        set_flags: flag,
        clear_flags: 0,
    };
    let foreign = signed_asset_transaction(
        &fixture.genesis,
        &fixture.sender_key,
        postfiat_types::ACCOUNT_SET_TRANSACTION_KIND,
        100,
        3,
        AssetTransactionOperation::AccountSet(other),
    );
    let before = ledger.clone();
    let receipt = asset_in_block(&fixture.genesis, &mut ledger, &foreign, FEE_CLAIM_HEIGHT, rules);
    assert_eq!(receipt.code, "bad_asset_transaction_envelope");
    assert_eq!(ledger, before);
}

#[test]
fn require_memo_rejects_memo_less_payments_and_accepts_memo_payments() {
    let fixture = fee_claim_fixture(1_000);
    let rules = memo_and_fee_claim_rules(FEE_CLAIM_HEIGHT);
    let mut flagged = fixture.ledger.clone();
    set_require_memo(&mut flagged, &fixture.recipient);

    // payment_v2 without a memo: rejected; with fee claims active, charged.
    let memo_less = signed_payment_v2_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        25,
        1,
        Vec::new(),
    );
    let mut ledger = flagged.clone();
    let receipt = payment_v2_in_block(&fixture.genesis, &mut ledger, &memo_less, FEE_CLAIM_HEIGHT, rules);
    assert_eq!(receipt.code, "tec_destination_memo_required");
    assert_eq!(receipt.fee_burned, memo_less.unsigned.fee);
    assert_eq!(ledger.account(&fixture.recipient).expect("recipient").balance, 1_000);
    assert_eq!(
        ledger.account(&fixture.sender).expect("sender").balance,
        1_000 - memo_less.unsigned.fee
    );

    // Without the fee-claim gate the same rejection is free.
    let mut ledger = flagged.clone();
    let receipt = payment_v2_in_block(
        &fixture.genesis,
        &mut ledger,
        &memo_less,
        FEE_CLAIM_HEIGHT,
        AssetExecutionCompatibility::strict()
            .with_require_memo_activation_height(Some(FEE_CLAIM_HEIGHT)),
    );
    assert_eq!(receipt.code, "destination_memo_required");
    assert_eq!(ledger, flagged);

    // payment_v2 with a memo: accepted.
    let with_memo = signed_payment_v2_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        25,
        1,
        vec![sample_memo()],
    );
    let mut ledger = flagged.clone();
    let receipt = payment_v2_in_block(&fixture.genesis, &mut ledger, &with_memo, FEE_CLAIM_HEIGHT, rules);
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(ledger.account(&fixture.recipient).expect("recipient").balance, 1_025);

    // transparent_transfer cannot carry a memo, so it cannot pay a flagged account.
    let transfer = signed_transfer_with_minimum_fee(
        &fixture.genesis,
        &fixture.sender_key,
        fixture.recipient.clone(),
        25,
        1,
    );
    let mut ledger = flagged.clone();
    let receipt = transfer_in_block(&fixture.genesis, &mut ledger, &transfer, FEE_CLAIM_HEIGHT, rules);
    assert_eq!(receipt.code, "tec_destination_memo_required");
    assert_eq!(ledger.account(&fixture.recipient).expect("recipient").balance, 1_000);

    // An unflagged recipient still accepts memo-less payments and transfers.
    let mut ledger = fixture.ledger.clone();
    assert!(payment_v2_in_block(&fixture.genesis, &mut ledger, &memo_less, FEE_CLAIM_HEIGHT, rules).accepted);
    let mut ledger = fixture.ledger.clone();
    assert!(transfer_in_block(&fixture.genesis, &mut ledger, &transfer, FEE_CLAIM_HEIGHT, rules).accepted);
}

#[test]
fn require_memo_covers_issued_asset_payments_through_issued_payment_v2() {
    let fixture = fee_claim_fixture(1_000);
    let issuer_key = seeded_key(11);
    let issuer = address_from_public_key(&issuer_key.public_key);
    let rules = memo_and_fee_claim_rules(FEE_CLAIM_HEIGHT);
    let mut ledger = fixture.ledger.clone();
    ledger.accounts.push(Account::new(
        issuer.clone(),
        1_000,
        Some(bytes_to_hex(&issuer_key.public_key)),
    ));
    let create = signed_asset_transaction_with_minimum_fee(
        &fixture.genesis,
        &ledger,
        &issuer_key,
        ASSET_CREATE_TRANSACTION_KIND,
        1,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.clone(),
            code: "USD".to_string(),
            version: 1,
            precision: 6,
            display_name: "US Dollar".to_string(),
            max_supply: Some(1_000),
            requires_authorization: false,
            freeze_enabled: false,
            clawback_enabled: false,
        }),
    );
    assert!(asset_in_block(&fixture.genesis, &mut ledger, &create, FEE_CLAIM_HEIGHT, rules).accepted);
    let asset_id = ledger.asset_definitions[0].asset_id.clone();
    set_require_memo(&mut ledger, &fixture.recipient);

    let v1 = signed_asset_transaction_with_minimum_fee(
        &fixture.genesis,
        &ledger,
        &issuer_key,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        2,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.clone(),
            to: fixture.recipient.clone(),
            issuer: issuer.clone(),
            asset_id: asset_id.clone(),
            amount: 10,
        }),
    );
    let mut scratch = ledger.clone();
    let receipt = asset_in_block(&fixture.genesis, &mut scratch, &v1, FEE_CLAIM_HEIGHT, rules);
    assert_eq!(receipt.code, "tec_destination_memo_required");
    assert!(scratch.trustline_for_account_asset(&fixture.recipient, &asset_id).is_none());

    let v2 = |sequence: u64, memos: Vec<PaymentMemo>, ledger: &LedgerState| {
        signed_asset_transaction_with_minimum_fee(
            &fixture.genesis,
            ledger,
            &issuer_key,
            postfiat_types::ISSUED_PAYMENT_V2_TRANSACTION_KIND,
            sequence,
            AssetTransactionOperation::IssuedPaymentV2(postfiat_types::IssuedPaymentV2Operation {
                from: issuer.clone(),
                to: fixture.recipient.clone(),
                issuer: issuer.clone(),
                asset_id: asset_id.clone(),
                amount: 10,
                memos,
            }),
        )
    };
    let memo_less_v2 = v2(2, Vec::new(), &ledger);
    let mut scratch = ledger.clone();
    let receipt = asset_in_block(&fixture.genesis, &mut scratch, &memo_less_v2, FEE_CLAIM_HEIGHT, rules);
    assert_eq!(receipt.code, "tec_destination_memo_required");

    let with_memo = v2(2, vec![sample_memo()], &ledger);
    let receipt = asset_in_block(&fixture.genesis, &mut ledger, &with_memo, FEE_CLAIM_HEIGHT, rules);
    assert!(receipt.accepted, "{receipt:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&fixture.recipient, &asset_id)
            .expect("recipient line")
            .balance,
        10
    );

    // issued_payment_v2 is part of the require-memo feature and is disabled
    // (and never fee-claimed) before that gate.
    let mut scratch = fixture.ledger.clone();
    scratch.accounts.push(Account::new(
        issuer.clone(),
        1_000,
        Some(bytes_to_hex(&issuer_key.public_key)),
    ));
    let early = v2(1, vec![sample_memo()], &scratch);
    let before = scratch.clone();
    let receipt = asset_in_block(
        &fixture.genesis,
        &mut scratch,
        &early,
        FEE_CLAIM_HEIGHT,
        fee_claim_rules(Some(FEE_CLAIM_HEIGHT)),
    );
    assert_eq!(receipt.code, "issued_payment_v2_not_active");
    assert_eq!(scratch, before);

    // The memo envelope is bound by the signature.
    let mut tampered = with_memo.clone();
    if let AssetTransactionOperation::IssuedPaymentV2(operation) = &mut tampered.unsigned.operation
    {
        operation.memos[0].memo_data = "00".to_string();
    }
    assert_ne!(
        tampered.unsigned.signing_bytes(),
        with_memo.unsigned.signing_bytes()
    );
}

#[test]
fn fee_claim_exclusions_cover_expiry_and_disabled_features_only() {
    for code in FEE_CLAIM_EXCLUDED_REJECTION_CODES {
        assert!(!rejection_code_is_fee_claimable(code));
    }
    for code in [
        "insufficient_funds",
        "below_account_reserve",
        "destination_memo_required",
        "missing_trustline",
        "vault_bridge_route_authority_mismatch",
    ] {
        assert!(rejection_code_is_fee_claimable(code), "{code}");
    }
    assert!(!rejection_code_is_fee_claimable("tec_insufficient_funds"));
}
