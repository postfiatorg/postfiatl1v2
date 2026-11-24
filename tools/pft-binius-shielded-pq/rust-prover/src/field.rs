//! Binary field tower GF(2) ⊂ K_16 = GF(2^16) ⊂ K = GF(2^32) ⊂ E = GF(2^384)
//! and Lin-Chung-Han Additive NTT in the Novel Polynomial Basis.
//!
//! - K_16 = GF(2)[x] / (x^16 + x^5 + x^3 + x + 1)         (0x1002B, primitive generator g = 3)
//! - K    = K_16[Z]  / (Z^2 + Z + x^11)                    (c = 0x0800, Tr_{K_16/GF(2)}(c) = 1)
//! - E    = K[Y]     / (Y^12 + Y^3 + Y + (Z + x))          (alpha = 0x00010002, 48 bytes per element)
//! - Disjoint affine subspaces inside K = GF(2^32):
//!     H = V_9  = {0..511}                                 (|H| = 512)
//!     L = 0x00200000 + V_21                               (|L| = 2,097,152, H ∩ L = ∅)
//!
//! Constant-time PCLMULQDQ:
//!   All secret-dependent prover arithmetic (K_32 multiplies, H-domain inverse ANTT,
//!   streaming 1024-point L-domain coset ANTT, and 54-PCLMULQDQ E = GF(2^384) Karatsuba)
//!   dispatches to branchless zero-lookup x86_64 PCLMULQDQ instructions when available.

use std::sync::OnceLock;

pub const POLY16: u32 = 0x1002B;
pub const GEN16: u16 = 3;
pub const C32: u16 = 0x0800;          // Z^2 + Z + 0x0800 is irreducible over GF(2^16)
pub const E_ALPHA: u32 = 0x0001_0002; // Y^12 + Y^3 + Y + 0x00010002 is irreducible over GF(2^32)
pub const L_SHIFT: u32 = 1u32 << 21;  // 0x00200000, disjoint from V_21 and H = V_9
pub const H_LOG: usize = 9;
pub const H_SIZE: usize = 1 << H_LOG; // 512

pub struct KTables {
    /// Compact L2-cache-resident GF(2^16) antilog/log tables (portable fallback).
    pub exp16: Box<[u16; 131072]>,
    pub log16: Box<[u16; 65536]>,
    pub mul_c32: Box<[u16; 65536]>,
    /// Normalized subspace vanishing polynomial basis over V_21 + shift s = 1<<21:
    /// what_basis[i][j] = \hat{W}_i(1 << j) for i in 0..21, j in 0..22.
    pub what_basis: [[u32; 22]; 21],
    /// Precomputed twiddle tables for Additive NTT on H (shift=0, log_n=9).
    pub tw_h: Vec<Vec<u32>>,
    /// Prover-only twiddle tables for L = 0x00200000 + V_21 (lazily initialized on first prove,
    /// NEVER allocated by verifiers -> saves ~115 ms cold start and ~16 MiB verifier RSS!).
    tw_l: OnceLock<Vec<Vec<u32>>>,
}

static KTABLES: OnceLock<KTables> = OnceLock::new();

#[inline]
pub fn k_tables() -> &'static KTables {
    KTABLES.get_or_init(KTables::build)
}

fn mul16_slow(mut a: u32, mut b: u32) -> u16 {
    let mut r: u32 = 0;
    while b != 0 {
        if (b & 1) != 0 {
            r ^= a;
        }
        a <<= 1;
        if (a & 0x10000) != 0 {
            a ^= POLY16;
        }
        b >>= 1;
    }
    r as u16
}

impl KTables {
    fn build() -> Self {
        let mut exp16: Box<[u16; 131072]> = vec![0u16; 131072].into_boxed_slice().try_into().unwrap();
        let mut log16: Box<[u16; 65536]> = vec![0u16; 65536].into_boxed_slice().try_into().unwrap();
        let exp_ref: &mut [u16; 131072] = &mut exp16;
        let log_ref: &mut [u16; 65536] = &mut log16;
        let mut v: u16 = 1;
        for i in 0..65535usize {
            exp_ref[i] = v;
            exp_ref[i + 65535] = v;
            log_ref[v as usize] = i as u16;
            v = mul16_slow(v as u32, GEN16 as u32);
        }
        let mut mul_c32: Box<[u16; 65536]> = vec![0u16; 65536].into_boxed_slice().try_into().unwrap();
        let lc32 = log_ref[C32 as usize] as usize;
        for a in 1..65536usize {
            let la = log_ref[a] as usize;
            mul_c32[a] = exp_ref[la + lc32];
        }

        let kmul16_t = |a: u16, b: u16| -> u16 {
            if a == 0 || b == 0 {
                0
            } else {
                exp_ref[log_ref[a as usize] as usize + log_ref[b as usize] as usize]
            }
        };
        let kmul32_t = |a: u32, b: u32| -> u32 {
            let a0 = (a & 0xFFFF) as u16;
            let a1 = (a >> 16) as u16;
            let b0 = (b & 0xFFFF) as u16;
            let b1 = (b >> 16) as u16;
            let p0 = kmul16_t(a0, b0);
            let p1 = kmul16_t(a1, b1);
            let p2 = kmul16_t(a0 ^ a1, b0 ^ b1);
            let lo = p0 ^ mul_c32[p1 as usize];
            let hi = p2 ^ p0;
            (lo as u32) | ((hi as u32) << 16)
        };
        let kinv32_t = |a: u32| -> u32 {
            debug_assert!(a != 0);
            let a0 = (a & 0xFFFF) as u16;
            let a1 = (a >> 16) as u16;
            let n = kmul16_t(a0, a0) ^ kmul16_t(a0, a1) ^ mul_c32[kmul16_t(a1, a1) as usize];
            debug_assert!(n != 0);
            let n_inv = exp_ref[65535 - log_ref[n as usize] as usize];
            let lo = kmul16_t(a0 ^ a1, n_inv);
            let hi = kmul16_t(a1, n_inv);
            (lo as u32) | ((hi as u32) << 16)
        };

        let mut what_basis = [[0u32; 22]; 21];
        let mut cur = [0u32; 22];
        for j in 0..22 {
            cur[j] = 1u32 << j;
        }
        for i in 0..21 {
            let inv_vi = kinv32_t(cur[i]);
            for j in 0..22 {
                what_basis[i][j] = kmul32_t(cur[j], inv_vi);
            }
            for j in 0..22 {
                let y = what_basis[i][j];
                cur[j] = kmul32_t(y, y) ^ y;
            }
        }

        let eval_what_t = |i: usize, x: u32| -> u32 {
            let mut acc = 0u32;
            for b in i..22 {
                if ((x >> b) & 1) != 0 {
                    acc ^= what_basis[i][b];
                }
            }
            acc
        };

        let mut tw_h = Vec::with_capacity(H_LOG);
        for i in 0..H_LOG {
            let step = 1usize << i;
            let blocks = H_SIZE / (2 * step);
            let mut row = Vec::with_capacity(blocks);
            for b in 0..blocks {
                let off = (b * 2 * step) as u32;
                row.push(eval_what_t(i, off));
            }
            tw_h.push(row);
        }

        Self {
            exp16,
            log16,
            mul_c32,
            what_basis,
            tw_h,
            tw_l: OnceLock::new(),
        }
    }

