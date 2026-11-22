use super::*;

const RECIPIENT_FORMAT_NODE_ACTIVATION_HEIGHT: u32 = 40;

fn recipient_format_amendment(height: u32) -> GovernanceAmendment {
    GovernanceAmendment {
        amendment_id: format!("recipient-format-activation-{height}"),
        chain_id: "postfiat-local".to_string(),
        genesis_hash: "00".repeat(48),
        protocol_version: 1,
        instance_id: "recipient-format-instance".to_string(),
        proposal_id: "recipient-format-proposal".to_string(),
        certificate_id: "recipient-format-certificate".to_string(),
        proposer: "validator-0".to_string(),
        validators: vec!["validator-0".to_string()],
        quorum: 1,
        kind: GOVERNANCE_KIND_RECIPIENT_FORMAT_V2_ACTIVATION_HEIGHT.to_string(),
        value: height,
        activation_height: 0,
        veto_until_height: 0,
        paused: false,
        support: vec!["validator-0".to_string()],
        votes: Vec::new(),
        signed_authorizations: Vec::new(),
    }
}

fn recipient_format_valid_address(seed: u8) -> String {
    address_from_public_key(&ml_dsa_65_keygen_from_seed(&[seed; 32]).public_key)
}

fn recipient_format_wrong_checksum(address: &str) -> String {
    let checksummed = postfiat_crypto_provider::checksum_address(address).expect("checksum");
    let mut bytes = checksummed.into_bytes();
    let index = bytes
        .iter()
        .skip(2)
        .position(u8::is_ascii_alphabetic)
        .expect("fixture letter")
        + 2;
    bytes[index] ^= 0x20;
    String::from_utf8(bytes).expect("ascii")
}

fn recipient_format_signed_transfer(
    genesis: &Genesis,
    key_file: &DevKeyFile,
    to: &str,
    fee: u64,
    sequence: u64,
) -> SignedTransfer {
    let unsigned = UnsignedTransfer {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: postfiat_types::TRANSFER_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from: key_file.address.clone(),
        to: to.to_string(),
        amount: ACCOUNT_RESERVE * 3,
        fee,
        sequence,
    };
    let private_key = hex_to_bytes(&key_file.private_key_hex).expect("private key bytes");
    let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign");
    SignedTransfer {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: key_file.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    }
}

fn recipient_format_signed_payment_v2(
    genesis: &Genesis,
    key_file: &DevKeyFile,
    to: &str,
    fee: u64,
    sequence: u64,
) -> SignedPaymentV2 {
    let unsigned = UnsignedPaymentV2 {
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(genesis),
        protocol_version: genesis.protocol_version,
        address_namespace: postfiat_types::ADDRESS_NAMESPACE.to_string(),
        transaction_kind: PAYMENT_V2_TRANSACTION_KIND.to_string(),
        signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        from: key_file.address.clone(),
        to: to.to_string(),
        amount: ACCOUNT_RESERVE * 3,
        fee,
        sequence,
        memos: vec![PaymentMemo {
            memo_type: "74616700".to_string(),
            memo_format: String::new(),
            memo_data: "2a".to_string(),
        }],
    };
    let private_key = hex_to_bytes(&key_file.private_key_hex).expect("private key bytes");
    let signature = ml_dsa_65_sign(&private_key, &unsigned.signing_bytes()).expect("sign");
    SignedPaymentV2 {
        unsigned,
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        public_key_hex: key_file.public_key_hex.clone(),
        signature_hex: bytes_to_hex(&signature),
    }
}

