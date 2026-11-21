// Independent Go implementation of the 56-permutation Depth-24 global Merkle
// + 9,216 Merkle mux + 6,144 sibling + 448 is_dummy + 193 adder circuit verifier weights.
package main

const (
	NumPerms           = 56
	VSC1               = 19
	MerkleDepth        = 24
	AddrBytes          = 48
	CoinBytes          = 48
	SNBytes            = 48
	NodeBytes          = 48
	OwnerSecretBytes   = 48
	BlindSecretBytes   = 78
	ActiveSubrowsPoly  = 28

	TagAddr byte = 0x00
	TagCoin byte = 0x01
	TagSN   byte = 0x02
	TagNode byte = 0x03

	RowMuxBase       = NumPerms * 7680 // 430080
	MuxRowsPerLevel  = 192
	RowSibBase       = RowMuxBase + 2*MerkleDepth*MuxRowsPerLevel // 439296
	SibRowsPerLevel  = 128
	RowDummyValBase  = RowSibBase + 2*MerkleDepth*SibRowsPerLevel // 445440
	RowDummyRootBase = RowDummyValBase + 2*32                      // 445504
	RowAdderLHS1     = RowDummyRootBase + 2*192                    // 445888
	RowAdderLHS      = RowAdderLHS1 + 64                           // 445952
	RowAdderMID      = RowAdderLHS + 65                            // 446017
	RowAdderRHS      = RowAdderMID + 64                            // 446081
	RowPrivBase      = RowAdderRHS + 65                            // 446146

	OffSIn0     = 0
	OffSIn1     = 384
	OffVIn0     = 768
	OffVIn1     = 832
	OffRIn0     = 896
	OffRIn1     = 1520
	OffAOut0    = 2144
	OffAOut1    = 2528
	OffVOut0    = 2912
	OffVOut1    = 2976
	OffROut0    = 3040
	OffROut1    = 3664
	OffIsDummy0 = 4288
	OffIsDummy1 = 4289
	OffDir0     = 4290
	OffDir1     = 4314
	TotalPriv   = 4338
	PrivRows    = (TotalPriv + 4) / 5
	ActiveRows  = RowPrivBase + PrivRows // 446949
)

var KeccakRC = [24]uint64{
	0x0000000000000001, 0x0000000000008082, 0x800000000000808a, 0x8000000080008000,
	0x000000000000808b, 0x0000000080000001, 0x8000000080008081, 0x8000000000008009,
	0x000000000000008a, 0x0000000000000088, 0x0000000080008009, 0x000000008000000a,
	0x000000008000808b, 0x800000000000008b, 0x8000000000008089, 0x8000000000008003,
	0x8000000000008002, 0x8000000000000080, 0x000000000000800a, 0x800000008000000a,
	0x8000000080008081, 0x8000000000008080, 0x0000000080000001, 0x8000000080008008,
}

var RhoOffsets = [5][5]int{
	{0, 36, 3, 41, 18},
	{1, 44, 10, 45, 2},
	{62, 6, 43, 15, 61},
	{28, 55, 25, 21, 56},
	{27, 20, 39, 8, 14},
}

func keccakRow(perm, round, y, z int) int {
	return perm*7680 + round*320 + y*64 + z
}

func muxRow(inputB, lvl, k int) int {
	return RowMuxBase + (inputB*MerkleDepth+lvl)*MuxRowsPerLevel + k
}

func sibLoc(inputB, lvl, bitIdx int) (int, int) {
	return bitIdx % 3, RowSibBase + (inputB*MerkleDepth+lvl)*SibRowsPerLevel + bitIdx/3
}

func dummyValRow(inputB, k int) int {
	return RowDummyValBase + inputB*32 + k
}

func dummyRootRow(inputB, k int) int {
	return RowDummyRootBase + inputB*192 + k
}

func privLoc(bitOff int) (int, int) {
	return bitOff % 5, RowPrivBase + bitOff/5
}

func getByteBit(b []byte, bitIdx int) uint8 {
	return (b[bitIdx/8] >> (bitIdx % 8)) & 1
}

type BitCoord struct{ X, Y, Z int }

func trpInputBits(nx, ny, nz int) [11]BitCoord {
	y := nx
	x := (3 * (ny + 2*nx)) % 5
	z := (nz + 64 - RhoOffsets[x][y]) % 64
	xm1 := (x + 4) % 5
	xp1 := (x + 1) % 5
	zm1 := (z + 63) % 64
	return [11]BitCoord{
		{x, y, z},
		{xm1, 0, z}, {xm1, 1, z}, {xm1, 2, z}, {xm1, 3, z}, {xm1, 4, z},
		{xp1, 0, zm1}, {xp1, 1, zm1}, {xp1, 2, zm1}, {xp1, 3, zm1}, {xp1, 4, zm1},
	}
}

type PaymentStatement struct {
	Anchor    [NodeBytes]byte
	SN0       [SNBytes]byte
	SN1       [SNBytes]byte
	CoinOut0  [CoinBytes]byte
	CoinOut1  [CoinBytes]byte
	VMint     uint64
	VUnshield uint64
	Fee       uint64
	TxCtx     [48]byte
}

func (s *PaymentStatement) pubExit() uint64 {
	return s.Fee + s.VUnshield
}

type InputBitKind uint8

const (
	SrcConst InputBitKind = iota
	SrcPriv
	SrcPermOut
)

