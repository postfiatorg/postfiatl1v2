// Independent Go implementation of the binary field tower
// GF(2) ⊂ K_16 = GF(2^16) ⊂ K = GF(2^32) ⊂ E = GF(2^384)
// and Lin-Chung-Han Additive NTT on H = V_9 (512 pts) and L = 0x00200000 + V_21.
package main

import (
	"encoding/binary"
	"math/bits"
	"sync"
)

const (
	Poly16 = 0x1002B
	Gen16  = 3
	C32    = 0x0800
	LShift = uint32(1 << 21)
	HLog   = 9
	HSize  = 1 << HLog // 512
	EBytes = 48
)

type KTables struct {
	exp16     [131072]uint16
	log16     [65536]uint16
	mulC32    [65536]uint16
	whatBasis [21][22]uint32
	twH       [HLog][]uint32
}

var (
	ktOnce sync.Once
	ktInst *KTables
)

func getKTables() *KTables {
	ktOnce.Do(func() {
		ktInst = buildKTables()
	})
	return ktInst
}

func mul16Slow(a, b uint32) uint16 {
	var r uint32
	for b != 0 {
		if (b & 1) != 0 {
			r ^= a
		}
		a <<= 1
		if (a & 0x10000) != 0 {
			a ^= Poly16
		}
		b >>= 1
	}
	return uint16(r)
}

func buildKTables() *KTables {
	t := &KTables{}
	var v uint16 = 1
	for i := 0; i < 65535; i++ {
		t.exp16[i] = v
		t.exp16[i+65535] = v
		t.log16[v] = uint16(i)
		v = mul16Slow(uint32(v), Gen16)
	}
	lc32 := int(t.log16[C32])
	for a := 1; a < 65536; a++ {
		la := int(t.log16[a])
		t.mulC32[a] = t.exp16[la+lc32]
	}

	var cur [22]uint32
	for j := 0; j < 22; j++ {
		cur[j] = uint32(1) << j
	}
	for i := 0; i < 21; i++ {
		invVi := t.kinv(cur[i])
		for j := 0; j < 22; j++ {
			t.whatBasis[i][j] = t.kmul(cur[j], invVi)
		}
		for j := 0; j < 22; j++ {
			y := t.whatBasis[i][j]
			cur[j] = t.kmul(y, y) ^ y
		}
	}

	for i := 0; i < HLog; i++ {
		step := 1 << i
		blocks := HSize / (2 * step)
		row := make([]uint32, blocks)
		for b := 0; b < blocks; b++ {
			row[b] = t.evalWhat(i, uint32(b*2*step))
		}
		t.twH[i] = row
	}
	return t
}

func (t *KTables) kmul16(a, b uint16) uint16 {
	if a == 0 || b == 0 {
		return 0
	}
	return t.exp16[int(t.log16[a])+int(t.log16[b])]
}

func (t *KTables) kmul(a, b uint32) uint32 {
	if a == 0 || b == 0 {
		return 0
	}
	a0 := uint16(a & 0xFFFF)
	a1 := uint16(a >> 16)
	b0 := uint16(b & 0xFFFF)
	b1 := uint16(b >> 16)
	var p0, p1, p2 uint16
	if a0 != 0 && b0 != 0 {
		p0 = t.exp16[int(t.log16[a0])+int(t.log16[b0])]
	}
	if a1 != 0 && b1 != 0 {
		p1 = t.exp16[int(t.log16[a1])+int(t.log16[b1])]
	}
	a01 := a0 ^ a1
	b01 := b0 ^ b1
	if a01 != 0 && b01 != 0 {
		p2 = t.exp16[int(t.log16[a01])+int(t.log16[b01])]
	}
	lo := p0 ^ mulC32Inline(p1)
	hi := p2 ^ p0
	return uint32(lo) | (uint32(hi) << 16)
}

func (t *KTables) kinv(a uint32) uint32 {
	if a == 0 {
		panic("division by zero in GF(2^32)")
	}
	a0 := uint16(a & 0xFFFF)
	a1 := uint16(a >> 16)
	n := t.kmul16(a0, a0) ^ t.kmul16(a0, a1) ^ t.mulC32[t.kmul16(a1, a1)]
	nInv := t.exp16[65535-int(t.log16[n])]
	lo := t.kmul16(a0^a1, nInv)
	hi := t.kmul16(a1, nInv)
	return uint32(lo) | (uint32(hi) << 16)
}