    #[inline]
    pub fn tw_l(&self) -> &[Vec<u32>] {
        self.tw_l.get_or_init(|| {
            let mut tw_l = Vec::with_capacity(10);
            for i in 0..10 {
                let step = 1usize << i;
                let blocks = 2097152 / (2 * step);
                let mut row = Vec::with_capacity(blocks);
                for b in 0..blocks {
                    let off = (b * 2 * step) as u32;
                    row.push(self.eval_what(i, L_SHIFT ^ off));
                }
                tw_l.push(row);
            }
            tw_l
        })
    }

    #[inline(always)]
    pub fn kmul16(&self, a: u16, b: u16) -> u16 {
        if a == 0 || b == 0 {
            0
        } else {
            unsafe {
                let la = *self.log16.get_unchecked(a as usize) as usize;
                let lb = *self.log16.get_unchecked(b as usize) as usize;
                *self.exp16.get_unchecked(la + lb)
            }
        }
    }

    #[inline(always)]
    pub fn kmul_portable(&self, a: u32, b: u32) -> u32 {
        if a == 0 || b == 0 {
            return 0;
        }
        let a0 = (a & 0xFFFF) as u16;
        let a1 = (a >> 16) as u16;
        let b0 = (b & 0xFFFF) as u16;
        let b1 = (b >> 16) as u16;
        let p0 = self.kmul16(a0, b0);
        let p1 = self.kmul16(a1, b1);
        let p2 = self.kmul16(a0 ^ a1, b0 ^ b1);
        let h = (p1 >> 5) as u32;
        let lo = p0 ^ (((p1 as u32) << 11) ^ h ^ (h << 1) ^ (h << 3) ^ (h << 5)) as u16;
        let hi = p2 ^ p0;
        (lo as u32) | ((hi as u32) << 16)
    }

    /// Branchless constant-time K_32 multiply via PCLMULQDQ (x86_64) or PMULL (aarch64).
    #[inline(always)]
    pub fn kmul(&self, a: u32, b: u32) -> u32 {
        #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
        {
            if std::arch::is_x86_feature_detected!("pclmulqdq") {
                return unsafe { kmul32_pclmul(a, b) };
            }
        }
        #[cfg(all(target_arch = "aarch64", not(feature = "portable")))]
        {
            if std::arch::is_aarch64_feature_detected!("pmull") {
                return unsafe { kmul32_pmull(a, b) };
            }
        }
        self.kmul_portable(a, b)
    }

    #[inline(always)]
    pub fn kinv(&self, a: u32) -> u32 {
        assert!(a != 0, "division by zero in GF(2^32)");
        let a0 = (a & 0xFFFF) as u16;
        let a1 = (a >> 16) as u16;
        let n = self.kmul16(a0, a0) ^ self.kmul16(a0, a1) ^ self.mul_c32[self.kmul16(a1, a1) as usize];
        let n_inv = self.exp16[65535 - self.log16[n as usize] as usize];
        let lo = self.kmul16(a0 ^ a1, n_inv);
        let hi = self.kmul16(a1, n_inv);
        (lo as u32) | ((hi as u32) << 16)
    }

    #[inline(always)]
    pub fn eval_what(&self, i: usize, x: u32) -> u32 {
        let mut acc = 0u32;
        let row = &self.what_basis[i];
        let mut rem = x >> i;
        let mut b = i;
        while rem != 0 {
            if (rem & 1) != 0 {
                acc ^= row[b];
            }
            rem >>= 1;
            b += 1;
        }
        acc
    }

    /// Inverse Additive NTT over K = GF(2^32) on H = V_9 (512 points).
    /// Uses branchless PCLMULQDQ when available on x86_64 (zero secret-dependent table lookups).
    pub fn antt_inverse_h_k(&self, a: &mut [u32]) {
        debug_assert_eq!(a.len(), H_SIZE);
        #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
        {
            if std::arch::is_x86_feature_detected!("pclmulqdq") {
                unsafe { self.antt_inverse_h_k_pclmul(a); }
                return;
            }
        }
        for i in 0..H_LOG {
            let step = 1usize << i;
            let tws = &self.tw_h[i];
            for (b, &tw) in tws.iter().enumerate() {
                let off = b * (2 * step);
                for j in off..off + step {
                    let x0 = a[j];
                    let x1 = a[j + step];
                    let v = x0 ^ x1;
                    let u = x0 ^ self.kmul_portable(tw, v);
                    a[j] = u;
                    a[j + step] = v;
                }
            }
        }
    }

    #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
    #[target_feature(enable = "pclmulqdq")]
    unsafe fn antt_inverse_h_k_pclmul(&self, a: &mut [u32]) {
        for i in 0..H_LOG {
            let step = 1usize << i;
            let tws = &self.tw_h[i];
            for (b, &tw) in tws.iter().enumerate() {
                let off = b * (2 * step);
                let tw_u64 = ((tw & 0xFFFF) as u64) | (((tw >> 16) as u64) << 32);
                let vtw = core::arch::x86_64::_mm_cvtsi64_si128(tw_u64 as i64);
                for j in off..off + step {
                    let x0 = *a.get_unchecked(j);
                    let x1 = *a.get_unchecked(j + step);
                    let v = x0 ^ x1;
                    let v_u64 = ((v & 0xFFFF) as u64) | (((v >> 16) as u64) << 32);
                    let vv = core::arch::x86_64::_mm_cvtsi64_si128(v_u64 as i64);
                    let p = core::arch::x86_64::_mm_clmulepi64_si128(vtw, vv, 0x00);
                    let mut pair = [0u64; 2];
                    core::arch::x86_64::_mm_storeu_si128(
                        pair.as_mut_ptr() as *mut core::arch::x86_64::__m128i,
                        p,
                    );
                    let p00 = pair[0] & 0x7FFF_FFFF;
                    let pmid = pair[0] >> 32;
                    let p11 = pair[1];
                    let r0 = reduce_poly16_42(p00 ^ (p11 << 11));
                    let r1 = reduce_poly16_31(pmid ^ p11);
                    let prod = (r0 as u32) | ((r1 as u32) << 16);
                    *a.get_unchecked_mut(j) = x0 ^ prod;
                    *a.get_unchecked_mut(j + step) = v;
                }
            }
        }
    }

    /// Streaming L1-cache-resident 1024-point coset Additive NTT over K = GF(2^32) on
    /// L[blk*1024 .. (blk+1)*1024] for ANY polynomial of degree < 1024 (coeffs.len() <= 1024).
    /// Bit-for-bit identical to the full 2^21 Additive NTT on L = 0x00200000 + V_21,
    /// skipping the 11 top levels (i = 20..10) where upper halves are identically zero.
    #[inline]
    pub fn antt_forward_block1024_k(&self, coeffs: &[u32], blk: usize, out: &mut [u32; 1024]) {
        debug_assert!(coeffs.len() <= 1024 && blk < 2048);
        out[..coeffs.len()].copy_from_slice(coeffs);
        out[coeffs.len()..].fill(0);
        #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
        {
            if std::arch::is_x86_feature_detected!("pclmulqdq") {
                unsafe { self.antt_forward_block1024_k_pclmul(blk, out); }
                return;
            }
        }
        let tw_l = self.tw_l();
        for i in (0..10).rev() {
            let step = 1usize << i;
            let sub_blocks = 1024 / (2 * step);
            let tws = &tw_l[i];
            let base_b = blk * sub_blocks;
            for sb in 0..sub_blocks {
                let tw = tws[base_b + sb];
                let off = sb * (2 * step);
                for j in off..off + step {
                    let u = out[j];
                    let v = out[j + step];
                    let prod = self.kmul_portable(tw, v);
                    let x0 = u ^ prod;
                    out[j] = x0;
                    out[j + step] = x0 ^ v;
                }
            }
        }
    }

