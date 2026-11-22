package main

import (
	"bytes"
	"crypto/sha512"
	"encoding/binary"
	"encoding/hex"
	"errors"
	"math"

	"github.com/cloudflare/circl/sign/mldsa/mldsa65"
)

type l1Chain struct {
	id       string
	genesis  [48]byte
	protocol uint32
}

// The outer fields through the public key (but not the signature) are
// the unsigned fee/debit terms. Raw txCtx commits to the complete inner header.
func verifyL1(b []byte, chain l1Chain) error {
	bad := errors.New("invalid L1 payment")
	if len(b) < 72 || len(b) > BudgetBytes || string(b[:6]) != "PFPQ\x01\x01" || b[6] > 2 {
		return bad
	}
	issued := !bytes.Equal(b[7:55], make([]byte, 48))
	needAuth := issued || b[6] == 1
	if b[71] > 1 || (b[71] == 1) != needAuth || (binary.LittleEndian.Uint64(b[63:71]) != 0) != issued {
		return bad
	}
	end := 72
	inner := b[end:]
	var auth MintAuth
	if needAuth {
		end += 20 + 8 + mldsa65.PublicKeySize
		if len(b) < end+mldsa65.SignatureSize || binary.LittleEndian.Uint64(b[92:100]) == 0 {
			return bad
		}
		copy(auth.PublicKey[:], b[100:end])
		copy(auth.Signature[:], b[end:end+mldsa65.SignatureSize])
		if auth.source() != "pf"+hex.EncodeToString(b[72:92]) {
			return bad
		}
		inner = b[end+mldsa65.SignatureSize:]
	}
	if len(inner) > 140476 {
		return bad
	}
	tx, err := parseProfileInner(inner, false)
	if err != nil {
		return err
	}
	if (tx.VMint > 0) != (b[6] == 1) || (tx.VUnshield > 0) != (b[6] == 2) || tx.VUnshield > math.MaxUint64-tx.Fee || (issued && tx.Fee != 0) {
		return bad
	}
	if !bytes.Equal(tx.UnshieldDest[:12], make([]byte, 12)) || (tx.UnshieldDest != [32]byte{}) != (b[6] == 2) {
		return bad
	}
	stmt := tx.statement()
	raw := stmt.TxCtx
	h := sha512.New384()
	h.Write([]byte("PFT-SH-L1-CONTEXT-V1"))
	h.Write(binary.LittleEndian.AppendUint64(nil, uint64(len(chain.id))))
	h.Write([]byte(chain.id))
	h.Write(chain.genesis[:])
	h.Write(binary.LittleEndian.AppendUint32(nil, chain.protocol))
	h.Write([]byte("PFT-PQ-SHIELDED-POOL-V1"))
	h.Write(b[:end])
	h.Write(raw[:])
	h.Sum(stmt.TxCtx[:0])
	if needAuth {
		domain := "PFT-SH-FEE-AUTH-V1"
		if b[6] == 1 {
			domain = "PFT-SH-MINT-AUTH-V2"
		}
		msg := append([]byte(domain), b[72:92]...)
		msg = append(msg, raw[:]...)
		msg = binary.LittleEndian.AppendUint32(msg, 48)
		msg = append(msg, stmt.TxCtx[:]...)
		var pk mldsa65.PublicKey
		pk.Unpack(&auth.PublicKey)
		if !mldsa65.Verify(&pk, msg, []byte("postfiat-l1-v2/tx/v1"), auth.Signature[:]) {
			return bad
		}
	}
	return verifyBiniusProof(stmt, tx.Proof)
}