type InputBitSrc struct {
	Kind    InputBitKind
	Val     int
	X, Y, Z int
}

func basePermInputSpec(p int) [1600]InputBitSrc {
	var spec [1600]InputBitSrc
	setBytesConst := func(byteOff int, data []byte) {
		for bi, b := range data {
			for bit := 0; bit < 8; bit++ {
				spec[(byteOff+bi)*8+bit] = InputBitSrc{Kind: SrcConst, Val: int((b >> bit) & 1)}
			}
		}
	}
	setPriv := func(byteOff, privBitOff, nbytes int) {
		for b := 0; b < nbytes*8; b++ {
			spec[byteOff*8+b] = InputBitSrc{Kind: SrcPriv, Val: privBitOff + b}
		}
	}
	setPermOut := func(byteOff, srcPerm, nbytes int) {
		for b := 0; b < nbytes*8; b++ {
			lane := b / 64
			spec[byteOff*8+b] = InputBitSrc{Kind: SrcPermOut, Val: srcPerm, X: lane % 5, Y: lane / 5, Z: b % 64}
		}
	}
	setShakePad := func(msgLen int) {
		for bit := 0; bit < 8; bit++ {
			spec[msgLen*8+bit] = InputBitSrc{Kind: SrcConst, Val: int((0x1F >> bit) & 1)}
		}
		spec[135*8+7] = InputBitSrc{Kind: SrcConst, Val: 1}
	}
	switch p {
	case 0:
		setBytesConst(0, []byte{TagAddr})
		setPriv(1, OffSIn0, OwnerSecretBytes)
		setShakePad(1 + OwnerSecretBytes)
	case 1:
		setBytesConst(0, []byte{TagCoin})
		setPermOut(1, 0, AddrBytes)
		setPriv(49, OffVIn0, 8)
		setPriv(57, OffRIn0, BlindSecretBytes)
		setShakePad(135)
	case 2:
		setBytesConst(0, []byte{TagSN})
		setPriv(1, OffSIn0, OwnerSecretBytes)
		setPermOut(49, 1, CoinBytes)
		setShakePad(97)
	case 3:
		setBytesConst(0, []byte{TagAddr})
		setPriv(1, OffSIn1, OwnerSecretBytes)
		setShakePad(1 + OwnerSecretBytes)
	case 4:
		setBytesConst(0, []byte{TagCoin})
		setPermOut(1, 3, AddrBytes)
		setPriv(49, OffVIn1, 8)
		setPriv(57, OffRIn1, BlindSecretBytes)
		setShakePad(135)
	case 5:
		setBytesConst(0, []byte{TagSN})
		setPriv(1, OffSIn1, OwnerSecretBytes)
		setPermOut(49, 4, CoinBytes)
		setShakePad(97)
	case 6:
		setBytesConst(0, []byte{TagCoin})
		setPriv(1, OffAOut0, AddrBytes)
		setPriv(49, OffVOut0, 8)
		setPriv(57, OffROut0, BlindSecretBytes)
		setShakePad(135)
	case 7:
		setBytesConst(0, []byte{TagCoin})
		setPriv(1, OffAOut1, AddrBytes)
		setPriv(49, OffVOut1, 8)
		setPriv(57, OffROut1, BlindSecretBytes)
		setShakePad(135)
	}
	return spec
}

type Term struct {
	X   uint8
	Row uint32
}

type ScalarConstraint struct {
	Lin  []Term
	Quad []Term
	RHS  uint8
}

func permOutTerm(perm, bitIdx int) (Term, Term, uint8) {
	lane := bitIdx / 64
	x := lane % 5
	y := lane / 5
	z := bitIdx % 64
	row := uint32(keccakRow(perm, 23, y, z))
	var rc uint8
	if x == 0 && y == 0 {
		rc = uint8((KeccakRC[23] >> z) & 1)
	}
	t := Term{uint8(x), row}
	return t, t, rc
}