    #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
    #[target_feature(enable = "pclmulqdq")]
    unsafe fn antt_forward_block1024_k_pclmul(&self, blk: usize, out: &mut [u32; 1024]) {
        let tw_l = self.tw_l();
        for i in (0..10).rev() {
            let step = 1usize << i;
            let sub_blocks = 1024 / (2 * step);
            let tws = &tw_l[i];
            let base_b = blk * sub_blocks;
            for sb in 0..sub_blocks {
                let tw = *tws.get_unchecked(base_b + sb);
                let off = sb * (2 * step);
                let tw_u64 = ((tw & 0xFFFF) as u64) | (((tw >> 16) as u64) << 32);
                let vtw = core::arch::x86_64::_mm_cvtsi64_si128(tw_u64 as i64);
                for j in off..off + step {
                    let u = *out.get_unchecked(j);
                    let v = *out.get_unchecked(j + step);
                    let v_u64 = ((v & 0xFFFF) as u64) | (((v >> 16) as u64) << 32);
                    let vv = core::arch::x86_64::_mm_cvtsi64_si128(v_u64 as i64);
                    let p = core::arch::x86_64::_mm_clmulepi64_si128(vtw, vv, 0x00);
                    let mut pair = [0u64; 2];
                    core::arch::x86_64::_mm_storeu_si128(
                        pair.as_mut_ptr() as *mut core::arch::x86_64::__m128i,
                        p,
                    );
                    let p00 = pair[0] & 0x7FFF_FFFF;
                    let pmid = pair[0] >> 32;
                    let p11 = pair[1];
                    let r0 = reduce_poly16_42(p00 ^ (p11 << 11));
                    let r1 = reduce_poly16_31(pmid ^ p11);
                    let prod = (r0 as u32) | ((r1 as u32) << 16);
                    let x0 = u ^ prod;
                    *out.get_unchecked_mut(j) = x0;
                    *out.get_unchecked_mut(j + step) = x0 ^ v;
                }
            }
        }
    }

    /// Streaming L1/L2-cache-resident 1024-point coset Additive NTT over E = GF(2^384)
    /// for degree < 1024 polynomials (`B_pcs`, `B_prox`).
    #[inline]
    pub fn antt_forward_block1024_e(&self, coeffs: &[E], blk: usize, out: &mut [E; 1024]) {
        debug_assert!(coeffs.len() <= 1024 && blk < 2048);
        out[..coeffs.len()].copy_from_slice(coeffs);
        out[coeffs.len()..].fill(E::ZERO);
        #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
        {
            if std::arch::is_x86_feature_detected!("pclmulqdq") {
                unsafe { self.antt_forward_block1024_e_pclmul(blk, out); }
                return;
            }
        }
        let tw_l = self.tw_l();
        for i in (0..10).rev() {
            let step = 1usize << i;
            let sub_blocks = 1024 / (2 * step);
            let tws = &tw_l[i];
            let base_b = blk * sub_blocks;
            for sb in 0..sub_blocks {
                let tw = tws[base_b + sb];
                let off = sb * (2 * step);
                for j in off..off + step {
                    let u = out[j];
                    let v = out[j + step];
                    let x0 = u.add(v.scale_k(tw, self));
                    out[j] = x0;
                    out[j + step] = x0.add(v);
                }
            }
        }
    }

    #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
    #[target_feature(enable = "pclmulqdq")]
    unsafe fn antt_forward_block1024_e_pclmul(&self, blk: usize, out: &mut [E; 1024]) {
        let tw_l = self.tw_l();
        for i in (0..10).rev() {
            let step = 1usize << i;
            let sub_blocks = 1024 / (2 * step);
            let tws = &tw_l[i];
            let base_b = blk * sub_blocks;
            for sb in 0..sub_blocks {
                let tw = *tws.get_unchecked(base_b + sb);
                let off = sb * (2 * step);
                let tw_u64 = ((tw & 0xFFFF) as u64) | (((tw >> 16) as u64) << 32);
                let vtw = core::arch::x86_64::_mm_cvtsi64_si128(tw_u64 as i64);
                for j in off..off + step {
                    let u = *out.get_unchecked(j);
                    let v = *out.get_unchecked(j + step);
                    let mut x0 = u;
                    for limb in 0..12 {
                        let vl = v.c[limb];
                        let v_u64 = ((vl & 0xFFFF) as u64) | (((vl >> 16) as u64) << 32);
                        let vv = core::arch::x86_64::_mm_cvtsi64_si128(v_u64 as i64);
                        let p = core::arch::x86_64::_mm_clmulepi64_si128(vtw, vv, 0x00);
                        let mut pair = [0u64; 2];
                        core::arch::x86_64::_mm_storeu_si128(
                            pair.as_mut_ptr() as *mut core::arch::x86_64::__m128i,
                            p,
                        );
                        let p00 = pair[0] & 0x7FFF_FFFF;
                        let pmid = pair[0] >> 32;
                        let p11 = pair[1];
                        let r0 = reduce_poly16_42(p00 ^ (p11 << 11));
                        let r1 = reduce_poly16_31(pmid ^ p11);
                        x0.c[limb] ^= (r0 as u32) | ((r1 as u32) << 16);
                    }
                    *out.get_unchecked_mut(j) = x0;
                    *out.get_unchecked_mut(j + step) = x0.add(v);
                }
            }
        }
    }

    /// Forward Additive NTT over E = GF(2^384) on H = V_9 (512 points).
    pub fn antt_forward_h_e(&self, a: &mut [E]) {
        debug_assert_eq!(a.len(), H_SIZE);
        for i in (0..H_LOG).rev() {
            let step = 1usize << i;
            let tws = &self.tw_h[i];
            for (b, &tw) in tws.iter().enumerate() {
                let off = b * (2 * step);
                for j in off..off + step {
                    let u = a[j];
                    let v = a[j + step];
                    let x0 = u.add(v.scale_k(tw, self));
                    a[j] = x0;
                    a[j + step] = x0.add(v);
                }
            }
        }
    }

    /// Evaluate Novel Polynomial Basis vector X_0(x)..X_{len-1}(x) in K = GF(2^32).
    pub fn novel_basis_evals(&self, x: u32, len: usize, out: &mut [u32]) {
        debug_assert!(out.len() >= len);
        let mut w = [0u32; 21];
        for i in 0..21 {
            w[i] = self.eval_what(i, x);
        }
        out[0] = 1;
        for j in 1..len {
            let lsb = j.trailing_zeros() as usize;
            out[j] = self.kmul(out[j ^ (1 << lsb)], w[lsb]);
        }
    }
}

/// Element of E = GF(2^384) = K[Y] / (Y^12 + Y^3 + Y + 0x00010002).
/// Represented directly as 12 limbs in K = GF(2^32) (48 bytes).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
#[repr(C, align(16))]
pub struct E {
    pub c: [u32; 12],
}

pub const E_BYTES: usize = 48;

// Polynomial products over K before reduction by the degree-12 modulus.
#[inline]
fn product3(a: &[u32], b: &[u32], kt: &KTables) -> [u32; 5] {
    let p0 = kt.kmul_portable(a[0], b[0]);
    let p1 = kt.kmul_portable(a[1], b[1]);
    let p2 = kt.kmul_portable(a[2], b[2]);
    let cross = |i: usize, j: usize| kt.kmul_portable(a[i] ^ a[j], b[i] ^ b[j]);
    [p0, cross(0, 1) ^ p0 ^ p1, cross(0, 2) ^ p0 ^ p1 ^ p2,
        cross(1, 2) ^ p1 ^ p2, p2]
}

