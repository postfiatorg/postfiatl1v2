package main

import (
	"encoding/binary"
	"fmt"
	"os"
	"time"
)

// Independent reconstruction from the public synthetic leaf recipe and Rust's two wire transactions.
func verifyFullTree(spendPath, mintPath string) error {
	mintBytes, err := os.ReadFile(mintPath)
	if err != nil {
		return err
	}
	mint, err := parseShieldedTx(mintBytes)
	if err != nil {
		return err
	}
	spendBytes, err := os.ReadFile(spendPath)
	if err != nil {
		return err
	}
	start := time.Now()
	ls := &LedgerState{
		Coins:   make([][CoinBytes]byte, MaxPoolCoins-2, MaxPoolCoins),
		CoinSet: make(map[[CoinBytes]byte]bool, MaxPoolCoins),
		Anchors: make(map[[NodeBytes]byte]bool),
		SpentSN: make(map[[SNBytes]byte]bool),
	}
	for i := range ls.Coins {
		binary.LittleEndian.PutUint64(ls.Coins[i][:8], uint64(i+1))
		ls.CoinSet[ls.Coins[i]] = true
	}
	ls.Anchors[computeMerkle24Root(ls.Coins)] = true
	if err := verifyFullSettlement(ls, mint); err != nil {
		return err
	}
	ls.Coins = append(ls.Coins, mint.CoinOut0, mint.CoinOut1)
	ls.SpentSN[mint.SN0], ls.SpentSN[mint.SN1] = true, true
	ls.CoinSet[mint.CoinOut0], ls.CoinSet[mint.CoinOut1] = true, true
	frozenRoot := computeMerkle24Root(ls.Coins)
	ls.Anchors[frozenRoot], ls.Anchors[computeMerkle24Root(nil)] = true, true
	build := time.Since(start)
	start = time.Now()
	spend, err := parseShieldedTx(spendBytes)
	if err != nil {
		return err
	}
	if spend.Anchor != frozenRoot || ls.effectiveLeafCount() != 0 {
		return fmt.Errorf("unexpected rollover anchor or occupancy")
	}
	if err := verifyFullSettlement(ls, spend); err != nil {
		return err
	}
	fmt.Printf("GO FULL TREE PASS: leaves=%d epoch=1 wire=%d B build_and_fill=%.2f ms parse_verify=%.2f ms\n",
		len(ls.Coins), len(spendBytes), float64(build.Microseconds())/1000, float64(time.Since(start).Microseconds())/1000)
	return nil
}
