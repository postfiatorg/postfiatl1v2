use super::*;

fn signed_owned_order(
    owner: &postfiat_crypto_provider::MlDsa65KeyPair,
    owner_pubkey_hex: &str,
    order: postfiat_types::OwnedTransferOrder,
) -> postfiat_types::SignedOwnedTransferOrder {
    let signing_bytes = postfiat_execution::owned_transfer_signing_bytes(&order);
    let signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
        &owner.private_key,
        &signing_bytes,
        postfiat_execution::OWNED_TRANSFER_CONTEXT,
    )
    .expect("owner sign");
    postfiat_types::SignedOwnedTransferOrder {
        order,
        owner_pubkey_hex: owner_pubkey_hex.to_string(),
        owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&signature),
    }
}

fn owned_sign_fixture(
    label: &str,
) -> (
    PathBuf,
    postfiat_crypto_provider::MlDsa65KeyPair,
    String,
) {
    let data_dir = unique_test_dir(label);
    std::fs::create_dir_all(&data_dir).expect("create fixture");
    let owner = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner keygen");
    let owner_pubkey_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
    let validator = postfiat_crypto_provider::ml_dsa_65_keygen().expect("validator keygen");
    let validator_keys = serde_json::json!({
        "validators": [{
            "node_id": "validator-0",
            "public_key_hex": postfiat_crypto_provider::bytes_to_hex(&validator.public_key),
            "private_key_hex": postfiat_crypto_provider::bytes_to_hex(&validator.private_key),
        }]
    });
    atomic_write(
        data_dir.join("validator_keys.json"),
        format!("{}\n", serde_json::to_string_pretty(&validator_keys).expect("keys json")),
    )
    .expect("write validator keys");
    atomic_write(
        data_dir.join("validator_registry.json"),
        b"{\"validators\":[\"validator-0\"]}\n",
    )
    .expect("write registry");
    NodeStore::new(&data_dir)
        .write_genesis(&postfiat_types::Genesis::new_with_validator_count(
            "postfiat-fastpay-payment-safety",
            1,
        ))
        .expect("write genesis");

    let mut ledger = postfiat_types::LedgerState::empty();
    ledger.owned_objects.push(postfiat_types::OwnedObject {
        id: "object-0".to_string(),
        version: 7,
        owner_pubkey_hex: owner_pubkey_hex.clone(),
        value: 100,
        asset: "PFT".to_string(),
    });
    NodeStore::new(&data_dir)
        .write_ledger(&ledger)
        .expect("write ledger");
    (data_dir, owner, owner_pubkey_hex)
}

fn transfer_order(
    data_dir: &std::path::Path,
    recipient: &str,
    nonce: u64,
) -> postfiat_types::OwnedTransferOrder {
    postfiat_types::OwnedTransferOrder {
        domain: owned_certificate_domain(data_dir).expect("owned certificate domain"),
        inputs: vec![postfiat_types::OwnedObjectRef {
            id: "object-0".to_string(),
            version: 7,
        }],
        outputs: vec![postfiat_types::OwnedOutputSpec {
            owner_pubkey_hex: recipient.to_string(),
            value: 99,
            asset: "PFT".to_string(),
        }],
        fee: 1,
        nonce,
        memos: Vec::new(),
    }
}

fn commit_fastlane_primary_for_test(
    data_dir: &Path,
    label: &str,
    transaction: postfiat_types::FastLanePrimaryTransactionV1,
) -> Receipt {
    admit_fastlane_primary_to_mempool(data_dir, transaction)
        .unwrap_or_else(|error| panic!("admit {label}: {error}"));
    let batch_file = data_dir.join(format!("{label}.batch.json"));
    create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file: batch_file.clone(),
        max_transactions: 1,
    })
    .unwrap_or_else(|error| panic!("build {label} batch: {error}"));
    let mut receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.to_path_buf(),
        batch_file,
        certificate_file: None,
    })
    .unwrap_or_else(|error| panic!("apply {label}: {error}"));
    assert_eq!(receipts.len(), 1, "{label} receipt count");
    receipts.remove(0)
}

fn copy_fastpay_node_dir(source: &Path, destination: &Path) {
    std::fs::create_dir_all(destination).expect("create copied FastPay node");
    for entry in std::fs::read_dir(source).expect("read FastPay source node") {
        let entry = entry.expect("read FastPay source entry");
        let target = destination.join(entry.file_name());
        if entry.file_type().expect("FastPay source entry type").is_dir() {
            copy_fastpay_node_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("copy FastPay node file");
        }
    }
}

fn rewrite_fastpay_node_id(data_dir: &Path, node_id: &str) {
    let path = data_dir.join(NODE_STATE_FILE);
    let mut state: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&path).expect("read copied node state"))
            .expect("parse copied node state");
    state["node_id"] = serde_json::json!(node_id);
    atomic_write(
        path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&state).expect("copied node state JSON")
        ),
    )
    .expect("write copied node identity");
}

fn signed_owned_deposit_for_test(
    genesis: &postfiat_types::Genesis,
    source: &DevKeyFile,
    destination_owner_pubkey: Vec<u8>,
    sequence: u64,
    amount_atoms: u64,
    nonce: [u8; 32],
) -> postfiat_types::FastLanePrimaryTransactionV1 {
    let deposit = postfiat_types::OwnedDepositV1 {
        domain: postfiat_types::FastSwapChainDomainV1 {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: postfiat_types::FastSwapOpaqueHashV1(
                postfiat_crypto_provider::hex_to_bytes(&genesis_hash(genesis))
                    .expect("genesis hash bytes")
                    .try_into()
                    .expect("genesis hash width"),
            ),
            protocol_version: genesis.protocol_version,
        },
        source_address: source.address.clone(),
        source_pubkey: postfiat_crypto_provider::hex_to_bytes(&source.public_key_hex)
            .expect("source public key"),
        sequence,
        fee_pft: 1,
        destination_owner_pubkey,
        asset: "PFT".to_string(),
        amount_atoms,
        valid_through_height: 20,
        nonce,
    };
    let signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
        &postfiat_crypto_provider::hex_to_bytes(&source.private_key_hex)
            .expect("source private key"),
        &deposit.signing_bytes().expect("owned deposit signing bytes"),
        postfiat_types::OWNED_DEPOSIT_CONTEXT_V1,
    )
    .expect("sign owned deposit");
    postfiat_types::FastLanePrimaryTransactionV1 {
        operation: postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit {
            signed: postfiat_types::SignedOwnedDepositV1 {
                deposit,
                algorithm_id: postfiat_types::FASTSWAP_ML_DSA_65.to_string(),
                signature,
            },
        },
    }
}

