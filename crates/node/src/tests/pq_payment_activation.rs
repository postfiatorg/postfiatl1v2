//! Rehearsal validity and measurements on the same paths used by the validator.
use super::*;
use postfiat_types::pq_payment::{Mode, PqPayment, PqPool};

pub(super) fn payment(name: &str) -> PqPayment {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/pft-binius-shielded-pq/l1-vectors")
        .join(format!("{name}.bin"));
    PqPayment::from_bytes(fs::read(path).unwrap()).unwrap()
}

pub(super) fn rehearsal_genesis(dir: &Path) -> Genesis {
    init_consensus_v2(InitConsensusV2Options {
        data_dir: dir.into(),
        chain_id: pq_payment_fixture::CHAIN_ID.into(),
        node_id: "validator-0".into(),
        validator_count: 6,
        activation_height: 1000,
        storage_activation_height: None,
    })
    .unwrap();
    NodeStore::new(dir).read_genesis().unwrap()
}

pub(super) fn amendment(genesis: &Genesis, height: u32) -> GovernanceAmendment {
    let mut a =
        super::vault_bridge_governed_route::amendment(postfiat_types::PQ_REHEARSAL_V1, height, 0);
    a.chain_id = genesis.chain_id.clone();
    a.genesis_hash = genesis_hash(genesis);
    a.protocol_version = genesis.protocol_version;
    a
}

