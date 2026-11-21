use pft_binius_shielded::binius_pcs::{prove, verify};
use pft_binius_shielded::circuit::{
    build_group_c_constraints, build_witness_bits, eval_verifier_weights_at_r1, CircuitChallenge,
    PaymentWitness, V_SC1,
};
use pft_binius_shielded::field::{k_tables, E};
use pft_binius_shielded::payment::{
    create_shielded_tx, LedgerJson, LedgerState, ShieldedTx, ShieldedWallet, SpendInput,
    MAX_POOL_COINS, UNSHIELD_DEST_BYTES, W1_ARCHIVAL_BUDGET_BYTES,
};
use pft_binius_shielded::shake256::{
    coin_commit, nullifier, owner_addr, BLIND_SECRET_BYTES, MERKLE_DEPTH, NODE_BYTES,
    OWNER_SECRET_BYTES,
};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use serde::Serialize;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

#[derive(Serialize)]
struct WalletFixtureJson {
    spend_secret_hex: String,
    kem_seed_hex: String,
    addr_hex: String,
    ek_hex: String,
    expected_value: u64,
    expected_r_hex: String,
    expected_coin_hex: String,
}

#[derive(Serialize)]
struct GenerateSummary {
    protocol: &'static str,
    merkle_depth: usize,
    global_pool_capacity: usize,
    tx_mint_bytes: usize,
    tx_hop1_bytes: usize,
    tx_dummy_1in_bytes: usize,
    tx_hop2_bytes: usize,
    budget_bytes: usize,
    mint_prove_ms: f64,
    hop1_prove_ms: f64,
    dummy_1in_prove_ms: f64,
    hop2_prove_ms: f64,
    bob_offline_scan_verified: bool,
}

#[derive(Serialize)]
struct BenchJson {
    trials: usize,
    rayon_threads: usize,
    prove_ms_med: f64,
    prove_ms_p95: f64,
    prove_ms_max: f64,
    verify_1c_ms_med: f64,
    verify_1c_ms_p95: f64,
    verify_1c_ms_max: f64,
    wire_bytes_min: usize,
    wire_bytes_med: usize,
    wire_bytes_max: usize,
    budget_bytes: usize,
}

#[derive(Serialize)]
struct IrConstraintJson {
    lin: Vec<[u32; 2]>,
    quad: Vec<[u32; 2]>,
    rhs: u8,
}