// buildGroupCNonC2 builds (C1 [1536], C3+C4+C5 [3778]) for fast verifier evaluation.
func buildGroupCNonC2(stmt *PaymentStatement) ([]ScalarConstraint, []ScalarConstraint) {
	c1 := make([]ScalarConstraint, 0, 1536)
	rest := make([]ScalarConstraint, 0, 3800)

	pubOuts := []struct {
		perm   int
		target []byte
	}{
		{2, stmt.SN0[:]},
		{5, stmt.SN1[:]},
		{6, stmt.CoinOut0[:]},
		{7, stmt.CoinOut1[:]},
	}
	for _, po := range pubOuts {
		for b := 0; b < 384; b++ {
			lt, qt, rc := permOutTerm(po.perm, b)
			c1 = append(c1, ScalarConstraint{Lin: []Term{lt}, Quad: []Term{qt}, RHS: getByteBit(po.target, b) ^ rc})
		}
	}

	// C3: is_dummy value zero-gating
	for inputB := 0; inputB < 2; inputB++ {
		dOff := OffIsDummy0
		vBase := OffVIn0
		if inputB == 1 {
			dOff = OffIsDummy1
			vBase = OffVIn1
		}
		dx, dr := privLoc(dOff)
		dTerm := Term{uint8(dx), uint32(dr)}
		for k := 0; k < 32; k++ {
			rDv := uint32(dummyValRow(inputB, k))
			vx0, vr0 := privLoc(vBase + 2*k)
			vx1, vr1 := privLoc(vBase + 2*k + 1)
			rest = append(rest,
				ScalarConstraint{Lin: []Term{{0, rDv}, dTerm}, RHS: 1},
				ScalarConstraint{Lin: []Term{{1, rDv}, {uint8(vx0), uint32(vr0)}}, RHS: 0},
				ScalarConstraint{Quad: []Term{{4, rDv}}, RHS: 0},
				ScalarConstraint{Lin: []Term{{2, rDv}, dTerm}, RHS: 1},
				ScalarConstraint{Lin: []Term{{3, rDv}, {uint8(vx1), uint32(vr1)}}, RHS: 0},
				ScalarConstraint{Quad: []Term{{1, rDv}}, RHS: 0},
			)
		}
	}

	// C4: is_dummy anchor root-gating
	for inputB := 0; inputB < 2; inputB++ {
		dOff := OffIsDummy0
		if inputB == 1 {
			dOff = OffIsDummy1
		}
		dx, dr := privLoc(dOff)
		dTerm := Term{uint8(dx), uint32(dr)}
		rootPerm := 8 + inputB*MerkleDepth + (MerkleDepth - 1)
		for k := 0; k < 192; k++ {
			rDr := uint32(dummyRootRow(inputB, k))
			lt0, qt0, rc0 := permOutTerm(rootPerm, 2*k)
			lt1, qt1, rc1 := permOutTerm(rootPerm, 2*k+1)
			a0 := getByteBit(stmt.Anchor[:], 2*k)
			a1 := getByteBit(stmt.Anchor[:], 2*k+1)
			rest = append(rest,
				ScalarConstraint{Lin: []Term{{0, rDr}, dTerm}, RHS: 0},
				ScalarConstraint{Lin: []Term{{1, rDr}, lt0}, Quad: []Term{qt0}, RHS: a0 ^ rc0},
				ScalarConstraint{Quad: []Term{{4, rDr}}, RHS: 0},
				ScalarConstraint{Lin: []Term{{2, rDr}, dTerm}, RHS: 0},
				ScalarConstraint{Lin: []Term{{3, rDr}, lt1}, Quad: []Term{qt1}, RHS: a1 ^ rc1},
				ScalarConstraint{Quad: []Term{{1, rDr}}, RHS: 0},
			)
		}
	}

	// C5: 65-bit ripple-carry conservation adders
	pubExit := stmt.pubExit()
	addAdder := func(base, bits int) {
		rest = append(rest, ScalarConstraint{Lin: []Term{{2, uint32(base)}}, RHS: 1})
		for i := 0; i < bits; i++ {
			row := uint32(base + i)
			rest = append(rest, ScalarConstraint{Lin: []Term{{0, row}, {1, row}, {3, row}}, RHS: 1})
			rest = append(rest, ScalarConstraint{Lin: []Term{{4, row}}, Quad: []Term{{4, row}, {1, row}}, RHS: 0})
			if i > 0 {
				rest = append(rest, ScalarConstraint{Lin: []Term{{2, row}, {4, row - 1}}, RHS: 1})
			}
		}
	}
	addAdder(RowAdderLHS1, 64)
	addAdder(RowAdderLHS, 65)
	addAdder(RowAdderMID, 64)
	addAdder(RowAdderRHS, 65)

	for i := 0; i < 64; i++ {
		xa0, ra0 := privLoc(OffVIn0 + i)
		xb0, rb0 := privLoc(OffVIn1 + i)
		rl1 := uint32(RowAdderLHS1 + i)
		rest = append(rest,
			ScalarConstraint{Lin: []Term{{0, rl1}, {uint8(xa0), uint32(ra0)}}, RHS: 1},
			ScalarConstraint{Lin: []Term{{1, rl1}, {uint8(xb0), uint32(rb0)}}, RHS: 0},
		)
		xa1, ra1 := privLoc(OffVOut0 + i)
		xb1, rb1 := privLoc(OffVOut1 + i)
		rm := uint32(RowAdderMID + i)
		rest = append(rest,
			ScalarConstraint{Lin: []Term{{0, rm}, {uint8(xa1), uint32(ra1)}}, RHS: 1},
			ScalarConstraint{Lin: []Term{{1, rm}, {uint8(xb1), uint32(rb1)}}, RHS: 0},
		)
	}
	for i := 0; i < 65; i++ {
		rl := uint32(RowAdderLHS + i)
		var mintBit uint8
		if i < 64 {
			mintBit = uint8((stmt.VMint >> i) & 1)
		}
		rest = append(rest, ScalarConstraint{Lin: []Term{{1, rl}}, RHS: mintBit})
		if i < 64 {
			rl1 := uint32(RowAdderLHS1 + i)
			rest = append(rest, ScalarConstraint{Lin: []Term{{0, rl}, {3, rl1}, {2, rl1}}, RHS: 0})
		} else {
			rl1_63 := uint32(RowAdderLHS1 + 63)
			rest = append(rest, ScalarConstraint{Lin: []Term{{0, rl}, {4, rl1_63}}, RHS: 1})
		}
	}
	for i := 0; i < 65; i++ {
		rr := uint32(RowAdderRHS + i)
		var exitBit uint8
		if i < 64 {
			exitBit = uint8((pubExit >> i) & 1)
		}
		rest = append(rest, ScalarConstraint{Lin: []Term{{1, rr}}, RHS: exitBit})
		if i < 64 {
			rm := uint32(RowAdderMID + i)
			rest = append(rest, ScalarConstraint{Lin: []Term{{0, rr}, {3, rm}, {2, rm}}, RHS: 0})
		} else {
			rm63 := uint32(RowAdderMID + 63)
			rest = append(rest, ScalarConstraint{Lin: []Term{{0, rr}, {4, rm63}}, RHS: 1})
		}
	}
	for i := 0; i < 65; i++ {
		rl := uint32(RowAdderLHS + i)
		rr := uint32(RowAdderRHS + i)
		rest = append(rest, ScalarConstraint{Lin: []Term{{3, rl}, {2, rl}, {3, rr}, {2, rr}}, RHS: 0})
	}
	rest = append(rest, ScalarConstraint{Lin: []Term{{4, uint32(RowAdderLHS + 64)}, {4, uint32(RowAdderRHS + 64)}}, RHS: 0})
	return c1, rest
}