#[test]
fn unsigned_wrap_owned_rejects_every_asset_without_mutation() {
    let data_dir = unique_test_dir("postfiat-wrap-owned-native-only");
    std::fs::create_dir_all(&data_dir).expect("create fixture");
    atomic_write(
        data_dir.join("validator_registry.json"),
        b"{\"validators\":[\"validator-0\"]}\n",
    )
    .expect("write registry");
    let mut ledger = postfiat_types::LedgerState::empty();
    ledger.accounts.push(postfiat_types::Account {
        address: "pf-alice".to_string(),
        balance: 100,
        sequence: 0,
        public_key_hex: None,
    });
    NodeStore::new(&data_dir).write_ledger(&ledger).expect("write ledger");
    let object_id = "ab".repeat(32);

    for asset in ["PFT", "pfUSDC", "a651", "pft"] {
        let error = wrap_owned(
            NodeOptions {
                data_dir: data_dir.clone(),
            },
            "pf-alice",
            "alicepk",
            50,
            asset,
            Some(&object_id),
        )
        .expect_err("unsigned wrap must fail closed");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
        assert!(error.to_string().contains("signed FastLane primary deposit"));
    }
    let untouched = NodeStore::new(&data_dir).read_ledger().expect("read ledger");
    assert_eq!(untouched.account("pf-alice").map(|a| a.balance), Some(100));
    assert!(untouched.owned_objects.is_empty());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn concurrent_unsigned_wrap_requests_all_reject_without_mutation() {
    let data_dir = unique_test_dir("postfiat-wrap-owned-concurrent-rejection");
    std::fs::create_dir_all(&data_dir).expect("create fixture");
    let mut ledger = postfiat_types::LedgerState::empty();
    ledger.accounts.push(postfiat_types::Account {
        address: "pf-alice".to_string(),
        balance: 1_000,
        sequence: 0,
        public_key_hex: None,
    });
    NodeStore::new(&data_dir)
        .write_ledger(&ledger)
        .expect("write ledger");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let workers = (0..8)
        .map(|worker| {
            let data_dir = data_dir.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                wrap_owned(
                    NodeOptions { data_dir },
                    "pf-alice",
                    "attacker-owner",
                    100,
                    "PFT",
                    Some(&format!("{:064x}", worker)),
                )
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        let error = worker
            .join()
            .expect("wrap worker")
            .expect_err("unsigned wrap must reject under concurrency");
        assert_eq!(error.kind(), io::ErrorKind::PermissionDenied);
    }
    assert_eq!(
        NodeStore::new(&data_dir).read_ledger().expect("ledger after"),
        ledger
    );

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn certified_owned_unwrap_cannot_convert_issued_asset_to_native_balance() {
    let data_dir = unique_test_dir("postfiat-owned-unwrap-asset-binding");
    std::fs::create_dir_all(&data_dir).expect("create fixture");
    let owner = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner keygen");
    let owner_pubkey_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
    let validator = postfiat_crypto_provider::ml_dsa_65_keygen().expect("validator keygen");
    let validator_pubkey_hex =
        postfiat_crypto_provider::bytes_to_hex(&validator.public_key);
    let registry = ValidatorRegistry {
        validators: vec![ValidatorRegistryRecord {
            node_id: "validator-0".to_string(),
            algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
            public_key_hex: validator_pubkey_hex.clone(),
        }],
    };
    write_validator_registry_file(&data_dir.join("validator_registry.json"), &registry)
        .expect("write registry");
    atomic_write(
        data_dir.join("validator_keys.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&serde_json::json!({
                "validators": [{
                    "node_id": "validator-0",
                    "algorithm_id": ML_DSA_65_ALGORITHM,
                    "public_key_hex": validator_pubkey_hex,
                    "private_key_hex": postfiat_crypto_provider::bytes_to_hex(
                        &validator.private_key,
                    ),
                }]
            }))
            .expect("key json")
        ),
    )
    .expect("write validator key");
    let store = NodeStore::new(&data_dir);
    store
        .write_genesis(&postfiat_types::Genesis::new_with_validator_count(
            "postfiat-owned-unwrap-asset-binding",
            1,
        ))
        .expect("write genesis");
    let mut ledger = postfiat_types::LedgerState::empty();
    ledger.owned_objects.push(postfiat_types::OwnedObject {
        id: "issued-object".to_string(),
        version: 1,
        owner_pubkey_hex: owner_pubkey_hex.clone(),
        value: 25,
        asset: "pfUSDC".to_string(),
    });
    store.write_ledger(&ledger).expect("write ledger");

    let order = postfiat_types::OwnedUnwrapOrder {
        domain: owned_certificate_domain(&data_dir).expect("certificate domain"),
        inputs: vec![postfiat_types::OwnedObjectRef {
            id: "issued-object".to_string(),
            version: 1,
        }],
        to_address: "pf-recipient".to_string(),
        amount: 25,
        asset: "pfUSDC".to_string(),
        fee: 0,
        nonce: 1,
        memos: Vec::new(),
    };
    let signing_bytes = postfiat_execution::owned_unwrap_signing_bytes(&order);
    let owner_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
        &owner.private_key,
        &signing_bytes,
        postfiat_execution::OWNED_UNWRAP_CONTEXT,
    )
    .expect("owner sign");
    let validator_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
        &validator.private_key,
        &signing_bytes,
        postfiat_execution::OWNED_UNWRAP_CONTEXT,
    )
    .expect("validator sign");
    let certificate = postfiat_types::OwnedUnwrapCertificate {
        order,
        owner_pubkey_hex,
        owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_signature),
        votes: vec![postfiat_types::OwnedUnwrapVote {
            validator_id: "validator-0".to_string(),
            signature_hex: postfiat_crypto_provider::bytes_to_hex(&validator_signature),
        }],
    };
    let before = store.read_ledger().expect("ledger before");
    let error = owned_unwrap_apply_report(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &serde_json::to_string(&certificate).expect("certificate json"),
    )
    .expect_err("issued owned value must not credit the native account lane");
    assert!(error.to_string().contains("UnsupportedAsset"));
    assert_eq!(store.read_ledger().expect("ledger after"), before);

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn signed_account_deposit_enters_fastpay_through_ordered_batch_and_replays() {
    let data_dir = unique_test_dir("postfiat-signed-owned-deposit-consensus-boundary");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-owned-deposit-boundary".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let source = read_transfer_key_file(&data_dir, None).expect("faucet key");
    let source_pubkey = postfiat_crypto_provider::hex_to_bytes(&source.public_key_hex)
        .expect("source public key");
    let source_private_key = postfiat_crypto_provider::hex_to_bytes(&source.private_key_hex)
        .expect("source private key");
    let genesis_hash_bytes: [u8; 48] = postfiat_crypto_provider::hex_to_bytes(
        &genesis_hash(&genesis),
    )
    .expect("genesis hash hex")
    .try_into()
    .expect("genesis hash length");
    let before = store.read_ledger().expect("ledger before");
    let before_balance = before
        .account(&source.address)
        .expect("source account")
        .balance;
    let deposit = postfiat_types::OwnedDepositV1 {
        domain: postfiat_types::FastSwapChainDomainV1 {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: postfiat_types::FastSwapOpaqueHashV1(genesis_hash_bytes),
            protocol_version: genesis.protocol_version,
        },
        source_address: source.address.clone(),
        source_pubkey: source_pubkey.clone(),
        sequence: 1,
        fee_pft: 2,
        destination_owner_pubkey: source_pubkey,
        asset: "PFT".to_string(),
        amount_atoms: 40,
        valid_through_height: 10,
        nonce: [71; 32],
    };
    let signed = postfiat_types::SignedOwnedDepositV1 {
        signature: postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &source_private_key,
            &deposit.signing_bytes().expect("deposit signing bytes"),
            postfiat_types::OWNED_DEPOSIT_CONTEXT_V1,
        )
        .expect("sign deposit"),
        algorithm_id: postfiat_types::FASTSWAP_ML_DSA_65.to_string(),
        deposit,
    };
    let transaction = postfiat_types::FastLanePrimaryTransactionV1 {
        operation: postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit { signed },
    };
    let entry = admit_fastlane_primary_to_mempool(&data_dir, transaction.clone())
        .expect("admit signed deposit");
    assert_eq!(mempool_state(NodeOptions { data_dir: data_dir.clone() }).expect("mempool").pending_fastlane_primary.len(), 1);

    let batch_file = data_dir.join("owned-deposit.batch.json");
    let batch = create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 1,
    })
    .expect("create ordered batch");
    assert_eq!(batch.fastlane_primary_transactions, vec![transaction]);
    let receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file,
        certificate_file: None,
    })
    .expect("apply ordered batch");
    assert_eq!(receipts.len(), 1);
    assert!(receipts[0].accepted);
    assert_eq!(receipts[0].tx_id, entry.tx_id);
    assert_eq!(receipts[0].code, "owned_deposit_applied");
    assert_eq!(receipts[0].fee_burned, 2);

    // Reproduce a non-proposer that admitted the same transaction before
    // learning the certified block from the actual proposer.
    store
        .append_mempool_fastlane_primary_entry(entry.clone())
        .expect("seed non-proposer stale entry");
    assert_eq!(
        mempool_state(NodeOptions {
            data_dir: data_dir.clone()
        })
        .expect("stale mempool")
        .pending_fastlane_primary
        .len(),
        1
    );
    assert_eq!(
        reconcile_terminal_mempool_entries(&data_dir).expect("reconcile terminal mempool"),
        1
    );
    assert!(mempool_state(NodeOptions {
        data_dir: data_dir.clone()
    })
    .expect("reconciled mempool")
    .pending_fastlane_primary
    .is_empty());

    let after = store.read_ledger().expect("ledger after");
    let account = after.account(&source.address).expect("source account after");
    assert_eq!(account.balance, before_balance - 42);
    assert_eq!(account.sequence, 1);
    assert_eq!(after.owned_objects.len(), 1);
    assert_eq!(after.owned_objects[0].value, 40);
    assert_eq!(account.balance + after.owned_objects[0].value + 2, before_balance);
    let replay = verify_blocks(NodeOptions { data_dir: data_dir.clone() })
        .expect("replay verification");
    assert!(replay.verified);
    assert_eq!(replay.block_count, 1);

    let after_success = after.clone();
    let duplicate = admit_fastlane_primary_to_mempool(&data_dir, batch.fastlane_primary_transactions[0].clone())
        .expect_err("sequence replay must reject at admission");
    assert_eq!(duplicate.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(store.read_ledger().expect("ledger after duplicate"), after_success);
    assert!(mempool_state(NodeOptions { data_dir: data_dir.clone() })
        .expect("mempool after duplicate")
        .pending_fastlane_primary
        .is_empty());

    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn signed_account_deposit_wrong_chain_domain_rejects_without_mempool_or_ledger_mutation() {
    let data_dir = unique_test_dir("postfiat-owned-deposit-domain-rejection");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-owned-deposit-domain".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("init");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("genesis");
    let source = read_transfer_key_file(&data_dir, None).expect("faucet key");
    let source_pubkey = postfiat_crypto_provider::hex_to_bytes(&source.public_key_hex)
        .expect("source public key");
    let source_private_key = postfiat_crypto_provider::hex_to_bytes(&source.private_key_hex)
        .expect("source private key");
    let deposit = postfiat_types::OwnedDepositV1 {
        domain: postfiat_types::FastSwapChainDomainV1 {
            chain_id: "wrong-chain".to_string(),
            genesis_hash: postfiat_types::FastSwapOpaqueHashV1([72; 48]),
            protocol_version: genesis.protocol_version,
        },
        source_address: source.address,
        source_pubkey: source_pubkey.clone(),
        sequence: 1,
        fee_pft: 1,
        destination_owner_pubkey: source_pubkey,
        asset: "PFT".to_string(),
        amount_atoms: 10,
        valid_through_height: 10,
        nonce: [73; 32],
    };
    let signed = postfiat_types::SignedOwnedDepositV1 {
        signature: postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &source_private_key,
            &deposit.signing_bytes().expect("deposit signing bytes"),
            postfiat_types::OWNED_DEPOSIT_CONTEXT_V1,
        )
        .expect("sign deposit"),
        algorithm_id: postfiat_types::FASTSWAP_ML_DSA_65.to_string(),
        deposit,
    };
    let before = store.read_ledger().expect("ledger before");
    let error = admit_fastlane_primary_to_mempool(
        &data_dir,
        postfiat_types::FastLanePrimaryTransactionV1 {
            operation: postfiat_types::FastLanePrimaryOperationV1::OwnedDeposit { signed },
        },
    )
    .expect_err("wrong domain must reject");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("owned_deposit_wrong_domain"));
    assert_eq!(store.read_ledger().expect("ledger after"), before);
    assert!(mempool_state(NodeOptions { data_dir: data_dir.clone() })
        .expect("mempool after")
        .pending_fastlane_primary
        .is_empty());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn owned_sign_rejects_unauthorized_and_invalid_admission_without_locking() {
    let (data_dir, owner, owner_pubkey_hex) =
        owned_sign_fixture("postfiat-owned-sign-admission-safety");
    let mut unauthorized = signed_owned_order(
        &owner,
        &owner_pubkey_hex,
        transfer_order(&data_dir, "recipient-a", 1),
    );
    unauthorized.owner_signature_hex = "00".to_string();
    let unauthorized_error = owned_sign(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &serde_json::to_string(&unauthorized).expect("signed json"),
        "validator-0",
    )
    .expect_err("unauthorized order must fail");
    assert_eq!(unauthorized_error.kind(), io::ErrorKind::InvalidInput);
    assert!(!data_dir.join("owned_locks.json").exists());

    let mut invalid_order = transfer_order(&data_dir, "recipient-b", 2);
    invalid_order.inputs[0].version = 8;
    let invalid = signed_owned_order(&owner, &owner_pubkey_hex, invalid_order);
    let invalid_error = owned_sign(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &serde_json::to_string(&invalid).expect("signed json"),
        "validator-0",
    )
    .expect_err("stale input must fail");
    assert_eq!(invalid_error.kind(), io::ErrorKind::InvalidInput);
    assert!(!data_dir.join("owned_locks.json").exists());
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn concurrent_conflicting_owned_sign_calls_emit_at_most_one_vote() {
    let (data_dir, owner, owner_pubkey_hex) =
        owned_sign_fixture("postfiat-owned-sign-concurrent-safety");
    let signed_a = serde_json::to_string(&signed_owned_order(
        &owner,
        &owner_pubkey_hex,
        transfer_order(&data_dir, "recipient-a", 10),
    ))
    .expect("signed a");
    let signed_b = serde_json::to_string(&signed_owned_order(
        &owner,
        &owner_pubkey_hex,
        transfer_order(&data_dir, "recipient-b", 11),
    ))
    .expect("signed b");
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(3));
    let spawn = |signed_json: String| {
        let thread_data_dir = data_dir.clone();
        let thread_barrier = std::sync::Arc::clone(&barrier);
        std::thread::spawn(move || {
            thread_barrier.wait();
            owned_sign(
                NodeOptions {
                    data_dir: thread_data_dir,
                },
                &signed_json,
                "validator-0",
            )
        })
    };
    let handle_a = spawn(signed_a);
    let handle_b = spawn(signed_b);
    barrier.wait();
    let results = [handle_a.join().expect("join a"), handle_b.join().expect("join b")];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter_map(|result| result.as_ref().err())
            .filter(|error| error.kind() == io::ErrorKind::AlreadyExists)
            .count(),
        1
    );
    let locks = super::super::consensus_artifacts::load_owned_input_locks_for_test(&data_dir)
        .expect("load locks");
    assert_eq!(locks.len(), 1);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn persisted_lock_survives_signer_restart_window_and_refuses_conflict() {
    let (data_dir, owner, owner_pubkey_hex) =
        owned_sign_fixture("postfiat-owned-sign-restart-safety");
    let signed_a = signed_owned_order(
        &owner,
        &owner_pubkey_hex,
        transfer_order(&data_dir, "recipient-a", 20),
    );
    super::super::consensus_artifacts::reserve_owned_transfer_lock_for_test(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &signed_a.order,
    )
    .expect("persist lock before simulated crash");
    assert!(data_dir.join("owned_locks.wal").exists());
    assert!(
        !data_dir.join("owned_locks.json").exists(),
        "new reservations must use the single-sync WAL path"
    );

    owned_sign(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &serde_json::to_string(&signed_a).expect("same signed json"),
        "validator-0",
    )
    .expect("idempotent recovery may emit the same vote");

    let signed_b = signed_owned_order(
        &owner,
        &owner_pubkey_hex,
        transfer_order(&data_dir, "recipient-b", 21),
    );
    let conflict = owned_sign(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &serde_json::to_string(&signed_b).expect("conflicting signed json"),
        "validator-0",
    )
    .expect_err("durable lock must refuse conflict after restart");
    assert_eq!(conflict.kind(), io::ErrorKind::AlreadyExists);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn owned_lock_wal_ignores_only_an_uncommitted_torn_tail() {
    let (data_dir, owner, owner_pubkey_hex) =
        owned_sign_fixture("postfiat-owned-lock-wal-torn-tail");
    let signed = signed_owned_order(
        &owner,
        &owner_pubkey_hex,
        transfer_order(&data_dir, "recipient-a", 30),
    );
    super::super::consensus_artifacts::reserve_owned_transfer_lock_for_test(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &signed.order,
    )
    .expect("persist lock");
    let mut wal = std::fs::OpenOptions::new()
        .append(true)
        .open(data_dir.join("owned_locks.wal"))
        .expect("open WAL");
    use std::io::Write as _;
    wal.write_all(b"{\"schema\":\"postfiat-owned-lock-wal-v1\"")
        .expect("append simulated torn tail");
    wal.sync_all().expect("sync simulated torn tail");

    let locks = super::super::consensus_artifacts::load_owned_input_locks_for_test(&data_dir)
        .expect("load committed prefix");
    assert_eq!(locks.len(), 1);

    let conflict = owned_sign(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &serde_json::to_string(&signed_owned_order(
            &owner,
            &owner_pubkey_hex,
            transfer_order(&data_dir, "recipient-b", 31),
        ))
        .expect("conflicting signed json"),
        "validator-0",
    )
    .expect_err("committed prefix lock must survive a torn tail");
    assert_eq!(conflict.kind(), io::ErrorKind::AlreadyExists);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn owned_safe_unlock_is_structurally_disabled_and_preserves_old_locks() {
    let data_dir = unique_test_dir("postfiat-owned-safe-unlock-disabled");
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    let original = b"{\"object-0:7:old-registry\":\"order-hash\"}\n";
    atomic_write(data_dir.join("owned_locks.json"), original).expect("write old lock");
    let error = owned_safe_unlock(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("unsafe unlock must fail closed");
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    assert_eq!(
        std::fs::read(data_dir.join("owned_locks.json")).expect("read retained lock"),
        original
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn fastpay_v3_real_store_persists_certificate_effect_fence_before_signed_ack() {
    let data_dir = unique_test_dir("postfiat-fastpay-v3-atomic-apply");
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    let owner = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner keygen");
    let owner_pubkey_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
    let validators = (0..4)
        .map(|index| {
            (
                format!("validator-{index}"),
                postfiat_crypto_provider::ml_dsa_65_keygen().expect("validator keygen"),
            )
        })
        .collect::<Vec<_>>();
    let registry = ValidatorRegistry {
        validators: validators
            .iter()
            .map(|(node_id, keypair)| ValidatorRegistryRecord {
                node_id: node_id.clone(),
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: postfiat_crypto_provider::bytes_to_hex(&keypair.public_key),
            })
            .collect(),
    };
    write_validator_registry_file(&data_dir.join("validator_registry.json"), &registry)
        .expect("write registry");
    let key_records = validators
        .iter()
        .map(|(node_id, keypair)| {
            serde_json::json!({
                "node_id": node_id,
                "algorithm_id": ML_DSA_65_ALGORITHM,
                "public_key_hex": postfiat_crypto_provider::bytes_to_hex(&keypair.public_key),
                "private_key_hex": postfiat_crypto_provider::bytes_to_hex(&keypair.private_key),
            })
        })
        .collect::<Vec<_>>();
    atomic_write(
        data_dir.join("validator_keys.json"),
        format!(
            "{}\n",
            serde_json::to_string_pretty(&serde_json::json!({"validators": key_records}))
                .expect("key json")
        ),
    )
    .expect("write validator keys");

    let genesis = postfiat_types::Genesis::new_with_validator_count("fastpay-v3-node-test", 4);
    let store = NodeStore::new(&data_dir);
    store.write_genesis(&genesis).expect("write genesis");
    store
        .write_governance(&postfiat_types::GovernanceState::new(4))
        .expect("write governance");
    store
        .write_chain_tip(&postfiat_types::ChainTipState {
            schema: CHAIN_TIP_SCHEMA.to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            height: 105,
            block_hash: "aa".repeat(48),
            state_root: "bb".repeat(48),
            ordered_batch_count: 0,
            receipt_count: 0,
            history_base_height: 0,
        })
        .expect("write tip");
    let mut ledger = postfiat_types::LedgerState::empty();
    ledger.fastpay_recovery_policy = Some(postfiat_types::FastPayRecoveryPolicyV1 {
        schema: postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
        activation_height: 90,
        max_validity_blocks: 20,
        max_recovery_blocks: 20,
    });
    ledger.fastpay_recovery_committees.push(
        postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
            genesis.chain_id.clone(),
            genesis_hash(&genesis),
            genesis.protocol_version,
            1,
            90,
            120,
            validators
                .iter()
                .map(|(validator_id, keypair)| {
                    (
                        validator_id.clone(),
                        postfiat_crypto_provider::bytes_to_hex(&keypair.public_key),
                    )
                })
                .collect(),
        )
        .expect("build v3 recovery committee"),
    );
    ledger.owned_objects.push(postfiat_types::OwnedObject {
        id: "v3-node-input".to_string(),
        version: 7,
        owner_pubkey_hex: owner_pubkey_hex.clone(),
        value: 100,
        asset: "PFT".to_string(),
    });
    store.write_ledger(&ledger).expect("write ledger");

    let mut order = postfiat_types::OwnedTransferOrderV3 {
        domain: owned_certificate_domain_v3(&data_dir).expect("v3 domain"),
        recovery: postfiat_types::FastPayOrderRecoveryV1 {
            schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
            committee_epoch: 1,
            lock_id: "00".repeat(48),
            valid_from_height: 100,
            expires_at_height: 110,
            recovery_closes_at_height: 120,
        },
        inputs: vec![postfiat_types::OwnedObjectRef {
            id: "v3-node-input".to_string(),
            version: 7,
        }],
        outputs: vec![postfiat_types::OwnedOutputSpec {
            owner_pubkey_hex: "recipient-v3".to_string(),
            value: 99,
            asset: "PFT".to_string(),
        }],
        fee: 1,
        nonce: 1,
        memos: Vec::new(),
    };
    order.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&order);
    let signing_bytes = postfiat_execution::owned_transfer_v3_signing_bytes(&order);
    let owner_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
        &owner.private_key,
        &signing_bytes,
        postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
    )
    .expect("owner sign");
    let signed = postfiat_types::SignedOwnedTransferOrderV3 {
        order: order.clone(),
        owner_pubkey_hex: owner_pubkey_hex.clone(),
        owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_signature),
    };
    let signed_json = serde_json::to_string(&signed).expect("signed order json");
    let votes = validators
        .iter()
        .take(3)
        .map(|(validator_id, _)| {
            let response = owned_sign_v3(
                NodeOptions {
                    data_dir: data_dir.clone(),
                },
                &signed_json,
                validator_id,
            )
            .expect("v3 validator vote");
            serde_json::from_str::<postfiat_types::OwnedTransferVote>(&response)
                .expect("vote response")
        })
        .collect::<Vec<_>>();
    let certificate = postfiat_types::OwnedTransferCertificateV3 {
        order,
        owner_pubkey_hex: owner_pubkey_hex.clone(),
        owner_signature_hex: signed.owner_signature_hex,
        votes,
    };
    let cert_json = serde_json::to_string(&certificate).expect("certificate json");
    let ack_json = owned_apply_v3(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &cert_json,
        "validator-0",
    )
    .expect("v3 apply");
    let acknowledgement: postfiat_types::FastPayApplyAckV1 =
        serde_json::from_str(&ack_json).expect("ack json");
    assert!(postfiat_execution::verify_fastpay_apply_ack_v1(
        &acknowledgement,
        &postfiat_crypto_provider::bytes_to_hex(&validators[0].1.public_key),
    ));

    let after = store.read_ledger().expect("ledger after apply");
    assert!(after
        .owned_objects
        .iter()
        .all(|object| object.id != "v3-node-input"));
    assert_eq!(
        after.owned_objects.iter().map(|object| object.value).sum::<u64>(),
        99
    );
    assert_eq!(after.fastpay_version_fences.len(), 1);
    assert!(after.fastpay_version_fences[0].certificate.is_some());
    assert_eq!(
        after.fastpay_version_fences[0].lock_id,
        certificate.order.recovery.lock_id
    );

    let replay_ack = owned_apply_v3(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &cert_json,
        "validator-1",
    )
    .expect("idempotent apply ack");
    let replay_ack: postfiat_types::FastPayApplyAckV1 =
        serde_json::from_str(&replay_ack).expect("replay ack json");
    assert!(postfiat_execution::verify_fastpay_apply_ack_v1(
        &replay_ack,
        &postfiat_crypto_provider::bytes_to_hex(&validators[1].1.public_key),
    ));
    assert_eq!(store.read_ledger().expect("idempotent ledger"), after);

    let retrieved = owned_certificate_v3(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &certificate.order.recovery.lock_id,
    )
    .expect("retrieve durable certificate");
    assert_eq!(
        serde_json::from_str::<postfiat_types::FastPayCertificateV1>(&retrieved)
            .expect("retrieved certificate"),
        postfiat_types::FastPayCertificateV1::Transfer(certificate)
    );
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn ordered_fastpay_recovery_cancels_partial_and_withheld_certificates_and_replays() {
    let data_dir = unique_test_dir("postfiat-fastpay-v3-ordered-recovery");
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-fastpay-v3-ordered-recovery".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 4,
    })
    .expect("init ordered recovery node");
    let store = NodeStore::new(&data_dir);
    let genesis = store.read_genesis().expect("ordered recovery genesis");
    let validator_keys = read_validator_key_file(&data_dir.join(VALIDATOR_KEYS_FILE))
        .expect("ordered recovery validator keys");
    let validators = local_validator_ids(4).expect("ordered recovery validators");
    let committee = postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
        genesis.chain_id.clone(),
        genesis_hash(&genesis),
        genesis.protocol_version,
        1,
        2,
        100,
        validator_keys
            .validators
            .iter()
            .map(|record| (record.node_id.clone(), record.public_key_hex.clone()))
            .collect(),
    )
    .expect("ordered recovery committee");
    let payload = postfiat_types::FastPayRecoveryGovernancePayloadV1 {
        policy: postfiat_types::FastPayRecoveryPolicyV1 {
            schema: postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
            activation_height: 2,
            max_validity_blocks: 2,
            max_recovery_blocks: 2,
        },
        committee: committee.clone(),
    };
    let payload_file = data_dir.join("ordered-recovery-policy.json");
    let amendment_file = data_dir.join("ordered-recovery-amendment.json");
    let bootstrap_file = data_dir.join("ordered-recovery-bootstrap.json");
    atomic_write(
        &payload_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&payload).expect("recovery payload JSON")
        ),
    )
    .expect("write recovery payload");
    create_fastpay_recovery_governance_bootstrap(
        FastPayRecoveryGovernanceBootstrapOptions {
            data_dir: data_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            veto_until_height: 0,
            payload_file,
            amendment_file,
            batch_file: bootstrap_file.clone(),
        },
    )
    .expect("create recovery bootstrap");
    let bootstrap_receipts =
        apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: bootstrap_file,
            certificate_file: None,
        })
        .expect("commit recovery bootstrap");
    assert_eq!(bootstrap_receipts.len(), 1);
    assert!(bootstrap_receipts[0].accepted, "{bootstrap_receipts:?}");
    assert_eq!(
        store.read_chain_tip().expect("tip after bootstrap").height,
        1
    );

    let owner = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owned object owner");
    let owner_pubkey_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
    let source = read_transfer_key_file(&data_dir, None).expect("faucet transfer key");
    let initial_balance = store
        .read_ledger()
        .expect("initial ledger")
        .account(&source.address)
        .expect("faucet account")
        .balance;
    let first_deposit = commit_fastlane_primary_for_test(
        &data_dir,
        "recovery-deposit-one",
        signed_owned_deposit_for_test(&genesis, &source, owner.public_key.clone(), 1, 100, [81; 32]),
    );
    assert!(first_deposit.accepted, "{first_deposit:?}");
    assert_eq!(first_deposit.code, "owned_deposit_applied");
    assert_eq!(store.read_chain_tip().expect("tip after first deposit").height, 2);
    let first_object = store
        .read_ledger()
        .expect("ledger after first deposit")
        .owned_objects
        .into_iter()
        .find(|object| object.owner_pubkey_hex == owner_pubkey_hex && object.value == 100)
        .expect("first deposited object");

    let signed_order = |object: &postfiat_types::OwnedObject,
                        valid_from_height: u64,
                        expires_at_height: u64,
                        recovery_closes_at_height: u64,
                        nonce: u64| {
        let mut order = postfiat_types::OwnedTransferOrderV3 {
            domain: committee.certificate_domain(),
            recovery: postfiat_types::FastPayOrderRecoveryV1 {
                schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
                committee_epoch: committee.committee_epoch,
                lock_id: "00".repeat(48),
                valid_from_height,
                expires_at_height,
                recovery_closes_at_height,
            },
            inputs: vec![postfiat_types::OwnedObjectRef {
                id: object.id.clone(),
                version: object.version,
            }],
            outputs: vec![postfiat_types::OwnedOutputSpec {
                owner_pubkey_hex: "ordered-recovery-recipient".to_string(),
                value: object.value - 1,
                asset: object.asset.clone(),
            }],
            fee: 1,
            nonce,
            memos: Vec::new(),
        };
        order.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&order);
        let signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
            &owner.private_key,
            &postfiat_execution::owned_transfer_v3_signing_bytes(&order),
            postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
        )
        .expect("sign v3 owned order");
        postfiat_types::SignedOwnedTransferOrderV3 {
            order,
            owner_pubkey_hex: owner_pubkey_hex.clone(),
            owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&signature),
        }
    };
    let withheld_signed = signed_order(&first_object, 2, 2, 4, 1);
    let withheld_json = serde_json::to_string(&withheld_signed).expect("withheld order JSON");
    let withheld_votes = validators
        .iter()
        .take(committee.quorum)
        .map(|validator| {
            serde_json::from_str::<postfiat_types::OwnedTransferVote>(
                &owned_sign_v3(
                    NodeOptions {
                        data_dir: data_dir.clone(),
                    },
                    &withheld_json,
                    validator,
                )
                .expect("withheld certificate vote"),
            )
            .expect("decode withheld vote")
        })
        .collect::<Vec<_>>();
    let withheld_certificate = postfiat_types::OwnedTransferCertificateV3 {
        order: withheld_signed.order.clone(),
        owner_pubkey_hex: withheld_signed.owner_pubkey_hex.clone(),
        owner_signature_hex: withheld_signed.owner_signature_hex.clone(),
        votes: withheld_votes,
    };

    let second_deposit = commit_fastlane_primary_for_test(
        &data_dir,
        "recovery-deposit-two",
        signed_owned_deposit_for_test(&genesis, &source, owner.public_key.clone(), 2, 20, [82; 32]),
    );
    assert!(second_deposit.accepted, "{second_deposit:?}");
    assert_eq!(store.read_chain_tip().expect("tip after second deposit").height, 3);
    let second_object = store
        .read_ledger()
        .expect("ledger after second deposit")
        .owned_objects
        .into_iter()
        .find(|object| object.owner_pubkey_hex == owner_pubkey_hex && object.value == 20)
        .expect("second deposited object");
    let partial_signed = signed_order(&second_object, 3, 3, 4, 2);
    let partial_json = serde_json::to_string(&partial_signed).expect("partial order JSON");
    for validator in validators.iter().take(committee.quorum - 1) {
        owned_sign_v3(
            NodeOptions {
                data_dir: data_dir.clone(),
            },
            &partial_json,
            validator,
        )
        .expect("partial order vote");
    }

    let decision = |signed: postfiat_types::SignedOwnedTransferOrderV3| {
        postfiat_types::FastLanePrimaryTransactionV1 {
            operation: postfiat_types::FastLanePrimaryOperationV1::FastPayRecoveryDecision {
                request: postfiat_types::FastPayRecoveryDecisionRequestV1 {
                    schema: postfiat_types::FASTPAY_RECOVERY_DECISION_REQUEST_SCHEMA_V1.to_string(),
                    submitted_at_height: 4,
                    signed_order: postfiat_types::FastPaySignedOrderV1::Transfer(signed),
                },
            },
        }
    };
    for transaction in [decision(withheld_signed), decision(partial_signed)] {
        admit_fastlane_primary_to_mempool(&data_dir, transaction)
            .expect("admit exact-boundary recovery decision");
    }
    let decision_batch_file = data_dir.join("ordered-recovery-decisions.batch.json");
    let decision_batch = create_mempool_batch(MempoolBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: decision_batch_file.clone(),
        max_transactions: 2,
    })
    .expect("create recovery decision batch");
    assert_eq!(decision_batch.fastlane_primary_transactions.len(), 2);
    let decision_receipts = apply_batch(ApplyBatchOptions {
        data_dir: data_dir.clone(),
        batch_file: decision_batch_file,
        certificate_file: None,
    })
    .expect("commit recovery decisions");
    assert_eq!(decision_receipts.len(), 2);
    assert!(decision_receipts.iter().all(|receipt| receipt.accepted));
    assert!(decision_receipts
        .iter()
        .all(|receipt| receipt.code == "fastpay_recovery_cancelled"));
    assert_eq!(store.read_chain_tip().expect("tip after recovery").height, 4);

    let recovered = store.read_ledger().expect("ledger after recovery");
    assert_eq!(recovered.fastpay_version_fences.len(), 2);
    for original in [&first_object, &second_object] {
        let advanced = recovered
            .owned_objects
            .iter()
            .find(|object| object.id == original.id)
            .expect("advanced recovery object");
        assert_eq!(advanced.version, original.version + 1);
        assert_eq!(advanced.value, original.value);
        assert_eq!(advanced.owner_pubkey_hex, original.owner_pubkey_hex);
    }
    let final_account_balance = recovered
        .account(&source.address)
        .expect("faucet after deposits")
        .balance;
    assert_eq!(final_account_balance + 100 + 20 + 2, initial_balance);

    let before_late = recovered.clone();
    let late_apply = owned_apply_v3(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &serde_json::to_string(&withheld_certificate).expect("withheld certificate JSON"),
        "validator-0",
    )
    .expect_err("withheld certificate must not apply after ordered cancellation");
    assert!(
        late_apply.to_string().contains("Expired")
            || late_apply.to_string().contains("VersionFenced"),
        "{late_apply}"
    );
    assert_eq!(store.read_ledger().expect("ledger after late apply"), before_late);

    let late_reveal = postfiat_types::FastLanePrimaryTransactionV1 {
        operation: postfiat_types::FastLanePrimaryOperationV1::FastPayRecoveryReveal {
            certificate: postfiat_types::FastPayCertificateV1::Transfer(withheld_certificate),
        },
    };
    let late_reveal_error = admit_fastlane_primary_to_mempool(&data_dir, late_reveal)
        .expect_err("ordered late reveal must fail without entering the mempool");
    assert_eq!(late_reveal_error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(store.read_ledger().expect("ledger after late reveal"), before_late);
    assert!(mempool_state(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("mempool after late reveal")
    .pending_fastlane_primary
    .is_empty());

    let third_deposit = commit_fastlane_primary_for_test(
        &data_dir,
        "recovery-deposit-three",
        signed_owned_deposit_for_test(&genesis, &source, owner.public_key.clone(), 3, 30, [83; 32]),
    );
    assert!(third_deposit.accepted, "{third_deposit:?}");
    let third_object = store
        .read_ledger()
        .expect("ledger after third deposit")
        .owned_objects
        .into_iter()
        .find(|object| object.owner_pubkey_hex == owner_pubkey_hex && object.value == 30)
        .expect("third deposited object");
    let lagging_snapshot_dir = data_dir.with_file_name(format!(
        "{}-lagging-snapshot",
        data_dir.file_name().unwrap().to_string_lossy()
    ));
    let lagging_data_dir = data_dir.with_file_name(format!(
        "{}-lagging",
        data_dir.file_name().unwrap().to_string_lossy()
    ));
    export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: lagging_snapshot_dir.clone(),
    })
    .expect("export pre-FastPay lagging snapshot");
    import_snapshot(SnapshotImportOptions {
        data_dir: lagging_data_dir.clone(),
        snapshot_dir: lagging_snapshot_dir.clone(),
        node_id: Some("validator-lagging".to_string()),
    })
    .expect("restore pre-FastPay lagging validator");
    let direct_signed = signed_order(&third_object, 5, 7, 9, 3);
    let direct_json = serde_json::to_string(&direct_signed).expect("direct order JSON");
    let direct_votes = validators
        .iter()
        .take(committee.quorum)
        .map(|validator| {
            serde_json::from_str::<postfiat_types::OwnedTransferVote>(
                &owned_sign_v3(
                    NodeOptions {
                        data_dir: data_dir.clone(),
                    },
                    &direct_json,
                    validator,
                )
                .expect("direct certificate vote"),
            )
            .expect("decode direct vote")
        })
        .collect::<Vec<_>>();
    let direct_certificate = postfiat_types::OwnedTransferCertificateV3 {
        order: direct_signed.order.clone(),
        owner_pubkey_hex: direct_signed.owner_pubkey_hex.clone(),
        owner_signature_hex: direct_signed.owner_signature_hex.clone(),
        votes: direct_votes,
    };
    owned_apply_v3(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &serde_json::to_string(&direct_certificate).expect("direct certificate JSON"),
        "validator-0",
    )
    .expect("direct FastPay apply at block tip");
    let tip_overlay_replay = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("replay direct FastPay effect after its decision height");
    assert_eq!(tip_overlay_replay.block_count, 5);

    let fourth_deposit = commit_fastlane_primary_for_test(
        &data_dir,
        "recovery-deposit-four",
        signed_owned_deposit_for_test(&genesis, &source, owner.public_key.clone(), 4, 10, [84; 32]),
    );
    assert!(fourth_deposit.accepted, "{fourth_deposit:?}");

    let source_block = store
        .read_blocks()
        .expect("source blocks after direct effect anchor")
        .blocks
        .last()
        .cloned()
        .expect("source anchor block");
    assert_eq!(source_block.fastpay_pre_state_effects.len(), 1);
    assert_eq!(
        source_block.fastpay_pre_state_effects[0].lock_id,
        direct_certificate.order.recovery.lock_id
    );
    let lagging_block_file = lagging_data_dir.join("fastpay-anchor-block.json");
    let lagging_certificate_file = lagging_data_dir.join("fastpay-anchor-certificate.json");
    atomic_write(
        &lagging_block_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&source_block).expect("anchor block JSON")
        ),
    )
    .expect("write lagging anchor block");
    let lagging_certificate = reconstruct_block_certificate_from_archive(
        BlockCertificateFromArchiveOptions {
        data_dir: lagging_data_dir.clone(),
        block_file: lagging_block_file.clone(),
        batch_file: data_dir.join("recovery-deposit-four.batch.json"),
        certificate_file: lagging_certificate_file.clone(),
    },
    )
    .expect("reconstruct lagging anchor certificate");
    assert_eq!(
        lagging_certificate.fastpay_pre_state_effects,
        source_block.fastpay_pre_state_effects
    );
    apply_batch_with_replay(
        ApplyBatchOptions {
            data_dir: lagging_data_dir.clone(),
            batch_file: data_dir.join("recovery-deposit-four.batch.json"),
            certificate_file: Some(lagging_certificate_file),
        },
        Some(lagging_block_file),
    )
    .expect("lagging validator catches up through block-carried FastPay effect");
    assert_eq!(
        NodeStore::new(&lagging_data_dir)
            .read_ledger()
            .expect("lagging ledger after anchor"),
        store.read_ledger().expect("source ledger after anchor"),
        "lagging validator must reconstruct the direct effect from certified history",
    );

    for (label, mutate) in [
        (
            "omitted",
            (|block: &mut BlockRecord| block.fastpay_pre_state_effects.clear())
                as fn(&mut BlockRecord),
        ),
        (
            "duplicated",
            |block: &mut BlockRecord| {
                block
                    .fastpay_pre_state_effects
                    .push(block.fastpay_pre_state_effects[0].clone());
            },
        ),
    ] {
        let mut malformed = source_block.clone();
        mutate(&mut malformed);
        let malformed_block_file = lagging_data_dir.join(format!("fastpay-{label}-block.json"));
        let malformed_certificate_file =
            lagging_data_dir.join(format!("fastpay-{label}-certificate.json"));
        atomic_write(
            &malformed_block_file,
            format!(
                "{}\n",
                serde_json::to_string_pretty(&malformed).expect("malformed block JSON")
            ),
        )
        .expect("write malformed anchor block");
        reconstruct_block_certificate_from_archive(BlockCertificateFromArchiveOptions {
            data_dir: lagging_data_dir.clone(),
            block_file: malformed_block_file,
            batch_file: data_dir.join("recovery-deposit-four.batch.json"),
            certificate_file: malformed_certificate_file,
        })
        .expect_err("omitted or duplicated FastPay anchor evidence must reject");
    }

    let replay = verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("replay ordered and consensusless FastPay history");
    assert!(replay.verified);
    assert_eq!(replay.block_count, 6);
    let final_ledger = store.read_ledger().expect("final recovery ledger");
    assert_eq!(final_ledger.fastpay_version_fences.len(), 3);
    assert!(final_ledger
        .owned_objects
        .iter()
        .any(|object| object.owner_pubkey_hex == "ordered-recovery-recipient" && object.value == 29));
    let mut tampered_ledger = final_ledger.clone();
    let direct_fence = tampered_ledger
        .fastpay_version_fences
        .iter_mut()
        .find(|fence| fence.lock_id == direct_certificate.order.recovery.lock_id)
        .expect("direct confirmed fence");
    match direct_fence.certificate.as_mut().expect("direct certificate") {
        postfiat_types::FastPayCertificateV1::Transfer(certificate) => {
            let replacement = if certificate.owner_signature_hex.starts_with("00") {
                "01"
            } else {
                "00"
            };
            certificate
                .owner_signature_hex
                .replace_range(0..2, replacement);
        }
        postfiat_types::FastPayCertificateV1::Unwrap(_) => panic!("expected transfer fence"),
    }
    store
        .write_ledger(&tampered_ledger)
        .expect("write tampered FastPay replay fixture");
    verify_blocks(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect_err("replay must reject a tampered retained FastPay certificate");
    store
        .write_ledger(&final_ledger)
        .expect("restore authenticated FastPay ledger");
    let source_status = status(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("ordered recovery status");
    let snapshot_dir = data_dir.with_file_name(format!(
        "{}-snapshot",
        data_dir.file_name().unwrap().to_string_lossy()
    ));
    let restored_dir = data_dir.with_file_name(format!(
        "{}-restored",
        data_dir.file_name().unwrap().to_string_lossy()
    ));
    export_snapshot(SnapshotExportOptions {
        data_dir: data_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
    })
    .expect("export ordered recovery snapshot");
    let restored = import_snapshot(SnapshotImportOptions {
        data_dir: restored_dir.clone(),
        snapshot_dir: snapshot_dir.clone(),
        node_id: Some("validator-restored".to_string()),
    })
    .expect("restore ordered recovery snapshot");
    assert_eq!(restored.state_root, source_status.state_root);
    assert_eq!(
        NodeStore::new(&restored_dir)
            .read_ledger()
            .expect("restored recovery ledger"),
        final_ledger
    );
    verify_blocks(NodeOptions {
        data_dir: restored_dir.clone(),
    })
    .expect("replay restored ordered recovery history");

    std::fs::remove_dir_all(data_dir).expect("cleanup ordered recovery data");
    std::fs::remove_dir_all(lagging_snapshot_dir).expect("cleanup lagging snapshot");
    std::fs::remove_dir_all(lagging_data_dir).expect("cleanup lagging validator");
    std::fs::remove_dir_all(snapshot_dir).expect("cleanup ordered recovery snapshot");
    std::fs::remove_dir_all(restored_dir).expect("cleanup ordered recovery restore");
}

#[test]
fn six_validators_certify_anchor_and_catch_up_one_missing_fastpay_effect() {
    let root = unique_test_dir("postfiat-fastpay-v3-six-validator-anchor");
    let source_dir = root.join("validator-0");
    init(InitOptions {
        data_dir: source_dir.clone(),
        chain_id: "postfiat-fastpay-v3-six-validator-anchor".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 6,
    })
    .expect("init six-validator FastPay source");
    let source_store = NodeStore::new(&source_dir);
    let genesis = source_store.read_genesis().expect("six-validator genesis");
    let validators = local_validator_ids(6).expect("six-validator IDs");
    let validator_keys = read_validator_key_file(&source_dir.join(VALIDATOR_KEYS_FILE))
        .expect("six-validator keys");
    let committee = postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
        genesis.chain_id.clone(),
        genesis_hash(&genesis),
        genesis.protocol_version,
        1,
        2,
        100,
        validator_keys
            .validators
            .iter()
            .map(|record| (record.node_id.clone(), record.public_key_hex.clone()))
            .collect(),
    )
    .expect("six-validator FastPay committee");
    let payload = postfiat_types::FastPayRecoveryGovernancePayloadV1 {
        policy: postfiat_types::FastPayRecoveryPolicyV1 {
            schema: postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
            activation_height: 2,
            max_validity_blocks: 4,
            max_recovery_blocks: 4,
        },
        committee: committee.clone(),
    };
    let payload_file = root.join("fastpay-policy.json");
    let amendment_file = root.join("fastpay-amendment.json");
    let bootstrap_file = root.join("fastpay-bootstrap.json");
    atomic_write(
        &payload_file,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&payload).expect("six-validator policy JSON")
        ),
    )
    .expect("write six-validator FastPay policy");
    create_fastpay_recovery_governance_bootstrap(
        FastPayRecoveryGovernanceBootstrapOptions {
            data_dir: source_dir.clone(),
            validators: validators.clone(),
            support: validators.clone(),
            veto_until_height: 0,
            payload_file,
            amendment_file,
            batch_file: bootstrap_file.clone(),
        },
    )
    .expect("create six-validator FastPay bootstrap");
    let bootstrap_receipts =
        apply_unsigned_governance_fixture_for_test(ApplyBatchOptions {
            data_dir: source_dir.clone(),
            batch_file: bootstrap_file,
            certificate_file: None,
        })
        .expect("apply six-validator FastPay bootstrap");
    assert_eq!(bootstrap_receipts.len(), 1);
    assert!(bootstrap_receipts[0].accepted, "{bootstrap_receipts:?}");

    let owner = postfiat_crypto_provider::ml_dsa_65_keygen().expect("six-validator owner");
    let owner_pubkey_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
    let source = read_transfer_key_file(&source_dir, None).expect("six-validator faucet key");
    let deposit = commit_fastlane_primary_for_test(
        &source_dir,
        "six-validator-deposit",
        signed_owned_deposit_for_test(
            &genesis,
            &source,
            owner.public_key.clone(),
            1,
            100,
            [91; 32],
        ),
    );
    assert!(deposit.accepted, "{deposit:?}");
    let input = source_store
        .read_ledger()
        .expect("six-validator deposited ledger")
        .owned_objects
        .into_iter()
        .find(|object| object.owner_pubkey_hex == owner_pubkey_hex && object.value == 100)
        .expect("six-validator deposited object");

    let mut data_dirs = vec![source_dir.clone()];
    for index in 1..6 {
        let data_dir = root.join(format!("validator-{index}"));
        copy_fastpay_node_dir(&source_dir, &data_dir);
        rewrite_fastpay_node_id(&data_dir, &format!("validator-{index}"));
        data_dirs.push(data_dir);
    }

    let mut order = postfiat_types::OwnedTransferOrderV3 {
        domain: committee.certificate_domain(),
        recovery: postfiat_types::FastPayOrderRecoveryV1 {
            schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
            committee_epoch: committee.committee_epoch,
            lock_id: "00".repeat(48),
            valid_from_height: 2,
            expires_at_height: 6,
            recovery_closes_at_height: 10,
        },
        inputs: vec![postfiat_types::OwnedObjectRef {
            id: input.id.clone(),
            version: input.version,
        }],
        outputs: vec![postfiat_types::OwnedOutputSpec {
            owner_pubkey_hex: "six-validator-recipient".to_string(),
            value: 99,
            asset: input.asset.clone(),
        }],
        fee: 1,
        nonce: 1,
        memos: Vec::new(),
    };
    order.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&order);
    let owner_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
        &owner.private_key,
        &postfiat_execution::owned_transfer_v3_signing_bytes(&order),
        postfiat_execution::OWNED_TRANSFER_CONTEXT_V3,
    )
    .expect("sign six-validator FastPay order");
    let signed = postfiat_types::SignedOwnedTransferOrderV3 {
        order: order.clone(),
        owner_pubkey_hex: owner_pubkey_hex.clone(),
        owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_signature),
    };
    let signed_json = serde_json::to_string(&signed).expect("six-validator signed order JSON");
    let votes = data_dirs
        .iter()
        .zip(&validators)
        .take(committee.quorum)
        .map(|(data_dir, validator)| {
            serde_json::from_str::<postfiat_types::OwnedTransferVote>(
                &owned_sign_v3(
                    NodeOptions {
                        data_dir: data_dir.clone(),
                    },
                    &signed_json,
                    validator,
                )
                .expect("six-validator FastPay vote"),
            )
            .expect("decode six-validator FastPay vote")
        })
        .collect::<Vec<_>>();
    assert_eq!(votes.len(), 5);
    let certificate = postfiat_types::OwnedTransferCertificateV3 {
        order,
        owner_pubkey_hex: owner_pubkey_hex.clone(),
        owner_signature_hex: signed.owner_signature_hex,
        votes,
    };
    let certificate_json =
        serde_json::to_string(&certificate).expect("six-validator certificate JSON");
    for (data_dir, validator) in data_dirs.iter().zip(&validators).take(committee.quorum) {
        owned_apply_v3(
            NodeOptions {
                data_dir: data_dir.clone(),
            },
            &certificate_json,
            validator,
        )
        .expect("quorum validator applies FastPay certificate");
    }
    assert!(NodeStore::new(&data_dirs[5])
        .read_ledger()
        .expect("lagging sixth ledger")
        .fastpay_version_fences
        .is_empty());

    let anchor_transaction = signed_owned_deposit_for_test(
        &genesis,
        &source,
        owner.public_key.clone(),
        2,
        10,
        [92; 32],
    );
    admit_fastlane_primary_to_mempool(&source_dir, anchor_transaction)
        .expect("admit FastPay anchor transaction");
    let batch_file = root.join("fastpay-anchor-batch.json");
    create_mempool_batch(MempoolBatchOptions {
        data_dir: source_dir.clone(),
        batch_file: batch_file.clone(),
        max_transactions: 1,
    })
    .expect("create FastPay anchor batch");
    let proposal_file = root.join("fastpay-anchor-proposal.json");
    let proposal = propose_batch(BatchProposalOptions {
        data_dir: source_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: batch_file.clone(),
        proposal_file: proposal_file.clone(),
        view: Some(0),
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose FastPay anchor block");
    assert_eq!(proposal.fastpay_pre_state_effects.len(), 1);
    let oversized_effects = vec![
        proposal.fastpay_pre_state_effects[0].clone();
        postfiat_types::MAX_FASTPAY_PRE_STATE_EFFECTS_PER_BLOCK + 1
    ];
    let oversized_error = validate_fastpay_pre_state_effects(&oversized_effects)
        .expect_err("oversized FastPay block evidence must reject before allocation or replay");
    assert!(oversized_error.to_string().contains("per-block limit"));

    let mut vote_files = Vec::new();
    for (data_dir, validator) in data_dirs.iter().zip(&validators) {
        let vote_file = root.join(format!("{validator}.anchor-vote.json"));
        create_block_vote(BlockVoteOptions {
            data_dir: data_dir.clone(),
            verify_block_log: true,
            key_file: data_dir.join(VALIDATOR_KEYS_FILE),
            validator_id: Some(validator.clone()),
            batch_file: Some(batch_file.clone()),
            proposal_file: Some(proposal_file.clone()),
            timeout_certificate_file: None,
            block_height: Some(proposal.block_height),
            vote_file: vote_file.clone(),
        })
        .expect("validator votes for FastPay anchor block");
        vote_files.push(vote_file);
    }
    let block_certificate_file = root.join("fastpay-anchor-block-certificate.json");
    let block_certificate = aggregate_verified_block_certificate(BlockCertificateOptions {
        data_dir: source_dir.clone(),
        verify_block_log: true,
        batch_file: Some(batch_file.clone()),
        proposal_file: Some(proposal_file),
        timeout_certificate_file: None,
        block_height: Some(proposal.block_height),
        vote_files,
        certificate_file: block_certificate_file.clone(),
    })
    .expect("aggregate FastPay anchor block certificate");
    assert_eq!(
        block_certificate
            .as_block_certificate_file()
            .fastpay_pre_state_effects,
        proposal.fastpay_pre_state_effects
    );

    for data_dir in &data_dirs {
        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: batch_file.clone(),
            certificate_file: Some(block_certificate_file.clone()),
        })
        .expect("apply certified FastPay anchor block");
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].accepted, "{receipts:?}");
        verify_blocks(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("replay six-validator FastPay anchor history");
    }
    let terminal_ledgers = data_dirs
        .iter()
        .map(|data_dir| NodeStore::new(data_dir).read_ledger().expect("terminal ledger"))
        .collect::<Vec<_>>();
    assert!(terminal_ledgers
        .windows(2)
        .all(|pair| pair[0] == pair[1]));
    let terminal_statuses = data_dirs
        .iter()
        .map(|data_dir| {
            status(NodeOptions {
                data_dir: data_dir.clone(),
            })
            .expect("terminal FastPay status")
        })
        .collect::<Vec<_>>();
    assert!(terminal_statuses.windows(2).all(|pair| {
        pair[0].block_height == pair[1].block_height
            && pair[0].block_tip_hash == pair[1].block_tip_hash
            && pair[0].state_root == pair[1].state_root
    }));

    // A certificate applied by fewer than q validators is not product final.
    // The other q validators can therefore certify a canonical block without
    // that speculative effect. The minority validator must be able to follow
    // the certified chain while retaining the full certificate for ordered
    // recovery; otherwise one withheld apply strands an honest replica.
    let minority_input = terminal_ledgers[0]
        .owned_objects
        .iter()
        .find(|object| object.owner_pubkey_hex == owner_pubkey_hex && object.value == 10)
        .cloned()
        .expect("minority FastPay input");
    let mut minority_order = postfiat_types::OwnedUnwrapOrderV3 {
        domain: committee.certificate_domain(),
        recovery: postfiat_types::FastPayOrderRecoveryV1 {
            schema: postfiat_types::FASTPAY_ORDER_RECOVERY_SCHEMA_V1.to_string(),
            committee_epoch: committee.committee_epoch,
            lock_id: "00".repeat(48),
            valid_from_height: proposal.block_height,
            expires_at_height: proposal.block_height + 4,
            recovery_closes_at_height: proposal.block_height + 8,
        },
        inputs: vec![postfiat_types::OwnedObjectRef {
            id: minority_input.id.clone(),
            version: minority_input.version,
        }],
        to_address: "pf-minority-unwrap-recipient".to_string(),
        amount: 9,
        asset: minority_input.asset.clone(),
        fee: 1,
        nonce: 2,
        memos: Vec::new(),
    };
    minority_order.recovery.lock_id =
        postfiat_types::fastpay_unwrap_lock_id_v1(&minority_order);
    let minority_owner_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
        &owner.private_key,
        &postfiat_execution::owned_unwrap_v3_signing_bytes(&minority_order),
        postfiat_execution::OWNED_UNWRAP_CONTEXT_V3,
    )
    .expect("sign minority FastPay order");
    let minority_signed = postfiat_types::SignedOwnedUnwrapOrderV3 {
        order: minority_order.clone(),
        owner_pubkey_hex: owner_pubkey_hex.clone(),
        owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&minority_owner_signature),
    };
    let minority_signed_json =
        serde_json::to_string(&minority_signed).expect("minority signed order JSON");
    let minority_votes = data_dirs
        .iter()
        .zip(&validators)
        .take(committee.quorum)
        .map(|(data_dir, validator)| {
            serde_json::from_str::<postfiat_types::OwnedUnwrapVote>(
                &owned_unwrap_sign_v3(
                    NodeOptions {
                        data_dir: data_dir.clone(),
                    },
                    &minority_signed_json,
                    validator,
                )
                .expect("minority FastPay vote"),
            )
            .expect("decode minority FastPay vote")
        })
        .collect::<Vec<_>>();
    let minority_certificate = postfiat_types::OwnedUnwrapCertificateV3 {
        order: minority_order,
        owner_pubkey_hex: owner_pubkey_hex.clone(),
        owner_signature_hex: minority_signed.owner_signature_hex,
        votes: minority_votes,
    };
    let minority_certificate_json = serde_json::to_string(&minority_certificate)
        .expect("minority FastPay certificate JSON");
    let minority_direct_crash_seed_dir = root.join("minority-direct-journal-crash-seed");
    copy_fastpay_node_dir(&data_dirs[5], &minority_direct_crash_seed_dir);
    let minority_pre_apply_ledger = NodeStore::new(&data_dirs[5])
        .read_ledger()
        .expect("minority pre-apply ledger");
    owned_unwrap_apply_v3(
        NodeOptions {
            data_dir: data_dirs[5].clone(),
        },
        &minority_certificate_json,
        &validators[5],
    )
    .expect("one validator applies non-final FastPay certificate");
    let minority_applied_ledger = NodeStore::new(&data_dirs[5])
        .read_ledger()
        .expect("minority FastPay unwrap ledger");
    assert!(minority_applied_ledger
        .owned_objects
        .iter()
        .all(|object| object.id != minority_input.id));
    assert_eq!(
        minority_applied_ledger
            .account(&minority_signed.order.to_address)
            .expect("minority unwrap recipient")
            .balance,
        9
    );
    std::fs::copy(
        data_dirs[5].join(FASTPAY_SPECULATIVE_JOURNAL_FILE),
        minority_direct_crash_seed_dir.join(FASTPAY_SPECULATIVE_JOURNAL_FILE),
    )
    .expect("simulate crash after FastPay inverse journal and before ledger");
    status(NodeOptions {
        data_dir: minority_direct_crash_seed_dir.clone(),
    })
    .expect("restart after FastPay inverse journal persistence");
    assert_eq!(
        NodeStore::new(&minority_direct_crash_seed_dir)
            .read_ledger()
            .expect("journal-only crash ledger"),
        minority_pre_apply_ledger,
        "journal persistence alone must not expose the speculative effect"
    );
    owned_unwrap_apply_v3(
        NodeOptions {
            data_dir: minority_direct_crash_seed_dir.clone(),
        },
        &minority_certificate_json,
        &validators[5],
    )
    .expect("retry after journal-only crash completes the exact effect");
    assert_eq!(
        NodeStore::new(&minority_direct_crash_seed_dir)
            .read_ledger()
            .expect("recovered direct FastPay ledger"),
        minority_applied_ledger
    );
    verify_blocks(NodeOptions {
        data_dir: minority_direct_crash_seed_dir.clone(),
    })
    .expect("replay recovered direct FastPay overlay");
    std::fs::remove_dir_all(minority_direct_crash_seed_dir)
        .expect("cleanup direct FastPay crash seed");
    let minority_fence = minority_applied_ledger
        .fastpay_version_fences
        .last()
        .cloned()
        .expect("minority FastPay fence");
    let mut canonical_effect_order = vec![
        proposal.fastpay_pre_state_effects[0].clone(),
        minority_fence,
    ];
    canonical_effect_order.sort_by(|left, right| left.lock_id.cmp(&right.lock_id));
    validate_fastpay_pre_state_effects(&canonical_effect_order)
        .expect("canonical FastPay effect ordering");
    canonical_effect_order.reverse();
    let reorder_error = validate_fastpay_pre_state_effects(&canonical_effect_order)
        .expect_err("reordered FastPay block evidence must reject");
    assert!(reorder_error.to_string().contains("strictly ordered"));

    let canonical_transaction = signed_owned_deposit_for_test(
        &genesis,
        &source,
        owner.public_key.clone(),
        3,
        5,
        [93; 32],
    );
    admit_fastlane_primary_to_mempool(&source_dir, canonical_transaction)
        .expect("admit canonical block after minority FastPay apply");
    let canonical_batch_file = root.join("fastpay-minority-canonical-batch.json");
    create_mempool_batch(MempoolBatchOptions {
        data_dir: source_dir.clone(),
        batch_file: canonical_batch_file.clone(),
        max_transactions: 1,
    })
    .expect("create canonical block after minority FastPay apply");
    let canonical_proposal_file = root.join("fastpay-minority-canonical-proposal.json");
    let canonical_proposal = propose_batch(BatchProposalOptions {
        data_dir: source_dir.clone(),
        verify_block_log: true,
        batch_kind: Some(BATCH_KIND_TRANSPARENT.to_string()),
        batch_file: canonical_batch_file.clone(),
        proposal_file: canonical_proposal_file.clone(),
        view: Some(0),
        timeout_certificate_file: None,
        key_file: None,
        validator_id: None,
    })
    .expect("propose canonical block without minority FastPay effect");
    assert!(canonical_proposal.fastpay_pre_state_effects.is_empty());

    let minority_vote_error = create_block_vote(BlockVoteOptions {
        data_dir: data_dirs[5].clone(),
        verify_block_log: true,
        key_file: data_dirs[5].join(VALIDATOR_KEYS_FILE),
        validator_id: Some(validators[5].clone()),
        batch_file: Some(canonical_batch_file.clone()),
        proposal_file: Some(canonical_proposal_file.clone()),
        timeout_certificate_file: None,
        block_height: Some(canonical_proposal.block_height),
        vote_file: root.join("validator-5.minority-omission-vote.json"),
    })
    .expect_err("minority validator must not vote to omit its local FastPay effect");
    assert!(
        minority_vote_error
            .to_string()
            .contains("omitted a locally durable unanchored FastPay effect"),
        "{minority_vote_error}"
    );

    let mut canonical_vote_files = Vec::new();
    for (data_dir, validator) in data_dirs.iter().zip(&validators).take(committee.quorum) {
        let vote_file = root.join(format!("{validator}.minority-canonical-vote.json"));
        create_block_vote(BlockVoteOptions {
            data_dir: data_dir.clone(),
            verify_block_log: true,
            key_file: data_dir.join(VALIDATOR_KEYS_FILE),
            validator_id: Some(validator.clone()),
            batch_file: Some(canonical_batch_file.clone()),
            proposal_file: Some(canonical_proposal_file.clone()),
            timeout_certificate_file: None,
            block_height: Some(canonical_proposal.block_height),
            vote_file: vote_file.clone(),
        })
        .expect("canonical quorum votes without minority FastPay effect");
        canonical_vote_files.push(vote_file);
    }
    let canonical_certificate_file = root.join("fastpay-minority-canonical-certificate.json");
    aggregate_verified_block_certificate(BlockCertificateOptions {
        data_dir: source_dir.clone(),
        verify_block_log: true,
        batch_file: Some(canonical_batch_file.clone()),
        proposal_file: Some(canonical_proposal_file),
        timeout_certificate_file: None,
        block_height: Some(canonical_proposal.block_height),
        vote_files: canonical_vote_files,
        certificate_file: canonical_certificate_file.clone(),
    })
    .expect("certify canonical block without minority FastPay effect");
    let minority_crash_seed_dir = root.join("minority-certified-rollback-crash-seed");
    copy_fastpay_node_dir(&data_dirs[5], &minority_crash_seed_dir);
    for data_dir in &data_dirs {
        let receipts = apply_batch(ApplyBatchOptions {
            data_dir: data_dir.clone(),
            batch_file: canonical_batch_file.clone(),
            certificate_file: Some(canonical_certificate_file.clone()),
        })
        .expect("all validators follow canonical block after minority FastPay apply");
        assert_eq!(receipts.len(), 1);
        assert!(receipts[0].accepted, "{receipts:?}");
    }
    let post_minority_ledgers = data_dirs
        .iter()
        .map(|data_dir| NodeStore::new(data_dir).read_ledger().expect("post-minority ledger"))
        .collect::<Vec<_>>();
    assert!(post_minority_ledgers.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(post_minority_ledgers[5]
        .owned_objects
        .iter()
        .any(|object| object == &minority_input));
    assert!(post_minority_ledgers[5]
        .account(&minority_signed.order.to_address)
        .is_none());

    let minority_terminal_store = NodeStore::new(&data_dirs[5]);
    let minority_terminal_receipts = minority_terminal_store
        .read_receipts()
        .expect("minority terminal receipts");
    let minority_terminal_ordered = minority_terminal_store
        .read_ordered_batches()
        .expect("minority terminal ordered batches");
    let minority_terminal_archive = minority_terminal_store
        .read_batch_archive()
        .expect("minority terminal archive");
    let minority_terminal_blocks = minority_terminal_store
        .read_blocks()
        .expect("minority terminal blocks");
    let minority_terminal_tip = minority_terminal_store
        .read_chain_tip()
        .expect("minority terminal tip");
    let minority_terminal_block = minority_terminal_blocks
        .blocks
        .last()
        .cloned()
        .expect("minority terminal block");
    let minority_terminal_archive_entry = minority_terminal_archive
        .batches
        .last()
        .cloned()
        .expect("minority terminal archive entry");
    let minority_terminal_receipt_delta = receipts_for_block(
        &minority_terminal_receipts,
        &minority_terminal_block.receipt_ids,
    )
    .expect("minority terminal receipt delta");
    let minority_rollback_journal = OrderedCommitDeltaJournal {
        schema: "postfiat-ordered-commit-delta-journal-v1".to_string(),
        height: minority_terminal_block.header.height,
        ledger: Some(post_minority_ledgers[5].clone()),
        governance: None,
        shielded: None,
        bridge: None,
        receipt_delta: minority_terminal_receipt_delta,
        ordered_batch_id: minority_terminal_block.header.batch_id.clone(),
        archive_entry: minority_terminal_archive_entry,
        block: minority_terminal_block,
        validator_registry: None,
    };
    for write_prefix in 0..=9 {
        let crash_dir = root.join(format!("minority-rollback-crash-prefix-{write_prefix}"));
        copy_fastpay_node_dir(&minority_crash_seed_dir, &crash_dir);
        let crash_store = NodeStore::new(&crash_dir);
        crash_store
            .write_ordered_commit_journal(&minority_rollback_journal)
            .expect("write FastPay rollback ordered journal");
        if write_prefix >= 1 {
            crash_store
                .write_ledger(
                    minority_rollback_journal
                        .ledger
                        .as_ref()
                        .expect("FastPay rollback journal ledger"),
                )
                .expect("write FastPay rollback ledger prefix");
        }
        if write_prefix >= 5 {
            for receipt in &minority_rollback_journal.receipt_delta {
                crash_store
                    .append_receipt_record(receipt)
                    .expect("write FastPay rollback receipt prefix");
            }
        }
        if write_prefix >= 6 {
            crash_store
                .append_ordered_batch_record(&minority_rollback_journal.ordered_batch_id)
                .expect("write FastPay rollback ordered-batch prefix");
        }
        if write_prefix >= 7 {
            crash_store
                .append_batch_archive_entry(minority_rollback_journal.archive_entry.clone())
                .expect("write FastPay rollback archive prefix");
        }
        if write_prefix >= 8 {
            crash_store
                .append_block_record(&minority_rollback_journal.block)
                .expect("write FastPay rollback block prefix");
        }
        if write_prefix >= 9 {
            crash_store
                .write_chain_tip(&minority_terminal_tip)
                .expect("write FastPay rollback tip prefix");
        }
        status(NodeOptions {
            data_dir: crash_dir.clone(),
        })
        .expect("recover FastPay rollback ordered journal");
        assert_eq!(
            crash_store.read_ledger().expect("recovered FastPay ledger"),
            post_minority_ledgers[5]
        );
        assert_eq!(
            crash_store
                .read_receipts()
                .expect("recovered FastPay receipts"),
            minority_terminal_receipts
        );
        assert_eq!(
            crash_store
                .read_ordered_batches()
                .expect("recovered FastPay ordered batches"),
            minority_terminal_ordered
        );
        assert_eq!(
            crash_store
                .read_batch_archive()
                .expect("recovered FastPay archive"),
            minority_terminal_archive
        );
        assert_eq!(
            crash_store.read_blocks().expect("recovered FastPay blocks"),
            minority_terminal_blocks
        );
        assert_eq!(
            crash_store.read_chain_tip().expect("recovered FastPay tip"),
            minority_terminal_tip
        );
        verify_blocks(NodeOptions {
            data_dir: crash_dir.clone(),
        })
        .expect("replay recovered FastPay rollback prefix");
        assert!(crash_store
            .read_ordered_commit_journal_raw()
            .expect("read recovered FastPay ordered journal")
            .is_none());
        std::fs::remove_dir_all(crash_dir).expect("cleanup FastPay crash prefix");
    }
    let retained_certificate = owned_certificate_v3(
        NodeOptions {
            data_dir: data_dirs[5].clone(),
        },
        &minority_certificate.order.recovery.lock_id,
    )
    .expect("minority validator retains certificate for ordered recovery");
    assert_eq!(
        serde_json::from_str::<postfiat_types::FastPayCertificateV1>(&retained_certificate)
            .expect("retained minority certificate"),
        postfiat_types::FastPayCertificateV1::Unwrap(minority_certificate)
    );

    let minority_snapshot_dir = root.join("minority-recovery-snapshot");
    let minority_restored_dir = root.join("minority-recovery-restored");
    export_snapshot(SnapshotExportOptions {
        data_dir: data_dirs[5].clone(),
        snapshot_dir: minority_snapshot_dir.clone(),
    })
    .expect("snapshot rolled-back minority recovery evidence");
    import_snapshot(SnapshotImportOptions {
        data_dir: minority_restored_dir.clone(),
        snapshot_dir: minority_snapshot_dir,
        node_id: Some("validator-minority-restored".to_string()),
    })
    .expect("restore rolled-back minority recovery evidence");
    verify_blocks(NodeOptions {
        data_dir: minority_restored_dir.clone(),
    })
    .expect("replay restored minority recovery state");
    assert_eq!(
        load_owned_input_locks_for_test(&minority_restored_dir)
            .expect("restored FastPay input locks"),
        load_owned_input_locks_for_test(&data_dirs[5]).expect("source FastPay input locks")
    );
    assert_eq!(
        serde_json::from_str::<postfiat_types::FastPayCertificateV1>(
            &owned_certificate_v3(
                NodeOptions {
                    data_dir: minority_restored_dir,
                },
                &minority_signed.order.recovery.lock_id,
            )
            .expect("restored minority certificate"),
        )
        .expect("restored minority certificate JSON"),
        postfiat_types::FastPayCertificateV1::Unwrap(
            serde_json::from_str::<postfiat_types::OwnedUnwrapCertificateV3>(
                &minority_certificate_json,
            )
            .expect("minority certificate fixture"),
        )
    );

    std::fs::remove_dir_all(root).expect("cleanup six-validator FastPay anchor test");
}