// Karatsuba in characteristic two. The two levels split 12 into 6 and 6
// into 3, requiring 54 base-field multiplications for dense elements.
#[inline]
fn split_product<const HALF: usize, const PART: usize, const TOTAL: usize>(
    a: &[u32], b: &[u32], kt: &KTables,
    multiply: impl Fn(&[u32], &[u32], &KTables) -> [u32; PART],
) -> [u32; TOTAL] {
    let (a0, a1) = a.split_at(HALF);
    let (b0, b1) = b.split_at(HALF);
    let sum_a: [u32; HALF] = core::array::from_fn(|i| a0[i] ^ a1[i]);
    let sum_b: [u32; HALF] = core::array::from_fn(|i| b0[i] ^ b1[i]);
    let low = multiply(a0, b0, kt);
    let high = multiply(a1, b1, kt);
    let middle = multiply(&sum_a, &sum_b, kt);
    let mut out = [0; TOTAL];
    for i in 0..PART {
        out[i] ^= low[i];
        out[HALF + i] ^= middle[i] ^ low[i] ^ high[i];
        out[2 * HALF + i] ^= high[i];
    }
    out
}

impl E {
    pub const ZERO: E = E { c: [0u32; 12] };

    #[inline(always)]
    pub const fn one() -> Self {
        let mut c = [0u32; 12];
        c[0] = 1;
        Self { c }
    }

    #[inline(always)]
    pub const fn from_k(k: u32) -> Self {
        let mut c = [0u32; 12];
        c[0] = k;
        Self { c }
    }

    #[inline(always)]
    pub const fn from_basis_vu(v: usize, u: usize) -> Self {
        let mut c = [0u32; 12];
        c[v] = 1u32 << u;
        Self { c }
    }

    #[inline(always)]
    pub fn is_zero(&self) -> bool {
        self.c == [0u32; 12]
    }

    #[inline(always)]
    pub fn add(self, rhs: E) -> E {
        let mut out = [0u32; 12];
        for i in 0..12 {
            out[i] = self.c[i] ^ rhs.c[i];
        }
        E { c: out }
    }

    #[inline(always)]
    pub fn add_assign(&mut self, rhs: E) {
        for i in 0..12 {
            self.c[i] ^= rhs.c[i];
        }
    }

    #[inline(always)]
    pub fn scale_k(self, k: u32, kt: &KTables) -> E {
        if k == 0 {
            return E::ZERO;
        }
        if k == 1 {
            return self;
        }
        #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
        {
            if std::arch::is_x86_feature_detected!("pclmulqdq") {
                return unsafe { self.scale_k_pclmul(k) };
            }
        }
        #[cfg(all(target_arch = "aarch64", not(feature = "portable")))]
        {
            if std::arch::is_aarch64_feature_detected!("pmull") {
                return unsafe { self.scale_k_pmull(k) };
            }
        }
        let b0 = (k & 0xFFFF) as u16;
        let b1 = (k >> 16) as u16;
        let b01 = b0 ^ b1;
        let lb0 = unsafe { *kt.log16.get_unchecked(b0 as usize) } as usize;
        let lb1 = unsafe { *kt.log16.get_unchecked(b1 as usize) } as usize;
        let lb01 = unsafe { *kt.log16.get_unchecked(b01 as usize) } as usize;
        let mut out = [0u32; 12];
        for i in 0..12 {
            let a = self.c[i];
            if a == 0 {
                continue;
            }
            let a0 = (a & 0xFFFF) as u16;
            let a1 = (a >> 16) as u16;
            let a01 = a0 ^ a1;
            let p0 = if a0 != 0 && b0 != 0 {
                unsafe { *kt.exp16.get_unchecked(*kt.log16.get_unchecked(a0 as usize) as usize + lb0) }
            } else {
                0
            };
            let p1 = if a1 != 0 && b1 != 0 {
                unsafe { *kt.exp16.get_unchecked(*kt.log16.get_unchecked(a1 as usize) as usize + lb1) }
            } else {
                0
            };
            let p2 = if a01 != 0 && b01 != 0 {
                unsafe { *kt.exp16.get_unchecked(*kt.log16.get_unchecked(a01 as usize) as usize + lb01) }
            } else {
                0
            };
            let h = (p1 >> 5) as u32;
            let lo = p0 ^ (((p1 as u32) << 11) ^ h ^ (h << 1) ^ (h << 3) ^ (h << 5)) as u16;
            let hi = p2 ^ p0;
            out[i] = (lo as u32) | ((hi as u32) << 16);
        }
        E { c: out }
    }

    #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
    #[target_feature(enable = "pclmulqdq")]
    unsafe fn scale_k_pclmul(self, k: u32) -> E {
        let k_u64 = ((k & 0xFFFF) as u64) | (((k >> 16) as u64) << 32);
        let vk = core::arch::x86_64::_mm_cvtsi64_si128(k_u64 as i64);
        let mut out = [0u32; 12];
        for i in 0..12 {
            let a = self.c[i];
            let a_u64 = ((a & 0xFFFF) as u64) | (((a >> 16) as u64) << 32);
            let va = core::arch::x86_64::_mm_cvtsi64_si128(a_u64 as i64);
            let p = core::arch::x86_64::_mm_clmulepi64_si128(vk, va, 0x00);
            let mut pair = [0u64; 2];
            core::arch::x86_64::_mm_storeu_si128(
                pair.as_mut_ptr() as *mut core::arch::x86_64::__m128i,
                p,
            );
            let p00 = pair[0] & 0x7FFF_FFFF;
            let pmid = pair[0] >> 32;
            let p11 = pair[1];
            let r0 = reduce_poly16_42(p00 ^ (p11 << 11));
            let r1 = reduce_poly16_31(pmid ^ p11);
            out[i] = (r0 as u32) | ((r1 as u32) << 16);
        }
        E { c: out }
    }

    #[inline(always)]
    pub fn add_mul_k(&mut self, k: u32, rhs: E, kt: &KTables) {
        self.add_assign(rhs.scale_k(k, kt));
    }

    /// Multiply two elements in E = GF(2^384) = K[Y] / (Y^12 + Y^3 + Y + 0x00010002).
    /// Dispatches to 54-PCLMULQDQ branchless zero-lookup Karatsuba on x86_64.
    #[inline(always)]
    pub fn mul(self, rhs: E, kt: &KTables) -> E {
        #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
        {
            if std::arch::is_x86_feature_detected!("pclmulqdq")
                && std::arch::is_x86_feature_detected!("sse4.1")
            {
                return unsafe { self.mul_pclmul(rhs) };
            }
        }
        #[cfg(all(target_arch = "aarch64", not(feature = "portable")))]
        {
            if std::arch::is_aarch64_feature_detected!("pmull") {
                return unsafe { self.mul_pmull(rhs) };
            }
        }
        // Sparse basis elements remain cheaper with the reference loop.
        if self.c.iter().filter(|&&v| v != 0).take(3).count() < 3
            || rhs.c.iter().filter(|&&v| v != 0).take(3).count() < 3
        {
            return self.mul_portable(rhs, kt);
        }
        let product = split_product::<6, 11, 23>(&self.c, &rhs.c, kt, |a, b, kt| {
            split_product::<3, 5, 11>(a, b, kt, product3)
        });
        Self::reduce_product(product)
    }

    /// Portable schoolbook reference multiplication in E = GF(2^384).
    pub fn mul_portable(self, rhs: E, kt: &KTables) -> E {
        let mut prod = [0u32; 23];
        for i in 0..12 {
            let a = self.c[i];
            if a == 0 {
                continue;
            }
            for j in 0..12 {
                let b = rhs.c[j];
                if b != 0 {
                    prod[i + j] ^= kt.kmul_portable(a, b);
                }
            }
        }
        Self::reduce_product(prod)
    }

