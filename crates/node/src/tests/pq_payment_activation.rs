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
        check(&g, h, &[&batch, &mixed, &mixed]).unwrap();
        assert_eq!(
            check(&g, h, &[&batch, &mixed, &batch])
                .unwrap_err()
                .to_string(),
            "pq_block_limit"
        );
    }
    g.apply(amendment(&genesis, 0));
    assert!(check(&g, 7, &[&batch]).is_err());
    g.apply(amendment(&genesis, 9));
    check(&g, 9, &[&batch]).unwrap();
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
