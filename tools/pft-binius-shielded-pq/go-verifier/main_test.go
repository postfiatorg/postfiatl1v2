package main

import (
	"os"
	"testing"
)

func TestPCLMULQDQEquivalence10000(t *testing.T) {
	kt := getKTables()
	var s uint64 = 0x123456789ABCDEF0
	nextU32 := func() uint32 {
		s ^= s << 13
		s ^= s >> 7
		s ^= s << 17
		return uint32(s)
	}
	for i := 0; i < 10000; i++ {
		var a, b E
		for j := 0; j < 12; j++ {
			a.C[j] = nextU32()
			b.C[j] = nextU32()
		}
		got := a.mulPCLMUL(b)
		want := a.mulPortable(b, kt)
		if got != want {
			t.Fatalf("mulPCLMUL mismatch at iter %d", i)
		}
	}
	// Also test RingSwitchLinearMap ASM vs portable on 1000 random inputs
	wRS := make([]E, 384)
	for i := range wRS {
		for j := 0; j < 12; j++ {
			wRS[i].C[j] = nextU32()
		}
	}
	lMap := newRingSwitchLinearMap(wRS)
	for i := 0; i < 1000; i++ {
		var z E
		for j := 0; j < 12; j++ {
			z.C[j] = nextU32()
		}
		if lMap.applyFast(z) != lMap.applyPortable(z) {
			t.Fatalf("ringSwitchApplyASM mismatch at iter %d", i)
		}
	}
	// Also test 3D-Karatsuba Ring-Switch inner product vs naive 384x384 bilinear sum
	var rArr, vArr [12][2][16]E
	for v := 0; v < 12; v++ {
		for z := 0; z < 2; z++ {
			for u := 0; u < 16; u++ {
				for j := 0; j < 12; j++ {
					rArr[v][z][u].C[j] = nextU32()
					vArr[v][z][u].C[j] = nextU32()
				}
			}
		}
	}
	fastIP := evalBilinearRSIP(&rArr, &vArr, wRS, kt)
	var naiveIP E
	for a := 0; a < 384; a++ {
		ra := rArr[a/32][(a/16)&1][a&15]
		ba := EFromBasisVU(a/32, a%32)
		for b := 0; b < 384; b++ {
			vb := vArr[b/32][(b/16)&1][b&15]
			bb := EFromBasisVU(b/32, b%32)
			wab := lMap.applyFast(ba.Mul(bb, kt))
			naiveIP.AddAssign(ra.Mul(vb, kt).Mul(wab, kt))
		}
	}
	if fastIP != naiveIP {
		t.Fatal("Go 3D-Karatsuba evalBilinearRSIP diverged from naive 384x384 bilinear sum!")
	}
}

