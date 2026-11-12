use super::b23_signing::{offer_envelope_for_browser, sign_offer_fields_for_browser};
use postfiat_crypto_provider::{hex_to_bytes, ml_dsa_65_verify};
use postfiat_execution::{execute_offer_transaction, genesis_hash};
use postfiat_node::{
    init, submit_signed_offer_transaction_json_to_mempool, InitOptions,
    SignedOfferTransactionJsonSubmitOptions,
};
use postfiat_rpc_sdk::*;
use postfiat_storage::NodeStore;
use postfiat_types::*;
use serde_json::{json, Value};

fn assert_reviewed_fields(envelope: &Value, operation: &Value) {
    for (field, value) in operation.as_object().unwrap() {
        assert_eq!(
            &envelope["unsigned"][field], value,
            "reviewed field {field}"
        );
    }
}

fn wallet() -> (WalletBackupFile, WalletIdentity) {
    let mut seed = zeroize::Zeroizing::new([0u8; 32]);
    getrandom::getrandom(seed.as_mut()).unwrap();
    let backup = wallet_backup_from_master_seed(
        "b23-wallet-test",
        postfiat_crypto_provider::bytes_to_hex(seed.as_ref()),
        0,
    )
    .unwrap();
    let identity = wallet_identity_from_backup(&backup).unwrap();
    (backup, identity)
}

#[test]
fn browser_offers_round_trip_and_pass_node_admission_and_execution() {
    for expiration in [None, Some(0), Some(100)] {
        let dir = tempfile::tempdir().unwrap();
        init(InitOptions {
            data_dir: dir.path().to_owned(),
            chain_id: "b23-wallet-test".into(),
            node_id: "validator-0".into(),
            validator_count: 2,
        })
        .unwrap();
        let store = NodeStore::new(dir.path());
        let genesis = store.read_genesis().unwrap();
        let mut ledger = store.read_ledger().unwrap();
        let (backup, identity) = wallet();
        ledger.accounts.push(Account::new(
            identity.address.clone(),
            10_000_000,
            Some(identity.public_key_hex.clone()),
        ));
        let issuer = ledger.accounts[0].address.clone();
        let asset = AssetDefinition::new(&genesis.chain_id, &issuer, "B23TEST", 1, 6).unwrap();
        ledger.trustlines.push(
            TrustLine::new(
                identity.address.clone(),
                issuer,
                asset.asset_id.clone(),
                100_000,
                0,
            )
            .unwrap(),
        );
        ledger.asset_definitions.push(asset.clone());
        store.write_ledger(&ledger).unwrap();
        let tip = dir.path().join("chain_tip.json");
        if tip.exists() {
            std::fs::remove_file(tip).unwrap();
        }
        let mut create = json!({"operation":"offer_create", "owner":identity.address,
            "taker_gets_asset_id":"PFT", "taker_gets_amount":100,
            "taker_pays_asset_id":asset.asset_id, "taker_pays_amount":10});
        if let Some(height) = expiration {
            create["expiration_height"] = json!(height);
        }
        let fields = json!({"chain_id":genesis.chain_id, "genesis_hash":genesis_hash(&genesis),
            "protocol_version":genesis.protocol_version, "source":identity.address,
            "fee":100_000, "sequence":1, "operation":create});
        let envelope = sign_offer_fields_for_browser(&backup, &fields.to_string()).unwrap();
        assert_reviewed_fields(&envelope, &create);
        assert_eq!(
            envelope["unsigned"].get("expiration_height"),
            expiration.map(Value::from).as_ref()
        );
        let signed: SignedOfferTransaction = serde_json::from_value(envelope.clone()).unwrap();
        // A browser-only presentation change must never alter the node's signed bytes.
        let sdk = wallet_sign_offer_transaction_from_fields(
            &backup,
            serde_json::from_value(fields).unwrap(),
        )
        .unwrap();
        assert_eq!(
            signed.unsigned.signing_bytes(),
            sdk.unsigned.signing_bytes()
        );
        let entry = submit_signed_offer_transaction_json_to_mempool(
            SignedOfferTransactionJsonSubmitOptions {
                data_dir: dir.path().to_owned(),
                signed_offer_transaction_json: envelope.to_string(),
            },
        )
        .unwrap();
        assert_eq!(entry.transaction, signed);
        let receipt = execute_offer_transaction(&genesis, &mut ledger, &signed, 1);
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(ledger.offers.len(), 1);
        let cancel = json!({"operation":"offer_cancel", "owner":identity.address, "offer_id":ledger.offers[0].offer_id});
        let cancel_fields = json!({"chain_id":genesis.chain_id, "genesis_hash":genesis_hash(&genesis),
            "protocol_version":genesis.protocol_version, "source":identity.address,
            "fee":100_000, "sequence":2, "operation":cancel});
        let cancel_envelope =
            sign_offer_fields_for_browser(&backup, &cancel_fields.to_string()).unwrap();
        assert_reviewed_fields(&cancel_envelope, &cancel);
        submit_signed_offer_transaction_json_to_mempool(SignedOfferTransactionJsonSubmitOptions {
            data_dir: dir.path().to_owned(),
            signed_offer_transaction_json: cancel_envelope.to_string(),
        })
        .unwrap(); // Admission replays the pending create before the cancel.
        let signed: SignedOfferTransaction = serde_json::from_value(cancel_envelope).unwrap();
        let receipt = execute_offer_transaction(&genesis, &mut ledger, &signed, 1);
        assert!(receipt.accepted, "{receipt:?}");
        assert_eq!(ledger.offers[0].state, OFFER_STATE_CANCELED);
        println!("offer_create expiration={expiration:?}: node admission + execution accepted; offer_cancel accepted");
    }
}

