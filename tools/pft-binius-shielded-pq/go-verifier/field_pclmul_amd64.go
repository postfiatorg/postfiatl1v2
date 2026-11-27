//go:build amd64 && !purego

package main

func hasPCLMULQDQ() bool

//go:noescape
func clmulVec54(a, b, lo, hi *uint64)

//go:noescape
func ringSwitchApplyASM(table *[96][16]E, z *E, out *E)

var usePCLMUL = hasPCLMULQDQ()

func (m *RingSwitchLinearMap) applyFast(z E) E {
	var out E
	ringSwitchApplyASM(&m.table, &z, &out)
	return out
}

// mulPCLMUL computes E = K_32[Y] / (Y^12 + Y^3 + Y + 0x00010002) multiplication via
// 2x2x3 Karatsuba using exactly 3 x 18 = 54 branchless PCLMULQDQ instructions.
func (e E) mulPCLMUL(rhs E) E {
	var a3 [18]uint64
	var b3 [18]uint64
	for i := 0; i < 6; i++ {
		u0 := e.C[i]
		u1 := e.C[i+6]
		v0 := rhs.C[i]
		v1 := rhs.C[i+6]
		x0 := uint64(u0&0xFFFF) | (uint64(u0>>16) << 32)
		x1 := uint64(u1&0xFFFF) | (uint64(u1>>16) << 32)
		y0 := uint64(v0&0xFFFF) | (uint64(v0>>16) << 32)
		y1 := uint64(v1&0xFFFF) | (uint64(v1>>16) << 32)
		a3[i] = x0
		a3[i+6] = x1
		a3[i+12] = x0 ^ x1
		b3[i] = y0
		b3[i+6] = y1
		b3[i+12] = y0 ^ y1
	}

	var ua, ub, plo, phi [54]uint64
	for g := 0; g < 3; g++ {
		ap := g * 6
		goff := g * 18
		for sub := 0; sub < 3; sub++ {
			var a0, a1, a2, b0, b1, b2 uint64
			if sub == 0 {
				a0, a1, a2 = a3[ap], a3[ap+1], a3[ap+2]
				b0, b1, b2 = b3[ap], b3[ap+1], b3[ap+2]
			} else if sub == 1 {
				a0, a1, a2 = a3[ap+3], a3[ap+4], a3[ap+5]
				b0, b1, b2 = b3[ap+3], b3[ap+4], b3[ap+5]
			} else {
				a0, a1, a2 = a3[ap]^a3[ap+3], a3[ap+1]^a3[ap+4], a3[ap+2]^a3[ap+5]
				b0, b1, b2 = b3[ap]^b3[ap+3], b3[ap+1]^b3[ap+4], b3[ap+2]^b3[ap+5]
			}
			o := goff + sub*6
			ua[o], ub[o] = a0, b0
			ua[o+1], ub[o+1] = a1, b1
			ua[o+2], ub[o+2] = a2, b2
			ua[o+3], ub[o+3] = a0^a1, b0^b1
			ua[o+4], ub[o+4] = a0^a2, b0^b2
			ua[o+5], ub[o+5] = a1^a2, b1^b2
		}
	}
	clmulVec54(&ua[0], &ub[0], &plo[0], &phi[0])

	var gLo, gHi [3][11]uint64
	for g := 0; g < 3; g++ {
		goff := g * 18
		for pass := 0; pass < 2; pass++ {
			p := plo[goff : goff+18]
			out := &gLo[g]
			if pass == 1 {
				p = phi[goff : goff+18]
				out = &gHi[g]
			}
			// sub=0 (3x3 Karatsuba -> 5 outputs)
			z00 := p[0]
			z01 := p[3] ^ p[0] ^ p[1]
			z02 := p[4] ^ p[0] ^ p[1] ^ p[2]
			z03 := p[5] ^ p[1] ^ p[2]
			z04 := p[2]
			// sub=1
			z20 := p[6]
			z21 := p[9] ^ p[6] ^ p[7]
			z22 := p[10] ^ p[6] ^ p[7] ^ p[8]
			z23 := p[11] ^ p[7] ^ p[8]
			z24 := p[8]
			// sub=2
			z10 := p[12]
			z11 := p[15] ^ p[12] ^ p[13]
			z12 := p[16] ^ p[12] ^ p[13] ^ p[14]
			z13 := p[17] ^ p[13] ^ p[14]
			z14 := p[14]

			out[0] = z00
			out[1] = z01
			out[2] = z02
			out[3] = z03 ^ z10 ^ z00 ^ z20
			out[4] = z04 ^ z11 ^ z01 ^ z21
			out[5] = z12 ^ z02 ^ z22
			out[6] = z20 ^ z13 ^ z03 ^ z23
			out[7] = z21 ^ z14 ^ z04 ^ z24
			out[8] = z22
			out[9] = z23
			out[10] = z24
		}
	}

	var prod [23]uint32
	for i := 0; i < 23; i++ {
		var rl, rh uint64
		if i < 6 {
			rl, rh = gLo[0][i], gHi[0][i]
		} else if i < 11 {
			j := i - 6
			rl = gLo[0][i] ^ gLo[2][j] ^ gLo[0][j] ^ gLo[1][j]
			rh = gHi[0][i] ^ gHi[2][j] ^ gHi[0][j] ^ gHi[1][j]
		} else if i == 11 {
			rl = gLo[2][5] ^ gLo[0][5] ^ gLo[1][5]
			rh = gHi[2][5] ^ gHi[0][5] ^ gHi[1][5]
		} else if i < 17 {
			j := i - 12
			rl = gLo[1][j] ^ gLo[2][j+6] ^ gLo[0][j+6] ^ gLo[1][j+6]
			rh = gHi[1][j] ^ gHi[2][j+6] ^ gHi[0][j+6] ^ gHi[1][j+6]
		} else {
			rl, rh = gLo[1][i-12], gHi[1][i-12]
		}
		p00 := rl & 0x7FFFFFFF
		pmid := rl >> 32
		w0 := p00 ^ (rh << 11)
		h1 := w0 >> 32
		w0 = (w0 & 0xFFFFFFFF) ^ h1 ^ (h1 << 2) ^ (h1 << 6) ^ (h1 << 10)
		h2 := w0 >> 16
		w0 = (w0 & 0xFFFF) ^ h2 ^ (h2 << 1) ^ (h2 << 3) ^ (h2 << 5)
		h3 := w0 >> 16
		r0 := uint32((w0 & 0xFFFF) ^ h3 ^ (h3 << 1) ^ (h3 << 3) ^ (h3 << 5))

		w1 := pmid ^ rh
		k2 := w1 >> 16
		w1 = (w1 & 0xFFFF) ^ k2 ^ (k2 << 1) ^ (k2 << 3) ^ (k2 << 5)
		k3 := w1 >> 16
		r1 := uint32((w1 & 0xFFFF) ^ k3 ^ (k3 << 1) ^ (k3 << 3) ^ (k3 << 5))
		prod[i] = r0 | (r1 << 16)
	}

	// Reduce modulo Y^12 + Y^3 + Y + (Z + x)
	for i := 22; i >= 12; i-- {
		v := prod[i]
		d := i - 12
		prod[d+3] ^= v
		prod[d+1] ^= v
		v0 := uint64(v & 0xFFFF)
		v1 := uint64(v >> 16)
		w0 := (v0 << 1) ^ (v1 << 11)
		h2 := w0 >> 16
		w0 = (w0 & 0xFFFF) ^ h2 ^ (h2 << 1) ^ (h2 << 3) ^ (h2 << 5)
		h3 := w0 >> 16
		lo := uint32((w0 & 0xFFFF) ^ h3 ^ (h3 << 1) ^ (h3 << 3) ^ (h3 << 5))
		w1 := v0 ^ v1 ^ (v1 << 1)
		k2 := w1 >> 16
		w1 = (w1 & 0xFFFF) ^ k2 ^ (k2 << 1) ^ (k2 << 3) ^ (k2 << 5)
		k3 := w1 >> 16
		hi := uint32((w1 & 0xFFFF) ^ k3 ^ (k3 << 1) ^ (k3 << 3) ^ (k3 << 5))
		prod[d] ^= lo | (hi << 16)
	}
	var out E
	copy(out.C[:], prod[:12])
	return out
}