    #[inline]
    fn reduce_product(mut prod: [u32; 23]) -> E {
        for i in (12..23).rev() {
            let v = prod[i];
            if v != 0 {
                prod[i - 12 + 3] ^= v;
                prod[i - 12 + 1] ^= v;
                prod[i - 12] ^= kmul_alpha_branchless(v);
            }
        }
        let mut out = [0u32; 12];
        out.copy_from_slice(&prod[..12]);
        E { c: out }
    }

    /// Packed-lane 54-PCLMULQDQ 2x2x3 Karatsuba multiplication in E = GF(2^384).
    /// Zero table lookups, 100% branchless, bit-for-bit identical to `mul_portable`.
    /// Uses SSE4.1 `PEXTRQ` (zero store-forwarding stalls) and defers K_16 reduction until after
    /// reducing Y^12..Y^22 mod (Y^12 + Y^3 + Y + alpha) in 128-bit registers!
    #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
    #[inline]
    #[target_feature(enable = "pclmulqdq,sse4.1")]
    pub unsafe fn mul_pclmul(self, rhs: E) -> E {
        use core::arch::x86_64::{
            _mm_cvtsi128_si64, _mm_extract_epi64, _mm_setzero_si128, _mm_xor_si128,
        };
        let mut pa = [0u64; 12];
        let mut pb = [0u64; 12];
        for i in 0..12 {
            let a = self.c[i];
            let b = rhs.c[i];
            pa[i] = ((a & 0xFFFF) as u64) | (((a >> 16) as u64) << 32);
            pb[i] = ((b & 0xFFFF) as u64) | (((b >> 16) as u64) << 32);
        }
        let mut p = [_mm_setzero_si128(); 23];
        karatsuba12_clmul(&pa, &pb, &mut p);

        // Reduce i = 22..12 directly in unreduced (p00, pmid, p11) lanes!
        // Note: each p[i] has p00 in bits 0..30, pmid in bits 32..62, p11 in bits 64..94.
        // We reduce p[i] to (r0, r1) in K_32 ONLY when folding *alpha into i-12,
        // and directly XOR p[i] into p[i-12+3] and p[i-12+1] in 128-bit XMM!
        let mut prod = [0u32; 23];
        for i in (12..23).rev() {
            let v = p[i];
            p[i - 12 + 3] = _mm_xor_si128(p[i - 12 + 3], v);
            p[i - 12 + 1] = _mm_xor_si128(p[i - 12 + 1], v);
            let lo64 = _mm_cvtsi128_si64(v) as u64;
            let hi64 = _mm_extract_epi64(v, 1) as u64;
            let r0 = reduce_poly16_42((lo64 & 0x7FFF_FFFF) ^ (hi64 << 11));
            let r1 = reduce_poly16_31((lo64 >> 32) ^ hi64);
            prod[i - 12] ^= kmul_alpha_branchless((r0 as u32) | ((r1 as u32) << 16));
        }
        let mut out = [0u32; 12];
        for i in 0..12 {
            let v = p[i];
            let lo64 = _mm_cvtsi128_si64(v) as u64;
            let hi64 = _mm_extract_epi64(v, 1) as u64;
            let r0 = reduce_poly16_42((lo64 & 0x7FFF_FFFF) ^ (hi64 << 11));
            let r1 = reduce_poly16_31((lo64 >> 32) ^ hi64);
            out[i] = prod[i] ^ ((r0 as u32) | ((r1 as u32) << 16));
        }
        E { c: out }
    }

    #[inline(always)]
    pub fn sq(self, kt: &KTables) -> E {
        let mut prod = [0u32; 23];
        for i in 0..12 {
            prod[2 * i] = kt.kmul(self.c[i], self.c[i]);
        }
        for i in (12..23).rev() {
            let v = prod[i];
            prod[i - 12 + 3] ^= v;
            prod[i - 12 + 1] ^= v;
            prod[i - 12] ^= kmul_alpha_branchless(v);
        }
        let mut out = [0u32; 12];
        out.copy_from_slice(&prod[..12]);
        E { c: out }
    }

    /// Frobenius map x -> x^{2^{32*steps}} in E = GF(2^384).
    pub fn pow_q(self, steps: usize, kt: &KTables) -> E {
        let mut cur = self;
        for _ in 0..(32 * steps) {
            cur = cur.sq(kt);
        }
        cur
    }

    /// Itoh-Tsujii inversion in E / K_32 (deg 12): 6 E::mul + 11 Frobenius maps + 1 K_32::inv.
    pub fn inv(self, kt: &KTables) -> E {
        assert!(!self.is_zero(), "division by zero in GF(2^384)");
        let p2 = self.mul(self.pow_q(1, kt), kt);          // exp 0,1
        let p3 = p2.pow_q(1, kt).mul(self, kt);            // exp 0,1,2
        let p6 = p3.mul(p3.pow_q(3, kt), kt);              // exp 0..5
        let p5 = p3.pow_q(2, kt).mul(p2, kt);              // exp 0..4
        let r = p6.pow_q(6, kt).mul(p5.pow_q(1, kt), kt);  // exp 1..11
        let norm = self.mul(r, kt);                        // exp 0..11 in K_32
        debug_assert!(norm.c[1..].iter().all(|&x| x == 0));
        r.scale_k(kt.kinv(norm.c[0]), kt)
    }