#[test]
fn candidate_height_and_limits_cover_every_batch_and_non_atomic_attempt() {
    let dir = unique_test_dir("pq-activation");
    let genesis = rehearsal_genesis(&dir);
    let mac = "0".repeat(2 * postfiat_storage::integrity::MAC_BYTES);
    let envelope = serde_json::json!({"pftmac":postfiat_storage::integrity::JSONL_ENVELOPE_KIND,
        "chain":mac,"record":null,"mac":mac});
    let jsonl = serde_json::to_vec(&envelope).unwrap().len() - 4 + 1 + 96;
    let redb = 2 + "shielded".len() + 96 + 96 + 2 * (1 + postfiat_storage::integrity::MAC_BYTES);
    assert_eq!(
        jsonl.max(redb),
        postfiat_types::PQ_REHEARSAL_ARCHIVE_OVERHEAD
    );
    let pay = ShieldedAction::PqPaymentV1(payment("pft-pay"));
    let batch = build_atomic_shielded_action_batch(&genesis, vec![pay.clone(); 2]).unwrap();
    let mixed = build_shielded_action_batch(
        &genesis,
        vec![
            pay.clone(),
            ShieldedAction::ShieldedSwapV1(postfiat_types::ShieldedSwapActionPayload {
                swap_json: "{}".into(),
            }),
        ],
    )
    .unwrap();
    let mut g = GovernanceState::new(6);
    let check = |g: &GovernanceState, h, batches: &[&ShieldedActionBatch]| {
        consensus_artifacts::check_pq_block(&genesis, g, h, batches.iter().copied())
    };
    for height in [5, 6, 7] {
        assert!(check(&g, height, &[&mixed])
            .unwrap_err()
            .to_string()
            .contains("pq_settlement_inactive"));
    }
    let mut wrong = amendment(&genesis, 6);
    wrong.genesis_hash = "11".repeat(48);
    g.apply(wrong);
    assert!(check(&g, 6, &[&batch]).is_err());
    g.apply(amendment(&genesis, 6));
    assert!(check(&g, 5, &[&batch, &mixed]).is_err());
    for h in [6, 7] {
        assert_eq!(check(&g, h, &[&mixed]).unwrap_err().to_string(), "pq_atomic_batch_required");
        check(&g, h, &[&batch, &batch]).unwrap();
        assert_eq!(
            check(&g, h, &[&batch, &batch, &batch])
                .unwrap_err()
                .to_string(),
            "pq_block_limit"
        );
    }
    g.apply(amendment(&genesis, 0));
    assert!(check(&g, 7, &[&batch]).is_err());
    g.apply(amendment(&genesis, 9));
    check(&g, 9, &[&batch]).unwrap();
    let disk = NodeStore::new(&dir);
    let mut atomic = amendment(&genesis, 1);
    atomic.kind = GOVERNANCE_KIND_SHIELDED_ATOMIC_BATCH_ACTIVATION_HEIGHT.into();
    g.apply(atomic);
    // The CLI builder consumes four separate source files. At its actual
    // next height a future PQ amendment must still block their combination.
    disk.write_governance(&g).unwrap();
    let source = build_shielded_action_batch(&genesis, vec![pay.clone()]).unwrap();
    let path = dir.join("source-batch.json");
    fs::write(&path, serde_json::to_vec(&source).unwrap()).unwrap();
    let options = |n| ShieldedAtomicBatchOptions {
        data_dir: dir.clone(),
        source_batch_files: vec![path.clone(); n],
        batch_file: dir.join("combined.json"),
    };
    assert_eq!(
        create_shielded_atomic_batch(options(4))
            .unwrap_err()
            .to_string(),
        "pq_settlement_inactive"
    );
    g.apply(amendment(&genesis, 1));
    disk.write_governance(&g).unwrap();
    assert_eq!(
        create_shielded_atomic_batch(options(4))
            .unwrap()
            .actions
            .len(),
        4
    );
    assert!(create_shielded_atomic_batch(options(5)).is_err());
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn replay_rejects_preactivation_non_atomic_body_before_receipts() {
    let dir = unique_test_dir("pq-replay-activation");
    let genesis = rehearsal_genesis(&dir);
    let disk = NodeStore::new(&dir);
    let batch = build_shielded_action_batch(
        &genesis,
        vec![ShieldedAction::PqPaymentV1(payment("pfusdc-shield"))],
    )
    .unwrap();
    let entry = BatchArchiveEntry {
        batch_kind: "shielded".into(),
        batch_id: batch.batch_id.clone(),
        payload_hash: String::new(),
        payload_json: serde_json::to_string(&batch).unwrap(),
    };
    let registry = read_validator_registry_file(&dir.join(VALIDATOR_REGISTRY_FILE)).unwrap();
    for configured in [false, true] {
        for h in [5, 6, 7] {
            let mut governance = disk.read_governance().unwrap();
            if configured {
                governance.apply(amendment(&genesis, 6));
            }
            let mut ledger = disk.read_ledger().unwrap();
            let mut shielded = disk.read_shielded().unwrap();
            let mut bridge = disk.read_bridge().unwrap();
            let mut block = super::dummy_block_record(h);
            block.header.batch_kind = "shielded".into();
            block.header.batch_id = batch.batch_id.clone();
            let result = block_replay_wallet::replay_archived_payload(
                &genesis,
                &block,
                &entry,
                block_replay_wallet::ArchivedReplayState {
                    governance: &mut governance,
                    ledger: &mut ledger,
                    shielded: &mut shielded,
                    bridge: &mut bridge,
                    validator_registry: &registry,
                },
            );
            if configured && h >= 6 {
                assert_eq!(result.unwrap_err().to_string(), "pq_atomic_batch_required");
            } else {
                assert_eq!(result.unwrap_err().to_string(), "pq_settlement_inactive");
            }
            assert_eq!(shielded, disk.read_shielded().unwrap());
        }
    }
    fs::remove_dir_all(dir).unwrap();
}

// The cost sample includes compact transport decode, batch identity, proof,
// signature, ordered-state checks, append, account writes and both state roots.
// The unrelated tree supplies a populated global index without changing the
// signed input roots. Its index entries are synthetic measurement data.
pub(super) fn benchmark(
    genesis: &Genesis,
    gov: &GovernanceState,
    ledger: &LedgerState,
    mut shielded: ShieldedState,
    bridge: &BridgeState,
    ordered: &[String],
    height: u64,
    payments: &[PqPayment],
    other_asset: &str,
    late_recipient: &str,
) {
    use std::time::Instant;
    let issued = payments[0].parts().unwrap().0.asset != [0; 48];
    let asset = if issued { "pfUSDC" } else { "PFT" };
    let chain = postfiat_types::pq_payment::Chain {
        id: &genesis.chain_id,
        genesis: hex_to_bytes(&genesis_hash(genesis))
            .unwrap()
            .try_into()
            .unwrap(),
        protocol: genesis.protocol_version,
    };
    let (seed, _) =
        crate::pq_pool_state::prepare_pair(&PqPool::default(), &payment("pft-shield"), chain, 1)
            .unwrap();
    let mut pair = seed.pending[0].as_ref().clone();
    pair.asset = other_asset.into();
    pair.kind = Mode::Internal;
    pair.mint = 0;
    pair.fee = 0;
    pair.height = height;
    let mut pool = shielded.pq_pool_v1.take().unwrap_or_default();
    for index in 0..1024u32 {
        let tree = pool
            .head
            .assets
            .get(other_asset)
            .cloned()
            .unwrap_or_default()
            .writable;
        pair.first_index = tree.count;
        pair.previous = pool.head.last_page.clone();
        pair.anchor = tree.anchors.back().unwrap().clone();
        for j in 0..2 {
            let key = (2 * index + j as u32).to_le_bytes();
            pair.serials[j] = hash_hex("benchmark serial", &key);
            pair.outputs[j].commitment = hash_hex("benchmark output", &key);
        }
        pool = pool.append_pair(pair.clone()).unwrap();
        pool.pending.clear();
    }
    shielded.pq_pool_v1 = Some(pool);
    let mut eviction = vec![0u8; 128 * 1024 * 1024];
    for count in [1, payments.len()] {
        let batch = build_atomic_shielded_action_batch(
            genesis,
            payments[..count]
                .iter()
                .cloned()
                .map(ShieldedAction::PqPaymentV1)
                .collect(),
        )
        .unwrap();
        let json = serde_json::to_vec(&batch).unwrap();
        for valid in [true, false] {
            let mut view = ledger.clone();
            if !valid {
                if count == 1 {
                    let payer = payments[0].parts().unwrap().0.account.unwrap();
                    view.account_mut(&format!("pf{}", bytes_to_hex(&payer.id)))
                        .unwrap()
                        .balance = 0;
                } else if issued {
                    let issue = bytes_to_hex(&payments[0].parts().unwrap().0.asset);
                    view.trustlines
                        .iter_mut()
                        .find(|l| l.account == late_recipient && l.asset_id == issue)
                        .unwrap()
                        .limit = 25;
                } else {
                    view.ensure_account(late_recipient).balance = u64::MAX - 25;
                }
            }
            for cold in [false, true] {
                let mut times = Vec::new();
                for _ in 0..21 {
                    pq_payment_actions::clear_verified();
                    if cold {
                        for byte in eviction.iter_mut().step_by(64) {
                            *byte = byte.wrapping_add(1);
                        }
                        std::hint::black_box(&eviction);
                    }
                    let start = Instant::now();
                    let decoded = serde_json::from_slice(&json).unwrap();
                    let result = simulate_shielded_batch_against_state(
                        genesis, gov, &view, ordered, &shielded, bridge, height, &decoded,
                    )
                    .unwrap();
                    times.push(start.elapsed().as_secs_f64() * 1e3);
                    assert_eq!(result.all_accepted, valid, "{:?}", result.receipts);
                    if !valid {
                        assert!(!result.receipts.last().unwrap().accepted);
                    }
                }
                times.sort_by(f64::total_cmp);
                println!("PQ benchmark asset={asset} actions={count} valid={valid} cold={cold} n=21 history=1024 median={:.2} p95={:.2} max={:.2} ms", times[10],times[19],times[20]);
            }
        }
    }
}

#[test]
fn pq_registry_threshold_applies_to_signed_schedule_and_withdrawal() {
    let dir = unique_test_dir("pq-governance-threshold");
    let genesis = rehearsal_genesis(&dir);
    let store = NodeStore::new(&dir);
    let governance = store.read_governance().unwrap();
    let registry = read_validator_registry_file(&dir.join(VALIDATOR_REGISTRY_FILE)).unwrap();
    let validators = active_validator_ids(&governance).unwrap();
    let keys = read_validator_key_file(&dir.join(VALIDATOR_KEYS_FILE)).unwrap();
    let split = super::consensus_history::write_split_validator_key_files(&dir, &keys);
    for kind in [postfiat_types::PQ_REHEARSAL_V1, "pq_payment_rehearsal_v1:fee:1",
                 &format!("pq_private_fee_withdraw_v1:pf{}", "22".repeat(20))] {
        let mut config = EssentialSubsetConfig::all_of(validators.clone());
        config.quorum = 1;
        let mut amendment = ratify_governance_amendment_with_lifecycle(
            &cobalt_domain(&genesis), &config, kind, 2, vec![validators[0].clone()],
            GovernanceAmendmentLifecycle { activation_height: 0, veto_until_height: 0, paused: false },
        ).unwrap();
        let file = dir.join("amendment.json");
        write_amendment_file(&file, &amendment).unwrap();
        let authorization = sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
            data_dir: dir.clone(), amendment_file: file, validator: validators[0].clone(),
            validator_key_file: split.iter().find(|(id, _)| id == &validators[0]).unwrap().1.clone(), proposal_slot: 1,
            expires_at_height: 8, authorization_file: dir.join("authorization.json"),
        }).unwrap();
        amendment.signed_authorizations.push(authorization);
        let batch = GovernanceActionBatch::new("test", vec![amendment]);
        let error = verify_live_signed_governance_batch(&genesis, &governance, &registry, &batch, 1)
            .expect_err("one signature cannot authorize PQ governance");
        assert!(error.to_string().contains("BFT threshold"), "{error}");
    }
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn pq_replacement_fee_waits_for_activation_and_links_the_two_schedules() {
    use postfiat_types::PQ_REHEARSAL_ISSUE;
    let genesis = Genesis::new_with_validator_count("fee-history", 6);
    let mut g = GovernanceState::new(6);
    for (height, fee) in [(3, 1), (9, 7)] {
        let mut schedule = amendment(&genesis, height);
        schedule.kind = format!("pq_payment_rehearsal_v1:fee:{fee}");
        crate::execution_actions::apply_governance_amendment_with_lifecycle_records(
            &mut g, schedule, "fee-batch", u64::from(height - 2));
    }
    assert_eq!(g.pq_issued_fee(PQ_REHEARSAL_ISSUE, 2), None);
    assert_eq!(g.pq_issued_fee(PQ_REHEARSAL_ISSUE, 8), Some(1));
    assert_eq!(g.pq_issued_fee(PQ_REHEARSAL_ISSUE, 9), Some(7));
    let [record] = g.amendment_supersession_records.as_slice() else { panic!("missing schedule history") };
    assert_eq!(record.superseded_amendment_id, g.amendments[0].amendment_id);
    assert_eq!(record.previous_value, 3);
    crate::execution_actions::verify_governance_amendment_supersession_records(&genesis, &g).unwrap();
    let bare = GovernanceActionBatch::new("bare", vec![amendment(&genesis, 12)]);
    let receipts = crate::execution_actions::execute_governance_batch(&mut g, None, Some(&genesis), &bare, 10);
    assert_eq!(receipts[0].code, "pq_fee_unconfigured");
    let pause = GovernanceActionBatch::new("pause", vec![amendment(&genesis, 0)]);
    let receipts = crate::execution_actions::execute_governance_batch(&mut g, None, Some(&genesis), &pause, 11);
    assert!(receipts[0].accepted);
    assert_eq!(g.pq_issued_fee(PQ_REHEARSAL_ISSUE, 12), None);
}

