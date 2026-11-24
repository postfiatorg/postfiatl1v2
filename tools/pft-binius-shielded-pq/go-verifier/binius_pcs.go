// Independent Go implementation of the Peel-Binius-Shielded verifier
// with single fixed-range SHA-384 compiler oracle H: {0,1}^* -> {0,1}^{1152}
// and exact 3D-Karatsuba Ring-Switch inner product.
package main

import (
	"crypto/sha512"
	"encoding/binary"
	"errors"
	"fmt"
	"math/bits"
	"sort"
)

const (
	NCode         = 2097152
	MerkleHeight  = 21
	KCols         = 512
	NumQueries    = 52
	PowBits       = 20
	PowNonceBytes = 48
	DegBound      = KCols + NumQueries + 1 // 565

	HashBytes    = 48 // 384-bit MT_1 digest prefix of H = 3 x SHA-512
	StateBytes   = 64 // 512-bit internal Fiat-Shamir transcript state (ZERO wire bytes!)
	VSC2         = 8
	VirtualRows  = 256
	MWitRows     = 140
	MRSMaskRows  = 5
	MG1Rows      = 2
	MSC2MaskRows = 16
	MNonMaskRows = MWitRows + MRSMaskRows + MG1Rows // 147
	MKRows       = MNonMaskRows + MSC2MaskRows      // 163

	Zeta1K uint32 = 2
	Zeta2K uint32 = 3
)

var (
	ProofMagic = [8]byte{'P', 'F', 'S', 'H', 'P', 'K', '0', '1'}
	Mask2Pairs = [VSC2][2]int{
		{0, 128}, {1, 65}, {2, 34}, {3, 19}, {4, 12}, {16, 20}, {8, 10}, {6, 7},
	}
	Mask2Rows = [MSC2MaskRows]int{0, 1, 2, 3, 4, 6, 7, 8, 10, 12, 16, 19, 20, 34, 65, 128}
)

type RowLayout struct {
	nonMaskRows [MNonMaskRows]int
	activeRows  [MKRows]int
}

func buildRowLayout() RowLayout {
	var isMask [VirtualRows]bool
	for _, p := range Mask2Pairs {
		isMask[p[0]] = true
		isMask[p[1]] = true
	}
	var rl RowLayout
	nm := 0
	for r := 0; r < VirtualRows && nm < MNonMaskRows; r++ {
		if !isMask[r] {
			rl.nonMaskRows[nm] = r
			nm++
		}
	}
	act := make([]int, 0, MKRows)
	act = append(act, Mask2Rows[:]...)
	act = append(act, rl.nonMaskRows[:]...)
	sort.Ints(act)
	copy(rl.activeRows[:], act)
	return rl
}

func encodeStatement(stmt *PaymentStatement) []byte {
	out := make([]byte, 0, NodeBytes+2*SNBytes+2*CoinBytes+24+48+2*PkHashBytes)
	out = append(out, stmt.Anchor[:]...)
	out = append(out, stmt.SN0[:]...)
	out = append(out, stmt.SN1[:]...)
	out = append(out, stmt.CoinOut0[:]...)
	out = append(out, stmt.CoinOut1[:]...)
	var b8 [8]byte
	binary.LittleEndian.PutUint64(b8[:], stmt.VMint)
	out = append(out, b8[:]...)
	binary.LittleEndian.PutUint64(b8[:], stmt.VUnshield)
	out = append(out, b8[:]...)
	binary.LittleEndian.PutUint64(b8[:], stmt.Fee)
	out = append(out, b8[:]...)
	out = append(out, stmt.TxCtx[:]...)
	out = append(out, stmt.PkHash0[:]...)
	out = append(out, stmt.PkHash1[:]...)
	return out
}

func oracleLeaf(colIdx int, kVals *[MKRows]uint32, bPCS, bProx E) [HashBytes]byte {
	h := sha512.New()
	h.Write([]byte{0x00})
	h.Write([]byte("PFS-LEAF"))
	var cb [4]byte
	binary.LittleEndian.PutUint32(cb[:], uint32(colIdx))
	h.Write(cb[:])
	var kb [MKRows * 4]byte
	for i := 0; i < MKRows; i++ {
		binary.LittleEndian.PutUint32(kb[4*i:4*i+4], kVals[i])
	}
	h.Write(kb[:])
	bp := bPCS.ToBytes()
	bx := bProx.ToBytes()
	h.Write(bp[:])
	h.Write(bx[:])
	var full [StateBytes]byte
	h.Sum(full[:0])
	var out [HashBytes]byte
	copy(out[:], full[:HashBytes])
	return out
}

func oracleNode(level uint8, pos int, left, right *[HashBytes]byte) [HashBytes]byte {
	h := sha512.New()
	h.Write([]byte{0x00})
	h.Write([]byte("PFS-NODE"))
	var meta [5]byte
	meta[0] = level
	binary.LittleEndian.PutUint32(meta[1:5], uint32(pos))
	h.Write(meta[:])
	h.Write(left[:])
	h.Write(right[:])
	var full [StateBytes]byte
	h.Sum(full[:0])
	var out [HashBytes]byte
	copy(out[:], full[:HashBytes])
	return out
}

type Transcript struct {
	state [StateBytes]byte
}

