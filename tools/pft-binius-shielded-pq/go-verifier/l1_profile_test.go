package main

import (
	"encoding/hex"
	"os"
	"testing"
)

func TestL1Profile(t *testing.T) {
	genesis, _ := hex.DecodeString("e9c735eecfc4550f152330b52f782de84eea2aa7d59dc7e9e317718ed2ba28473854f0cb7c6c972f7f2825ba944b72e8")
	domain := l1Chain{"postfiat-pfsh02-m1-disposable", [48]byte(genesis), 1}
	for _, currency := range []string{"pft", "pfusdc"} {
		for _, mode := range []string{"shield", "pay", "topup", "exit"} {
			data, err := os.ReadFile("../l1-vectors/" + currency + "-" + mode + ".bin")
			if err != nil {
				t.Fatal(err)
			}
			if err = verifyL1(data, domain); err != nil {
				t.Fatalf("%s/%s: %v", currency, mode, err)
			}
			for _, i := range []int{4, 5, 6, 7, 55, 63, 71, len(data) - 1} {
				data[i] ^= 1
				if verifyL1(data, domain) == nil {
					t.Fatalf("accepted changed byte %d", i)
				}
				data[i] ^= 1
			}
			for i := 0; i < 3; i++ {
				wrong := domain
				switch i {
				case 0:
					wrong.id += "X"
				case 1:
					wrong.genesis[0] ^= 1
				case 2:
					wrong.protocol++
				}
				if verifyL1(data, wrong) == nil {
					t.Fatal("accepted wrong chain domain")
				}
			}
		}
	}
	l1, _ := os.ReadFile("../l1-vectors/pft-pay.bin")
	fixture, err := os.ReadFile("../payments/tx_hop1.bin")
	if err != nil {
		t.Fatal(err)
	}
	if verifyL1(append(l1[:72:72], fixture...), domain) == nil {
		t.Fatal("accepted standalone proof")
	}
}
