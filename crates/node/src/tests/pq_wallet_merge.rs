//! A closed, certificate-producing chain starting at M3's checked depth-24
//! frontier. The compact pre-birthday prefix exists only in this test. The
//! validator has no RPC or import path for setting occupancy or wallet state.
use super::*;
use crate::{
    asset_orchard_pricing_reads::FinalizedPairSnapshot,
    private_pool_reads::{private_pool_page_with, private_pool_snapshot_from},
};
use serde_json::{json, Value};
use std::{
    io::{Read, Write},
    net::TcpListener,
    os::unix::fs::PermissionsExt,
    process::Command,
    time::Duration,
};

#[test]
fn pq_inactive_snapshot_accepts_explicit_heights() {
    let f = Fixture::new();
    let store = NodeStore::new(&f.dir);
    let mut snap = FinalizedPairSnapshot {
        genesis: f.genesis.clone(),
        ledger: store.read_ledger().unwrap(),
        governance: store.read_governance().unwrap(),
        tip: storage_commit::reconstruct_chain_tip_for_genesis(&store, &f.genesis).unwrap(),
        shielded: store.read_shielded().unwrap(),
    };
    snap.genesis.chain_id = "unrelated-chain".into();
    for height in [0, snap.tip.height + 2] {
        let report = private_pool_snapshot_from(PrivatePoolSnapshotOptions {
            data_dir:f.dir.clone(), pool_id:pq::POOL_ID.into(), address:None,
            base_asset_id:None, quote_asset_id:None, inclusion_height:Some(height),
        }, &snap).unwrap();
        assert_eq!(report.inclusion_height, height);
        assert!(report.pq_issued_fee.is_none());
    }
}