func newTranscript(stmtBytes []byte, root *[HashBytes]byte, sG1, sMask2 E) Transcript {
	h := sha512.New()
	h.Write([]byte{0x00})
	h.Write([]byte("PFS-STMT"))
	h.Write(stmtBytes)
	h.Write(root[:])
	b1 := sG1.ToBytes()
	b2 := sMask2.ToBytes()
	h.Write(b1[:])
	h.Write(b2[:])
	var tr Transcript
	h.Sum(tr.state[:0])
	return tr
}

func (tr *Transcript) absorbE(tag string, elems []E) {
	h := sha512.New()
	h.Write([]byte{0x00})
	h.Write([]byte("PFS-ABS "))
	h.Write([]byte(tag))
	h.Write(tr.state[:])
	for _, e := range elems {
		eb := e.ToBytes()
		h.Write(eb[:])
	}
	h.Sum(tr.state[:0])
}

func (tr *Transcript) squeezeE(tag string) E {
	h0 := sha512.New()
	h0.Write([]byte{0x00})
	h0.Write([]byte("PFS-CH  "))
	h0.Write([]byte(tag))
	h0.Write(tr.state[:])
	var d0 [StateBytes]byte
	h0.Sum(d0[:0])

	h1 := sha512.New()
	h1.Write([]byte{0x01})
	h1.Write([]byte("PFS-CH  "))
	h1.Write([]byte(tag))
	h1.Write(tr.state[:])
	h1.Sum(tr.state[:0])

	return EFromBytes(d0[:EBytes])
}

func evalHC4(trFinal *[StateBytes]byte, nonce *[PowNonceBytes]byte) (bool, [NumQueries]int) {
	var indices [NumQueries]int
	h0 := sha512.New()
	h0.Write([]byte{0x00})
	h0.Write([]byte("PFS-C4  "))
	h0.Write(trFinal[:])
	h0.Write(nonce[:])
	var d0 [StateBytes]byte
	h0.Sum(d0[:0])

	if d0[0] != 0 || d0[1] != 0 || (d0[2]&0x0F) != 0 {
		return false, indices
	}

	h1 := sha512.New()
	h1.Write([]byte{0x01})
	h1.Write([]byte("PFS-C4  "))
	h1.Write(trFinal[:])
	h1.Write(nonce[:])
	var d1 [StateBytes]byte
	h1.Sum(d1[:0])

	h2 := sha512.New()
	h2.Write([]byte{0x02})
	h2.Write([]byte("PFS-C4  "))
	h2.Write(trFinal[:])
	h2.Write(nonce[:])
	var d2 [StateBytes]byte
	h2.Sum(d2[:0])

	var buf [192]byte
	copy(buf[0:64], d0[:])
	copy(buf[64:128], d1[:])
	copy(buf[128:192], d2[:])

	bitBytes := buf[3:]
	for q := 0; q < NumQueries; q++ {
		bitPos := q * MerkleHeight
		bytePos := bitPos / 8
		shift := uint(bitPos % 8)
		var word uint32
		word |= uint32(bitBytes[bytePos])
		if bytePos+1 < len(bitBytes) {
			word |= uint32(bitBytes[bytePos+1]) << 8
		}
		if bytePos+2 < len(bitBytes) {
			word |= uint32(bitBytes[bytePos+2]) << 16
		}
		if bytePos+3 < len(bitBytes) {
			word |= uint32(bitBytes[bytePos+3]) << 24
		}
		indices[q] = int((word >> shift) & ((1 << MerkleHeight) - 1))
	}
	return true, indices
}

type OpenedColumn struct {
	KVals [MKRows]uint32
	BPCS  E
	BProx E
}

type BiniusProof struct {
	MerkleRoot     [HashBytes]byte
	SG1            E
	SC1Rounds      [VSC1][3]E
	EvalC1         E
	EvalC2         E
	EvalZ          [5]E
	EvalZG         E
	RSSPrime       [32]E
	SMask2         E
	SC2Rounds      [VSC2][2]E
	BEval          E
	VProxCoeffs    []E
	VOpenCoeffs    []E
	PowNonce       [PowNonceBytes]byte
	OpenedCols     []OpenedColumn
	MerkleSiblings [][HashBytes]byte
}

