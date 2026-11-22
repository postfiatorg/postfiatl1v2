#[cfg(test)]
mod pq_payment_local_chain_tests {
    use super::private_action_six_validator_tests::{
        fleet_on_chain, FleetOptions, ATOMIC_ACTIVATION,
    };
    use postfiat_crypto_provider::{bytes_to_hex, hash_hex, hex_to_bytes};
    use postfiat_node::{
        batch_archive_payload_hash, build_atomic_shielded_action_batch, create_block_vote, init,
        propose_batch_with_timings, verify_blocks, BatchProposalOptions, BlockVoteOptions,
        InitOptions, NodeOptions, VALIDATOR_KEYS_FILE,
    };
    use postfiat_storage::NodeStore;
    use postfiat_types::{Genesis, ShieldedAction, ShieldedActionBatch};
    use sha2::{Digest, Sha384};
    use std::{fs, path::Path, process::Command};

    const CHAIN: &str = "postfiat-pfsh02-m1-disposable";
    const HOP: &[u8] =
        include_bytes!("../../../../../tools/pft-binius-shielded-pq/payments/tx_hop1.bin");

    fn batch(genesis: &Genesis, wire: &[u8], atomic: bool) -> ShieldedActionBatch {
        let actions = vec![ShieldedAction::PqFixtureHop {
            payment_hex: bytes_to_hex(wire),
        }];
        if atomic {
            return build_atomic_shielded_action_batch(genesis, actions).unwrap();
        }
        let encoded = serde_json::to_vec(&(
            &genesis.chain_id,
            postfiat_execution::genesis_hash(genesis),
            genesis.protocol_version,
            "shielded",
            &actions,
        ))
        .unwrap();
        ShieldedActionBatch::new(
            hash_hex("postfiat.shielded_action_batch.v1", &encoded),
            actions,
        )
    }

    fn proposal(data_dir: &Path, dir: &Path, batch: &ShieldedActionBatch) -> BatchProposalOptions {
        let batch_file = dir.join(format!("{}.json", batch.batch_id));
        fs::write(&batch_file, serde_json::to_vec(batch).unwrap()).unwrap();
        BatchProposalOptions {
            data_dir: data_dir.to_owned(),
            verify_block_log: false,
            batch_kind: Some("shielded".into()),
            batch_file,
            proposal_file: dir.join(format!("{}.proposal.json", batch.batch_id)),
            view: Some(0),
            timeout_certificate_file: None,
            key_file: None,
            validator_id: None,
        }
    }

