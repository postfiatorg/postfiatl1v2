use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign,
    ml_dsa_65_sign_with_context, MlDsa65KeyPair, ML_DSA_65_ALGORITHM,
};
use postfiat_types::{
    asset_bridge_hold_key, escrow_id, fastpay_transfer_lock_id_v1, issued_asset_id, nft_id,
    offer_id, Account, AssetBurnOperation, AssetClawbackOperation, AssetCreateOperation,
    AssetTransactionOperation, BridgeHoldClaimOperation, BridgeHoldReturnOperation,
    EscrowCreateOperation, EscrowTransactionOperation, FastAssetDefinitionHashV1, FastAssetIdV1,
    FastAssetRuleV1, FastHolderPermitV1, FastLaneControlActionV1, FastLaneControlCertificateV1,
    FastLaneControlVoteV1, FastPayOrderRecoveryV1, FastPayRecoveryCommitteeV1,
    FastPayRecoveryPolicyV1, FastSwapChainDomainV1, FastSwapCommitteeDomainV1,
    FastSwapCommitteeRootV1, FastSwapCommitteeV1, FastSwapOpaqueHashV1, FastSwapValidatorV1,
    Genesis, IssuedPaymentOperation, LedgerState, NftBurnOperation, NftMintOperation,
    NftTransactionOperation, OfferCreateOperation, OfferPruneIneligibleOperation,
    OfferTransactionOperation, OwnedObject, OwnedObjectRef, OwnedOutputSpec, OwnedTransferOrderV3,
    SignedAssetTransaction, SignedEscrowTransaction, SignedFastLaneDepositV1, SignedNftTransaction,
    SignedOfferTransaction, SignedOwnedTransferOrderV3, TrustLine, TrustSetOperation,
    UnsignedAssetTransaction, UnsignedEscrowTransaction, UnsignedNftTransaction,
    UnsignedOfferTransaction, ADDRESS_NAMESPACE, ASSET_BURN_TRANSACTION_KIND,
    ASSET_CLAWBACK_TRANSACTION_KIND, ASSET_CREATE_TRANSACTION_KIND,
    BRIDGE_HOLD_CLAIM_TRANSACTION_KIND, BRIDGE_HOLD_RETURN_TRANSACTION_KIND,
    ESCROW_CREATE_TRANSACTION_KIND, FASTLANE_CONTROL_CONTEXT_V1, FASTLANE_DEPOSIT_CONTEXT_V1,
    FASTPAY_ORDER_RECOVERY_SCHEMA_V1, FASTPAY_RECOVERY_POLICY_SCHEMA_V1, FASTSWAP_ML_DSA_65,
    FASTSWAP_SCHEMA_VERSION_V1, ISSUED_PAYMENT_TRANSACTION_KIND, MAX_CLAWBACK_LOCKED_TARGETS,
    MAX_OFFER_PRUNE_BATCH, MAX_OWNED_OBJECTS_PER_ACCOUNT_SOFT_CAP, NFT_BURN_TRANSACTION_KIND,
    NFT_COLLECTION_FLAG_BURN_LOCKED, NFT_COLLECTION_FLAG_TRANSFER_LOCKED, NFT_FLAG_ISSUER_BURNABLE,
    NFT_FLAG_TRANSFERABLE, NFT_MIGRATED_SERIAL_BIT, NFT_MINT_TRANSACTION_KIND,
    OFFER_CREATE_TRANSACTION_KIND, OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND,
    TRUST_SET_TRANSACTION_KIND,
};

use crate::fastswap_bridge::{
    asset_definition_hash, execute_fastlane_deposit, FastLaneBridgeError,
};
use crate::fastswap_control::{
    execute_fastlane_control_with_compatibility, register_fastlane_asset_rule,
    register_fastlane_holder_permit, register_fastswap_committee,
};
use crate::{
    credit_issued_asset_from_shielded_pool_with_compatibility, ensure_owned_object_capacity_v2,
    ensure_shielded_entry_allowed_with_compatibility, execute_asset_transaction_with_compatibility,
    execute_escrow_transaction_with_compatibility, execute_nft_transaction_with_compatibility,
    execute_offer_transaction_with_compatibility, fastpay_minimum_fee_atoms, genesis_hash,
    issued_asset_supply, migrate_nfts_to_keyed_state_if_needed, owned_transfer_v3_signing_bytes,
    route_bridge_credit_to_hold, validate_owned_transfer_v3_admission_with_fee_v1,
    AssetExecutionCompatibility, OwnedTransferError, NATIVE_PFT_ESCROW_ASSET_ID,
    OWNED_NATIVE_ASSET, OWNED_TRANSFER_CONTEXT_V3, TRUSTLINE_STATE_EXPANSION_FEE,
};

struct TestKey {
    address: String,
    public_key_hex: String,
    keypair: MlDsa65KeyPair,
}

fn test_key(seed_byte: u8) -> TestKey {
    let keypair = ml_dsa_65_keygen_from_seed(&[seed_byte; 32]);
    let address = address_from_public_key(&keypair.public_key);
    let public_key_hex = bytes_to_hex(&keypair.public_key);
    TestKey {
        address,
        public_key_hex,
        keypair,
    }
}

fn test_genesis() -> Genesis {
    Genesis::new_with_validator_count("postfiat-train-a-test", 1)
}

fn funded_account(key: &TestKey, balance: u64) -> Account {
    Account {
        address: key.address.clone(),
        balance,
        sequence: 0,
        public_key_hex: Some(key.public_key_hex.clone()),
        flags: 0,
    }
}

fn find_line<'a>(ledger: &'a LedgerState, account: &str, asset_id: &str) -> Option<&'a TrustLine> {
    ledger
        .trustlines
        .iter()
        .find(|line| line.account == account && line.asset_id == asset_id)
}

fn sign_asset_tx(
    genesis: &Genesis,
    signer: &TestKey,
    sequence: u64,
    fee: u64,
    kind: &str,
    operation: AssetTransactionOperation,
) -> SignedAssetTransaction {
    let unsigned = UnsignedAssetTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: kind.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: signer.address.clone(),
        fee,
        sequence,
        operation,
    };
    let signature = ml_dsa_65_sign(&signer.keypair.private_key, &unsigned.signing_bytes())
        .expect("sign asset tx");
    SignedAssetTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: signer.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    }
}

fn sign_escrow_tx(
    genesis: &Genesis,
    signer: &TestKey,
    sequence: u64,
    fee: u64,
    kind: &str,
    operation: EscrowTransactionOperation,
) -> SignedEscrowTransaction {
    let unsigned = UnsignedEscrowTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: kind.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: signer.address.clone(),
        fee,
        sequence,
        operation,
    };
    let signature = ml_dsa_65_sign(&signer.keypair.private_key, &unsigned.signing_bytes())
        .expect("sign escrow tx");
    SignedEscrowTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: signer.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    }
}

fn sign_nft_tx(
    genesis: &Genesis,
    signer: &TestKey,
    sequence: u64,
    fee: u64,
    kind: &str,
    operation: NftTransactionOperation,
) -> SignedNftTransaction {
    let unsigned = UnsignedNftTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: kind.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: signer.address.clone(),
        fee,
        sequence,
        operation,
    };
    let signature = ml_dsa_65_sign(&signer.keypair.private_key, &unsigned.signing_bytes())
        .expect("sign nft tx");
    SignedNftTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: signer.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    }
}

