fn atomic_swap_hash_fixture() -> SignedAtomicSwapTransaction {
    let owner_0 = format!("pf{}", "01".repeat(20));
    let owner_1 = format!("pf{}", "02".repeat(20));
    SignedAtomicSwapTransaction {
        unsigned: postfiat_types::UnsignedAtomicSwapTransaction {
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
            leg_0: postfiat_types::AtomicSwapLeg {
                owner: owner_0.clone(),
                recipient: owner_1.clone(),
                issuer: format!("pf{}", "03".repeat(20)),
                asset_id: "10".repeat(48),
                amount: 20_000,
                sequence: 3,
                fee: 22,
            },
            leg_1: postfiat_types::AtomicSwapLeg {
                owner: owner_1.clone(),
                recipient: owner_0.clone(),
                issuer: format!("pf{}", "04".repeat(20)),
                asset_id: "20".repeat(48),
                amount: 164_020,
                sequence: 5,
                fee: 22,
            },
        },
        authorization_0: postfiat_types::AtomicSwapAuthorization {
            owner: owner_0,
            algorithm_id: "ML-DSA-65".to_string(),
            public_key_hex: "aa".to_string(),
            signature_hex: "bb".to_string(),
        },
        authorization_1: postfiat_types::AtomicSwapAuthorization {
            owner: owner_1,
            algorithm_id: "ML-DSA-65".to_string(),
            public_key_hex: "cc".to_string(),
            signature_hex: "dd".to_string(),
        },
    }
}

#[test]
fn atomic_swap_tx_id_golden_vector_and_domain_separation() {
    let transaction = atomic_swap_hash_fixture();
    assert_eq!(
        "f38e8cde0690ed857ba747de329ca7d9dafd685b62002a989ef946ae923a4e84e35ffbc38e3455668371f8212bccda2a",
        atomic_swap_transaction_tx_id(&transaction)
    );
    assert_ne!(
        hash_hex(
            "postfiat.asset_transaction.tx_id.v1",
            &transaction.tx_id_preimage_bytes()
        ),
        atomic_swap_transaction_tx_id(&transaction)
    );
}

#[test]
fn atomic_swap_tx_id_covers_every_envelope_and_authorization_field() {
    let base = atomic_swap_hash_fixture();
    let base_hash = atomic_swap_transaction_tx_id(&base);
    macro_rules! assert_hash_changes {
        ($field:literal, $mutate:expr) => {{
            let mut changed = base.clone();
            $mutate(&mut changed);
            assert_ne!(
                base_hash,
                atomic_swap_transaction_tx_id(&changed),
                "{} did not affect atomic swap tx id",
                $field
            );
        }};
    }

    assert_hash_changes!("chain_id", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .chain_id += "-other");
    assert_hash_changes!("genesis_hash", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .genesis_hash.replace_range(0..2, "ab"));
    assert_hash_changes!("protocol_version", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .protocol_version += 1);
    assert_hash_changes!("address_namespace", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .address_namespace += ".other");
    assert_hash_changes!("signature_algorithm_id", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .signature_algorithm_id += "-other");
    assert_hash_changes!("rfq_hash", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .rfq_hash.replace_range(0..2, "bc"));
    assert_hash_changes!("market_envelope_hash", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .market_envelope_hash.replace_range(0..2, "cd"));
    assert_hash_changes!("nav_epoch", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .nav_epoch += 1);
    assert_hash_changes!("expires_at_height", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .expires_at_height += 1);
    assert_hash_changes!("swap_nonce", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .swap_nonce.replace_range(0..2, "de"));

    assert_hash_changes!("leg_0.owner", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .owner.replace_range(2..4, "05"));
    assert_hash_changes!("leg_0.recipient", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .recipient.replace_range(2..4, "05"));
    assert_hash_changes!("leg_0.issuer", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .issuer.replace_range(2..4, "05"));
    assert_hash_changes!("leg_0.asset_id", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .asset_id.replace_range(0..2, "11"));
    assert_hash_changes!("leg_0.amount", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .amount += 1);
    assert_hash_changes!("leg_0.sequence", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .sequence += 1);
    assert_hash_changes!("leg_0.fee", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_0
        .fee += 1);

    assert_hash_changes!("leg_1.owner", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_1
        .owner.replace_range(2..4, "06"));
    assert_hash_changes!("leg_1.recipient", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_1
        .recipient.replace_range(2..4, "06"));
    assert_hash_changes!("leg_1.issuer", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_1
        .issuer.replace_range(2..4, "06"));
    assert_hash_changes!("leg_1.asset_id", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_1
        .asset_id.replace_range(0..2, "21"));
    assert_hash_changes!("leg_1.amount", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_1
        .amount += 1);
    assert_hash_changes!("leg_1.sequence", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_1
        .sequence += 1);
    assert_hash_changes!("leg_1.fee", |tx: &mut SignedAtomicSwapTransaction| tx
        .unsigned
        .leg_1
        .fee += 1);

    assert_hash_changes!("authorization_0.algorithm_id", |tx: &mut SignedAtomicSwapTransaction| tx
        .authorization_0
        .algorithm_id += "-other");
    assert_hash_changes!("authorization_0.public_key_hex", |tx: &mut SignedAtomicSwapTransaction| tx
        .authorization_0
        .public_key_hex = "ab".to_string());
    assert_hash_changes!("authorization_0.signature_hex", |tx: &mut SignedAtomicSwapTransaction| tx
        .authorization_0
        .signature_hex = "bc".to_string());
    assert_hash_changes!("authorization_1.algorithm_id", |tx: &mut SignedAtomicSwapTransaction| tx
        .authorization_1
        .algorithm_id += "-other");
    assert_hash_changes!("authorization_1.public_key_hex", |tx: &mut SignedAtomicSwapTransaction| tx
        .authorization_1
        .public_key_hex = "cd".to_string());
    assert_hash_changes!("authorization_1.signature_hex", |tx: &mut SignedAtomicSwapTransaction| tx
        .authorization_1
        .signature_hex = "de".to_string());

    let mut owner_only = base.clone();
    owner_only.authorization_0.owner = owner_only.unsigned.leg_1.owner.clone();
    assert!(owner_only.validate().is_err());
    assert_eq!(base_hash, atomic_swap_transaction_tx_id(&owner_only));
}

