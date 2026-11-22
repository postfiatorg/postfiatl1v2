//! Conditional economic execution on a ledger copy. No network or key-file APIs.
//! A fresh in-memory test subscriber is added ONLY to the copy; this
//! does not establish actual authorization, bridge finality, or block finality.
use anyhow::{ensure, Result};
use clap::Parser;
use postfiat_crypto_provider::{address_from_public_key, bytes_to_hex, ml_dsa_65_keygen,
    ml_dsa_65_sign, ML_DSA_65_ALGORITHM};
use postfiat_execution::{execute_asset_transaction_with_compatibility, AssetExecutionCompatibility};
use postfiat_types::{AssetTransactionOperation, Genesis, GovernanceState, LedgerState,
    SignedAssetTransaction, UnsignedAssetTransaction, trustline_id};
use serde_json::{json, Value};
use std::{fs, path::PathBuf, time::Instant};

const ALICE: &str = "pfab9b9228942e5c529633a13aa271d5297bec6353";
const S: &str = "541e451e6ae96b6680988655571a0feec59c7f1d073c9b5d29547254cab38a3be540f8767446d25baebc7081ce97aef7";
const F: &str = "02c46a36eb0da3516b4d8affea8f4028ad3f36825a3e8f0e009ea9dbbbcfb3c233f6830bd5221fe2717fb6a1a7005d7b";
const ROUTE: &str = "pftl-b24-ethereum-wB24-usdc-v1";
const B24: &str = "52285e4ded1d58113cfaa12bbc034b8b10213f699b3224c74160450229d6bf7ccfe505c78f2599d975ae2ecf98f7d8af";

#[derive(Parser)]
struct Args {
    #[arg(long)] snapshot: PathBuf,
    #[arg(long)] operations: PathBuf,
    #[arg(long)] output: PathBuf,
    #[arg(long)] allow_explicit_test_identity_and_credit: bool,
}

fn replace(v: &mut Value, from: &str, to: &str) {
    match v {
        Value::String(s) if s == from => *s = to.to_string(),
        Value::Array(xs) => xs.iter_mut().for_each(|v| replace(v, from, to)),
        Value::Object(map) => {
            if let Some(v) = map.remove(from) { map.insert(to.to_string(), v); }
            map.values_mut().for_each(|v| replace(v, from, to));
        }
        _ => (),
    }
}