#[test]
fn mempool_admission_rejects_malformed_recipients_without_governance_activation() {
    let data_dir = unique_test_dir("postfiat-recipient-format-admission");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-local".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    assert_eq!(
        store
            .read_governance()
            .expect("governance")
            .recipient_format_v2_activation_height(),
        None,
        "admission policy must not depend on the consensus activation"
    );
    let key_file = read_transfer_key_file(&data_dir, None).expect("faucet key");
    let valid_recipient = recipient_format_valid_address(31);
    let quote = transfer_fee_quote(TransferFeeQuoteOptions {
        data_dir: data_dir.clone(),
        from: key_file.address.clone(),
        to: valid_recipient.clone(),
        amount: ACCOUNT_RESERVE * 3,
        sequence: None,
        memo_type: None,
        memo_format: None,
        memo_data: None,
    })
    .expect("fee quote");
    // Pad the fee so the shorter malformed fixtures never fail on fee first.
    let fee = quote.minimum_fee + 4;

    let cases = [
        ("pfrecipient".to_string(), "invalid_recipient_address"),
        (
            "pfpaymentv2recipient0000000000000001".to_string(),
            "invalid_recipient_address",
        ),
        (format!("PF{}", &valid_recipient[2..]), "invalid_recipient_address"),
        (
            recipient_format_wrong_checksum(&valid_recipient),
            "invalid_recipient_checksum",
        ),
        (
            postfiat_crypto_provider::checksum_address(&valid_recipient).expect("checksum"),
            "noncanonical_recipient_address",
        ),
    ];
    for (to, expected_code) in &cases {
        let transfer =
            recipient_format_signed_transfer(&genesis, &key_file, to, fee, quote.sequence);
        let error = submit_signed_transfer_json_to_mempool(SignedTransferJsonSubmitOptions {
            data_dir: data_dir.clone(),
            signed_transfer_json: serde_json::to_string(&transfer).expect("transfer json"),
        })
        .expect_err("malformed transfer recipient admitted");
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput, "{to}");
        let message = error.to_string();
        assert!(message.starts_with(&format!("{expected_code}: transfer.to")), "{message}");
        assert_eq!(recipient_address_error_code(&message), Some(*expected_code));

        let payment =
            recipient_format_signed_payment_v2(&genesis, &key_file, to, fee, quote.sequence);
        let error = submit_signed_payment_v2_json_to_mempool(SignedPaymentV2JsonSubmitOptions {
            data_dir: data_dir.clone(),
            signed_payment_v2_json: serde_json::to_string(&payment).expect("payment json"),
        })
        .expect_err("malformed payment_v2 recipient admitted");
        assert_eq!(
            recipient_address_error_code(&error.to_string()),
            Some(*expected_code),
            "{error}"
        );
    }
    let mempool = store.read_mempool().expect("mempool");
    assert!(mempool.pending.is_empty());
    assert!(mempool.pending_payment_v2.is_empty());

    let valid =
        recipient_format_signed_transfer(&genesis, &key_file, &valid_recipient, fee, quote.sequence);
    let admitted = submit_signed_transfer_json_to_mempool(SignedTransferJsonSubmitOptions {
        data_dir: data_dir.clone(),
        signed_transfer_json: serde_json::to_string(&valid).expect("transfer json"),
    })
    .expect("canonical recipient admitted");
    assert_eq!(admitted.tx_id, transfer_tx_id(&valid));
    let _ = fs::remove_dir_all(data_dir);
}

#[test]
fn recipient_format_activation_amendment_is_scheduled_once_and_never_retroactive() {
    let block_height = 100;
    for value in [0, 99, 100] {
        let (code, _) = governance_amendment_lifecycle_rejection(
            &recipient_format_amendment(value),
            block_height,
        )
        .expect("non-future activation accepted");
        assert_eq!(code, "invalid_recipient_format_v2_activation_height");
    }
    assert!(
        governance_amendment_lifecycle_rejection(&recipient_format_amendment(101), block_height)
            .is_none()
    );

    let mut governance = GovernanceState::new(1);
    assert_eq!(
        governance_amendment_current_value(
            &governance,
            GOVERNANCE_KIND_RECIPIENT_FORMAT_V2_ACTIVATION_HEIGHT
        ),
        0
    );
    governance.apply(recipient_format_amendment(500));
    governance.apply(recipient_format_amendment(900));
    assert_eq!(governance.recipient_format_v2_activation_height(), Some(500));
    assert_eq!(
        governance_amendment_current_value(
            &governance,
            GOVERNANCE_KIND_RECIPIENT_FORMAT_V2_ACTIVATION_HEIGHT
        ),
        500
    );
    let genesis = Genesis::new("postfiat-local");
    assert_eq!(
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance)
            .recipient_format_activation_height,
        Some(500)
    );
    assert_eq!(
        asset_execution_compatibility_for_genesis_and_governance(
            &genesis,
            &GovernanceState::new(1)
        )
        .recipient_format_activation_height,
        None
    );
}

