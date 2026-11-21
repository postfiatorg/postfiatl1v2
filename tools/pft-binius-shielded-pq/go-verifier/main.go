// Independent Go 1.25.1 Settlement Verifier for Peel-Binius-Shielded (ZERO external dependencies!).
// Uses ONLY Go 1.25.1 standard library (crypto/sha512, crypto/sha3) + 60-line Plan 9 amd64 PCLMULQDQ.
package main

import (
	"crypto/sha3"
	"crypto/sha512"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"flag"
	"fmt"
	"os"
	"sort"
	"time"
)

const (
	KemCTBytes        = 1568
	NoteCTBytes       = 102
	EnvBytes          = KemCTBytes + NoteCTBytes // 1670
	UnshieldDestBytes = 32
	HeaderBytes       = 8 + NodeBytes + 8 + 8 + UnshieldDestBytes + 8 + 2*SNBytes + 2*CoinBytes + 2*EnvBytes + 4 // 3648
	BudgetBytes       = 149130
	MaxPoolCoins      = 1 << MerkleDepth // 16,777,216
)

var TxMagic = [8]byte{'P', 'F', 'S', 'H', '0', '2', 0, 0}

type ShieldedTx struct {
	Anchor       [NodeBytes]byte
	VMint        uint64
	VUnshield    uint64
	UnshieldDest [UnshieldDestBytes]byte
	Fee          uint64
	SN0          [SNBytes]byte
	SN1          [SNBytes]byte
	CoinOut0     [CoinBytes]byte
	CoinOut1     [CoinBytes]byte
	Env0         [EnvBytes]byte
	Env1         [EnvBytes]byte
	Proof        *BiniusProof
}

func (tx *ShieldedTx) statement() *PaymentStatement {
	h := sha512.New384()
	h.Write([]byte("PFT-SH-TXCTX-V2"))
	h.Write(TxMagic[:])
	h.Write(tx.Anchor[:])
	var b8 [8]byte
	binary.LittleEndian.PutUint64(b8[:], tx.VMint)
	h.Write(b8[:])
	binary.LittleEndian.PutUint64(b8[:], tx.VUnshield)
	h.Write(b8[:])
	h.Write(tx.UnshieldDest[:])
	binary.LittleEndian.PutUint64(b8[:], tx.Fee)
	h.Write(b8[:])
	h.Write(tx.SN0[:])
	h.Write(tx.SN1[:])
	h.Write(tx.CoinOut0[:])
	h.Write(tx.CoinOut1[:])
	h.Write(tx.Env0[:])
	h.Write(tx.Env1[:])
	var txCtx [48]byte
	h.Sum(txCtx[:0])
	return &PaymentStatement{
		Anchor:    tx.Anchor,
		SN0:       tx.SN0,
		SN1:       tx.SN1,
		CoinOut0:  tx.CoinOut0,
		CoinOut1:  tx.CoinOut1,
		VMint:     tx.VMint,
		VUnshield: tx.VUnshield,
		Fee:       tx.Fee,
		TxCtx:     txCtx,
	}
}

func parseShieldedTx(data []byte) (*ShieldedTx, error) {
	if len(data) < HeaderBytes {
		return nil, errors.New("truncated shielded tx header")
	}
	pos := 0
	take := func(n int) ([]byte, error) {
		if pos+n > len(data) {
			return nil, errors.New("truncated shielded tx")
		}
		s := data[pos : pos+n]
		pos += n
		return s, nil
	}
	magic, err := take(8)
	if err != nil {
		return nil, err
	}
	for i := 0; i < 8; i++ {
		if magic[i] != TxMagic[i] {
			return nil, errors.New("bad shielded tx magic")
		}
	}
	tx := &ShieldedTx{}
	b, _ := take(NodeBytes)
	copy(tx.Anchor[:], b)
	mb, _ := take(8)
	tx.VMint = binary.LittleEndian.Uint64(mb)
	ub, _ := take(8)
	tx.VUnshield = binary.LittleEndian.Uint64(ub)
	db, _ := take(UnshieldDestBytes)
	copy(tx.UnshieldDest[:], db)
	fb, _ := take(8)
	tx.Fee = binary.LittleEndian.Uint64(fb)
	b, _ = take(SNBytes)
	copy(tx.SN0[:], b)
	b, _ = take(SNBytes)
	copy(tx.SN1[:], b)
	b, _ = take(CoinBytes)
	copy(tx.CoinOut0[:], b)
	b, _ = take(CoinBytes)
	copy(tx.CoinOut1[:], b)
	b, _ = take(EnvBytes)
	copy(tx.Env0[:], b)
	b, _ = take(EnvBytes)
	copy(tx.Env1[:], b)
	plb, _ := take(4)
	proofLen := int(binary.LittleEndian.Uint32(plb))
	proofBytes, err := take(proofLen)
	if err != nil {
		return nil, err
	}
	if pos != len(data) {
		return nil, errors.New("trailing bytes in shielded tx")
	}
	proof, err := parseBiniusProof(proofBytes)
	if err != nil {
		return nil, err
	}
	tx.Proof = proof
	return tx, nil
}