func TestGoIndependentVerificationAndAdversarialControls(t *testing.T) {
	// Verify all 4 real generated lifecycle scenarios:
	// ZK-Shielded Mint, Hop-1 (2-in/2-out), Dummy 1-in, Hop-2 (multi-hop + partial transparent unshield)
	scenarios := [][2]string{
		{"../payments/tx_mint.bin", "../payments/ledger_mint.json"},
		{"../payments/tx_hop1.bin", "../payments/ledger_hop1.json"},
		{"../payments/tx_dummy_1in.bin", "../payments/ledger_dummy_1in.json"},
		{"../payments/tx_hop2.bin", "../payments/ledger_hop2.json"},
	}
	for _, sc := range scenarios {
		raw, err := os.ReadFile(sc[0])
		if err != nil {
			t.Fatalf("read %s: %v", sc[0], err)
		}
		ls, err := loadLedger(sc[1])
		if err != nil {
			t.Fatalf("load %s: %v", sc[1], err)
		}
		tx, err := parseShieldedTx(raw)
		if err != nil {
			t.Fatalf("parse %s: %v", sc[0], err)
		}
		if err := verifyFullSettlement(ls, tx); err != nil {
			t.Fatalf("verify %s failed: %v", sc[0], err)
		}
	}

	raw, err := os.ReadFile("../payments/tx_hop1.bin")
	if err != nil {
		t.Fatal(err)
	}
	baseLS, err := loadLedger("../payments/ledger_hop1.json")
	if err != nil {
		t.Fatal(err)
	}

	cloneTx := func() *ShieldedTx {
		tx, err := parseShieldedTx(raw)
		if err != nil {
			t.Fatal(err)
		}
		return tx
	}
	cloneLS := func() *LedgerState {
		ls, err := loadLedger("../payments/ledger_hop1.json")
		if err != nil {
			t.Fatal(err)
		}
		return ls
	}

	expectReject := func(name string, ls *LedgerState, tx *ShieldedTx) {
		if err := verifyFullSettlement(ls, tx); err == nil {
			t.Fatalf("ADVERSARIAL CONTROL FAILED (ACCEPTED): %s", name)
		}
	}

	// 33 named independent Go adversarial controls (+2 parser controls = 35 total)
	tx := cloneTx(); tx.Anchor[0] ^= 1; expectReject("forged_anchor", baseLS, tx)
	ls := cloneLS(); delete(ls.Anchors, cloneTx().Anchor); expectReject("missing_anchor", ls, cloneTx())
	ls = cloneLS(); ls.SpentSN[cloneTx().SN0] = true; expectReject("double_spend_sn0", ls, cloneTx())
	ls = cloneLS(); ls.SpentSN[cloneTx().SN1] = true; expectReject("double_spend_sn1", ls, cloneTx())
	tx = cloneTx(); tx.SN1 = tx.SN0; expectReject("duplicate_sn_in_tx", baseLS, tx)
	tx = cloneTx(); tx.CoinOut0 = [CoinBytes]byte{}; expectReject("rule_r4_zero_coin0", baseLS, tx)
	ls = cloneLS(); ov := MaxPoolCoins - 1; ls.LeafCountOverride = &ov; expectReject("rule_r6_capacity_overflow", ls, cloneTx())
	tx = cloneTx(); tx.VMint = 10; tx.VUnshield = 10; expectReject("rule_r5_mint_and_unshield", baseLS, tx)
	tx = cloneTx(); tx.VUnshield = ^uint64(0); expectReject("rule_r5_fee_unshield_overflow", baseLS, tx)
	tx = cloneTx(); tx.UnshieldDest[0] ^= 1; expectReject("relayer_malleability_unshield_dest", baseLS, tx)
	tx = cloneTx(); tx.CoinOut1 = tx.CoinOut0; expectReject("rule_a3b_dup_outputs", baseLS, tx)
	ls = cloneLS(); ls.CoinSet[cloneTx().CoinOut0] = true; expectReject("rule_a3b_existing_coin0", ls, cloneTx())
	ls = cloneLS(); ls.CoinSet[cloneTx().CoinOut1] = true; expectReject("rule_a3b_existing_coin1", ls, cloneTx())
	tx = cloneTx(); tx.Fee++; expectReject("tampered_fee", baseLS, tx)
	tx = cloneTx(); tx.SN0[12] ^= 0x80; expectReject("tampered_sn0", baseLS, tx)
	tx = cloneTx(); tx.CoinOut0[7] ^= 0x04; expectReject("tampered_coin_out0", baseLS, tx)
	tx = cloneTx(); tx.CoinOut1[9] ^= 0x08; expectReject("tampered_coin_out1", baseLS, tx)
	tx = cloneTx(); tx.Env0[42] ^= 0x01; expectReject("relayer_malleability_env0", baseLS, tx)
	tx = cloneTx(); tx.Env1[1600] ^= 0x01; expectReject("relayer_malleability_env1", baseLS, tx)
	tx = cloneTx(); tx.Proof.MerkleRoot[0] ^= 1; expectReject("tampered_mt1_root", baseLS, tx)
	tx = cloneTx(); tx.Proof.SG1.C[0] ^= 1; expectReject("tampered_s_g1", baseLS, tx)
	tx = cloneTx(); tx.Proof.SMask2.C[0] ^= 1; expectReject("tampered_s_mask2", baseLS, tx)
	tx = cloneTx(); tx.Proof.SC1Rounds[5][1].C[2] ^= 1; expectReject("tampered_sc1_round", baseLS, tx)
	tx = cloneTx(); tx.Proof.EvalC1.C[0] ^= 1; expectReject("tampered_eval_c1", baseLS, tx)
	tx = cloneTx(); tx.Proof.EvalZ[1].C[0] ^= 1; expectReject("tampered_eval_z", baseLS, tx)
	tx = cloneTx(); tx.Proof.RSSPrime[11].C[0] ^= 1; expectReject("tampered_rs_s_prime", baseLS, tx)
	tx = cloneTx(); tx.Proof.SC2Rounds[4][0].C[0] ^= 1; expectReject("tampered_sc2_round", baseLS, tx)
	tx = cloneTx(); tx.Proof.BEval.C[0] ^= 1; expectReject("tampered_b_eval", baseLS, tx)
	tx = cloneTx(); tx.Proof.VProxCoeffs[50].C[0] ^= 1; expectReject("tampered_v_prox", baseLS, tx)
	tx = cloneTx(); tx.Proof.VOpenCoeffs[99].C[0] ^= 1; expectReject("tampered_v_open", baseLS, tx)
	tx = cloneTx(); tx.Proof.PowNonce[0] ^= 0xFF; expectReject("tampered_pow_nonce", baseLS, tx)
	tx = cloneTx(); tx.Proof.OpenedCols[0].KVals[3] ^= 1; expectReject("tampered_opened_col", baseLS, tx)
	tx = cloneTx(); tx.Proof.MerkleSiblings[0][0] ^= 1; expectReject("tampered_merkle_sibling", baseLS, tx)

	if _, err := parseShieldedTx(raw[:len(raw)-1]); err == nil {
		t.Fatal("truncated tx accepted")
	}
	withTrail := append(append([]byte{}, raw...), 0x00)
	if _, err := parseShieldedTx(withTrail); err == nil {
		t.Fatal("trailing bytes tx accepted")
	}
}
