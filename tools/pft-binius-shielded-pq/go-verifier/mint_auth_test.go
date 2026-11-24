package main

import (
	"os"
	"testing"

	"github.com/cloudflare/circl/sign/mldsa/mldsa65"
)

func TestMintAuthorization(t *testing.T) {
	raw, err := os.ReadFile("../payments/tx_mint.bin")
	if err != nil {
		t.Fatal(err)
	}
	ls, err := loadLedger("../payments/ledger_mint.json")
	if err != nil {
		t.Fatal(err)
	}
	for name, mutate := range map[string]func(*ShieldedTx){
		"wrong amount":              func(tx *ShieldedTx) { tx.VMint++ },
		"swapped outputs":           func(tx *ShieldedTx) { tx.CoinOut0, tx.CoinOut1 = tx.CoinOut1, tx.CoinOut0 },
		"replay on changed context": func(tx *ShieldedTx) { tx.Env1[0] ^= 1 },
		"missing signature":         func(tx *ShieldedTx) { tx.MintAuth = nil },
		"wrong source key":          func(tx *ShieldedTx) { tx.MintAuth.PublicKey[0] ^= 1 },
	} {
		t.Run(name, func(t *testing.T) {
			tx, err := parseShieldedTx(raw)
			if err != nil {
				t.Fatal(err)
			}
			mutate(tx)
			if err := verifyFullSettlement(ls, tx); err == nil || err.Error() != "R0-Auth: invalid mint authorization" {
				t.Fatalf("expected R0-Auth rejection, got %v", err)
			}
		})
	}
	if _, err := parseShieldedTx(raw[:len(raw)-mldsa65.SignatureSize]); err == nil {
		t.Fatal("accepted missing wire signature")
	}
}
