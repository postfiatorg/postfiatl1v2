//! End-to-end benchmark at the real depth-24 epoch boundary.
use fips204::{ml_dsa_65, traits::KeyGen};
use pft_binius_shielded::payment::{
    create_shielded_tx, LedgerState, ShieldedTx, ShieldedWallet, SpendInput, MAX_POOL_COINS,
    W1_ARCHIVAL_BUDGET_BYTES,
};
use rand::SeedableRng;
use rand_chacha::ChaCha20Rng;
use std::{fs, path::Path, time::Instant};

pub fn run(out: &Path) -> Result<(), String> {
    let mut rng = ChaCha20Rng::from_seed([0x17; 32]);
    let wallet = ShieldedWallet::from_seed(&mut rng);
    let dest = wallet.stealth_address();
    let (_, depositor) = ml_dsa_65::KG::keygen_from_seed(&[0x64; 32]);

    let start = Instant::now();
    let mut ledger = LedgerState::from_coins((1..=MAX_POOL_COINS - 2).map(|i| {
        let mut coin = [0; 48];
        coin[..8].copy_from_slice(&(i as u64).to_le_bytes());
        Ok(coin)
    }))?;
    let build = start.elapsed();
    let initial_anchor = ledger.tree.root();
    // A real settlement fills the last two slots. Its second output becomes the deep input.
    let (mint, _, blind) = create_shielded_tx(
        &ledger.tree,
        SpendInput::Dummy,
        SpendInput::Dummy,
        &dest,
        3,
        &dest,
        7,
        10,
        0,
        [0; 32],
        0,
        Some(&depositor),
        &mut rng,
    )?;
    let finish = Instant::now();
    let (_, last) = ledger.verify_and_apply(&mint)?;
    let build_and_fill = build + finish.elapsed();
    assert_eq!(last, MAX_POOL_COINS - 1);
    assert_eq!(ledger.active_epoch(), 1);
    assert!(ledger.tree.is_empty());
    assert!(ledger.anchors.contains(&initial_anchor));
    assert!(ledger.verify_tx(&mint).unwrap_err().contains("R3"));
    assert!(ledger
        .mint_coin(mint.coin_out1)
        .unwrap_err()
        .contains("duplicate"));
    let (frozen, index) = ledger.locate(last).ok_or("lost frozen epoch")?;
    assert_eq!(frozen.len(), MAX_POOL_COINS);
    let frozen_root = frozen.root();
    let start = Instant::now();
    let (transfer, _, _) = create_shielded_tx(
        frozen,
        SpendInput::Real {
            leaf_index: index,
            spend_secret: wallet.spend_secret,
            value: 7,
            r_blind: blind,
        },
        SpendInput::Dummy,
        &dest,
        4,
        &dest,
        2,
        0,
        0,
        [0; 32],
        1,
        None,
        &mut rng,
    )?;
    let prove = start.elapsed();
    let wire = transfer.to_bytes();
    assert!(wire.len() <= W1_ARCHIVAL_BUDGET_BYTES);
    let start = Instant::now();
    let parsed = ShieldedTx::from_bytes(&wire)?;
    let (out0, out1) = ledger.verify_and_apply(&parsed)?;
    let verify = start.elapsed();
    assert_eq!((out0, out1), (MAX_POOL_COINS, MAX_POOL_COINS + 1));
    assert_eq!(
        ledger.tree.leaves.as_slice(),
        &[parsed.coin_out0, parsed.coin_out1]
    );
    assert_eq!(ledger.locate(last).unwrap().0.root(), frozen_root);
    assert!(ledger.anchors.contains(&frozen_root) && ledger.anchors.contains(&ledger.tree.root()));
    assert_eq!(ledger.spent_sn.len(), 4);
    assert!(ledger.verify_tx(&parsed).unwrap_err().contains("R3"));
    fs::create_dir_all(out).map_err(|e| e.to_string())?;
    fs::write(out.join("boundary-mint.bin"), mint.to_bytes()).map_err(|e| e.to_string())?;
    fs::write(out.join("old-tree-spend.bin"), wire.as_slice()).map_err(|e| e.to_string())?;
    println!(
        "RUST FULL TREE PASS: leaves={} input={} epoch=0->1 wire={} B build_and_fill={:.2} ms prove={:.2} ms parse_verify_apply={:.2} ms",
        MAX_POOL_COINS, index, wire.len(), build_and_fill.as_secs_f64() * 1000.0,
        prove.as_secs_f64() * 1000.0, verify.as_secs_f64() * 1000.0,
    );
    Ok(())
}