// buildAllGroupCConstraints builds full [C1, C2, C3, C4, C5] for 3-way IR equivalence audit.
func buildAllGroupCConstraints(stmt *PaymentStatement) []ScalarConstraint {
	c1, rest := buildGroupCNonC2(stmt)
	cs := make([]ScalarConstraint, 0, len(c1)+36864+len(rest))
	cs = append(cs, c1...)
	for inputB := 0; inputB < 2; inputB++ {
		dirBase := OffDir0
		if inputB == 1 {
			dirBase = OffDir1
		}
		for lvl := 0; lvl < MerkleDepth; lvl++ {
			srcPerm := 8 + inputB*MerkleDepth + lvl - 1
			if lvl == 0 {
				if inputB == 0 {
					srcPerm = 1
				} else {
					srcPerm = 4
				}
			}
			dx, dr := privLoc(dirBase + lvl)
			dTerm := Term{uint8(dx), uint32(dr)}
			for k := 0; k < MuxRowsPerLevel; k++ {
				rMux := uint32(muxRow(inputB, lvl, k))
				lt0, qt0, rc0 := permOutTerm(srcPerm, 2*k)
				lt1, qt1, rc1 := permOutTerm(srcPerm, 2*k+1)
				sx0, sr0 := sibLoc(inputB, lvl, 2*k)
				sx1, sr1 := sibLoc(inputB, lvl, 2*k+1)
				cs = append(cs,
					ScalarConstraint{Lin: []Term{{0, rMux}, dTerm}, RHS: 1},
					ScalarConstraint{Lin: []Term{{2, rMux}, dTerm}, RHS: 1},
					ScalarConstraint{Lin: []Term{{1, rMux}, lt0, {uint8(sx0), uint32(sr0)}}, Quad: []Term{qt0}, RHS: rc0},
					ScalarConstraint{Lin: []Term{{3, rMux}, lt1, {uint8(sx1), uint32(sr1)}}, Quad: []Term{qt1}, RHS: rc1},
				)
			}
		}
	}
	cs = append(cs, rest...)
	return cs
}

func buildTRPTablesPow(alpha E, t *KTables) (*[5][320]E, *[5][320]E, E) {
	uNext := new([5][320]E)
	uCurr := new([5][320]E)
	w := EOne
	for ny := 0; ny < 5; ny++ {
		for nx := 0; nx < 5; nx++ {
			for nz := 0; nz < 64; nz++ {
				uNext[nx][64*ny+nz] = w
				for _, c := range trpInputBits(nx, ny, nz) {
					uCurr[c.X][64*c.Y+c.Z].AddAssign(w)
				}
				w = w.Mul(alpha, t)
			}
		}
	}
	return uNext, uCurr, w
}