#[test]
fn atomic_swap_both_parties_sign_the_identical_whole_trade_preimage() {
    let key_0 = postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[0x31; 32]);
    let key_1 = postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[0x32; 32]);
    let mut transaction = atomic_swap_hash_fixture();
    transaction.unsigned.leg_0.owner = address_from_public_key(&key_0.public_key);
    transaction.unsigned.leg_0.recipient = address_from_public_key(&key_1.public_key);
    transaction.unsigned.leg_1.owner = transaction.unsigned.leg_0.recipient.clone();
    transaction.unsigned.leg_1.recipient = transaction.unsigned.leg_0.owner.clone();
    transaction.authorization_0.owner = transaction.unsigned.leg_0.owner.clone();
    transaction.authorization_1.owner = transaction.unsigned.leg_1.owner.clone();
    transaction.authorization_0.public_key_hex = bytes_to_hex(&key_0.public_key);
    transaction.authorization_1.public_key_hex = bytes_to_hex(&key_1.public_key);

    let envelope = transaction.unsigned.signing_bytes();
    let signature_0 = postfiat_crypto_provider::ml_dsa_65_sign_with_context_seed(
        &key_0.private_key,
        &envelope,
        postfiat_crypto_provider::TX_SIGNATURE_CONTEXT,
        &[0x41; 32],
    )
    .expect("sign authorization 0");
    let signature_1 = postfiat_crypto_provider::ml_dsa_65_sign_with_context_seed(
        &key_1.private_key,
        &envelope,
        postfiat_crypto_provider::TX_SIGNATURE_CONTEXT,
        &[0x42; 32],
    )
    .expect("sign authorization 1");
    transaction.authorization_0.signature_hex = bytes_to_hex(&signature_0);
    transaction.authorization_1.signature_hex = bytes_to_hex(&signature_1);

    assert!(transaction.validate().is_ok());
    assert!(postfiat_crypto_provider::ml_dsa_65_verify(
        &key_0.public_key,
        &envelope,
        &signature_0
    ));
    assert!(postfiat_crypto_provider::ml_dsa_65_verify(
        &key_1.public_key,
        &envelope,
        &signature_1
    ));

    let mut substituted = transaction.unsigned.clone();
    substituted.leg_0.amount += 1;
    let substituted = substituted.signing_bytes();
    assert!(!postfiat_crypto_provider::ml_dsa_65_verify(
        &key_0.public_key,
        &substituted,
        &signature_0
    ));
    assert!(!postfiat_crypto_provider::ml_dsa_65_verify(
        &key_1.public_key,
        &substituted,
        &signature_1
    ));
}
