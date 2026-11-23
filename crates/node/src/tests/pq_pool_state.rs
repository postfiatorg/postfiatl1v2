use super::*;
use crate::pq_pool_state::prepare_pair;
use postfiat_privacy_binius::MerkleTree24;
use postfiat_types::pq_payment::*;

fn chain() -> Chain<'static> {
    Chain {
        id: pq_payment_fixture::CHAIN_ID,
        genesis: hex_to_bytes(pq_payment_fixture::GENESIS_HASH)
            .unwrap()
            .try_into()
            .unwrap(),
        protocol: 1,
    }
}
fn vector(folder: &str, name: &str) -> PqPayment {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tools/pft-binius-shielded-pq")
        .join(folder)
        .join(format!("{name}.bin"));
    PqPayment::from_bytes(fs::read(path).unwrap()).unwrap()
}
fn native() -> String {
    "00".repeat(48)
}

#[test]
fn pq_pair_window_and_historical_witness_match_the_reference() {
    let (mut pool, _) = prepare_pair(
        &PqPool::default(),
        &vector("l1-vectors", "pft-shield"),
        chain(),
        1,
    )
    .unwrap();
    let template = pool.pending.back().unwrap().as_ref().clone();
    let mut reference = MerkleTree24::new();
    let asset = native();
    let opening = bytes_to_hex(&reference.root());
    let mut historical = Vec::new();
    for n in 1..=257 {
        let mut pair = template.clone();
        if n > 1 {
            pair.previous = pool.head.last_page.clone();
            pair.first_index = 2 * (n - 1);
            for i in 0..2 {
                pair.serials[i] = hash_hex("test serial", &(2 * n + i as u32).to_le_bytes());
                pair.outputs[i].commitment =
                    hash_hex("test output", &(2 * n + i as u32).to_le_bytes());
            }
            pair.anchor = pool
                .tree(&asset, 0)
                .unwrap()
                .anchors
                .back()
                .unwrap()
                .clone();
            pool = pool.append_pair(pair.clone()).unwrap();
        }
        reference.append(
            hex_to_bytes(&pair.outputs[0].commitment)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let halfway = bytes_to_hex(&reference.root());
        reference.append(
            hex_to_bytes(&pair.outputs[1].commitment)
                .unwrap()
                .try_into()
                .unwrap(),
        );
        let tree = pool.tree(&asset, 0).unwrap();
        assert_eq!(tree.count, 2 * n);
        assert_eq!(
            tree.anchors.back().unwrap(),
            &bytes_to_hex(&reference.root())
        );
        assert!(!tree.anchors.contains(&halfway));
        assert_eq!(tree.anchors.len(), ((n + 1) as usize).min(PQ_WINDOW));
        assert_eq!(tree.anchors.contains(&opening), n < 256);
        assert_eq!(
            pool.witness(&asset, 0, 2 * n, n - 1).unwrap(),
            reference.auth_path((n - 1) as usize).1
        );
        if n == 1 || n == 255 || n == 256 {
            historical.push((2 * n, n - 1, reference.auth_path((n - 1) as usize).1));
        }
        if n == 256 {
            let mut test = pair.clone();
            test.first_index = tree.count;
            test.previous = pool.head.last_page.clone();
            test.anchor = opening.clone();
            assert_eq!(pool.append_pair(test).unwrap_err(), "pq_unknown_anchor");
        }
    }
    for (count, index, path) in historical {
        assert_eq!(pool.witness(&asset, 0, count, index).unwrap(), path);
    }
}

#[test]
fn pq_spent_or_stale_payment_is_refused_before_its_proof_is_verified() {
    let shield = vector("l1-vectors", "pft-shield");
    let (pool, _) = prepare_pair(&PqPool::default(), &shield, chain(), 5).unwrap();
    crate::pq_payment_actions::clear_verified();
    let before = crate::pq_payment_actions::proof_verifications();
    // Spent nullifiers: the same deposit again.
    assert_eq!(
        prepare_pair(&pool, &shield, chain(), 6).unwrap_err(),
        "pq_spent_serial"
    );
    // A root the pfUSDC tree has never had.
    assert_eq!(
        prepare_pair(&PqPool::default(), &vector("l1-vectors", "pfusdc-pay"), chain(), 5)
            .unwrap_err(),
        "pq_unknown_anchor"
    );
    assert_eq!(crate::pq_payment_actions::proof_verifications(), before);
}

#[test]
fn pq_global_uniqueness_and_checked_public_flows_are_atomic() {
    let pft = vector("l1-vectors", "pft-shield");
    let (pool, _) = prepare_pair(&PqPool::default(), &pft, chain(), 5).unwrap();
    let (both, _) =
        prepare_pair(&pool, &vector("l1-vectors", "pfusdc-shield"), chain(), 5).unwrap();
    let (both, _) = prepare_pair(&both, &vector("l1-vectors", "pfusdc-pay"), chain(), 6).unwrap();
    assert_eq!(both.head.assets.len(), 2);
    assert_eq!(both.head.assets[&native()].reserve, 99);
    for (asset, state) in &both.head.assets {
        if asset != &native() {
            assert_eq!((state.reserve, state.fees, state.ingress), (100, 0, 100));
        }
    }
    for h in [5, 6] {
        assert_eq!(
            prepare_pair(&pool, &pft, chain(), h).unwrap_err(),
            "pq_spent_serial"
        );
    }
    let minted = pool.pending.front().unwrap();
    let mut pair = both.pending.back().unwrap().as_ref().clone();
    pair.previous = both.head.last_page.clone();
    pair.first_index = 4;
    pair.serials = [hash_hex("s", b"0"), hash_hex("s", b"1")];
    pair.outputs[0].commitment = hash_hex("c", b"0");
    pair.outputs[1].commitment = hash_hex("c", b"1");
    for mutation in 0..6 {
        let mut bad = pair.clone();
        let reason = match mutation {
            0 => {
                bad.serials[1] = bad.serials[0].clone();
                "pq_spent_serial"
            }
            1 => {
                bad.serials[0] = minted.serials[1].clone();
                "pq_spent_serial"
            }
            2 => {
                bad.outputs[1].commitment = bad.outputs[0].commitment.clone();
                "pq_invalid_commitment"
            }
            3 => {
                bad.outputs[0].commitment = native();
                "pq_invalid_commitment"
            }
            4 => {
                bad.outputs[1].commitment = minted.outputs[0].commitment.clone();
                "pq_invalid_commitment"
            }
            _ => {
                bad.kind = Mode::Exit;
                bad.exit = u64::MAX;
                "pq_reserve"
            }
        };
        assert_eq!(both.append_pair(bad).unwrap_err(), reason);
    }
    assert_eq!(both.head.pairs, 3);
    assert!(both.indexes.is_spent(&minted.serials[1]));
    assert!(both
        .indexes
        .contains_commitment(&minted.outputs[0].commitment));
}

#[test]
fn pq_old_note_spends_after_two_real_depth_rollovers() {
    let mut pool = PqPool::default();
    let asset = native();
    let mut terminals = Vec::new();
    for epoch in 0..2 {
        // Test-only compressed prefix from the standalone full tree. Go
        // independently reconstructs each root from 16,777,214 distinct leaves.
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../tools/pft-binius-shielded-pq/rollover-vectors/frontier-{epoch}.json"
        ));
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let terminal = value
            .as_object_mut()
            .unwrap()
            .remove("terminal")
            .unwrap()
            .as_str()
            .unwrap()
            .to_owned();
        let prefix: PqEpoch = serde_json::from_value(value).unwrap();
        assert_eq!(prefix.count, PQ_CAPACITY - 2);
        assert_eq!(
            prefix.anchors.back(),
            Some(&bytes_to_hex(&prefix.frontier_root().unwrap()))
        );
        for (level, slot) in prefix.frontier.iter().enumerate() {
            assert_eq!(slot.is_some(), level != 0);
        }
        let state = pool.head.assets.entry(asset.clone()).or_default();
        assert_eq!(state.epoch, epoch);
        assert_eq!(state.writable.count, 0);
        state.writable = prefix;
        let mint = vector("rollover-vectors", &format!("boundary-{epoch}"));
        let (next, _) = prepare_pair(&pool, &mint, chain(), epoch + 1).unwrap();
        pool = next;
        assert_eq!(
            pool.tree(&asset, epoch).unwrap().anchors.back(),
            Some(&terminal)
        );
        assert_eq!(pool.tree(&asset, epoch).unwrap().count, PQ_CAPACITY);
        assert_eq!(pool.tree(&asset, epoch + 1).unwrap(), &PqEpoch::default());
        let page = pool.pending.back().unwrap();
        assert_eq!((page.epoch, page.first_index), (epoch, PQ_CAPACITY - 2));
        assert_eq!(
            pool.indexes.page_for_output(&asset, epoch, PQ_CAPACITY - 1),
            Some(pool.head.last_page.as_str())
        );
        assert_eq!(
            prepare_pair(&pool, &mint, chain(), 4).unwrap_err(),
            "pq_spent_serial"
        );
        terminals.push(terminal);
    }
    let mixed = vector("rollover-vectors", "mixed-inputs");
    assert!(postfiat_privacy_binius::verify_l1(&mixed, chain()).is_ok());
    assert_eq!(
        prepare_pair(&pool, &mixed, chain(), 5).unwrap_err(),
        "pq_unknown_anchor"
    );
    let old = vector("rollover-vectors", "old-spend");
    let mut changed = old.bytes().to_vec();
    changed[55] = 1;
    assert_eq!(
        prepare_pair(&pool, &PqPayment::from_bytes(changed).unwrap(), chain(), 5).unwrap_err(),
        "pq_invalid_proof"
    );
    let (pool, _) = prepare_pair(&pool, &old, chain(), 5).unwrap();
    let pair = pool.pending.back().unwrap();
    assert_eq!((pair.read_epoch, pair.epoch, pair.first_index), (0, 2, 0));
    assert_eq!(
        prepare_pair(&pool, &old, chain(), 6).unwrap_err(),
        "pq_spent_serial"
    );
    let (pool, _) = prepare_pair(
        &pool,
        &vector("rollover-vectors", "successor-spend"),
        chain(),
        6,
    )
    .unwrap();
    assert_eq!(pool.head.assets[&asset].reserve, 16);
    assert_eq!(
        (
            pool.head.assets[&asset].ingress,
            pool.head.assets[&asset].fees
        ),
        (20, 4)
    );
    for (epoch, root) in terminals.iter().enumerate() {
        assert_eq!(
            pool.tree(&asset, epoch as u64).unwrap().anchors.back(),
            Some(root)
        );
    }
    println!("Two 16,777,216-leaf epochs closed; epoch-0 note moved to positions 0 and 1 in epoch 2, then spent again");
}
