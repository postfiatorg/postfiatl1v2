//! Prepare an Orchard-only birthday, then run the browser against the shipping
//! RPC and both gateway routes. Secrets stay in the disposable local directory.
use super::*;
use std::process::Command;

#[test]
fn two_browser_wallets_and_orchard_migration() {
    let f = Fixture::new();
    let disk = NodeStore::new(&f.dir);
    let mut governance = disk.read_governance().unwrap();
    let mut schedule = crate::tests::pq_payment_activation::amendment(&f.genesis, 1);
    schedule.kind = "pq_payment_rehearsal_v1:fee:2".into();
    governance.apply(schedule);
    disk.write_governance(&governance).unwrap();
    let key = |name: &str, seed| {
        let path = f.dir.join(name);
        let key = write_test_master_key(&path, seed);
        (path, key.address)
    };
    let (alice_key, alice) = key("alice.key", [41; 32]);
    let (bob_key, bob) = key("bob.key", [42; 32]);
    let seed = bytes_to_hex(&rand::random::<[u8; 32]>());
    let ingress = f.dir.join("ingress.json");
    create_asset_orchard_ingress(AssetOrchardIngressCreateOptions {
        data_dir: f.dir.clone(),
        key_file: alice_key.clone(),
        asset_id: f.child.clone(),
        amount: 9,
        fee: 0,
        note_seed_hex: seed.clone(),
        encrypted_output_hex: None,
        ingress_file: ingress.clone(),
        note_file: f.dir.join("initial.note"),
        overwrite: false,
    })
    .unwrap();
    let batch = f.dir.join("ingress.batch");
    create_asset_orchard_ingress_batch(AssetOrchardIngressBatchOptions {
        data_dir: f.dir.clone(),
        ingress_file: ingress,
        batch_file: batch.clone(),
    })
    .unwrap();
    let receipt = apply_shielded_batch(ApplyBatchOptions {
        data_dir: f.dir.clone(),
        batch_file: batch,
        certificate_file: None,
    })
    .unwrap();
    assert!(receipt[0].accepted, "{}", receipt[0].code);
    let historical = private_pool_snapshot(PrivatePoolSnapshotOptions {
        data_dir: f.dir.clone(),
        pool_id: ASSET_ORCHARD_POOL_ID_V1.into(),
        address: None,
        base_asset_id: None,
        quote_asset_id: None,
        inclusion_height: None,
    })
    .unwrap();
    assert_eq!(historical.counts.outputs, 1);
    assert!(disk.read_shielded().unwrap().pq_pool_v1.is_none());
    let note = f.dir.join("recovered.note");
    let scan = asset_orchard_scan(AssetOrchardScanOptions {
        data_dir: f.dir.clone(),
        note_seed_hex: seed,
        note_file: note.clone(),
        overwrite: false,
    })
    .unwrap();
    assert!(scan.recovered);
    println!("Orchard-only finalized snapshot: one encrypted output recovered before the first PQ deposit.");
    let exit = f.dir.join("exit.json");
    let report = create_asset_orchard_private_egress(AssetOrchardPrivateEgressCreateOptions {
        data_dir: f.dir.clone(),
        note_file: note,
        to: alice.clone(),
        asset_id: Some(f.child.clone()),
        amount: Some(9),
        fee: 0,
        policy_id: "wallet_private_egress_public_exit_v1".into(),
        disclosure_hash: "pq-wallet-migration".into(),
        pending_output_commitments: vec![],
        egress_file: exit.clone(),
        overwrite: false,
    })
    .unwrap();
    assert!(report.verified);
    let batch = f.dir.join("exit.batch");
    create_asset_orchard_private_egress_batch(AssetOrchardPrivateEgressBatchOptions {
        data_dir: f.dir.clone(),
        egress_file: exit,
        batch_file: batch.clone(),
    })
    .unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let result = Command::new("node")
        .arg("wallet-web/scripts/test-pq-payments.mjs")
        .arg("--exercise")
        .current_dir(root)
        .env("PQ_CHAIN_DIR", &f.dir)
        .env("PQ_EXIT_BATCH", batch)
        .env("PQ_ALICE", alice)
        .env("PQ_ALICE_KEY", alice_key)
        .env("PQ_BOB", bob)
        .env("PQ_BOB_KEY", bob_key)
        .env("PQ_ISSUE", &f.child)
        .status()
        .unwrap();
    assert!(result.success());
    let ledger = disk.read_ledger().unwrap();
    let shielded = disk.read_shielded().unwrap();
    let orchard = &shielded.orchard.as_ref().unwrap().asset_orchard_balances[0];
    let pq = shielded.pq_pool_v1.as_ref().unwrap().head.assets[&f.child].reserve;
    let public: u64 = ledger
        .trustlines
        .iter()
        .filter(|l| l.asset_id == f.child)
        .map(|l| l.balance)
        .sum();
    assert_eq!(orchard.ingress_total, orchard.egress_total);
    assert_eq!(pq + public + ledger.collected_private_fees[&f.child], 100);
    assert_eq!(
        global_issued_asset_supply(&ledger, &shielded, &f.family).unwrap(),
        110
    );
    println!("Final child issue: public {public}, PQ {pq}, Orchard 0. Family supply 110.");
    fs::remove_dir_all(&f.dir).unwrap();
}
