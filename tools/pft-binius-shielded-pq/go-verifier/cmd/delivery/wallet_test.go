package main

import (
	"crypto/hkdf"
	"crypto/mlkem"
	"crypto/sha512"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"os"
	"testing"
)

func TestWalletRecoveryVector(t *testing.T) {
	const dir = "../../../../../crates/privacy_binius_wallet/vectors/"
	raw, err := os.ReadFile(dir + "derivation.json")
	if err != nil {
		t.Fatal(err)
	}
	var v struct {
		Chain            string `json:"chain_id"`
		Genesis          string `json:"genesis_hash"`
		Account, Index   uint32
		Seed, Spend, Kem string
	}
	if err = json.Unmarshal(raw, &v); err != nil {
		t.Fatal(err)
	}
	unhex := func(s string) []byte {
		b, e := hex.DecodeString(s)
		if e != nil {
			t.Fatal(e)
		}
		return b
	}
	derive := func(role string, size int) []byte {
		var info []byte
		for _, f := range [][]byte{[]byte("postfiat/pq-wallet/v1/" + role), []byte(v.Chain), unhex(v.Genesis), binary.BigEndian.AppendUint32(nil, v.Account), binary.BigEndian.AppendUint32(nil, v.Index)} {
			info = binary.BigEndian.AppendUint32(info, uint32(len(f)))
			info = append(info, f...)
		}
		k, e := hkdf.Key(sha512.New384, unhex(v.Seed), []byte("postfiat/pq-wallet/v1/hkdf-salt"), string(info), size)
		if e != nil {
			t.Fatal(e)
		}
		return k
	}
	spend, kem := derive("spend", 48), derive("kem", 64)
	if hex.EncodeToString(spend) != v.Spend || hex.EncodeToString(kem) != v.Kem {
		t.Fatal("key mismatch")
	}
	buf, e := os.ReadFile(dir + "delivery.bin")
	if e != nil || len(buf) != 3388 {
		t.Fatal("invalid vector")
	}
	addr := shake48([]byte{0}, spend)
	dk, e := mlkem.NewDecapsulationKey1024(kem)
	if e != nil {
		t.Fatal(e)
	}
	coin, valid, bad := buf[:48], buf[48:1718], buf[1718:]
	if val, _, ok := tryOpen(dk, addr, coin, valid); !ok || val != 123 {
		t.Fatal("opening")
	}
	if _, _, ok := tryOpen(dk, addr, coin, bad); ok {
		t.Fatal("authenticated wrong opening")
	}
	wrong, e := mlkem.NewDecapsulationKey1024(make([]byte, 64))
	if e != nil {
		t.Fatal(e)
	}
	if _, _, ok := tryOpen(wrong, addr, coin, valid); ok {
		t.Fatal("wrong key")
	}
	valid[len(valid)-1] ^= 1
	if _, _, ok := tryOpen(dk, addr, coin, valid); ok {
		t.Fatal("corrupt")
	}
}