func (t *KTables) evalWhat(i int, x uint32) uint32 {
	var acc uint32
	rem := x >> i
	b := i
	for rem != 0 {
		if (rem & 1) != 0 {
			acc ^= t.whatBasis[i][b]
		}
		rem >>= 1
		b++
	}
	return acc
}

func (t *KTables) novelBasisEvals(x uint32, length int, out []uint32) {
	var w [21]uint32
	for i := 0; i < 21; i++ {
		w[i] = t.evalWhat(i, x)
	}
	out[0] = 1
	for j := 1; j < length; j++ {
		lsb := bits.TrailingZeros(uint(j))
		out[j] = t.kmul(out[j^(1<<lsb)], w[lsb])
	}
}

func (t *KTables) anttForwardHE(a []E) {
	for i := HLog - 1; i >= 0; i-- {
		step := 1 << i
		tws := t.twH[i]
		for b, tw := range tws {
			off := b * 2 * step
			for j := off; j < off+step; j++ {
				u := a[j]
				v := a[j+step]
				x0 := u.Add(v.ScaleK(tw, t))
				a[j] = x0
				a[j+step] = x0.Add(v)
			}
		}
	}
}

// E is an element of GF(2^384) = K_32[Y] / (Y^12 + Y^3 + Y + 0x00010002).
type E struct {
	C [12]uint32
}

var (
	EZero = E{}
	EOne  = E{C: [12]uint32{1}}
)

func EFromK(k uint32) E {
	var e E
	e.C[0] = k
	return e
}

func EFromBasisVU(v, u int) E {
	var e E
	e.C[v] = uint32(1) << u
	return e
}

func EFromBytes(b []byte) E {
	var e E
	for i := 0; i < 12; i++ {
		e.C[i] = binary.LittleEndian.Uint32(b[4*i : 4*i+4])
	}
	return e
}

func (e E) ToBytes() [EBytes]byte {
	var b [EBytes]byte
	for i := 0; i < 12; i++ {
		binary.LittleEndian.PutUint32(b[4*i:4*i+4], e.C[i])
	}
	return b
}

func (e E) IsZero() bool {
	return e == EZero
}

func (e E) Add(rhs E) E {
	var out E
	for i := 0; i < 12; i++ {
		out.C[i] = e.C[i] ^ rhs.C[i]
	}
	return out
}

func (e *E) AddAssign(rhs E) {
	for i := 0; i < 12; i++ {
		e.C[i] ^= rhs.C[i]
	}
}

func (e E) ScaleK(k uint32, t *KTables) E {
	if k == 0 {
		return EZero
	}
	if k == 1 {
		return e
	}
	b0 := uint16(k & 0xFFFF)
	b1 := uint16(k >> 16)
	b01 := b0 ^ b1
	lb0 := int(t.log16[b0])
	lb1 := int(t.log16[b1])
	lb01 := int(t.log16[b01])
	var out E
	for i := 0; i < 12; i++ {
		if av := e.C[i]; av != 0 {
			a0 := uint16(av & 0xFFFF)
			a1 := uint16(av >> 16)
			a01 := a0 ^ a1
			var p0, p1, p2 uint16
			if a0 != 0 && b0 != 0 {
				p0 = t.exp16[int(t.log16[a0])+lb0]
			}
			if a1 != 0 && b1 != 0 {
				p1 = t.exp16[int(t.log16[a1])+lb1]
			}
			if a01 != 0 && b01 != 0 {
				p2 = t.exp16[int(t.log16[a01])+lb01]
			}
			out.C[i] = uint32(p0^mulC32Inline(p1)) | (uint32(p2^p0) << 16)
		}
	}
	return out
}

func (e *E) AddMulK(k uint32, rhs E, t *KTables) {
	if k == 0 {
		return
	}
	if k == 1 {
		e.AddAssign(rhs)
		return
	}
	b0 := uint16(k & 0xFFFF)
	b1 := uint16(k >> 16)
	b01 := b0 ^ b1
	lb0 := int(t.log16[b0])
	lb1 := int(t.log16[b1])
	lb01 := int(t.log16[b01])
	for i := 0; i < 12; i++ {
		if av := rhs.C[i]; av != 0 {
			a0 := uint16(av & 0xFFFF)
			a1 := uint16(av >> 16)
			a01 := a0 ^ a1
			var p0, p1, p2 uint16
			if a0 != 0 && b0 != 0 {
				p0 = t.exp16[int(t.log16[a0])+lb0]
			}
			if a1 != 0 && b1 != 0 {
				p1 = t.exp16[int(t.log16[a1])+lb1]
			}
			if a01 != 0 && b01 != 0 {
				p2 = t.exp16[int(t.log16[a01])+lb01]
			}
			e.C[i] ^= uint32(p0^mulC32Inline(p1)) | (uint32(p2^p0) << 16)
		}
	}
}

