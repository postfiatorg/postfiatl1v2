// Independent Go 1.25.1 stdlib FIPS-203 ML-KEM-1024 + HMAC-SHA-384 + AES-256-GCM + SHAKE256
// 0-round offline stealth receiver scanner (ZERO third-party dependencies!).
package main

import (
	"crypto/aes"
	"crypto/cipher"
	"crypto/hmac"
	"crypto/mlkem"
	"crypto/sha3"
	"crypto/sha512"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"flag"
	"fmt"
	"os"

	"github.com/cloudflare/circl/sign/mldsa/mldsa65"
)

type WalletFixture struct {
	SpendSecretHex  string `json:"spend_secret_hex"`
	KemSeedHex      string `json:"kem_seed_hex"`
	AddrHex         string `json:"addr_hex"`
	EkHex           string `json:"ek_hex"`
	ExpectedValue   uint64 `json:"expected_value"`
	ExpectedRHex    string `json:"expected_r_hex"`
	ExpectedCoinHex string `json:"expected_coin_hex"`
}

func shake48(parts ...[]byte) [48]byte {
	h := sha3.NewSHAKE256()
	for _, p := range parts {
		h.Write(p)
	}
	var out [48]byte
	h.Read(out[:])
	return out
}

func deriveAEADKeyNonce(ss, ctKem, coin []byte) ([]byte, []byte) {
	derive := func(label string) []byte {
		mac := hmac.New(sha512.New384, ss)
		mac.Write([]byte(label))
		mac.Write(ctKem)
		mac.Write(coin)
		return mac.Sum(nil)
	}
	return derive("PFT-SH-KEM-KEY-V2")[:32], derive("PFT-SH-KEM-NONCE-V2")[:12]
}

func tryOpen(dk *mlkem.DecapsulationKey1024, addr [48]byte, coin, env []byte) (uint64, []byte, bool) {
	ctKem := env[:1568]
	ctAEAD := env[1568:]
	ss, err := dk.Decapsulate(ctKem)
	if err != nil {
		return 0, nil, false
	}
	key, nonce := deriveAEADKeyNonce(ss, ctKem, coin)
	blk, err := aes.NewCipher(key)
	if err != nil {
		return 0, nil, false
	}
	gcm, err := cipher.NewGCM(blk)
	if err != nil {
		return 0, nil, false
	}
	pt, err := gcm.Open(nil, nonce, ctAEAD, coin)
	if err != nil || len(pt) != 86 {
		return 0, nil, false
	}
	val := binary.LittleEndian.Uint64(pt[:8])
	rBlind := pt[8:86]
	gotCoin := shake48([]byte{0x01}, addr[:], pt[:8], rBlind)
	if hex.EncodeToString(gotCoin[:]) != hex.EncodeToString(coin) {
		return 0, nil, false
	}
	return val, rBlind, true
}

func main() {
	txPath := flag.String("tx", "", "tx binary path")
	walletPath := flag.String("wallet", "", "wallet fixture json path")
	flag.Parse()

	txBytes, err := os.ReadFile(*txPath)
	if err != nil {
		fmt.Fprintln(os.Stderr, "ERROR:", err)
		os.Exit(1)
	}
	wRaw, err := os.ReadFile(*walletPath)
	if err != nil {
		fmt.Fprintln(os.Stderr, "ERROR:", err)
		os.Exit(1)
	}
	var wf WalletFixture
	if err := json.Unmarshal(wRaw, &wf); err != nil {
		fmt.Fprintln(os.Stderr, "ERROR:", err)
		os.Exit(1)
	}

	spendSec, _ := hex.DecodeString(wf.SpendSecretHex)
	kemSeed, _ := hex.DecodeString(wf.KemSeedHex)
	dsaSeedFull := shake48([]byte("PFT-SH-SPEND-DSA-SEED-V3"), spendSec)
	var dsaSeed [32]byte
	copy(dsaSeed[:], dsaSeedFull[:32])
	pkDsa, _ := mldsa65.NewKeyFromSeed(&dsaSeed)
	pkDsaBytes, _ := pkDsa.MarshalBinary()
	pkHash := shake48([]byte{0x04}, pkDsaBytes)
	rPk := shake48([]byte("PFT-SH-SPEND-RPK-V3"), spendSec)
	addr := shake48([]byte{0x00}, pkHash[:], rPk[:])
	if hex.EncodeToString(addr[:]) != wf.AddrHex {
		fmt.Fprintln(os.Stderr, "ERROR: Go SHAKE256+ML-DSA-65 addr mismatch with Rust")
		os.Exit(1)
	}

	dk, err := mlkem.NewDecapsulationKey1024(kemSeed)
	if err != nil {
		fmt.Fprintln(os.Stderr, "ERROR: mlkem keygen:", err)
		os.Exit(1)
	}
	if hex.EncodeToString(dk.EncapsulationKey().Bytes()) != wf.EkHex {
		fmt.Fprintln(os.Stderr, "ERROR: Go stdlib FIPS-203 ML-KEM-1024 EK mismatch with Rust fips203")
		os.Exit(1)
	}

	// Parse coin_out0, coin_out1, env0, env1 directly from PFSH02 wire offsets:
	// magic(8) + anchor(48) + v_mint(8) + v_unshield(8) + unshield_dest(32) + fee(8) + sn0(48) + sn1(48) = 208
	coin0 := txBytes[208:256]
	coin1 := txBytes[256:304]
	env0 := txBytes[304 : 304+1670]
	env1 := txBytes[304+1670 : 304+3340]

	val0, r0, ok0 := tryOpen(dk, addr, coin0, env0)
	_, _, ok1 := tryOpen(dk, addr, coin1, env1)
	if !ok0 || ok1 {
		fmt.Fprintf(os.Stderr, "ERROR: expected output 0 match only, got ok0=%v ok1=%v\n", ok0, ok1)
		os.Exit(1)
	}
	if val0 != wf.ExpectedValue || hex.EncodeToString(r0) != wf.ExpectedRHex || hex.EncodeToString(coin0) != wf.ExpectedCoinHex {
		fmt.Fprintln(os.Stderr, "ERROR: recovered stealth note mismatch!")
		os.Exit(1)
	}
	fmt.Printf("GO STDLIB ML-KEM-1024 STEALTH SCAN PASS: recovered v=%d, coin=%s.. (output 1 cleanly rejected)\n",
		val0, wf.ExpectedCoinHex[:16])
}