#[test]
fn owned_apply_uses_public_registry_with_isolated_local_signer() {
    let data_dir = unique_test_dir("postfiat-owned-apply-public-registry");
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    let owner = postfiat_crypto_provider::ml_dsa_65_keygen().expect("owner keygen");
    let owner_pubkey_hex = postfiat_crypto_provider::bytes_to_hex(&owner.public_key);
    let validators: Vec<(String, postfiat_crypto_provider::MlDsa65KeyPair)> = (0..6)
        .map(|index| {
            (
                format!("validator-{index}"),
                postfiat_crypto_provider::ml_dsa_65_keygen().expect("validator keygen"),
            )
        })
        .collect();
    let registry = ValidatorRegistry {
        validators: validators
            .iter()
            .map(|(node_id, keypair)| ValidatorRegistryRecord {
                node_id: node_id.clone(),
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: postfiat_crypto_provider::bytes_to_hex(&keypair.public_key),
            })
            .collect(),
    };
    write_validator_registry_file(&data_dir.join("validator_registry.json"), &registry)
        .expect("write public registry");
    let local_key = serde_json::json!({
        "validators": [{
            "node_id": "validator-0",
            "algorithm_id": ML_DSA_65_ALGORITHM,
            "public_key_hex": postfiat_crypto_provider::bytes_to_hex(&validators[0].1.public_key),
            "private_key_hex": postfiat_crypto_provider::bytes_to_hex(&validators[0].1.private_key),
        }]
    });
    atomic_write(
        data_dir.join("validator_keys.json"),
        format!("{}\n", serde_json::to_string_pretty(&local_key).expect("keys json")),
    )
    .expect("write isolated local signer");
    let genesis = postfiat_types::Genesis::new_with_validator_count("fastpay-registry-test", 6);
    let store = NodeStore::new(&data_dir);
    store.write_genesis(&genesis).expect("write genesis");
    let mut ledger = postfiat_types::LedgerState::empty();
    ledger.owned_objects.push(postfiat_types::OwnedObject {
        id: "registry-object".to_string(),
        version: 1,
        owner_pubkey_hex: owner_pubkey_hex.clone(),
        value: 100,
        asset: "PFT".to_string(),
    });
    store.write_ledger(&ledger).expect("write ledger");

    let order = postfiat_types::OwnedTransferOrder {
        domain: owned_certificate_domain(&data_dir).expect("owned certificate domain"),
        inputs: vec![postfiat_types::OwnedObjectRef {
            id: "registry-object".to_string(),
            version: 1,
        }],
        outputs: vec![postfiat_types::OwnedOutputSpec {
            owner_pubkey_hex: "recipient".to_string(),
            value: 99,
            asset: "PFT".to_string(),
        }],
        fee: 1,
        nonce: 1,
        memos: Vec::new(),
    };
    let signing_bytes = postfiat_execution::owned_transfer_signing_bytes(&order);
    let owner_signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
        &owner.private_key,
        &signing_bytes,
        postfiat_execution::OWNED_TRANSFER_CONTEXT,
    )
    .expect("owner sign");
    let votes = validators
        .iter()
        .skip(1)
        .map(|(validator_id, keypair)| {
            let signature = postfiat_crypto_provider::ml_dsa_65_sign_with_context(
                &keypair.private_key,
                &signing_bytes,
                postfiat_execution::OWNED_TRANSFER_CONTEXT,
            )
            .expect("validator sign");
            postfiat_types::OwnedTransferVote {
                validator_id: validator_id.clone(),
                signature_hex: postfiat_crypto_provider::bytes_to_hex(&signature),
            }
        })
        .collect();
    let certificate = postfiat_types::OwnedTransferCertificate {
        order,
        owner_pubkey_hex,
        owner_signature_hex: postfiat_crypto_provider::bytes_to_hex(&owner_signature),
        votes,
    };
    let report = owned_apply_report(
        NodeOptions {
            data_dir: data_dir.clone(),
        },
        &serde_json::to_string(&certificate).expect("certificate json"),
    )
    .expect("five-vote certificate applies with isolated local signer");
    assert_eq!(report.validator_count, 6);
    assert_eq!(report.quorum, 5);
    assert_eq!(report.consumed_count, 1);
    assert_eq!(report.created_count, 1);
    let _ = std::fs::remove_dir_all(data_dir);
}
