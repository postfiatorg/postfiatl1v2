use postfiat_crypto_provider::{address_from_public_key, bytes_to_hex, ml_dsa_65_keygen_from_seed};
use postfiat_storage::{
    CommitFinalizedBlock, CurrentStateUpdate, NodeStore, OrderedHistoryCommitment,
    StorageBackendMode, TransactionalStore,
};
use postfiat_types::{
    asset_bridge_hold_key, issued_asset_id, nft_collection_policy_key, nft_id, Account,
    AssetBridgeHoldV1, AssetDefinition, BatchArchiveEntry, BlockCertificate, BlockHeader,
    BlockRecord, BridgeState, ChainTipState, FastPayCertificateV1, FastPayFenceOriginV1,
    FastPayOperationKindV1, FastPayOrderRecoveryV1, FastPayRecoveryCommitteeV1,
    FastPayRecoveryDecisionV1, FastPayVersionFenceV1, Genesis, GovernanceAmendment,
    GovernanceState, LedgerState, NftCollectionPolicyV1, NftDefinition, OwnedObject,
    OwnedObjectRef, OwnedOutputSpec, OwnedTransferCertificateV3, OwnedTransferOrderV3,
    PaymentMemo, ShieldedState, FASTPAY_ORDER_RECOVERY_SCHEMA_V1,
    FASTPAY_VERSION_FENCE_SCHEMA_V1, GOVERNANCE_KIND_FASTPAY_FEE_V1_ACTIVATION_HEIGHT,
    MAX_OWNED_OBJECTS_GLOBAL_V2, MAX_OWNED_OBJECTS_PER_ACCOUNT_SOFT_CAP,
};

use super::super::*;

fn unique_test_dir(label: &str) -> PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("unix epoch")
        .as_nanos();
    let dir = std::env::temp_dir().join(format!(
        "postfiat-train-a-node-{label}-{}-{nanos}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create temp test dir");
    dir
}

fn init_basic_store(label: &str) -> (PathBuf, NodeStore, Genesis) {
    let dir = unique_test_dir(label);
    let genesis = Genesis::new_with_validator_count("postfiat-train-a-node", 1);
    let node_state = NodeState::initialized("val-0");
    let store = NodeStore::new(&dir);
    store.init(&genesis, &node_state).expect("init store");
    (dir, store, genesis)
}