#[test]
fn recover_both_assets_and_spend_across_rollover_from_python() {
    let f = Fixture::new();
    let store = NodeStore::new(&f.dir);
    let bridge = store.read_bridge().unwrap();
    let mut snap = FinalizedPairSnapshot {
        genesis: f.genesis.clone(),
        ledger: store.read_ledger().unwrap(),
        governance: store.read_governance().unwrap(),
        tip: storage_commit::reconstruct_chain_tip_for_genesis(&store, &f.genesis).unwrap(),
        shielded: store.read_shielded().unwrap(),
    };
    let mut prefix: Value = serde_json::from_str(include_str!(
        "../../../../tools/pft-binius-shielded-pq/rollover-vectors/frontier-0.json"
    ))
    .unwrap();
    prefix.as_object_mut().unwrap().remove("terminal");
    let mut prefix: pq::PqEpoch = serde_json::from_value(prefix).unwrap();
    assert_eq!(
        prefix.anchors.back(),
        Some(&bytes_to_hex(&prefix.frontier_root().unwrap()))
    );
    assert_eq!(prefix.count, pq::PQ_CAPACITY - 2);
    // Remove the last complete two-leaf subtree, leaving room for shield + pay.
    prefix.count -= 2;
    prefix.frontier[1].take().unwrap();
    prefix.anchors = [bytes_to_hex(&prefix.frontier_root().unwrap())].into();
    snap.shielded
        .pq_pool_v1
        .get_or_insert_with(Default::default)
        .head
        .assets
        .entry(pq::NATIVE_ASSET_HEX.into())
        .or_default()
        .writable = prefix;
    let mut ordered = vec![];
    snap.tip.state_root = replicated_state_root(
        &f.genesis,
        &snap.governance,
        &snap.ledger,
        &ordered,
        &snap.shielded,
        &bridge,
    )
    .unwrap();
    let validators = active_validator_ids(&snap.governance).unwrap();
    let keys = read_validator_key_file(&f.dir.join(VALIDATOR_KEYS_FILE)).unwrap();
    let registry = validator_registry_from_keys_for_validators(&keys, &validators).unwrap();
    let mut blocks = Vec::<BlockRecord>::new();
    let mut receipts = std::collections::BTreeMap::<String, Value>::new();
    let mut burned = 0;

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let python = std::env::var("PQ_PYTHON").unwrap_or_else(|_| "python3".to_owned());
    let mut command = Command::new(python);
    command
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../python/tests/test_pq_wallet_cli.py"))
        .arg("--exercise");
    command
        .env("PQ_ENDPOINT", listener.local_addr().unwrap().to_string())
        .env("PQ_ISSUE", &f.child);
    let secret_dir = f.dir.join("test-accounts");
    fs::create_dir(&secret_dir).unwrap();
    fs::set_permissions(&secret_dir, fs::Permissions::from_mode(0o700)).unwrap();
    for (who, key) in [("ALICE", &f.alice), ("SPONSOR", &f.sponsor)] {
        let path = secret_dir.join(who);
        let file = fs::File::create(&path).unwrap();
        file.set_permissions(fs::Permissions::from_mode(0o600))
            .unwrap();
        serde_json::to_writer(file,&json!({"public_key_hex":bytes_to_hex(&key.public_key),"private_key_hex":bytes_to_hex(&key.private_key)})).unwrap();
        command.env(format!("PQ_{who}_KEY"), path).env(
            format!("PQ_{who}"),
            address_from_public_key(&key.public_key),
        );
    }
    let mut child = command.spawn().unwrap();
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            assert!(status.success(), "CLI scenario failed");
            break;
        }
        let (mut s, _) = match listener.accept() {
            Ok(v) => v,
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(20));
                continue;
            }
            Err(e) => panic!("{e}"),
        };
        let mut buf = Vec::new();
        (&mut s).take(1_048_577).read_to_end(&mut buf).unwrap();
        let req: Value = serde_json::from_slice(&buf).unwrap();
        let p = &req["params"];
        let text = |k: &str| p[k].as_str().unwrap().to_owned();
        let method = req["method"].as_str().unwrap();
        let allowed: &[&str] = match method {
            "private_pool_snapshot" => &["pool_id"],
            "private_pool_page" => &["snapshot_token", "section", "limit", "cursor"],
            "blocks" => &["from_height", "limit"],
            "private_action_submit" => &[
                "action_json",
                "action_hash",
                "intent_id",
                "protocol_version",
                "chain_id",
                "genesis_hash",
                "pool_id",
                "session_scope",
            ],
            "private_action_receipt" => &["intent_id"],
            _ => panic!("unexpected wallet method"),
        };
        assert!(p
            .as_object()
            .unwrap()
            .keys()
            .all(|k| allowed.contains(&k.as_str())));
        let out = match method {
            "private_pool_snapshot" => serde_json::to_value(
                private_pool_snapshot_from(
                    PrivatePoolSnapshotOptions {
                        data_dir: f.dir.clone(),
                        pool_id: text("pool_id"),
                        address: None,
                        base_asset_id: None,
                        quote_asset_id: None,
                        inclusion_height: None,
                    },
                    &snap,
                )
                .unwrap(),
            )
            .unwrap(),
            "private_pool_page" => serde_json::to_value(
                private_pool_page_with(
                    PrivatePoolPageOptions {
                        data_dir: f.dir.clone(),
                        snapshot_token: text("snapshot_token"),
                        section: text("section"),
                        cursor: p["cursor"].as_str().map(str::to_owned),
                        limit: p["limit"].as_u64().map(|n| n as usize),
                    },
                    |_| Ok(snap.clone()),
                )
                .unwrap(),
            )
            .unwrap(),
            "blocks" => json!([blocks[(p["from_height"].as_u64().unwrap() - 1) as usize]]),
            "private_action_receipt" => receipts[&text("intent_id")].clone(),
            _ => {
                let action: ShieldedAction = serde_json::from_str(&text("action_json")).unwrap();
                let intent = text("intent_id");
                assert_eq!(
                    postfiat_types::private_action_hash(&action).unwrap(),
                    text("action_hash")
                );
                if let Some(prior) = receipts.get(&intent) {
                    json!({"record":prior["record"]})
                } else {
                    let ShieldedAction::PqPaymentV1(ref payment) = action else {
                        panic!("unexpected action")
                    };
                    let (terms, _, _) = payment.parts().unwrap();
                    let asset = bytes_to_hex(&terms.asset);
                    let read_epoch = terms.epoch;
                    let b = build_atomic_shielded_action_batch(&f.genesis, vec![action]).unwrap();
                    let h = snap.tip.height + 1;
                    consensus_artifacts::check_pq_block(&f.genesis, &snap.governance, h, [&b])
                        .unwrap();
                    let compatibility = asset_execution_compatibility_for_genesis_and_governance(
                        &f.genesis,
                        &snap.governance,
                    );
                    let rs = execute_shielded_batch(
                        &f.genesis,
                        &mut snap.ledger,
                        &mut snap.shielded,
                        &b,
                        h,
                        compatibility,
                        false,
                        snap.governance.shielded_atomic_batch_activation_height(),
                        &snap.governance,
                        false,
                    );
                    assert!(rs[0].accepted, "payment rejected: {}", rs[0].message);
                    let c = prepare_ordered_commit(OrderedCommitPlan {
                        genesis: &f.genesis,
                        governance: &snap.governance,
                        ledger: &snap.ledger,
                        ordered_batches: &ordered,
                        ordered_history: None,
                        shielded: &snap.shielded,
                        bridge: &bridge,
                        block_height: h,
                        parent_hash: snap.tip.block_hash.clone(),
                        batch_kind: "shielded",
                        batch_id: &b.batch_id,
                        payload: &b,
                        batch_receipts: &rs,
                        archived_payload_json: None,
                        validator_keys: Some(&keys),
                        external_certificate: None,
                        external_validator_registry: None,
                        external_certificate_preverified: false,
                        historical_replay: None,
                        certificate_validators: &validators,
                        fastpay_pre_state_effects: &[],
                    })
                    .unwrap();
                    verify_block_certificate_evidence(&f.genesis, &c.block, &registry, &validators)
                        .unwrap();
                    ordered.push(b.batch_id);
                    burned += rs[0].fee_burned;
                    let pool = snap.shielded.pq_pool_v1.as_ref().unwrap();
                    assert_eq!(
                        snap.ledger.accounts.iter().map(|a| a.balance).sum::<u64>()
                            + pool.head.assets[pq::NATIVE_ASSET_HEX].reserve
                            + burned,
                        f.starting_supply
                    );
                    let custody = pool.head.assets.get(&f.child).map_or(0, |a| a.reserve);
                    let issued = snap
                        .ledger
                        .trustlines
                        .iter()
                        .filter(|l| l.asset_id == f.child || l.asset_id == f.sibling)
                        .map(|l| l.balance)
                        .sum::<u64>();
                    let collected = snap.ledger.collected_private_fees.get(&f.child).copied().unwrap_or(0);
                    assert_eq!(issued + custody + collected, 110);
                    let write_epoch = pool.pending.back().unwrap().epoch;
                    println!("certified block {h}, {asset} read epoch {read_epoch} output epoch {write_epoch}, PFT supply {}, pfUSDC family 110",f.starting_supply);
                    snap.tip.height = h;
                    snap.tip.block_hash = c.block.header.block_hash.clone();
                    snap.tip.state_root = c.block.header.state_root.clone();
                    assert_eq!(
                        snap.tip.state_root,
                        replicated_state_root(
                            &f.genesis,
                            &snap.governance,
                            &snap.ledger,
                            &ordered,
                            &snap.shielded,
                            &bridge
                        )
                        .unwrap()
                    );
                    postfiat_storage::pq_pool::persist_pending(&f.dir, &snap.shielded).unwrap();
                    snap.shielded.pq_pool_v1.as_mut().unwrap().pending.clear();
                    blocks.push(c.block);
                    let record = json!({"state":"accepted","intent_id":intent,"action_hash":text("action_hash"),"outcome":{"accepted":true,"height":h}});
                    receipts.insert(
                        intent.clone(),
                        json!({"state":"accepted","intent_id":intent,"record":record}),
                    );
                    json!({"record":record})
                }
            }
        };
        serde_json::to_writer(
            &mut s,
            &json!({"version":"postfiat-local-rpc-v1","ok":true,"id":req["id"],"result":out}),
        )
        .unwrap();
        s.flush().unwrap();
    }
    assert_eq!(blocks.len(), 8);
    fs::remove_dir_all(&f.dir).unwrap();
}

