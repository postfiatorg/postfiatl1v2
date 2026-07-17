fn atomic_swap_fixture() -> SignedAtomicSwapTransaction {
    let owner_0 = format!("pf{}", "01".repeat(20));
    let owner_1 = format!("pf{}", "02".repeat(20));
    SignedAtomicSwapTransaction {
        unsigned: UnsignedAtomicSwapTransaction {
            chain_id: "postfiat-local".to_string(),
            genesis_hash: "aa".repeat(48),
            protocol_version: 1,
            address_namespace: ADDRESS_NAMESPACE.to_string(),
            signature_algorithm_id: "ML-DSA-65".to_string(),
            rfq_hash: "bb".repeat(48),
            market_envelope_hash: "cc".repeat(48),
            nav_epoch: 7,
            expires_at_height: 99,
            swap_nonce: "dd".repeat(48),
            leg_0: AtomicSwapLeg {
                owner: owner_0.clone(),
                recipient: owner_1.clone(),
                issuer: format!("pf{}", "03".repeat(20)),
                asset_id: "10".repeat(48),
                amount: 20_000,
                sequence: 3,
                fee: 22,
            },
            leg_1: AtomicSwapLeg {
                owner: owner_1.clone(),
                recipient: owner_0.clone(),
                issuer: format!("pf{}", "04".repeat(20)),
                asset_id: "20".repeat(48),
                amount: 164_020,
                sequence: 5,
                fee: 22,
            },
        },
        authorization_0: AtomicSwapAuthorization {
            owner: owner_0,
            algorithm_id: "ML-DSA-65".to_string(),
            public_key_hex: "aa".to_string(),
            signature_hex: "bb".to_string(),
        },
        authorization_1: AtomicSwapAuthorization {
            owner: owner_1,
            algorithm_id: "ML-DSA-65".to_string(),
            public_key_hex: "cc".to_string(),
            signature_hex: "dd".to_string(),
        },
    }
}

#[test]
fn atomic_swap_signing_and_tx_id_preimages_are_golden() {
    let transaction = atomic_swap_fixture();
    assert!(transaction.validate().is_ok());
    let owner_0 = format!("pf{}", "01".repeat(20));
    let owner_1 = format!("pf{}", "02".repeat(20));
    let issuer_0 = format!("pf{}", "03".repeat(20));
    let issuer_1 = format!("pf{}", "04".repeat(20));
    let expected_signing = format!(
        "postfiat.atomic_swap_transaction.v1\nchain_id=postfiat-local\ngenesis_hash={}\nprotocol_version=1\naddress_namespace={}\nsignature_algorithm_id=ML-DSA-65\nrfq_hash={}\nmarket_policy_hash={}\nnav_epoch=7\nexpires_at_height=99\nswap_nonce={}\nleg_0=\nowner={}\nrecipient={}\nissuer={}\nasset_id={}\namount=20000\nsequence=3\nfee=22\nleg_1=\nowner={}\nrecipient={}\nissuer={}\nasset_id={}\namount=164020\nsequence=5\nfee=22\n",
        "aa".repeat(48),
        ADDRESS_NAMESPACE,
        "bb".repeat(48),
        "cc".repeat(48),
        "dd".repeat(48),
        owner_0,
        owner_1,
        issuer_0,
        "10".repeat(48),
        owner_1,
        owner_0,
        issuer_1,
        "20".repeat(48),
    )
    .into_bytes();
    assert_eq!(expected_signing, transaction.unsigned.signing_bytes());

    let mut expected_tx_id = expected_signing;
    expected_tx_id.extend_from_slice(
        b"algorithm=ML-DSA-65\npublic_key=aa\nsignature=bb\nalgorithm=ML-DSA-65\npublic_key=cc\nsignature=dd\n",
    );
    assert_eq!(expected_tx_id, transaction.tx_id_preimage_bytes());

    let json = serde_json::to_string(&transaction).expect("serialize atomic swap");
    assert!(json.contains("\"market_envelope_hash\""));
    assert!(!json.contains("\"market_policy_hash\""));
    let decoded: SignedAtomicSwapTransaction =
        serde_json::from_str(&json).expect("deserialize atomic swap");
    assert_eq!(transaction, decoded);
}