// Independent Depth-24 SHAKE256 Merkle root computation (FIPS-202 crypto/sha3).
func shake256Node(left, right *[NodeBytes]byte) [NodeBytes]byte {
	h := sha3.NewSHAKE256()
	h.Write([]byte{TagNode})
	h.Write(left[:])
	h.Write(right[:])
	var out [NodeBytes]byte
	h.Read(out[:])
	return out
}

func computeMerkle24Root(leaves [][CoinBytes]byte) [NodeBytes]byte {
	var empty [MerkleDepth + 1][NodeBytes]byte
	for lvl := 0; lvl < MerkleDepth; lvl++ {
		empty[lvl+1] = shake256Node(&empty[lvl], &empty[lvl])
	}
	if len(leaves) == 0 {
		return empty[MerkleDepth]
	}
	cur := make([][NodeBytes]byte, len(leaves))
	copy(cur, leaves)
	for lvl := 0; lvl < MerkleDepth; lvl++ {
		nNext := (len(cur) + 1) / 2
		next := make([][NodeBytes]byte, nNext)
		for i := 0; i < nNext; i++ {
			left := cur[2*i]
			right := empty[lvl]
			if 2*i+1 < len(cur) {
				right = cur[2*i+1]
			}
			next[i] = shake256Node(&left, &right)
		}
		cur = next
	}
	return cur[0]
}

type LedgerJson struct {
	Coins             []string `json:"coins"`
	Anchors           []string `json:"anchors"`
	SpentSN           []string `json:"spent_sn"`
	LeafCountOverride *int     `json:"leaf_count_override,omitempty"`
}

type LedgerState struct {
	Coins             [][CoinBytes]byte
	CoinSet           map[[CoinBytes]byte]bool
	Anchors           map[[NodeBytes]byte]bool
	SpentSN           map[[SNBytes]byte]bool
	LeafCountOverride *int
}

func (ls *LedgerState) effectiveLeafCount() int {
	if ls.LeafCountOverride != nil {
		return *ls.LeafCountOverride
	}
	return len(ls.Coins)
}

func loadLedger(path string) (*LedgerState, error) {
	raw, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}
	var lj LedgerJson
	if err := json.Unmarshal(raw, &lj); err != nil {
		return nil, err
	}
	ls := &LedgerState{
		Coins:             make([][CoinBytes]byte, 0, len(lj.Coins)),
		CoinSet:           make(map[[CoinBytes]byte]bool, len(lj.Coins)),
		Anchors:           make(map[[NodeBytes]byte]bool, len(lj.Anchors)),
		SpentSN:           make(map[[SNBytes]byte]bool, len(lj.SpentSN)),
		LeafCountOverride: lj.LeafCountOverride,
	}
	for _, h := range lj.Coins {
		b, err := hex.DecodeString(h)
		if err != nil || len(b) != CoinBytes {
			return nil, errors.New("invalid coin hex")
		}
		var c [CoinBytes]byte
		copy(c[:], b)
		ls.Coins = append(ls.Coins, c)
		ls.CoinSet[c] = true
	}
	for _, h := range lj.Anchors {
		b, err := hex.DecodeString(h)
		if err != nil || len(b) != NodeBytes {
			return nil, errors.New("invalid anchor hex")
		}
		var a [NodeBytes]byte
		copy(a[:], b)
		ls.Anchors[a] = true
	}
	computedRoot := computeMerkle24Root(ls.Coins)
	if !ls.Anchors[computedRoot] {
		return nil, errors.New("ledger anchors missing independently recomputed Depth-24 Merkle root")
	}
	for _, h := range lj.SpentSN {
		b, err := hex.DecodeString(h)
		if err != nil || len(b) != SNBytes {
			return nil, errors.New("invalid spent_sn hex")
		}
		var sn [SNBytes]byte
		copy(sn[:], b)
		ls.SpentSN[sn] = true
	}
	return ls, nil
}