// addMulKPair computes dstA ^= k * a and dstB ^= k * b while looking up log16(k) ONCE.
func addMulKPair(k uint32, a, b, dstA, dstB *E, t *KTables) {
	if k == 0 {
		return
	}
	if k == 1 {
		for i := 0; i < 12; i++ {
			dstA.C[i] ^= a.C[i]
			dstB.C[i] ^= b.C[i]
		}
		return
	}
	b0 := uint16(k & 0xFFFF)
	b1 := uint16(k >> 16)
	b01 := b0 ^ b1
	lb0 := int(t.log16[b0])
	lb1 := int(t.log16[b1])
	lb01 := int(t.log16[b01])
	for i := 0; i < 12; i++ {
		if av := a.C[i]; av != 0 {
			a0 := uint16(av & 0xFFFF)
			a1 := uint16(av >> 16)
			a01 := a0 ^ a1
			var p0, p1, p2 uint16
			if a0 != 0 && b0 != 0 {
				p0 = t.exp16[int(t.log16[a0])+lb0]
			}
			if a1 != 0 && b1 != 0 {
				p1 = t.exp16[int(t.log16[a1])+lb1]
			}
			if a01 != 0 && b01 != 0 {
				p2 = t.exp16[int(t.log16[a01])+lb01]
			}
			dstA.C[i] ^= uint32(p0^mulC32Inline(p1)) | (uint32(p2^p0) << 16)
		}
		if bv := b.C[i]; bv != 0 {
			a0 := uint16(bv & 0xFFFF)
			a1 := uint16(bv >> 16)
			a01 := a0 ^ a1
			var p0, p1, p2 uint16
			if a0 != 0 && b0 != 0 {
				p0 = t.exp16[int(t.log16[a0])+lb0]
			}
			if a1 != 0 && b1 != 0 {
				p1 = t.exp16[int(t.log16[a1])+lb1]
			}
			if a01 != 0 && b01 != 0 {
				p2 = t.exp16[int(t.log16[a01])+lb01]
			}
			dstB.C[i] ^= uint32(p0^mulC32Inline(p1)) | (uint32(p2^p0) << 16)
		}
	}
}

func (e E) mulByX() E {
	var out E
	for i := 0; i < 12; i++ {
		v := e.C[i]
		lo := v & 0xFFFF
		hi := v >> 16
		lo = (lo << 1) ^ (uint32(-int32(lo>>15)) & Poly16)
		hi = (hi << 1) ^ (uint32(-int32(hi>>15)) & Poly16)
		out.C[i] = (lo & 0xFFFF) | ((hi & 0xFFFF) << 16)
	}
	return out
}

func (e E) mulByZ(t *KTables) E {
	var out E
	for i := 0; i < 12; i++ {
		v := e.C[i]
		a0 := uint16(v & 0xFFFF)
		a1 := uint16(v >> 16)
		out.C[i] = uint32(t.mulC32[a1]) | (uint32(a0^a1) << 16)
	}
	return out
}

func (e E) mulByY() E {
	var out E
	top := e.C[11]
	for i := 11; i >= 1; i-- {
		out.C[i] = e.C[i-1]
	}
	out.C[0] = kmulAlphaBranchless(top)
	out.C[1] ^= top
	out.C[3] ^= top
	return out
}

// mulC32Inline computes p1 * 0x0800 mod (X^16 + X^5 + X^3 + X + 1) in one step.
// Because (p1 >> 5) < 2^11, ((p1 >> 5) << 5) < 2^16, so no second reduction pass is needed.
func mulC32Inline(p1 uint16) uint16 {
	h := uint32(p1 >> 5)
	return uint16((uint32(p1) << 11) ^ h ^ (h << 1) ^ (h << 3) ^ (h << 5))
}

func reducePoly16_31(w uint64) uint16 {
	h2 := w >> 16
	w = (w & 0xFFFF) ^ h2 ^ (h2 << 1) ^ (h2 << 3) ^ (h2 << 5)
	h3 := w >> 16
	return uint16((w & 0xFFFF) ^ h3 ^ (h3 << 1) ^ (h3 << 3) ^ (h3 << 5))
}

