use crate::*;
use postfiat_types::{VaultBridgeDepositRecord, VaultBridgeRouteProfileRecordV1};

const CONTEXT: &str = include_str!(
    "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/block-1256-public-context.json"
);
const BATCH: &str = include_str!(
    "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/block-1256-governance.json"
);

fn fixture() -> (Genesis, GovernanceState, LedgerState, GovernanceActionBatch) {
    let raw: serde_json::Value = serde_json::from_str(CONTEXT).unwrap();
    let mut ledger = LedgerState::new(vec![]);
    ledger
        .asset_definitions
        .push(serde_json::from_value(raw["asset_definition"].clone()).unwrap());
    ledger
        .nav_assets
        .push(serde_json::from_value(raw["nav_asset"].clone()).unwrap());
    ledger
        .nav_proof_profiles
        .push(serde_json::from_value(raw["nav_profile"].clone()).unwrap());
    ledger.beacon_finality_states =
        serde_json::from_value(raw["beacon_finality_states"].clone()).unwrap();
    (
        serde_json::from_value(raw["genesis"].clone()).unwrap(),
        serde_json::from_value(raw["governance"].clone()).unwrap(),
        ledger,
        serde_json::from_str(BATCH).unwrap(),
    )
}

fn current(governance: &GovernanceState) -> &VaultBridgeRouteProfileRecordV1 {
    governance
        .vault_bridge_route_profiles
        .iter()
        .find(|r| r.profile.route_epoch == 11)
        .unwrap()
}

#[test]
fn b23_release4_real_1256_replay_preserves_accepted_receipt_and_stored_bytes() {
    let (_, governance, ledger, batch) = fixture();
    let activation = &batch.vault_bridge_route_profile_activations[0];
    let record = current(&governance);
    assert_eq!(
        record,
        &VaultBridgeRouteProfileRecordV1::new_for_replay(activation, 1256).unwrap()
    );
    assert!(!serde_json::to_string(record)
        .unwrap()
        .contains("beacon_bootstrap_hash"));
    crate::consensus_artifacts::verify_archived_governance_action_batch_id(&fixture().0, &batch)
        .unwrap();
    crate::consensus_artifacts::verify_governance_amendment_evidence(
        &fixture().0,
        &activation.amendment,
    )
    .unwrap();
    for archived in [true, false] {
        let mut replay = governance.clone();
        replay
            .vault_bridge_route_profiles
            .retain(|r| r.profile.route_epoch != 11);
        replay.amendment_activation_records.clear();
        replay
            .amendments
            .retain(|a| a.amendment_id != record.governance_amendment_id);
        let mut state = ledger.clone();
        state.beacon_finality_states.clear();
        let receipts = if archived {
            crate::execution_actions::execute_archived_governance_batch(
                &mut replay,
                Some(&mut state),
                &batch,
                1256,
            )
        } else {
            crate::execution_actions::execute_governance_batch(
                &mut replay,
                Some(&mut state),
                &batch,
                1256,
            )
        };
        let expected: Receipt = serde_json::from_str(include_str!(
            "../../../programs/pfusdc-eth-mainnet-ingress/tests/fixtures/block-1256-receipt.json"
        ))
        .unwrap();
        assert_eq!(
            serde_json::to_vec(&receipts).unwrap(),
            serde_json::to_vec(&vec![expected]).unwrap()
        );
        assert!(
            serde_json::to_vec(&replay).unwrap() == serde_json::to_vec(&governance).unwrap(),
            "governance bytes changed"
        );
        assert_eq!(
            serde_json::to_vec(&state).unwrap(),
            serde_json::to_vec(&ledger).unwrap()
        );
    }
}