#[test]
fn pq_wallet_pages_pin_real_disk_history_after_another_commit() {
    let f = Fixture::new();
    let vector = |name: &str| {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/pft-binius-shielded-pq/l1-vectors")
            .join(format!("{name}.bin"));
        PqPayment::from_bytes(fs::read(path).unwrap()).unwrap()
    };
    let snapshot = || {
        private_pool_snapshot(PrivatePoolSnapshotOptions {
            data_dir: f.dir.clone(),
            pool_id: pq::POOL_ID.into(),
            address: None,
            base_asset_id: None,
            quote_asset_id: None,
            inclusion_height: None,
        })
        .unwrap()
    };
    let page = |token: &str, section: &str, cursor: Option<String>| {
        private_pool_page(PrivatePoolPageOptions {
            data_dir: f.dir.clone(),
            snapshot_token: token.into(),
            section: section.into(),
            cursor,
            limit: Some(1),
        })
        .unwrap()
    };
    assert!(f.apply(&[vector("pft-shield")]).unwrap()[0].accepted);
    assert!(
        private_pool_snapshot(PrivatePoolSnapshotOptions {
            data_dir: f.dir.clone(),
            pool_id: pq::POOL_ID.into(),
            address: None,
            base_asset_id: None,
            quote_asset_id: None,
            inclusion_height: Some(0),
        })
        .is_err(),
        "PQ quotes only support the next block"
    );
    let old = snapshot();
    let first = page(&old.snapshot_token, "pq_pairs", None);
    assert_eq!(first.item_count, 1);
    assert_eq!(f.apply(&[vector("pft-pay")]).unwrap().len(), 1);
    assert_eq!(page(&old.snapshot_token, "pq_pairs", None), first);
    let current = snapshot();
    let pairs = page(&current.snapshot_token, "pq_pairs", None);
    assert_eq!(pairs.total, 2);
    let req = postfiat_rpc_sdk::pq_pairs_request("resume", &current.snapshot_token, 1);
    let cursor = req.params["cursor"].as_str().unwrap().to_owned();
    assert_eq!(pairs.next_cursor.as_ref(), Some(&cursor));
    let last = page(&current.snapshot_token, "pq_pairs", Some(cursor.clone()));
    assert_eq!(last.item_count, 1);
    assert!(last.next_cursor.is_none());
    if let Some(crate::private_pool_reads::PrivatePoolPageItems::PqPairs(pairs)) = last.items {
        let digest = postfiat_rpc_sdk::validate_pq_pairs(
            &pairs,
            old.pq_last_page.as_deref().unwrap(),
            current.finalized_height,
        )
        .unwrap();
        assert_eq!(Some(digest), current.pq_last_page);
    } else {
        panic!("missing finalized pair");
    }
    assert_eq!(
        page(&old.snapshot_token, "pq_pairs", Some(cursor)).status,
        "refused"
    );
    let epoch = page(&old.snapshot_token, "pq_epochs", None);
    if let Some(crate::private_pool_reads::PrivatePoolPageItems::PqEpochs(rows)) = epoch.items {
        assert_eq!(
            (rows[0].0.as_str(), rows[0].1, rows[0].2.count),
            (pq::NATIVE_ASSET_HEX, 0, 2)
        );
        assert_eq!(rows[0].2.anchors.len(), 2);
    } else {
        panic!("missing epoch")
    }
    fs::remove_dir_all(&f.dir).unwrap();
}