func verifyFullSettlement(ls *LedgerState, tx *ShieldedTx) error {
	// Rule R6: strict Depth-24 pool capacity guard (|Coins| + 2 <= 2^24)
	if ls.effectiveLeafCount()+2 > MaxPoolCoins {
		return errors.New("Rule R6 violation: Depth-24 Merkle pool capacity 2^24 exceeded")
	}
	// Rule R1: anchor membership
	if !ls.Anchors[tx.Anchor] {
		return errors.New("Rule R1 violation: unknown or invalid Merkle anchor")
	}
	// Rule R2: intra-tx nullifier distinctness
	if tx.SN0 == tx.SN1 {
		return errors.New("Rule R2 violation: duplicate nullifiers sn0 == sn1 within tx")
	}
	// Rule R3: global nullifier freshness
	if ls.SpentSN[tx.SN0] {
		return errors.New("Rule R3 violation: double spend: sn0 already spent")
	}
	if ls.SpentSN[tx.SN1] {
		return errors.New("Rule R3 violation: double spend: sn1 already spent")
	}
	// Rule R4: strict output coin uniqueness & non-zero sentinel exclusion (coin != 0^48)
	var zeroCoin [CoinBytes]byte
	if tx.CoinOut0 == zeroCoin || tx.CoinOut1 == zeroCoin {
		return errors.New("Rule R4 violation: output coin commitment equals empty sentinel 0^48")
	}
	if tx.CoinOut0 == tx.CoinOut1 {
		return errors.New("Rule R4 violation: duplicate output coin commitments")
	}
	if ls.CoinSet[tx.CoinOut0] || ls.CoinSet[tx.CoinOut1] {
		return errors.New("Rule R4 violation: output coin already exists in ledger")
	}
	// Rule R5: mutually exclusive mint/unshield, u64 pub_exit check, and ZKP verification
	if tx.VMint > 0 && tx.VUnshield > 0 {
		return errors.New("Rule R5 violation: v_mint and v_unshield cannot both be non-zero")
	}
	if tx.Fee > ^uint64(0)-tx.VUnshield {
		return errors.New("Rule R5 violation: fee + v_unshield overflows u64")
	}
	return verifyBiniusProof(tx.statement(), tx.Proof)
}

type IrConstraintJson struct {
	Lin  [][2]uint32 `json:"lin"`
	Quad [][2]uint32 `json:"quad"`
	RHS  uint8       `json:"rhs"`
}

type IrDumpJson struct {
	NumGroupC   int                `json:"num_group_c"`
	Constraints []IrConstraintJson `json:"constraints"`
	AlphaHex    string             `json:"alpha_hex"`
	R1Hex       []string           `json:"r1_hex"`
	WBHex       []string           `json:"wb_hex"`
	WCHex       []string           `json:"wc_hex"`
	PubRHSHex   string             `json:"pub_rhs_hex"`
}