func evalVerifierWeightsAtR1(stmt *PaymentStatement, alpha E, r1 *[VSC1]E, t *KTables) ([5]E, [5]E, E) {
	numHi := (ActiveRows + 63) / 64
	eqHi := eqTableScaledLimit(r1[:13], EOne, numHi, t)
	eqLo := eqTable(r1[13:], t)

	var wbEval, wcEval [5]E
	var pubRHS E

	uNext, uCurr, stride := buildTRPTablesPow(alpha, t)

	var unDot, ucDot [5][5]E
	for x := 0; x < 5; x++ {
		for y := 0; y < 5; y++ {
			var an, ac E
			for z := 0; z < 64; z++ {
				an.AddAssign(uNext[x][64*y+z].Mul(eqLo[z], t))
				ac.AddAssign(uCurr[x][64*y+z].Mul(eqLo[z], t))
			}
			unDot[x][y] = an
			ucDot[x][y] = ac
		}
	}

	// Group A
	curScale := EOne
	var sumNextY, sumCurrY [5]E
	var rcAcc [64]E
	for p := 0; p < NumPerms; p++ {
		for r := 0; r < 23; r++ {
			s := curScale
			curScale = curScale.Mul(stride, t)
			hiNext := (p*7680 + (r+1)*320) >> 6
			hiCurr := (p*7680 + r*320) >> 6
			for y := 0; y < 5; y++ {
				sumNextY[y].AddAssign(s.Mul(eqHi[hiNext+y], t))
				sumCurrY[y].AddAssign(s.Mul(eqHi[hiCurr+y], t))
			}
			rc := KeccakRC[r]
			for z := 0; z < 64; z++ {
				if ((rc >> z) & 1) != 0 {
					rcAcc[z].AddAssign(s)
				}
			}
		}
	}
	for x := 0; x < 5; x++ {
		for y := 0; y < 5; y++ {
			wbEval[x].AddAssign(sumNextY[y].Mul(unDot[x][y], t))
			tc := sumCurrY[y].Mul(ucDot[x][y], t)
			wbEval[x].AddAssign(tc)
			wcEval[x].AddAssign(tc)
		}
	}
	for z := 0; z < 64; z++ {
		if !rcAcc[z].IsZero() {
			pubRHS.AddAssign(rcAcc[z].Mul(uCurr[0][z], t))
		}
	}

	var bLo, cLo [5][64]E
	var sumR0Y [5]E

	// Group B1: Base 8 perms (factor `s` outside per-bit loop -> cuts ~3,800 E.Mul calls!)
	for p := 0; p < 8; p++ {
		s := curScale
		curScale = curScale.Mul(stride, t)
		hiR0 := (p * 7680) >> 6
		for y := 0; y < 5; y++ {
			sumR0Y[y].AddAssign(s.Mul(eqHi[hiR0+y], t))
		}
		spec := basePermInputSpec(p)
		var constAcc E
		var pBLo, pCLo [5][64]E
		for bitIdx, src := range spec {
			lane := bitIdx / 64
			ix := lane % 5
			iy := lane / 5
			iz := bitIdx % 64
			u := uCurr[ix][64*iy+iz]
			switch src.Kind {
			case SrcConst:
				if src.Val != 0 {
					constAcc.AddAssign(u)
				}
			case SrcPriv:
				px, prow := privLoc(src.Val)
				pBLo[px][prow&63].AddAssign(u.Mul(eqHi[prow>>6], t))
			case SrcPermOut:
				orow := keccakRow(src.Val, 23, src.Y, src.Z)
				we := u.Mul(eqHi[orow>>6], t)
				pBLo[src.X][orow&63].AddAssign(we)
				pCLo[src.X][orow&63].AddAssign(we)
				if src.X == 0 && src.Y == 0 && ((KeccakRC[23]>>src.Z)&1) != 0 {
					constAcc.AddAssign(u)
				}
			}
		}
		for x := 0; x < 5; x++ {
			for z := 0; z < 64; z++ {
				if !pBLo[x][z].IsZero() {
					bLo[x][z].AddAssign(s.Mul(pBLo[x][z], t))
				}
				if !pCLo[x][z].IsZero() {
					cLo[x][z].AddAssign(s.Mul(pCLo[x][z], t))
				}
			}
		}
		pubRHS.AddAssign(s.Mul(constAcc, t))
	}

	// Group B2: Factored 48 Merkle perms
	var bCurDot [6]E
	var bMuxDot [3][2]E
	var bSibDot [2][3]E
	var bMerkleRHS E

	addConstBit := func(bitIdx int) {
		lane := bitIdx / 64
		bMerkleRHS.AddAssign(uCurr[lane%5][64*(lane/5)+(bitIdx%64)])
	}
	for bit := 0; bit < 8; bit++ {
		if ((TagNode >> bit) & 1) != 0 {
			addConstBit(bit)
		}
		if ((0x1F >> bit) & 1) != 0 {
			addConstBit(97*8 + bit)
		}
	}
	addConstBit(135*8 + 7)

	for i := 0; i < 384; i++ {
		lbit := 8 + i
		ll := lbit / 64
		ul := uCurr[ll%5][64*(ll/5)+(lbit%64)]

		rbit := 49*8 + i
		rl := rbit / 64
		ur := uCurr[rl%5][64*(rl/5)+(rbit%64)]

		lane := i / 64
		z := i % 64
		bCurDot[lane].AddAssign(ul.Mul(eqLo[z], t))
		if lane == 0 && ((KeccakRC[23]>>z)&1) != 0 {
			bMerkleRHS.AddAssign(ul)
		}

		k := i / 2
		bMuxDot[k>>6][i&1].AddAssign(ul.Add(ur).Mul(eqLo[k&63], t))

		m := i / 3
		bSibDot[m>>6][i%3].AddAssign(ur.Mul(eqLo[m&63], t))
	}

	var bCurScale [6]E
	var bMuxScale [3]E
	var bSibScale [2]E
	var bRHSScale E
	for p := 8; p < NumPerms; p++ {
		s := curScale
		curScale = curScale.Mul(stride, t)
		rel := p - 8
		inputB := rel / MerkleDepth
		lvl := rel % MerkleDepth
		srcPerm := p - 1
		if lvl == 0 {
			if inputB == 0 {
				srcPerm = 1
			} else {
				srcPerm = 4
			}
		}

		hiR0 := (p * 7680) >> 6
		for y := 0; y < 5; y++ {
			sumR0Y[y].AddAssign(s.Mul(eqHi[hiR0+y], t))
		}
		bRHSScale.AddAssign(s)

		srcHiY0 := keccakRow(srcPerm, 23, 0, 0) >> 6
		srcHiY1 := keccakRow(srcPerm, 23, 1, 0) >> 6
		sY0 := s.Mul(eqHi[srcHiY0], t)
		sY1 := s.Mul(eqHi[srcHiY1], t)
		for lane := 0; lane < 5; lane++ {
			bCurScale[lane].AddAssign(sY0)
		}
		bCurScale[5].AddAssign(sY1)

		muxHi0 := muxRow(inputB, lvl, 0) >> 6
		for blk := 0; blk < 3; blk++ {
			bMuxScale[blk].AddAssign(s.Mul(eqHi[muxHi0+blk], t))
		}
		sibHi0 := (RowSibBase + rel*SibRowsPerLevel) >> 6
		for blk := 0; blk < 2; blk++ {
			bSibScale[blk].AddAssign(s.Mul(eqHi[sibHi0+blk], t))
		}
	}
	for x := 0; x < 5; x++ {
		for y := 0; y < 5; y++ {
			wbEval[x].AddAssign(sumR0Y[y].Mul(unDot[x][y], t))
		}
	}
	pubRHS.AddAssign(bRHSScale.Mul(bMerkleRHS, t))
	for lane := 0; lane < 6; lane++ {
		x := lane % 5
		tc := bCurScale[lane].Mul(bCurDot[lane], t)
		wbEval[x].AddAssign(tc)
		wcEval[x].AddAssign(tc)
	}
	for blk := 0; blk < 3; blk++ {
		wcEval[4].AddAssign(bMuxScale[blk].Mul(bMuxDot[blk][0], t))
		wcEval[1].AddAssign(bMuxScale[blk].Mul(bMuxDot[blk][1], t))
	}
	for blk := 0; blk < 2; blk++ {
		for x := 0; x < 3; x++ {
			wbEval[x].AddAssign(bSibScale[blk].Mul(bSibDot[blk][x], t))
		}
	}

	// Group C1 (0..1536): zero heap allocations!
	w := curScale
	addLin := func(x, row int) {
		bLo[x][row&63].AddAssign(w.Mul(eqHi[row>>6], t))
	}
	addQuad := func(x, row int) {
		cLo[x][row&63].AddAssign(w.Mul(eqHi[row>>6], t))
	}
	pubOuts := [4]struct {
		perm   int
		target []byte
	}{
		{2, stmt.SN0[:]},
		{5, stmt.SN1[:]},
		{6, stmt.CoinOut0[:]},
		{7, stmt.CoinOut1[:]},
	}
	for _, po := range pubOuts {
		// Factor eqHi per (perm, y in {0,1}): cuts 1,536 E.Mul -> 8 E.Mul!
		eqY0 := eqHi[keccakRow(po.perm, 23, 0, 0)>>6]
		eqY1 := eqHi[keccakRow(po.perm, 23, 1, 0)>>6]
		var acc0 [5]E
		var acc1 E
		for b := 0; b < 384; b++ {
			lane := b / 64
			x := lane % 5
			y := lane / 5
			z := b % 64
			var rc uint8
			if x == 0 && y == 0 {
				rc = uint8((KeccakRC[23] >> z) & 1)
			}
			we := w.Mul(eqLo[z], t)
			if y == 0 {
				acc0[x].AddAssign(we)
			} else {
				acc1.AddAssign(we)
			}
			if (getByteBit(po.target, b) ^ rc) != 0 {
				pubRHS.AddAssign(w)
			}
			w = w.Mul(alpha, t)
		}
		for x := 0; x < 5; x++ {
			tc := acc0[x].Mul(eqY0, t)
			wbEval[x].AddAssign(tc)
			wcEval[x].AddAssign(tc)
		}
		tc1 := acc1.Mul(eqY1, t)
		wbEval[0].AddAssign(tc1)
		wcEval[0].AddAssign(tc1)
	}

	// C2 factored across all 48 levels
	var c2MuxDot [3][4]E
	var c2CurDot [6]E
	var c2SibDot [2][3]E
	var c2DirSum, c2RHSTotal E
	pw := EOne
	for k := 0; k < MuxRowsPerLevel; k++ {
		w0 := pw
		w1 := w0.Mul(alpha, t)
		w2 := w1.Mul(alpha, t)
		w3 := w2.Mul(alpha, t)
		pw = w3.Mul(alpha, t)

		eqK := eqLo[k&63]
		blk := k >> 6
		c2MuxDot[blk][0].AddAssign(w0.Mul(eqK, t))
		c2MuxDot[blk][2].AddAssign(w1.Mul(eqK, t))
		c2MuxDot[blk][1].AddAssign(w2.Mul(eqK, t))
		c2MuxDot[blk][3].AddAssign(w3.Mul(eqK, t))

		w01 := w0.Add(w1)
		c2DirSum.AddAssign(w01)
		c2RHSTotal.AddAssign(w01)

		for idx, wbI := range [2]E{w2, w3} {
			bitOff := 2*k + idx
			lane := bitOff / 64
			z := bitOff % 64
			c2CurDot[lane].AddAssign(wbI.Mul(eqLo[z], t))
			if lane == 0 && ((KeccakRC[23]>>z)&1) != 0 {
				c2RHSTotal.AddAssign(wbI)
			}
			m := bitOff / 3
			c2SibDot[m>>6][bitOff%3].AddAssign(wbI.Mul(eqLo[m&63], t))
		}
	}
	c2Stride := pw

	var c2MuxScale [3]E
	var c2CurScale [6]E
	var c2SibScale [2]E
	var c2RHSScale E
	for inputB := 0; inputB < 2; inputB++ {
		dirBase := OffDir0
		if inputB == 1 {
			dirBase = OffDir1
		}
		for lvl := 0; lvl < MerkleDepth; lvl++ {
			lvlW := w
			w = w.Mul(c2Stride, t)
			rel := inputB*MerkleDepth + lvl
			srcPerm := 8 + rel - 1
			if lvl == 0 {
				if inputB == 0 {
					srcPerm = 1
				} else {
					srcPerm = 4
				}
			}

			c2RHSScale.AddAssign(lvlW)
			dx, dr := privLoc(dirBase + lvl)
			bLo[dx][dr&63].AddAssign(lvlW.Mul(c2DirSum, t).Mul(eqHi[dr>>6], t))

			muxHi0 := muxRow(inputB, lvl, 0) >> 6
			for blk := 0; blk < 3; blk++ {
				c2MuxScale[blk].AddAssign(lvlW.Mul(eqHi[muxHi0+blk], t))
			}
			srcHiY0 := keccakRow(srcPerm, 23, 0, 0) >> 6
			srcHiY1 := keccakRow(srcPerm, 23, 1, 0) >> 6
			sY0 := lvlW.Mul(eqHi[srcHiY0], t)
			sY1 := lvlW.Mul(eqHi[srcHiY1], t)
			for lane := 0; lane < 5; lane++ {
				c2CurScale[lane].AddAssign(sY0)
			}
			c2CurScale[5].AddAssign(sY1)

			sibHi0 := (RowSibBase + rel*SibRowsPerLevel) >> 6
			for blk := 0; blk < 2; blk++ {
				c2SibScale[blk].AddAssign(lvlW.Mul(eqHi[sibHi0+blk], t))
			}
		}
	}
	pubRHS.AddAssign(c2RHSScale.Mul(c2RHSTotal, t))
	for blk := 0; blk < 3; blk++ {
		for x := 0; x < 4; x++ {
			wbEval[x].AddAssign(c2MuxScale[blk].Mul(c2MuxDot[blk][x], t))
		}
	}
	for lane := 0; lane < 6; lane++ {
		x := lane % 5
		tc := c2CurScale[lane].Mul(c2CurDot[lane], t)
		wbEval[x].AddAssign(tc)
		wcEval[x].AddAssign(tc)
	}
	for blk := 0; blk < 2; blk++ {
		for x := 0; x < 3; x++ {
			wbEval[x].AddAssign(c2SibScale[blk].Mul(c2SibDot[blk][x], t))
		}
	}

	// Group C3, C4, C5: zero heap allocations!
	// C3: is_dummy value zero-gating (384 constraints)
	for inputB := 0; inputB < 2; inputB++ {
		dOff := OffIsDummy0
		vBase := OffVIn0
		if inputB == 1 {
			dOff = OffIsDummy1
			vBase = OffVIn1
		}
		dx, dr := privLoc(dOff)
		for k := 0; k < 32; k++ {
			rDv := dummyValRow(inputB, k)
			vx0, vr0 := privLoc(vBase + 2*k)
			vx1, vr1 := privLoc(vBase + 2*k + 1)
			addLin(0, rDv); addLin(dx, dr); pubRHS.AddAssign(w); w = w.Mul(alpha, t)
			addLin(1, rDv); addLin(vx0, vr0); w = w.Mul(alpha, t)
			addQuad(4, rDv); w = w.Mul(alpha, t)
			addLin(2, rDv); addLin(dx, dr); pubRHS.AddAssign(w); w = w.Mul(alpha, t)
			addLin(3, rDv); addLin(vx1, vr1); w = w.Mul(alpha, t)
			addQuad(1, rDv); w = w.Mul(alpha, t)
		}
	}
	// C4: is_dummy anchor root-gating (2,304 constraints) — factor eqHi per 64-row block!
	for inputB := 0; inputB < 2; inputB++ {
		dOff := OffIsDummy0
		if inputB == 1 {
			dOff = OffIsDummy1
		}
		dx, dr := privLoc(dOff)
		rootPerm := 8 + inputB*MerkleDepth + (MerkleDepth - 1)
		eqY0 := eqHi[keccakRow(rootPerm, 23, 0, 0)>>6]
		eqY1 := eqHi[keccakRow(rootPerm, 23, 1, 0)>>6]
		var dAcc E
		var rpY0 [5]E
		var rpY1 E
		for blk := 0; blk < 3; blk++ {
			eqBlk := eqHi[dummyRootRow(inputB, blk*64)>>6]
			var b0, b1, b2, b3, c4, c1 E
			for j := 0; j < 64; j++ {
				k := blk*64 + j
				eqJ := eqLo[j]
				// bit 0
				b0.AddAssign(w.Mul(eqJ, t)); dAcc.AddAssign(w); w = w.Mul(alpha, t)
				b1.AddAssign(w.Mul(eqJ, t))
				{
					bitIdx := 2 * k
					lane := bitIdx / 64
					x, y, z := lane%5, lane/5, bitIdx%64
					we := w.Mul(eqLo[z], t)
					if y == 0 {
						rpY0[x].AddAssign(we)
					} else {
						rpY1.AddAssign(we)
					}
					var rc0 uint8
					if x == 0 && y == 0 {
						rc0 = uint8((KeccakRC[23] >> z) & 1)
					}
					if (getByteBit(stmt.Anchor[:], bitIdx) ^ rc0) != 0 {
						pubRHS.AddAssign(w)
					}
				}
				w = w.Mul(alpha, t)
				c4.AddAssign(w.Mul(eqJ, t)); w = w.Mul(alpha, t)
				// bit 1
				b2.AddAssign(w.Mul(eqJ, t)); dAcc.AddAssign(w); w = w.Mul(alpha, t)
				b3.AddAssign(w.Mul(eqJ, t))
				{
					bitIdx := 2*k + 1
					lane := bitIdx / 64
					x, y, z := lane%5, lane/5, bitIdx%64
					we := w.Mul(eqLo[z], t)
					if y == 0 {
						rpY0[x].AddAssign(we)
					} else {
						rpY1.AddAssign(we)
					}
					var rc1 uint8
					if x == 0 && y == 0 {
						rc1 = uint8((KeccakRC[23] >> z) & 1)
					}
					if (getByteBit(stmt.Anchor[:], bitIdx) ^ rc1) != 0 {
						pubRHS.AddAssign(w)
					}
				}
				w = w.Mul(alpha, t)
				c1.AddAssign(w.Mul(eqJ, t)); w = w.Mul(alpha, t)
			}
			wbEval[0].AddAssign(b0.Mul(eqBlk, t))
			wbEval[1].AddAssign(b1.Mul(eqBlk, t))
			wbEval[2].AddAssign(b2.Mul(eqBlk, t))
			wbEval[3].AddAssign(b3.Mul(eqBlk, t))
			wcEval[4].AddAssign(c4.Mul(eqBlk, t))
			wcEval[1].AddAssign(c1.Mul(eqBlk, t))
		}
		bLo[dx][dr&63].AddAssign(dAcc.Mul(eqHi[dr>>6], t))
		for x := 0; x < 5; x++ {
			tc := rpY0[x].Mul(eqY0, t)
			wbEval[x].AddAssign(tc)
			wcEval[x].AddAssign(tc)
		}
		tc1 := rpY1.Mul(eqY1, t)
		wbEval[0].AddAssign(tc1)
		wcEval[0].AddAssign(tc1)
	}
	// C5: 66-bit exact ripple-carry conservation adders (1,360 constraints)
	pubExit := stmt.pubExit()
	evalAdder := func(base, bits int) {
		addLin(2, base); pubRHS.AddAssign(w); w = w.Mul(alpha, t)
		for i := 0; i < bits; i++ {
			row := base + i
			addLin(0, row); addLin(1, row); addLin(3, row); pubRHS.AddAssign(w); w = w.Mul(alpha, t)
			addLin(4, row); addQuad(4, row); addQuad(1, row); w = w.Mul(alpha, t)
			if i > 0 {
				addLin(2, row); addLin(4, row-1); pubRHS.AddAssign(w); w = w.Mul(alpha, t)
			}
		}
	}
	evalAdder(RowAdderLHS1, 64)
	evalAdder(RowAdderLHS, 65)
	evalAdder(RowAdderMID, 64)
	evalAdder(RowAdderRHS, 65)
	for i := 0; i < 64; i++ {
		xa0, ra0 := privLoc(OffVIn0 + i)
		xb0, rb0 := privLoc(OffVIn1 + i)
		rl1 := RowAdderLHS1 + i
		addLin(0, rl1); addLin(xa0, ra0); pubRHS.AddAssign(w); w = w.Mul(alpha, t)
		addLin(1, rl1); addLin(xb0, rb0); w = w.Mul(alpha, t)
		xa1, ra1 := privLoc(OffVOut0 + i)
		xb1, rb1 := privLoc(OffVOut1 + i)
		rm := RowAdderMID + i
		addLin(0, rm); addLin(xa1, ra1); pubRHS.AddAssign(w); w = w.Mul(alpha, t)
		addLin(1, rm); addLin(xb1, rb1); w = w.Mul(alpha, t)
	}
	for i := 0; i < 65; i++ {
		rl := RowAdderLHS + i
		addLin(1, rl)
		if i < 64 && ((stmt.VMint>>i)&1) != 0 {
			pubRHS.AddAssign(w)
		}
		w = w.Mul(alpha, t)
		if i < 64 {
			rl1 := RowAdderLHS1 + i
			addLin(0, rl); addLin(3, rl1); addLin(2, rl1); w = w.Mul(alpha, t)
		} else {
			rl1_63 := RowAdderLHS1 + 63
			addLin(0, rl); addLin(4, rl1_63); pubRHS.AddAssign(w); w = w.Mul(alpha, t)
		}
	}
	for i := 0; i < 65; i++ {
		rr := RowAdderRHS + i
		addLin(1, rr)
		if i < 64 && ((pubExit>>i)&1) != 0 {
			pubRHS.AddAssign(w)
		}
		w = w.Mul(alpha, t)
		if i < 64 {
			rm := RowAdderMID + i
			addLin(0, rr); addLin(3, rm); addLin(2, rm); w = w.Mul(alpha, t)
		} else {
			rm63 := RowAdderMID + 63
			addLin(0, rr); addLin(4, rm63); pubRHS.AddAssign(w); w = w.Mul(alpha, t)
		}
	}
	for i := 0; i < 65; i++ {
		rl := RowAdderLHS + i
		rr := RowAdderRHS + i
		addLin(3, rl); addLin(2, rl); addLin(3, rr); addLin(2, rr); w = w.Mul(alpha, t)
	}
	addLin(4, RowAdderLHS+64); addLin(4, RowAdderRHS+64); w = w.Mul(alpha, t)
	_ = w

	for x := 0; x < 5; x++ {
		for z := 0; z < 64; z++ {
			if !bLo[x][z].IsZero() {
				wbEval[x].AddAssign(bLo[x][z].Mul(eqLo[z], t))
			}
			if !cLo[x][z].IsZero() {
				wcEval[x].AddAssign(cLo[x][z].Mul(eqLo[z], t))
			}
		}
	}
	return wbEval, wcEval, pubRHS
}