func kmulAlphaBranchless(v uint32) uint32 {
	v0 := uint32(v & 0xFFFF)
	v1 := uint32(v >> 16)
	// v0 * X mod Poly16 + v1 * X^11 mod Poly16
	lo := uint16((v0 << 1) ^ (uint32(-int32(v0>>15)) & Poly16)) ^ mulC32Inline(uint16(v1))
	// v0 + v1 * (1 + X) mod Poly16
	hi := uint16(v0 ^ v1 ^ (v1 << 1) ^ (uint32(-int32(v1>>15)) & Poly16))
	return uint32(lo) | (uint32(hi) << 16)
}

func karatsuba3Portable(a0, a1, a2, b0, b1, b2 uint32, t *KTables) [5]uint32 {
	pa := [6]uint32{a0, a1, a2, a0 ^ a1, a0 ^ a2, a1 ^ a2}
	pb := [6]uint32{b0, b1, b2, b0 ^ b1, b0 ^ b2, b1 ^ b2}
	var pr [6]uint32
	log16 := &t.log16
	exp16 := &t.exp16
	for i := 0; i < 6; i++ {
		av := pa[i]
		bv := pb[i]
		if av == 0 || bv == 0 {
			continue
		}
		x0 := uint16(av & 0xFFFF)
		x1 := uint16(av >> 16)
		y0 := uint16(bv & 0xFFFF)
		y1 := uint16(bv >> 16)
		var p0, p1, p2 uint16
		if x0 != 0 && y0 != 0 {
			p0 = exp16[int(log16[x0])+int(log16[y0])]
		}
		if x1 != 0 && y1 != 0 {
			p1 = exp16[int(log16[x1])+int(log16[y1])]
		}
		x01 := x0 ^ x1
		y01 := y0 ^ y1
		if x01 != 0 && y01 != 0 {
			p2 = exp16[int(log16[x01])+int(log16[y01])]
		}
		h := uint32(p1 >> 5)
		lo := p0 ^ uint16((uint32(p1)<<11)^h^(h<<1)^(h<<3)^(h<<5))
		pr[i] = uint32(lo) | (uint32(p2^p0) << 16)
	}
	return [5]uint32{
		pr[0],
		pr[3] ^ pr[0] ^ pr[1],
		pr[4] ^ pr[0] ^ pr[1] ^ pr[2],
		pr[5] ^ pr[1] ^ pr[2],
		pr[2],
	}
}

func karatsuba6Portable(a, b []uint32, t *KTables) [11]uint32 {
	z0 := karatsuba3Portable(a[0], a[1], a[2], b[0], b[1], b[2], t)
	z2 := karatsuba3Portable(a[3], a[4], a[5], b[3], b[4], b[5], t)
	z1 := karatsuba3Portable(a[0]^a[3], a[1]^a[4], a[2]^a[5], b[0]^b[3], b[1]^b[4], b[2]^b[5], t)
	return [11]uint32{
		z0[0],
		z0[1],
		z0[2],
		z0[3] ^ z1[0] ^ z0[0] ^ z2[0],
		z0[4] ^ z1[1] ^ z0[1] ^ z2[1],
		z1[2] ^ z0[2] ^ z2[2],
		z2[0] ^ z1[3] ^ z0[3] ^ z2[3],
		z2[1] ^ z1[4] ^ z0[4] ^ z2[4],
		z2[2],
		z2[3],
		z2[4],
	}
}

// mulPortable uses 2-level Karatsuba (54 K_32 muls instead of 144 schoolbook muls)
// for dense elements, falling back to schoolbook when either operand has < 3 nonzero limbs.
func (e E) mulPortable(rhs E, t *KTables) E {
	na, nb := 0, 0
	for i := 0; i < 12; i++ {
		if e.C[i] != 0 {
			na++
		}
		if rhs.C[i] != 0 {
			nb++
		}
	}
	var prod [23]uint32
	if na < 3 || nb < 3 {
		for i := 0; i < 12; i++ {
			a := e.C[i]
			if a == 0 {
				continue
			}
			for j := 0; j < 12; j++ {
				b := rhs.C[j]
				if b != 0 {
					prod[i+j] ^= t.kmul(a, b)
				}
			}
		}
	} else {
		var aSum, bSum [6]uint32
		for i := 0; i < 6; i++ {
			aSum[i] = e.C[i] ^ e.C[i+6]
			bSum[i] = rhs.C[i] ^ rhs.C[i+6]
		}
		low := karatsuba6Portable(e.C[0:6], rhs.C[0:6], t)
		high := karatsuba6Portable(e.C[6:12], rhs.C[6:12], t)
		mid := karatsuba6Portable(aSum[:], bSum[:], t)
		for i := 0; i < 11; i++ {
			prod[i] ^= low[i]
			prod[6+i] ^= mid[i] ^ low[i] ^ high[i]
			prod[12+i] ^= high[i]
		}
	}
	for i := 22; i >= 12; i-- {
		v := prod[i]
		if v != 0 {
			prod[i-12+3] ^= v
			prod[i-12+1] ^= v
			prod[i-12] ^= kmulAlphaBranchless(v)
		}
	}
	var out E
	copy(out.C[:], prod[:12])
	return out
}

