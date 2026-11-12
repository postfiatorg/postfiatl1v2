use postfiat_crypto_provider::{
    address_from_public_key, bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign,
    ML_DSA_65_ALGORITHM,
};
use postfiat_execution::{asset_transaction_tx_id, genesis_hash};
use postfiat_mempool_dag::{build_mixed_transaction_batch_with_assets, MempoolBatchDomain};
use postfiat_node::{
    navcoin_bridge_routes, navcoin_bridge_supply_status, pftl_uniswap_receipt,
    NavcoinBridgeRoutesOptions, NavcoinBridgeSupplyStatusOptions, NodeOptions,
};
use postfiat_storage::NodeStore;
use postfiat_types::*;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

fn read_route_fixture() -> PftlUniswapConsensusRouteState {
    let mut route: PftlUniswapConsensusRouteState=serde_json::from_str(r#"{
  "route_id": "b23-legacy-commitment",
  "route_family": "primary_pftl_mint",
  "route_config_digest": "111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111111",
  "route_trust_class": "BFT_CHECKPOINT",
  "native_nav_asset_id": "222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222222",
  "settlement_asset_id": "333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333333",
  "handoff_controller": "0x4444444444444444444444444444444444444444",
  "settlement_adapter": "0x5555555555555555555555555555555555555555",
  "wrapped_navcoin_token": "0x6666666666666666666666666666666666666666",
  "ethereum_chain_id": 1,
  "route_supply_cap_atoms": 1000,
  "packet_notional_cap_atoms": 100,
  "latest_finalized_nav_epoch": 1,
  "return_finality_blocks": 12,
  "authorized_valid_supply_atoms": 100,
  "pftl_spendable_supply_atoms": 100,
  "native_spendable_balances_atoms": {
    "alice": 100
  },
  "ethereum_spendable_supply_atoms": 0,
  "other_registered_venue_supply_atoms": 0,
  "outstanding_bridge_claims_atoms": 0,
  "pending_return_import_claims_atoms": 0,
  "settlement_reserve_atoms": 100,
  "paused": false,
  "v2": {
    "route_schema_version": 2,
    "route_epoch": 1,
    "outbound_verification_class": "TRUSTLESS_FINALITY",
    "return_verification_class": "BFT_CHECKPOINT",
    "issue_capacity_used_atoms": 100,
    "redeem_capacity_used_atoms": 0,
    "non_nav_spread_atoms": 5,
    "primary_market_policy": {
      "policy_hash": "777777777777777777777777777777777777777777777777777777777777777777777777777777777777777777777777",
      "policy_epoch": 1,
      "issue_multiplier_bps": 10050,
      "redeem_multiplier_bps": 9995,
      "issue_capacity_atoms": 1000,
      "redeem_capacity_atoms": 1000,
      "max_order_atoms": 100,
      "min_order_atoms": 1,
      "valid_from_height": 1,
      "expires_at_height": 100,
      "max_nav_age_blocks": 10,
      "pricing_nav_epoch": 1,
      "pricing_reserve_packet_hash": "888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888888"
    }
  }
}"#).unwrap();
    let policy = &mut route.v2.as_mut().unwrap().primary_market_policy;
    policy.policy_hash = policy.computed_hash();
    route.validate().unwrap();
    route
}

static NEXT: AtomicU64 = AtomicU64::new(0);
struct Fixture {
    dir: PathBuf,
    tx_ids: Vec<String>,
    ledger: LedgerState,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}