#[derive(Serialize)]
struct IrDumpJson {
    num_group_c: usize,
    constraints: Vec<IrConstraintJson>,
    alpha_hex: String,
    r1_hex: Vec<String>,
    wb_hex: Vec<String>,
    wc_hex: Vec<String>,
    pub_rhs_hex: String,
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn cmd_generate(out_dir: &str) -> Result<(), String> {
    fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let mut rng = ChaCha20Rng::from_seed([0x5Au8; 32]);

    let alice = ShieldedWallet::from_seed(&mut rng);
    let bob = ShieldedWallet::from_seed(&mut rng);
    let carol = ShieldedWallet::from_seed(&mut rng);

    let mut ledger = LedgerState::new();

    // Populate initial background Depth-24 pool with 98 coins (simulated prior shielded deposits)
    for i in 0..98usize {
        let mut s_bg = [0u8; OWNER_SECRET_BYTES];
        let mut r_bg = [0u8; BLIND_SECRET_BYTES];
        rng.fill_bytes(&mut s_bg);
        rng.fill_bytes(&mut r_bg);
        let a_bg = owner_addr(&s_bg);
        let c_bg = coin_commit(&a_bg, 500 + (i as u64), &r_bg);
        ledger.mint_coin(c_bg)?;
    }

    // Snapshot ledger BEFORE Scenario 0 (ZK-Proven Shielded Mint: A and R NEVER on-chain!)
    fs::write(
        PathBuf::from(out_dir).join("ledger_mint.json"),
        serde_json::to_string_pretty(&ledger.to_json()).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    // Scenario 0: ZK-Shielded Deposit (v_mint = 1000, is_dummy0 = is_dummy1 = true -> Alice 700 + Alice 290, fee 10)
    let zero_dest = [0u8; UNSHIELD_DEST_BYTES];
    let t_mint = Instant::now();
    let (tx_mint, r_alice_m0, r_alice_m1) = create_shielded_tx(
        &ledger.tree,
        SpendInput::Dummy,
        SpendInput::Dummy,
        &alice.stealth_address(),
        700,
        &alice.stealth_address(),
        290,
        1000, // v_mint
        0,    // v_unshield
        zero_dest,
        10,   // fee
        &mut rng,
    )?;
    let mint_prove_ms = t_mint.elapsed().as_secs_f64() * 1000.0;
    let tx_mint_bytes = tx_mint.to_bytes();
    assert!(
        tx_mint_bytes.len() <= W1_ARCHIVAL_BUDGET_BYTES,
        "Mint tx {} > budget {}",
        tx_mint_bytes.len(),
        W1_ARCHIVAL_BUDGET_BYTES
    );
    fs::write(PathBuf::from(out_dir).join("tx_mint.bin"), &tx_mint_bytes)
        .map_err(|e| e.to_string())?;
    let (alice_idx0, alice_idx1) = ledger.verify_and_apply(&tx_mint)?;

    // Verify Alice recovers her ZK-minted stealth notes offline!
    let (m0_v, m0_r) = alice
        .try_scan_output(&tx_mint.coin_out0, &tx_mint.env0)
        .ok_or("Alice failed to scan ZK-minted note 0")?;
    let (m1_v, m1_r) = alice
        .try_scan_output(&tx_mint.coin_out1, &tx_mint.env1)
        .ok_or("Alice failed to scan ZK-minted note 1")?;
    assert_eq!((m0_v, m0_r), (700, r_alice_m0));
    assert_eq!((m1_v, m1_r), (290, r_alice_m1));

    // Snapshot ledger BEFORE Hop 1
    let ledger_pre_hop1 = ledger.clone();
    fs::write(
        PathBuf::from(out_dir).join("ledger_hop1.json"),
        serde_json::to_string_pretty(&ledger_pre_hop1.to_json()).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    // Scenario 1: 2-real-input 0-round shielded payment (Alice -> offline Bob 650 + Alice change 330, fee 10)
    let t0 = Instant::now();
    let (tx_hop1, r_bob_note, r_alice_chg) = create_shielded_tx(
        &ledger.tree,
        SpendInput::Real {
            leaf_index: alice_idx0,
            spend_secret: alice.spend_secret,
            value: 700,
            r_blind: m0_r,
        },
        SpendInput::Real {
            leaf_index: alice_idx1,
            spend_secret: alice.spend_secret,
            value: 290,
            r_blind: m1_r,
        },
        &bob.stealth_address(),
        650,
        &alice.stealth_address(),
        330,
        0,
        0,
        zero_dest,
        10,
        &mut rng,
    )?;
    let hop1_prove_ms = t0.elapsed().as_secs_f64() * 1000.0;
    let tx_hop1_bytes = tx_hop1.to_bytes();
    assert!(
        tx_hop1_bytes.len() <= W1_ARCHIVAL_BUDGET_BYTES,
        "Hop-1 tx {} > budget {}",
        tx_hop1_bytes.len(),
        W1_ARCHIVAL_BUDGET_BYTES
    );
    fs::write(PathBuf::from(out_dir).join("tx_hop1.bin"), &tx_hop1_bytes)
        .map_err(|e| e.to_string())?;
    let (bob_leaf_idx, alice_chg_idx) = ledger.verify_and_apply(&tx_hop1)?;

    // Save Bob's wallet fixture for independent Go ML-KEM-1024 offline scanner cross-check!
    let bob_fixture = WalletFixtureJson {
        spend_secret_hex: hex::encode(bob.spend_secret),
        kem_seed_hex: hex::encode(bob.kem_seed),
        addr_hex: hex::encode(bob.addr),
        ek_hex: hex::encode(bob.ek_bytes),
        expected_value: 650,
        expected_r_hex: hex::encode(r_bob_note),
        expected_coin_hex: hex::encode(tx_hop1.coin_out0),
    };
    fs::write(
        PathBuf::from(out_dir).join("bob_wallet.json"),
        serde_json::to_string_pretty(&bob_fixture).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    // Verify Alice can offline-scan her own change AND Bob cannot decrypt Alice's change!
    let (alice_v_rec, alice_r_rec) = alice
        .try_scan_output(&tx_hop1.coin_out1, &tx_hop1.env1)
        .ok_or("Alice failed to scan her own change output")?;
    assert_eq!(alice_v_rec, 330);
    assert_eq!(alice_r_rec, r_alice_chg);
    assert!(bob.try_scan_output(&tx_hop1.coin_out1, &tx_hop1.env1).is_none());

    // Snapshot ledger BEFORE Scenario 2 (1-input native is_dummy spend!)
    fs::write(
        PathBuf::from(out_dir).join("ledger_dummy_1in.json"),
        serde_json::to_string_pretty(&ledger.to_json()).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    // Scenario 2: 1-real + 1-native-dummy shielded spend (Alice spends her 330 change + dummy -> Carol 200 + Alice 125, fee 5)
    let t1 = Instant::now();
    let (tx_dummy_1in, _r_carol1, _r_alice2) = create_shielded_tx(
        &ledger.tree,
        SpendInput::Real {
            leaf_index: alice_chg_idx,
            spend_secret: alice.spend_secret,
            value: 330,
            r_blind: alice_r_rec,
        },
        SpendInput::Dummy,
        &carol.stealth_address(),
        200,
        &alice.stealth_address(),
        125,
        0,
        0,
        zero_dest,
        5,
        &mut rng,
    )?;
    let dummy_1in_prove_ms = t1.elapsed().as_secs_f64() * 1000.0;
    let tx_dummy_1in_bytes = tx_dummy_1in.to_bytes();
    assert!(tx_dummy_1in_bytes.len() <= W1_ARCHIVAL_BUDGET_BYTES);
    fs::write(
        PathBuf::from(out_dir).join("tx_dummy_1in.bin"),
        &tx_dummy_1in_bytes,
    )
    .map_err(|e| e.to_string())?;
    ledger.verify_and_apply(&tx_dummy_1in)?;

    // Intervening ledger deposits (25 new background coins in Depth-24 pool)
    for i in 0..25u64 {
        let mut s_bg = [0u8; OWNER_SECRET_BYTES];
        let mut r_bg = [0u8; BLIND_SECRET_BYTES];
        rng.fill_bytes(&mut s_bg);
        rng.fill_bytes(&mut r_bg);
        let c_bg = coin_commit(&owner_addr(&s_bg), 1000 + i, &r_bg);
        ledger.mint_coin(c_bg)?;
    }

    // Bob wakes up offline, scans tx_hop1.bin from public bytes, recovers (650, r_bob_note)!
    let parsed_hop1 = ShieldedTx::from_bytes(&tx_hop1_bytes)?;
    let (bob_val, bob_r) = bob
        .try_scan_output(&parsed_hop1.coin_out0, &parsed_hop1.env0)
        .ok_or("Bob offline stealth scan failed!")?;
    assert_eq!(bob_val, 650);
    assert_eq!(bob_r, r_bob_note);

    // Snapshot ledger BEFORE Hop 2
    fs::write(
        PathBuf::from(out_dir).join("ledger_hop2.json"),
        serde_json::to_string_pretty(&ledger.to_json()).unwrap(),
    )
    .map_err(|e| e.to_string())?;

    // Scenario 3 (Hop 2 multi-hop spend + partial transparent unshield):
    // Bob spends his offline-received 650 note + native dummy -> Carol 390 + Bob 150 + transparent unshield 100 + fee 10!
    let unshield_dest = [0xEEu8; UNSHIELD_DEST_BYTES];
    let t2 = Instant::now();
    let (tx_hop2, _r_carol2, _r_bob_chg) = create_shielded_tx(
        &ledger.tree,
        SpendInput::Real {
            leaf_index: bob_leaf_idx,
            spend_secret: bob.spend_secret,
            value: bob_val,
            r_blind: bob_r,
        },
        SpendInput::Dummy,
        &carol.stealth_address(),
        390,
        &bob.stealth_address(),
        150,
        0,
        100, // v_unshield (transparent L1 exit)
        unshield_dest,
        10,
        &mut rng,
    )?;
    let hop2_prove_ms = t2.elapsed().as_secs_f64() * 1000.0;
    let tx_hop2_bytes = tx_hop2.to_bytes();
    assert!(tx_hop2_bytes.len() <= W1_ARCHIVAL_BUDGET_BYTES);
    fs::write(PathBuf::from(out_dir).join("tx_hop2.bin"), &tx_hop2_bytes)
        .map_err(|e| e.to_string())?;
    ledger.verify_and_apply(&tx_hop2)?;

    let summary = GenerateSummary {
        protocol: "Peel-Binius-Shielded-v2",
        merkle_depth: MERKLE_DEPTH,
        global_pool_capacity: 1usize << MERKLE_DEPTH,
        tx_mint_bytes: tx_mint_bytes.len(),
        tx_hop1_bytes: tx_hop1_bytes.len(),
        tx_dummy_1in_bytes: tx_dummy_1in_bytes.len(),
        tx_hop2_bytes: tx_hop2_bytes.len(),
        budget_bytes: W1_ARCHIVAL_BUDGET_BYTES,
        mint_prove_ms,
        hop1_prove_ms,
        dummy_1in_prove_ms,
        hop2_prove_ms,
        bob_offline_scan_verified: true,
    };
    println!("{}", serde_json::to_string_pretty(&summary).unwrap());
    Ok(())
}

fn cmd_verify(tx_path: &str, ledger_path: &str) -> Result<(), String> {
    let tx_bytes = fs::read(tx_path).map_err(|e| e.to_string())?;
    let ledger_str = fs::read_to_string(ledger_path).map_err(|e| e.to_string())?;
    let lj: LedgerJson = serde_json::from_str(&ledger_str).map_err(|e| e.to_string())?;
    let mut ledger = LedgerState::from_json(&lj)?;
    let tx = ShieldedTx::from_bytes(&tx_bytes)?;
    let t0 = Instant::now();
    ledger.verify_and_apply(&tx)?;
    let ms = t0.elapsed().as_secs_f64() * 1000.0;
    println!(
        "RUST VERIFY PASS: {} ({} B <= {} B budget) in {:.2} ms",
        tx_path,
        tx_bytes.len(),
        W1_ARCHIVAL_BUDGET_BYTES,
        ms
    );
    Ok(())
}

fn cmd_adversarial(tx_path: &str, ledger_path: &str) -> Result<(), String> {
    let tx_bytes = fs::read(tx_path).map_err(|e| e.to_string())?;
    let ledger_str = fs::read_to_string(ledger_path).map_err(|e| e.to_string())?;
    let lj: LedgerJson = serde_json::from_str(&ledger_str).map_err(|e| e.to_string())?;
    let base_ledger = LedgerState::from_json(&lj)?;
    let base_tx = ShieldedTx::from_bytes(&tx_bytes)?;
    base_ledger.verify_tx(&base_tx)?;

    let mut passed = 0usize;
    let mut check_reject = |name: &str, res: Result<(), String>| -> Result<(), String> {
        if res.is_ok() {
            return Err(format!("ADVERSARIAL CONTROL FAILED (ACCEPTED): {}", name));
        }
        passed += 1;
        Ok(())
    };

    // 1. Unknown/forged Merkle anchor
    {
        let mut tx = base_tx.clone();
        tx.anchor[0] ^= 0x01;
        check_reject("forged_anchor", base_ledger.verify_tx(&tx))?;
    }
    // 2. Stale/unknown anchor even if ZKP is skipped -> anchor check rejects
    {
        let mut l = base_ledger.clone();
        l.anchors.remove(&base_tx.anchor);
        check_reject("missing_anchor_in_ledger", l.verify_tx(&base_tx))?;
    }
    // 3. Double spend sn0
    {
        let mut l = base_ledger.clone();
        l.spent_sn.insert(base_tx.sn0);
        check_reject("double_spend_sn0", l.verify_tx(&base_tx))?;
    }
    // 4. Double spend sn1
    {
        let mut l = base_ledger.clone();
        l.spent_sn.insert(base_tx.sn1);
        check_reject("double_spend_sn1", l.verify_tx(&base_tx))?;
    }
    // 5. Duplicate within-tx nullifier sn0 == sn1
    {
        let mut tx = base_tx.clone();
        tx.sn1 = tx.sn0;
        check_reject("duplicate_sn_in_tx", base_ledger.verify_tx(&tx))?;
    }
    // 6. Rule A3b: duplicate output coin commitments coin_out0 == coin_out1
    {
        let mut tx = base_tx.clone();
        tx.coin_out1 = tx.coin_out0;
        check_reject("rule_a3b_dup_outputs", base_ledger.verify_tx(&tx))?;
    }
    // 7. Rule A3b: output coin0 already exists in global ledger
    {
        let mut l = base_ledger.clone();
        l.coin_set.insert(base_tx.coin_out0);
        check_reject("rule_a3b_existing_coin0", l.verify_tx(&base_tx))?;
    }
    // 8. Tampered fee
    {
        let mut tx = base_tx.clone();
        tx.fee = tx.fee.wrapping_add(1);
        check_reject("tampered_fee", base_ledger.verify_tx(&tx))?;
    }
    // 9. Tampered sn0
    {
        let mut tx = base_tx.clone();
        tx.sn0[10] ^= 0x80;
        check_reject("tampered_sn0", base_ledger.verify_tx(&tx))?;
    }
    // 10. Tampered coin_out0
    {
        let mut tx = base_tx.clone();
        tx.coin_out0[5] ^= 0x40;
        check_reject("tampered_coin_out0", base_ledger.verify_tx(&tx))?;
    }
    // 11. Tampered env0 (relayer malleability attack — caught by tx_ctx binding!)
    {
        let mut tx = base_tx.clone();
        tx.env0[100] ^= 0x01;
        check_reject("relayer_malleability_env0", base_ledger.verify_tx(&tx))?;
    }
    // 12. Tampered env1 (relayer malleability attack — caught by tx_ctx binding!)
    {
        let mut tx = base_tx.clone();
        tx.env1[1669] ^= 0x01;
        check_reject("relayer_malleability_env1", base_ledger.verify_tx(&tx))?;
    }
    // 13. Tampered MT_1 root
    {
        let mut tx = base_tx.clone();
        tx.proof.merkle_root[0] ^= 0x01;
        check_reject("tampered_mt1_root", base_ledger.verify_tx(&tx))?;
    }
    // 14. Tampered s_g1
    {
        let mut tx = base_tx.clone();
        tx.proof.s_g1.c[0] ^= 1;
        check_reject("tampered_s_g1", base_ledger.verify_tx(&tx))?;
    }
    // 15. Tampered s_mask2
    {
        let mut tx = base_tx.clone();
        tx.proof.s_mask2.c[0] ^= 1;
        check_reject("tampered_s_mask2", base_ledger.verify_tx(&tx))?;
    }
    // 16. Tampered sc1_rounds
    {
        let mut tx = base_tx.clone();
        tx.proof.sc1_rounds[7][1].c[3] ^= 1;
        check_reject("tampered_sc1_round", base_ledger.verify_tx(&tx))?;
    }
    // 17. Tampered eval_c1
    {
        let mut tx = base_tx.clone();
        tx.proof.eval_c1.c[0] ^= 1;
        check_reject("tampered_eval_c1", base_ledger.verify_tx(&tx))?;
    }
    // 18. Tampered eval_z
    {
        let mut tx = base_tx.clone();
        tx.proof.eval_z[2].c[0] ^= 1;
        check_reject("tampered_eval_z", base_ledger.verify_tx(&tx))?;
    }
    // 19. Tampered rs_s_prime
    {
        let mut tx = base_tx.clone();
        tx.proof.rs_s_prime[15].c[0] ^= 1;
        check_reject("tampered_rs_s_prime", base_ledger.verify_tx(&tx))?;
    }
    // 20. Tampered sc2_rounds
    {
        let mut tx = base_tx.clone();
        tx.proof.sc2_rounds[3][0].c[0] ^= 1;
        check_reject("tampered_sc2_round", base_ledger.verify_tx(&tx))?;
    }
    // 21. Tampered b_eval
    {
        let mut tx = base_tx.clone();
        tx.proof.b_eval.c[0] ^= 1;
        check_reject("tampered_b_eval", base_ledger.verify_tx(&tx))?;
    }
    // 22. Tampered v_prox_coeffs
    {
        let mut tx = base_tx.clone();
        tx.proof.v_prox_coeffs[100].c[0] ^= 1;
        check_reject("tampered_v_prox", base_ledger.verify_tx(&tx))?;
    }
    // 23. Tampered v_open_coeffs
    {
        let mut tx = base_tx.clone();
        tx.proof.v_open_coeffs[200].c[0] ^= 1;
        check_reject("tampered_v_open", base_ledger.verify_tx(&tx))?;
    }
    // 24. Tampered pow_nonce
    {
        let mut tx = base_tx.clone();
        tx.proof.pow_nonce[0] ^= 0xFF;
        check_reject("tampered_pow_nonce", base_ledger.verify_tx(&tx))?;
    }
    // 25. Tampered opened column cell
    {
        let mut tx = base_tx.clone();
        tx.proof.opened_cols[0].k_vals[10] ^= 1;
        check_reject("tampered_opened_col", base_ledger.verify_tx(&tx))?;
    }
    // 26. Tampered Merkle sibling
    {
        let mut tx = base_tx.clone();
        tx.proof.merkle_siblings[0][0] ^= 1;
        check_reject("tampered_merkle_sibling", base_ledger.verify_tx(&tx))?;
    }
    // 27. Truncated & trailing bytes parser controls
    {
        let mut trunc = tx_bytes.clone();
        trunc.pop();
        check_reject("truncated_tx", ShieldedTx::from_bytes(&trunc).map(|_| ()))?;
        let mut trail = tx_bytes.clone();
        trail.push(0x00);
        check_reject("trailing_bytes_tx", ShieldedTx::from_bytes(&trail).map(|_| ()))?;
    }
    // 28. Rule R4: zero coin output sentinel 0^48 rejected
    {
        let mut tx = base_tx.clone();
        tx.coin_out0 = [0u8; 48];
        check_reject("rule_r4_zero_coin_output", base_ledger.verify_tx(&tx))?;
    }
    // 29. Rule R6: Depth-24 pool capacity overflow (|Coins| + 2 > 2^24) rejected
    {
        let mut l = base_ledger.clone();
        l.leaf_count_override = Some(MAX_POOL_COINS - 1);
        check_reject("rule_r6_tree_capacity_overflow", l.verify_tx(&base_tx))?;
    }
    // 30. Rule R5: mutually exclusive v_mint > 0 and v_unshield > 0 rejected
    {
        let mut tx = base_tx.clone();
        tx.v_mint = 10;
        tx.v_unshield = 10;
        check_reject("rule_r5_mint_and_unshield_both_nonzero", base_ledger.verify_tx(&tx))?;
    }
    // 31. Rule R5: fee + v_unshield u64 overflow rejected
    {
        let mut tx = base_tx.clone();
        tx.v_unshield = u64::MAX;
        check_reject("rule_r5_fee_plus_unshield_overflow", base_ledger.verify_tx(&tx))?;
    }
    // 32. Context binding: tampered unshield_dest caught by tx_ctx!
    {
        let mut tx = base_tx.clone();
        tx.unshield_dest[0] ^= 0x01;
        check_reject("relayer_malleability_unshield_dest", base_ledger.verify_tx(&tx))?;
    }
    // 33. Soundness attack: malicious prover tries `is_dummy=1` with `v_in = 500` (inflation!) -> ZKP MUST REJECT!
    {
        let mut rng = ChaCha20Rng::from_seed([0xCCu8; 32]);
        let s0 = [0x11u8; OWNER_SECRET_BYTES];
        let s1 = [0x22u8; OWNER_SECRET_BYTES];
        let a0 = owner_addr(&s0);
        let a1 = owner_addr(&s1);
        let r0 = [0x33u8; BLIND_SECRET_BYTES];
        let r1 = [0x44u8; BLIND_SECRET_BYTES];
        let c0 = coin_commit(&a0, 500, &r0); // INFLATION: dummy coin with v_in=500!
        let c1 = coin_commit(&a1, 0, &r1);
        let a_out = owner_addr(&[0x55u8; OWNER_SECRET_BYTES]);
        let stmt = pft_binius_shielded::circuit::PaymentStatement {
            anchor: base_ledger.tree.root(),
            sn0: nullifier(&s0, &c0),
            sn1: nullifier(&s1, &c1),
            coin_out0: coin_commit(&a_out, 490, &r0),
            coin_out1: coin_commit(&a_out, 0, &r1),
            v_mint: 0,
            v_unshield: 0,
            fee: 10,
            tx_ctx: [0x11u8; 48],
        };
        let wit = PaymentWitness {
            is_dummy0: true, // maliciously sets is_dummy=1 with v_in0=500!
            is_dummy1: true,
            dirs0: [0u8; MERKLE_DEPTH],
            sibs0: [[0u8; NODE_BYTES]; MERKLE_DEPTH],
            dirs1: [0u8; MERKLE_DEPTH],
            sibs1: [[0u8; NODE_BYTES]; MERKLE_DEPTH],
            s_in0: s0,
            s_in1: s1,
            v_in0: 500,
            v_in1: 0,
            r_in0: r0,
            r_in1: r1,
            addr_out0: a_out,
            addr_out1: a_out,
            v_out0: 490,
            v_out1: 0,
            r_out0: r0,
            r_out1: r1,
        };
        let bits = build_witness_bits(&stmt, &wit);
        let bad_proof = prove(&stmt, &bits, &mut rng);
        check_reject("soundness_dummy_inflation_attack", verify(&stmt, &bad_proof))?;
    }
    // 29. Soundness attack: malicious prover tries `is_dummy=0` with forged Merkle path -> ZKP MUST REJECT!
    {
        let mut rng = ChaCha20Rng::from_seed([0xDDu8; 32]);
        let s0 = [0x12u8; OWNER_SECRET_BYTES];
        let s1 = [0x23u8; OWNER_SECRET_BYTES];
        let a0 = owner_addr(&s0);
        let a1 = owner_addr(&s1);
        let r0 = [0x34u8; BLIND_SECRET_BYTES];
        let r1 = [0x45u8; BLIND_SECRET_BYTES];
        let c0 = coin_commit(&a0, 500, &r0);
        let c1 = coin_commit(&a1, 0, &r1);
        let a_out = owner_addr(&[0x56u8; OWNER_SECRET_BYTES]);
        let stmt = pft_binius_shielded::circuit::PaymentStatement {
            anchor: base_ledger.tree.root(),
            sn0: nullifier(&s0, &c0),
            sn1: nullifier(&s1, &c1),
            coin_out0: coin_commit(&a_out, 490, &r0),
            coin_out1: coin_commit(&a_out, 0, &r1),
            v_mint: 0,
            v_unshield: 0,
            fee: 10,
            tx_ctx: [0x22u8; 48],
        };
        let wit = PaymentWitness {
            is_dummy0: false, // claims real coin in global tree, but path is bogus!
            is_dummy1: true,
            dirs0: [0u8; MERKLE_DEPTH],
            sibs0: [[0xABu8; NODE_BYTES]; MERKLE_DEPTH],
            dirs1: [0u8; MERKLE_DEPTH],
            sibs1: [[0u8; NODE_BYTES]; MERKLE_DEPTH],
            s_in0: s0,
            s_in1: s1,
            v_in0: 500,
            v_in1: 0,
            r_in0: r0,
            r_in1: r1,
            addr_out0: a_out,
            addr_out1: a_out,
            v_out0: 490,
            v_out1: 0,
            r_out0: r0,
            r_out1: r1,
        };
        let bits = build_witness_bits(&stmt, &wit);
        let bad_proof = prove(&stmt, &bits, &mut rng);
        check_reject("soundness_forged_merkle_path_attack", verify(&stmt, &bad_proof))?;
    }

    // 150-trial deterministic structured byte fuzzer
    let mut fuzz_rng = ChaCha20Rng::from_seed([0xF0u8; 32]);
    let mut fuzz_pass = 0usize;
    for _ in 0..150 {
        let mut mutated = tx_bytes.clone();
        let pos = (fuzz_rng.next_u64() as usize) % mutated.len();
        let bit = 1u8 << ((fuzz_rng.next_u32() as usize) % 8);
        mutated[pos] ^= bit;
        let accepted = ShieldedTx::from_bytes(&mutated)
            .and_then(|tx| base_ledger.verify_tx(&tx))
            .is_ok();
        if accepted {
            return Err(format!("FUZZ MUTATION ACCEPTED at byte {}", pos));
        }
        fuzz_pass += 1;
    }

    println!(
        "RUST ADVERSARIAL PASS: {}/{} named controls rejected (including live dummy-inflation & forged-Merkle-path ZKP soundness attacks!) + {}/150 structured fuzz mutations rejected",
        passed, passed, fuzz_pass
    );
    Ok(())
}

fn cmd_bench(trials: usize, out_path: &str) -> Result<(), String> {
    let mut rng = ChaCha20Rng::from_seed([0x7Bu8; 32]);
    let alice = ShieldedWallet::from_seed(&mut rng);
    let bob = ShieldedWallet::from_seed(&mut rng);
    let mut ledger = LedgerState::new();
    let mut r_in0 = [0u8; BLIND_SECRET_BYTES];
    let mut r_in1 = [0u8; BLIND_SECRET_BYTES];
    rng.fill_bytes(&mut r_in0);
    rng.fill_bytes(&mut r_in1);
    let idx0 = ledger.mint_coin(coin_commit(&alice.addr, 700, &r_in0))?;
    let idx1 = ledger.mint_coin(coin_commit(&alice.addr, 300, &r_in1))?;

    // Warmup
    let _ = k_tables();
    let zero_dest = [0u8; UNSHIELD_DEST_BYTES];
    let (warm_tx, _, _) = create_shielded_tx(
        &ledger.tree,
        SpendInput::Real { leaf_index: idx0, spend_secret: alice.spend_secret, value: 700, r_blind: r_in0 },
        SpendInput::Real { leaf_index: idx1, spend_secret: alice.spend_secret, value: 300, r_blind: r_in1 },
        &bob.stealth_address(), 650,
        &alice.stealth_address(), 340,
        0, 0, zero_dest, 10,
        &mut rng,
    )?;
    ledger.verify_tx(&warm_tx)?;

    let mut prove_ms = Vec::with_capacity(trials);
    let mut verify_ms = Vec::with_capacity(trials);
    let mut sizes = Vec::with_capacity(trials);
    for _ in 0..trials {
        let t0 = Instant::now();
        let (tx, _, _) = create_shielded_tx(
            &ledger.tree,
            SpendInput::Real { leaf_index: idx0, spend_secret: alice.spend_secret, value: 700, r_blind: r_in0 },
            SpendInput::Real { leaf_index: idx1, spend_secret: alice.spend_secret, value: 300, r_blind: r_in1 },
            &bob.stealth_address(), 650,
            &alice.stealth_address(), 340,
            0, 0, zero_dest, 10,
            &mut rng,
        )?;
        prove_ms.push(t0.elapsed().as_secs_f64() * 1000.0);
        let bytes = tx.to_bytes();
        sizes.push(bytes.len());
        let parsed = ShieldedTx::from_bytes(&bytes)?;
        let t1 = Instant::now();
        ledger.verify_tx(&parsed)?;
        verify_ms.push(t1.elapsed().as_secs_f64() * 1000.0);
    }
    prove_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    verify_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    sizes.sort_unstable();

    let res = BenchJson {
        trials,
        rayon_threads: rayon::current_num_threads(),
        prove_ms_med: percentile(&prove_ms, 0.50),
        prove_ms_p95: percentile(&prove_ms, 0.95),
        prove_ms_max: *prove_ms.last().unwrap(),
        verify_1c_ms_med: percentile(&verify_ms, 0.50),
        verify_1c_ms_p95: percentile(&verify_ms, 0.95),
        verify_1c_ms_max: *verify_ms.last().unwrap(),
        wire_bytes_min: sizes[0],
        wire_bytes_med: sizes[sizes.len() / 2],
        wire_bytes_max: *sizes.last().unwrap(),
        budget_bytes: W1_ARCHIVAL_BUDGET_BYTES,
    };
    let json = serde_json::to_string_pretty(&res).unwrap();
    fs::write(out_path, &json).map_err(|e| e.to_string())?;
    println!("{}", json);
    Ok(())
}

fn cmd_dump_ir(tx_path: &str, out_path: &str) -> Result<(), String> {
    let tx_bytes = fs::read(tx_path).map_err(|e| e.to_string())?;
    let tx = ShieldedTx::from_bytes(&tx_bytes)?;
    let stmt = tx.statement();
    let cs = build_group_c_constraints(&stmt);
    let constraints: Vec<IrConstraintJson> = cs
        .iter()
        .map(|c| IrConstraintJson {
            lin: c.lin.iter().map(|&(x, r)| [x as u32, r]).collect(),
            quad: c.quad.iter().map(|&(x, r)| [x as u32, r]).collect(),
            rhs: c.rhs,
        })
        .collect();

    let kt = k_tables();
    let alpha = E {
        c: core::array::from_fn(|i| 0x1357_2468u32.wrapping_mul((i + 1) as u32)),
    };
    let ch = CircuitChallenge { alpha };
    let r1: [E; V_SC1] = core::array::from_fn(|k| E {
        c: core::array::from_fn(|i| ((k * 17 + i * 11 + 5) as u32).wrapping_mul(0x9E37_79B9)),
    });
    let (wb, wc, pub_rhs) = eval_verifier_weights_at_r1(&stmt, &ch, &r1, kt);
    let dump = IrDumpJson {
        num_group_c: constraints.len(),
        constraints,
        alpha_hex: hex::encode(alpha.to_bytes()),
        r1_hex: r1.iter().map(|e| hex::encode(e.to_bytes())).collect(),
        wb_hex: wb.iter().map(|e| hex::encode(e.to_bytes())).collect(),
        wc_hex: wc.iter().map(|e| hex::encode(e.to_bytes())).collect(),
        pub_rhs_hex: hex::encode(pub_rhs.to_bytes()),
    };
    fs::write(out_path, serde_json::to_string(&dump).unwrap()).map_err(|e| e.to_string())?;
    println!(
        "Dumped {} Group-C IR constraints + exact Group A+B+C weights to {}",
        dump.num_group_c, out_path
    );
    Ok(())
}

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: pft-binius-shielded <generate|verify|adversarial|bench|dump-ir> ...");
        std::process::exit(1);
    }
    let get_arg = |flag: &str| -> Option<String> {
        args.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
    };
    let res = match args[1].as_str() {
        "generate" => {
            let out = get_arg("--out").unwrap_or_else(|| "../payments".into());
            cmd_generate(&out)
        }
        "verify" => {
            let tx = get_arg("--tx").expect("--tx required");
            let ledger = get_arg("--ledger").expect("--ledger required");
            cmd_verify(&tx, &ledger)
        }
        "adversarial" => {
            let tx = get_arg("--tx").expect("--tx required");
            let ledger = get_arg("--ledger").expect("--ledger required");
            cmd_adversarial(&tx, &ledger)
        }
        "bench" => {
            let trials: usize = get_arg("--trials").and_then(|s| s.parse().ok()).unwrap_or(10);
            let out = get_arg("--out").unwrap_or_else(|| "../bench_results.json".into());
            cmd_bench(trials, &out)
        }
        "dump-ir" => {
            let tx = get_arg("--tx").expect("--tx required");
            let out = get_arg("--out").expect("--out required");
            cmd_dump_ir(&tx, &out)
        }
        other => Err(format!("unknown subcommand: {}", other)),
    };
    if let Err(e) = res {
        eprintln!("ERROR: {}", e);
        std::process::exit(1);
    }
}