#[test]
fn b23_release4_real_stored_route_read_succeeds_without_reactivation() {
    let (genesis, governance, ledger, _) = fixture();
    let record = current(&governance);
    let dir = std::env::temp_dir().join(format!("b23-ingress-r4-read-{}", std::process::id()));
    let store = NodeStore::new(&dir);
    store.write_genesis(&genesis).unwrap();
    store.write_governance(&governance).unwrap();
    store.write_ledger(&ledger).unwrap();
    store
        .write_chain_tip(&ChainTipState {
            schema: CHAIN_TIP_SCHEMA.into(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            height: 1256,
            block_hash: "aa".repeat(48),
            state_root: "bb".repeat(48),
            ordered_batch_count: 1256,
            receipt_count: 1,
            history_base_height: 0,
        })
        .unwrap();
    let result = crate::market_bridge::vault_bridge_route(VaultBridgeRouteOptions {
        data_dir: dir.clone(),
        asset_id: record.profile.asset_id.clone(),
    });
    std::fs::remove_dir_all(&dir).unwrap();
    let report = result.expect("real accepted beacon route must be readable");
    assert_eq!(report.profile_hash, record.profile_hash);
    assert_eq!(
        report.beacon_finality_anchor,
        Some(ledger.beacon_finality_states[0].clone())
    );
}

fn lifecycle_rejection(claim: bool) -> Receipt {
    let (genesis, governance, mut ledger, _) = fixture();
    let record = current(&governance);
    // A synthetic already-proposed deposit isolates the lifecycle authority gate.
    // This is not a new Ethereum deposit or a proof qualification.
    let recipient = "pf1111111111111111111111111111111111111111";
    let mut evidence = postfiat_types::VaultBridgeDepositEvidence {
        source_chain_id: 1,
        vault_address: record.profile.vault_address.clone(),
        token_address: record.profile.token_address.clone(),
        depositor: "0x1111111111111111111111111111111111111111".into(),
        pftl_recipient: recipient.into(),
        pftl_recipient_hash: postfiat_types::vault_bridge_pftl_recipient_hash(recipient).unwrap(),
        amount_atoms: 1_000_000,
        nonce: "12".repeat(32),
        route_binding: vault_bridge_route_binding(&record.profile_hash, 11).unwrap(),
        deposit_id: String::new(),
        block_hash: "34".repeat(32),
        tx_hash: "56".repeat(32),
        log_index: 0,
    };
    evidence.deposit_id = postfiat_types::vault_bridge_deposit_id(&evidence).unwrap();
    let root = postfiat_types::vault_bridge_deposit_evidence_root(&evidence).unwrap();
    ledger.vault_bridge_deposits.push(
        VaultBridgeDepositRecord::new(
            record.profile.asset_id.clone(),
            root.clone(),
            evidence,
            record.profile_hash.clone(),
            "",
            "",
            "",
            recipient,
            1257,
            2000,
        )
        .unwrap(),
    );
    let operation = if claim {
        AssetTransactionOperation::VaultBridgeDepositClaim(
            postfiat_types::VaultBridgeDepositClaimOperation {
                claimer: recipient.into(),
                asset_id: record.profile.asset_id.clone(),
                evidence_root: root,
                policy_hash: record.profile_hash.clone(),
                route_epoch: 11,
                recipient: recipient.into(),
                amount_atoms: 1_000_000,
            },
        )
    } else {
        AssetTransactionOperation::VaultBridgeDepositFinalize(
            postfiat_types::VaultBridgeDepositFinalizeOperation {
                finalizer: recipient.into(),
                asset_id: record.profile.asset_id.clone(),
                evidence_root: root,
            },
        )
    };
    let transaction = SignedAssetTransaction {
        unsigned: UnsignedAssetTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: postfiat_types::ADDRESS_NAMESPACE.into(),
            transaction_kind: if claim {
                postfiat_types::VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND
            } else {
                postfiat_types::VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND
            }
            .into(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.into(),
            source: recipient.into(),
            fee: 1,
            sequence: 1,
            operation,
        },
        algorithm_id: ML_DSA_65_ALGORITHM.into(),
        public_key_hex: "00".repeat(32),
        signature_hex: "00".repeat(64),
    };
    crate::execution_actions::execute_transparent_batch(
        &genesis,
        &governance,
        &mut ledger,
        &TransactionBatch::new_with_asset_transactions(
            "r4-authority-test",
            vec![],
            vec![],
            vec![transaction],
        ),
        1258,
        AssetExecutionCompatibility::strict(),
    )
    .remove(0)
}

#[test]
fn b23_release4_real_stored_finalize_passes_route_authority_gate() {
    let receipt = lifecycle_rejection(false);
    assert_ne!(
        receipt.code, "vault_bridge_route_authority_mismatch",
        "{receipt:?}"
    );
}

#[test]
fn b23_release4_real_stored_claim_passes_route_authority_gate() {
    let receipt = lifecycle_rejection(true);
    assert_ne!(
        receipt.code, "vault_bridge_route_authority_mismatch",
        "{receipt:?}"
    );
}

#[test]
fn b23_release4_forged_suffix_and_changed_domain_are_rejected() {
    let (_, governance, _, _) = fixture();
    let record = current(&governance).clone();
    for mutation in 0..4 {
        let mut tampered = governance.clone();
        let amendment = tampered
            .amendments
            .iter_mut()
            .find(|a| a.amendment_id == record.governance_amendment_id)
            .unwrap();
        match mutation {
            0 => {
                amendment.kind = format!(
                    "{}:beacon-bootstrap-v2:{}",
                    postfiat_types::vault_bridge_route_amendment_kind(&record.profile).unwrap(),
                    "ab".repeat(48)
                )
            }
            1 => amendment.kind.push('0'),
            2 => amendment.chain_id.push_str("-wrong"),
            _ => amendment.kind = amendment.kind.to_uppercase(),
        }
        assert!(tampered
            .active_vault_bridge_route_policy_hash(&record.profile.asset_id, 1257)
            .is_err());
        assert!(tampered
            .authorized_vault_bridge_route_profile(&record.profile.asset_id, &record.profile_hash)
            .is_err());
    }
}

#[test]
fn b23_release4_legacy_no_bootstrap_authorization_is_unchanged() {
    let (_, mut governance, _, _) = fixture();
    governance
        .vault_bridge_route_profiles
        .retain(|r| r.profile.route_epoch == 10);
    let record = governance.vault_bridge_route_profiles[0].clone();
    assert_eq!(
        governance
            .active_vault_bridge_route_policy_hash(&record.profile.asset_id, 1257)
            .unwrap()
            .0,
        record.profile_hash
    );
    assert_eq!(
        governance
            .authorized_vault_bridge_route_profile(&record.profile.asset_id, &record.profile_hash)
            .unwrap(),
        &record
    );
    let amendment = governance
        .amendments
        .iter_mut()
        .find(|a| a.amendment_id == record.governance_amendment_id)
        .unwrap();
    amendment.kind.push_str(":beacon-bootstrap-v2:");
    amendment.kind.push_str(&"ab".repeat(48));
    assert!(governance
        .authorized_vault_bridge_route_profile(&record.profile.asset_id, &record.profile_hash)
        .is_err());
}

#[test]
fn b23_release4_signed_finalize_and_claim_credit_epoch11_child_from_postproof_fixture() {
    // Start at the consensus post-proof boundary. No synthetic proof is admitted
    // by this test. The only available historical GPU proof binds epoch 10 and
    // cannot be reused for epoch 11, even with a new finality witness.
    let (genesis, governance, mut ledger, _) = fixture();
    let route = current(&governance);
    let key = ml_dsa_65_keygen().unwrap();
    let recipient = address_from_public_key(&key.public_key);
    ledger.accounts.push(Account::new(
        &recipient,
        10_000_000_000,
        Some(bytes_to_hex(&key.public_key)),
    ));
    let mut evidence = postfiat_types::VaultBridgeDepositEvidence {
        source_chain_id: 1,
        vault_address: route.profile.vault_address.clone(),
        token_address: route.profile.token_address.clone(),
        depositor: "0x1111111111111111111111111111111111111111".into(),
        pftl_recipient: recipient.clone(),
        pftl_recipient_hash: postfiat_types::vault_bridge_pftl_recipient_hash(&recipient).unwrap(),
        amount_atoms: 1_000_000,
        nonce: "12".repeat(32),
        route_binding: vault_bridge_route_binding(&route.profile_hash, 11).unwrap(),
        deposit_id: String::new(),
        block_hash: "34".repeat(32),
        tx_hash: "56".repeat(32),
        log_index: 0,
    };
    evidence.deposit_id = postfiat_types::vault_bridge_deposit_id(&evidence).unwrap();
    let root = postfiat_types::vault_bridge_deposit_evidence_root(&evidence).unwrap();
    ledger.vault_bridge_deposits.push(
        VaultBridgeDepositRecord::new_with_source_nullifier(
            &route.profile.asset_id,
            &root,
            evidence,
            &route.profile_hash,
            postfiat_types::SOURCE_PROOF_KIND_SP1_ETHEREUM_FINALITY_V1,
            "ab".repeat(48),
            "cd".repeat(48),
            "ef".repeat(32),
            &recipient,
            1257,
            2000,
        )
        .unwrap(),
    );
    let child = postfiat_types::pfusdc_source_series_id(
        &genesis.chain_id,
        &route.profile.asset_id,
        1,
        &route.profile.vault_address,
        &route.profile.token_address,
        11,
        &route.profile_hash,
    )
    .unwrap();
    let operations = [
        (
            postfiat_types::VAULT_BRIDGE_DEPOSIT_FINALIZE_TRANSACTION_KIND,
            AssetTransactionOperation::VaultBridgeDepositFinalize(
                postfiat_types::VaultBridgeDepositFinalizeOperation {
                    finalizer: recipient.clone(),
                    asset_id: route.profile.asset_id.clone(),
                    evidence_root: root.clone(),
                },
            ),
        ),
        (
            postfiat_types::VAULT_BRIDGE_DEPOSIT_CLAIM_TRANSACTION_KIND,
            AssetTransactionOperation::VaultBridgeDepositClaim(
                postfiat_types::VaultBridgeDepositClaimOperation {
                    claimer: recipient.clone(),
                    asset_id: route.profile.asset_id.clone(),
                    evidence_root: root.clone(),
                    policy_hash: route.profile_hash.clone(),
                    route_epoch: 11,
                    recipient: recipient.clone(),
                    amount_atoms: 1_000_000,
                },
            ),
        ),
    ];
    for (index, (kind, operation)) in operations.into_iter().enumerate() {
        let unsigned = UnsignedAssetTransaction {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            address_namespace: postfiat_types::ADDRESS_NAMESPACE.into(),
            transaction_kind: kind.into(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.into(),
            source: recipient.clone(),
            fee: 100_000,
            sequence: index as u64 + 1,
            operation,
        };
        let signature = ml_dsa_65_sign(&key.private_key, &unsigned.signing_bytes()).unwrap();
        let signed = SignedAssetTransaction {
            unsigned,
            algorithm_id: ML_DSA_65_ALGORITHM.into(),
            public_key_hex: bytes_to_hex(&key.public_key),
            signature_hex: bytes_to_hex(&signature),
        };
        let compatibility =
            crate::execution_actions::asset_execution_compatibility_with_chain_activation(
                AssetExecutionCompatibility::strict(),
                &genesis,
                &governance,
            );
        let receipts = crate::execution_actions::execute_transparent_batch(
            &genesis,
            &governance,
            &mut ledger,
            &TransactionBatch::new_with_asset_transactions(
                format!("r4-lifecycle-{index}"),
                vec![],
                vec![],
                vec![signed],
            ),
            1258 + index as u64,
            compatibility,
        );
        assert!(receipts[0].accepted, "{kind}: {:?}", receipts[0]);
    }
    assert_eq!(
        ledger
            .trustlines
            .iter()
            .find(|l| l.asset_id == child && l.account == recipient)
            .unwrap()
            .balance,
        1_000_000
    );
    assert!(ledger
        .trustlines
        .iter()
        .all(|l| l.asset_id != route.profile.asset_id || l.balance == 0));
    assert_eq!(
        ledger.vault_bridge_deposits[0].status,
        postfiat_types::VAULT_BRIDGE_DEPOSIT_STATUS_FINALIZED
    );
    assert_eq!(
        ledger.asset_definition(&child).unwrap().asset_family_id,
        route.profile.asset_id
    );
}

#[test]
#[ignore = "read-only public state extraction supplied by B23_R4_STATE_COPY"]
fn b23_release4_public_state_copy_root_and_route_readback() {
    let root = PathBuf::from(std::env::var("B23_R4_STATE_COPY").unwrap());
    let read = |name: &str| std::fs::read(root.join(name)).unwrap();
    let genesis = fixture().0;
    let governance: GovernanceState = serde_json::from_slice(&read("governance.json")).unwrap();
    let ledger: LedgerState = serde_json::from_slice(&read("ledger.json")).unwrap();
    let shielded: ShieldedState = serde_json::from_slice(&read("shielded.json")).unwrap();
    let bridge: BridgeState = serde_json::from_slice(&read("bridge.json")).unwrap();
    let meta: serde_json::Value = serde_json::from_slice(&read("store-meta-v1.json")).unwrap();
    let history: postfiat_storage::OrderedHistoryCommitment = serde_json::from_value(serde_json::json!({
        "schema": meta["ordered_history_schema"], "chain_id": genesis.chain_id, "genesis_hash": genesis_hash(&genesis),
        "protocol_version": genesis.protocol_version, "count": meta["ordered_batch_count"], "accumulator": meta["ordered_history_accumulator"]
    })).unwrap();
    let computed = crate::state_commitment::replicated_state_root_v2(
        &genesis,
        &governance,
        &ledger,
        &history,
        &shielded,
        &bridge,
    )
    .unwrap();
    assert_eq!(computed, meta["finalized_state_root"].as_str().unwrap());
    let dir = root.join("isolated-read-store");
    let store = NodeStore::new(&dir);
    store.write_genesis(&genesis).unwrap();
    store.write_governance(&governance).unwrap();
    store.write_ledger(&ledger).unwrap();
    store
        .write_chain_tip(&ChainTipState {
            schema: CHAIN_TIP_SCHEMA.into(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            height: meta["finalized_height"].as_u64().unwrap(),
            block_hash: meta["finalized_block_hash"].as_str().unwrap().into(),
            state_root: computed.clone(),
            ordered_batch_count: history.count,
            receipt_count: meta["receipt_count"].as_u64().unwrap(),
            history_base_height: meta["history_base_height"].as_u64().unwrap(),
        })
        .unwrap();
    let report = crate::market_bridge::vault_bridge_route(VaultBridgeRouteOptions {
        data_dir: dir.clone(),
        asset_id: current(&governance).profile.asset_id.clone(),
    })
    .unwrap();
    std::fs::write(
        root.join("fixed-route-report.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    std::fs::remove_dir_all(dir).unwrap();
    assert_eq!(report.profile.route_epoch, 11);
    assert!(report.beacon_finality_anchor.is_some());
    println!(
        "public state root unchanged at {}: {computed}; epoch-11 route read succeeded",
        meta["finalized_height"]
    );
}