impl Fixture {
    fn options(&self) -> NodeOptions {
        NodeOptions {
            data_dir: self.dir.clone(),
        }
    }
    fn store(&self) -> NodeStore {
        NodeStore::new(&self.dir)
    }
    fn new(accepted: &[bool], legacy: bool) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "b23-facility-reads-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&dir).unwrap();
        let store = NodeStore::new(&dir);
        let genesis = Genesis::new("b23-read-fixture");
        store.write_genesis(&genesis).unwrap();
        let mut ledger = LedgerState::empty();
        let mut route = read_route_fixture();
        for code in ["B23", "USD"] {
            let asset = AssetDefinition::new(&genesis.chain_id, "issuer", code, 1, 6).unwrap();
            let mut nav = NavTrackedAsset::new(
                asset.asset_id.clone(),
                "issuer",
                "operator",
                "read-fixture",
                "USDC",
                "issuer",
            )
            .unwrap();
            nav.nav_per_unit = 1_000_000;
            nav.finalized_epoch = 1;
            if code == "B23" {
                route.native_nav_asset_id = asset.asset_id.clone();
            } else {
                route.settlement_asset_id = asset.asset_id.clone();
            }
            ledger.asset_definitions.push(asset);
            ledger.nav_assets.push(nav);
        }
        let policy = &mut route.v2.as_mut().unwrap().primary_market_policy;
        policy.spread_recipient = if legacy {
            None
        } else {
            Some("current-management".to_string())
        };
        policy.policy_hash = policy.computed_hash();
        let key = ml_dsa_65_keygen_from_seed(&[0x73; 32]);
        let owner = address_from_public_key(&key.public_key);
        let mut transactions = Vec::new();
        let mut receipts = Vec::new();
        let mut tx_ids = Vec::new();
        for (index, is_accepted) in accepted.iter().enumerate() {
            let operation = PftlUniswapPrimaryRedeemOperation {
                settlement_source_asset_id: None,
                owner: owner.clone(),
                settlement_recipient: owner.clone(),
                route_id: route.route_id.clone(),
                redemption_nonce: format!("{:064x}", index + 1),
                nav_amount_atoms: 1000,
                min_settlement_value_atoms: 998,
                route_epoch: 1,
                policy_epoch: 1,
                policy_hash: policy.policy_hash.clone(),
                pricing_nav_epoch: 1,
                pricing_reserve_packet_hash: "88".repeat(48),
                expires_at_height: 100,
            };
            let unsigned = UnsignedAssetTransaction {
                chain_id: genesis.chain_id.clone(),
                genesis_hash: genesis_hash(&genesis),
                protocol_version: genesis.protocol_version,
                address_namespace: ADDRESS_NAMESPACE.to_string(),
                transaction_kind: PFTL_UNISWAP_PRIMARY_REDEEM_TRANSACTION_KIND.to_string(),
                signature_algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                source: owner.clone(),
                fee: 100,
                sequence: index as u64 + 1,
                operation: AssetTransactionOperation::PftlUniswapPrimaryRedeem(operation),
            };
            let signature = ml_dsa_65_sign(&key.private_key, &unsigned.signing_bytes()).unwrap();
            let transaction = SignedAssetTransaction {
                unsigned,
                algorithm_id: ML_DSA_65_ALGORITHM.to_string(),
                public_key_hex: bytes_to_hex(&key.public_key),
                signature_hex: bytes_to_hex(&signature),
            };
            let tx_id = asset_transaction_tx_id(&transaction);
            let mut receipt = Receipt::accepted(&tx_id, "synthetic persisted redemption");
            if !is_accepted {
                receipt.accepted = false;
                receipt.code = "pftl_uniswap_redemption_unavailable".to_string();
                receipt.message = "insufficient reserve".to_string();
            }
            receipts.push(receipt);
            tx_ids.push(tx_id);
            transactions.push(transaction);
            if *is_accepted {
                let mut consensus = PftlUniswapConsensusReceipt {
                    redemption_settlement: if legacy {
                        None
                    } else {
                        Some(PftlUniswapRedemptionSettlement {
                            settlement_asset_id: route.settlement_asset_id.clone(),
                            settlement_recipient: owner.clone(),
                            settlement_value_atoms: 998,
                            spread_recipient: "historical-management".to_string(),
                            spread_value_atoms: 2,
                        })
                    },
                    receipt_hash: String::new(),
                    transition: "primary_redeemed".to_string(),
                    route_id: route.route_id.clone(),
                    state_before_hash: format!("{:096x}", index + 1),
                    state_after_hash: format!("{:096x}", index + 2),
                    packet_hash: None,
                    burn_event_hash: None,
                    wallet: Some(owner.clone()),
                    amount_atoms: Some(1000),
                    block_height: 1,
                };
                consensus.receipt_hash = pftl_uniswap_consensus_receipt_computed_hash(&consensus);
                ledger.pftl_uniswap_receipts.push(consensus);
            }
        }
        ledger.pftl_uniswap_routes.push(route);
        let domain = MempoolBatchDomain {
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
        };
        let batch =
            build_mixed_transaction_batch_with_assets(&domain, vec![], vec![], transactions)
                .unwrap();
        store
            .write_batch_archive(&BatchArchive {
                batches: vec![BatchArchiveEntry {
                    batch_kind: "transparent".to_string(),
                    batch_id: batch.reference.batch_id.clone(),
                    payload_hash: batch.reference.payload_hash,
                    payload_json: serde_json::to_string(&batch.batch).unwrap(),
                }],
            })
            .unwrap();
        let block = BlockRecord {
            header: BlockHeader {
                height: 1,
                view: 0,
                parent_hash: "genesis".to_string(),
                proposer: "validator-0".to_string(),
                batch_kind: "transparent".to_string(),
                batch_id: batch.reference.batch_id,
                state_root: "00".repeat(48),
                bridge_exit_root: None,
                pftl_uniswap_receipt_root: if ledger.pftl_uniswap_receipts.is_empty() {
                    None
                } else {
                    Some(
                        pftl_uniswap_consensus_receipt_root(&ledger.pftl_uniswap_receipts).unwrap(),
                    )
                },
                receipt_count: tx_ids.len() as u64,
                certificate_id: "11".repeat(48),
                certificate: BlockCertificate {
                    validators: vec![],
                    quorum: 0,
                    registry_root: String::new(),
                    votes: vec![],
                },
                consensus_v2_commit: None,
                block_hash: "22".repeat(48),
            },
            receipt_ids: tx_ids.clone(),
            fastpay_pre_state_effects: vec![],
        };
        store
            .write_blocks(&BlockLog {
                blocks: vec![block],
            })
            .unwrap();
        store.write_receipts(&receipts).unwrap();
        store.write_ordered_batches(&[]).unwrap();
        store.write_ledger(&ledger).unwrap();
        Self {
            dir,
            tx_ids,
            ledger,
        }
    }
}
fn directory_bytes(path: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut files = BTreeMap::new();
    for entry in fs::read_dir(path).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            files.extend(directory_bytes(&path));
        } else {
            files.insert(path.clone(), fs::read(path).unwrap());
        }
    }
    files
}
fn emit_response(name: &str, value: &impl serde::Serialize) {
    let response = postfiat_rpc_sdk::success_response("facility-read", value, vec![]).unwrap();
    println!(
        "READ_RESPONSE_{name}={}",
        serde_json::to_string(&response).unwrap()
    );
}