fn main() -> Result<()> {
    let args = Args::parse();
    ensure!(args.allow_explicit_test_identity_and_credit, "explicit fixture acknowledgement required");
    ensure!(!args.output.exists(), "output already exists");
    let started = Instant::now();
    let mut snapshot: Value = serde_json::from_slice(&fs::read(&args.snapshot)?)?;
    let mut operations: Value = serde_json::from_slice(&fs::read(&args.operations)?)?;
    ensure!(operations.as_array().is_some_and(|ops| ops.len() == 3), "expected reserve/subscribe/export");
    let key = ml_dsa_65_keygen()?;
    let test_owner = address_from_public_key(&key.public_key);
    replace(&mut operations, ALICE, &test_owner);
    let accounts = snapshot["ledger"]["accounts"].as_array_mut().unwrap();
    let alice = accounts.iter_mut().find(|x| x["address"] == ALICE).unwrap();
    ensure!(alice["balance"].as_u64().unwrap() >= 1_000_000, "fixture fee funding absent");
    alice["balance"] = (alice["balance"].as_u64().unwrap() - 1_000_000).into();
    let mut test_account = alice.clone();
    test_account["address"] = test_owner.clone().into();
    test_account["public_key_hex"] = bytes_to_hex(&key.public_key).into();
    test_account["balance"] = 1_000_000u64.into();
    test_account["sequence"] = 0u64.into();
    accounts.push(test_account);
    let lines = snapshot["ledger"]["trustlines"].as_array_mut().unwrap();
    for asset in [S, B24] {
        let mut line = lines.iter().find(|x| x["account"] == ALICE && x["asset_id"] == asset).unwrap().clone();
        line["account"] = test_owner.clone().into();
        line["balance"] = 0u64.into();
        line["trustline_id"] = trustline_id(&test_owner, line["issuer"].as_str().unwrap(), asset).map_err(anyhow::Error::msg)?.into();
        lines.push(line);
    }
    let amount = 5_025_000_000u64;
    let line = snapshot["ledger"]["trustlines"].as_array_mut().unwrap().iter_mut()
        .find(|x| x["account"] == test_owner && x["asset_id"] == S).unwrap();
    let source_before = line["balance"].as_u64().unwrap() + amount;
    line["balance"] = source_before.into();
    line["limit"] = line["limit"].as_u64().unwrap().max(source_before).into();
    let bucket_id = snapshot["ledger"]["asset_definitions"].as_array().unwrap().iter()
        .find(|x| x["asset_id"] == S).unwrap()["source_bucket_id"].clone();
    let bucket = snapshot["ledger"]["vault_bridge_bucket_states"].as_array_mut().unwrap().iter_mut()
        .find(|x| x["bucket_id"] == bucket_id).unwrap();
    for field in ["gross_receipt_atoms", "counted_value_atoms", "outstanding_vault_bridge_atoms"] {
        bucket[field] = (bucket[field].as_u64().unwrap() + amount).into();
    }
    let nav = snapshot["ledger"]["nav_assets"].as_array_mut().unwrap().iter_mut()
        .find(|x| x["asset_id"] == F).unwrap();
    nav["circulating_supply"] = (nav["circulating_supply"].as_u64().unwrap() + amount).into();
    let genesis: Genesis = serde_json::from_value(snapshot["genesis"].clone())?;
    let governance: GovernanceState = serde_json::from_value(snapshot["governance"].clone())?;
    let mut ledger: LedgerState = serde_json::from_value(snapshot["ledger"].clone())?;
    let compatibility = AssetExecutionCompatibility::strict()
        .with_bridge_verification_activation_height(governance.bridge_verification_activation_height().or(genesis.bridge_verification_activation_height))
        .with_orchard_aware_bridge_claim_activation_height(governance.orchard_aware_bridge_claim_activation_height().or(genesis.orchard_aware_bridge_claim_activation_height))
        .with_pfusdc_source_series_activation_height(governance.pfusdc_source_series_activation_height().or(genesis.pfusdc_source_series_activation_height))
        .with_atomic_swap_activation_height(governance.atomic_swap_activation_height().or(genesis.atomic_swap_activation_height))
        .with_yolo_target_activation_height(governance.yolo_target_activation_height())
        .with_atomic_swap_paused(governance.atomic_swap_paused);
    let before_route = serde_json::to_value(ledger.pftl_uniswap_routes.iter().find(|r| r.route_id == ROUTE).unwrap())?;
    let mut receipts = Vec::new();
    for (i, row) in operations.as_array().unwrap().iter().enumerate() {
        let operation: AssetTransactionOperation = serde_json::from_value(row["operation"].clone())?;
        let unsigned = UnsignedAssetTransaction {
            chain_id: snapshot["tip"]["chain_id"].as_str().unwrap().into(),
            genesis_hash: snapshot["tip"]["genesis_hash"].as_str().unwrap().into(),
            protocol_version: snapshot["tip"]["protocol_version"].as_u64().unwrap() as u32,
            address_namespace: "postfiat.address.v1".into(), transaction_kind: operation.transaction_kind().into(),
            signature_algorithm_id: ML_DSA_65_ALGORITHM.into(), source: test_owner.clone(), fee: 1000,
            sequence: ledger.accounts.iter().find(|a| a.address == test_owner).unwrap().sequence + 1,
            operation,
        };
        let signed = SignedAssetTransaction {
            signature_hex: bytes_to_hex(&ml_dsa_65_sign(&key.private_key, &unsigned.signing_bytes())?),
            public_key_hex: bytes_to_hex(&key.public_key), algorithm_id: ML_DSA_65_ALGORITHM.into(), unsigned,
        };
        let now = Instant::now();
        let receipt = execute_asset_transaction_with_compatibility(&genesis, &mut ledger, &signed,
            snapshot["tip"]["height"].as_u64().unwrap() + i as u64 + 1, compatibility);
        ensure!(receipt.accepted, "{}: {} {}", row["label"], receipt.code, receipt.message);
        receipts.push(json!({"label": row["label"], "receipt": receipt,
            "execution_wall_seconds": now.elapsed().as_secs_f64(), "PFT_fee_atoms": 1000, "Ethereum_gas": 0}));
    }
    let after = serde_json::to_value(&ledger)?;
    let route = after["pftl_uniswap_routes"].as_array().unwrap().iter().find(|r| r["route_id"] == ROUTE).unwrap();
    let source_after = after["trustlines"].as_array().unwrap().iter()
        .find(|x| x["account"] == test_owner && x["asset_id"] == S).unwrap()["balance"].as_u64().unwrap();
    ensure!(source_before - source_after == amount, "source debit differs");
    ensure!(route["authorized_valid_supply_atoms"].as_u64().unwrap() == before_route["authorized_valid_supply_atoms"].as_u64().unwrap() + 5_000_000_000, "authorized supply delta differs");
    for field in ["pftl_spendable_supply_atoms", "ethereum_spendable_supply_atoms"] {
        ensure!(route[field] == before_route[field], "unexpected spendable supply delta: {field}");
    }
    ensure!(route["outstanding_bridge_claims_atoms"].as_u64().unwrap() == before_route["outstanding_bridge_claims_atoms"].as_u64().unwrap() + 5_000_000_000, "export claims differ");
    let conserved: u64 = ["pftl_spendable_supply_atoms", "ethereum_spendable_supply_atoms", "other_registered_venue_supply_atoms", "outstanding_bridge_claims_atoms", "pending_return_import_claims_atoms"]
        .iter().map(|k| route[k].as_u64().unwrap()).sum();
    ensure!(conserved == route["authorized_valid_supply_atoms"].as_u64().unwrap(), "conservation failed");
    let result = json!({"conditional_native_copy_passed": true, "real_authorization": false,
        "real_bridge_credit": false, "real_finality": false, "key_files_read_or_written": false,
        "fixture_identity": test_owner, "fixture_credit_atoms": amount, "source_debit_atoms": source_before - source_after,
        "export_atoms": 5_000_000_000u64, "conservation_passed": true, "receipts": receipts,
        "wall_seconds": started.elapsed().as_secs_f64(), "live_writes": 0,
        "fixture_changes": ["Fresh in-memory test subscriber/account and S/B24 trustlines; 1 PFT moved from Alice for copy fees; operations use test identity", "Test S balance/limit and backing bucket +5025", "PFUSDC family supply +5025"],
        "native_snapshot_height": snapshot["tip"]["height"], "after_route": route});
    fs::write(args.output, serde_json::to_vec_pretty(&result)?)?;
    println!("conditional native reserve/subscribe/export passed; no actual authorization or finality");
    Ok(())
}