    #[test]
    fn l1_authorized_archive_finalizes_and_replays_from_snapshot() {
        use postfiat_node::{
            export_snapshot, import_snapshot, SnapshotExportOptions, SnapshotImportOptions,
        };
        use postfiat_types::pq_payment::PqPayment;
        let fleet = fleet_on_chain(
            CHAIN,
            FleetOptions {
                pricing: false,
                max_nav_age_blocks: None,
                extra_asset: false,
            },
        );
        while fleet.height() + 1 < ATOMIC_ACTIVATION {
            fleet.transparent_round();
        }
        let peer = &fleet.data_dirs[1];
        let store = NodeStore::new(peer);
        let genesis = store.read_genesis().unwrap();
        let bytes = include_bytes!(
            "../../../../../tools/pft-binius-shielded-pq/l1-vectors/pfusdc-exit.bin"
        );
        let payment = PqPayment::from_bytes(bytes.to_vec()).unwrap();
        let id = payment.digest();
        let actions = vec![ShieldedAction::PqPaymentV1(payment.clone())];
        let encoded = serde_json::to_vec(&(
            &genesis.chain_id,
            postfiat_execution::genesis_hash(&genesis),
            genesis.protocol_version,
            "shielded",
            &actions,
        ))
        .unwrap();
        let batch = ShieldedActionBatch::new(
            hash_hex("postfiat.shielded_action_batch.v1", &encoded),
            actions,
        );
        let proposer = &fleet.data_dirs[fleet.proposer_index(fleet.height() + 1)];
        let mut corrupt = bytes.to_vec();
        *corrupt.last_mut().unwrap() ^= 1;
        let bad = build_atomic_shielded_action_batch(
            &genesis,
            vec![ShieldedAction::PqPaymentV1(
                PqPayment::from_bytes(corrupt).unwrap(),
            )],
        )
        .unwrap();
        assert!(
            propose_batch_with_timings(proposal(proposer, &fleet.root, &bad))
                .unwrap_err()
                .to_string()
                .contains("PQ L1 verification failed")
        );
        let mut foreign = genesis.clone();
        foreign.chain_id.push('x');
        assert!(
            postfiat_node::verify_shielded_action_batch_id(&foreign, &batch)
                .unwrap_err()
                .to_string()
                .contains("development genesis")
        );
        let opts = proposal(proposer, &fleet.root, &batch);
        let proposed = propose_batch_with_timings(opts.clone()).unwrap();
        let bad_opts = proposal(proposer, &fleet.root, &bad);
        let mut direct = proposed.proposal;
        direct.batch_id = bad.batch_id.clone();
        direct.payload_hash = batch_archive_payload_hash(
            &genesis,
            "shielded",
            &bad.batch_id,
            &serde_json::to_string(&bad).unwrap(),
        )
        .unwrap();
        fs::write(
            &bad_opts.proposal_file,
            serde_json::to_vec(&direct).unwrap(),
        )
        .unwrap();
        let vote_file = fleet.root.join("bad-l1.vote");
        let vote = BlockVoteOptions {
            data_dir: peer.clone(),
            verify_block_log: false,
            key_file: peer.join(VALIDATOR_KEYS_FILE),
            validator_id: Some("validator-1".into()),
            batch_file: Some(bad_opts.batch_file.clone()),
            proposal_file: Some(bad_opts.proposal_file),
            timeout_certificate_file: None,
            block_height: Some(fleet.height() + 1),
            vote_file: vote_file.clone(),
        };
        assert!(create_block_vote(vote.clone())
            .unwrap_err()
            .to_string()
            .contains("PQ L1 verification failed"));
        fs::remove_file(bad_opts.batch_file).unwrap();
        assert!(create_block_vote(vote).is_err());
        assert!(!vote_file.exists());
        fleet.round("shielded", opts.batch_file);
        let entry = store.read_batch_archive().unwrap().batches.pop().unwrap();
        assert_eq!(entry.batch_id, batch.batch_id);
        let history = serde_json::to_value(&entry).unwrap();
        assert!(history["pq_reference_v1"].as_str().unwrap().contains(&id));
        assert_eq!(
            serde_json::from_value::<postfiat_types::BatchArchiveEntry>(history).unwrap(),
            entry
        );
        assert_eq!(
            serde_json::from_str::<ShieldedActionBatch>(&entry.payload_json).unwrap(),
            batch
        );
        assert!(
            verify_blocks(NodeOptions {
                data_dir: peer.clone()
            })
            .unwrap()
            .verified
        );
        let block = store.read_blocks().unwrap().blocks.pop().unwrap();
        assert!(
            block
                .header
                .certificate
                .votes
                .iter()
                .filter(|v| v.accept)
                .count()
                >= 5
        );
        // Settlement is a later milestone. The development block archives a
        // verified body and explicitly records that no balance moved.
        assert_eq!(
            store.read_receipts().unwrap().last().unwrap().code,
            "pq_settlement_inactive"
        );
        let snapshot = fleet.root.join("pq-snapshot");
        let restored = fleet.root.join("pq-restored");
        export_snapshot(SnapshotExportOptions {
            data_dir: peer.clone(),
            snapshot_dir: snapshot.clone(),
        })
        .unwrap();
        let portable: serde_json::Value =
            serde_json::from_slice(&fs::read(snapshot.join("batch_archive.json")).unwrap())
                .unwrap();
        assert_eq!(portable["pq_bodies_v1"].as_object().unwrap().len(), 1);
        assert!(portable["pq_bodies_v1"].get(&id).is_some());
        import_snapshot(SnapshotImportOptions {
            data_dir: restored.clone(),
            snapshot_dir: snapshot,
            node_id: None,
        })
        .unwrap();
        assert!(
            verify_blocks(NodeOptions {
                data_dir: restored.clone()
            })
            .unwrap()
            .verified
        );
        let body = restored.join("pq_bodies_v1").join(id);
        assert_eq!(fs::read(&body).unwrap(), bytes);
        let mut bad = bytes.to_vec();
        *bad.last_mut().unwrap() ^= 1;
        fs::write(&body, bad).unwrap();
        assert!(verify_blocks(NodeOptions {
            data_dir: restored.clone()
        })
        .is_err());
        fs::remove_file(body).unwrap();
        assert!(verify_blocks(NodeOptions { data_dir: restored }).is_err());
        println!("Verified authorized L1 archive: {} body bytes, certified height {}, snapshot restored, missing and mismatched bodies rejected",bytes.len(),block.header.height);
    }