#[test]
fn browser_offer_quote_presentation_and_unknown_fields() {
    let (backup, identity) = wallet();
    for expiration in [None, Some(0), Some(100)] {
        let mut operation = json!({"operation":"offer_create", "owner":identity.address,
            "taker_gets_asset_id":"PFT", "taker_gets_amount":100,
            "taker_pays_asset_id":"12".repeat(48), "taker_pays_amount":10});
        if let Some(height) = expiration {
            operation["expiration_height"] = json!(height);
        }
        let fields = json!({"chain_id":backup.chain_id, "genesis_hash":"22".repeat(48),
            "protocol_version":1, "source":identity.address, "fee":100_000, "sequence":1, "operation":operation});
        let signed = wallet_sign_offer_transaction_from_fields(
            &backup,
            serde_json::from_value(fields.clone()).unwrap(),
        )
        .unwrap();
        assert_reviewed_fields(
            &offer_envelope_for_browser(&signed, &operation).unwrap(),
            &operation,
        );
        let mut unknown = fields;
        unknown["operation"]["unsupported_review_field"] = json!(true);
        assert!(sign_offer_fields_for_browser(&backup, &unknown.to_string()).is_err());
    }
}

#[test]
fn browser_b23_asset_fields_and_payments_preserve_review_and_verify() {
    let (backup, identity) = wallet();
    let mut policy = PftlUniswapPrimaryMarketPolicyV2 {
        policy_hash: String::new(),
        policy_epoch: 2,
        spread_recipient: Some("management".into()),
        issue_multiplier_bps: 10_000,
        redeem_multiplier_bps: 9980,
        issue_capacity_atoms: 100_000_000,
        redeem_capacity_atoms: 100_000_000,
        min_order_atoms: 1_000_000,
        max_order_atoms: 50_000_000,
        valid_from_height: 1,
        expires_at_height: 100,
        max_nav_age_blocks: 100,
        pricing_nav_epoch: 1,
        pricing_reserve_packet_hash: "33".repeat(48),
    };
    policy.policy_hash = policy.computed_hash();
    let operations = [
        json!({"operation":"pftl_uniswap_primary_redeem", "owner":identity.address,
            "settlement_recipient":"redeemer-credit", "settlement_source_asset_id":"12".repeat(48),
            "route_id":"b23-route", "redemption_nonce":"44".repeat(32), "nav_amount_atoms":1_000_000,
            "min_settlement_value_atoms":998_000, "route_epoch":1,"policy_epoch":2,
            "policy_hash":policy.policy_hash,"pricing_nav_epoch":1,"pricing_reserve_packet_hash":"33".repeat(48),"expires_at_height":100}),
        json!({"operation":"pftl_uniswap_redemption_fund", "funder":identity.address,
            "route_id":"b23-route", "settlement_asset_id":"12".repeat(48),
            "funding_nonce":"44".repeat(32), "funding_sequence":1,"amount_atoms":1_000_000}),
        json!({"operation":"pftl_uniswap_route_epoch_advance", "operator":identity.address,
            "route_id":"b23-route", "prior_route_epoch":1, "next_route_epoch":2,
            "next_route_config_digest":"55".repeat(48),"live_value_enabled":true,
            "settlement_source_asset_ids":["12".repeat(48)], "next_primary_market_policy":policy}),
        json!({"operation":"issued_payment", "from":identity.address,"to":"recipient",
            "asset_id":"12".repeat(48),"issuer":"issuer", "amount":1_000_000}),
    ];
    for operation in operations {
        let fields = json!({"chain_id":backup.chain_id,"genesis_hash":"22".repeat(48),
            "protocol_version":1,"source":identity.address,"fee":100_000,"sequence":1,"operation":operation});
        // Same JSON -> SDK -> JSON path as wallet_sign_asset_transaction_fields.
        let signed = wallet_sign_asset_transaction_from_fields(
            &backup,
            serde_json::from_str(&fields.to_string()).unwrap(),
        )
        .unwrap();
        let envelope = serde_json::to_value(&signed).unwrap();
        assert_reviewed_fields(&envelope, &operation);
        assert!(ml_dsa_65_verify(
            &hex_to_bytes(&signed.public_key_hex).unwrap(),
            &signed.unsigned.signing_bytes(),
            &hex_to_bytes(&signed.signature_hex).unwrap()
        ));
    }
    let transfer = json!({"chain_id":backup.chain_id,"genesis_hash":"22".repeat(48),"protocol_version":1,
        "to":"recipient", "amount":1_000_000,"fee":100,"sequence":1});
    let signed = wallet_sign_transfer_from_fields(
        &backup,
        serde_json::from_str(&transfer.to_string()).unwrap(),
    )
    .unwrap();
    assert!(ml_dsa_65_verify(
        &hex_to_bytes(&signed.public_key_hex).unwrap(),
        &signed.unsigned.signing_bytes(),
        &hex_to_bytes(&signed.signature_hex).unwrap()
    ));
    let envelope = serde_json::to_value(&signed).unwrap();
    for (key, value) in transfer.as_object().unwrap() {
        assert_eq!(&envelope["unsigned"][key], value);
    }
    let mut payment = transfer;
    payment["memos"] =
        json!([{ "memo_type":"623233", "memo_format":"74657874", "memo_data":"6f6b" }]);
    let signed = wallet_sign_payment_v2_from_fields(
        &backup,
        serde_json::from_str(&payment.to_string()).unwrap(),
    )
    .unwrap();
    assert!(ml_dsa_65_verify(
        &hex_to_bytes(&signed.public_key_hex).unwrap(),
        &signed.unsigned.signing_bytes(),
        &hex_to_bytes(&signed.signature_hex).unwrap()
    ));
    let envelope = serde_json::to_value(&signed).unwrap();
    for (key, value) in payment.as_object().unwrap() {
        assert_eq!(&envelope["unsigned"][key], value);
    }
}
