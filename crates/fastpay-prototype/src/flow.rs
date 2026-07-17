//! End-to-end FastPay flow on L1 types: mint -> owner+validator sign (the
//! consensusless certificate) -> verify cert -> on-chain `apply_owned_transfer`
//! -> replay rejected. Self-contained demonstration of the full owned-value lane:
//! the off-chain consensusless certificate (validator lock+sign) AND the on-chain
//! landing (the real L1 execution function over `LedgerState`).
use postfiat_crypto_provider as crypto;
use postfiat_execution::{
    apply_owned_transfer, owned_transfer_signing_bytes, wrap_to_owned, OWNED_TRANSFER_CONTEXT,
};
use postfiat_types::{Account, LedgerState, OwnedObjectRef, OwnedOutputSpec, OwnedTransferOrder};

fn main() {
    // --- keygen (outside the timed path) ---
    let owner = crypto::ml_dsa_65_keygen().expect("owner keygen");
    let owner_pk_hex = crypto::bytes_to_hex(&owner.public_key);
    let recipient = crypto::ml_dsa_65_keygen().expect("recipient keygen");
    let recipient_pk_hex = crypto::bytes_to_hex(&recipient.public_key);
    let validators: Vec<_> = (0..3)
        .map(|_| crypto::ml_dsa_65_keygen().expect("vkeygen"))
        .collect();

    // --- ledger: move native value from the account lane into the owned lane ---
    let mut ledger = LedgerState::new(vec![Account {
        address: "pf-fastpay-demo-source".to_string(),
        balance: 100,
        sequence: 0,
        public_key_hex: None,
    }]);
    let obj_id =
        crypto::bytes_to_hex(&crypto::hash_bytes("postfiat.owned-mint.v1", b"seed-1")[..32]);
    wrap_to_owned(
        &mut ledger,
        "pf-fastpay-demo-source",
        owner_pk_hex.clone(),
        100,
        "PFT".to_string(),
        obj_id.clone(),
    )
    .expect("native-backed owned wrap");
    println!(
        "wrapped native-backed owned object id={}… owner={}… value=100 asset=PFT",
        &obj_id[..16],
        &owner_pk_hex[..12]
    );

    // --- order: 100 -> 90 to recipient + 9 change + 1 fee (conserved) ---
    let order = OwnedTransferOrder {
        domain: postfiat_types::OwnedCertificateDomain {
            schema: postfiat_types::OWNED_CERTIFICATE_DOMAIN_SCHEMA_V2.to_owned(),
            chain_id: "postfiat-fastpay-flow".to_owned(),
            genesis_hash: "ab".repeat(48),
            protocol_version: 1,
            registry_id: "cd".repeat(48),
        },
        inputs: vec![OwnedObjectRef {
            id: obj_id.clone(),
            version: 1,
        }],
        outputs: vec![
            OwnedOutputSpec {
                owner_pubkey_hex: recipient_pk_hex.clone(),
                value: 90,
                asset: "PFT".into(),
            },
            OwnedOutputSpec {
                owner_pubkey_hex: owner_pk_hex.clone(),
                value: 9,
                asset: "PFT".into(),
            },
        ],
        fee: 1,
        nonce: 1,
        memos: vec![postfiat_types::PaymentMemo {
            memo_type: "7061796d656e74".into(),         // "payment"
            memo_format: "746578742f706c61696e".into(), // "text/plain"
            memo_data: "68656c6c6f".into(),             // "hello"
        }],
    };
    let sb = owned_transfer_signing_bytes(&order);

    // --- off-chain consensusless certificate: owner + 3 validators sign ---
    let owner_sig =
        crypto::ml_dsa_65_sign_with_context(&owner.private_key, &sb, OWNED_TRANSFER_CONTEXT)
            .expect("owner sign");
    let votes: Vec<Vec<u8>> = validators
        .iter()
        .map(|v| {
            crypto::ml_dsa_65_sign_with_context(&v.private_key, &sb, OWNED_TRANSFER_CONTEXT)
                .expect("vsign")
        })
        .collect();
    // verify the certificate (owner auth + every validator vote)
    assert!(
        crypto::ml_dsa_65_verify_with_context(
            &owner.public_key,
            &sb,
            &owner_sig,
            OWNED_TRANSFER_CONTEXT,
        ),
        "owner authorization"
    );
    for (v, sig) in validators.iter().zip(votes.iter()) {
        assert!(
            crypto::ml_dsa_65_verify_with_context(&v.public_key, &sb, sig, OWNED_TRANSFER_CONTEXT,),
            "validator vote"
        );
    }
    println!("consensusless certificate: owner + 3 validators signed and verified (ML-DSA-65)");

    // --- on-chain landing: apply the certified order to the real ledger ---
    let outcome = apply_owned_transfer(&mut ledger, &order, &owner_pk_hex).expect("apply");
    println!(
        "on-chain apply_owned_transfer: consumed {} input(s), created {} output(s)",
        outcome.consumed,
        outcome.created.len()
    );
    let input_retired = !ledger.owned_objects.iter().any(|o| o.id == obj_id);
    println!(
        "  ledger owned_objects={} | input retired={} | outputs conserved (90+9+1=100)={}",
        ledger.owned_objects.len(),
        input_retired,
        outcome.created.iter().map(|o| o.value).sum::<u64>() + 1 == 100
    );

    // --- single-consumption: replay the same order is rejected ---
    let replay = apply_owned_transfer(&mut ledger, &order, &owner_pk_hex);
    println!(
        "  replay rejected (single-consumption): {}",
        replay.is_err()
    );

    println!("FastPay end-to-end flow OK — cert + on-chain apply on L1 LedgerState");
}