#[test]
fn b23_read_reports_expose_nullable_management_and_do_not_write() {
    for legacy in [true, false] {
        let fixture = Fixture::new(&[true], legacy);
        let before = directory_bytes(&fixture.dir);
        let routes = navcoin_bridge_routes(NavcoinBridgeRoutesOptions {
            data_dir: fixture.dir.clone(),
        })
        .unwrap();
        let supply = navcoin_bridge_supply_status(NavcoinBridgeSupplyStatusOptions {
            data_dir: fixture.dir.clone(),
            route_id: fixture.ledger.pftl_uniswap_routes[0].route_id.clone(),
        })
        .unwrap();
        let expected = if legacy {
            serde_json::Value::Null
        } else {
            serde_json::json!("current-management")
        };
        assert_eq!(
            serde_json::to_value(&routes).unwrap()["routes"][0]["spread_recipient"],
            expected
        );
        assert_eq!(
            serde_json::to_value(&supply).unwrap()["spread_recipient"],
            expected
        );
        let sequence = if legacy {
            serde_json::Value::Null
        } else {
            serde_json::json!(0)
        };
        assert_eq!(
            serde_json::to_value(&routes).unwrap()["routes"][0]["funding_sequence"],
            sequence
        );
        assert_eq!(
            serde_json::to_value(&supply).unwrap()["funding_sequence"],
            sequence
        );
        assert_eq!(directory_bytes(&fixture.dir), before);
        if !legacy {
            emit_response("ROUTES", &routes);
            emit_response("SUPPLY", &supply);
        }
    }
}
#[test]
fn b23_receipt_read_binds_transaction_and_returns_persisted_historical_credits() {
    let fixture = Fixture::new(&[true], false);
    let before = directory_bytes(&fixture.dir);
    let report = pftl_uniswap_receipt(fixture.options(), &fixture.tx_ids[0]).unwrap();
    assert!(report.accepted);
    assert_eq!(report.tx_id, fixture.tx_ids[0]);
    assert_eq!(report.redemption_nonce, format!("{:064x}", 1));
    assert_eq!(
        report.consensus_receipt,
        Some(fixture.ledger.pftl_uniswap_receipts[0].clone())
    );
    assert_eq!(
        report
            .consensus_receipt
            .as_ref()
            .unwrap()
            .redemption_settlement
            .as_ref()
            .unwrap()
            .spread_recipient,
        "historical-management"
    );
    assert_eq!(directory_bytes(&fixture.dir), before);
    emit_response("RECEIPT", &report);
}
#[test]
fn b23_receipt_read_rejected_transaction_never_borrows_an_accepted_receipt() {
    let fixture = Fixture::new(&[true, false], false);
    let report = pftl_uniswap_receipt(fixture.options(), &fixture.tx_ids[1]).unwrap();
    assert!(!report.accepted);
    assert!(report.consensus_receipt.is_none());
    assert_eq!(report.code, "pftl_uniswap_redemption_unavailable");
    emit_response("REJECTED", &report);
}
#[test]
fn b23_receipt_read_rejects_ambiguous_identical_redemptions() {
    let fixture = Fixture::new(&[true, true], false);
    let error = pftl_uniswap_receipt(fixture.options(), &fixture.tx_ids[0]).unwrap_err();
    assert!(
        error.to_string().contains("ambiguous_redemption_receipt"),
        "{error}"
    );
}
#[test]
fn b23_receipt_read_legacy_extension_stays_absent() {
    let fixture = Fixture::new(&[true], true);
    let report = pftl_uniswap_receipt(fixture.options(), &fixture.tx_ids[0]).unwrap();
    assert!(report.accepted);
    let value = serde_json::to_value(report).unwrap();
    assert!(value["consensus_receipt"]
        .get("redemption_settlement")
        .is_none());
}
#[test]
fn b23_receipt_read_rejects_missing_tampered_and_mismatched_evidence() {
    for case in 0..6 {
        let mut fixture = Fixture::new(&[true], false);
        match case {
            0 => fixture.ledger.pftl_uniswap_receipts.clear(),
            1 => {
                fixture.ledger.pftl_uniswap_receipts[0]
                    .redemption_settlement
                    .as_mut()
                    .unwrap()
                    .spread_value_atoms += 1
            }
            2 => {
                let mut blocks = fixture.store().read_blocks().unwrap();
                blocks.blocks[0].header.pftl_uniswap_receipt_root = Some("ff".repeat(48));
                fixture.store().write_blocks(&blocks).unwrap();
            }
            3 => {
                let mut archive = fixture.store().read_batch_archive().unwrap();
                archive.batches[0].payload_json = archive.batches[0]
                    .payload_json
                    .replace("\"nav_amount_atoms\":1000", "\"nav_amount_atoms\":1001");
                fixture.store().write_batch_archive(&archive).unwrap();
            }
            4 => {
                let mut receipts = fixture.store().read_receipts().unwrap();
                receipts.push(receipts[0].clone());
                fixture.store().write_receipts(&receipts).unwrap();
            }
            _ => {
                let mut blocks = fixture.store().read_blocks().unwrap();
                blocks.blocks[0].receipt_ids[0] = "aa".repeat(48);
                fixture.store().write_blocks(&blocks).unwrap();
            }
        }
        fixture.store().write_ledger(&fixture.ledger).unwrap();
        let before = directory_bytes(&fixture.dir);
        assert!(
            pftl_uniswap_receipt(fixture.options(), &fixture.tx_ids[0]).is_err(),
            "tamper case {case}"
        );
        assert_eq!(directory_bytes(&fixture.dir), before);
    }
    let fixture = Fixture::new(&[true], false);
    assert_eq!(
        pftl_uniswap_receipt(fixture.options(), "bad")
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::InvalidInput
    );
    assert_eq!(
        pftl_uniswap_receipt(fixture.options(), &"00".repeat(48))
            .unwrap_err()
            .kind(),
        std::io::ErrorKind::NotFound
    );
}
#[test]
fn b23_receipt_rpc_dispatch_returns_json_without_state_changes() {
    let fixture = Fixture::new(&[true], false);
    let request_file = fixture.dir.join("read-request.json");
    fs::write(
        &request_file,
        serde_json::to_vec(&serde_json::json!({
            "version":"postfiat-local-rpc-v1", "id":"facility-read",
            "method":"pftl_uniswap_receipt", "params":{"tx_id":fixture.tx_ids[0]},
        }))
        .unwrap(),
    )
    .unwrap();
    let before = directory_bytes(&fixture.dir);
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_postfiat-node"))
        .args([
            "rpc",
            "--data-dir",
            fixture.dir.to_str().unwrap(),
            "--request-file",
            request_file.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], true);
    assert_eq!(value["result"]["tx_id"], fixture.tx_ids[0]);
    assert_eq!(
        value["result"]["consensus_receipt"]["redemption_settlement"]["spread_value_atoms"],
        2
    );
    assert_eq!(directory_bytes(&fixture.dir), before);
}