#[test]
fn pq_governance_archive_requires_signatures_before_execution() {
    let dir = unique_test_dir("pq-governance-archive");
    let genesis = rehearsal_genesis(&dir);
    let disk = NodeStore::new(&dir);
    let registry = read_validator_registry_file(&dir.join(VALIDATOR_REGISTRY_FILE)).unwrap();
    let mut governance = disk.read_governance().unwrap();
    let validators = active_validator_ids(&governance).unwrap();
    let schedule = ratify_governance(RatifyGovernanceOptions {
        data_dir: dir.clone(), validators: validators.clone(), support: validators,
        kind: "pq_payment_rehearsal_v1:fee:4".into(), value: 10, activation_height: 0,
        veto_until_height: 0, paused: false, amendment_file: dir.join("schedule.json"),
    }).unwrap();
    let batch = GovernanceActionBatch::new("test", vec![schedule]);
    let entry = BatchArchiveEntry { batch_kind: "governance".into(), batch_id: batch.batch_id.clone(),
        payload_hash: String::new(), payload_json: serde_json::to_string(&batch).unwrap() };
    let mut block = super::dummy_block_record(1);
    block.header.batch_kind = "governance".into();
    block.header.batch_id = batch.batch_id.clone();
    let historical = update_governance_for_certificate_replay(&mut governance, &registry, &genesis, &block, &entry);
    let replay = block_replay_wallet::replay_archived_payload(&genesis, &block, &entry,
        block_replay_wallet::ArchivedReplayState {
            governance: &mut governance, ledger: &mut disk.read_ledger().unwrap(),
            shielded: &mut disk.read_shielded().unwrap(), bridge: &mut disk.read_bridge().unwrap(),
            validator_registry: &registry,
        });
    for err in [historical.unwrap_err(), replay.unwrap_err()] {
        assert!(err.to_string().contains("one signed governance authorization"), "{err}");
    }
    assert_eq!(governance, disk.read_governance().unwrap());
    fs::remove_dir_all(dir).unwrap();
}