/// Live block execution and archive replay agree before and after the
/// governed height, and a chain without the amendment replays exactly as
/// before this rule existed.
#[test]
fn recipient_format_rule_gates_live_execution_and_archive_replay_identically() {
    let genesis = Genesis::new("postfiat-local");
    let sender_key = ml_dsa_65_keygen_from_seed(&[32u8; 32]);
    let sender = address_from_public_key(&sender_key.public_key);
    let sender_key_file = DevKeyFile {
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        address: sender.clone(),
        public_key_hex: bytes_to_hex(&sender_key.public_key),
        private_key_hex: bytes_to_hex(&sender_key.private_key),
    };
    let ledger = LedgerState::new(vec![Account::new(
        sender.clone(),
        10_000,
        Some(bytes_to_hex(&sender_key.public_key)),
    )]);
    let malformed = "pfrecipient";
    let valid = recipient_format_valid_address(33);
    let fee = 64;
    let malformed_transfer =
        recipient_format_signed_transfer(&genesis, &sender_key_file, malformed, fee, 1);
    let malformed_payment =
        recipient_format_signed_payment_v2(&genesis, &sender_key_file, malformed, fee, 3);
    let valid_transfer = recipient_format_signed_transfer(&genesis, &sender_key_file, &valid, fee, 2);
    let mut batch = TransactionBatch::new(
        "recipient-format-batch",
        vec![malformed_transfer.clone(), valid_transfer.clone()],
    );
    batch.payments_v2.push(malformed_payment.clone());

    let legacy_governance = GovernanceState::new(1);
    let mut scheduled_governance = GovernanceState::new(1);
    scheduled_governance.apply(recipient_format_amendment(
        RECIPIENT_FORMAT_NODE_ACTIVATION_HEIGHT,
    ));
    let activation = u64::from(RECIPIENT_FORMAT_NODE_ACTIVATION_HEIGHT);

    let run_live = |governance: &GovernanceState, height: u64| {
        let compatibility =
            asset_execution_compatibility_for_genesis_and_governance(&genesis, governance);
        let mut live_ledger = ledger.clone();
        let receipts = execute_transparent_batch(
            &genesis,
            governance,
            &mut live_ledger,
            &batch,
            height,
            compatibility,
        );
        (receipts, live_ledger)
    };
    let run_replay = |governance: &GovernanceState, height: u64| {
        let mut block = dummy_block_record(height);
        block.receipt_ids = vec![
            transfer_tx_id(&malformed_transfer),
            transfer_tx_id(&valid_transfer),
            payment_v2_tx_id(&malformed_payment),
        ];
        block.header.receipt_count = 3;
        let mut replay_ledger = ledger.clone();
        let receipts = execute_transparent_batch_for_archive_replay(
            &genesis,
            &mut replay_ledger,
            &batch,
            &block,
            governance,
            &[],
        )
        .expect("archive replay");
        (receipts, replay_ledger)
    };

    // Without the amendment the rule never applies, at any height.
    let (legacy_receipts, legacy_ledger) = run_live(&legacy_governance, activation + 1_000);
    assert!(legacy_receipts.iter().all(|receipt| receipt.accepted), "{legacy_receipts:?}");
    assert!(legacy_ledger.account(malformed).is_some());
    assert_eq!(
        run_replay(&legacy_governance, activation + 1_000),
        (legacy_receipts.clone(), legacy_ledger.clone())
    );

    // Scheduled but not reached: byte-identical to legacy execution.
    let pre = run_live(&scheduled_governance, activation - 1);
    assert_eq!(pre, run_live(&legacy_governance, activation - 1));
    assert!(pre.0.iter().all(|receipt| receipt.accepted), "{:?}", pre.0);
    assert_eq!(run_replay(&scheduled_governance, activation - 1), pre);

    // From the activation height: malformed recipients are rejected without
    // charging the sender; the canonical transfer still applies.
    let (active_receipts, active_ledger) = run_live(&scheduled_governance, activation);
    assert_eq!(active_receipts.len(), 3);
    assert!(!active_receipts[0].accepted);
    assert_eq!(active_receipts[0].code, "invalid_recipient_address");
    assert_eq!(active_receipts[0].fee_charged, 0);
    assert!(!active_receipts[2].accepted);
    assert_eq!(active_receipts[2].code, "invalid_recipient_address");
    // The valid transfer was signed with sequence 2, so it now fails on
    // sequence: the rejected transfer did not consume sequence 1.
    assert_eq!(active_receipts[1].code, "bad_sequence", "{active_receipts:?}");
    assert!(active_ledger.account(malformed).is_none());
    assert_eq!(active_ledger, ledger, "rejections must not mutate the ledger");
    assert_eq!(
        run_replay(&scheduled_governance, activation),
        (active_receipts, active_ledger)
    );
}