    #[test]
    fn pq_funded_pair_survives_journal_recovery_snapshot_and_genesis_replay() {
        use postfiat_node::{
            create_transfer_batch, export_snapshot, import_snapshot, BatchTransferOptions,
            SnapshotExportOptions, SnapshotImportOptions,
        };
        use postfiat_types::pq_payment::PqPayment;
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
        let mint = PqPayment::from_bytes(
            include_bytes!("../../../../../tools/pft-binius-shielded-pq/l1-vectors/pft-shield.bin")
                .to_vec(),
        )
        .unwrap();
        let pay = PqPayment::from_bytes(
            include_bytes!("../../../../../tools/pft-binius-shielded-pq/l1-vectors/pft-pay.bin")
                .to_vec(),
        )
        .unwrap();
        let payer = format!(
            "pf{}",
            bytes_to_hex(&mint.parts().unwrap().0.account.unwrap().id)
        );
        let file = fleet.root.join("pq-funding.json");
        create_transfer_batch(BatchTransferOptions {
            data_dir: peer.clone(),
            key_file: Some(fleet.data_dirs[0].join("faucet_key.json")),
            to: payer.clone(),
            amount: 1000,
            batch_file: file.clone(),
        })
        .unwrap();
        fleet.round("transparent", file);
        while fleet.height() + 1 < ATOMIC_ACTIVATION {
            fleet.transparent_round();
        }
        let atomic = |payments: &[&PqPayment]| {
            build_atomic_shielded_action_batch(
                &genesis,
                payments
                    .iter()
                    .map(|p| ShieldedAction::PqPaymentV1((*p).clone()))
                    .collect(),
            )
            .unwrap()
        };
        let propose = |batch: &ShieldedActionBatch| {
            proposal(
                &fleet.data_dirs[fleet.proposer_index(fleet.height() + 1)],
                &fleet.root,
                batch,
            )
        };
        let balance = store
            .read_ledger()
            .unwrap()
            .account(&payer)
            .unwrap()
            .balance;
        // An internal spend conflicts even when the first action funded its root
        // earlier in this block. The failed triple must publish no partial write.
        let conflict = atomic(&[&mint, &pay, &pay]);
        let err = propose_batch_with_timings(propose(&conflict)).unwrap_err();
        assert!(err.to_string().contains("pq_spent_serial"), "{err}");
        assert!(store.read_shielded().unwrap().pq_pool_v1.is_none());
        assert_eq!(
            store
                .read_ledger()
                .unwrap()
                .account(&payer)
                .unwrap()
                .balance,
            balance
        );
        let base = fleet.root.join("pre-pq");
        export_snapshot(SnapshotExportOptions {
            data_dir: peer.clone(),
            snapshot_dir: base.clone(),
        })
        .unwrap();
        let crashed = fleet.root.join("interrupted");
        let prefix = import_snapshot(SnapshotImportOptions {
            data_dir: crashed.clone(),
            snapshot_dir: base,
            node_id: None,
        })
        .unwrap();
        postfiat_node::rebuild_transactional_storage(postfiat_node::StorageMigrationOptions {
            data_dir: crashed.clone(),
            output_dir: crashed.join("pq-generation"),
            expected_tip: prefix.block_tip_hash,
            expected_state_root: prefix.state_root,
            verify_only: false,
        })
        .unwrap();
        let batch = atomic(&[&mint, &pay]);
        fleet.round("shielded", propose(&batch).batch_file);
        let after = store.read_shielded().unwrap();
        let post = after.pq_pool_v1.as_ref().unwrap();
        let asset = "00".repeat(48);
        assert_eq!(post.head.pairs, 2);
        assert_eq!(post.head.assets[&asset].reserve, 98);
        let ledger = store.read_ledger().unwrap();
        assert_eq!(ledger.account(&payer).unwrap().balance, balance - 100);
        assert_eq!(ledger.account(&payer).unwrap().sequence, 1);
        let conflict = atomic(&[&pay]);
        let err = propose_batch_with_timings(propose(&conflict)).unwrap_err();
        assert!(err.to_string().contains("pq_spent_serial"), "{err}");
        let receipts = store.read_receipts().unwrap();
        let delta = &receipts[receipts.len() - 2..];
        assert!(delta.iter().all(|r| r.accepted && r.fee_burned == 1));
        let block = store.read_blocks().unwrap().blocks.pop().unwrap();
        assert!(
            block
                .header
                .certificate
                .votes
                .iter()
                .filter(|v| v.accept)
                .count()
                >= 5
        );
        // Recreate the durable crash prefix: pages and journal reached disk,
        // followed by the source debit, before the shielded head or block tip.
        postfiat_storage::pq_pool::audit(peer, &after, Some(&crashed)).unwrap();
        let cut = NodeStore::new(&crashed);
        let journal = serde_json::json!({
            "schema":"postfiat-ordered-commit-delta-journal-v1","height":block.header.height,
            "ledger":ledger,"shielded":after,"governance":null,"bridge":null,
            "receipt_delta":delta,"ordered_batch_id":batch.batch_id,
            "archive_entry":store.read_batch_archive().unwrap().batches.last().unwrap(),
            "block":block,"validator_registry":null});
        cut.write_ordered_commit_journal(&journal).unwrap();
        cut.write_ledger(&ledger).unwrap();
        drop(cut);
        let snapshot = fleet.root.join("final-pq");
        let restored = fleet.root.join("restored-pq");
        let publisher = fleet.data_dirs[0].join("faucet_key.json");
        let trusted = fleet.root.join("trusted-publisher.json");
        postfiat_node::export_snapshot_publisher_public_key(
            postfiat_node::SnapshotPublisherKeyExportOptions {
                publisher_key_file: publisher.clone(),
                public_key_file: trusted.clone(),
            },
        )
        .unwrap();
        postfiat_node::export_signed_snapshot(postfiat_node::SignedSnapshotExportOptions {
            data_dir: peer.clone(),
            snapshot_dir: snapshot.clone(),
            publisher_key_file: publisher,
        })
        .unwrap();
        postfiat_node::import_signed_snapshot(postfiat_node::SignedSnapshotImportOptions {
            data_dir: restored.clone(),
            snapshot_dir: snapshot.clone(),
            trusted_publisher_key_file: trusted,
            node_id: None,
        })
        .unwrap();
        for dir in [peer, &crashed, &restored] {
            let check = verify_blocks(NodeOptions {
                data_dir: dir.clone(),
            })
            .unwrap();
            assert_eq!(check.state_root, block.header.state_root);
            assert_eq!(check.tip_hash, block.header.block_hash);
            let disk = NodeStore::new(dir);
            assert_eq!(disk.read_receipts().unwrap(), receipts);
            assert_eq!(disk.read_ledger().unwrap(), ledger);
            let p = disk.read_shielded().unwrap().pq_pool_v1.unwrap();
            assert_eq!(p.head, post.head);
            for (i, wire) in [&mint, &pay].iter().enumerate() {
                let inner = wire.parts().unwrap().2;
                for slot in 0..2 {
                    let pos = (2 * i + slot) as u32;
                    let id = p.indexes.page_for_output(&asset, 0, pos).unwrap();
                    let page = postfiat_storage::pq_pool::read_page(dir, id).unwrap();
                    assert_eq!(page.epoch, 0);
                    assert_eq!(page.first_index + (slot as u32), pos);
                    assert_eq!(page.height, block.header.height);
                    let start = 304 + 1670 * slot;
                    assert_eq!(
                        page.outputs[slot].envelope().unwrap(),
                        inner[start..start + 1670]
                    );
                    assert_eq!(
                        page.outputs[slot].commitment,
                        bytes_to_hex(&inner[208 + 48 * slot..256 + 48 * slot])
                    );
                    assert!(p
                        .indexes
                        .is_spent(&bytes_to_hex(&inner[112 + 48 * slot..160 + 48 * slot])));
                }
            }
        }
        let db = NodeStore::new(&crashed).transactional_store().unwrap();
        let mut redb_state = db.shielded().unwrap().unwrap();
        postfiat_storage::pq_pool::hydrate(&crashed, &mut redb_state).unwrap();
        assert_eq!(redb_state, after);
        assert_eq!(
            db.meta().unwrap().finalized_block_hash,
            block.header.block_hash
        );
        let source = NodeStore::new(&restored);
        let mut missing = after.clone();
        missing.pq_pool_v1 = None;
        source.write_shielded(&missing).unwrap();
        assert!(verify_blocks(NodeOptions { data_dir: restored }).is_err());
        let id = &post.head.last_page;
        fs::remove_file(snapshot.join("pq_pool_pages_v1").join(&id[..2]).join(id)).unwrap();
        let dest = fleet.root.join("rejected");
        assert!(import_snapshot(SnapshotImportOptions {
            data_dir: dest.clone(),
            snapshot_dir: snapshot,
            node_id: None
        })
        .is_err());
        assert!(!dest.exists());
        println!("Height {} certified two ordered payments, four serials, four outputs, reserve 98; journal restart, snapshot and genesis replay agree",block.header.height);
    }