func (e E) Mul(rhs E, t *KTables) E {
	if usePCLMUL {
		return e.mulPCLMUL(rhs)
	}
	return e.mulPortable(rhs, t)
}

func (e E) Sq(t *KTables) E {
	var prod [23]uint32
	for i := 0; i < 12; i++ {
		if e.C[i] != 0 {
			prod[2*i] = t.kmul(e.C[i], e.C[i])
		}
	}
	for i := 22; i >= 12; i-- {
		v := prod[i]
		if v != 0 {
			prod[i-12+3] ^= v
			prod[i-12+1] ^= v
			prod[i-12] ^= kmulAlphaBranchless(v)
		}
	}
	var out E
	copy(out.C[:], prod[:12])
	return out
}

func eqTableScaledLimit(r []E, scale E, limit int, t *KTables) []E {
	cur := []E{scale}
	for idx, ri := range r {
		rem := len(r) - 1 - idx
		maxLen := (limit + (1 << rem) - 1) >> rem
		nextLen := len(cur) * 2
		if nextLen > maxLen {
			nextLen = maxLen
		}
		next := make([]E, nextLen)
		for i, val := range cur {
			p := val.Mul(ri, t)
			if 2*i < nextLen {
				next[2*i] = val.Add(p)
			}
			if 2*i+1 < nextLen {
				next[2*i+1] = p
			}
		}
		cur = next
	}
	return cur
}

func eqTable(r []E, t *KTables) []E {
	return eqTableScaledLimit(r, EOne, 1<<len(r), t)
}

func eqEvalPoint(r []E, bitsMSB int, t *KTables) E {
	acc := EOne
	n := len(r)
	for i := 0; i < n; i++ {
		bit := (bitsMSB >> (n - 1 - i)) & 1
		f := EOne.Add(r[i])
		if bit == 1 {
			f = r[i]
		}
		acc = acc.Mul(f, t)
	}
	return acc
}

type RingSwitchLinearMap struct {
	table [96][16]E
}

func newRingSwitchLinearMap(wRS []E) *RingSwitchLinearMap {
	m := &RingSwitchLinearMap{}
	for nib := 0; nib < 96; nib++ {
		base := nib * 4
		e0, e1, e2, e3 := wRS[base], wRS[base+1], wRS[base+2], wRS[base+3]
		s := &m.table[nib]
		s[0] = EZero
		s[1] = e0
		s[2] = e1
		s[3] = e0.Add(e1)
		s[4] = e2
		s[5] = e2.Add(e0)
		s[6] = e2.Add(e1)
		s[7] = e2.Add(s[3])
		for j := 0; j < 8; j++ {
			s[j+8] = s[j].Add(e3)
		}
	}
	return m
}

func (m *RingSwitchLinearMap) applyPortable(z E) E {
	var c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11 uint32
	for v := 0; v < 12; v++ {
		w := z.C[v]
		if w != 0 {
			base := v * 8
			for nib := 0; nib < 8; nib++ {
				n := (w >> (uint(nib) * 4)) & 0xF
				if n != 0 {
					p := &m.table[base+nib][n]
					c0 ^= p.C[0]
					c1 ^= p.C[1]
					c2 ^= p.C[2]
					c3 ^= p.C[3]
					c4 ^= p.C[4]
					c5 ^= p.C[5]
					c6 ^= p.C[6]
					c7 ^= p.C[7]
					c8 ^= p.C[8]
					c9 ^= p.C[9]
					c10 ^= p.C[10]
					c11 ^= p.C[11]
				}
			}
		}
	}
	return E{C: [12]uint32{c0, c1, c2, c3, c4, c5, c6, c7, c8, c9, c10, c11}}
}

func (m *RingSwitchLinearMap) apply(z E) E {
	return m.applyFast(z)
}