#[test]
fn b23_status_rpc_dispatch_preserves_existing_envelopes_and_adds_recipient() {
    let fixture = Fixture::new(&[true], false);
    let before = directory_bytes(&fixture.dir);
    for (method, label) in [
        ("navcoin_bridge_routes", "ROUTES_RPC"),
        ("navcoin_bridge_supply_status", "SUPPLY_RPC"),
    ] {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_postfiat-node"))
            .args([
                "rpc",
                "--data-dir",
                fixture.dir.to_str().unwrap(),
                "--method",
                method,
                "--route-id",
                &fixture.ledger.pftl_uniswap_routes[0].route_id,
                "--id",
                "facility-read",
            ])
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(value["ok"], true);
        let report = if method == "navcoin_bridge_routes" {
            &value["result"]["routes"][0]
        } else {
            &value["result"]
        };
        assert_eq!(report["spread_recipient"], "current-management");
        assert_eq!(report["funding_sequence"], 0);
        assert_eq!(value["events"][0]["event_type"], method);
        println!(
            "READ_RESPONSE_{label}={}",
            serde_json::to_string(&value).unwrap()
        );
    }
    assert_eq!(directory_bytes(&fixture.dir), before);
}

#[test]
fn b23_route_reads_report_last_successful_funding_sequence() {
    let mut fixture = Fixture::new(&[true], false);
    fixture.ledger.pftl_uniswap_routes[0]
        .v2
        .as_mut()
        .unwrap()
        .funding_sequence = Some(17);
    fixture.store().write_ledger(&fixture.ledger).unwrap();
    let before = directory_bytes(&fixture.dir);
    let routes = navcoin_bridge_routes(NavcoinBridgeRoutesOptions {
        data_dir: fixture.dir.clone(),
    })
    .unwrap();
    let supply = navcoin_bridge_supply_status(NavcoinBridgeSupplyStatusOptions {
        data_dir: fixture.dir.clone(),
        route_id: fixture.ledger.pftl_uniswap_routes[0].route_id.clone(),
    })
    .unwrap();
    assert_eq!(routes.routes[0].funding_sequence, Some(17));
    assert_eq!(supply.funding_sequence, Some(17));
    assert_eq!(directory_bytes(&fixture.dir), before);
}

#[path = "b23_facility_reads/read_only.rs"]
mod read_only_regressions;