func parseBiniusProof(data []byte) (*BiniusProof, error) {
	pos := 0
	take := func(n int) ([]byte, error) {
		if pos+n > len(data) {
			return nil, errors.New("truncated Binius proof")
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
		if magic[i] != ProofMagic[i] {
			return nil, errors.New("bad Binius proof magic")
		}
	}
	p := &BiniusProof{}
	b, err := take(HashBytes)
	if err != nil {
		return nil, err
	}
	copy(p.MerkleRoot[:], b)
	takeE := func() (E, error) {
		eb, err := take(EBytes)
		if err != nil {
			return EZero, err
		}
		return EFromBytes(eb), nil
	}
	if p.SG1, err = takeE(); err != nil {
		return nil, err
	}
	for d := 0; d < VSC1; d++ {
		for k := 0; k < 3; k++ {
			if p.SC1Rounds[d][k], err = takeE(); err != nil {
				return nil, err
			}
		}
	}
	if p.EvalC1, err = takeE(); err != nil {
		return nil, err
	}
	if p.EvalC2, err = takeE(); err != nil {
		return nil, err
	}
	for x := 0; x < 5; x++ {
		if p.EvalZ[x], err = takeE(); err != nil {
			return nil, err
		}
	}
	if p.EvalZG, err = takeE(); err != nil {
		return nil, err
	}
	for i := 0; i < 32; i++ {
		if p.RSSPrime[i], err = takeE(); err != nil {
			return nil, err
		}
	}
	if p.SMask2, err = takeE(); err != nil {
		return nil, err
	}
	for d := 0; d < VSC2; d++ {
		for k := 0; k < 2; k++ {
			if p.SC2Rounds[d][k], err = takeE(); err != nil {
				return nil, err
			}
		}
	}
	if p.BEval, err = takeE(); err != nil {
		return nil, err
	}
	p.VProxCoeffs = make([]E, DegBound)
	for i := 0; i < DegBound; i++ {
		if p.VProxCoeffs[i], err = takeE(); err != nil {
			return nil, err
		}
	}
	p.VOpenCoeffs = make([]E, DegBound)
	for i := 0; i < DegBound; i++ {
		if p.VOpenCoeffs[i], err = takeE(); err != nil {
			return nil, err
		}
	}
	nb, err := take(PowNonceBytes)
	if err != nil {
		return nil, err
	}
	copy(p.PowNonce[:], nb)
	ncb, err := take(2)
	if err != nil {
		return nil, err
	}
	nCols := int(binary.LittleEndian.Uint16(ncb))
	if nCols == 0 || nCols > NumQueries {
		return nil, errors.New("invalid opened column count")
	}
	p.OpenedCols = make([]OpenedColumn, nCols)
	for i := 0; i < nCols; i++ {
		kb, err := take(MKRows * 4)
		if err != nil {
			return nil, err
		}
		for r := 0; r < MKRows; r++ {
			p.OpenedCols[i].KVals[r] = binary.LittleEndian.Uint32(kb[4*r : 4*r+4])
		}
		if p.OpenedCols[i].BPCS, err = takeE(); err != nil {
			return nil, err
		}
		if p.OpenedCols[i].BProx, err = takeE(); err != nil {
			return nil, err
		}
	}
	nsb, err := take(2)
	if err != nil {
		return nil, err
	}
	nSibs := int(binary.LittleEndian.Uint16(nsb))
	if nSibs > NumQueries*MerkleHeight {
		return nil, errors.New("invalid merkle sibling count")
	}
	p.MerkleSiblings = make([][HashBytes]byte, nSibs)
	for i := 0; i < nSibs; i++ {
		sb, err := take(HashBytes)
		if err != nil {
			return nil, err
		}
		copy(p.MerkleSiblings[i][:], sb)
	}
	if pos != len(data) {
		return nil, errors.New("trailing bytes in Binius proof")
	}
	return p, nil
}

func expandGammas(gamma E, t *KTables) [5]E {
	g2 := gamma.Sq(t)
	g3 := g2.Mul(gamma, t)
	g4 := g2.Sq(t)
	return [5]E{EOne, gamma, g2, g3, g4}
}

func expandRhoRS(rhoRS E, t *KTables) ([]E, E, E, E) {
	wRS := make([]E, 384)
	cur := EOne
	for i := 0; i < 384; i++ {
		wRS[i] = cur
		cur = cur.Mul(rhoRS, t)
	}
	eta1 := cur
	eta2 := eta1.Mul(rhoRS, t)
	eta3 := eta2.Mul(rhoRS, t)
	return wRS, eta1, eta2, eta3
}

func ringSwitchTarget(sPrime *[32]E, wRS []E, t *KTables) E {
	var acc E
	for v := 0; v < 12; v++ {
		for u := 0; u < 32; u++ {
			var sHat uint32
			for b := 0; b < 32; b++ {
				if ((sPrime[b].C[v] >> u) & 1) != 0 {
					sHat |= uint32(1) << b
				}
			}
			if sHat != 0 {
				acc.AddMulK(sHat, wRS[32*v+u], t)
			}
		}
	}
	return acc
}

func evalG1MGWeights(r1 *[VSC1]E, tau, eta1, eta2 E, t *KTables) [78]E {
	var w [78]E
	var pref, suff [VSC1 + 1]E
	pref[0] = EOne
	for i := 0; i < VSC1; i++ {
		pref[i+1] = pref[i].Mul(r1[i], t)
	}
	suff[VSC1] = EOne
	for i := VSC1 - 1; i >= 0; i-- {
		suff[i] = suff[i+1].Mul(r1[i], t)
	}
	for d := 0; d < VSC1; d++ {
		base := eta1.Mul(pref[d].Mul(suff[d+1], t), t)
		rd := r1[d]
		rd2 := rd.Sq(t)
		rd3 := rd2.Mul(rd, t)
		w[4*d] = base
		w[4*d+1] = base.Mul(rd, t).Add(eta2)
		w[4*d+2] = base.Mul(rd2, t).Add(eta2)
		w[4*d+3] = base.Mul(rd3, t).Add(eta2)
	}
	w[76] = eta1.Mul(tau, t)
	w[77] = eta1.Mul(tau.Sq(t), t)
	return w
}

// 3D Karatsuba polynomial multiplication in E[Y, Z, x] (13,122 E::muls, bit-exact).
func karatsuba4E(a, b *[4]E, out *[7]E, t *KTables) {
	z00 := a[0].Mul(b[0], t)
	z02 := a[1].Mul(b[1], t)
	z01 := a[0].Add(a[1]).Mul(b[0].Add(b[1]), t).Add(z00).Add(z02)

	z20 := a[2].Mul(b[2], t)
	z22 := a[3].Mul(b[3], t)
	z21 := a[2].Add(a[3]).Mul(b[2].Add(b[3]), t).Add(z20).Add(z22)

	sa0, sa1 := a[0].Add(a[2]), a[1].Add(a[3])
	sb0, sb1 := b[0].Add(b[2]), b[1].Add(b[3])
	z10 := sa0.Mul(sb0, t)
	z12 := sa1.Mul(sb1, t)
	z11 := sa0.Add(sa1).Mul(sb0.Add(sb1), t).Add(z10).Add(z12)

	out[0] = z00
	out[1] = z01
	out[2] = z02.Add(z10).Add(z00).Add(z20)
	out[3] = z11.Add(z01).Add(z21)
	out[4] = z20.Add(z12).Add(z02).Add(z22)
	out[5] = z21
	out[6] = z22
}

func karatsuba8E(a, b *[8]E, out *[15]E, t *KTables) {
	a0 := [4]E{a[0], a[1], a[2], a[3]}
	a1 := [4]E{a[4], a[5], a[6], a[7]}
	as := [4]E{a[0].Add(a[4]), a[1].Add(a[5]), a[2].Add(a[6]), a[3].Add(a[7])}
	b0 := [4]E{b[0], b[1], b[2], b[3]}
	b1 := [4]E{b[4], b[5], b[6], b[7]}
	bs := [4]E{b[0].Add(b[4]), b[1].Add(b[5]), b[2].Add(b[6]), b[3].Add(b[7])}
	var z0, z1, z2 [7]E
	karatsuba4E(&a0, &b0, &z0, t)
	karatsuba4E(&a1, &b1, &z2, t)
	karatsuba4E(&as, &bs, &z1, t)
	for i := 0; i < 4; i++ {
		out[i] = z0[i]
	}
	for i := 0; i < 3; i++ {
		out[4+i] = z0[4+i].Add(z1[i]).Add(z0[i]).Add(z2[i])
	}
	out[7] = z1[3].Add(z0[3]).Add(z2[3])
	for i := 0; i < 3; i++ {
		out[8+i] = z2[i].Add(z1[4+i]).Add(z0[4+i]).Add(z2[4+i])
	}
	for i := 3; i < 7; i++ {
		out[8+i] = z2[i]
	}
}

func karatsuba16E(a, b *[16]E, out *[31]E, t *KTables) {
	var a0, a1, as, b0, b1, bs [8]E
	for i := 0; i < 8; i++ {
		a0[i], a1[i], as[i] = a[i], a[8+i], a[i].Add(a[8+i])
		b0[i], b1[i], bs[i] = b[i], b[8+i], b[i].Add(b[8+i])
	}
	var z0, z1, z2 [15]E
	karatsuba8E(&a0, &b0, &z0, t)
	karatsuba8E(&a1, &b1, &z2, t)
	karatsuba8E(&as, &bs, &z1, t)
	for i := 0; i < 8; i++ {
		out[i] = z0[i]
	}
	for i := 0; i < 7; i++ {
		out[8+i] = z0[8+i].Add(z1[i]).Add(z0[i]).Add(z2[i])
	}
	out[15] = z1[7].Add(z0[7]).Add(z2[7])
	for i := 0; i < 7; i++ {
		out[16+i] = z2[i].Add(z1[8+i]).Add(z0[8+i]).Add(z2[8+i])
	}
	for i := 7; i < 15; i++ {
		out[16+i] = z2[i]
	}
}

func mul2x16E(a, b *[2][16]E, out *[3][31]E, t *KTables) {
	var z0, z1, z2 [31]E
	var as, bs [16]E
	for i := 0; i < 16; i++ {
		as[i] = a[0][i].Add(a[1][i])
		bs[i] = b[0][i].Add(b[1][i])
	}
	karatsuba16E(&a[0], &b[0], &z0, t)
	karatsuba16E(&a[1], &b[1], &z2, t)
	karatsuba16E(&as, &bs, &z1, t)
	out[0] = z0
	for i := 0; i < 31; i++ {
		out[1][i] = z1[i].Add(z0[i]).Add(z2[i])
	}
	out[2] = z2
}

func add2x16(a, b *[2][16]E) [2][16]E {
	var out [2][16]E
	for z := 0; z < 2; z++ {
		for u := 0; u < 16; u++ {
			out[z][u] = a[z][u].Add(b[z][u])
		}
	}
	return out
}

func add3x31Assign(dst, src *[3][31]E) {
	for z := 0; z < 3; z++ {
		for u := 0; u < 31; u++ {
			dst[z][u].AddAssign(src[z][u])
		}
	}
}

func mul3x2x16E(a, b *[3][2][16]E, out *[5][3][31]E, t *KTables) {
	var p0, p1, p2, q01, q02, q12 [3][31]E
	mul2x16E(&a[0], &b[0], &p0, t)
	mul2x16E(&a[1], &b[1], &p1, t)
	mul2x16E(&a[2], &b[2], &p2, t)
	a01, b01 := add2x16(&a[0], &a[1]), add2x16(&b[0], &b[1])
	a02, b02 := add2x16(&a[0], &a[2]), add2x16(&b[0], &b[2])
	a12, b12 := add2x16(&a[1], &a[2]), add2x16(&b[1], &b[2])
	mul2x16E(&a01, &b01, &q01, t)
	mul2x16E(&a02, &b02, &q02, t)
	mul2x16E(&a12, &b12, &q12, t)
	out[0] = p0
	out[1] = q01
	add3x31Assign(&out[1], &p0)
	add3x31Assign(&out[1], &p1)
	out[2] = q02
	add3x31Assign(&out[2], &p0)
	add3x31Assign(&out[2], &p1)
	add3x31Assign(&out[2], &p2)
	out[3] = q12
	add3x31Assign(&out[3], &p1)
	add3x31Assign(&out[3], &p2)
	out[4] = p2
}

func mul6x2x16E(a, b *[6][2][16]E, out *[11][3][31]E, t *KTables) {
	aLo := [3][2][16]E{a[0], a[1], a[2]}
	aHi := [3][2][16]E{a[3], a[4], a[5]}
	aSum := [3][2][16]E{add2x16(&a[0], &a[3]), add2x16(&a[1], &a[4]), add2x16(&a[2], &a[5])}
	bLo := [3][2][16]E{b[0], b[1], b[2]}
	bHi := [3][2][16]E{b[3], b[4], b[5]}
	bSum := [3][2][16]E{add2x16(&b[0], &b[3]), add2x16(&b[1], &b[4]), add2x16(&b[2], &b[5])}
	var z0, z1, z2 [5][3][31]E
	mul3x2x16E(&aLo, &bLo, &z0, t)
	mul3x2x16E(&aHi, &bHi, &z2, t)
	mul3x2x16E(&aSum, &bSum, &z1, t)
	for i := 0; i < 3; i++ {
		out[i] = z0[i]
	}
	for i := 0; i < 2; i++ {
		out[3+i] = z0[3+i]
		add3x31Assign(&out[3+i], &z1[i])
		add3x31Assign(&out[3+i], &z0[i])
		add3x31Assign(&out[3+i], &z2[i])
	}
	out[5] = z1[2]
	add3x31Assign(&out[5], &z0[2])
	add3x31Assign(&out[5], &z2[2])
	for i := 0; i < 2; i++ {
		out[6+i] = z2[i]
		add3x31Assign(&out[6+i], &z1[3+i])
		add3x31Assign(&out[6+i], &z0[3+i])
		add3x31Assign(&out[6+i], &z2[3+i])
	}
	for i := 2; i < 5; i++ {
		out[6+i] = z2[i]
	}
}

func evalBilinearRSIP(rArr, vArr *[12][2][16]E, wRS []E, t *KTables) E {
	var aLo, aHi, aSum, bLo, bHi, bSum [6][2][16]E
	for i := 0; i < 6; i++ {
		aLo[i], aHi[i] = rArr[i], rArr[6+i]
		aSum[i] = add2x16(&rArr[i], &rArr[6+i])
		bLo[i], bHi[i] = vArr[i], vArr[6+i]
		bSum[i] = add2x16(&vArr[i], &vArr[6+i])
	}
	var z0, z1, z2 [11][3][31]E
	mul6x2x16E(&aLo, &bLo, &z0, t)
	mul6x2x16E(&aHi, &bHi, &z2, t)
	mul6x2x16E(&aSum, &bSum, &z1, t)

	var c [23][4][44]E
	for i := 0; i < 6; i++ {
		for z := 0; z < 3; z++ {
			copy(c[i][z][:31], z0[i][z][:])
		}
	}
	for i := 0; i < 5; i++ {
		for z := 0; z < 3; z++ {
			for u := 0; u < 31; u++ {
				c[6+i][z][u] = z0[6+i][z][u].Add(z1[i][z][u]).Add(z0[i][z][u]).Add(z2[i][z][u])
			}
		}
	}
	for z := 0; z < 3; z++ {
		for u := 0; u < 31; u++ {
			c[11][z][u] = z1[5][z][u].Add(z0[5][z][u]).Add(z2[5][z][u])
		}
	}
	for i := 0; i < 5; i++ {
		for z := 0; z < 3; z++ {
			for u := 0; u < 31; u++ {
				c[12+i][z][u] = z2[i][z][u].Add(z1[6+i][z][u]).Add(z0[6+i][z][u]).Add(z2[6+i][z][u])
			}
		}
	}
	for i := 5; i < 11; i++ {
		for z := 0; z < 3; z++ {
			copy(c[12+i][z][:31], z2[i][z][:])
		}
	}

	// 1. Reduce Y^12 = Y^3 + Y + Z + x
	for v := 22; v >= 12; v-- {
		for z := 0; z < 3; z++ {
			for u := 0; u < 31; u++ {
				val := c[v][z][u]
				if !val.IsZero() {
					c[v-12+3][z][u].AddAssign(val)
					c[v-12+1][z][u].AddAssign(val)
					c[v-12][z+1][u].AddAssign(val)
					c[v-12][z][u+1].AddAssign(val)
				}
			}
		}
	}
	// 2. Reduce Z^2 = Z + x^11
	for v := 0; v < 12; v++ {
		for z := 3; z >= 2; z-- {
			for u := 0; u < 32; u++ {
				val := c[v][z][u]
				if !val.IsZero() {
					c[v][z-1][u].AddAssign(val)
					c[v][z-2][u+11].AddAssign(val)
				}
			}
		}
	}
	// 3. Reduce x^16 = x^5 + x^3 + x + 1
	var acc E
	for v := 0; v < 12; v++ {
		for z := 0; z < 2; z++ {
			for u := 42; u >= 16; u-- {
				val := c[v][z][u]
				if !val.IsZero() {
					c[v][z][u-16+5].AddAssign(val)
					c[v][z][u-16+3].AddAssign(val)
					c[v][z][u-16+1].AddAssign(val)
					c[v][z][u-16].AddAssign(val)
				}
			}
			for u := 0; u < 16; u++ {
				acc.AddAssign(c[v][z][u].Mul(wRS[32*v+16*z+u], t))
			}
		}
	}
	return acc
}

func evalVerifierIPLhs(
	rl *RowLayout,
	r1 *[VSC1]E,
	tau E,
	gammas *[5]E,
	wRS []E,
	eta1, eta2, eta3 E,
	rRowP *[VSC2]E,
	vOpenH []E,
	t *KTables,
) E {
	eqSubrow := eqTable(r1[0:5], t)
	eqCol := eqTable(r1[5:14], t)
	eqRP := eqTable(rRowP[:], t)

	var rArr, vArr [12][2][16]E
	for x := 0; x < 5; x++ {
		gx := gammas[x]
		for sub := 0; sub < ActiveSubrowsPoly; sub++ {
			gEs := eqSubrow[sub]
			if x > 0 {
				gEs = gx.Mul(gEs, t)
			}
			rWt := eqRP[rl.nonMaskRows[ActiveSubrowsPoly*x+sub]]
			for v := 0; v < 12; v++ {
				lo := gEs.C[v] & 0xFFFF
				for lo != 0 {
					u := bits.TrailingZeros32(lo)
					rArr[v][0][u].AddAssign(rWt)
					lo &= lo - 1
				}
				hi := gEs.C[v] >> 16
				for hi != 0 {
					u := bits.TrailingZeros32(hi)
					rArr[v][1][u].AddAssign(rWt)
					hi &= hi - 1
				}
			}
		}
	}
	for col := 0; col < KCols; col++ {
		ec := eqCol[col]
		vo := vOpenH[col]
		for v := 0; v < 12; v++ {
			lo := ec.C[v] & 0xFFFF
			for lo != 0 {
				u := bits.TrailingZeros32(lo)
				vArr[v][0][u].AddAssign(vo)
				lo &= lo - 1
			}
			hi := ec.C[v] >> 16
			for hi != 0 {
				u := bits.TrailingZeros32(hi)
				vArr[v][1][u].AddAssign(vo)
				hi &= hi - 1
			}
		}
	}
	ipLhs := evalBilinearRSIP(&rArr, &vArr, wRS, t)

	// 2. Ring-Switch mask rows (5 x 384) — fast basis stepping (eliminates 1,920 E.Mul calls!)
	lMap := newRingSwitchLinearMap(wRS)
	for x := 0; x < 5; x++ {
		gTau := tau
		if x > 0 {
			gTau = gammas[x].Mul(tau, t)
		}
		rWt := eqRP[rl.nonMaskRows[MWitRows+x]]
		var rowAcc E
		curY := gTau
		for v := 0; v < 12; v++ {
			curLo := curY
			curHi := curY.mulByZ(t)
			base := v * 32
			for u := 0; u < 16; u++ {
				rowAcc.AddAssign(lMap.apply(curLo).Mul(vOpenH[base+u], t))
				rowAcc.AddAssign(lMap.apply(curHi).Mul(vOpenH[base+16+u], t))
				curLo = curLo.mulByX()
				curHi = curHi.mulByX()
			}
			curY = curY.mulByY()
		}
		ipLhs.AddAssign(rWt.Mul(rowAcc, t))
	}

	// 3. G_1 + m_g1 + m_g2 rows (936 entries) — factored rG10/rG11 + mulByY (eliminates 1,870 E.Mul calls!)
	w78 := evalG1MGWeights(r1, tau, eta1, eta2, t)
	rG10 := eqRP[rl.nonMaskRows[MWitRows+MRSMaskRows]]
	rG11 := eqRP[rl.nonMaskRows[MWitRows+MRSMaskRows+1]]
	var accG10, accG11 E
	for eIdx := 0; eIdx < 78; eIdx++ {
		cur := w78[eIdx]
		for v := 0; v < 12; v++ {
			flat := eIdx*12 + v
			col := flat & 0x1FF
			term := cur.Mul(vOpenH[col], t)
			if (flat >> 9) == 0 {
				accG10.AddAssign(term)
			} else {
				accG11.AddAssign(term)
			}
			cur = cur.mulByY()
		}
	}
	ipLhs.AddAssign(rG10.Mul(accG10, t))
	ipLhs.AddAssign(rG11.Mul(accG11, t))

	// 4. Sumcheck-2 16 mask rows
	var sumA, sumB E
	for _, pair := range Mask2Pairs {
		sumA.AddAssign(eqRP[pair[0]])
		sumB.AddAssign(eqRP[pair[1]])
	}
	wBlk0 := eta3.Mul(sumA.Add(sumB.ScaleK(Zeta1K, t)), t)
	wBlk1 := eta3.Mul(sumA.Add(sumB.ScaleK(Zeta2K, t)), t)
	for v := 0; v < 12; v++ {
		yv := EFromBasisVU(v, 0)
		ipLhs.AddAssign(wBlk0.Mul(yv, t).Mul(vOpenH[v], t))
		ipLhs.AddAssign(wBlk1.Mul(yv, t).Mul(vOpenH[12+v], t))
	}
	return ipLhs
}

func verifyBiniusProof(stmt *PaymentStatement, proof *BiniusProof) error {
	if len(proof.VProxCoeffs) != DegBound || len(proof.VOpenCoeffs) != DegBound {
		return errors.New("invalid v_prox/v_open degree bound")
	}
	t := getKTables()
	rl := buildRowLayout()
	stmtBytes := encodeStatement(stmt)

	tr := newTranscript(stmtBytes, &proof.MerkleRoot, proof.SG1, proof.SMask2)
	alpha := tr.squeezeE("C02")

	var r1 [VSC1]E
	for d := 0; d < VSC1; d++ {
		scTag := fmt.Sprintf("S1%02d", d)
		chTag := fmt.Sprintf("C%02d", d+3)
		tr.absorbE(scTag, proof.SC1Rounds[d][:])
		rCh := tr.squeezeE(chTag)
		if rCh.IsZero() {
			return errors.New("zero SC1 challenge")
		}
		r1[d] = rCh
	}

	wbR1, wcR1, pubRHS := evalVerifierWeightsAtR1(stmt, alpha, &r1, t)
	claim1 := pubRHS.Add(proof.SG1)
	for d := 0; d < VSC1; d++ {
		c0, c2, c3 := proof.SC1Rounds[d][0], proof.SC1Rounds[d][1], proof.SC1Rounds[d][2]
		c1 := claim1.Add(c2).Add(c3)
		rd := r1[d]
		claim1 = c0.Add(rd.Mul(c1.Add(rd.Mul(c2.Add(rd.Mul(c3, t)), t)), t))
	}

	tr.absorbE("DEG2C", []E{proof.EvalC1, proof.EvalC2})
	tau := tr.squeezeE("C22")
	if tau.IsZero() {
		return errors.New("zero tau challenge")
	}
	lhsDeg2 := claim1.Add(tau.Mul(proof.EvalC1, t)).Add(tau.Sq(t).Mul(proof.EvalC2, t))
	rhsDeg2 := proof.EvalZG
	for x := 0; x < 5; x++ {
		xp1 := (x + 1) % 5
		xp2 := (x + 2) % 5
		rhsDeg2.AddAssign(wbR1[x].Mul(proof.EvalZ[x], t))
		rhsDeg2.AddAssign(wcR1[x].Mul(EOne.Add(proof.EvalZ[xp1]).Mul(proof.EvalZ[xp2], t), t))
	}
	if lhsDeg2 != rhsDeg2 {
		return errors.New("Degree-2 evaluation hiding check failed")
	}

	tr.absorbE("DEG2Z", []E{
		proof.EvalZ[0], proof.EvalZ[1], proof.EvalZ[2], proof.EvalZ[3], proof.EvalZ[4], proof.EvalZG,
	})

	gamma := tr.squeezeE("C23")
	gammas := expandGammas(gamma, t)
	eqBit := eqTable(r1[14:19], t)
	var rsLhs E
	for b := 0; b < 32; b++ {
		rsLhs.AddAssign(eqBit[b].Mul(proof.RSSPrime[b], t))
	}
	rsRhs := proof.EvalZ[0]
	for x := 1; x < 5; x++ {
		rsRhs.AddAssign(gammas[x].Mul(proof.EvalZ[x], t))
	}
	if rsLhs != rsRhs {
		return errors.New("Ring-Switching sum check failed")
	}

	tr.absorbE("RSSP", proof.RSSPrime[:])
	rhoRS := tr.squeezeE("C24")
	if rhoRS.IsZero() {
		return errors.New("zero rho_rs challenge")
	}
	wRS, eta1, eta2, eta3 := expandRhoRS(rhoRS, t)

	rsTarget := ringSwitchTarget(&proof.RSSPrime, wRS, t)
	claim2 := rsTarget.Add(eta1.Mul(proof.EvalZG, t)).Add(eta2.Mul(proof.SG1, t)).Add(eta3.Mul(proof.SMask2, t))
	var rRowP [VSC2]E
	for d := 0; d < VSC2; d++ {
		c0, c2 := proof.SC2Rounds[d][0], proof.SC2Rounds[d][1]
		c1 := claim2.Add(c2)
		tr.absorbE(fmt.Sprintf("S2%02d", d), proof.SC2Rounds[d][:])
		rCh := tr.squeezeE(fmt.Sprintf("C%02d", d+25))
		rRowP[d] = rCh
		claim2 = c0.Add(rCh.Mul(c1.Add(rCh.Mul(c2, t)), t))
	}

	tr.absorbE("BEVAL", []E{proof.BEval})
	lambdaP := tr.squeezeE("C33P")
	tr.absorbE("VPROX", proof.VProxCoeffs)
	lambdaO := tr.squeezeE("C33O")
	tr.absorbE("VOPEN", proof.VOpenCoeffs)
	trFinal := tr.state

	vOpenH := make([]E, KCols)
	copy(vOpenH, proof.VOpenCoeffs[:KCols])
	t.anttForwardHE(vOpenH)
	ipLhs := evalVerifierIPLhs(&rl, &r1, tau, &gammas, wRS, eta1, eta2, eta3, &rRowP, vOpenH, t)
	ipRhs := proof.BEval.Add(lambdaO.Mul(claim2, t))
	if ipLhs != ipRhs {
		return errors.New("PCS H-domain inner-product check failed")
	}

	powOK, qIndices := evalHC4(&trFinal, &proof.PowNonce)
	if !powOK {
		return errors.New("PoW verification failed")
	}
	uniq := make([]int, NumQueries)
	copy(uniq, qIndices[:])
	sort.Ints(uniq)
	nUniq := 0
	for _, idx := range uniq {
		if nUniq == 0 || uniq[nUniq-1] != idx {
			uniq[nUniq] = idx
			nUniq++
		}
	}
	uniq = uniq[:nUniq]
	if len(proof.OpenedCols) != len(uniq) {
		return errors.New("opened column count mismatch")
	}

	// Pruned SHA-384 Merkle verification
	levelMap := make(map[int][HashBytes]byte, len(uniq))
	for i, idx := range uniq {
		col := &proof.OpenedCols[i]
		levelMap[idx] = oracleLeaf(idx, &col.KVals, col.BPCS, col.BProx)
	}
	sibCursor := 0
	for lvl := 0; lvl < MerkleHeight; lvl++ {
		keys := make([]int, 0, len(levelMap))
		for k := range levelMap {
			keys = append(keys, k)
		}
		sort.Ints(keys)
		nextMap := make(map[int][HashBytes]byte, len(keys))
		for ki := 0; ki < len(keys); {
			pos := keys[ki]
			leftPos := pos & ^1
			rightPos := leftPos | 1
			var lh, rh [HashBytes]byte
			if pos == leftPos {
				lh = levelMap[leftPos]
				if ki+1 < len(keys) && keys[ki+1] == rightPos {
					rh = levelMap[rightPos]
					ki += 2
				} else {
					if sibCursor >= len(proof.MerkleSiblings) {
						return errors.New("missing Merkle sibling")
					}
					rh = proof.MerkleSiblings[sibCursor]
					sibCursor++
					ki++
				}
			} else {
				if sibCursor >= len(proof.MerkleSiblings) {
					return errors.New("missing Merkle sibling")
				}
				lh = proof.MerkleSiblings[sibCursor]
				sibCursor++
				rh = levelMap[rightPos]
				ki++
			}
			parent := pos >> 1
			nextMap[parent] = oracleNode(uint8(lvl), parent, &lh, &rh)
		}
		levelMap = nextMap
	}
	if sibCursor != len(proof.MerkleSiblings) {
		return errors.New("unused trailing Merkle siblings")
	}
	rootComputed, ok := levelMap[0]
	if !ok || rootComputed != proof.MerkleRoot {
		return errors.New("Merkle root mismatch")
	}

	// Pointwise Dual-Polynomial PCS Consistency
	var proxWeights [MKRows]E
	curLP := lambdaP.Sq(t)
	for i := 0; i < MKRows; i++ {
		proxWeights[i] = curLP
		curLP = curLP.Mul(lambdaP, t)
	}
	var openWeights [MKRows]E
	for i := 0; i < MKRows; i++ {
		openWeights[i] = lambdaO.Mul(eqEvalPoint(rRowP[:], rl.activeRows[i], t), t)
	}

	basis := make([]uint32, DegBound)
	for i, idx := range uniq {
		col := &proof.OpenedCols[i]
		x := LShift ^ uint32(idx)
		t.novelBasisEvals(x, DegBound, basis)

		var vProxEval, vOpenEval E
		for j := 0; j < DegBound; j++ {
			addMulKPair(basis[j], &proof.VProxCoeffs[j], &proof.VOpenCoeffs[j], &vProxEval, &vOpenEval, t)
		}
		colProx := col.BProx.Add(lambdaP.Mul(col.BPCS, t))
		colOpen := col.BPCS
		for aidx := 0; aidx < MKRows; aidx++ {
			if v := col.KVals[aidx]; v != 0 {
				addMulKPair(v, &proxWeights[aidx], &openWeights[aidx], &colProx, &colOpen, t)
			}
		}
		if vProxEval != colProx {
			return errors.New("v_prox consistency check failed")
		}
		if vOpenEval != colOpen {
			return errors.New("v_open consistency check failed")
		}
	}
	return nil
}