fn sign_offer_tx(
    genesis: &Genesis,
    signer: &TestKey,
    sequence: u64,
    fee: u64,
    kind: &str,
    operation: OfferTransactionOperation,
) -> SignedOfferTransaction {
    let unsigned = UnsignedOfferTransaction {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: ADDRESS_NAMESPACE.to_string(),
        transaction_kind: kind.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        source: signer.address.clone(),
        fee,
        sequence,
        operation,
    };
    let signature = ml_dsa_65_sign(&signer.keypair.private_key, &unsigned.signing_bytes())
        .expect("sign offer tx");
    SignedOfferTransaction {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: signer.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    }
}

#[allow(clippy::too_many_arguments)]
fn create_gated_asset(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    issuer: &TestKey,
    code: &str,
    requires_authorization: bool,
    clawback_enabled: bool,
    compatibility: AssetExecutionCompatibility,
    height: u64,
) -> String {
    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let tx = sign_asset_tx(
        genesis,
        issuer,
        seq,
        50_000,
        ASSET_CREATE_TRANSACTION_KIND,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.address.clone(),
            code: code.to_string(),
            version: 1,
            precision: 6,
            display_name: format!("Test {code}"),
            max_supply: Some(1_000_000_000),
            requires_authorization,
            freeze_enabled: true,
            clawback_enabled,
        }),
    );
    let receipt =
        execute_asset_transaction_with_compatibility(genesis, ledger, &tx, height, compatibility);
    assert!(receipt.accepted, "asset_create failed: {receipt:?}");
    issued_asset_id(&genesis.chain_id, &issuer.address, code, 1).expect("issued_asset_id")
}

#[allow(clippy::too_many_arguments)]
fn set_trustline(
    genesis: &Genesis,
    ledger: &mut LedgerState,
    signer: &TestKey,
    issuer: &str,
    account: &str,
    asset_id: &str,
    limit: u64,
    authorized: bool,
    frozen: bool,
    compatibility: AssetExecutionCompatibility,
    height: u64,
) {
    let seq = ledger.account(&signer.address).unwrap().sequence + 1;
    let tx = sign_asset_tx(
        genesis,
        signer,
        seq,
        50_000,
        TRUST_SET_TRANSACTION_KIND,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: account.to_string(),
            asset_id: asset_id.to_string(),
            issuer: issuer.to_string(),
            limit,
            authorized,
            frozen,
            reserve_paid: TRUSTLINE_STATE_EXPANSION_FEE,
        }),
    );
    let receipt =
        execute_asset_transaction_with_compatibility(genesis, ledger, &tx, height, compatibility);
    assert!(receipt.accepted, "trust_set failed: {receipt:?}");
}