    pub fn to_bytes(self) -> [u8; E_BYTES] {
        let mut b = [0u8; E_BYTES];
        for i in 0..12 {
            b[4 * i..4 * i + 4].copy_from_slice(&self.c[i].to_le_bytes());
        }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Self {
        assert_eq!(b.len(), E_BYTES);
        let mut c = [0u32; 12];
        for i in 0..12 {
            c[i] = u32::from_le_bytes([b[4 * i], b[4 * i + 1], b[4 * i + 2], b[4 * i + 3]]);
        }
        E { c }
    }
}

#[inline(always)]
fn reduce_poly16_31(mut w: u64) -> u16 {
    let h2 = w >> 16;
    w = (w & 0xFFFF) ^ h2 ^ (h2 << 1) ^ (h2 << 3) ^ (h2 << 5);
    let h3 = w >> 16;
    ((w & 0xFFFF) ^ h3 ^ (h3 << 1) ^ (h3 << 3) ^ (h3 << 5)) as u16
}

#[cfg(all(any(target_arch = "x86_64", target_arch = "aarch64"), not(feature = "portable")))]
#[inline(always)]
fn reduce_poly16_42(mut w: u64) -> u16 {
    let h1 = w >> 32;
    w = (w & 0xFFFF_FFFF) ^ h1 ^ (h1 << 2) ^ (h1 << 6) ^ (h1 << 10);
    reduce_poly16_31(w)
}

/// ARMv8 PMULL (vmull_p64) branchless binary-field kernels for Apple Silicon and ARM64 servers/mobile.
#[cfg(all(target_arch = "aarch64", not(feature = "portable")))]
#[inline(always)]
#[target_feature(enable = "pmull")]
unsafe fn pmull64(a: u64, b: u64) -> u128 {
    core::arch::aarch64::vmull_p64(a, b)
}

#[cfg(all(target_arch = "aarch64", not(feature = "portable")))]
#[inline]
#[target_feature(enable = "pmull")]
unsafe fn kmul32_pmull(a: u32, b: u32) -> u32 {
    let a_u64 = ((a & 0xFFFF) as u64) | (((a >> 16) as u64) << 32);
    let b_u64 = ((b & 0xFFFF) as u64) | (((b >> 16) as u64) << 32);
    let p = pmull64(a_u64, b_u64);
    let lo64 = p as u64;
    let hi64 = (p >> 64) as u64;
    let r0 = reduce_poly16_42((lo64 & 0x7FFF_FFFF) ^ (hi64 << 11));
    let r1 = reduce_poly16_31((lo64 >> 32) ^ hi64);
    (r0 as u32) | ((r1 as u32) << 16)
}

#[cfg(all(target_arch = "aarch64", not(feature = "portable")))]
#[inline]
#[target_feature(enable = "pmull")]
unsafe fn karatsuba3_pmull(
    a0: u64, a1: u64, a2: u64,
    b0: u64, b1: u64, b2: u64,
    out: &mut [u128; 5],
) {
    let p0 = pmull64(a0, b0);
    let p1 = pmull64(a1, b1);
    let p2 = pmull64(a2, b2);
    let q01 = pmull64(a0 ^ a1, b0 ^ b1);
    let q02 = pmull64(a0 ^ a2, b0 ^ b2);
    let q12 = pmull64(a1 ^ a2, b1 ^ b2);
    out[0] = p0;
    out[1] = q01 ^ p0 ^ p1;
    out[2] = q02 ^ p0 ^ p1 ^ p2;
    out[3] = q12 ^ p1 ^ p2;
    out[4] = p2;
}

#[cfg(all(target_arch = "aarch64", not(feature = "portable")))]
#[inline]
#[target_feature(enable = "pmull")]
unsafe fn karatsuba6_pmull(a: &[u64; 6], b: &[u64; 6], out: &mut [u128; 11]) {
    let mut z0 = [0u128; 5];
    let mut z1 = [0u128; 5];
    let mut z2 = [0u128; 5];
    karatsuba3_pmull(a[0], a[1], a[2], b[0], b[1], b[2], &mut z0);
    karatsuba3_pmull(a[3], a[4], a[5], b[3], b[4], b[5], &mut z2);
    karatsuba3_pmull(
        a[0] ^ a[3], a[1] ^ a[4], a[2] ^ a[5],
        b[0] ^ b[3], b[1] ^ b[4], b[2] ^ b[5],
        &mut z1,
    );
    out[0] = z0[0];
    out[1] = z0[1];
    out[2] = z0[2];
    out[3] = z0[3] ^ z1[0] ^ z0[0] ^ z2[0];
    out[4] = z0[4] ^ z1[1] ^ z0[1] ^ z2[1];
    out[5] = z1[2] ^ z0[2] ^ z2[2];
    out[6] = z2[0] ^ z1[3] ^ z0[3] ^ z2[3];
    out[7] = z2[1] ^ z1[4] ^ z0[4] ^ z2[4];
    out[8] = z2[2];
    out[9] = z2[3];
    out[10] = z2[4];
}

#[cfg(all(target_arch = "aarch64", not(feature = "portable")))]
#[inline]
#[target_feature(enable = "pmull")]
unsafe fn karatsuba12_pmull(a: &[u64; 12], b: &[u64; 12], out: &mut [u128; 23]) {
    let mut a_lo = [0u64; 6];
    let mut a_hi = [0u64; 6];
    let mut a_sum = [0u64; 6];
    let mut b_lo = [0u64; 6];
    let mut b_hi = [0u64; 6];
    let mut b_sum = [0u64; 6];
    for i in 0..6 {
        a_lo[i] = a[i];
        a_hi[i] = a[i + 6];
        a_sum[i] = a[i] ^ a[i + 6];
        b_lo[i] = b[i];
        b_hi[i] = b[i + 6];
        b_sum[i] = b[i] ^ b[i + 6];
    }
    let mut z0 = [0u128; 11];
    let mut z1 = [0u128; 11];
    let mut z2 = [0u128; 11];
    karatsuba6_pmull(&a_lo, &b_lo, &mut z0);
    karatsuba6_pmull(&a_hi, &b_hi, &mut z2);
    karatsuba6_pmull(&a_sum, &b_sum, &mut z1);
    for i in 0..6 {
        out[i] = z0[i];
    }
    for i in 0..5 {
        out[6 + i] = z0[6 + i] ^ z1[i] ^ z0[i] ^ z2[i];
    }
    out[11] = z1[5] ^ z0[5] ^ z2[5];
    for i in 0..5 {
        out[12 + i] = z2[i] ^ z1[6 + i] ^ z0[6 + i] ^ z2[6 + i];
    }
    for i in 0..6 {
        out[17 + i] = z2[5 + i];
    }
}

#[cfg(all(target_arch = "aarch64", not(feature = "portable")))]
impl E {
    #[target_feature(enable = "pmull")]
    unsafe fn scale_k_pmull(self, k: u32) -> E {
        let k_u64 = ((k & 0xFFFF) as u64) | (((k >> 16) as u64) << 32);
        let mut out = [0u32; 12];
        for i in 0..12 {
            let a = self.c[i];
            let a_u64 = ((a & 0xFFFF) as u64) | (((a >> 16) as u64) << 32);
            let p = pmull64(k_u64, a_u64);
            let lo64 = p as u64;
            let hi64 = (p >> 64) as u64;
            let r0 = reduce_poly16_42((lo64 & 0x7FFF_FFFF) ^ (hi64 << 11));
            let r1 = reduce_poly16_31((lo64 >> 32) ^ hi64);
            out[i] = (r0 as u32) | ((r1 as u32) << 16);
        }
        E { c: out }
    }

