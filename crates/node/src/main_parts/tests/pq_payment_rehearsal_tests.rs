#[cfg(test)]
mod pq_payment_rehearsal_tests {
    use super::pq_payment_local_chain_tests::{
        assert_inactive_proposal, exercise_active_traffic, l1_action, proposal, CHAIN,
    };
    use super::private_action_six_validator_tests::{
        fleet_on_chain, governance_batch, FleetOptions, ATOMIC_ACTIVATION,
    };
    use postfiat_crypto_provider::{bytes_to_hex, hex_to_bytes};
    use postfiat_execution::genesis_hash;
    use postfiat_node::*;
    use postfiat_storage::NodeStore;
    use postfiat_types::{pq_payment, BatchArchiveEntry, ShieldedAction, ShieldedActionBatch};
    use std::{
        fs,
        path::{Path, PathBuf},
        process::Command,
        time::Instant,
    };

    use super::pq_rehearsal_inventory::inventory;
    use pft_binius_shielded::payment::ShieldedWallet;
    use rand::{rngs::StdRng, SeedableRng};

    fn recover_output(dir: &Path, asset: &str, index: u32, wallet: &ShieldedWallet, amount: u64) {
        let pool = NodeStore::new(dir)
            .read_shielded()
            .unwrap()
            .pq_pool_v1
            .unwrap();
        let id = pool.indexes.page_for_output(asset, 0, index).unwrap();
        let pair = postfiat_storage::pq_pool::read_page(dir, id).unwrap();
        let out = &pair.outputs[(index - pair.first_index) as usize];
        let coin = hex_to_bytes(&out.commitment).unwrap().try_into().unwrap();
        let envelope = out.envelope().unwrap().try_into().unwrap();
        assert_eq!(wallet.try_scan_output(&coin, &envelope).unwrap().0, amount);
        let serial = pft_binius_shielded::shake256::nullifier(&wallet.spend_secret, &coin);
        assert!(!pool.indexes.is_spent(&bytes_to_hex(&serial)));
    }

    // Bind the compressed prefix to proofs for this chain. The final append
    // carries through every occupied left subtree. Full Go reconstruction and
    // the node's pair/serial transition are separate regression gates.
    fn check_rollover(chain: pq_payment::Chain<'_>) {
        use pft_binius_shielded::shake256::{merkle_node, MerkleTree24};
        let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/pft-binius-shielded-pq/rollover-vectors");
        let verify = |name: &str| {
            let p = pq_payment::PqPayment::from_bytes(
                fs::read(dir.join(format!("{name}.bin"))).unwrap(),
            )
            .unwrap();
            postfiat_privacy_binius::verify_l1(&p, chain).unwrap()
        };
        let old = verify("old-spend");
        for epoch in 0..2 {
            let mut record: serde_json::Value = serde_json::from_slice(
                &fs::read(dir.join(format!("frontier-{epoch}.json"))).unwrap(),
            )
            .unwrap();
            let expected = record.as_object_mut().unwrap().remove("terminal").unwrap();
            let tree: pq_payment::PqEpoch = serde_json::from_value(record).unwrap();
            let tx = verify(&format!("boundary-{epoch}"));
            assert_eq!(tree.count, pq_payment::PQ_CAPACITY - 2);
            assert_eq!(tree.frontier_root().unwrap(), tx.anchor);
            let mut root = merkle_node(&tx.coin_out0, &tx.coin_out1);
            for left in &tree.frontier[1..] {
                let sibling = hex_to_bytes(left.as_ref().unwrap())
                    .unwrap()
                    .try_into()
                    .unwrap();
                root = merkle_node(&sibling, &root);
            }
            assert_eq!(bytes_to_hex(&root), expected.as_str().unwrap());
            if epoch == 0 {
                assert_eq!(root, old.anchor);
            }
            println!(
                "ROLLOVER epoch={epoch} leaves={} terminal={}",
                pq_payment::PQ_CAPACITY,
                bytes_to_hex(&root)
            );
        }
        let moved = MerkleTree24::from_leaves(&[old.coin_out0, old.coin_out1]);
        assert_eq!(verify("successor-spend").anchor, moved.root());
    }

