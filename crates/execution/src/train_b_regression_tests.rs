use postfiat_crypto_provider::{
    address_from_public_key, bip340_sign_deterministic, bip340_xonly_pubkey_from_secret,
    bytes_to_hex, hex_to_bytes, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign,
    ml_dsa_65_sign_with_context, secp256k1_compressed_pubkey_from_secret,
    secp256k1_ecdsa_sign_low_s_der, MlDsa65KeyPair, ML_DSA_65_ALGORITHM,
};
use postfiat_types::{
    credential_delegated_accept_preimage, credential_id, credential_issuer_id, escrow_id,
    identity_controller_link_message, identity_handle_attestation_message, identity_handle_key,
    identity_handle_reassign_message, identity_handle_reservation_commitment,
    identity_nostr_proof_message, identity_old_chain_link_message, identity_reassignment_id,
    identity_rotate_account_message, issued_asset_id, nav_fee_accrual_key, Account,
    AccountSetOperation, AssetAccessSetOperation, AssetApprovalMode, AssetCreateOperation,
    AssetLapsedExitRule, AssetTransactionOperation, AssetWrapperMode,
    CredentialCreateAndAcceptOperation, CredentialDeleteOperation, CredentialPolicyRuleV1,
    CredentialPolicyV1, CredentialRequirementPairV1, CredentialRevokeOperation,
    EscrowCreateOperation, EscrowFinishOperation, EscrowTransactionOperation, Genesis,
    HandleAttestationV1, IdentityHandleReassignCancelOperation,
    IdentityHandleReassignExecuteOperation, IdentityHandleReassignProposeOperation,
    IdentityHandleReserveOperation, IdentityKind, IdentityLinkKind, IdentityLinkStatus,
    IdentityRegistrarRegisterOperation, IdentityRotateMode, IdentitySetActionV1,
    IdentitySetOperation, IdentityStatus, IssuedPaymentOperation, IssuerAdminTransferAction,
    IssuerAdminTransferOperation, IssuerRegisterOperation, IssuerUpdateOperation, LedgerState,
    NavAssetRegisterOperation, NavAttestorRegisterOperation, NavEpochFinalizeOperation,
    NavFeeClaimOperation, NavFeeScheduleAttachOperation, NavFeeScheduleCancelOperation,
    NavFeeScheduleInputV1, NavFeeScheduleSetOperation, NavProfileRegisterOperation,
    NavReserveAttestOperation, NavReserveSubmitOperation, OfferCreateOperation,
    OfferTransactionOperation, RegistrarCosignatureV1, SignedAssetTransaction,
    SignedEscrowTransaction, SignedOfferTransaction, TrustSetOperation, UnsignedAssetTransaction,
    UnsignedEscrowTransaction, UnsignedOfferTransaction, ACCOUNT_FLAG_REFUSE_UNSOLICITED_ASSETS,
    ACCOUNT_SET_TRANSACTION_KIND, ADDRESS_NAMESPACE, ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS,
    ASSET_ACCESS_SET_TRANSACTION_KIND, ASSET_CREATE_TRANSACTION_KIND,
    CREDENTIAL_CREATE_AND_ACCEPT_TRANSACTION_KIND, CREDENTIAL_DELEGATED_ACCEPT_DOMAIN,
    CREDENTIAL_DELETE_TRANSACTION_KIND, CREDENTIAL_DEPOSIT_ATOMS,
    CREDENTIAL_REVOKE_TRANSACTION_KIND, ESCROW_CREATE_TRANSACTION_KIND,
    ESCROW_FINISH_TRANSACTION_KIND, IDENTITY_CONTROLLER_LINK_DOMAIN,
    IDENTITY_HANDLE_ATTESTATION_DOMAIN, IDENTITY_HANDLE_HOLDBACK_BLOCKS,
    IDENTITY_HANDLE_REASSIGN_CANCEL_TRANSACTION_KIND, IDENTITY_HANDLE_REASSIGN_DOMAIN,
    IDENTITY_HANDLE_REASSIGN_EXECUTE_TRANSACTION_KIND,
    IDENTITY_HANDLE_REASSIGN_PROPOSE_TRANSACTION_KIND, IDENTITY_HANDLE_RESERVE_TRANSACTION_KIND,
    IDENTITY_REGISTRAR_REGISTER_TRANSACTION_KIND, IDENTITY_ROTATE_ACCOUNT_DOMAIN,
    IDENTITY_SET_TRANSACTION_KIND, ISSUED_PAYMENT_TRANSACTION_KIND,
    ISSUER_ADMIN_TRANSFER_TRANSACTION_KIND, ISSUER_REGISTER_TRANSACTION_KIND,
    ISSUER_UPDATE_TRANSACTION_KIND, NAV_ASSET_REGISTER_TRANSACTION_KIND,
    NAV_ATTESTOR_REGISTER_TRANSACTION_KIND, NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
    NAV_FEE_CLAIM_TRANSACTION_KIND, NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
    NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND, NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND,
    NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND, NAV_PROFILE_REGISTER_TRANSACTION_KIND,
    NAV_PROFILE_VERIFIER_MULTI_FETCH, NAV_RESERVE_ATTEST_TRANSACTION_KIND,
    NAV_RESERVE_SUBMIT_TRANSACTION_KIND, OFFER_CREATE_TRANSACTION_KIND, TRUST_SET_TRANSACTION_KIND,
};