    #[test]
    fn six_validators_finalize_and_replay_the_published_hop() {
        let fleet = fleet_on_chain(
            CHAIN,
            FleetOptions {
                pricing: false,
                max_nav_age_blocks: None,
                extra_asset: false,
            },
        );
        while fleet.height() + 1 < ATOMIC_ACTIVATION {
            fleet.transparent_round();
        }
        let height = fleet.height() + 1;
        let proposer = fleet.proposer_index(height);
        let peer = (proposer + 1) % fleet.data_dirs.len();
        let peer_dir = &fleet.data_dirs[peer];
        let store = NodeStore::new(peer_dir);
        let genesis = store.read_genesis().unwrap();
        let pre = store.read_shielded().unwrap().pq_fixture.unwrap();
        assert_eq!(pre.reserve, 54_743);
        let ledger =
            include_bytes!("../../../../../tools/pft-binius-shielded-pq/payments/ledger_hop1.json");
        assert_eq!(bytes_to_hex(&Sha384::digest(ledger)), "2b0a19667f50562ff66c075a3a323c604c37115e1f77eb3a985c6ff05b45768775d68ece9bd167af2dabbe05181b7731");
        let source: serde_json::Value = serde_json::from_slice(ledger).unwrap();
        assert_eq!(serde_json::json!(pre.commitments), source["coins"]);
        assert_eq!(serde_json::json!(pre.spent_serials), source["spent_sn"]);
        assert_eq!(pre.root, bytes_to_hex(&HOP[8..56]));

        let good = batch(&genesis, HOP, true);
        let opts = proposal(&fleet.data_dirs[proposer], &fleet.root, &good);
        let good_file = opts.batch_file.clone();
        let proposed = propose_batch_with_timings(opts).unwrap();
        println!(
            "In-validator single-hop execution (decode, proof, ledger checks and append): {:.3} ms",
            proposed
                .timings
                .shielded_breakdown
                .as_ref()
                .unwrap()
                .state_exec_ms
        );

        let mut changed = HOP.to_vec();
        *changed.last_mut().unwrap() ^= 1; // final Merkle sibling in the proof
        let bad = batch(&genesis, &changed, false);
        let bad_opts = proposal(&fleet.data_dirs[proposer], &fleet.root, &bad);
        assert!(propose_batch_with_timings(bad_opts.clone())
            .unwrap_err()
            .to_string()
            .contains("pq_fixture_invalid_proof"));

        // Bypass proposal construction. Supply a peer with the bad body's own ID and payload
        // hash and claim that it executes with the original receipt. It must rerun the proof.
        let mut direct = proposed.proposal;
        direct.batch_id = bad.batch_id.clone();
        direct.payload_hash = batch_archive_payload_hash(
            &genesis,
            "shielded",
            &bad.batch_id,
            &serde_json::to_string(&bad).unwrap(),
        )
        .unwrap();
        fs::write(
            &bad_opts.proposal_file,
            serde_json::to_vec(&direct).unwrap(),
        )
        .unwrap();
        let vote_file = fleet.root.join("changed-proof.vote");
        let error = create_block_vote(BlockVoteOptions {
            data_dir: peer_dir.clone(),
            verify_block_log: false,
            key_file: peer_dir.join(VALIDATOR_KEYS_FILE),
            validator_id: Some(format!("validator-{peer}")),
            batch_file: Some(bad_opts.batch_file),
            proposal_file: Some(bad_opts.proposal_file),
            timeout_certificate_file: None,
            block_height: Some(height),
            vote_file: vote_file.clone(),
        })
        .unwrap_err();
        assert!(
            error.to_string().contains("pq_fixture_invalid_proof"),
            "{error}"
        );
        assert!(!vote_file.exists());
        assert_eq!(
            store.read_shielded().unwrap().pq_fixture.as_ref(),
            Some(&pre)
        );

        fleet.round("shielded", good_file);
        let block = store.read_blocks().unwrap().blocks.pop().unwrap();
        assert_eq!(block.header.batch_id, good.batch_id);
        assert_eq!(block.header.height, height);
        let cert = &block.header.certificate;
        assert_eq!(cert.validators.len(), 6);
        assert_eq!(cert.quorum, 5);
        assert!(cert.votes.iter().filter(|v| v.accept).count() >= cert.quorum);
        let receipt = store.read_receipts().unwrap().pop().unwrap();
        assert!(receipt.accepted);
        assert_eq!(block.receipt_ids, [receipt.tx_id.clone()]);
        let post = store.read_shielded().unwrap().pq_fixture.unwrap();
        assert_eq!(post.reserve, pre.reserve - 10);
        assert_eq!(post.commitments[..100], pre.commitments);
        assert_eq!(post.spent_serials[..2], pre.spent_serials);
        assert_eq!(
            post.spent_serials[2..],
            [bytes_to_hex(&HOP[112..160]), bytes_to_hex(&HOP[160..208])]
        );
        assert_eq!(
            post.commitments[100..],
            [bytes_to_hex(&HOP[208..256]), bytes_to_hex(&HOP[256..304])]
        );

        let archived = store.read_batch_archive().unwrap().batches.pop().unwrap();
        let archived: ShieldedActionBatch = serde_json::from_str(&archived.payload_json).unwrap();
        assert_eq!(archived, good);
        let [ShieldedAction::PqFixtureHop { payment_hex }] = archived.actions.as_slice() else {
            panic!("missing hop")
        };
        let wire = hex_to_bytes(payment_hex).unwrap();
        assert_eq!(wire, HOP);
        let archived_file = fleet.root.join("archived-hop.bin");
        fs::write(&archived_file, wire).unwrap();
        let tool = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../tools/pft-binius-shielded-pq/go-verifier");
        let go = std::env::var_os("GO_BIN").unwrap_or_else(|| "go".into());
        let out = Command::new(go)
            .current_dir(tool)
            .args(["run", ".", "verify", "-post-root", "-tx"])
            .arg(archived_file)
            .args(["-ledger", "../payments/ledger_hop1.json"])
            .output()
            .expect("cannot run the Go cross-check verifier: set GO_BIN to a Go 1.25.1 binary or put go on PATH");
        assert!(
            out.status.success(),
            "{}",
            String::from_utf8_lossy(&out.stderr)
        );
        let go_output = String::from_utf8(out.stdout).unwrap();
        assert_eq!(
            go_output.lines().last(),
            Some(format!("post-root {}", post.root).as_str())
        );

        // A different batch ID reaches the spent-serial check instead of the batch deduplicator.
        let second = proposal(peer_dir, &fleet.root, &batch(&genesis, HOP, false));
        assert!(propose_batch_with_timings(second)
            .unwrap_err()
            .to_string()
            .contains("pq_fixture_spent_serial"));
        drop(store);
        let replay = verify_blocks(NodeOptions {
            data_dir: peer_dir.clone(),
        })
        .unwrap();
        assert!(replay.verified);
        assert_eq!(replay.tip_hash, block.header.block_hash);
        assert_eq!(replay.state_root, block.header.state_root);
        assert_eq!(
            NodeStore::new(peer_dir).read_shielded().unwrap().pq_fixture,
            Some(post.clone())
        );
        println!("Finalized height {height}, {}/{} votes, accepted receipt {}, two new serials, Go post-root {}, replay verified",
            cert.votes.len(), cert.validators.len(), receipt.tx_id, post.root);

        // Replay must detect changes to each fixture component even when the archive and
        // its quorum certificate are intact. Use a disposable replica after the round.
        let replica_dir = &fleet.data_dirs[(peer + 1) % fleet.data_dirs.len()];
        let replica = NodeStore::new(replica_dir);
        let certified = replica.read_shielded().unwrap();
        let changes: [fn(&mut postfiat_types::PqFixtureState); 4] = [
            |p: &mut postfiat_types::PqFixtureState| p.reserve += 1,
            |p: &mut postfiat_types::PqFixtureState| p.root.clear(),
            |p: &mut postfiat_types::PqFixtureState| p.spent_serials.swap(2, 3),
            |p: &mut postfiat_types::PqFixtureState| p.commitments.swap(100, 101),
        ];
        for change in changes {
            let mut altered = certified.clone();
            change(altered.pq_fixture.as_mut().unwrap());
            replica.write_shielded(&altered).unwrap();
            let err = verify_blocks(NodeOptions {
                data_dir: replica_dir.clone(),
            })
            .unwrap_err();
            assert!(err.to_string().contains("state root"), "{err}");
        }

        let foreign = fleet.root.join("foreign");
        init(InitOptions {
            data_dir: foreign.clone(),
            chain_id: "ordinary-test".into(),
            node_id: "validator-0".into(),
            validator_count: 1,
        })
        .unwrap();
        let other = NodeStore::new(&foreign);
        let foreign_genesis = other.read_genesis().unwrap();
        let foreign_opts = proposal(&foreign, &fleet.root, &batch(&foreign_genesis, HOP, false));
        assert!(propose_batch_with_timings(foreign_opts)
            .unwrap_err()
            .to_string()
            .contains("pq_fixture_wrong_genesis"));
        let mut state = other.read_shielded().unwrap();
        state.pq_fixture = Some(pre);
        other.write_shielded(&state).unwrap();
        assert!(verify_blocks(NodeOptions { data_dir: foreign })
            .unwrap_err()
            .to_string()
            .contains("pq_fixture_state_genesis_mismatch"));
        let same_name = init(InitOptions {
            data_dir: fleet.root.join("wrong-genesis"),
            chain_id: CHAIN.into(),
            node_id: "validator-0".into(),
            validator_count: 1,
        })
        .unwrap_err();
        assert!(same_name.to_string().contains("pq_fixture_wrong_genesis"));
        fs::remove_dir_all(&fleet.root).unwrap();
    }
}