// XR-01 (exchange-readiness review 2026-10-23): an Orchard deposit's funding
// transfer targets the 41-character `FEE_COLLECTOR_ADDRESS` sink. It must stay
// on plain `execute_transfer`, outside the governed recipient and fee-claim
// wrappers, or every Orchard deposit fails once the recipient rule is active.
#[test]
fn orchard_deposit_funding_transfer_stays_on_plain_execute_transfer() {
    assert!(!postfiat_crypto_provider::is_canonical_address(
        FEE_COLLECTOR_ADDRESS
    ));

    let source = include_str!("../orchard_state_application.rs");
    let source = source
        .split("#[cfg(test)]")
        .next()
        .expect("non-test source")
        .lines()
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        source
            .matches("execute_transfer(genesis, &mut trial_ledger, &payload.funding_transfer)")
            .count(),
        1,
        "the Orchard deposit funding transfer must execute through plain execute_transfer"
    );
    for governed in [
        "execute_transfer_with_compatibility",
        "claim_rejected_transfer_fee",
        "destination_address_rejection",
    ] {
        assert!(
            !source.contains(governed),
            "orchard_state_application.rs must not route the funding transfer through `{governed}`"
        );
    }

    // Why the pin matters: with the rule active, the governed wrapper refuses
    // the sink while plain execution still applies the funding transfer.
    let genesis = Genesis::new("postfiat-local");
    let sender_key = ml_dsa_65_keygen_from_seed(&[34u8; 32]);
    let sender = address_from_public_key(&sender_key.public_key);
    let sender_key_file = DevKeyFile {
        algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
        address: sender.clone(),
        public_key_hex: bytes_to_hex(&sender_key.public_key),
        private_key_hex: bytes_to_hex(&sender_key.private_key),
    };
    let ledger = LedgerState::new(vec![
        Account::new(sender, 10_000, Some(bytes_to_hex(&sender_key.public_key))),
        Account::new(FEE_COLLECTOR_ADDRESS.to_string(), ACCOUNT_RESERVE, None),
    ]);
    let funding =
        recipient_format_signed_transfer(&genesis, &sender_key_file, FEE_COLLECTOR_ADDRESS, 64, 1);
    let activation = u64::from(RECIPIENT_FORMAT_NODE_ACTIVATION_HEIGHT);
    let mut governance = GovernanceState::new(1);
    governance.apply(recipient_format_amendment(
        RECIPIENT_FORMAT_NODE_ACTIVATION_HEIGHT,
    ));
    let compatibility =
        asset_execution_compatibility_for_genesis_and_governance(&genesis, &governance);

    let mut plain_ledger = ledger.clone();
    let plain = execute_transfer(&genesis, &mut plain_ledger, &funding);
    assert!(plain.accepted, "{plain:?}");

    let mut governed_ledger = ledger.clone();
    let governed = execute_transfer_with_compatibility(
        &genesis,
        &mut governed_ledger,
        &funding,
        activation,
        compatibility,
    );
    assert!(!governed.accepted);
    assert_eq!(governed.code, "invalid_recipient_address");
    assert_eq!(governed_ledger, ledger);
}