#[test]
fn atomic_swap_validation_rejects_structural_malleability() {
    let base = atomic_swap_fixture();

    let mut changed = base.clone();
    std::mem::swap(&mut changed.unsigned.leg_0, &mut changed.unsigned.leg_1);
    std::mem::swap(
        &mut changed.authorization_0,
        &mut changed.authorization_1,
    );
    assert!(changed.validate().is_err(), "reversed canonical legs accepted");

    let mut changed = base.clone();
    std::mem::swap(
        &mut changed.authorization_0,
        &mut changed.authorization_1,
    );
    assert!(changed.validate().is_err(), "swapped authorizations accepted");

    let mut changed = base.clone();
    changed.unsigned.leg_1.owner = changed.unsigned.leg_0.owner.clone();
    assert!(changed.validate().is_err(), "same owner accepted");

    let mut changed = base.clone();
    changed.unsigned.leg_1.asset_id = changed.unsigned.leg_0.asset_id.clone();
    assert!(changed.validate().is_err(), "same asset accepted");

    let mut changed = base.clone();
    changed.unsigned.leg_1.recipient = format!("pf{}", "05".repeat(20));
    assert!(changed.validate().is_err(), "nonreciprocal legs accepted");

    let mut changed = base.clone();
    changed.unsigned.leg_0.issuer = changed.unsigned.leg_0.owner.clone();
    assert!(changed.validate().is_err(), "issuer owner endpoint accepted");

    let mut changed = base.clone();
    changed.unsigned.leg_1.issuer = changed.unsigned.leg_1.recipient.clone();
    assert!(
        changed.validate().is_err(),
        "issuer recipient endpoint accepted"
    );

    for mutate in [
        |tx: &mut SignedAtomicSwapTransaction| tx.unsigned.leg_0.amount = 0,
        |tx: &mut SignedAtomicSwapTransaction| tx.unsigned.leg_1.amount = 0,
    ] {
        let mut changed = base.clone();
        mutate(&mut changed);
        assert!(changed.validate().is_err(), "zero amount accepted");
    }
}

#[test]
fn legacy_receipt_json_omits_atomic_swap_extension() {
    let receipt = Receipt::accepted("tx-id", "legacy receipt");
    let json = serde_json::to_string(&receipt).expect("serialize legacy receipt");
    assert_eq!(
        json,
        "{\"tx_id\":\"tx-id\",\"accepted\":true,\"code\":\"accepted\",\"message\":\"legacy receipt\",\"fee_charged\":0,\"fee_burned\":0,\"minimum_fee\":0,\"account_reserve\":0,\"state_expansion_fee\":0}"
    );
    assert!(!json.contains("atomic_swap_legs"));
    let decoded: Receipt = serde_json::from_str(&json).expect("deserialize legacy receipt");
    assert_eq!(decoded.atomic_swap_legs, None);
    assert_eq!(json, serde_json::to_string(&decoded).expect("reserialize receipt"));
}

#[test]
fn atomic_swap_validation_rejects_malformed_fields() {
    let base = atomic_swap_fixture();
    macro_rules! assert_invalid {
        ($label:literal, $mutate:expr) => {{
            let mut changed = base.clone();
            $mutate(&mut changed);
            assert!(changed.validate().is_err(), "{} accepted", $label);
        }};
    }

    assert_invalid!("blank chain", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .chain_id = " ".to_string());
    assert_invalid!("uppercase genesis", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .genesis_hash = "AA".repeat(48));
    assert_invalid!("zero protocol version", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .protocol_version = 0);
    assert_invalid!("blank namespace", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .address_namespace = String::new());
    assert_invalid!("malformed rfq hash", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .rfq_hash = "00".repeat(47));
    assert_invalid!("malformed envelope hash", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .market_envelope_hash = "gg".repeat(48));
    assert_invalid!("malformed nonce", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .swap_nonce = "DD".repeat(48));
    assert_invalid!("malformed owner", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .owner = "pf01".to_string());
    assert_invalid!("malformed recipient", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .recipient = format!("PF{}", "02".repeat(20)));
    assert_invalid!("malformed issuer", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .issuer = format!("pf{}", "GG".repeat(20)));
    assert_invalid!("malformed asset", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .asset_id = "10".repeat(47));
    assert_invalid!("authorization owner mismatch", |tx: &mut SignedAtomicSwapTransaction| tx
        .authorization_0
        .owner = tx.unsigned.leg_1.owner.clone());
    assert_invalid!("authorization algorithm mismatch", |tx: &mut SignedAtomicSwapTransaction| tx
        .authorization_1
        .algorithm_id = "other".to_string());
    assert_invalid!("malformed public key", |tx: &mut SignedAtomicSwapTransaction| tx
        .authorization_0
        .public_key_hex = "AA".to_string());
    assert_invalid!("malformed signature", |tx: &mut SignedAtomicSwapTransaction| tx
        .authorization_1
        .signature_hex = "d".to_string());
}