    fn next_native_deposit(dir: &Path) -> ShieldedActionBatch {
        use pft_binius_shielded::payment::{create_profiled_tx, SpendInput};
        use postfiat_privacy_binius::{sign_l1_shield, MerkleTree24, ShieldOutputIntent};
        let store = NodeStore::new(dir);
        let genesis = store.read_genesis().unwrap();
        let chain = pq_payment::Chain {
            id: &genesis.chain_id,
            genesis: hex_to_bytes(&genesis_hash(&genesis))
                .unwrap()
                .try_into()
                .unwrap(),
            protocol: genesis.protocol_version,
        };
        let ShieldedAction::PqPaymentV1(template) = l1_action("pft-shield") else {
            unreachable!()
        };
        let mut terms = template.parts().unwrap().0;
        let source = terms.account.as_mut().unwrap();
        source.sequence = store
            .read_ledger()
            .unwrap()
            .account(&format!("pf{}", bytes_to_hex(&source.id)))
            .unwrap()
            .sequence
            + 1;
        let key = postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[7; 32]);
        let mut rng = StdRng::seed_from_u64(829);
        let wallet = ShieldedWallet::from_seed(&mut rng);
        let (tx, r0, r1) = create_profiled_tx(
            &MerkleTree24::new(),
            SpendInput::Dummy,
            SpendInput::Dummy,
            &wallet.stealth_address(),
            9,
            &wallet.stealth_address(),
            0,
            10,
            0,
            [0; 32],
            1,
            None,
            Some(&|raw| terms.statement_context(chain, raw)),
            &mut rng,
        )
        .unwrap();
        let intents = [
            ShieldOutputIntent {
                owner: &wallet.addr,
                value: 9,
                blinder: &r0,
                envelope: &tx.env0,
            },
            ShieldOutputIntent {
                owner: &wallet.addr,
                value: 0,
                blinder: &r1,
                envelope: &tx.env1,
            },
        ];
        let payment = sign_l1_shield(
            chain,
            &terms,
            &tx.to_bytes(),
            10,
            1,
            intents,
            &key.private_key,
        )
        .unwrap();
        build_atomic_shielded_action_batch(&genesis, vec![ShieldedAction::PqPaymentV1(payment)])
            .unwrap()
    }

    fn certified_payments(dir: &Path) -> Vec<(u64, String, String)> {
        let store = NodeStore::new(dir);
        let genesis = store.read_genesis().unwrap();
        let hash = hex_to_bytes(&genesis_hash(&genesis))
            .unwrap()
            .try_into()
            .unwrap();
        let chain = pq_payment::Chain {
            id: &genesis.chain_id,
            genesis: hash,
            protocol: genesis.protocol_version,
        };
        let receipts = store.read_receipts().unwrap();
        let blocks = store.read_blocks().unwrap();
        let mut history = Vec::new();
        let mut largest = 0;
        let mut largest_body = 0;
        let mut worst = 0.0_f64;
        for entry in store.read_batch_archive().unwrap().batches {
            if entry.batch_kind != "shielded" {
                continue;
            }
            let batch: ShieldedActionBatch = serde_json::from_str(&entry.payload_json).unwrap();
            let block = blocks
                .blocks
                .iter()
                .find(|b| b.header.batch_id == batch.batch_id)
                .unwrap();
            let cert = &block.header.certificate;
            assert_eq!((cert.quorum, cert.validators.len()), (5, 6));
            assert!(cert.votes.iter().filter(|v| v.accept).count() >= cert.quorum);
            let mut charge = 0;
            let mut proofs = 0;
            for (i, action) in batch.actions.iter().enumerate() {
                let ShieldedAction::PqPaymentV1(p) = action else {
                    continue;
                };
                proofs += 1;
                let id = p.digest();
                let stored = fs::read(dir.join("pq_bodies_v1").join(&id)).unwrap();
                assert_eq!(stored, p.bytes());
                let start = Instant::now();
                postfiat_privacy_binius::verify_l1(p, chain).unwrap();
                worst = worst.max(start.elapsed().as_secs_f64() * 1000.0);
                let logical = BatchArchiveEntry::pq_archived_payment_bytes(
                    &batch.batch_id,
                    &entry.payload_hash,
                    batch.atomic,
                    batch.actions.len() > 1,
                    &serde_json::json!({"pq_body_v1":id}),
                    stored.len(),
                )
                .unwrap();
                let size = pq_payment::checked_archive_size(
                    logical,
                    postfiat_types::PQ_REHEARSAL_ARCHIVE_OVERHEAD,
                )
                .unwrap();
                largest = largest.max(size);
                largest_body = largest_body.max(stored.len());
                charge += size;
                let receipt_id = &block.receipt_ids[i];
                assert!(receipts
                    .iter()
                    .any(|r| r.tx_id == *receipt_id && r.accepted));
                let asset = bytes_to_hex(&p.parts().unwrap().0.asset);
                println!(
                    "RECEIPT h={} asset={asset} id={receipt_id} body={} charged={size}",
                    block.header.height,
                    stored.len()
                );
                history.push((block.header.height, asset, receipt_id.clone()));
            }
            assert!(
                charge <= postfiat_types::PQ_REHEARSAL_BYTES
                    && proofs <= postfiat_types::PQ_REHEARSAL_PROOFS
            );
            println!(
                "BLOCK h={} proofs={proofs} charged={charge} votes={} root={}",
                block.header.height,
                cert.votes.len(),
                block.header.state_root
            );
        }
        println!("READBACK receipts={} largest_body_bytes={largest_body} largest_charged_bytes={largest} worst_uncached_verify_l1_ms={worst:.3}",history.len());
        history
    }

    fn cli(bin: &Path, dir: &Path, args: &[&str]) -> serde_json::Value {
        let output = Command::new(bin)
            .args(args)
            .arg("--data-dir")
            .arg(dir)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{bin:?} {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
                .lines()
                .next()
                .unwrap_or("no diagnostic")
        );
        serde_json::from_slice(&output.stdout).unwrap()
    }

    #[test]
    fn six_validators_pause_reopen_and_replay_the_full_history() {
        let fleet = fleet_on_chain(
            CHAIN,
            FleetOptions {
                pricing: false,
                max_nav_age_blocks: None,
                extra_asset: false,
            },
        );
        let peer = &fleet.data_dirs[1];
        let store = NodeStore::new(peer);
        let genesis = store.read_genesis().unwrap();
        let before = fleet.root.join("before-h");
        exercise_active_traffic(&fleet, |_| {
            assert_eq!(fleet.height(), ATOMIC_ACTIVATION - 1);
            let snapshot = fleet.root.join("before-h-snapshot");
            export_snapshot(SnapshotExportOptions {
                data_dir: peer.clone(),
                snapshot_dir: snapshot.clone(),
            })
            .unwrap();
            import_snapshot(SnapshotImportOptions {
                data_dir: before.clone(),
                snapshot_dir: snapshot,
                node_id: None,
            })
            .unwrap();
        });
        let ids = (0..6).map(|i| format!("validator-{i}")).collect::<Vec<_>>();
        let holder_wallet = super::pq_rehearsal_issue::shield_source_series(&fleet, &ids);
        let active = certified_payments(peer);
        assert_eq!(active.len(), 7);
        assert!(active
            .iter()
            .any(|(_, asset, _)| asset == postfiat_types::PQ_REHEARSAL_ISSUE));
        let pause = fleet.height() + 1;
        let eligible = next_native_deposit(peer);
        let candidate = |height| {
            proposal(
                &fleet.data_dirs[fleet.proposer_index(height)],
                &fleet.root,
                &eligible,
            )
        };
        // Both candidates extend P-1. The deposit is valid at P, where the
        // fleet chooses to finalize the signed pause.
        propose_batch_with_timings(candidate(pause)).expect("valid before pause");
        let file = governance_batch(
            &fleet.data_dirs[0],
            &fleet.root,
            &ids,
            postfiat_types::PQ_REHEARSAL_V1,
            0,
            pause,
        );
        fleet.round("governance", file);
        assert_eq!(fleet.height(), pause);
        assert_eq!(
            propose_batch_with_timings(candidate(pause + 1))
                .unwrap_err()
                .to_string(),
            "pq_settlement_inactive"
        );
        assert_inactive_proposal(&fleet, &genesis, pause + 1);
        fleet.transparent_round();
        assert_inactive_proposal(&fleet, &genesis, pause + 2);
        let expected = verify_blocks(NodeOptions {
            data_dir: peer.clone(),
        })
        .unwrap();
        assert!(expected.verified);
        let signed = fleet.root.join("signed-at-pause");
        let restored = fleet.root.join("restarted");
        let publisher = fleet.data_dirs[0].join("faucet_key.json");
        let trusted = fleet.root.join("publisher.public.json");
        export_snapshot_publisher_public_key(SnapshotPublisherKeyExportOptions {
            publisher_key_file: publisher.clone(),
            public_key_file: trusted.clone(),
        })
        .unwrap();
        export_signed_snapshot(SignedSnapshotExportOptions {
            data_dir: peer.clone(),
            snapshot_dir: signed.clone(),
            publisher_key_file: publisher,
        })
        .unwrap();
        import_signed_snapshot(SignedSnapshotImportOptions {
            data_dir: restored.clone(),
            snapshot_dir: signed,
            trusted_publisher_key_file: trusted,
            node_id: None,
        })
        .unwrap();
        for path in fleet.data_dirs.iter().chain([&restored]) {
            let replay = verify_blocks(NodeOptions {
                data_dir: path.clone(),
            })
            .unwrap();
            assert!(replay.verified);
            assert_eq!(
                (replay.block_count, &replay.tip_hash, &replay.state_root),
                (
                    expected.block_count,
                    &expected.tip_hash,
                    &expected.state_root
                )
            );
        }
        assert_eq!(certified_payments(&restored), active);
        assert_eq!(
            store.read_shielded().unwrap().pq_pool_v1.unwrap().head,
            NodeStore::new(&restored)
                .read_shielded()
                .unwrap()
                .pq_pool_v1
                .unwrap()
                .head
        );
        let native_wallet = ShieldedWallet::from_seed(&mut StdRng::seed_from_u64(42));
        recover_output(
            &restored,
            pq_payment::NATIVE_ASSET_HEX,
            4,
            &native_wallet,
            99,
        );
        recover_output(
            &restored,
            postfiat_types::PQ_REHEARSAL_ISSUE,
            0,
            &holder_wallet,
            100,
        );
        check_rollover(pq_payment::Chain {
            id: &genesis.chain_id,
            genesis: hex_to_bytes(&genesis_hash(&genesis))
                .unwrap()
                .try_into()
                .unwrap(),
            protocol: genesis.protocol_version,
        });
        inventory(&restored);
        let ledger = store.read_ledger().unwrap();
        let shielded = store.read_shielded().unwrap();
        let burned: u128 = store
            .read_receipts()
            .unwrap()
            .iter()
            .map(|r| u128::from(r.fee_burned))
            .sum();
        assert_eq!(
            native_pft_live_total(&ledger, &shielded).unwrap() + burned,
            u128::from(genesis.expected_native_supply_atoms())
        );
        println!(
            "FRESH H={ATOMIC_ACTIVATION} pause={pause} first_blocked={} height={} tip={} root={}",
            pause + 1,
            expected.block_count,
            expected.tip_hash,
            expected.state_root
        );
        // Both executables are supplied together by the release rehearsal. A normal
        // single-revision unit run still verifies every replica and signed restore.
        if let Some(post) = std::env::var_os("PQ_POST_H_NODE") {
            let post = PathBuf::from(post);
            let pre = PathBuf::from(std::env::var_os("PQ_PRE_H_NODE").expect("pre-upgrade node"));
            for path in [peer, &restored] {
                let cold = cli(&post, path, &["verify-blocks"]);
                assert_eq!(cold["verified"], true);
                assert_eq!(cold["block_count"], expected.block_count);
                assert_eq!(cold["tip_hash"], expected.tip_hash);
                assert_eq!(cold["state_root"], expected.state_root);
            }
            println!("COLD START original replica and signed import independently verified all {} blocks", expected.block_count);
            let args = ["blocks", "--from-height", "1", "--limit", "512"];
            let rows = cli(&post, &before, &args);
            assert_eq!(rows.as_array().unwrap().len() as u64, ATOMIC_ACTIVATION - 1);
            assert_eq!(cli(&pre, &before, &args), rows);
            let current = cli(&post, &before, &["verify-blocks"]);
            assert_eq!(cli(&pre, &before, &["verify-blocks"]), current);
            println!(
                "CROSS-BUILD {} pre-H blocks replayed, tip={}, root={}",
                current["block_count"], current["tip_hash"], current["state_root"]
            );
        }
        fs::remove_dir_all(fleet.root).unwrap();
    }
}