#[test]
fn pillar8_m1_closes_issued_payment_and_shielded_bypasses_and_preserves_return_only_exits() {
    let genesis = test_genesis();
    let issuer = test_key(1);
    let alice = test_key(2);
    let bob = test_key(3);
    let mut ledger = LedgerState::new(vec![
        funded_account(&issuer, 10_000_000),
        funded_account(&alice, 10_000_000),
        funded_account(&bob, 10_000_000),
    ]);
    let compat =
        AssetExecutionCompatibility::strict().with_asset_gate_v1_activation_height(Some(10));

    let gated_id = create_gated_asset(
        &genesis,
        &mut ledger,
        &issuer,
        "GUSD",
        true,
        true,
        compat,
        10,
    );
    let open_id = create_gated_asset(
        &genesis,
        &mut ledger,
        &issuer,
        "OUSD",
        false,
        false,
        compat,
        10,
    );

    // 1) Bypass path #1 closed: issued_payment to unauthorized Alice MUST reject post-activation.
    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let pay_unauth = sign_asset_tx(
        &genesis,
        &issuer,
        seq,
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.address.clone(),
            to: alice.address.clone(),
            asset_id: gated_id.clone(),
            issuer: issuer.address.clone(),
            amount: 500,
        }),
    );
    let receipt = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &pay_unauth,
        10,
        compat,
    );
    assert!(!receipt.accepted);
    assert_eq!(receipt.code, "asset_gate_recipient_ineligible");
    assert!(
        find_line(&ledger, &alice.address, &gated_id).is_none(),
        "rejected gated payment must not create an implicit trustline"
    );

    // Pre-activation height 9 preserves historical replay semantics (creates implicit authorized line).
    let mut pre_ledger = ledger.clone();
    let pre_receipt = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut pre_ledger,
        &pay_unauth,
        9,
        compat,
    );
    assert!(
        pre_receipt.accepted,
        "pre-activation replay must remain unchanged: {pre_receipt:?}"
    );
    assert!(
        find_line(&pre_ledger, &alice.address, &gated_id)
            .unwrap()
            .authorized
    );

    // 2) Bypass path #2 closed: gated asset barred from shielded pool entry and exit.
    let entry_err = ensure_shielded_entry_allowed_with_compatibility(
        &ledger,
        &alice.address,
        &gated_id,
        10,
        compat,
    )
    .unwrap_err();
    assert_eq!(entry_err.0, "gated_asset_shielded_pool_forbidden");
    let exit_err = credit_issued_asset_from_shielded_pool_with_compatibility(
        &mut ledger,
        &alice.address,
        &gated_id,
        100,
        10,
        compat,
    )
    .unwrap_err();
    assert_eq!(exit_err.0, "gated_asset_shielded_pool_forbidden");

    // Ungated asset CAN exit shielded pool, and implicit line is created with authorized = false!
    credit_issued_asset_from_shielded_pool_with_compatibility(
        &mut ledger,
        &alice.address,
        &open_id,
        100,
        10,
        compat,
    )
    .unwrap();
    let implicit_open = find_line(&ledger, &alice.address, &open_id).unwrap();
    assert!(
        !implicit_open.authorized,
        "implicit lines must NEVER set authorized = true"
    );
    assert_eq!(implicit_open.balance, 100);
    ensure_shielded_entry_allowed_with_compatibility(&ledger, &alice.address, &open_id, 10, compat)
        .unwrap();

    // Returning full implicit ungated balance back to issuer auto-prunes the empty implicit line.
    let seq = ledger.account(&alice.address).unwrap().sequence + 1;
    let return_open = sign_asset_tx(
        &genesis,
        &alice,
        seq,
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: alice.address.clone(),
            to: issuer.address.clone(),
            asset_id: open_id.clone(),
            issuer: issuer.address.clone(),
            amount: 100,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &return_open,
        10,
        compat,
    );
    assert!(r.accepted, "{r:?}");
    assert!(
        find_line(&ledger, &alice.address, &open_id).is_none(),
        "empty implicit line should be auto-pruned on zero balance"
    );

    // 3) Authorize Alice & Bob explicitly, fund Alice with 1_000 gated units, then revoke Alice.
    set_trustline(
        &genesis,
        &mut ledger,
        &alice,
        &issuer.address,
        &alice.address,
        &gated_id,
        10_000,
        false,
        false,
        compat,
        10,
    );
    set_trustline(
        &genesis,
        &mut ledger,
        &issuer,
        &issuer.address,
        &alice.address,
        &gated_id,
        10_000,
        true,
        false,
        compat,
        10,
    );
    set_trustline(
        &genesis,
        &mut ledger,
        &bob,
        &issuer.address,
        &bob.address,
        &gated_id,
        10_000,
        false,
        false,
        compat,
        10,
    );
    set_trustline(
        &genesis,
        &mut ledger,
        &issuer,
        &issuer.address,
        &bob.address,
        &gated_id,
        10_000,
        true,
        false,
        compat,
        10,
    );

    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let fund_alice = sign_asset_tx(
        &genesis,
        &issuer,
        seq,
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.address.clone(),
            to: alice.address.clone(),
            asset_id: gated_id.clone(),
            issuer: issuer.address.clone(),
            amount: 1_000,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &fund_alice,
            10,
            compat
        )
        .accepted
    );

    // Issuer revokes Alice's authorization (line stays non-frozen).
    set_trustline(
        &genesis,
        &mut ledger,
        &issuer,
        &issuer.address,
        &alice.address,
        &gated_id,
        10_000,
        false,
        false,
        compat,
        11,
    );

    // Revoked Alice CANNOT send to authorized Bob (local transfer blocked).
    let seq = ledger.account(&alice.address).unwrap().sequence + 1;
    let send_to_bob = sign_asset_tx(
        &genesis,
        &alice,
        seq,
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: alice.address.clone(),
            to: bob.address.clone(),
            asset_id: gated_id.clone(),
            issuer: issuer.address.clone(),
            amount: 200,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &send_to_bob,
        11,
        compat,
    );
    assert!(!r.accepted);
    assert_eq!(r.code, "asset_gate_sender_ineligible");

    // Revoked Alice CAN exit via Return-only paths: payment to issuer AND asset_burn!
    let seq = ledger.account(&alice.address).unwrap().sequence + 1;
    let return_to_issuer = sign_asset_tx(
        &genesis,
        &alice,
        seq,
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: alice.address.clone(),
            to: issuer.address.clone(),
            asset_id: gated_id.clone(),
            issuer: issuer.address.clone(),
            amount: 300,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &return_to_issuer,
        11,
        compat,
    );
    assert!(
        r.accepted,
        "return-to-issuer exit must succeed for revoked holder: {r:?}"
    );

    let seq = ledger.account(&alice.address).unwrap().sequence + 1;
    let burn_exit = sign_asset_tx(
        &genesis,
        &alice,
        seq,
        50_000,
        ASSET_BURN_TRANSACTION_KIND,
        AssetTransactionOperation::AssetBurn(AssetBurnOperation {
            owner: alice.address.clone(),
            asset_id: gated_id.clone(),
            issuer: issuer.address.clone(),
            amount: 700,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &burn_exit, 11, compat);
    assert!(
        r.accepted,
        "asset_burn exit must succeed for revoked holder: {r:?}"
    );
    assert_eq!(
        find_line(&ledger, &alice.address, &gated_id)
            .unwrap()
            .balance,
        0
    );
}

#[test]
fn pillar8_m1_aggregated_bridge_holds_claim_return_and_supply_reconciliation() {
    let genesis = test_genesis();
    let issuer = test_key(10);
    let recipient = test_key(11);
    let mut ledger = LedgerState::new(vec![
        funded_account(&issuer, 10_000_000),
        funded_account(&recipient, 10_000_000),
    ]);
    let compat =
        AssetExecutionCompatibility::strict().with_asset_gate_v1_activation_height(Some(10));
    let gated_id = create_gated_asset(
        &genesis,
        &mut ledger,
        &issuer,
        "BGUSD",
        true,
        true,
        compat,
        10,
    );
    let gated_def = ledger.asset_definition(&gated_id).unwrap().clone();

    // Two consecutive bridge credits for ineligible recipient aggregate into ONE hold entry.
    let ev1 = route_bridge_credit_to_hold(
        &mut ledger,
        &gated_def,
        &recipient.address,
        400,
        "bridge-ref-1",
        "asset_gate_recipient_ineligible",
        10,
    )
    .expect("hold credit 1");
    let ev2 = route_bridge_credit_to_hold(
        &mut ledger,
        &gated_def,
        &recipient.address,
        600,
        "bridge-ref-2",
        "asset_gate_recipient_ineligible",
        11,
    )
    .expect("hold credit 2");
    assert_eq!(ev1.hold_key, ev2.hold_key);
    assert_eq!(
        ev1.hold_key,
        asset_bridge_hold_key(&gated_id, &recipient.address)
    );
    assert_eq!(ledger.asset_bridge_holds.len(), 1);
    let hold = ledger
        .asset_bridge_hold(&gated_id, &recipient.address)
        .unwrap();
    assert_eq!(hold.amount, 1_000);
    assert_eq!(hold.credit_count, 2);
    assert_eq!(issued_asset_supply(&ledger, &gated_id).unwrap(), 1_000);

    // bridge_hold_claim fails while recipient is still ineligible.
    let seq = ledger.account(&recipient.address).unwrap().sequence + 1;
    let claim_fail = sign_asset_tx(
        &genesis,
        &recipient,
        seq,
        50_000,
        BRIDGE_HOLD_CLAIM_TRANSACTION_KIND,
        AssetTransactionOperation::BridgeHoldClaim(BridgeHoldClaimOperation {
            asset_id: gated_id.clone(),
            recipient: recipient.address.clone(),
            amount: 400,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &claim_fail,
        12,
        compat,
    );
    assert!(!r.accepted);
    assert_eq!(r.code, "asset_gate_recipient_ineligible");

    // bridge_hold_return SUCCEEDS even while recipient is ineligible (Return-only exit!).
    let seq = ledger.account(&recipient.address).unwrap().sequence + 1;
    let return_ok = sign_asset_tx(
        &genesis,
        &recipient,
        seq,
        50_000,
        BRIDGE_HOLD_RETURN_TRANSACTION_KIND,
        AssetTransactionOperation::BridgeHoldReturn(BridgeHoldReturnOperation {
            asset_id: gated_id.clone(),
            recipient: recipient.address.clone(),
            amount: 250,
            burn: true,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &return_ok, 12, compat);
    assert!(r.accepted, "{r:?}");
    assert_eq!(r.bridge_hold_events.len(), 1);
    assert_eq!(r.bridge_hold_events[0].reason_code, "bridge_hold_burned");
    assert_eq!(issued_asset_supply(&ledger, &gated_id).unwrap(), 750);

    // After issuer authorizes recipient, bridge_hold_claim claims remaining 750 and deletes hold entry.
    set_trustline(
        &genesis,
        &mut ledger,
        &recipient,
        &issuer.address,
        &recipient.address,
        &gated_id,
        10_000,
        false,
        false,
        compat,
        13,
    );
    set_trustline(
        &genesis,
        &mut ledger,
        &issuer,
        &issuer.address,
        &recipient.address,
        &gated_id,
        10_000,
        true,
        false,
        compat,
        13,
    );
    let seq = ledger.account(&recipient.address).unwrap().sequence + 1;
    let claim_ok = sign_asset_tx(
        &genesis,
        &recipient,
        seq,
        50_000,
        BRIDGE_HOLD_CLAIM_TRANSACTION_KIND,
        AssetTransactionOperation::BridgeHoldClaim(BridgeHoldClaimOperation {
            asset_id: gated_id.clone(),
            recipient: recipient.address.clone(),
            amount: 750,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &claim_ok, 13, compat);
    assert!(r.accepted, "{r:?}");
    assert_eq!(r.bridge_hold_events.len(), 1);
    assert_eq!(r.bridge_hold_events[0].reason_code, "bridge_hold_claimed");
    assert!(ledger
        .asset_bridge_hold(&gated_id, &recipient.address)
        .is_none());
    assert_eq!(
        find_line(&ledger, &recipient.address, &gated_id)
            .unwrap()
            .balance,
        750
    );
    assert_eq!(issued_asset_supply(&ledger, &gated_id).unwrap(), 750);
}

#[test]
fn pillar8_m1_dex_anti_poisoning_prune_and_bounded_clawback() {
    let genesis = test_genesis();
    let issuer = test_key(20);
    let bad_maker = test_key(21);
    let good_maker = test_key(22);
    let taker = test_key(23);
    let pruner = test_key(24);
    let mut ledger = LedgerState::new(vec![
        funded_account(&issuer, 50_000_000),
        funded_account(&bad_maker, 50_000_000),
        funded_account(&good_maker, 50_000_000),
        funded_account(&taker, 50_000_000),
        funded_account(&pruner, 50_000_000),
    ]);
    let compat =
        AssetExecutionCompatibility::strict().with_asset_gate_v1_activation_height(Some(10));
    let gated_id = create_gated_asset(
        &genesis,
        &mut ledger,
        &issuer,
        "DEXG",
        true,
        true,
        compat,
        10,
    );
    let gated_def = ledger.asset_definition(&gated_id).unwrap().clone();

    for k in [&bad_maker, &good_maker, &taker] {
        set_trustline(
            &genesis,
            &mut ledger,
            k,
            &issuer.address,
            &k.address,
            &gated_id,
            100_000,
            false,
            false,
            compat,
            10,
        );
        set_trustline(
            &genesis,
            &mut ledger,
            &issuer,
            &issuer.address,
            &k.address,
            &gated_id,
            100_000,
            true,
            false,
            compat,
            10,
        );
    }
    for (k, amt) in [(&bad_maker, 5_000_u64), (&good_maker, 5_000_u64)] {
        let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
        let tx = sign_asset_tx(
            &genesis,
            &issuer,
            seq,
            50_000,
            ISSUED_PAYMENT_TRANSACTION_KIND,
            AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
                from: issuer.address.clone(),
                to: k.address.clone(),
                asset_id: gated_id.clone(),
                issuer: issuer.address.clone(),
                amount: amt,
            }),
        );
        assert!(
            execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &tx, 10, compat)
                .accepted
        );
    }

    // bad_maker posts 2 offers selling gated asset for PFT; good_maker posts 1 offer behind them.
    let bad_seq_1 = ledger.account(&bad_maker.address).unwrap().sequence + 1;
    let bad_offer_1 = offer_id(&genesis.chain_id, &bad_maker.address, bad_seq_1).unwrap();
    let tx = sign_offer_tx(
        &genesis,
        &bad_maker,
        bad_seq_1,
        50_000,
        OFFER_CREATE_TRANSACTION_KIND,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: bad_maker.address.clone(),
            taker_gets_asset_id: gated_id.clone(),
            taker_gets_amount: 100,
            taker_pays_asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            taker_pays_amount: 100,
            expiration_height: 0,
        }),
    );
    assert!(
        execute_offer_transaction_with_compatibility(&genesis, &mut ledger, &tx, 10, compat)
            .accepted
    );

    let bad_seq_2 = ledger.account(&bad_maker.address).unwrap().sequence + 1;
    let bad_offer_2 = offer_id(&genesis.chain_id, &bad_maker.address, bad_seq_2).unwrap();
    let tx = sign_offer_tx(
        &genesis,
        &bad_maker,
        bad_seq_2,
        50_000,
        OFFER_CREATE_TRANSACTION_KIND,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: bad_maker.address.clone(),
            taker_gets_asset_id: gated_id.clone(),
            taker_gets_amount: 100,
            taker_pays_asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            taker_pays_amount: 50,
            expiration_height: 0,
        }),
    );
    assert!(
        execute_offer_transaction_with_compatibility(&genesis, &mut ledger, &tx, 10, compat)
            .accepted
    );

    let good_seq = ledger.account(&good_maker.address).unwrap().sequence + 1;
    let good_offer = offer_id(&genesis.chain_id, &good_maker.address, good_seq).unwrap();
    let tx = sign_offer_tx(
        &genesis,
        &good_maker,
        good_seq,
        50_000,
        OFFER_CREATE_TRANSACTION_KIND,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: good_maker.address.clone(),
            taker_gets_asset_id: gated_id.clone(),
            taker_gets_amount: 100,
            taker_pays_asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            taker_pays_amount: 100,
            expiration_height: 0,
        }),
    );
    assert!(
        execute_offer_transaction_with_compatibility(&genesis, &mut ledger, &tx, 10, compat)
            .accepted
    );

    // bad_maker also creates a 3rd offer and an escrow for bounded clawback testing.
    let bad_seq_3 = ledger.account(&bad_maker.address).unwrap().sequence + 1;
    let bad_offer_3 = offer_id(&genesis.chain_id, &bad_maker.address, bad_seq_3).unwrap();
    let tx = sign_offer_tx(
        &genesis,
        &bad_maker,
        bad_seq_3,
        50_000,
        OFFER_CREATE_TRANSACTION_KIND,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: bad_maker.address.clone(),
            taker_gets_asset_id: gated_id.clone(),
            taker_gets_amount: 500,
            taker_pays_asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            taker_pays_amount: 50_000,
            expiration_height: 0,
        }),
    );
    assert!(
        execute_offer_transaction_with_compatibility(&genesis, &mut ledger, &tx, 10, compat)
            .accepted
    );

    let esc_seq = ledger.account(&bad_maker.address).unwrap().sequence + 1;
    let bad_escrow = escrow_id(&genesis.chain_id, &bad_maker.address, esc_seq).unwrap();
    let esc_tx = sign_escrow_tx(
        &genesis,
        &bad_maker,
        esc_seq,
        50_000,
        ESCROW_CREATE_TRANSACTION_KIND,
        EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
            owner: bad_maker.address.clone(),
            recipient: good_maker.address.clone(),
            asset_id: gated_id.clone(),
            amount: 400,
            finish_after: 100,
            cancel_after: 200,
            condition: String::new(),
        }),
    );
    assert!(
        execute_escrow_transaction_with_compatibility(&genesis, &mut ledger, &esc_tx, 10, compat)
            .accepted
    );

    // Revoke bad_maker's authorization!
    set_trustline(
        &genesis,
        &mut ledger,
        &issuer,
        &issuer.address,
        &bad_maker.address,
        &gated_id,
        100_000,
        false,
        false,
        compat,
        11,
    );

    // Pruner cannot prune good_offer (still eligible!).
    let p_seq = ledger.account(&pruner.address).unwrap().sequence + 1;
    let prune_good = sign_offer_tx(
        &genesis,
        &pruner,
        p_seq,
        50_000,
        OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND,
        OfferTransactionOperation::OfferPruneIneligible(OfferPruneIneligibleOperation {
            pruner: pruner.address.clone(),
            offer_ids: vec![good_offer.clone()],
        }),
    );
    let r = execute_offer_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &prune_good,
        11,
        compat,
    );
    assert!(!r.accepted);
    assert_eq!(r.code, "offer_still_eligible");

    // Pruner CAN prune bad_offer_1 permissionlessly.
    let p_seq = ledger.account(&pruner.address).unwrap().sequence + 1;
    let prune_bad = sign_offer_tx(
        &genesis,
        &pruner,
        p_seq,
        50_000,
        OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND,
        OfferTransactionOperation::OfferPruneIneligible(OfferPruneIneligibleOperation {
            pruner: pruner.address.clone(),
            offer_ids: vec![bad_offer_1.clone()],
        }),
    );
    let r =
        execute_offer_transaction_with_compatibility(&genesis, &mut ledger, &prune_bad, 11, compat);
    assert!(r.accepted, "{r:?}");
    assert_eq!(ledger.offer(&bad_offer_1).unwrap().state, "canceled");

    // Taker crosses the book: skips bad_offer_2 inline (canceling it!) and fills good_offer!
    let t_seq = ledger.account(&taker.address).unwrap().sequence + 1;
    let taker_tx = sign_offer_tx(
        &genesis,
        &taker,
        t_seq,
        50_000,
        OFFER_CREATE_TRANSACTION_KIND,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: taker.address.clone(),
            taker_gets_asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            taker_gets_amount: 100,
            taker_pays_asset_id: gated_id.clone(),
            taker_pays_amount: 100,
            expiration_height: 0,
        }),
    );
    let r =
        execute_offer_transaction_with_compatibility(&genesis, &mut ledger, &taker_tx, 11, compat);
    assert!(
        r.accepted,
        "taker must bypass poisoned offer and fill good_offer: {r:?}"
    );
    assert_eq!(
        ledger.offer(&bad_offer_2).unwrap().state,
        "canceled",
        "skipped poisoned offer must be canceled inline"
    );
    assert_eq!(
        find_line(&ledger, &taker.address, &gated_id)
            .unwrap()
            .balance,
        100
    );

    // Add a bridge hold for bad_maker too (300 atoms).
    route_bridge_credit_to_hold(
        &mut ledger,
        &gated_def,
        &bad_maker.address,
        300,
        "bridge-bad-1",
        "asset_gate_recipient_ineligible",
        11,
    )
    .unwrap();

    // Bounded asset_clawback rejects >64 targets.
    let i_seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let over_limit = sign_asset_tx(
        &genesis,
        &issuer,
        i_seq,
        50_000,
        ASSET_CLAWBACK_TRANSACTION_KIND,
        AssetTransactionOperation::AssetClawback(AssetClawbackOperation {
            issuer: issuer.address.clone(),
            owner: bad_maker.address.clone(),
            asset_id: gated_id.clone(),
            amount: 1,
            offer_ids: (0..=MAX_CLAWBACK_LOCKED_TARGETS)
                .map(|i| format!("{i:096x}"))
                .collect(),
            escrow_ids: Vec::new(),
            include_bridge_hold: false,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &over_limit,
        12,
        compat,
    );
    assert!(!r.accepted);
    assert_eq!(r.code, "clawback_target_limit_exceeded");

    // Bounded sweep across free line (4_100) + hold (300) + bad_offer_3 (500) + bad_escrow (400) = 5_300!
    let i_seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let clawback = sign_asset_tx(
        &genesis,
        &issuer,
        i_seq,
        50_000,
        ASSET_CLAWBACK_TRANSACTION_KIND,
        AssetTransactionOperation::AssetClawback(AssetClawbackOperation {
            issuer: issuer.address.clone(),
            owner: bad_maker.address.clone(),
            asset_id: gated_id.clone(),
            amount: 5_300,
            offer_ids: vec![bad_offer_3.clone()],
            escrow_ids: vec![bad_escrow.clone()],
            include_bridge_hold: true,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &clawback, 12, compat);
    assert!(r.accepted, "bounded clawback failed: {r:?}");
    assert_eq!(
        find_line(&ledger, &bad_maker.address, &gated_id)
            .unwrap()
            .balance,
        0
    );
    assert!(ledger
        .asset_bridge_hold(&gated_id, &bad_maker.address)
        .is_none());
    assert_eq!(ledger.offer(&bad_offer_3).unwrap().state, "canceled");
    assert_eq!(ledger.escrow(&bad_escrow).unwrap().state, "canceled");

    // Check offer_prune_ineligible rejects > MAX_OFFER_PRUNE_BATCH.
    let p_seq = ledger.account(&pruner.address).unwrap().sequence + 1;
    let prune_too_many = sign_offer_tx(
        &genesis,
        &pruner,
        p_seq,
        50_000,
        OFFER_PRUNE_INELIGIBLE_TRANSACTION_KIND,
        OfferTransactionOperation::OfferPruneIneligible(OfferPruneIneligibleOperation {
            pruner: pruner.address.clone(),
            offer_ids: (0..=MAX_OFFER_PRUNE_BATCH)
                .map(|i| format!("{i:096x}"))
                .collect(),
        }),
    );
    let r = execute_offer_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &prune_too_many,
        12,
        compat,
    );
    assert!(!r.accepted);
    assert_eq!(r.code, "offer_prune_batch_limit_exceeded");
}

struct FastLanePermitFixture {
    ledger: LedgerState,
    compat: AssetExecutionCompatibility,
    permit: FastHolderPermitV1,
    cert: FastLaneControlCertificateV1,
    chain_domain: FastSwapChainDomainV1,
    rule: FastAssetRuleV1,
    signed_deposit: SignedFastLaneDepositV1,
}

/// A gated asset with a registered FastLane rule and holder permit, a signed
/// `RevokeHolderPermit` certificate for `revoked_at_height`, and a deposit to
/// the permit's holder.
fn fastlane_permit_fixture(revoked_at_height: u64) -> FastLanePermitFixture {
    let genesis = test_genesis();
    let issuer = test_key(30);
    let holder = test_key(31);
    let vals = [test_key(32), test_key(33), test_key(34), test_key(35)];
    let mut ledger = LedgerState::new(vec![
        funded_account(&issuer, 50_000_000),
        funded_account(&holder, 50_000_000),
    ]);
    let compat =
        AssetExecutionCompatibility::strict().with_asset_gate_v1_activation_height(Some(1));
    let gated_id = create_gated_asset(
        &genesis,
        &mut ledger,
        &issuer,
        "FLG",
        true,
        false,
        compat,
        1,
    );
    set_trustline(
        &genesis,
        &mut ledger,
        &holder,
        &issuer.address,
        &holder.address,
        &gated_id,
        10_000,
        false,
        false,
        compat,
        1,
    );
    set_trustline(
        &genesis,
        &mut ledger,
        &issuer,
        &issuer.address,
        &holder.address,
        &gated_id,
        10_000,
        true,
        false,
        compat,
        1,
    );
    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let fund = sign_asset_tx(
        &genesis,
        &issuer,
        seq,
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.address.clone(),
            to: holder.address.clone(),
            asset_id: gated_id.clone(),
            issuer: issuer.address.clone(),
            amount: 500,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &fund, 1, compat)
            .accepted
    );

    let chain_domain = FastSwapChainDomainV1 {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: FastSwapOpaqueHashV1([1; 48]),
        protocol_version: 1,
    };
    let mut committee = FastSwapCommitteeV1 {
        domain: FastSwapCommitteeDomainV1 {
            chain: chain_domain.clone(),
            fastswap_schema_version: FASTSWAP_SCHEMA_VERSION_V1,
            committee_epoch: 1,
            committee_root: FastSwapCommitteeRootV1([0; 48]),
            validator_count: 4,
            quorum: 3,
        },
        validators: vals
            .iter()
            .enumerate()
            .map(|(i, v)| FastSwapValidatorV1 {
                validator_id: format!("val-{i}"),
                public_key: v.keypair.public_key.clone(),
            })
            .collect(),
    };
    committee.domain.committee_root = committee.computed_root().unwrap();
    register_fastswap_committee(&mut ledger, committee.clone(), None).unwrap();

    let asset_bytes: [u8; 48] = postfiat_crypto_provider::hex_to_bytes(&gated_id)
        .unwrap()
        .try_into()
        .unwrap();
    let fast_asset_id = FastAssetIdV1(asset_bytes);
    let def = ledger.asset_definition(&gated_id).unwrap().clone();
    let rule = FastAssetRuleV1 {
        asset_id: fast_asset_id,
        asset_definition_hash: asset_definition_hash(&def)
            .unwrap_or(FastAssetDefinitionHashV1::ZERO),
        issuer_address: issuer.address.clone(),
        issuer_control_pubkey: issuer.keypair.public_key.clone(),
        fast_lane_enabled: true,
        requires_authorization: true,
        freeze_enabled: true,
        clawback_enabled: false,
        valid_from_height: 1,
        valid_through_height: 1_000,
    };
    register_fastlane_asset_rule(&mut ledger, rule.clone()).unwrap();

    let mut permit = FastHolderPermitV1 {
        permit_id: postfiat_types::FastHolderPermitIdV1([0; 48]),
        asset_id: fast_asset_id,
        owner_pubkey: holder.keypair.public_key.clone(),
        valid_from_height: 1,
        valid_through_height: 1_000,
        consensus_receipt_digest: FastSwapOpaqueHashV1([9; 48]),
    };
    permit.permit_id = permit.computed_id().unwrap();
    register_fastlane_holder_permit(&mut ledger, permit.clone()).unwrap();

    let action = FastLaneControlActionV1::RevokeHolderPermit {
        permit_id: permit.permit_id,
        revoked_at_height,
        reason_code: "compliance_revoked".to_string(),
    };
    let action_digest = action.digest().unwrap();
    let votes = vals
        .iter()
        .enumerate()
        .take(3)
        .map(|(i, v)| {
            let mut vote = FastLaneControlVoteV1 {
                committee: committee.domain.clone(),
                action_digest,
                validator_id: format!("val-{i}"),
                signature: Vec::new(),
            };
            vote.signature = ml_dsa_65_sign_with_context(
                &v.keypair.private_key,
                &vote.signing_bytes().unwrap(),
                FASTLANE_CONTROL_CONTEXT_V1,
            )
            .unwrap();
            vote
        })
        .collect();
    let cert = FastLaneControlCertificateV1 { action, votes };
    // Deposit referencing the revoked permit MUST fail with HolderPermitInvalid!
    let deposit = postfiat_types::FastLaneDepositV1 {
        domain: chain_domain.clone(),
        source_address: holder.address.clone(),
        source_pubkey: holder.keypair.public_key.clone(),
        sequence: ledger.account(&holder.address).unwrap().sequence + 1,
        nonce: [1; 32],
        asset_id: fast_asset_id,
        asset_rule_hash: rule.rule_hash().unwrap(),
        amount_atoms: 100,
        fee_pft: 100,
        destination_owner_pubkey: holder.keypair.public_key.clone(),
        destination_holder_permit_id: Some(permit.permit_id),
    };
    let dep_sig = ml_dsa_65_sign_with_context(
        &holder.keypair.private_key,
        &deposit.signing_bytes().unwrap(),
        FASTLANE_DEPOSIT_CONTEXT_V1,
    )
    .unwrap();
    let signed_deposit = SignedFastLaneDepositV1 {
        deposit,
        algorithm_id: FASTSWAP_ML_DSA_65.to_string(),
        signature: dep_sig,
    };
    FastLanePermitFixture {
        ledger,
        compat,
        permit,
        cert,
        chain_domain,
        rule,
        signed_deposit,
    }
}

#[test]
fn pillar8_m1_fastlane_revoke_holder_permit_blocks_deposit() {
    let FastLanePermitFixture {
        mut ledger,
        compat,
        permit,
        cert,
        chain_domain,
        rule,
        signed_deposit,
    } = fastlane_permit_fixture(5);
    execute_fastlane_control_with_compatibility(&mut ledger, &cert, 5, compat).unwrap();
    assert_eq!(
        ledger
            .revoked_fast_lane_holder_permits
            .get(&bytes_to_hex(&permit.permit_id.0)),
        Some(&5)
    );

    // Deposit referencing the revoked permit MUST fail with HolderPermitInvalid!
    let err = execute_fastlane_deposit(&mut ledger, &signed_deposit, &chain_domain, &rule, 6)
        .unwrap_err();
    assert_eq!(err, FastLaneBridgeError::HolderPermitInvalid);
}

/// Point 5 (A10, B06): `RevokeHolderPermit` is refused while `asset_gate_v1`
/// is not active, and writes nothing.
#[test]
fn fastlane_revoke_holder_permit_is_refused_before_the_asset_gate() {
    let fixture = fastlane_permit_fixture(5);
    let transaction = postfiat_types::FastLanePrimaryTransactionV1 {
        operation: postfiat_types::FastLanePrimaryOperationV1::Control {
            certificate: fixture.cert.clone(),
        },
    };
    let mut ledger = fixture.ledger.clone();
    let receipt =
        crate::fastlane_primary::execute_fastlane_primary_transaction(&mut ledger, &transaction, 5);
    assert!(!receipt.accepted, "{receipt:?}");
    assert_eq!(receipt.code, "fastlane_control_rejected");
    assert!(ledger.revoked_fast_lane_holder_permits.is_empty());
    assert_eq!(ledger, fixture.ledger);
}

/// Point 5 (A10, B11): a deposit honours the recorded revocation height.
#[test]
fn fastlane_deposit_honours_the_recorded_revocation_height() {
    let fixture = fastlane_permit_fixture(8);
    let mut ledger = fixture.ledger.clone();
    ledger
        .revoked_fast_lane_holder_permits
        .insert(bytes_to_hex(&fixture.permit.permit_id.0), 8);
    let before = execute_fastlane_deposit(
        &mut ledger.clone(),
        &fixture.signed_deposit,
        &fixture.chain_domain,
        &fixture.rule,
        6,
    );
    assert!(before.is_ok(), "{before:?}");
    let at = execute_fastlane_deposit(
        &mut ledger,
        &fixture.signed_deposit,
        &fixture.chain_domain,
        &fixture.rule,
        8,
    );
    assert_eq!(at.unwrap_err(), FastLaneBridgeError::HolderPermitInvalid);
}

#[test]
fn pillar4_keyed_nft_migration_tombstones_issuer_burn_and_spam_guard() {
    let genesis = test_genesis();
    let issuer = test_key(40);
    let owner = test_key(41);
    let mut ledger = LedgerState::new(vec![
        funded_account(&issuer, 50_000_000),
        funded_account(&owner, 50_000_000),
    ]);
    let compat =
        AssetExecutionCompatibility::strict().with_nft_scaling_v1_activation_height(Some(10));

    // Mint a legacy NFT at height 5 (pre-activation -> stored in Vec `ledger.nfts`).
    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let legacy_mint = sign_nft_tx(
        &genesis,
        &issuer,
        seq,
        50_000,
        NFT_MINT_TRANSACTION_KIND,
        NftTransactionOperation::NftMint(NftMintOperation {
            issuer: issuer.address.clone(),
            collection_id: "COL-A".to_string(),
            serial: 1,
            owner: owner.address.clone(),
            metadata_hash: "11".repeat(48),
            metadata_uri: "ipfs://legacy-1".to_string(),
            flags: NFT_FLAG_TRANSFERABLE | NFT_FLAG_ISSUER_BURNABLE,
            collection_flags: 0,
            issuer_transfer_fee: 0,
        }),
    );
    assert!(
        execute_nft_transaction_with_compatibility(&genesis, &mut ledger, &legacy_mint, 5, compat)
            .accepted
    );
    let legacy_id = nft_id(&genesis.chain_id, &issuer.address, "COL-A", 1).unwrap();
    assert_eq!(ledger.nfts.len(), 1);
    assert!(ledger.nfts_by_id.is_empty());

    // Pre-activation: NftBurnOperation with `issuer` set MUST reject (`nft_issuer_burn_not_active`).
    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let early_issuer_burn = sign_nft_tx(
        &genesis,
        &issuer,
        seq,
        50_000,
        NFT_BURN_TRANSACTION_KIND,
        NftTransactionOperation::NftBurn(NftBurnOperation {
            nft_id: legacy_id.clone(),
            owner: owner.address.clone(),
            issuer: issuer.address.clone(),
        }),
    );
    let r = execute_nft_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &early_issuer_burn,
        9,
        compat,
    );
    assert!(!r.accepted);
    assert_eq!(r.code, "nft_issuer_burn_not_active");

    // At activation height 10: deterministic migration moves `ledger.nfts` -> `nfts_by_id` + `nft_collection_policies`.
    migrate_nfts_to_keyed_state_if_needed(&mut ledger, 10, compat);
    assert!(ledger.nfts.is_empty());
    assert!(ledger.nfts_by_id.contains_key(&legacy_id));
    assert_eq!(ledger.nft_collection_policies.len(), 1);

    // Post-activation: issuer burns `legacy_id` using `NFT_FLAG_ISSUER_BURNABLE`!
    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let issuer_burn = sign_nft_tx(
        &genesis,
        &issuer,
        seq,
        50_000,
        NFT_BURN_TRANSACTION_KIND,
        NftTransactionOperation::NftBurn(NftBurnOperation {
            nft_id: legacy_id.clone(),
            owner: owner.address.clone(),
            issuer: issuer.address.clone(),
        }),
    );
    let r =
        execute_nft_transaction_with_compatibility(&genesis, &mut ledger, &issuer_burn, 10, compat);
    assert!(
        r.accepted,
        "issuer burn with NFT_FLAG_ISSUER_BURNABLE must succeed: {r:?}"
    );
    assert!(ledger.nfts_by_id.get(&legacy_id).unwrap().burned);
    assert!(ledger.used_nft_serials.contains_key(&legacy_id));

    // Burn-then-remint of the same serial MUST reject with `nft_serial_already_used`!
    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let remint = sign_nft_tx(
        &genesis,
        &issuer,
        seq,
        50_000,
        NFT_MINT_TRANSACTION_KIND,
        NftTransactionOperation::NftMint(NftMintOperation {
            issuer: issuer.address.clone(),
            collection_id: "COL-A".to_string(),
            serial: 1,
            owner: owner.address.clone(),
            metadata_hash: "22".repeat(48),
            metadata_uri: "ipfs://remint-1".to_string(),
            flags: NFT_FLAG_TRANSFERABLE,
            collection_flags: 0,
            issuer_transfer_fee: 0,
        }),
    );
    let r = execute_nft_transaction_with_compatibility(&genesis, &mut ledger, &remint, 11, compat);
    assert!(!r.accepted);
    assert_eq!(r.code, "nft_serial_already_used");

    // Non-issuer-burnable NFT cannot be burned by issuer (`nft_not_issuer_burnable`).
    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let migrated_serial = NFT_MIGRATED_SERIAL_BIT | 42;
    let mint_non_issuer_burn = sign_nft_tx(
        &genesis,
        &issuer,
        seq,
        50_000,
        NFT_MINT_TRANSACTION_KIND,
        NftTransactionOperation::NftMint(NftMintOperation {
            issuer: issuer.address.clone(),
            collection_id: "COL-A".to_string(),
            serial: migrated_serial,
            owner: owner.address.clone(),
            metadata_hash: "33".repeat(48),
            metadata_uri: "ipfs://migrated-42".to_string(),
            flags: NFT_FLAG_TRANSFERABLE,
            collection_flags: 0,
            issuer_transfer_fee: 0,
        }),
    );
    assert!(
        execute_nft_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &mint_non_issuer_burn,
            11,
            compat
        )
        .accepted
    );
    let migrated_id = nft_id(&genesis.chain_id, &issuer.address, "COL-A", migrated_serial).unwrap();

    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let bad_issuer_burn = sign_nft_tx(
        &genesis,
        &issuer,
        seq,
        50_000,
        NFT_BURN_TRANSACTION_KIND,
        NftTransactionOperation::NftBurn(NftBurnOperation {
            nft_id: migrated_id,
            owner: owner.address.clone(),
            issuer: issuer.address.clone(),
        }),
    );
    let r = execute_nft_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &bad_issuer_burn,
        11,
        compat,
    );
    assert!(!r.accepted);
    assert_eq!(r.code, "nft_not_issuer_burnable");

    // Unsolicited permanent-lock spam guard: TRANSFER_LOCKED | BURN_LOCKED to owner != issuer MUST reject!
    let seq = ledger.account(&issuer.address).unwrap().sequence + 1;
    let spam_mint = sign_nft_tx(
        &genesis,
        &issuer,
        seq,
        50_000,
        NFT_MINT_TRANSACTION_KIND,
        NftTransactionOperation::NftMint(NftMintOperation {
            issuer: issuer.address.clone(),
            collection_id: "SPAM-COL".to_string(),
            serial: 1,
            owner: owner.address.clone(),
            metadata_hash: "44".repeat(48),
            metadata_uri: "ipfs://spam".to_string(),
            flags: 0,
            collection_flags: NFT_COLLECTION_FLAG_TRANSFER_LOCKED | NFT_COLLECTION_FLAG_BURN_LOCKED,
            issuer_transfer_fee: 0,
        }),
    );
    let r =
        execute_nft_transaction_with_compatibility(&genesis, &mut ledger, &spam_mint, 11, compat);
    assert!(!r.accepted);
    assert_eq!(r.code, "nft_unsolicited_permanent_lock_forbidden");
}

