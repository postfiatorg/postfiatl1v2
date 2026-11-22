package main

import (
	"crypto/sha3"
	"crypto/sha512"
	"encoding/hex"

	"github.com/cloudflare/circl/sign/mldsa/mldsa65"
)

type MintAuth struct {
	PublicKey [mldsa65.PublicKeySize]byte
	Signature [mldsa65.SignatureSize]byte
}

func (a *MintAuth) source() string {
	h := sha3.New384()
	h.Write([]byte("postfiat.address.v1\x00"))
	h.Write(a.PublicKey[:])
	return "pf" + hex.EncodeToString(h.Sum(nil)[:20])
}

func (a *MintAuth) verify(ctx [48]byte) bool {
	h := sha512.New384()
	h.Write([]byte("PFT-SH-MINT-AUTH-V2"))
	h.Write([]byte(a.source()))
	h.Write(ctx[:])
	var pk mldsa65.PublicKey
	pk.Unpack(&a.PublicKey)
	return mldsa65.Verify(&pk, h.Sum(nil), []byte("postfiat-l1-v2/tx/v1"), a.Signature[:])
}
