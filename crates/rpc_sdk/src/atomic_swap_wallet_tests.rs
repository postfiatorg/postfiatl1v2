#[test]
fn wallet_dual_signs_exact_atomic_quote_without_exposing_a_balance_row_operation() {
    let backup_0 = wallet_backup_from_master_seed("postfiat-local", "11".repeat(32), 0)
        .expect("owner 0 backup");
    let backup_1 = wallet_backup_from_master_seed("postfiat-local", "22".repeat(32), 0)
        .expect("owner 1 backup");
    let owner_0 = wallet_identity_from_backup(&backup_0).expect("owner 0 identity");
    let owner_1 = wallet_identity_from_backup(&backup_1).expect("owner 1 identity");
    let unsigned = postfiat_types::UnsignedAtomicSwapTransaction {
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "aa".repeat(48),
        protocol_version: 1,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        signature_algorithm_id: postfiat_crypto_provider::ML_DSA_65_ALGORITHM.to_string(),
        rfq_hash: "bb".repeat(48),
        market_envelope_hash: "cc".repeat(48),
        nav_epoch: 7,
        expires_at_height: 99,
        swap_nonce: "dd".repeat(48),
        leg_0: postfiat_types::AtomicSwapLeg {
            owner: owner_0.address.clone(),
            recipient: owner_1.address.clone(),
            issuer: format!("pf{}", "33".repeat(20)),
            asset_id: "10".repeat(48),
            amount: 20_000,
            sequence: 3,
            fee: 22,
        },
        leg_1: postfiat_types::AtomicSwapLeg {
            owner: owner_1.address.clone(),
            recipient: owner_0.address.clone(),
            issuer: format!("pf{}", "44".repeat(20)),
            asset_id: "20".repeat(48),
            amount: 164_020,
            sequence: 5,
            fee: 22,
        },
    };
    let quote = AtomicSwapFeeQuoteSummary {
        transaction_kind: postfiat_types::ATOMIC_SWAP_TRANSACTION_KIND.to_string(),
        parent_height: 7,
        parent_hash: "01".repeat(48),
        parent_state_root: "02".repeat(48),
        quote_height: 8,
        account_reserve: 10,
        transfer_fee_byte_quantum: 512,
        transfer_fee_per_quantum: 1,
        atomic_swap_weight_bytes: 4096,
        leg_0: AtomicSwapLegFeeQuoteSummary {
            owner: owner_0.address,
            sender_balance: 1_000,
            sender_sequence: 2,
            sequence: 3,
            mempool_pending_for_owner: 0,
            base_atomic_swap_fee: 20,
            state_expansion_fee: 2,
            minimum_fee: 22,
            sender_balance_after_fee: Some(978),
            sender_meets_reserve_after_fee: true,
        },
        leg_1: AtomicSwapLegFeeQuoteSummary {
            owner: owner_1.address,
            sender_balance: 1_000,
            sender_sequence: 4,
            sequence: 5,
            mempool_pending_for_owner: 0,
            base_atomic_swap_fee: 20,
            state_expansion_fee: 2,
            minimum_fee: 22,
            sender_balance_after_fee: Some(978),
            sender_meets_reserve_after_fee: true,
        },
        unsigned_transaction: unsigned,
    };
    let quote_request = atomic_swap_fee_quote_request(
        "atomic-quote",
        quote.unsigned_transaction.rfq_hash.clone(),
        quote.unsigned_transaction.market_envelope_hash.clone(),
        quote.unsigned_transaction.nav_epoch,
        quote.unsigned_transaction.expires_at_height,
        quote.unsigned_transaction.swap_nonce.clone(),
        quote.unsigned_transaction.leg_0.owner.clone(),
        quote.unsigned_transaction.leg_0.recipient.clone(),
        quote.unsigned_transaction.leg_0.issuer.clone(),
        quote.unsigned_transaction.leg_0.asset_id.clone(),
        quote.unsigned_transaction.leg_0.amount,
        quote.unsigned_transaction.leg_1.owner.clone(),
        quote.unsigned_transaction.leg_1.recipient.clone(),
        quote.unsigned_transaction.leg_1.issuer.clone(),
        quote.unsigned_transaction.leg_1.asset_id.clone(),
        quote.unsigned_transaction.leg_1.amount,
    );
    let signed = wallet_sign_atomic_swap_from_quote(&backup_0, &backup_1, &quote_request, &quote)
        .expect("dual-sign atomic quote");
    assert_eq!(signed.unsigned, quote.unsigned_transaction);
    assert!(signed.validate().is_ok());
    let serialized = serde_json::to_string(&signed).expect("serialize signed atomic swap");
    for forbidden in ["trustline", "trust_set", "line_create"] {
        assert!(!serialized.contains(forbidden), "found `{forbidden}`");
    }

    let reversed = wallet_sign_atomic_swap_from_quote(&backup_1, &backup_0, &quote_request, &quote)
        .expect_err("reversed owners signed the quote");
    assert!(reversed.to_string().contains("leg_0 owner"));

    let mut altered_request = quote_request;
    altered_request.params["leg_1_amount"] =
        serde_json::json!(quote.unsigned_transaction.leg_1.amount + 1);
    let unbound =
        wallet_sign_atomic_swap_from_quote(&backup_0, &backup_1, &altered_request, &quote)
            .expect_err("request-substituted quote reached signing");
    assert!(unbound.to_string().contains("does not match its request"));
}
