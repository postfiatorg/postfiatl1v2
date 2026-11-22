package main

import (
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"os"
	"testing"
)

// Rebuild the synthetic prefix at actual depth. The Go oracle never consumes
// the Rust frontier to compute either root.
func TestL1ProfileRollover(t *testing.T) {
	genesis, _ := hex.DecodeString("e9c735eecfc4550f152330b52f782de84eea2aa7d59dc7e9e317718ed2ba28473854f0cb7c6c972f7f2825ba944b72e8")
	domain := l1Chain{"postfiat-pfsh02-m1-disposable", [48]byte(genesis), 1}
	load := func(name string) []byte {
		data, err := os.ReadFile("../rollover-vectors/" + name)
		if err != nil {
			t.Fatal(err)
		}
		return data
	}
	check := func(name string) *ShieldedTx {
		data := load(name + ".bin")
		if err := verifyL1(data, domain); err != nil {
			t.Fatal(err)
		}
		start := 72
		if data[71] == 1 {
			start += 20 + 8 + 1952 + 3309
		}
		tx, err := parseProfileInner(data[start:], false)
		if err != nil {
			t.Fatal(err)
		}
		return tx
	}
	leaves := make([][CoinBytes]byte, MaxPoolCoins)
	var terminal [2][NodeBytes]byte
	var minted [2][CoinBytes]byte
	for epoch := 0; epoch < 2; epoch++ {
		tx := check(fmt.Sprintf("boundary-%d", epoch))
		var roots struct {
			Anchors  []string
			Terminal string
		}
		if err := json.Unmarshal(load(fmt.Sprintf("frontier-%d.json", epoch)), &roots); err != nil {
			t.Fatal(err)
		}
		for i := 0; i < MaxPoolCoins-2; i++ {
			binary.LittleEndian.PutUint64(leaves[i][:8], uint64(i+1+epoch*MaxPoolCoins))
		}
		pre := computeMerkle24Root(leaves[:MaxPoolCoins-2])
		if len(roots.Anchors) != 1 || hex.EncodeToString(pre[:]) != roots.Anchors[0] || pre != tx.Anchor {
			t.Fatal("pre-root mismatch")
		}
		leaves[MaxPoolCoins-2], leaves[MaxPoolCoins-1] = tx.CoinOut0, tx.CoinOut1
		post := computeMerkle24Root(leaves)
		if hex.EncodeToString(post[:]) != roots.Terminal {
			t.Fatal("terminal root mismatch")
		}
		terminal[epoch], minted[epoch] = post, tx.CoinOut0
		t.Logf("epoch %d terminal %x", epoch, post)
	}
	old := check("old-spend")
	if old.Anchor != terminal[0] {
		t.Fatal("missing frozen anchor")
	}
	moved := computeMerkle24Root([][CoinBytes]byte{old.CoinOut0, old.CoinOut1})
	if check("successor-spend").Anchor != moved {
		t.Fatal("wrong writable tree")
	}
	if check("mixed-inputs").Anchor != computeMerkle24Root(minted[:]) {
		t.Fatal("wrong mixed-tree control")
	}
}