func main() {
	if len(os.Args) < 2 {
		fmt.Fprintln(os.Stderr, "Usage: binius-shielded-verifier <verify|bench|dump-ir> ...")
		os.Exit(1)
	}
	cmd := os.Args[1]
	fs := flag.NewFlagSet(cmd, flag.ExitOnError)
	txPath := fs.String("tx", "", "tx binary path")
	ledgerPath := fs.String("ledger", "", "ledger json path")
	trials := fs.Int("trials", 50, "benchmark trials")
	outPath := fs.String("out", "", "output json path")
	fs.Parse(os.Args[2:])

	switch cmd {
	case "verify":
		raw, err := os.ReadFile(*txPath)
		if err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		ls, err := loadLedger(*ledgerPath)
		if err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		tx, err := parseShieldedTx(raw)
		if err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		t0 := time.Now()
		if err := verifyFullSettlement(ls, tx); err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		ms := float64(time.Since(t0).Nanoseconds()) / 1e6
		fmt.Printf("GO VERIFY PASS (pclmul=%v): %s (%d B <= %d B budget) in %.2f ms\n",
			usePCLMUL, *txPath, len(raw), BudgetBytes, ms)

	case "bench":
		raw, err := os.ReadFile(*txPath)
		if err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		ls, err := loadLedger(*ledgerPath)
		if err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		// Warmup
		getKTables()
		tx0, err := parseShieldedTx(raw)
		if err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		if err := verifyFullSettlement(ls, tx0); err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		times := make([]float64, *trials)
		for i := 0; i < *trials; i++ {
			t0 := time.Now()
			tx, err := parseShieldedTx(raw)
			if err != nil {
				fmt.Fprintln(os.Stderr, "ERROR:", err)
				os.Exit(1)
			}
			if err := verifyFullSettlement(ls, tx); err != nil {
				fmt.Fprintln(os.Stderr, "ERROR:", err)
				os.Exit(1)
			}
			times[i] = float64(time.Since(t0).Nanoseconds()) / 1e6
		}
		sort.Float64s(times)
		med := times[len(times)/2]
		p95 := times[int(float64(len(times)-1)*0.95+0.5)]
		mx := times[len(times)-1]
		fmt.Printf("GO BENCH (pclmul=%v, trials=%d): med=%.2f ms p95=%.2f ms max=%.2f ms (wire=%d B)\n",
			usePCLMUL, *trials, med, p95, mx, len(raw))

	case "dump-ir":
		raw, err := os.ReadFile(*txPath)
		if err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		tx, err := parseShieldedTx(raw)
		if err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		stmt := tx.statement()
		cs := buildAllGroupCConstraints(stmt)
		outCS := make([]IrConstraintJson, len(cs))
		for i, c := range cs {
			lin := make([][2]uint32, len(c.Lin))
			for j, lt := range c.Lin {
				lin[j] = [2]uint32{uint32(lt.X), lt.Row}
			}
			quad := make([][2]uint32, len(c.Quad))
			for j, qt := range c.Quad {
				quad[j] = [2]uint32{uint32(qt.X), qt.Row}
			}
			outCS[i] = IrConstraintJson{Lin: lin, Quad: quad, RHS: c.RHS}
		}
		t := getKTables()
		var alpha E
		for i := 0; i < 12; i++ {
			alpha.C[i] = uint32(0x13572468) * uint32(i+1)
		}
		var r1 [VSC1]E
		for k := 0; k < VSC1; k++ {
			for i := 0; i < 12; i++ {
				r1[k].C[i] = uint32(k*17+i*11+5) * uint32(0x9E3779B9)
			}
		}
		wb, wc, pubRHS := evalVerifierWeightsAtR1(stmt, alpha, &r1, t)
		ab := alpha.ToBytes()
		r1Hex := make([]string, VSC1)
		for k := 0; k < VSC1; k++ {
			rb := r1[k].ToBytes()
			r1Hex[k] = hex.EncodeToString(rb[:])
		}
		wbHex := make([]string, 5)
		wcHex := make([]string, 5)
		for x := 0; x < 5; x++ {
			bb := wb[x].ToBytes()
			cb := wc[x].ToBytes()
			wbHex[x] = hex.EncodeToString(bb[:])
			wcHex[x] = hex.EncodeToString(cb[:])
		}
		prb := pubRHS.ToBytes()
		dump := IrDumpJson{
			NumGroupC:   len(outCS),
			Constraints: outCS,
			AlphaHex:    hex.EncodeToString(ab[:]),
			R1Hex:       r1Hex,
			WBHex:       wbHex,
			WCHex:       wcHex,
			PubRHSHex:   hex.EncodeToString(prb[:]),
		}
		enc, _ := json.Marshal(dump)
		if err := os.WriteFile(*outPath, enc, 0644); err != nil {
			fmt.Fprintln(os.Stderr, "ERROR:", err)
			os.Exit(1)
		}
		fmt.Printf("Go dumped %d Group-C IR constraints + exact Group A+B+C weights to %s\n", len(outCS), *outPath)

	default:
		fmt.Fprintln(os.Stderr, "unknown command:", cmd)
		os.Exit(1)
	}
}