    #[inline]
    #[target_feature(enable = "pmull")]
    pub unsafe fn mul_pmull(self, rhs: E) -> E {
        let mut pa = [0u64; 12];
        let mut pb = [0u64; 12];
        for i in 0..12 {
            let a = self.c[i];
            let b = rhs.c[i];
            pa[i] = ((a & 0xFFFF) as u64) | (((a >> 16) as u64) << 32);
            pb[i] = ((b & 0xFFFF) as u64) | (((b >> 16) as u64) << 32);
        }
        let mut p = [0u128; 23];
        karatsuba12_pmull(&pa, &pb, &mut p);
        let mut prod = [0u32; 23];
        for i in (12..23).rev() {
            let v = p[i];
            p[i - 12 + 3] ^= v;
            p[i - 12 + 1] ^= v;
            let lo64 = v as u64;
            let hi64 = (v >> 64) as u64;
            let r0 = reduce_poly16_42((lo64 & 0x7FFF_FFFF) ^ (hi64 << 11));
            let r1 = reduce_poly16_31((lo64 >> 32) ^ hi64);
            prod[i - 12] ^= kmul_alpha_branchless((r0 as u32) | ((r1 as u32) << 16));
        }
        let mut out = [0u32; 12];
        for i in 0..12 {
            let v = p[i];
            let lo64 = v as u64;
            let hi64 = (v >> 64) as u64;
            let r0 = reduce_poly16_42((lo64 & 0x7FFF_FFFF) ^ (hi64 << 11));
            let r1 = reduce_poly16_31((lo64 >> 32) ^ hi64);
            out[i] = prod[i] ^ ((r0 as u32) | ((r1 as u32) << 16));
        }
        E { c: out }
    }
}

#[inline(always)]
fn kmul_alpha_branchless(v: u32) -> u32 {
    // Multiply (v0 + v1*Z) by alpha = (2 + Z) mod (Z^2 + Z + x^11)
    let v0 = (v & 0xFFFF) as u64;
    let v1 = (v >> 16) as u64;
    let lo = reduce_poly16_31((v0 << 1) ^ (v1 << 11));
    let hi = reduce_poly16_31(v0 ^ v1 ^ (v1 << 1));
    (lo as u32) | ((hi as u32) << 16)
}

#[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
#[inline]
#[target_feature(enable = "pclmulqdq")]
unsafe fn kmul32_pclmul(a: u32, b: u32) -> u32 {
    let a_u64 = ((a & 0xFFFF) as u64) | (((a >> 16) as u64) << 32);
    let b_u64 = ((b & 0xFFFF) as u64) | (((b >> 16) as u64) << 32);
    let p = core::arch::x86_64::_mm_clmulepi64_si128(
        core::arch::x86_64::_mm_cvtsi64_si128(a_u64 as i64),
        core::arch::x86_64::_mm_cvtsi64_si128(b_u64 as i64),
        0x00,
    );
    let mut pair = [0u64; 2];
    core::arch::x86_64::_mm_storeu_si128(
        pair.as_mut_ptr() as *mut core::arch::x86_64::__m128i,
        p,
    );
    let p00 = pair[0] & 0x7FFF_FFFF;
    let pmid = pair[0] >> 32;
    let p11 = pair[1];
    let r0 = reduce_poly16_42(p00 ^ (p11 << 11));
    let r1 = reduce_poly16_31(pmid ^ p11);
    (r0 as u32) | ((r1 as u32) << 16)
}

#[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
#[inline]
#[target_feature(enable = "pclmulqdq")]
unsafe fn clmul_pair(a: u64, b: u64) -> core::arch::x86_64::__m128i {
    use core::arch::x86_64::{_mm_clmulepi64_si128, _mm_cvtsi64_si128};
    _mm_clmulepi64_si128(_mm_cvtsi64_si128(a as i64), _mm_cvtsi64_si128(b as i64), 0x00)
}

/// 3x3 Karatsuba over packed K_32 lanes: 6 PCLMULQDQ instructions -> 5 outputs.
#[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
#[inline]
#[target_feature(enable = "pclmulqdq")]
unsafe fn karatsuba3_clmul(
    a0: u64, a1: u64, a2: u64,
    b0: u64, b1: u64, b2: u64,
    out: &mut [core::arch::x86_64::__m128i; 5],
) {
    use core::arch::x86_64::_mm_xor_si128;
    let p0 = clmul_pair(a0, b0);
    let p1 = clmul_pair(a1, b1);
    let p2 = clmul_pair(a2, b2);
    let q01 = clmul_pair(a0 ^ a1, b0 ^ b1);
    let q02 = clmul_pair(a0 ^ a2, b0 ^ b2);
    let q12 = clmul_pair(a1 ^ a2, b1 ^ b2);
    out[0] = p0;
    out[1] = _mm_xor_si128(_mm_xor_si128(q01, p0), p1);
    out[2] = _mm_xor_si128(_mm_xor_si128(q02, p0), _mm_xor_si128(p1, p2));
    out[3] = _mm_xor_si128(_mm_xor_si128(q12, p1), p2);
    out[4] = p2;
}

/// 6x6 Karatsuba: 3 x karatsuba3_clmul = 18 PCLMULQDQ instructions -> 11 outputs.
#[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
#[inline]
#[target_feature(enable = "pclmulqdq")]
unsafe fn karatsuba6_clmul(
    a: &[u64; 6],
    b: &[u64; 6],
    out: &mut [core::arch::x86_64::__m128i; 11],
) {
    use core::arch::x86_64::{_mm_setzero_si128, _mm_xor_si128};
    let mut z0 = [_mm_setzero_si128(); 5];
    let mut z1 = [_mm_setzero_si128(); 5];
    let mut z2 = [_mm_setzero_si128(); 5];
    karatsuba3_clmul(a[0], a[1], a[2], b[0], b[1], b[2], &mut z0);
    karatsuba3_clmul(a[3], a[4], a[5], b[3], b[4], b[5], &mut z2);
    karatsuba3_clmul(
        a[0] ^ a[3], a[1] ^ a[4], a[2] ^ a[5],
        b[0] ^ b[3], b[1] ^ b[4], b[2] ^ b[5],
        &mut z1,
    );
    out[0] = z0[0];
    out[1] = z0[1];
    out[2] = z0[2];
    out[3] = _mm_xor_si128(_mm_xor_si128(z0[3], z1[0]), _mm_xor_si128(z0[0], z2[0]));
    out[4] = _mm_xor_si128(_mm_xor_si128(z0[4], z1[1]), _mm_xor_si128(z0[1], z2[1]));
    out[5] = _mm_xor_si128(_mm_xor_si128(z1[2], z0[2]), z2[2]);
    out[6] = _mm_xor_si128(_mm_xor_si128(z2[0], z1[3]), _mm_xor_si128(z0[3], z2[3]));
    out[7] = _mm_xor_si128(_mm_xor_si128(z2[1], z1[4]), _mm_xor_si128(z0[4], z2[4]));
    out[8] = z2[2];
    out[9] = z2[3];
    out[10] = z2[4];
}

/// 12x12 Karatsuba: 3 x karatsuba6_clmul = 54 PCLMULQDQ instructions -> 23 outputs.
#[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
#[inline]
#[target_feature(enable = "pclmulqdq")]
unsafe fn karatsuba12_clmul(
    a: &[u64; 12],
    b: &[u64; 12],
    out: &mut [core::arch::x86_64::__m128i; 23],
) {
    use core::arch::x86_64::{_mm_setzero_si128, _mm_xor_si128};
    let mut a_lo = [0u64; 6];
    let mut a_hi = [0u64; 6];
    let mut a_sum = [0u64; 6];
    let mut b_lo = [0u64; 6];
    let mut b_hi = [0u64; 6];
    let mut b_sum = [0u64; 6];
    for i in 0..6 {
        a_lo[i] = a[i];
        a_hi[i] = a[i + 6];
        a_sum[i] = a[i] ^ a[i + 6];
        b_lo[i] = b[i];
        b_hi[i] = b[i + 6];
        b_sum[i] = b[i] ^ b[i + 6];
    }
    let mut z0 = [_mm_setzero_si128(); 11];
    let mut z1 = [_mm_setzero_si128(); 11];
    let mut z2 = [_mm_setzero_si128(); 11];
    karatsuba6_clmul(&a_lo, &b_lo, &mut z0);
    karatsuba6_clmul(&a_hi, &b_hi, &mut z2);
    karatsuba6_clmul(&a_sum, &b_sum, &mut z1);
    for i in 0..6 {
        out[i] = z0[i];
    }
    for i in 0..5 {
        out[i + 6] = _mm_xor_si128(_mm_xor_si128(z0[i + 6], z1[i]), _mm_xor_si128(z0[i], z2[i]));
    }
    out[11] = _mm_xor_si128(_mm_xor_si128(z1[5], z0[5]), z2[5]);
    for i in 0..5 {
        out[i + 12] = _mm_xor_si128(_mm_xor_si128(z2[i], z1[i + 6]), _mm_xor_si128(z0[i + 6], z2[i + 6]));
    }
    for i in 5..11 {
        out[i + 12] = z2[i];
    }
}

/// Evaluate eq(r, x) for all x in {0,1}^m with big-endian bit order.
pub fn eq_table(r: &[E], kt: &KTables) -> Vec<E> {
    eq_table_scaled_limit(r, E::one(), 1usize << r.len(), kt)
}

pub fn eq_table_scaled_limit(r: &[E], scale: E, limit: usize, kt: &KTables) -> Vec<E> {
    let mut cur = vec![scale];
    for (idx, &ri) in r.iter().enumerate() {
        let rem = r.len() - 1 - idx;
        let max_len = (limit + (1usize << rem) - 1) >> rem;
        let next_len = (cur.len() * 2).min(max_len);
        let mut next = vec![E::ZERO; next_len];
        for (i, &val) in cur.iter().enumerate() {
            let p = val.mul(ri, kt);
            if 2 * i < next_len {
                next[2 * i] = val.add(p);
            }
            if 2 * i + 1 < next_len {
                next[2 * i + 1] = p;
            }
        }
        cur = next;
    }
    cur
}

pub fn eq_eval_point(r: &[E], bits_msb: usize, kt: &KTables) -> E {
    let mut acc = E::one();
    let one = E::one();
    let n = r.len();
    for i in 0..n {
        let bit = (bits_msb >> (n - 1 - i)) & 1;
        let f = if bit == 1 { r[i] } else { one.add(r[i]) };
        acc = acc.mul(f, kt);
    }
    acc
}

/// 4-bit nibble lookup table for Ring-Switching linear map over E = GF(2^384) (384 bits = 96 nibbles, 72 KiB).
pub struct RingSwitchLinearMap {
    table: Vec<[E; 16]>, // length 96
}

impl RingSwitchLinearMap {
    pub fn new(w_rs: &[E]) -> Self {
        assert_eq!(w_rs.len(), 384);
        let mut table = vec![[E::ZERO; 16]; 96];
        for nib in 0..96 {
            let base = nib * 4;
            let e0 = w_rs[base];
            let e1 = w_rs[base + 1];
            let e2 = w_rs[base + 2];
            let e3 = w_rs[base + 3];
            let slot = &mut table[nib];
            slot[0] = E::ZERO;
            slot[1] = e0;
            slot[2] = e1;
            slot[3] = e0.add(e1);
            slot[4] = e2;
            slot[5] = e2.add(e0);
            slot[6] = e2.add(e1);
            slot[7] = e2.add(slot[3]);
            for j in 0..8 {
                slot[j + 8] = slot[j].add(e3);
            }
        }
        Self { table }
    }