#[test]
fn pq_wallet_replaces_reserved_notes_across_a_fee_change() {
    use postfiat_privacy_binius_wallet::{
        keys::Network,
        wallet::{Order, Point, Recovery, Wallet},
    };
    let f = Fixture::new();
    let disk = NodeStore::new(&f.dir);
    let mut governance = disk.read_governance().unwrap();
    for (at, fee) in [(4, 2), (5, 3)] {
        let mut a = crate::tests::pq_payment_activation::amendment(&f.genesis, at);
        a.kind = format!("pq_payment_rehearsal_v1:fee:{fee}");
        governance.apply(a);
    }
    disk.write_governance(&governance).unwrap();
    let keyfile = f.dir.join("wallet.key");
    let payer = write_test_master_key(&keyfile, [41; 32]).address;
    fs::set_permissions(&keyfile, fs::Permissions::from_mode(0o600)).unwrap();
    let net = Network {
        chain_id: f.genesis.chain_id.clone(),
        genesis_hash: genesis_hash(&f.genesis),
        protocol_version: f.genesis.protocol_version,
    };
    let point = || {
        let tip = storage_commit::reconstruct_chain_tip_for_genesis(&disk, &f.genesis).unwrap();
        let head = disk
            .read_shielded()
            .unwrap()
            .pq_pool_v1
            .unwrap_or_default()
            .head;
        Point {
            network: net.clone(),
            height: tip.height,
            block_hash: tip.block_hash,
            state_root: tip.state_root,
            assets: head.assets,
            pairs: head.pairs,
            last_page: head.last_page,
        }
    };
    let mut birthday = point();
    birthday.block_hash = "genesis".into();
    let mut wallet = Wallet::restore(Recovery::new(birthday, 0, 0, 1).unwrap()).unwrap();
    let order = |amount, fee| Order {
        mode: Mode::Exit,
        amount,
        fee,
        asset: f.child.clone(),
        recipient: Some(payer.clone()),
        sequence: None,
        payer: None,
        account_key: None,
        migration: None,
        also_replace: None,
        different_payment: false,
        replace: None,
    };
    let prove = |w: &mut Wallet, order| {
        let result = w.prove(order).unwrap();
        let hash = result["action_hash"].as_str().unwrap().to_owned();
        let wire = w.prepared(Some(&hash)).unwrap();
        let ShieldedAction::PqPaymentV1(payment) = serde_json::from_value(wire).unwrap() else {
            unreachable!()
        };
        w.submitted(&hash).unwrap();
        (hash, payment)
    };
    for (sequence, amount) in [(1, 15), (2, 7)] {
        let (_, p) = prove(
            &mut wallet,
            Order {
                mode: Mode::Shield,
                payer: Some(payer.clone()),
                recipient: None,
                sequence: Some(sequence),
                account_key: Some(keyfile.clone()),
                ..order(amount, 1)
            },
        );
        assert!(f.apply(&[p]).unwrap()[0].accepted);
    }
    let snapshot = private_pool_snapshot(PrivatePoolSnapshotOptions {
        data_dir: f.dir.clone(),
        pool_id: pq::POOL_ID.into(),
        address: None,
        base_asset_id: None,
        quote_asset_id: None,
        inclusion_height: None,
    })
    .unwrap();
    let page = private_pool_page(PrivatePoolPageOptions {
        data_dir: f.dir.clone(),
        snapshot_token: snapshot.snapshot_token,
        section: "pq_pairs".into(),
        cursor: None,
        limit: Some(8),
    })
    .unwrap();
    let rows = f.dir.join("wallet.rows");
    let Some(crate::private_pool_reads::PrivatePoolPageItems::PqPairs(pairs)) = page.items else {
        panic!("missing PQ pairs")
    };
    fs::write(&rows, serde_json::to_vec(&pairs).unwrap()).unwrap();
    fs::set_permissions(&rows, fs::Permissions::from_mode(0o600)).unwrap();
    wallet.sync(point(), &rows).unwrap();
    let request = |payment: &PqPayment| {
        let domain = PrivateActionSubmitDomain {
            chain_id: net.chain_id.clone(),
            genesis_hash: net.genesis_hash.clone(),
            protocol_version: net.protocol_version,
            pool_id: pq::POOL_ID.into(),
            session_scope: "pq-private-spend".into(),
        };
        let action_json =
            serde_json::to_string(&ShieldedAction::PqPaymentV1(payment.clone())).unwrap();
        PrivateActionSubmitRequest {
            intent_id: private_action_intent_id(&domain.intent_domain(), &action_json).unwrap(),
            action_hash: private_action_hash_for_json(&action_json).unwrap(),
            action_json,
            domain,
        }
    };
    let (hash, old) = prove(&mut wallet, order(3, 3));
    let original = request(&old);
    assert!(!private_action_submit(&f.dir, &original).unwrap().existing);
    wallet = serde_json::from_slice(&serde_json::to_vec(&wallet).unwrap()).unwrap();
    assert!(private_action_submit(&f.dir, &original).unwrap().existing);
    assert!(wallet
        .prove(order(3, 3))
        .unwrap_err()
        .to_string()
        .contains("matching payment"));
    // An unrelated payment can consume the other note while the first remains reserved.
    let (other_hash, other) = prove(&mut wallet, order(4, 3));
    assert!(f.apply(&[other]).unwrap()[0].accepted);
    private_action_service_step(
        &f.dir,
        |_| Ok(false),
        |_, _| panic!("stale payment dispatched"),
    )
    .unwrap();
    let refused = private_action_receipt(&f.dir, &original.intent_id).unwrap();
    assert_eq!(refused.state, "rejected");
    assert!(private_action_submit(&f.dir, &original).unwrap().existing);
    let (replacement_hash, next) = prove(
        &mut wallet,
        Order {
            replace: Some(hash.clone()),
            ..order(13, 2)
        },
    );
    assert!(
        !private_action_submit(&f.dir, &request(&next))
            .unwrap()
            .existing
    );
    assert!(f.apply(&[next]).unwrap()[0].accepted);
    // The old numerical fee is valid again. Its note has already been consumed.
    let err = f.apply(&[old]).unwrap_err().to_string();
    assert!(err.contains("serial"), "{err}");
    let ledger = disk.read_ledger().unwrap();
    assert_eq!(ledger.collected_private_fees[&f.child], 5);
    assert_eq!(
        disk.read_shielded()
            .unwrap()
            .pq_pool_v1
            .unwrap()
            .head
            .assets[&f.child]
            .reserve,
        0
    );
    let snapshot = private_pool_snapshot(PrivatePoolSnapshotOptions {
        data_dir: f.dir.clone(),
        pool_id: pq::POOL_ID.into(),
        address: None,
        base_asset_id: None,
        quote_asset_id: None,
        inclusion_height: None,
    })
    .unwrap();
    let page = private_pool_page(PrivatePoolPageOptions {
        data_dir: f.dir.clone(),
        snapshot_token: snapshot.snapshot_token,
        section: "pq_pairs".into(),
        cursor: None,
        limit: Some(8),
    })
    .unwrap();
    let Some(crate::private_pool_reads::PrivatePoolPageItems::PqPairs(all)) = page.items else {
        panic!("missing settled pairs")
    };
    let settled = &all[wallet.point.pairs as usize..];
    fs::write(&rows, serde_json::to_vec(settled).unwrap()).unwrap();
    let before = serde_json::to_vec(&wallet).unwrap();
    let mut incomplete = point();
    incomplete.pairs += 1;
    assert!(wallet.sync(incomplete, &rows).is_err());
    assert!(
        serde_json::to_vec(&wallet).unwrap() == before,
        "an incomplete scan changed the wallet or an attempt"
    );
    wallet.sync(point(), &rows).unwrap();
    let reopened: Wallet = serde_json::from_slice(&serde_json::to_vec(&wallet).unwrap()).unwrap();
    let attempts = reopened.balance()["attempts"].as_array().unwrap().clone();
    for (id, phase) in [
        (&hash, "replaced"),
        (&replacement_hash, "final"),
        (&other_hash, "final"),
    ] {
        let stored = attempts
            .iter()
            .find(|a| a["action_hash"] == id.as_str())
            .unwrap();
        assert_eq!(stored["phase"], phase);
    }
    fs::remove_dir_all(&f.dir).unwrap();
}