use crate::{
    execute_asset_transaction_with_compatibility, execute_escrow_transaction_with_compatibility,
    execute_offer_transaction_with_compatibility, genesis_hash, issued_asset_supply,
    AssetExecutionCompatibility, NATIVE_PFT_ESCROW_ASSET_ID,
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
    Genesis::new_with_validator_count("postfiat-train-b-test", 1)
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

fn next_seq(ledger: &LedgerState, addr: &str) -> u64 {
    ledger.account(addr).map_or(1, |a| a.sequence + 1)
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

fn train_b_compat(activation: u64) -> AssetExecutionCompatibility {
    AssetExecutionCompatibility::strict()
        .with_asset_gate_v1_activation_height(Some(activation))
        .with_hive_identity_v1_activation_height(Some(activation))
        .with_nav_fee_v1_activation_height(Some(activation))
        .with_credential_did_v1_activation_height(Some(activation))
}

#[test]
fn pillar6_identity_registrar_links_rotation_kill_switch_and_reassignment() {
    let genesis = test_genesis();
    let alice = test_key(10);
    let alice_new = test_key(11);
    let bob = test_key(12);
    let controller = test_key(13);
    let reg_hot = test_key(14);
    let reg_cold_1 = test_key(15);
    let reg_cold_2 = test_key(16);
    let reg_cold_3 = test_key(17);

    let mut ledger = LedgerState::new(vec![
        funded_account(&alice, 1_000_000),
        funded_account(&alice_new, 1_000_000),
        funded_account(&bob, 1_000_000),
        funded_account(&controller, 1_000_000),
        funded_account(&reg_hot, 1_000_000),
        funded_account(&reg_cold_1, 1_000_000),
        funded_account(&reg_cold_2, 1_000_000),
        funded_account(&reg_cold_3, 1_000_000),
    ]);

    let compat = train_b_compat(100);
    let nostr_sk1 = [0x11_u8; 32];
    let nostr_pk1 = bytes_to_hex(&bip340_xonly_pubkey_from_secret(&nostr_sk1).unwrap());
    let nostr_msg1 = identity_nostr_proof_message("mint", &genesis.chain_id, &alice.address, 1);
    let nostr_sig1 = bytes_to_hex(&bip340_sign_deterministic(&nostr_sk1, &nostr_msg1).unwrap());

    // 1. Pre-activation MUST reject with hive_identity_not_active and leave ledger byte-identical.
    let pre_ledger = ledger.clone();
    let pre_mint = sign_asset_tx(
        &genesis,
        &alice,
        1,
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: alice.address.clone(),
            payload: IdentitySetActionV1::Mint {
                nostr_pubkey: nostr_pk1.clone(),
                nostr_proof_sig_hex: nostr_sig1.clone(),
                derivation_version: 1,
                key_index: 0,
                identity_kind: IdentityKind::Human,
                handle_attestation: None,
                reservation_salt_hex: None,
                relays: vec!["wss://relay.postfiat.org".to_string()],
                avatar_nft_id: None,
                message_key: None,
                pq_receive_address: None,
            },
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &pre_mint, 99, compat);
    assert!(!r.accepted);
    assert_eq!(r.code, "hive_identity_not_active");
    assert_eq!(ledger, pre_ledger);

    // 2. Register 2-of-3 registrar for tasknode.postfiat.org.
    let reg_tx = sign_asset_tx(
        &genesis,
        &reg_cold_1,
        next_seq(&ledger, &reg_cold_1.address),
        50_000,
        IDENTITY_REGISTRAR_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::IdentityRegistrarRegister(IdentityRegistrarRegisterOperation {
            caller: reg_cold_1.address.clone(),
            namespace: "tasknode.postfiat.org".to_string(),
            hot_attestation_account: reg_hot.address.clone(),
            cold_signers: [
                reg_cold_1.address.clone(),
                reg_cold_2.address.clone(),
                reg_cold_3.address.clone(),
            ],
            reassignment_timelock_blocks: IDENTITY_HANDLE_HOLDBACK_BLOCKS,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &reg_tx, 100, compat);
    assert!(r.accepted, "{r:?}");

    // 3. Reserve salted handles `goodalexander` and `hive-board`.
    let salt_hex = "aabbccddeeff00112233445566778899".to_string();
    let salt_bytes = hex_to_bytes(&salt_hex).unwrap();
    let bob_nostr_sk = [0x22_u8; 32];
    let bob_nostr_pk = bytes_to_hex(&bip340_xonly_pubkey_from_secret(&bob_nostr_sk).unwrap());
    let alice_comm =
        identity_handle_reservation_commitment("goodalexander", &nostr_pk1, &salt_bytes);
    let bob_comm = identity_handle_reservation_commitment("hive-board", &bob_nostr_pk, &salt_bytes);
    for (handle, comm) in [("goodalexander", alice_comm), ("hive-board", bob_comm)] {
        let res_tx = sign_asset_tx(
            &genesis,
            &reg_hot,
            next_seq(&ledger, &reg_hot.address),
            50_000,
            IDENTITY_HANDLE_RESERVE_TRANSACTION_KIND,
            AssetTransactionOperation::IdentityHandleReserve(IdentityHandleReserveOperation {
                registrar_account: reg_hot.address.clone(),
                namespace: "tasknode.postfiat.org".to_string(),
                handle: handle.to_string(),
                commitment_hash: comm,
                expires_at_height: 500,
            }),
        );
        let r = execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &res_tx,
            101,
            compat,
        );
        assert!(r.accepted, "reserve {handle}: {r:?}");
    }

    // 4. Bob cannot steal Alice's reserved handle `goodalexander`.
    let bob_msg = identity_nostr_proof_message("mint", &genesis.chain_id, &bob.address, 1);
    let bob_sig = bytes_to_hex(&bip340_sign_deterministic(&bob_nostr_sk, &bob_msg).unwrap());
    let bob_att_msg = identity_handle_attestation_message(
        &genesis.chain_id,
        "tasknode.postfiat.org",
        "goodalexander",
        &bob.address,
        500,
    );
    let bob_att_sig = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &reg_hot.keypair.private_key,
            &bob_att_msg,
            IDENTITY_HANDLE_ATTESTATION_DOMAIN.as_bytes(),
        )
        .unwrap(),
    );
    let steal_tx = sign_asset_tx(
        &genesis,
        &bob,
        next_seq(&ledger, &bob.address),
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: bob.address.clone(),
            payload: IdentitySetActionV1::Mint {
                nostr_pubkey: bob_nostr_pk.clone(),
                nostr_proof_sig_hex: bob_sig.clone(),
                derivation_version: 1,
                key_index: 0,
                identity_kind: IdentityKind::Human,
                handle_attestation: Some(HandleAttestationV1 {
                    namespace: "tasknode.postfiat.org".to_string(),
                    handle: "goodalexander".to_string(),
                    account: bob.address.clone(),
                    expires_at_height: 500,
                    registrar_account: reg_hot.address.clone(),
                    registrar_pubkey_hex: reg_hot.public_key_hex.clone(),
                    registrar_signature_hex: bob_att_sig,
                }),
                reservation_salt_hex: Some(salt_hex.clone()),
                relays: vec![],
                avatar_nft_id: None,
                message_key: None,
                pq_receive_address: None,
            },
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &steal_tx, 102, compat);
    assert!(!r.accepted);
    assert_eq!(r.code, "identity_handle_reservation_mismatch");

    // 5. Alice mints with valid BIP-340 proof + valid hot-key HandleAttestation + matching salt!
    let alice_att_msg = identity_handle_attestation_message(
        &genesis.chain_id,
        "tasknode.postfiat.org",
        "goodalexander",
        &alice.address,
        500,
    );
    let alice_att_sig = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &reg_hot.keypair.private_key,
            &alice_att_msg,
            IDENTITY_HANDLE_ATTESTATION_DOMAIN.as_bytes(),
        )
        .unwrap(),
    );
    let mint_tx = sign_asset_tx(
        &genesis,
        &alice,
        next_seq(&ledger, &alice.address),
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: alice.address.clone(),
            payload: IdentitySetActionV1::Mint {
                nostr_pubkey: nostr_pk1.clone(),
                nostr_proof_sig_hex: nostr_sig1,
                derivation_version: 1,
                key_index: 0,
                identity_kind: IdentityKind::Human,
                handle_attestation: Some(HandleAttestationV1 {
                    namespace: "tasknode.postfiat.org".to_string(),
                    handle: "goodalexander".to_string(),
                    account: alice.address.clone(),
                    expires_at_height: 500,
                    registrar_account: reg_hot.address.clone(),
                    registrar_pubkey_hex: reg_hot.public_key_hex.clone(),
                    registrar_signature_hex: alice_att_sig,
                }),
                reservation_salt_hex: Some(salt_hex.clone()),
                relays: vec!["wss://relay.postfiat.org".to_string()],
                avatar_nft_id: None,
                message_key: None,
                pq_receive_address: Some("pfpq1example".to_string()),
            },
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &mint_tx, 102, compat);
    assert!(r.accepted, "{r:?}");
    let hkey = identity_handle_key("tasknode.postfiat.org", "goodalexander");
    assert_eq!(
        ledger
            .identity_account_by_handle
            .get(&hkey)
            .map(String::as_str),
        Some(alice.address.as_str())
    );
    assert_eq!(
        ledger
            .identity_account_by_nostr
            .get(&nostr_pk1)
            .map(String::as_str),
        Some(alice.address.as_str())
    );

    // 6. Link OldChainAccount (low-S DER secp256k1) & AgentController (ML-DSA-65).
    let old_sk = [0x33_u8; 32];
    let old_pk_hex = bytes_to_hex(&secp256k1_compressed_pubkey_from_secret(&old_sk).unwrap());
    let old_target = "rOldChainAccount1234567890".to_string();
    let old_seq = next_seq(&ledger, &alice.address);
    let old_msg =
        identity_old_chain_link_message(&genesis.chain_id, &alice.address, &old_target, old_seq);
    let old_sig_hex = bytes_to_hex(&secp256k1_ecdsa_sign_low_s_der(&old_sk, &old_msg).unwrap());
    let link_old = sign_asset_tx(
        &genesis,
        &alice,
        old_seq,
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: alice.address.clone(),
            payload: IdentitySetActionV1::Link {
                kind: IdentityLinkKind::OldChainAccount,
                target: old_target.clone(),
                proof_pubkey_hex: Some(old_pk_hex),
                proof_sig_hex: Some(old_sig_hex),
                metadata_hash: None,
            },
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &link_old, 103, compat);
    assert!(r.accepted, "{r:?}");
    assert_eq!(
        ledger
            .identity_account_by_old_chain
            .get(&old_target)
            .map(String::as_str),
        Some(alice.address.as_str())
    );

    let ctrl_seq = next_seq(&ledger, &alice.address);
    let ctrl_msg = identity_controller_link_message(
        &genesis.chain_id,
        &alice.address,
        &controller.address,
        ctrl_seq,
    );
    let ctrl_sig = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &controller.keypair.private_key,
            &ctrl_msg,
            IDENTITY_CONTROLLER_LINK_DOMAIN.as_bytes(),
        )
        .unwrap(),
    );
    let link_ctrl = sign_asset_tx(
        &genesis,
        &alice,
        ctrl_seq,
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: alice.address.clone(),
            payload: IdentitySetActionV1::Link {
                kind: IdentityLinkKind::AgentController,
                target: controller.address.clone(),
                proof_pubkey_hex: Some(controller.public_key_hex.clone()),
                proof_sig_hex: Some(ctrl_sig),
                metadata_hash: None,
            },
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &link_ctrl,
        104,
        compat,
    );
    assert!(r.accepted, "{r:?}");

    // 7. Compromised Nostr key rotation permanently tombstones nostr_pk1!
    let nostr_sk2 = [0x44_u8; 32];
    let nostr_pk2 = bytes_to_hex(&bip340_xonly_pubkey_from_secret(&nostr_sk2).unwrap());
    let rot_tx_seq = next_seq(&ledger, &alice.address);
    let rot_msg =
        identity_nostr_proof_message("rotate", &genesis.chain_id, &alice.address, rot_tx_seq);
    let rot_sig = bytes_to_hex(&bip340_sign_deterministic(&nostr_sk2, &rot_msg).unwrap());
    let rot_tx = sign_asset_tx(
        &genesis,
        &alice,
        rot_tx_seq,
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: alice.address.clone(),
            payload: IdentitySetActionV1::Rotate {
                mode: IdentityRotateMode::Compromised,
                new_nostr_pubkey: nostr_pk2.clone(),
                new_nostr_proof_sig_hex: rot_sig,
                derivation_version: 1,
                key_index: 1,
            },
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &rot_tx, 105, compat);
    assert!(r.accepted, "{r:?}");
    assert!(ledger.compromised_nostr_pubkeys.contains(&nostr_pk1));
    assert!(!ledger.identity_account_by_nostr.contains_key(&nostr_pk1));
    assert_eq!(
        ledger
            .identity_account_by_nostr
            .get(&nostr_pk2)
            .map(String::as_str),
        Some(alice.address.as_str())
    );

    // 8. Safe Classical Proof Kill-Switch: blocks Mint/Rotate/OldChainAccount while ML-DSA Update/RotateAccount STILL WORK!
    let kill_compat = compat.with_hive_identity_classical_proof_kill_switch(Some(110));
    let bob_mint_blocked = sign_asset_tx(
        &genesis,
        &bob,
        next_seq(&ledger, &bob.address),
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: bob.address.clone(),
            payload: IdentitySetActionV1::Mint {
                nostr_pubkey: bob_nostr_pk.clone(),
                nostr_proof_sig_hex: bob_sig,
                derivation_version: 1,
                key_index: 0,
                identity_kind: IdentityKind::Human,
                handle_attestation: None,
                reservation_salt_hex: None,
                relays: vec![],
                avatar_nft_id: None,
                message_key: None,
                pq_receive_address: None,
            },
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &bob_mint_blocked,
        110,
        kill_compat,
    );
    assert!(!r.accepted);
    assert_eq!(r.code, "identity_classical_proof_disabled");

    // ML-DSA Update succeeds under kill-switch!
    let upd_tx = sign_asset_tx(
        &genesis,
        &alice,
        next_seq(&ledger, &alice.address),
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: alice.address.clone(),
            payload: IdentitySetActionV1::Update {
                relays: Some(vec!["wss://relay2.postfiat.org".to_string()]),
                avatar_nft_id: None,
                message_key: None,
                pq_receive_address: None,
                handle_attestation: None,
                reservation_salt_hex: None,
            },
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &upd_tx,
        111,
        kill_compat,
    );
    assert!(r.accepted, "{r:?}");

    // ML-DSA RotateAccount succeeds under kill-switch and marks links PendingReconfirmation!
    let ra_id_seq = ledger.identities_by_account[&alice.address].sequence;
    let ra_msg = identity_rotate_account_message(
        &genesis.chain_id,
        &alice.address,
        &alice_new.address,
        ra_id_seq,
    );
    let ra_sig = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &alice_new.keypair.private_key,
            &ra_msg,
            IDENTITY_ROTATE_ACCOUNT_DOMAIN.as_bytes(),
        )
        .unwrap(),
    );
    let ra_tx = sign_asset_tx(
        &genesis,
        &alice,
        next_seq(&ledger, &alice.address),
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: alice.address.clone(),
            payload: IdentitySetActionV1::RotateAccount {
                new_account: alice_new.address.clone(),
                new_account_pubkey_hex: alice_new.public_key_hex.clone(),
                new_account_sig_hex: ra_sig,
            },
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &ra_tx,
        112,
        kill_compat,
    );
    assert!(r.accepted, "{r:?}");
    let moved = ledger
        .identities_by_account
        .get(&alice_new.address)
        .unwrap();
    let ctrl_link = moved
        .links
        .iter()
        .find(|l| l.kind == IdentityLinkKind::AgentController)
        .expect("agent controller link");
    assert_eq!(ctrl_link.status, IdentityLinkStatus::PendingReconfirmation);
    assert_eq!(
        ledger
            .identity_account_by_old_chain
            .get(&old_target)
            .map(String::as_str),
        Some(alice_new.address.as_str())
    );

    // 9. 2-of-3 Cold Registrar Handle Reassignment: emergency_compromise cannot be canceled by owner, executes after timelock!
    let bob_mint_ok = sign_asset_tx(
        &genesis,
        &bob,
        next_seq(&ledger, &bob.address),
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: bob.address.clone(),
            payload: IdentitySetActionV1::Mint {
                nostr_pubkey: bob_nostr_pk,
                nostr_proof_sig_hex: bytes_to_hex(
                    &bip340_sign_deterministic(&bob_nostr_sk, &bob_msg).unwrap(),
                ),
                derivation_version: 1,
                key_index: 0,
                identity_kind: IdentityKind::Human,
                handle_attestation: None,
                reservation_salt_hex: None,
                relays: vec![],
                avatar_nft_id: None,
                message_key: None,
                pq_receive_address: None,
            },
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &bob_mint_ok,
        115,
        compat,
    );
    assert!(r.accepted, "{r:?}");

    // First test non-emergency reassignment: enforces 7-day timelock AND owner can cancel!
    let std_msg = identity_handle_reassign_message(
        &genesis.chain_id,
        "tasknode.postfiat.org",
        "goodalexander",
        &alice_new.address,
        &bob.address,
        "lost_key",
        false,
    );
    let std_c1 = RegistrarCosignatureV1 {
        signer_account: reg_cold_1.address.clone(),
        public_key_hex: reg_cold_1.public_key_hex.clone(),
        signature_hex: bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &reg_cold_1.keypair.private_key,
                &std_msg,
                IDENTITY_HANDLE_REASSIGN_DOMAIN.as_bytes(),
            )
            .unwrap(),
        ),
    };
    let std_c2 = RegistrarCosignatureV1 {
        signer_account: reg_cold_2.address.clone(),
        public_key_hex: reg_cold_2.public_key_hex.clone(),
        signature_hex: bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &reg_cold_2.keypair.private_key,
                &std_msg,
                IDENTITY_HANDLE_REASSIGN_DOMAIN.as_bytes(),
            )
            .unwrap(),
        ),
    };
    let std_prop_seq = next_seq(&ledger, &reg_cold_1.address);
    let std_prop_tx = sign_asset_tx(
        &genesis,
        &reg_cold_1,
        std_prop_seq,
        50_000,
        IDENTITY_HANDLE_REASSIGN_PROPOSE_TRANSACTION_KIND,
        AssetTransactionOperation::IdentityHandleReassignPropose(
            IdentityHandleReassignProposeOperation {
                proposer: reg_cold_1.address.clone(),
                namespace: "tasknode.postfiat.org".to_string(),
                handle: "goodalexander".to_string(),
                from_account: alice_new.address.clone(),
                to_account: bob.address.clone(),
                reason_code: "lost_key".to_string(),
                emergency_compromise: false,
                approvals: vec![std_c1, std_c2],
            },
        ),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &std_prop_tx,
            118,
            compat
        )
        .accepted
    );
    let std_rid = identity_reassignment_id(
        "tasknode.postfiat.org",
        "goodalexander",
        &alice_new.address,
        &bob.address,
        118,
        std_prop_seq,
    );
    let exec_early = sign_asset_tx(
        &genesis,
        &reg_cold_1,
        next_seq(&ledger, &reg_cold_1.address),
        50_000,
        IDENTITY_HANDLE_REASSIGN_EXECUTE_TRANSACTION_KIND,
        AssetTransactionOperation::IdentityHandleReassignExecute(
            IdentityHandleReassignExecuteOperation {
                executor: reg_cold_1.address.clone(),
                reassignment_id: std_rid.clone(),
            },
        ),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &exec_early,
        118 + IDENTITY_HANDLE_HOLDBACK_BLOCKS - 1,
        compat,
    );
    assert!(!r.accepted);
    // Owner can cancel non-emergency reassignment!
    let std_cancel = sign_asset_tx(
        &genesis,
        &alice_new,
        next_seq(&ledger, &alice_new.address),
        50_000,
        IDENTITY_HANDLE_REASSIGN_CANCEL_TRANSACTION_KIND,
        AssetTransactionOperation::IdentityHandleReassignCancel(
            IdentityHandleReassignCancelOperation {
                caller: alice_new.address.clone(),
                reassignment_id: std_rid,
                approvals: vec![],
            },
        ),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &std_cancel,
            119,
            compat
        )
        .accepted
    );

    // Now emergency_compromise reassignment: owner CANNOT cancel, executes cleanly!
    let reassign_msg = identity_handle_reassign_message(
        &genesis.chain_id,
        "tasknode.postfiat.org",
        "goodalexander",
        &alice_new.address,
        &bob.address,
        "emergency_compromise",
        true,
    );
    let cosig1 = RegistrarCosignatureV1 {
        signer_account: reg_cold_1.address.clone(),
        public_key_hex: reg_cold_1.public_key_hex.clone(),
        signature_hex: bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &reg_cold_1.keypair.private_key,
                &reassign_msg,
                IDENTITY_HANDLE_REASSIGN_DOMAIN.as_bytes(),
            )
            .unwrap(),
        ),
    };
    let cosig2 = RegistrarCosignatureV1 {
        signer_account: reg_cold_2.address.clone(),
        public_key_hex: reg_cold_2.public_key_hex.clone(),
        signature_hex: bytes_to_hex(
            &ml_dsa_65_sign_with_context(
                &reg_cold_2.keypair.private_key,
                &reassign_msg,
                IDENTITY_HANDLE_REASSIGN_DOMAIN.as_bytes(),
            )
            .unwrap(),
        ),
    };
    let prop_seq = next_seq(&ledger, &reg_cold_1.address);
    let prop_tx = sign_asset_tx(
        &genesis,
        &reg_cold_1,
        prop_seq,
        50_000,
        IDENTITY_HANDLE_REASSIGN_PROPOSE_TRANSACTION_KIND,
        AssetTransactionOperation::IdentityHandleReassignPropose(
            IdentityHandleReassignProposeOperation {
                proposer: reg_cold_1.address.clone(),
                namespace: "tasknode.postfiat.org".to_string(),
                handle: "goodalexander".to_string(),
                from_account: alice_new.address.clone(),
                to_account: bob.address.clone(),
                reason_code: "emergency_compromise".to_string(),
                emergency_compromise: true,
                approvals: vec![cosig1, cosig2],
            },
        ),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &prop_tx, 120, compat);
    assert!(r.accepted, "{r:?}");
    let rid = identity_reassignment_id(
        "tasknode.postfiat.org",
        "goodalexander",
        &alice_new.address,
        &bob.address,
        120,
        prop_seq,
    );

    // Owner cannot cancel emergency_compromise!
    let owner_cancel = sign_asset_tx(
        &genesis,
        &alice_new,
        next_seq(&ledger, &alice_new.address),
        50_000,
        IDENTITY_HANDLE_REASSIGN_CANCEL_TRANSACTION_KIND,
        AssetTransactionOperation::IdentityHandleReassignCancel(
            IdentityHandleReassignCancelOperation {
                caller: alice_new.address.clone(),
                reassignment_id: rid.clone(),
                approvals: vec![],
            },
        ),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &owner_cancel,
        125,
        compat,
    );
    assert!(!r.accepted);

    // Execute emergency compromise reassignment and transfer handle to Bob!
    let exec_ok = sign_asset_tx(
        &genesis,
        &reg_cold_1,
        next_seq(&ledger, &reg_cold_1.address),
        50_000,
        IDENTITY_HANDLE_REASSIGN_EXECUTE_TRANSACTION_KIND,
        AssetTransactionOperation::IdentityHandleReassignExecute(
            IdentityHandleReassignExecuteOperation {
                executor: reg_cold_1.address.clone(),
                reassignment_id: rid,
            },
        ),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &exec_ok, 126, compat);
    assert!(r.accepted, "{r:?}");
    assert_eq!(
        ledger
            .identity_account_by_handle
            .get(&hkey)
            .map(String::as_str),
        Some(bob.address.as_str())
    );

    // 10. Bob revokes identity -> 7d handle holdback active!
    let revoke_height = 130;
    let revoke_tx = sign_asset_tx(
        &genesis,
        &bob,
        next_seq(&ledger, &bob.address),
        50_000,
        IDENTITY_SET_TRANSACTION_KIND,
        AssetTransactionOperation::IdentitySet(IdentitySetOperation {
            account: bob.address.clone(),
            payload: IdentitySetActionV1::Revoke {
                reason_code: "retiring".to_string(),
            },
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &revoke_tx,
        revoke_height,
        compat,
    );
    assert!(r.accepted, "{r:?}");
    assert_eq!(
        ledger.identities_by_account[&bob.address].status,
        IdentityStatus::Revoked
    );
    assert_eq!(
        ledger.retired_handle_holdback.get(&hkey).copied(),
        Some(revoke_height + IDENTITY_HANDLE_HOLDBACK_BLOCKS)
    );
}

#[test]
fn pillar7_nav_fee_schedule_universal_floor_atomic_accrual_and_claim() {
    let genesis = test_genesis();
    let issuer = test_key(20);
    let holder = test_key(21);
    let fee_recipient = test_key(22);
    let attestor = test_key(23);

    let mut ledger = LedgerState::new(vec![
        funded_account(&issuer, 5_000_000),
        funded_account(&holder, 5_000_000),
        funded_account(&fee_recipient, 5_000_000),
        funded_account(&attestor, 5_000_000),
    ]);
    let compat = train_b_compat(100);

    // Create gated NAV asset (precision 6 => U = 10^6).
    let asset_id = issued_asset_id(&genesis.chain_id, &issuer.address, "NAVF", 1).unwrap();
    let create_tx = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        ASSET_CREATE_TRANSACTION_KIND,
        AssetTransactionOperation::AssetCreate(AssetCreateOperation {
            issuer: issuer.address.clone(),
            code: "NAVF".to_string(),
            version: 1,
            precision: 6,
            display_name: "NAV Fee Coin".to_string(),
            max_supply: None,
            requires_authorization: true,
            freeze_enabled: true,
            clawback_enabled: true,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &create_tx,
            100,
            compat
        )
        .accepted
    );

    // Register multi-fetch proof profile + independent attestor + NAV asset.
    let prof_op = NavProfileRegisterOperation {
        registrant: issuer.address.clone(),
        verifier_kind: NAV_PROFILE_VERIFIER_MULTI_FETCH.to_string(),
        source_class: String::new(),
        max_snapshot_age_blocks: 100,
        challenge_window_blocks: 1,
        max_epoch_gap_blocks: 1_000_000,
        settle_deadline_blocks: 100,
        min_challenge_bond: 10,
        min_attestations: 1,
        tolerance_bp: 10,
        bridge_observer_min_confirmations: 0,
        valuation_policy_hash: "cd".repeat(48),
        vault_bridge_route_policy_hash: String::new(),
        sp1_program_vkey: String::new(),
        sp1_proof_encoding: String::new(),
        max_proof_bytes: 0,
        max_public_values_bytes: 0,
        public_values_schema: String::new(),
        source_manifest_hash: String::new(),
        valuation_unit_id: String::new(),
        max_observation_span_blocks: 0,
        allow_controlled_sources: false,
    };
    let profile_id = prof_op.to_profile().unwrap().profile_id;
    let prof_tx = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_PROFILE_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::NavProfileRegister(prof_op),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &prof_tx, 100, compat);
    assert!(r.accepted, "{r:?}");

    let att_tx = sign_asset_tx(
        &genesis,
        &attestor,
        next_seq(&ledger, &attestor.address),
        50_000,
        NAV_ATTESTOR_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::NavAttestorRegister(NavAttestorRegisterOperation {
            attestor: attestor.address.clone(),
            domain: "attestor.postfiat.org".to_string(),
            bond: 100,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &att_tx, 100, compat)
            .accepted
    );

    let nav_reg = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_ASSET_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::NavAssetRegister(NavAssetRegisterOperation {
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            reserve_operator: issuer.address.clone(),
            proof_profile: profile_id.clone(),
            valuation_unit: "USD_E6".to_string(),
            redemption_account: issuer.address.clone(),
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &nav_reg, 100, compat)
            .accepted
    );

    // Authorize holder and prepare to mint 1_000_000_000_000 atoms.
    let holder_ts = sign_asset_tx(
        &genesis,
        &holder,
        next_seq(&ledger, &holder.address),
        50_000,
        TRUST_SET_TRANSACTION_KIND,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: holder.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            limit: 10_000_000_000_000,
            authorized: false,
            frozen: false,
            reserve_paid: 10,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &holder_ts,
            100,
            compat
        )
        .accepted
    );
    let auth_holder = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        TRUST_SET_TRANSACTION_KIND,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: holder.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            limit: 10_000_000_000_000,
            authorized: true,
            frozen: false,
            reserve_paid: 10,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &auth_holder,
            100,
            compat
        )
        .accepted
    );

    // Issue 1_000_000_000_000 units to authorized holder so live_ledger_supply = 1_000_000_000_000.
    let issue_initial = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.address.clone(),
            to: holder.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            amount: 1_000_000_000_000,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &issue_initial,
            100,
            compat
        )
        .accepted
    );

    // Bootstrap epoch 1 at height 101 with S = 1_000_000_000_000, V = 1_000_000_000_000 -> nav_per_unit = 1_000_000 (1.00 USD_E6).
    let pkt1_hash = "11".repeat(48);
    let sub1 = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer.address.clone(),
            submitter: issuer.address.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            nav_per_unit: 1_000_000,
            circulating_supply: 1_000_000_000_000,
            verified_net_assets: 1_000_000_000_000,
            proof_profile: profile_id.clone(),
            source_root: "01".repeat(48),
            attestor_root: "02".repeat(48),
            reserve_packet_hash: pkt1_hash.clone(),
            reserve_accounts: vec!["external:custodian-1".to_string()],
            sp1_proof_bytes: vec![],
            sp1_public_values: vec![],
        }),
    );
    let r = execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &sub1, 101, compat);
    assert!(r.accepted, "{r:?}");

    let att1 = sign_asset_tx(
        &genesis,
        &attestor,
        next_seq(&ledger, &attestor.address),
        50_000,
        NAV_RESERVE_ATTEST_TRANSACTION_KIND,
        AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
            attestor: attestor.address.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: pkt1_hash.clone(),
            pass: true,
            observation_root: "03".repeat(48),
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &att1, 101, compat)
            .accepted
    );
    let fin1 = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            epoch: 1,
            reserve_packet_hash: pkt1_hash,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &fin1, 102, compat)
            .accepted
    );

    // Attach optional NavFeeScheduleV1: reject annual_rate_bps > 500 cap, accept valid schedule!
    let bad_cap_schedule = NavFeeScheduleInputV1 {
        annual_rate_bps: 501,
        hwm_rate_bps: 1_000,
        issue_spread_bps: 50,
        redeem_spread_bps: 50,
        fix_fee_cap_bps: 25,
        recipient: fee_recipient.address.clone(),
        increase_notice_blocks: NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
        accrual_cap_blocks: 500_000,
        rates_locked: false,
        disclosure_ref: Some("ipfs://bafy-test".to_string()),
    };
    let bad_attach = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND,
        AssetTransactionOperation::NavFeeScheduleAttach(NavFeeScheduleAttachOperation {
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            schedule: bad_cap_schedule,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &bad_attach,
        103,
        compat,
    );
    assert!(!r.accepted);

    let good_schedule = NavFeeScheduleInputV1 {
        annual_rate_bps: 100,
        hwm_rate_bps: 1_000,
        issue_spread_bps: 50,
        redeem_spread_bps: 50,
        fix_fee_cap_bps: 25,
        recipient: fee_recipient.address.clone(),
        increase_notice_blocks: NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS,
        accrual_cap_blocks: 500_000,
        rates_locked: false,
        disclosure_ref: Some("ipfs://bafy-test".to_string()),
    };
    let attach_tx = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_FEE_SCHEDULE_ATTACH_TRANSACTION_KIND,
        AssetTransactionOperation::NavFeeScheduleAttach(NavFeeScheduleAttachOperation {
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            schedule: good_schedule.clone(),
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &attach_tx,
        103,
        compat,
    );
    assert!(r.accepted, "{r:?}");

    // Schedule hike -> goes to pending (effective_at = 104 + 604_800), then cancel!
    let mut hiked = good_schedule;
    hiked.annual_rate_bps = 200;
    let hike_tx = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_FEE_SCHEDULE_SET_TRANSACTION_KIND,
        AssetTransactionOperation::NavFeeScheduleSet(NavFeeScheduleSetOperation {
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            schedule: hiked,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &hike_tx, 104, compat);
    assert!(r.accepted, "{r:?}");
    assert_eq!(
        ledger
            .nav_asset(&asset_id)
            .unwrap()
            .fee_state
            .as_ref()
            .unwrap()
            .pending
            .as_ref()
            .unwrap()
            .effective_at_height,
        104 + NAV_FEE_MIN_INCREASE_NOTICE_BLOCKS
    );
    let cancel_tx = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_FEE_SCHEDULE_CANCEL_TRANSACTION_KIND,
        AssetTransactionOperation::NavFeeScheduleCancel(NavFeeScheduleCancelOperation {
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &cancel_tx,
            105,
            compat
        )
        .accepted
    );
    assert!(ledger
        .nav_asset(&asset_id)
        .unwrap()
        .fee_state
        .as_ref()
        .unwrap()
        .pending
        .is_none());

    // Universal floor check: V = 1_100_000_000_000, S = 1_000_000_000_000, U = 10^6 => exact floor = 1_100_000.
    // Understating nav_per_unit by 1 atom (1_099_999) MUST REJECT with nav_per_unit_floor_mismatch!
    let pkt2_bad = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer.address.clone(),
            submitter: issuer.address.clone(),
            asset_id: asset_id.clone(),
            epoch: 2,
            nav_per_unit: 1_099_999,
            circulating_supply: 1_000_000_000_000,
            verified_net_assets: 1_100_000_000_000,
            proof_profile: profile_id.clone(),
            source_root: "04".repeat(48),
            attestor_root: "05".repeat(48),
            reserve_packet_hash: "22".repeat(48),
            reserve_accounts: vec!["external:custodian-1".to_string()],
            sp1_proof_bytes: vec![],
            sp1_public_values: vec![],
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &pkt2_bad,
        315_463,
        compat,
    );
    assert!(!r.accepted);
    assert_eq!(r.code, "nav_per_unit_formula_mismatch");

    // Submit exact floor at height 315_463.
    let pkt2_hash = "23".repeat(48);
    let pkt2_ok = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_RESERVE_SUBMIT_TRANSACTION_KIND,
        AssetTransactionOperation::NavReserveSubmit(NavReserveSubmitOperation {
            issuer: issuer.address.clone(),
            submitter: issuer.address.clone(),
            asset_id: asset_id.clone(),
            epoch: 2,
            nav_per_unit: 1_100_000,
            circulating_supply: 1_000_000_000_000,
            verified_net_assets: 1_100_000_000_000,
            proof_profile: profile_id,
            source_root: "06".repeat(48),
            attestor_root: "07".repeat(48),
            reserve_packet_hash: pkt2_hash.clone(),
            reserve_accounts: vec!["external:custodian-1".to_string()],
            sp1_proof_bytes: vec![],
            sp1_public_values: vec![],
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &pkt2_ok,
        315_463,
        compat,
    );
    assert!(r.accepted, "{r:?}");
    let att2 = sign_asset_tx(
        &genesis,
        &attestor,
        next_seq(&ledger, &attestor.address),
        50_000,
        NAV_RESERVE_ATTEST_TRANSACTION_KIND,
        AssetTransactionOperation::NavReserveAttest(NavReserveAttestOperation {
            attestor: attestor.address.clone(),
            asset_id: asset_id.clone(),
            epoch: 2,
            reserve_packet_hash: pkt2_hash.clone(),
            pass: true,
            observation_root: "08".repeat(48),
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &att2, 315_463, compat)
            .accepted
    );

    // Finalize epoch 2 at height 315_464: atomic u256 accrual runs (bypassing asset_gate even though fee_recipient is NOT authorized)!
    let fin2 = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        NAV_EPOCH_FINALIZE_TRANSACTION_KIND,
        AssetTransactionOperation::NavEpochFinalize(NavEpochFinalizeOperation {
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            epoch: 2,
            reserve_packet_hash: pkt2_hash,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &fin2, 315_464, compat);
    assert!(r.accepted, "{r:?}");

    let nav_after = ledger.nav_asset(&asset_id).unwrap();
    let fee_state = nav_after.fee_state.as_ref().unwrap();
    assert!(fee_state.unclaimed_fee_units > 0);
    let minted_units = fee_state.unclaimed_fee_units;
    assert_eq!(
        nav_after.circulating_supply,
        1_000_000_000_000 + minted_units
    );
    assert_eq!(
        issued_asset_supply(&ledger, &asset_id).unwrap(),
        nav_after.circulating_supply
    );
    let accrual = ledger
        .nav_fee_accruals
        .get(&nav_fee_accrual_key(&asset_id, 2))
        .expect("accrual row recorded");
    assert_eq!(accrual.minted_fee_units, minted_units);
    assert_eq!(accrual.nav_after, nav_after.nav_per_unit);

    // nav_fee_claim MUST enforce asset_gate: fails while fee_recipient is unauthorized!
    let claim_unauth = sign_asset_tx(
        &genesis,
        &fee_recipient,
        next_seq(&ledger, &fee_recipient.address),
        50_000,
        NAV_FEE_CLAIM_TRANSACTION_KIND,
        AssetTransactionOperation::NavFeeClaim(NavFeeClaimOperation {
            recipient: fee_recipient.address.clone(),
            asset_id: asset_id.clone(),
            amount: minted_units,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &claim_unauth,
        315_465,
        compat,
    );
    assert!(!r.accepted);

    // Once issuer authorizes fee_recipient, nav_fee_claim succeeds and preserves total issued_asset_supply!
    let rec_ts = sign_asset_tx(
        &genesis,
        &fee_recipient,
        next_seq(&ledger, &fee_recipient.address),
        50_000,
        TRUST_SET_TRANSACTION_KIND,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: fee_recipient.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            limit: minted_units,
            authorized: false,
            frozen: false,
            reserve_paid: 10,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &rec_ts,
            315_465,
            compat
        )
        .accepted
    );
    let auth_rec = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        TRUST_SET_TRANSACTION_KIND,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: fee_recipient.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: asset_id.clone(),
            limit: minted_units,
            authorized: true,
            frozen: false,
            reserve_paid: 10,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &auth_rec,
            315_465,
            compat
        )
        .accepted
    );
    let claim_ok = sign_asset_tx(
        &genesis,
        &fee_recipient,
        next_seq(&ledger, &fee_recipient.address),
        50_000,
        NAV_FEE_CLAIM_TRANSACTION_KIND,
        AssetTransactionOperation::NavFeeClaim(NavFeeClaimOperation {
            recipient: fee_recipient.address.clone(),
            asset_id: asset_id.clone(),
            amount: minted_units,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &claim_ok,
        315_466,
        compat,
    );
    assert!(r.accepted, "{r:?}");
    assert_eq!(
        ledger
            .nav_asset(&asset_id)
            .unwrap()
            .fee_state
            .as_ref()
            .unwrap()
            .unclaimed_fee_units,
        0
    );
    assert_eq!(
        ledger
            .trustline_for_account_asset(&fee_recipient.address, &asset_id)
            .unwrap()
            .balance,
        minted_units
    );
    assert_eq!(
        issued_asset_supply(&ledger, &asset_id).unwrap(),
        1_000_000_000_000 + minted_units
    );
}

#[test]
fn pillar8_issuer_delegated_credentials_asset_access_and_permissionless_dex_escrow() {
    let genesis = test_genesis();
    let g_hash = genesis_hash(&genesis);
    let issuer = test_key(30);
    let alice = test_key(31);
    let bob = test_key(32); // NEVER calls trust_set on default asset!
    let zero_pft_user = test_key(33); // 0 PFT balance for delegated credential_create_and_accept!
    let kyc_admin = test_key(34);
    let kyc_admin_2 = test_key(35);
    let kyc_signer_1 = test_key(36);
    let kyc_signer_2 = test_key(37);

    let mut ledger = LedgerState::new(vec![
        funded_account(&issuer, 5_000_000),
        funded_account(&alice, 5_000_000),
        funded_account(&bob, 5_000_000),
        funded_account(&kyc_admin, 5_000_000),
        funded_account(&kyc_admin_2, 5_000_000),
        funded_account(&kyc_signer_1, 5_000_000),
        funded_account(&kyc_signer_2, 5_000_000),
    ]);
    let compat = train_b_compat(100);

    // 1. Create an UNGATED (default) asset and a GATED asset.
    let open_id = issued_asset_id(&genesis.chain_id, &issuer.address, "OPEN", 1).unwrap();
    let gated_id = issued_asset_id(&genesis.chain_id, &issuer.address, "GATE", 1).unwrap();
    for (code, req_auth) in [("OPEN", false), ("GATE", true)] {
        let tx = sign_asset_tx(
            &genesis,
            &issuer,
            next_seq(&ledger, &issuer.address),
            50_000,
            ASSET_CREATE_TRANSACTION_KIND,
            AssetTransactionOperation::AssetCreate(AssetCreateOperation {
                issuer: issuer.address.clone(),
                code: code.to_string(),
                version: 1,
                precision: 6,
                display_name: code.to_string(),
                max_supply: None,
                requires_authorization: req_auth,
                freeze_enabled: true,
                clawback_enabled: true,
            }),
        );
        assert!(
            execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &tx, 100, compat)
                .accepted
        );
    }

    // Issue 1_000 OPEN to Alice (implicit line created automatically without TrustSet!).
    let pay_alice = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.address.clone(),
            to: alice.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: open_id.clone(),
            amount: 1_000,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &pay_alice,
            101,
            compat
        )
        .accepted
    );

    // 2. Permissionless-by-default DEX OfferCreate + EscrowCreate/EscrowFinish for Bob (NO TrustSet!).
    let bob_offer = sign_offer_tx(
        &genesis,
        &bob,
        next_seq(&ledger, &bob.address),
        50_000,
        OFFER_CREATE_TRANSACTION_KIND,
        OfferTransactionOperation::OfferCreate(OfferCreateOperation {
            owner: bob.address.clone(),
            taker_gets_asset_id: NATIVE_PFT_ESCROW_ASSET_ID.to_string(),
            taker_gets_amount: 100,
            taker_pays_asset_id: open_id.clone(),
            taker_pays_amount: 100,
            expiration_height: 0,
        }),
    );
    let r = execute_offer_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &bob_offer,
        102,
        compat,
    );
    assert!(r.accepted, "permissionless offer_create failed: {r:?}");
    let bob_line = ledger
        .trustline_for_account_asset(&bob.address, &open_id)
        .expect("implicit trustline created on buy offer");
    assert!(bob_line.implicit);
    assert!(!bob_line.authorized);

    let escrow_recipient = test_key(38);
    ledger
        .accounts
        .push(funded_account(&escrow_recipient, 1_000_000));
    let esc_seq = next_seq(&ledger, &alice.address);
    let esc_create = sign_escrow_tx(
        &genesis,
        &alice,
        esc_seq,
        50_000,
        ESCROW_CREATE_TRANSACTION_KIND,
        EscrowTransactionOperation::EscrowCreate(EscrowCreateOperation {
            owner: alice.address.clone(),
            recipient: escrow_recipient.address.clone(),
            asset_id: open_id.clone(),
            amount: 200,
            condition: String::new(),
            finish_after: 103,
            cancel_after: 200,
        }),
    );
    let r = execute_escrow_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &esc_create,
        102,
        compat,
    );
    assert!(r.accepted, "permissionless escrow_create failed: {r:?}");
    let eid = escrow_id(&genesis.chain_id, &alice.address, esc_seq).unwrap();
    let esc_finish = sign_escrow_tx(
        &genesis,
        &escrow_recipient,
        next_seq(&ledger, &escrow_recipient.address),
        50_000,
        ESCROW_FINISH_TRANSACTION_KIND,
        EscrowTransactionOperation::EscrowFinish(EscrowFinishOperation {
            escrow_id: eid,
            owner: alice.address.clone(),
            recipient: escrow_recipient.address.clone(),
            fulfillment: String::new(),
        }),
    );
    let r = execute_escrow_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &esc_finish,
        103,
        compat,
    );
    assert!(r.accepted, "permissionless escrow_finish failed: {r:?}");
    assert_eq!(
        ledger
            .trustline_for_account_asset(&escrow_recipient.address, &open_id)
            .unwrap()
            .balance,
        200
    );

    // 3. Binding holder_limit & ACCOUNT_FLAG_REFUSE_UNSOLICITED_ASSETS.
    // Bob already has 100 OPEN reserved by his open buy offer, so set binding holder_limit = 150
    // and verify a payment of 51 (100 + 51 > 150) fails with trustline_limit_exceeded!
    let cap_tx = sign_asset_tx(
        &genesis,
        &bob,
        next_seq(&ledger, &bob.address),
        50_000,
        TRUST_SET_TRANSACTION_KIND,
        AssetTransactionOperation::TrustSet(TrustSetOperation {
            account: bob.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: open_id.clone(),
            limit: 150,
            authorized: false,
            frozen: false,
            reserve_paid: 0,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &cap_tx, 104, compat);
    assert!(r.accepted, "holder_limit trust_set failed: {r:?}");
    let over_cap_pay = sign_asset_tx(
        &genesis,
        &alice,
        next_seq(&ledger, &alice.address),
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: alice.address.clone(),
            to: bob.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: open_id.clone(),
            amount: 51,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &over_cap_pay,
        104,
        compat,
    );
    assert!(!r.accepted);
    assert_eq!(r.code, "trustline_limit_exceeded");

    // Set ACCOUNT_FLAG_REFUSE_UNSOLICITED_ASSETS on kyc_admin_2 and reject unsolicited OPEN payment!
    let refuse_tx = sign_asset_tx(
        &genesis,
        &kyc_admin_2,
        next_seq(&ledger, &kyc_admin_2.address),
        50_000,
        ACCOUNT_SET_TRANSACTION_KIND,
        AssetTransactionOperation::AccountSet(AccountSetOperation {
            account: kyc_admin_2.address.clone(),
            set_flags: ACCOUNT_FLAG_REFUSE_UNSOLICITED_ASSETS,
            clear_flags: 0,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &refuse_tx,
            105,
            compat
        )
        .accepted
    );
    let spam_pay = sign_asset_tx(
        &genesis,
        &alice,
        next_seq(&ledger, &alice.address),
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: alice.address.clone(),
            to: kyc_admin_2.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: open_id.clone(),
            amount: 10,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &spam_pay, 105, compat);
    assert!(!r.accepted);
    assert_eq!(r.code, "recipient_refuses_unsolicited_assets");

    // 4. Register KYC Issuer + 2-step admin transfer + 0-PFT delegated credential_create_and_accept!
    let issuer_reg = sign_asset_tx(
        &genesis,
        &kyc_admin,
        next_seq(&ledger, &kyc_admin.address),
        50_000,
        ISSUER_REGISTER_TRANSACTION_KIND,
        AssetTransactionOperation::IssuerRegister(IssuerRegisterOperation {
            admin_account: kyc_admin.address.clone(),
            domain: "kyc.postfiat.org".to_string(),
            signing_accounts: vec![kyc_signer_1.address.clone(), kyc_signer_2.address.clone()],
            allowed_credential_types: vec!["kyc.postfiat.org/tier1".to_string()],
            vendor_dids: vec!["did:web:kyc.postfiat.org".to_string()],
            registration_nonce: 1,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &issuer_reg,
        106,
        compat,
    );
    assert!(r.accepted, "{r:?}");
    let kyc_issuer_id = credential_issuer_id(&genesis.chain_id, &kyc_admin.address, 1);

    // 2-step admin transfer propose + accept.
    let prop_admin = sign_asset_tx(
        &genesis,
        &kyc_admin,
        next_seq(&ledger, &kyc_admin.address),
        50_000,
        ISSUER_ADMIN_TRANSFER_TRANSACTION_KIND,
        AssetTransactionOperation::IssuerAdminTransfer(IssuerAdminTransferOperation {
            caller: kyc_admin.address.clone(),
            issuer_id: kyc_issuer_id.clone(),
            action: IssuerAdminTransferAction::Propose,
            new_admin_account: Some(kyc_admin_2.address.clone()),
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &prop_admin,
            107,
            compat
        )
        .accepted
    );
    let accept_admin = sign_asset_tx(
        &genesis,
        &kyc_admin_2,
        next_seq(&ledger, &kyc_admin_2.address),
        50_000,
        ISSUER_ADMIN_TRANSFER_TRANSACTION_KIND,
        AssetTransactionOperation::IssuerAdminTransfer(IssuerAdminTransferOperation {
            caller: kyc_admin_2.address.clone(),
            issuer_id: kyc_issuer_id.clone(),
            action: IssuerAdminTransferAction::Accept,
            new_admin_account: None,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &accept_admin,
            108,
            compat
        )
        .accepted
    );

    // 0-PFT delegated credential_create_and_accept for zero_pft_user (who has NO account on chain yet!).
    let cred_commitment = "ee".repeat(48);
    let preimage = credential_delegated_accept_preimage(
        &genesis.chain_id,
        &g_hash,
        &zero_pft_user.address,
        &kyc_issuer_id,
        "kyc.postfiat.org/tier1",
        &cred_commitment,
        1_000_000,
        42,
    );
    let subject_sig = bytes_to_hex(
        &ml_dsa_65_sign_with_context(
            &zero_pft_user.keypair.private_key,
            &preimage,
            CREDENTIAL_DELEGATED_ACCEPT_DOMAIN.as_bytes(),
        )
        .unwrap(),
    );
    let signer1_balance_before = ledger.account(&kyc_signer_1.address).unwrap().balance;
    let deleg_tx = sign_asset_tx(
        &genesis,
        &kyc_signer_1,
        next_seq(&ledger, &kyc_signer_1.address),
        50_000,
        CREDENTIAL_CREATE_AND_ACCEPT_TRANSACTION_KIND,
        AssetTransactionOperation::CredentialCreateAndAccept(CredentialCreateAndAcceptOperation {
            signing_account: kyc_signer_1.address.clone(),
            issuer_id: kyc_issuer_id.clone(),
            subject: zero_pft_user.address.clone(),
            credential_type: "kyc.postfiat.org/tier1".to_string(),
            commitment_hash: cred_commitment,
            expires_at_height: 1_000_000,
            subject_pubkey_hex: zero_pft_user.public_key_hex.clone(),
            subject_accept_signature_hex: subject_sig,
            subject_accept_nonce: 42,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &deleg_tx, 109, compat);
    assert!(r.accepted, "{r:?}");
    let cid = credential_id(
        &zero_pft_user.address,
        &kyc_issuer_id,
        "kyc.postfiat.org/tier1",
    );
    assert_eq!(ledger.credentials[&cid].status, "accepted");
    assert_eq!(
        ledger.credentials[&cid].deposit_paid,
        CREDENTIAL_DEPOSIT_ATOMS
    );
    assert_eq!(
        ledger.account(&kyc_signer_1.address).unwrap().balance,
        signer1_balance_before - 50_000 - CREDENTIAL_DEPOSIT_ATOMS
    );

    // 5. asset_access_set: rejects retroactive gating on OPEN, attaches CredentialPolicyV1 to GATE!
    let retro_tx = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        ASSET_ACCESS_SET_TRANSACTION_KIND,
        AssetTransactionOperation::AssetAccessSet(AssetAccessSetOperation {
            issuer: issuer.address.clone(),
            asset_id: open_id,
            disable_authorization_permanently: false,
            approval_mode: Some(AssetApprovalMode::Either),
            lapsed_exit_rule: None,
            wrapper_mode: None,
            credential_policy: None,
            notice_blocks: None,
            emergency_remove_issuer_id: None,
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &retro_tx, 110, compat);
    assert!(!r.accepted);
    assert_eq!(r.code, "asset_gating_retroactive_enable_forbidden");

    let policy = CredentialPolicyV1 {
        any_of: vec![CredentialPolicyRuleV1 {
            all_of: vec![CredentialRequirementPairV1 {
                issuer_id: kyc_issuer_id.clone(),
                credential_type: "kyc.postfiat.org/tier1".to_string(),
            }],
        }],
    };
    let gate_policy_tx = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        ASSET_ACCESS_SET_TRANSACTION_KIND,
        AssetTransactionOperation::AssetAccessSet(AssetAccessSetOperation {
            issuer: issuer.address.clone(),
            asset_id: gated_id.clone(),
            disable_authorization_permanently: false,
            approval_mode: Some(AssetApprovalMode::Either),
            lapsed_exit_rule: Some(AssetLapsedExitRule::ReturnOnly),
            wrapper_mode: Some(AssetWrapperMode::PftlOnly),
            credential_policy: Some(policy),
            notice_blocks: Some(ASSET_ACCESS_MIN_TIGHTEN_NOTICE_BLOCKS),
            emergency_remove_issuer_id: None,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &gate_policy_tx,
        110,
        compat,
    );
    assert!(r.accepted, "{r:?}");

    // Ensure zero_pft_user account exists at minimum reserve (they NEVER called trust_set or credential_accept!).
    ledger.ensure_account(&zero_pft_user.address).balance = 10;
    // Zero-PFT credentialed user can receive GATED asset WITHOUT legacy issuer TrustSet!
    let pay_gated = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.address.clone(),
            to: zero_pft_user.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: gated_id.clone(),
            amount: 500,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &pay_gated,
        111,
        compat,
    );
    assert!(
        r.accepted,
        "credentialed recipient should pass asset_gate: {r:?}"
    );

    // 6. Revoke credential -> ReturnOnly default exit blocks receive but ALLOWS return to issuer!
    let rev_cred = sign_asset_tx(
        &genesis,
        &kyc_signer_1,
        next_seq(&ledger, &kyc_signer_1.address),
        50_000,
        CREDENTIAL_REVOKE_TRANSACTION_KIND,
        AssetTransactionOperation::CredentialRevoke(CredentialRevokeOperation {
            caller: kyc_signer_1.address.clone(),
            credential_id: cid.clone(),
            reason_code: "kyc_expired".to_string(),
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &rev_cred, 112, compat)
            .accepted
    );
    let pay_revoked = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: issuer.address.clone(),
            to: zero_pft_user.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: gated_id.clone(),
            amount: 100,
        }),
    );
    assert!(
        !execute_asset_transaction_with_compatibility(
            &genesis,
            &mut ledger,
            &pay_revoked,
            113,
            compat
        )
        .accepted
    );

    // Fund zero_pft_user for gas and verify Return-only exit back to issuer SUCCEEDS!
    ledger.ensure_account(&zero_pft_user.address).balance = 100_000;
    let return_to_issuer = sign_asset_tx(
        &genesis,
        &zero_pft_user,
        next_seq(&ledger, &zero_pft_user.address),
        50_000,
        ISSUED_PAYMENT_TRANSACTION_KIND,
        AssetTransactionOperation::IssuedPayment(IssuedPaymentOperation {
            from: zero_pft_user.address.clone(),
            to: issuer.address.clone(),
            issuer: issuer.address.clone(),
            asset_id: gated_id.clone(),
            amount: 500,
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &return_to_issuer,
        114,
        compat,
    );
    assert!(
        r.accepted,
        "ReturnOnly exit back to issuer must succeed: {r:?}"
    );
    assert!(ledger
        .trustline_for_account_asset(&zero_pft_user.address, &gated_id)
        .is_none());

    // 7. Compromise cutoff + emergency_remove_issuer_id + refundable credential_delete!
    let comp_upd = sign_asset_tx(
        &genesis,
        &kyc_admin_2,
        next_seq(&ledger, &kyc_admin_2.address),
        50_000,
        ISSUER_UPDATE_TRANSACTION_KIND,
        AssetTransactionOperation::IssuerUpdate(IssuerUpdateOperation {
            admin_account: kyc_admin_2.address.clone(),
            issuer_id: kyc_issuer_id.clone(),
            domain: None,
            add_signing_accounts: vec![],
            revoke_signing_accounts: vec![],
            compromise_cutoffs: vec![(kyc_signer_1.address.clone(), 115)],
            allowed_credential_types: None,
            vendor_dids: None,
        }),
    );
    assert!(
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &comp_upd, 115, compat)
            .accepted
    );
    let emerg_remove = sign_asset_tx(
        &genesis,
        &issuer,
        next_seq(&ledger, &issuer.address),
        50_000,
        ASSET_ACCESS_SET_TRANSACTION_KIND,
        AssetTransactionOperation::AssetAccessSet(AssetAccessSetOperation {
            issuer: issuer.address.clone(),
            asset_id: gated_id.clone(),
            disable_authorization_permanently: false,
            approval_mode: None,
            lapsed_exit_rule: None,
            wrapper_mode: None,
            credential_policy: None,
            notice_blocks: None,
            emergency_remove_issuer_id: Some(kyc_issuer_id),
        }),
    );
    let r = execute_asset_transaction_with_compatibility(
        &genesis,
        &mut ledger,
        &emerg_remove,
        116,
        compat,
    );
    assert!(r.accepted, "{r:?}");
    assert!(ledger
        .asset_definition(&gated_id)
        .unwrap()
        .access_policy
        .as_ref()
        .unwrap()
        .credential_policy
        .any_of
        .is_empty());

    // credential_delete refunds CREDENTIAL_DEPOSIT_ATOMS to deposit_payer (kyc_signer_1)!
    let s1_bal_before_del = ledger.account(&kyc_signer_1.address).unwrap().balance;
    let del_tx = sign_asset_tx(
        &genesis,
        &kyc_signer_1,
        next_seq(&ledger, &kyc_signer_1.address),
        10_000,
        CREDENTIAL_DELETE_TRANSACTION_KIND,
        AssetTransactionOperation::CredentialDelete(CredentialDeleteOperation {
            caller: kyc_signer_1.address.clone(),
            credential_id: cid.clone(),
        }),
    );
    let r =
        execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &del_tx, 117, compat);
    assert!(r.accepted, "{r:?}");
    assert!(!ledger.credentials.contains_key(&cid));
    assert_eq!(
        ledger.account(&kyc_signer_1.address).unwrap().balance,
        s1_bal_before_del - 10_000 + CREDENTIAL_DEPOSIT_ATOMS
    );
}