#[test]
fn train_a_paginated_account_nfts_and_issuer_nfts_with_cursor_validation() {
    let (dir, store, genesis) = init_basic_store("nft-pagination");
    let issuer = address_from_public_key(&ml_dsa_65_keygen_from_seed(&[1; 32]).public_key);
    let owner = address_from_public_key(&ml_dsa_65_keygen_from_seed(&[2; 32]).public_key);

    let mut ledger = LedgerState::empty();
    let mut expected_ids = Vec::new();
    for serial in 1_u64..=5 {
        let collection_id = if serial <= 3 { "COL-1" } else { "COL-2" };
        let id = nft_id(&genesis.chain_id, &issuer, collection_id, serial).unwrap();
        expected_ids.push(id.clone());
        ledger.nfts_by_id.insert(
            id.clone(),
            NftDefinition {
                nft_id: id,
                issuer: issuer.clone(),
                collection_id: collection_id.to_string(),
                serial,
                owner: owner.clone(),
                metadata_hash: format!("{serial:096x}"),
                metadata_uri: format!("ipfs://nft-{serial}"),
                flags: 1,
                collection_flags: 0,
                issuer_transfer_fee: 0,
                burned: false,
            },
        );
    }
    expected_ids.sort();
    store.write_ledger(&ledger).expect("write ledger");

    // Malformed cursor MUST reject with invalid_nft_cursor.
    let err = account_nfts(AccountNftsOptions {
        data_dir: dir.clone(),
        account: owner.clone(),
        include_burned: false,
        cursor: Some("not-a-valid-hex-cursor".to_string()),
        limit: Some(2),
    })
    .unwrap_err();
    assert!(
        err.to_string().contains("invalid_nft_cursor"),
        "unexpected error: {err}"
    );

    // Walk all 3 pages of size 2 for account_nfts.
    let mut collected = Vec::new();
    let mut cursor = None;
    let mut pages = 0;
    loop {
        let page = account_nfts(AccountNftsOptions {
            data_dir: dir.clone(),
            account: owner.clone(),
            include_burned: false,
            cursor: cursor.clone(),
            limit: Some(2),
        })
        .expect("account_nfts page");
        pages += 1;
        assert_eq!(page.total_count, Some(5));
        collected.extend(page.nfts.iter().map(|n| n.nft_id.clone()));
        if page.truncated {
            assert!(page.next_cursor.is_some());
            cursor = page.next_cursor;
        } else {
            assert!(page.next_cursor.is_none());
            break;
        }
    }
    assert_eq!(pages, 3);
    assert_eq!(collected, expected_ids);

    // Verify issuer_nfts collection filter + pagination.
    let col1_p1 = issuer_nfts(IssuerNftsOptions {
        data_dir: dir.clone(),
        issuer: issuer.clone(),
        collection_id: Some("COL-1".to_string()),
        include_burned: false,
        cursor: None,
        limit: Some(2),
    })
    .expect("issuer_nfts page 1");
    assert_eq!(col1_p1.total_count, Some(3));
    assert_eq!(col1_p1.nft_count, 2);
    assert!(col1_p1.truncated);
    let col1_p2 = issuer_nfts(IssuerNftsOptions {
        data_dir: dir.clone(),
        issuer,
        collection_id: Some("COL-1".to_string()),
        include_burned: false,
        cursor: col1_p1.next_cursor,
        limit: Some(2),
    })
    .expect("issuer_nfts page 2");
    assert_eq!(col1_p2.total_count, Some(3));
    assert_eq!(col1_p2.nft_count, 1);
    assert!(!col1_p2.truncated);
    assert!(col1_p2.next_cursor.is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn train_a_fastpay_fee_quote_and_bridge_hold_read_rpcs() {
    let (dir, store, genesis) = init_basic_store("fee-quote-and-holds");
    let owner_kp = ml_dsa_65_keygen_from_seed(&[10; 32]);
    let owner_pub = bytes_to_hex(&owner_kp.public_key);
    let issuer = address_from_public_key(&ml_dsa_65_keygen_from_seed(&[11; 32]).public_key);
    let alice = address_from_public_key(&ml_dsa_65_keygen_from_seed(&[12; 32]).public_key);
    let bob = address_from_public_key(&ml_dsa_65_keygen_from_seed(&[13; 32]).public_key);
    let asset_id = issued_asset_id(&genesis.chain_id, &issuer, "GUSD", 1).unwrap();

    let mut ledger = LedgerState::empty();
    for i in 0..3 {
        ledger.owned_objects.push(OwnedObject {
            id: format!("obj-{i}"),
            owner_pubkey_hex: owner_pub.clone(),
            asset: "PFT".to_string(),
            value: 100,
            version: 1,
        });
    }
    let hk_alice = asset_bridge_hold_key(&asset_id, &alice);
    let hk_bob = asset_bridge_hold_key(&asset_id, &bob);
    ledger.asset_bridge_holds.insert(
        hk_alice.clone(),
        AssetBridgeHoldV1 {
            hold_key: hk_alice.clone(),
            asset_id: asset_id.clone(),
            issuer: issuer.clone(),
            recipient: alice.clone(),
            amount: 400,
            credit_count: 2,
            created_at_height: 10,
            last_updated_height: 12,
            last_reason_code: "asset_gate_recipient_ineligible".to_string(),
        },
    );
    ledger.asset_bridge_holds.insert(
        hk_bob.clone(),
        AssetBridgeHoldV1 {
            hold_key: hk_bob.clone(),
            asset_id: asset_id.clone(),
            issuer: issuer.clone(),
            recipient: bob.clone(),
            amount: 600,
            credit_count: 1,
            created_at_height: 11,
            last_updated_height: 11,
            last_reason_code: "asset_gate_recipient_ineligible".to_string(),
        },
    );
    store.write_ledger(&ledger).expect("write ledger");
    store
        .write_chain_tip(&ChainTipState {
            schema: "postfiat-chain-tip-v1".to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            height: 20,
            block_hash: "aa".repeat(48),
            state_root: "bb".repeat(48),
            ordered_batch_count: 20,
            receipt_count: 0,
            history_base_height: 0,
        })
        .expect("write tip");

    // Pre-activation: fee_v1_active = false, minimum_fee = 0.
    let pre_quote = fastpay_fee_quote(FastPayFeeQuoteOptions {
        data_dir: dir.clone(),
        operation: "transfer".to_string(),
        owner_pubkey_hex: Some(owner_pub.clone()),
        inputs_count: 1,
        outputs_count: 2,
        memo_bytes: 180,
        estimated_weight_bytes: Some(620),
    })
    .expect("pre-activation fastpay_fee_quote");
    assert_eq!(pre_quote.schema, "postfiat-fastpay-fee-quote-v1");
    assert!(!pre_quote.fee_v1_active);
    assert_eq!(pre_quote.minimum_fee, 0);
    assert_eq!(pre_quote.owner_current_object_count, 3);
    assert_eq!(
        pre_quote.owner_soft_cap,
        MAX_OWNED_OBJECTS_PER_ACCOUNT_SOFT_CAP as u64
    );
    assert_eq!(
        pre_quote.global_hard_cap,
        MAX_OWNED_OBJECTS_GLOBAL_V2 as u64
    );

    // Activate fastpay_fee_v1 at height 15 (tip is 20 -> active!).
    let mut governance = store.read_governance().expect("read governance");
    governance.amendments.push(GovernanceAmendment {
        amendment_id: "amd-fp-fee-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        instance_id: "inst-1".to_string(),
        proposal_id: "prop-1".to_string(),
        certificate_id: "cert-1".to_string(),
        proposer: "val-0".to_string(),
        validators: vec!["val-0".to_string()],
        quorum: 1,
        support: vec!["val-0".to_string()],
        votes: Vec::new(),
        signed_authorizations: Vec::new(),
        kind: GOVERNANCE_KIND_FASTPAY_FEE_V1_ACTIVATION_HEIGHT.to_string(),
        value: 15,
        activation_height: 15,
        veto_until_height: 0,
        paused: false,
    });
    store.write_governance(&governance).expect("write governance");

    let active_quote = fastpay_fee_quote(FastPayFeeQuoteOptions {
        data_dir: dir.clone(),
        operation: "transfer".to_string(),
        owner_pubkey_hex: Some(owner_pub),
        inputs_count: 1,
        outputs_count: 2,
        memo_bytes: 180,
        estimated_weight_bytes: Some(620),
    })
    .expect("active fastpay_fee_quote");
    assert!(active_quote.fee_v1_active);
    assert_eq!(active_quote.base_fee, 25);
    assert_eq!(active_quote.byte_fee, 2);
    assert_eq!(active_quote.new_object_fee, 1_000);
    assert_eq!(active_quote.input_fee, 1);
    assert_eq!(active_quote.minimum_fee, 1_028);

    // account_bridge_holds for Alice.
    let alice_holds = account_bridge_holds(AccountBridgeHoldsOptions {
        data_dir: dir.clone(),
        recipient: alice.clone(),
        asset_id: None,
        limit: Some(10),
    })
    .expect("account_bridge_holds");
    assert_eq!(alice_holds.schema, "postfiat-account-bridge-holds-v1");
    assert_eq!(alice_holds.hold_count, 1);
    assert_eq!(alice_holds.holds[0].amount, 400);
    assert_eq!(alice_holds.holds[0].credit_count, 2);

    // asset_bridge_holds paginated with limit = 1.
    let asset_p1 = asset_bridge_holds(AssetBridgeHoldsOptions {
        data_dir: dir.clone(),
        asset_id: asset_id.clone(),
        cursor: None,
        limit: Some(1),
    })
    .expect("asset_bridge_holds p1");
    assert_eq!(asset_p1.schema, "postfiat-asset-bridge-holds-v1");
    assert_eq!(asset_p1.total_held_atoms, 1_000);
    assert_eq!(asset_p1.hold_count, 1);
    assert!(asset_p1.truncated);
    let asset_p2 = asset_bridge_holds(AssetBridgeHoldsOptions {
        data_dir: dir.clone(),
        asset_id,
        cursor: asset_p1.next_cursor,
        limit: Some(1),
    })
    .expect("asset_bridge_holds p2");
    assert_eq!(asset_p2.total_held_atoms, 1_000);
    assert_eq!(asset_p2.hold_count, 1);
    assert!(!asset_p2.truncated);
    assert!(asset_p2.next_cursor.is_none());

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn train_a_account_tx_v6_fastpay_rows_and_tx_order_hash_lookup() {
    let dir = unique_test_dir("fastpay-account-tx-v6");
    let genesis = Genesis::new_with_validator_count("postfiat-train-a-fp", 1);
    let node_state = NodeState::initialized("val-0");
    let store = NodeStore::new(&dir);
    store.init(&genesis, &node_state).expect("init store");

    let sender_kp = ml_dsa_65_keygen_from_seed(&[21; 32]);
    let recipient_kp = ml_dsa_65_keygen_from_seed(&[22; 32]);
    let val_kp = ml_dsa_65_keygen_from_seed(&[23; 32]);
    let sender_addr = address_from_public_key(&sender_kp.public_key);
    let recipient_addr = address_from_public_key(&recipient_kp.public_key);
    let sender_pub = bytes_to_hex(&sender_kp.public_key);
    let recipient_pub = bytes_to_hex(&recipient_kp.public_key);

    let committee = FastPayRecoveryCommitteeV1::from_public_keys(
        genesis.chain_id.clone(),
        genesis_hash(&genesis),
        1,
        1,
        1,
        100,
        vec![("val-0".to_string(), bytes_to_hex(&val_kp.public_key))],
    )
    .expect("committee");
    let domain = committee.certificate_domain();
    let memo = PaymentMemo {
        memo_type: bytes_to_hex(b"pf.ptr"),
        memo_format: bytes_to_hex(b"bin"),
        memo_data: bytes_to_hex(b"hello-fastpay-v6"),
    };
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
            id: "obj-fp-1".to_string(),
            version: 1,
        }],
        outputs: vec![
            OwnedOutputSpec {
                owner_pubkey_hex: recipient_pub.clone(),
                asset: "PFT".to_string(),
                value: 2_500,
            },
            OwnedOutputSpec {
                owner_pubkey_hex: sender_pub.clone(),
                asset: "PFT".to_string(),
                value: 6_472,
            },
        ],
        fee: 1_028,
        nonce: 7,
        memos: vec![memo],
    };
    order.recovery.lock_id = postfiat_types::fastpay_transfer_lock_id_v1(&order);
    let order_hash = postfiat_execution::fastpay_transfer_order_digest_v3(&order);
    let cert_digest = "cc".repeat(48);
    let cert = OwnedTransferCertificateV3 {
        order: order.clone(),
        owner_pubkey_hex: sender_pub.clone(),
        owner_signature_hex: "dd".repeat(64),
        votes: Vec::new(),
    };
    let fence = FastPayVersionFenceV1 {
        schema: FASTPAY_VERSION_FENCE_SCHEMA_V1.to_string(),
        operation: FastPayOperationKindV1::Transfer,
        origin: FastPayFenceOriginV1::Consensusless,
        committee_epoch: 1,
        registry_root: domain.registry_id.clone(),
        lock_id: order.recovery.lock_id.clone(),
        inputs: order.inputs.clone(),
        decision: FastPayRecoveryDecisionV1::Confirmed {
            order_digest: order_hash.clone(),
            certificate_digest: cert_digest.clone(),
        },
        certificate: Some(FastPayCertificateV1::Transfer(cert)),
        decided_at_height: 2,
        next_versions: vec![OwnedObjectRef {
            id: "obj-fp-1".to_string(),
            version: 2,
        }],
    };

    // Commit block 1 (scheduling transactional storage activation at height 2)
    // and block 2 (carrying the FastPay effect) so BOTH `fastpay_order_v1\0`
    // point lookup AND `account_tx` v6 index are exercised!
    let tx_store = TransactionalStore::open(&dir).expect("open transactional store");
    let genesis_tip = ChainTipState {
        schema: "postfiat-chain-tip-v1".to_string(),
        chain_id: genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&genesis),
        protocol_version: genesis.protocol_version,
        height: 0,
        block_hash: "genesis".to_string(),
        state_root: "00".repeat(48),
        ordered_batch_count: 0,
        receipt_count: 0,
        history_base_height: 0,
    };
    let genesis_comm = OrderedHistoryCommitment::genesis(
        &genesis.chain_id,
        &genesis_hash(&genesis),
        genesis.protocol_version,
    )
    .expect("genesis commitment");
    let gov = GovernanceState::new(1);
    let mut ledger = LedgerState::empty();
    ledger.fastpay_version_fences.push(fence.clone());
    let shielded = ShieldedState::empty();
    let bridge = BridgeState::empty();
    tx_store
        .initialize(
            &genesis_tip,
            &genesis_comm,
            CurrentStateUpdate {
                ledger: Some(&LedgerState::empty()),
                governance: Some(&gov),
                shielded: Some(&shielded),
                bridge: Some(&bridge),
                node_state: Some(&node_state),
                additional: &[],
            },
        )
        .expect("init tx store");

    let empty_batch_1 = TransactionBatch {
        batch_id: "batch-fp-1".to_string(),
        transactions: Vec::new(),
        payments_v2: Vec::new(),
        asset_transactions: Vec::new(),
        atomic_swap_transactions: Vec::new(),
        fastlane_primary_transactions: Vec::new(),
        escrow_transactions: Vec::new(),
        nft_transactions: Vec::new(),
        offer_transactions: Vec::new(),
    };
    let archive_1 = BatchArchiveEntry {
        batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
        batch_id: "batch-fp-1".to_string(),
        payload_hash: "e1".repeat(48),
        payload_json: serde_json::to_string(&empty_batch_1).unwrap(),
    };
    let block1 = BlockRecord {
        header: BlockHeader {
            height: 1,
            view: 1,
            parent_hash: "genesis".to_string(),
            proposer: "val-0".to_string(),
            batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
            batch_id: "batch-fp-1".to_string(),
            state_root: "11".repeat(48),
            bridge_exit_root: None,
            pftl_uniswap_receipt_root: None,
            external_lane_export_root: None,
            receipt_count: 0,
            certificate_id: "cert-1".to_string(),
            certificate: BlockCertificate {
                registry_root: domain.registry_id.clone(),
                validators: vec!["val-0".to_string()],
                quorum: 1,
                votes: Vec::new(),
            },
            consensus_v2_commit: None,
            block_hash: "21".repeat(48),
        },
        receipt_ids: Vec::new(),
        fastpay_pre_state_effects: Vec::new(),
    };
    let tip1 = ChainTipState {
        height: 1,
        block_hash: block1.header.block_hash.clone(),
        state_root: block1.header.state_root.clone(),
        ordered_batch_count: 1,
        receipt_count: 0,
        ..genesis_tip.clone()
    };
    let comm1 = genesis_comm.append("batch-fp-1").expect("append 1");
    tx_store
        .commit_finalized_block(CommitFinalizedBlock {
            expected_tip: &genesis_tip,
            new_tip: &tip1,
            block: &block1,
            receipts: &[],
            archive_entry: &archive_1,
            batch_id: "batch-fp-1",
            ordered_history: &comm1,
            current_state: CurrentStateUpdate::default(),
            scheduled_activation_height: Some(2),
            allow_legacy_receipt_id_mismatch: false,
        })
        .expect("commit block 1");

    let empty_batch_2 = TransactionBatch {
        batch_id: "batch-fp-2".to_string(),
        ..empty_batch_1
    };
    let archive_2 = BatchArchiveEntry {
        batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
        batch_id: "batch-fp-2".to_string(),
        payload_hash: "e2".repeat(48),
        payload_json: serde_json::to_string(&empty_batch_2).unwrap(),
    };
    let block2 = BlockRecord {
        header: BlockHeader {
            height: 2,
            view: 2,
            parent_hash: block1.header.block_hash.clone(),
            proposer: "val-0".to_string(),
            batch_kind: BATCH_KIND_TRANSPARENT.to_string(),
            batch_id: "batch-fp-2".to_string(),
            state_root: "12".repeat(48),
            bridge_exit_root: None,
            pftl_uniswap_receipt_root: None,
            external_lane_export_root: None,
            receipt_count: 0,
            certificate_id: "cert-2".to_string(),
            certificate: BlockCertificate {
                registry_root: domain.registry_id.clone(),
                validators: vec!["val-0".to_string()],
                quorum: 1,
                votes: Vec::new(),
            },
            consensus_v2_commit: None,
            block_hash: "22".repeat(48),
        },
        receipt_ids: Vec::new(),
        fastpay_pre_state_effects: vec![fence.clone()],
    };
    let tip2 = ChainTipState {
        height: 2,
        block_hash: block2.header.block_hash.clone(),
        state_root: block2.header.state_root.clone(),
        ordered_batch_count: 2,
        receipt_count: 0,
        ..genesis_tip.clone()
    };
    let comm2 = comm1.append("batch-fp-2").expect("append 2");
    tx_store
        .commit_finalized_block(CommitFinalizedBlock {
            expected_tip: &tip1,
            new_tip: &tip2,
            block: &block2,
            receipts: &[],
            archive_entry: &archive_2,
            batch_id: "batch-fp-2",
            ordered_history: &comm2,
            current_state: CurrentStateUpdate {
                ledger: Some(&ledger),
                ..CurrentStateUpdate::default()
            },
            scheduled_activation_height: Some(2),
            allow_legacy_receipt_id_mismatch: false,
        })
        .expect("commit block 2 with fastpay effect");
    // Activate transactional backend mode marker for NodeStore reads.
    store
        .write_storage_backend_mode(StorageBackendMode::Transactional)
        .expect("set transactional backend mode");
    drop(tx_store);

    // 1) Rebuild account_tx disk index (v6) and verify schema version!
    let build_report =
        rebuild_account_tx_index(AccountTxIndexOptions { data_dir: dir.clone() })
            .expect("rebuild account_tx v6 index");
    assert_eq!(build_report.indexed_row_count, 2);
    let idx_status = account_tx_index_status(AccountTxIndexOptions { data_dir: dir.clone() })
        .expect("account_tx_index_status");
    assert_eq!(idx_status.indexed_row_count, 2);

    // 2) Recipient sees 1 row (fastpay_transfer, amount 2500, fee 1028, order_hash, memos!).
    let recipient_history = account_tx(AccountTxQueryOptions {
        data_dir: dir.clone(),
        address: recipient_addr.clone(),
        from_height: Some(2),
        to_height: Some(2),
        limit: Some(10),
    })
    .expect("recipient account_tx");
    assert_eq!(recipient_history.row_count, 1);
    let row = &recipient_history.rows[0];
    assert_eq!(row.tx_id, order_hash);
    assert_eq!(row.order_hash.as_deref(), Some(order_hash.as_str()));
    assert_eq!(row.certificate_digest.as_deref(), Some(cert_digest.as_str()));
    assert_eq!(
        row.fastpay_lock_id.as_deref(),
        Some(order.recovery.lock_id.as_str())
    );
    assert_eq!(row.batch_kind, "fastpay");
    assert_eq!(row.transaction_kind, "owned_transfer_v3");
    assert_eq!(row.tx_role.as_deref(), Some("fastpay_transfer"));
    assert_eq!(row.from_address, sender_addr);
    assert_eq!(row.to_address, recipient_addr);
    assert_eq!(row.amount, 2_500);
    assert_eq!(row.fee, 1_028);
    assert_eq!(row.fill_index, Some(0));
    assert_eq!(
        row.memos.as_ref().unwrap()[0].memo_data_text.as_deref(),
        Some("hello-fastpay-v6")
    );

    // 3) Sender sees both legs: transfer leg (fill_index 0) and change self-transfer leg (fill_index 1).
    let sender_history = account_tx(AccountTxQueryOptions {
        data_dir: dir.clone(),
        address: sender_addr,
        from_height: Some(2),
        to_height: Some(2),
        limit: Some(10),
    })
    .expect("sender account_tx");
    assert_eq!(sender_history.row_count, 2);
    assert!(sender_history
        .rows
        .iter()
        .any(|r| r.tx_role.as_deref() == Some("fastpay_self_transfer") && r.fill_index == Some(1) && r.fee == 0));

    // 4) `tx` lookup by canonical `order_hash` resolves via `fastpay_order_v1\0` index!
    let finality = tx_finality(TxFinalityQueryOptions {
        data_dir: dir.clone(),
        tx_id: order_hash.clone(),
        audit_block_log: false,
    })
    .expect("tx lookup by order_hash");
    assert!(finality.confirmed);
    assert_eq!(finality.verification_mode, "fastpay_version_fence_v1");
    assert_eq!(finality.tx_id, order_hash);
    assert_eq!(finality.receipt.tx_id, order_hash);
    assert!(finality.receipt.accepted);
    assert_eq!(finality.receipt.fee_charged, 1_028);
    assert_eq!(finality.transaction_rows.len(), 2);

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn train_a_state_commitment_commits_new_maps_only_when_non_empty() {
    let genesis = Genesis::new_with_validator_count("postfiat-train-a-commit", 1);
    let governance = GovernanceState::new(1);
    let shielded = ShieldedState::empty();
    let bridge = BridgeState::empty();
    let issuer = address_from_public_key(&ml_dsa_65_keygen_from_seed(&[30; 32]).public_key);
    let recipient = address_from_public_key(&ml_dsa_65_keygen_from_seed(&[31; 32]).public_key);
    let asset_id = issued_asset_id(&genesis.chain_id, &issuer, "CMT", 1).unwrap();

    let mut base_ledger = LedgerState::new(vec![Account {
        address: issuer.clone(),
        balance: 1_000_000,
        sequence: 0,
        public_key_hex: None,
        flags: 0,
    }]);
    base_ledger.asset_definitions.push(AssetDefinition {
        asset_id: asset_id.clone(),
        asset_family_id: asset_id.clone(),
        source_series_id: asset_id.clone(),
        source_bucket_id: String::new(),
        issuer: issuer.clone(),
        code: "CMT".to_string(),
        version: 1,
        precision: 6,
        display_name: "Commit Test".to_string(),
        max_supply: Some(1_000_000),
        requires_authorization: true,
        freeze_enabled: true,
        clawback_enabled: true,
        access_policy: None,
    });

    let base_root = replicated_state_root(
        &genesis,
        &governance,
        &base_ledger,
        &[],
        &shielded,
        &bridge,
    )
    .expect("base root");

    // Adding a bridge hold MUST change the state root.
    let mut with_hold = base_ledger.clone();
    let hk = asset_bridge_hold_key(&asset_id, &recipient);
    with_hold.asset_bridge_holds.insert(
        hk.clone(),
        AssetBridgeHoldV1 {
            hold_key: hk,
            asset_id: asset_id.clone(),
            issuer: issuer.clone(),
            recipient: recipient.clone(),
            amount: 500,
            credit_count: 1,
            created_at_height: 10,
            last_updated_height: 10,
            last_reason_code: "asset_gate_recipient_ineligible".to_string(),
        },
    );
    let hold_root =
        replicated_state_root(&genesis, &governance, &with_hold, &[], &shielded, &bridge)
            .expect("hold root");
    assert_ne!(base_root, hold_root);

    // Adding a collection policy / tombstone / revoked permit MUST each change the state root.
    let mut with_policy = base_ledger.clone();
    let pk = nft_collection_policy_key(&issuer, "COL-1");
    with_policy.nft_collection_policies.insert(
        pk,
        NftCollectionPolicyV1 {
            issuer: issuer.clone(),
            collection_id: "COL-1".to_string(),
            collection_flags: 1,
            first_minted_height: 10,
        },
    );
    assert_ne!(
        base_root,
        replicated_state_root(&genesis, &governance, &with_policy, &[], &shielded, &bridge)
            .expect("policy root")
    );

    let mut with_tombstone = base_ledger.clone();
    with_tombstone.used_nft_serials.insert("aa".repeat(48), 10);
    assert_ne!(
        base_root,
        replicated_state_root(&genesis, &governance, &with_tombstone, &[], &shielded, &bridge)
            .expect("tombstone root")
    );

    let mut with_revoked = base_ledger.clone();
    with_revoked
        .revoked_fast_lane_holder_permits
        .insert("bb".repeat(48), 10);
    assert_ne!(
        base_root,
        replicated_state_root(&genesis, &governance, &with_revoked, &[], &shielded, &bridge)
            .expect("revoked permit root")
    );
}