#[test]
fn pillar2_fastpay_fee_v1_formula_admission_and_account_soft_cap() {
    // Contract §2.2 worked examples:
    assert_eq!(fastpay_minimum_fee_atoms(500, 1, 1).unwrap(), 27);
    assert_eq!(fastpay_minimum_fee_atoms(700, 1, 2).unwrap(), 1_028);
    assert_eq!(fastpay_minimum_fee_atoms(1_200, 5, 1).unwrap(), 33);
    assert_eq!(fastpay_minimum_fee_atoms(400, 1, 0).unwrap(), 27);

    // Per-account soft cap (4_096): net-new objects rejected, consolidation/unwrap allowed!
    let owner = test_key(50);
    let mut ledger = LedgerState::empty();
    for i in 0..MAX_OWNED_OBJECTS_PER_ACCOUNT_SOFT_CAP {
        ledger.owned_objects.push(OwnedObject {
            id: format!("obj-{i:05}"),
            owner_pubkey_hex: owner.public_key_hex.clone(),
            asset: OWNED_NATIVE_ASSET.to_string(),
            value: 100,
            version: 1,
        });
    }
    let two_outputs = [owner.public_key_hex.as_str(), owner.public_key_hex.as_str()];
    let cap_err = ensure_owned_object_capacity_v2(&ledger, &owner.public_key_hex, 1, &two_outputs)
        .unwrap_err();
    assert!(matches!(
        cap_err,
        OwnedTransferError::AccountOwnedObjectCapExceeded { .. }
    ));
    // Consolidation (2 inputs -> 1 output) and unwrap (1 input -> 0 outputs) ALWAYS succeed!
    let one_output = [owner.public_key_hex.as_str()];
    ensure_owned_object_capacity_v2(&ledger, &owner.public_key_hex, 2, &one_output).unwrap();
    ensure_owned_object_capacity_v2(&ledger, &owner.public_key_hex, 1, &[]).unwrap();

    // Admission enforces fee_v1 when active and allows 0-fee pre-activation!
    let val = test_key(51);
    let committee = FastPayRecoveryCommitteeV1::from_public_keys(
        "postfiat-train-a-test".to_string(),
        "aa".repeat(48),
        1,
        1,
        1,
        100,
        vec![("val-0".to_string(), val.public_key_hex.clone())],
    )
    .expect("committee");
    let domain = committee.certificate_domain();
    let policy = FastPayRecoveryPolicyV1 {
        schema: FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
        activation_height: 1,
        max_validity_blocks: 50,
        max_recovery_blocks: 50,
    };
    let mut single_ledger = LedgerState::empty();
    single_ledger.owned_objects.push(OwnedObject {
        id: "owned-1".to_string(),
        owner_pubkey_hex: owner.public_key_hex.clone(),
        asset: OWNED_NATIVE_ASSET.to_string(),
        value: 10_000,
        version: 1,
    });
    let mut order = OwnedTransferOrderV3 {
        domain: domain.clone(),
        recovery: FastPayOrderRecoveryV1 {
            schema: FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
            committee_epoch: 1,
            lock_id: "00".repeat(48),
            valid_from_height: 1,
            expires_at_height: 20,
            recovery_closes_at_height: 40,
        },
        inputs: vec![OwnedObjectRef {
            id: "owned-1".to_string(),
            version: 1,
        }],
        outputs: vec![OwnedOutputSpec {
            owner_pubkey_hex: owner.public_key_hex.clone(),
            asset: OWNED_NATIVE_ASSET.to_string(),
            value: 10_000,
        }],
        fee: 0,
        nonce: 1,
        memos: Vec::new(),
    };
    order.recovery.lock_id = fastpay_transfer_lock_id_v1(&order);
    let signing = owned_transfer_v3_signing_bytes(&order);
    let sig = ml_dsa_65_sign_with_context(
        &owner.keypair.private_key,
        &signing,
        OWNED_TRANSFER_CONTEXT_V3,
    )
    .unwrap();
    let signed = SignedOwnedTransferOrderV3 {
        order,
        owner_pubkey_hex: owner.public_key_hex.clone(),
        owner_signature_hex: bytes_to_hex(&sig),
    };
    // Pre-activation (fee_v1_active = false): 0-fee self-transfer passes admission!
    validate_owned_transfer_v3_admission_with_fee_v1(
        &single_ledger,
        &signed,
        &domain,
        1,
        &policy,
        10,
        false,
    )
    .unwrap();
    // Post-activation (fee_v1_active = true): 0-fee self-transfer rejects with InsufficientFee!
    let err = validate_owned_transfer_v3_admission_with_fee_v1(
        &single_ledger,
        &signed,
        &domain,
        1,
        &policy,
        10,
        true,
    )
    .unwrap_err();
    assert!(matches!(err, OwnedTransferError::InsufficientFee { .. }));
}