    #[inline(always)]
    pub fn apply(&self, z: E) -> E {
        #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
        {
            return unsafe { self.apply_sse2(z) };
        }
        #[cfg(any(not(target_arch = "x86_64"), feature = "portable"))]
        {
            let mut acc = [0u32; 12];
            for v in 0..12 {
                let w = z.c[v];
                if w != 0 {
                    let base = v * 8;
                    for s in 0..8 {
                        let nib = ((w >> (s * 4)) & 0xF) as usize;
                        if nib != 0 {
                            let row = unsafe { &self.table.get_unchecked(base + s).get_unchecked(nib).c };
                            for i in 0..12 {
                                acc[i] ^= row[i];
                            }
                        }
                    }
                }
            }
            E { c: acc }
        }
    }

    #[cfg(all(target_arch = "x86_64", not(feature = "portable")))]
    #[inline(always)]
    unsafe fn apply_sse2(&self, z: E) -> E {
        use core::arch::x86_64::{__m128i, _mm_loadu_si128, _mm_setzero_si128, _mm_storeu_si128, _mm_xor_si128};
        let mut a0 = _mm_setzero_si128();
        let mut a1 = _mm_setzero_si128();
        let mut a2 = _mm_setzero_si128();
        let tbl = self.table.as_ptr();
        for v in 0..12 {
            let w = z.c[v];
            if w != 0 {
                let base = v * 8;
                for s in 0..8 {
                    let nib = ((w >> (s * 4)) & 0xF) as usize;
                    let ptr = (*tbl.add(base + s)).as_ptr().add(nib) as *const __m128i;
                    a0 = _mm_xor_si128(a0, _mm_loadu_si128(ptr));
                    a1 = _mm_xor_si128(a1, _mm_loadu_si128(ptr.add(1)));
                    a2 = _mm_xor_si128(a2, _mm_loadu_si128(ptr.add(2)));
                }
            }
        }
        let mut out = E::ZERO;
        let dst = out.c.as_mut_ptr() as *mut __m128i;
        _mm_storeu_si128(dst, a0);
        _mm_storeu_si128(dst.add(1), a1);
        _mm_storeu_si128(dst.add(2), a2);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_field_e384_pclmul_and_streaming_antt() {
        let kt = k_tables();
        let a = E {
            c: core::array::from_fn(|i| (i as u32).wrapping_mul(0x9E37_79B9) ^ 0x5A3C_1129),
        };
        let b = E {
            c: core::array::from_fn(|i| (i as u32).wrapping_mul(0x85EB_CA6B) ^ 0xA109_33F1),
        };
        let c = E {
            c: core::array::from_fn(|i| (i as u32).wrapping_mul(0xC2B2_AE35) ^ 0x1234_5678),
        };
        assert_eq!(a.mul(b, kt), a.mul_portable(b, kt));
        assert_eq!(a.mul(b, kt), b.mul(a, kt));
        assert_eq!(a.mul(b, kt).mul(c, kt), a.mul(b.mul(c, kt), kt));
        assert_eq!(a.mul(b.add(c), kt), a.mul(b, kt).add(a.mul(c, kt)));
        assert_eq!(a.mul(a, kt), a.sq(kt));
        let a_inv = a.inv(kt);
        assert_eq!(a.mul(a_inv, kt), E::one());

        // 1000 random PCLMULQDQ vs portable equivalence checks
        let mut s = 0x1234_5678_9ABC_DEF0u64;
        let mut next_u32 = || -> u32 {
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            s as u32
        };
        for _ in 0..1000 {
            let x = E { c: core::array::from_fn(|_| next_u32()) };
            let y = E { c: core::array::from_fn(|_| next_u32()) };
            assert_eq!(x.mul(y, kt), x.mul_portable(y, kt));
        }

        // Streaming 1024-point coset ANTT on H=V_9 (512 pts) + deg-565 ZK padding -> L = 0x00200000 + V_21
        let mut w_h: Vec<u32> = (0..H_SIZE)
            .map(|i| (i as u32).wrapping_mul(0x1337_BEEF) ^ 0x4242_0017)
            .collect();
        let orig_h = w_h.clone();
        kt.antt_inverse_h_k(&mut w_h);
        let mut coeffs = vec![0u32; 565];
        coeffs[..H_SIZE].copy_from_slice(&w_h);
        for j in H_SIZE..565 {
            coeffs[j] = (j as u32).wrapping_mul(0x7331_CAFE) ^ 0x9191_0001;
        }
        let mut basis = vec![0u32; 565];
        for h in [0usize, 1, 77, 255, 511] {
            kt.novel_basis_evals(h as u32, 565, &mut basis);
            let mut acc = 0u32;
            for j in 0..565 {
                acc ^= kt.kmul(coeffs[j], basis[j]);
            }
            assert_eq!(acc, orig_h[h]);
        }
        let mut blk_out = [0u32; 1024];
        for &blk in &[0usize, 1, 73, 1024, 2047] {
            kt.antt_forward_block1024_k(&coeffs, blk, &mut blk_out);
            for &sub in &[0usize, 1, 333, 1023] {
                let idx = blk * 1024 + sub;
                let x = L_SHIFT ^ (idx as u32);
                kt.novel_basis_evals(x, 565, &mut basis);
                let mut acc = 0u32;
                for j in 0..565 {
                    acc ^= kt.kmul(coeffs[j], basis[j]);
                }
                assert_eq!(acc, blk_out[sub]);
            }
        }
    }
}