/// Point 4 (A9, B01): before NFT scaling is active, `nft_burn` keeps main's
/// check order (burned, owner, burn-locked); the new order applies only after.
#[test]
fn nft_burn_keeps_the_old_check_order_until_nft_scaling_is_active() {
    let genesis = test_genesis();
    let issuer = test_key(71);
    let owner = test_key(72);
    let other = test_key(73);
    let mut ledger = LedgerState::new(vec![
        funded_account(&issuer, 10_000_000),
        funded_account(&owner, 10_000_000),
        funded_account(&other, 10_000_000),
    ]);
    let mint = sign_nft_tx(
        &genesis,
        &issuer,
        1,
        50_000,
        NFT_MINT_TRANSACTION_KIND,
        NftTransactionOperation::NftMint(NftMintOperation {
            issuer: issuer.address.clone(),
            collection_id: "COL".to_string(),
            serial: 1,
            owner: owner.address.clone(),
            metadata_hash: "11".repeat(48),
            metadata_uri: "ipfs://x".to_string(),
            flags: NFT_FLAG_TRANSFERABLE,
            collection_flags: NFT_COLLECTION_FLAG_BURN_LOCKED,
            issuer_transfer_fee: 0,
        }),
    );
    let strict = AssetExecutionCompatibility::strict();
    let minted =
        execute_nft_transaction_with_compatibility(&genesis, &mut ledger, &mint, 5, strict);
    assert!(minted.accepted, "{minted:?}");
    let id = nft_id(&genesis.chain_id, &issuer.address, "COL", 1).expect("nft id");
    let burn = sign_nft_tx(
        &genesis,
        &other,
        1,
        50_000,
        NFT_BURN_TRANSACTION_KIND,
        NftTransactionOperation::NftBurn(NftBurnOperation {
            nft_id: id,
            owner: other.address.clone(),
            issuer: String::new(),
        }),
    );
    let scheduled = strict.with_nft_scaling_v1_activation_height(Some(10));
    for (height, compatibility) in [(6, strict), (6, scheduled), (9, scheduled)] {
        let receipt = execute_nft_transaction_with_compatibility(
            &genesis,
            &mut ledger.clone(),
            &burn,
            height,
            compatibility,
        );
        assert_eq!(receipt.code, "nft_owner_mismatch", "{receipt:?}");
    }
    let receipt =
        execute_nft_transaction_with_compatibility(&genesis, &mut ledger, &burn, 10, scheduled);
    assert_eq!(receipt.code, "nft_collection_burn_locked", "{receipt:?}");
}
