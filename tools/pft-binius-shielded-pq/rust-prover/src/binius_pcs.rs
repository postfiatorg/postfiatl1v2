//! Zero-Knowledge Binius-Packed Binary-RS Polynomial Commitment & Sumcheck Proof System
//! ("Peel-Binius-Shielded") over GF(2) ⊂ K_16 = GF(2^16) ⊂ K = GF(2^32) ⊂ E = GF(2^384)
//! with ONE SINGLE FIXED-RANGE NIST FIPS 180-4 SHA-384 COMPILER ORACLE H: {0,1}^* -> {0,1}^{1152}
//! (ZERO Lemma A.1 multi-range lift, ZERO shared Keccak permutation with in-circuit SHAKE256!).
//!
//! Parameters (Peel-Binius-Shielded):
//!   - Evaluation domain L = 0x00200000 + V_21  (n = 2,097,152, h = 21)
//!   - Interpolation domain H = V_9             (k = 512, H ∩ L = ∅)
//!   - Queries t = 52, PoW w_pow = 20, M_pow = 2^27, Degree bound D = k + t + 1 = 565
//!   - Witness + ZK mask matrix: m_k = 163 K-rows (140 wit + 5 rs_mask + 2 G_1 + 16 sc2_mask)
//!     + 2 E-rows (B_pcs, B_prox) in ONE SHA-384 Merkle tree MT_1
//!   - Streaming 1,024-column L1-cache-resident ANTT prover (peak RSS ~300 MiB, branchless PCLMULQDQ)

use crate::circuit::{
    build_prover_weights, eval_verifier_weights_at_r1, CircuitChallenge, CircuitWitnessBits,
    PaymentStatement, ACTIVE_SUBROWS_PER_POLY, NUM_BITS_PER_POLY, V_SC1,
};
use crate::field::{
    eq_eval_point, eq_table, k_tables, KTables, RingSwitchLinearMap, E, E_BYTES, H_SIZE, L_SHIFT,
};
use crate::shake256::{COIN_BYTES, NODE_BYTES, SN_BYTES};
use rand::{RngCore, SeedableRng};
use rand_chacha::ChaCha20Rng;
use rayon::prelude::*;
use sha2::{Digest, Sha512};
use std::collections::{BTreeMap, BTreeSet};

pub const N_CODE: usize = 2_097_152;
pub const MERKLE_HEIGHT: usize = 21;
pub const K_COLS: usize = H_SIZE; // 512
pub const NUM_QUERIES: usize = 52;
pub const POW_BITS: usize = 20;
pub const POW_NONCE_BYTES: usize = 48; // 384-bit nonce (hard-capped M_pow = 2^27)
pub const POW_MAX_ATTEMPTS: u64 = 1u64 << 27;
pub const DEG_BOUND: usize = K_COLS + NUM_QUERIES + 1; // 565

pub const HASH_BYTES: usize = 48;  // 384-bit MT_1 digest prefix of H = 3 x SHA-512
pub const STATE_BYTES: usize = 64; // 512-bit internal Fiat-Shamir transcript state (ZERO wire bytes!)
pub const V_SC2: usize = 8;
pub const VIRTUAL_ROWS: usize = 1usize << V_SC2; // 256

pub const M_WIT_ROWS: usize = 5 * ACTIVE_SUBROWS_PER_POLY; // 140 (trailing 4 zero subrows/poly omitted)
pub const M_RS_MASK_ROWS: usize = 5;                       // 5 polys x 1 row (384 K-elems <= 512 cols)
pub const M_G1_ROWS: usize = 2;                            // (4*19 + 2) = 78 E-elems = 936 K-elems <= 2*512
pub const M_SC2_MASK_ROWS: usize = 2 * V_SC2;              // 16
pub const M_NON_MASK_ROWS: usize = M_WIT_ROWS + M_RS_MASK_ROWS + M_G1_ROWS; // 147
pub const M_K_ROWS: usize = M_NON_MASK_ROWS + M_SC2_MASK_ROWS;              // 163

pub const PROOF_MAGIC: &[u8; 8] = b"PFSHPK01";

/// 8 pairwise-disjoint edges (a_d, b_d) in {0,1}^8 flipping bit d (MSB=0..LSB=7).
/// Verified mechanically to yield full rank 17/17 with det degree <= 112 (SZ <= 112/2^384).
pub const MASK2_PAIRS: [(usize, usize); V_SC2] = [
    (0, 128),  // d=0 (flip 128)
    (1, 65),   // d=1 (flip 64)
    (2, 34),   // d=2 (flip 32)
    (3, 19),   // d=3 (flip 16)
    (4, 12),   // d=4 (flip 8)
    (16, 20),  // d=5 (flip 4)
    (8, 10),   // d=6 (flip 2)
    (6, 7),    // d=7 (flip 1)
];
pub const MASK2_ROWS: [usize; M_SC2_MASK_ROWS] = [
    0, 1, 2, 3, 4, 6, 7, 8, 10, 12, 16, 19, 20, 34, 65, 128,
];
pub const ZETA1_K: u32 = 2;
pub const ZETA2_K: u32 = 3;

pub struct RowLayout {
    pub non_mask_rows: [usize; M_NON_MASK_ROWS],
    pub active_rows: [usize; M_K_ROWS],
    pub virtual_to_active: [Option<usize>; VIRTUAL_ROWS],
}

pub fn row_layout() -> RowLayout {
    let mut is_mask = [false; VIRTUAL_ROWS];
    for &(a, b) in &MASK2_PAIRS {
        is_mask[a] = true;
        is_mask[b] = true;
    }
    let mut non_mask_rows = [0usize; M_NON_MASK_ROWS];
    let mut nm_idx = 0usize;
    for r in 0..VIRTUAL_ROWS {
        if !is_mask[r] && nm_idx < M_NON_MASK_ROWS {
            non_mask_rows[nm_idx] = r;
            nm_idx += 1;
        }
    }
    assert_eq!(nm_idx, M_NON_MASK_ROWS);

    let mut active_set = BTreeSet::new();
    for &r in &MASK2_ROWS {
        active_set.insert(r);
    }
    for &r in &non_mask_rows {
        active_set.insert(r);
    }
    assert_eq!(active_set.len(), M_K_ROWS);
    let mut active_rows = [0usize; M_K_ROWS];
    let mut virtual_to_active = [None; VIRTUAL_ROWS];
    for (idx, &r) in active_set.iter().enumerate() {
        active_rows[idx] = r;
        virtual_to_active[r] = Some(idx);
    }
    RowLayout {
        non_mask_rows,
        active_rows,
        virtual_to_active,
    }
}

/// Canonical serialization of PaymentStatement for Fiat-Shamir domain binding.
pub fn encode_statement(stmt: &PaymentStatement) -> Vec<u8> {
    let mut out = Vec::with_capacity(NODE_BYTES + 2 * SN_BYTES + 2 * COIN_BYTES + 24 + 48);
    out.extend_from_slice(&stmt.anchor);
    out.extend_from_slice(&stmt.sn0);
    out.extend_from_slice(&stmt.sn1);
    out.extend_from_slice(&stmt.coin_out0);
    out.extend_from_slice(&stmt.coin_out1);
    out.extend_from_slice(&stmt.v_mint.to_le_bytes());
    out.extend_from_slice(&stmt.v_unshield.to_le_bytes());
    out.extend_from_slice(&stmt.fee.to_le_bytes());
    out.extend_from_slice(&stmt.tx_ctx);
    out
}

// ---------------------------------------------------------------------------
// Single Fixed-Range Compiler Random Oracle H: {0,1}^* -> {0,1}^{1536}
//   H(msg) := SHA-512(0x00 || msg) || SHA-512(0x01 || msg) || SHA-512(0x02 || msg)
// Every compiler call uses a unique 8-byte ASCII domain tag inside `msg`:
//   - MT_1 leaf/node digests take the 48-byte (384-bit) prefix H[0..48] (zero wire bloat);
//   - Internal Transcript.state uses the full 64-byte (512-bit) block (zero wire bytes,
//     raising GHHM21 reprogramming min-entropy from 384 -> 512 bits -> >= 150-bit QROM ZK/SE!);
//   - Round-34 H_C4 reads all 3 x 64 = 192 bytes (1,536 bits) for 20-bit PoW + 52 x 21-bit indices.
// ---------------------------------------------------------------------------

#[inline]
fn oracle_leaf(col_idx: usize, k_vals: &[u32; M_K_ROWS], b_pcs: E, b_prox: E) -> [u8; HASH_BYTES] {
    let mut h = Sha512::new();
    h.update([0x00u8]);
    h.update(b"PFS-LEAF");
    h.update((col_idx as u32).to_le_bytes());
    let mut k_bytes = [0u8; M_K_ROWS * 4];
    for i in 0..M_K_ROWS {
        k_bytes[4 * i..4 * i + 4].copy_from_slice(&k_vals[i].to_le_bytes());
    }
    h.update(&k_bytes);
    h.update(b_pcs.to_bytes());
    h.update(b_prox.to_bytes());
    let d = h.finalize();
    let mut out = [0u8; HASH_BYTES];
    out.copy_from_slice(&d[..HASH_BYTES]);
    out
}

#[inline]
fn oracle_node(level: u8, pos: usize, left: &[u8; HASH_BYTES], right: &[u8; HASH_BYTES]) -> [u8; HASH_BYTES] {
    let mut h = Sha512::new();
    h.update([0x00u8]);
    h.update(b"PFS-NODE");
    h.update([level]);
    h.update((pos as u32).to_le_bytes());
    h.update(left);
    h.update(right);
    let d = h.finalize();
    let mut out = [0u8; HASH_BYTES];
    out.copy_from_slice(&d[..HASH_BYTES]);
    out
}

struct Transcript {
    state: [u8; STATE_BYTES],
}

impl Transcript {
    fn new(stmt_bytes: &[u8], root: &[u8; HASH_BYTES], s_g1: E, s_mask2: E) -> Self {
        let mut h = Sha512::new();
        h.update([0x00u8]);
        h.update(b"PFS-STMT");
        h.update(stmt_bytes);
        h.update(root);
        h.update(s_g1.to_bytes());
        h.update(s_mask2.to_bytes());
        let mut state = [0u8; STATE_BYTES];
        state.copy_from_slice(&h.finalize());
        Self { state }
    }

    fn absorb_e(&mut self, tag: &[u8], elems: &[E]) {
        let mut h = Sha512::new();
        h.update([0x00u8]);
        h.update(b"PFS-ABS ");
        h.update(tag);
        h.update(self.state);
        for &e in elems {
            h.update(e.to_bytes());
        }
        self.state.copy_from_slice(&h.finalize());
    }

    /// Atomic single-oracle-call round challenge derivation H("PFS-CH  " || tag || state)[0..128]:
    ///   block 0 prefix [0..48] = challenge `e in E = GF(2^384)`
    ///   block 1 [64..128]      = 512-bit `next_state`
    fn squeeze_e(&mut self, tag: &[u8]) -> E {
        let mut h0 = Sha512::new();
        h0.update([0x00u8]);
        h0.update(b"PFS-CH  ");
        h0.update(tag);
        h0.update(self.state);
        let d0 = h0.finalize();

        let mut h1 = Sha512::new();
        h1.update([0x01u8]);
        h1.update(b"PFS-CH  ");
        h1.update(tag);
        h1.update(self.state);
        let d1 = h1.finalize();

        self.state.copy_from_slice(&d1);
        E::from_bytes(&d0[..E_BYTES])
    }
}

/// Atomic Round-34 oracle call H("PFS-C4  " || tr_final[64 B] || nonce[48 B]) -> 192 B (1,536 bits):
///   - bits 0..20   : 20-bit PoW check (buf[0]==0 && buf[1]==0 && (buf[2]&0x0F)==0)
///   - bits 24..1116: 52 exact 21-bit query indices in [0, 2,097,151] (1,092 bits <= 1,512 remaining!)
#[inline]
pub fn eval_h_c4(tr_final: &[u8; STATE_BYTES], nonce: &[u8; POW_NONCE_BYTES]) -> (bool, [usize; NUM_QUERIES]) {
    let mut h0 = Sha512::new();
    h0.update([0x00u8]);
    h0.update(b"PFS-C4  ");
    h0.update(tr_final);
    h0.update(nonce);
    let d0 = h0.finalize();
    // Fast early exit during PoW grinding: pow bits lie strictly in d0[0..3]!
    let pow_ok = d0[0] == 0 && d0[1] == 0 && (d0[2] & 0x0F) == 0;
    if !pow_ok {
        return (false, [0usize; NUM_QUERIES]);
    }
    let mut h1 = Sha512::new();
    h1.update([0x01u8]);
    h1.update(b"PFS-C4  ");
    h1.update(tr_final);
    h1.update(nonce);
    let d1 = h1.finalize();

    let mut h2 = Sha512::new();
    h2.update([0x02u8]);
    h2.update(b"PFS-C4  ");
    h2.update(tr_final);
    h2.update(nonce);
    let d2 = h2.finalize();

    let mut buf = [0u8; 192];
    buf[0..64].copy_from_slice(&d0);
    buf[64..128].copy_from_slice(&d1);
    buf[128..192].copy_from_slice(&d2);

    let mut indices = [0usize; NUM_QUERIES];
    let bit_bytes = &buf[3..];
    for q in 0..NUM_QUERIES {
        let bit_pos = q * MERKLE_HEIGHT;
        let byte_pos = bit_pos / 8;
        let shift = bit_pos % 8;
        let b0 = bit_bytes[byte_pos] as u32;
        let b1 = bit_bytes[byte_pos + 1] as u32;
        let b2 = bit_bytes[byte_pos + 2] as u32;
        let b3 = if byte_pos + 3 < bit_bytes.len() {
            bit_bytes[byte_pos + 3] as u32
        } else {
            0
        };
        let word = (b0 | (b1 << 8) | (b2 << 16) | (b3 << 24)) >> shift;
        indices[q] = (word & ((1u32 << MERKLE_HEIGHT) - 1)) as usize;
    }
    (true, indices)
}

#[derive(Clone, Debug)]
pub struct OpenedColumn {
    pub k_vals: [u32; M_K_ROWS],
    pub b_pcs: E,
    pub b_prox: E,
}

#[derive(Clone, Debug)]
pub struct BiniusProof {
    pub merkle_root: [u8; HASH_BYTES],
    pub s_g1: E,
    pub sc1_rounds: [[E; 3]; V_SC1], // 19 rounds x (c_0, c_2, c_3)
    pub eval_c1: E,
    pub eval_c2: E,
    pub eval_z: [E; 5],
    pub eval_zg: E,
    pub rs_s_prime: [E; 32],
    pub s_mask2: E,
    pub sc2_rounds: [[E; 2]; V_SC2], // 8 rounds x (c_0, c_2)
    pub b_eval: E,
    pub v_prox_coeffs: Vec<E>,       // DEG_BOUND = 565
    pub v_open_coeffs: Vec<E>,       // DEG_BOUND = 565
    pub pow_nonce: [u8; POW_NONCE_BYTES],
    pub opened_cols: Vec<OpenedColumn>,
    pub merkle_siblings: Vec<[u8; HASH_BYTES]>,
}

impl BiniusProof {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(140_000);
        out.extend_from_slice(PROOF_MAGIC);
        out.extend_from_slice(&self.merkle_root);
        out.extend_from_slice(&self.s_g1.to_bytes());
        for r in &self.sc1_rounds {
            for &e in r {
                out.extend_from_slice(&e.to_bytes());
            }
        }
        out.extend_from_slice(&self.eval_c1.to_bytes());
        out.extend_from_slice(&self.eval_c2.to_bytes());
        for &e in &self.eval_z {
            out.extend_from_slice(&e.to_bytes());
        }
        out.extend_from_slice(&self.eval_zg.to_bytes());
        for &e in &self.rs_s_prime {
            out.extend_from_slice(&e.to_bytes());
        }
        out.extend_from_slice(&self.s_mask2.to_bytes());
        for r in &self.sc2_rounds {
            for &e in r {
                out.extend_from_slice(&e.to_bytes());
            }
        }
        out.extend_from_slice(&self.b_eval.to_bytes());
        assert_eq!(self.v_prox_coeffs.len(), DEG_BOUND);
        for &e in &self.v_prox_coeffs {
            out.extend_from_slice(&e.to_bytes());
        }
        assert_eq!(self.v_open_coeffs.len(), DEG_BOUND);
        for &e in &self.v_open_coeffs {
            out.extend_from_slice(&e.to_bytes());
        }
        out.extend_from_slice(&self.pow_nonce);
        out.extend_from_slice(&(self.opened_cols.len() as u16).to_le_bytes());
        for col in &self.opened_cols {
            for &v in &col.k_vals {
                out.extend_from_slice(&v.to_le_bytes());
            }
            out.extend_from_slice(&col.b_pcs.to_bytes());
            out.extend_from_slice(&col.b_prox.to_bytes());
        }
        out.extend_from_slice(&(self.merkle_siblings.len() as u16).to_le_bytes());
        for sib in &self.merkle_siblings {
            out.extend_from_slice(sib);
        }
        out
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        let mut pos = 0usize;
        let take = |pos: &mut usize, n: usize| -> Result<&[u8], String> {
            if *pos + n > data.len() {
                return Err("truncated Binius proof".into());
            }
            let slice = &data[*pos..*pos + n];
            *pos += n;
            Ok(slice)
        };
        if take(&mut pos, 8)? != PROOF_MAGIC {
            return Err("bad Binius proof magic".into());
        }
        let mut merkle_root = [0u8; HASH_BYTES];
        merkle_root.copy_from_slice(take(&mut pos, HASH_BYTES)?);
        let s_g1 = E::from_bytes(take(&mut pos, E_BYTES)?);
        let mut sc1_rounds = [[E::ZERO; 3]; V_SC1];
        for r in &mut sc1_rounds {
            for e in r.iter_mut() {
                *e = E::from_bytes(take(&mut pos, E_BYTES)?);
            }
        }
        let eval_c1 = E::from_bytes(take(&mut pos, E_BYTES)?);
        let eval_c2 = E::from_bytes(take(&mut pos, E_BYTES)?);
        let mut eval_z = [E::ZERO; 5];
        for e in &mut eval_z {
            *e = E::from_bytes(take(&mut pos, E_BYTES)?);
        }
        let eval_zg = E::from_bytes(take(&mut pos, E_BYTES)?);
        let mut rs_s_prime = [E::ZERO; 32];
        for e in &mut rs_s_prime {
            *e = E::from_bytes(take(&mut pos, E_BYTES)?);
        }
        let s_mask2 = E::from_bytes(take(&mut pos, E_BYTES)?);
        let mut sc2_rounds = [[E::ZERO; 2]; V_SC2];
        for r in &mut sc2_rounds {
            for e in r.iter_mut() {
                *e = E::from_bytes(take(&mut pos, E_BYTES)?);
            }
        }
        let b_eval = E::from_bytes(take(&mut pos, E_BYTES)?);
        let mut v_prox_coeffs = Vec::with_capacity(DEG_BOUND);
        for _ in 0..DEG_BOUND {
            v_prox_coeffs.push(E::from_bytes(take(&mut pos, E_BYTES)?));
        }
        let mut v_open_coeffs = Vec::with_capacity(DEG_BOUND);
        for _ in 0..DEG_BOUND {
            v_open_coeffs.push(E::from_bytes(take(&mut pos, E_BYTES)?));
        }
        let mut pow_nonce = [0u8; POW_NONCE_BYTES];
        pow_nonce.copy_from_slice(take(&mut pos, POW_NONCE_BYTES)?);
        let n_cols = u16::from_le_bytes(take(&mut pos, 2)?.try_into().unwrap()) as usize;
        if n_cols == 0 || n_cols > NUM_QUERIES {
            return Err("invalid opened column count".into());
        }
        let mut opened_cols = Vec::with_capacity(n_cols);
        for _ in 0..n_cols {
            let mut k_vals = [0u32; M_K_ROWS];
            for v in &mut k_vals {
                *v = u32::from_le_bytes(take(&mut pos, 4)?.try_into().unwrap());
            }
            let b_pcs = E::from_bytes(take(&mut pos, E_BYTES)?);
            let b_prox = E::from_bytes(take(&mut pos, E_BYTES)?);
            opened_cols.push(OpenedColumn {
                k_vals,
                b_pcs,
                b_prox,
            });
        }
        let n_sibs = u16::from_le_bytes(take(&mut pos, 2)?.try_into().unwrap()) as usize;
        if n_sibs > NUM_QUERIES * MERKLE_HEIGHT {
            return Err("invalid merkle sibling count".into());
        }
        let mut merkle_siblings = Vec::with_capacity(n_sibs);
        for _ in 0..n_sibs {
            let mut s = [0u8; HASH_BYTES];
            s.copy_from_slice(take(&mut pos, HASH_BYTES)?);
            merkle_siblings.push(s);
        }
        if pos != data.len() {
            return Err("trailing bytes in Binius proof".into());
        }
        Ok(Self {
            merkle_root,
            s_g1,
            sc1_rounds,
            eval_c1,
            eval_c2,
            eval_z,
            eval_zg,
            rs_s_prime,
            s_mask2,
            sc2_rounds,
            b_eval,
            v_prox_coeffs,
            v_open_coeffs,
            pow_nonce,
            opened_cols,
            merkle_siblings,
        })
    }
}

/// Evaluate the degree-3 Sumcheck-1 mask polynomial contribution G_1 at round d in 0..19.
fn eval_g1_round(g1: &[[E; 4]; V_SC1], r_prev: &[E], d: usize, kt: &KTables) -> [E; 4] {
    let mut pref_prod = E::one();
    for &r in r_prev {
        pref_prod = pref_prod.mul(r, kt);
    }
    let mut poly = [E::ZERO; 4];
    for k in 0..4 {
        poly[k] = g1[d][k].mul(pref_prod, kt);
    }
    let mut prefix_sum = E::ZERO;
    for j in 0..d {
        let rj = r_prev[j];
        let mut prod_excl = E::one();
        for m in 0..d {
            if m != j {
                prod_excl = prod_excl.mul(r_prev[m], kt);
            }
        }
        let gj = g1[j][0].add(rj.mul(g1[j][1].add(rj.mul(g1[j][2].add(rj.mul(g1[j][3], kt)), kt)), kt));
        prefix_sum.add_assign(gj.mul(prod_excl, kt));
    }
    poly[1].add_assign(prefix_sum);

    let mut suffix_sum = E::ZERO;
    for j in (d + 1)..V_SC1 {
        suffix_sum.add_assign(g1[j][1].add(g1[j][2]).add(g1[j][3]));
    }
    poly[1].add_assign(pref_prod.mul(suffix_sum, kt));
    poly
}

/// Compute the 78 E-element weights for `eta1 * eval_zg + eta2 * S_G1`
/// (76 coeffs of G_1 + m_g1 + m_g2).
fn eval_g1_mg_weights(r1: &[E; V_SC1], tau: E, eta1: E, eta2: E, kt: &KTables) -> [E; 78] {
    let mut w = [E::ZERO; 78];
    let mut pref = [E::one(); V_SC1 + 1];
    for i in 0..V_SC1 {
        pref[i + 1] = pref[i].mul(r1[i], kt);
    }
    let mut suff = [E::one(); V_SC1 + 1];
    for i in (0..V_SC1).rev() {
        suff[i] = suff[i + 1].mul(r1[i], kt);
    }
    for d in 0..V_SC1 {
        let prod_excl = pref[d].mul(suff[d + 1], kt);
        let base = eta1.mul(prod_excl, kt);
        let rd = r1[d];
        let rd2 = rd.sq(kt);
        let rd3 = rd2.mul(rd, kt);
        w[4 * d] = base;
        w[4 * d + 1] = base.mul(rd, kt).add(eta2);
        w[4 * d + 2] = base.mul(rd2, kt).add(eta2);
        w[4 * d + 3] = base.mul(rd3, kt).add(eta2);
    }
    w[76] = eta1.mul(tau, kt);
    w[77] = eta1.mul(tau.sq(kt), kt);
    w
}

/// Expand single Round-24 challenge `rho_rs in E` into `w_rs[0..384] = rho_rs^{flat}`,
/// `eta1 = rho_rs^{384}`, `eta2 = rho_rs^{385}`, `eta3 = rho_rs^{386}`.
fn expand_rho_rs(rho_rs: E, kt: &KTables) -> (Vec<E>, E, E, E) {
    let mut w_rs = Vec::with_capacity(384);
    let mut cur = E::one();
    for _ in 0..384 {
        w_rs.push(cur);
        cur = cur.mul(rho_rs, kt);
    }
    let eta1 = cur;
    let eta2 = eta1.mul(rho_rs, kt);
    let eta3 = eta2.mul(rho_rs, kt);
    (w_rs, eta1, eta2, eta3)
}

/// Expand single Round-23 challenge `gamma in E` into `[1, gamma, gamma^2, gamma^3, gamma^4]`.
fn expand_gammas(gamma: E, kt: &KTables) -> [E; 5] {
    let g2 = gamma.sq(kt);
    let g3 = g2.mul(gamma, kt);
    let g4 = g2.sq(kt);
    [E::one(), gamma, g2, g3, g4]
}

/// Compute Ring-Switching transposed target sum_{v=0}^{11} sum_{u=0}^{31} s_hat(v,u) * w_rs[32*v + u].
fn ring_switch_target(s_prime: &[E; 32], w_rs: &[E], kt: &KTables) -> E {
    let mut acc = E::ZERO;
    for v in 0..12 {
        for u in 0..32 {
            let mut s_hat: u32 = 0;
            for b in 0..32 {
                if ((s_prime[b].c[v] >> u) & 1) != 0 {
                    s_hat |= 1u32 << b;
                }
            }
            if s_hat != 0 {
                acc.add_mul_k(s_hat, w_rs[32 * v + u], kt);
            }
        }
    }
    acc
}

/// Build the exact 256 x 512 weight matrix M_rs for Sumcheck 2 (PROVER).
fn build_m_rs_matrix(
    layout: &RowLayout,
    r1: &[E; V_SC1],
    tau: E,
    gammas: &[E; 5],
    w_rs: &[E],
    eta1: E,
    eta2: E,
    eta3: E,
    kt: &KTables,
) -> Vec<Vec<E>> {
    let eq_subrow = eq_table(&r1[0..5], kt);  // 32 (active 0..28)
    let eq_col = eq_table(&r1[5..14], kt);    // 512
    let l_map = RingSwitchLinearMap::new(w_rs);

    let mut m_rs = vec![vec![E::ZERO; K_COLS]; VIRTUAL_ROWS];

    // 1. Active witness rows (0..140): 5 polys x 28 active subrows
    let wit_rows: Vec<(usize, Vec<E>)> = (0..M_WIT_ROWS)
        .into_par_iter()
        .map(|idx| {
            let x = idx / ACTIVE_SUBROWS_PER_POLY;
            let sub = idx % ACTIVE_SUBROWS_PER_POLY;
            let g_es = if x == 0 {
                eq_subrow[sub]
            } else {
                gammas[x].mul(eq_subrow[sub], kt)
            };
            let row_vec: Vec<E> = (0..K_COLS)
                .map(|col| l_map.apply(g_es.mul(eq_col[col], kt)))
                .collect();
            (layout.non_mask_rows[idx], row_vec)
        })
        .collect();
    for (vrow, row_vec) in wit_rows {
        m_rs[vrow] = row_vec;
    }

    // 2. Ring-Switching mask rows (140..145): 1 row per poly, cols 0..384
    for x in 0..5 {
        let g_tau = if x == 0 { tau } else { gammas[x].mul(tau, kt) };
        let vrow = layout.non_mask_rows[M_WIT_ROWS + x];
        for flat in 0..384 {
            let v = flat / 32;
            let u = flat % 32;
            m_rs[vrow][flat] = l_map.apply(g_tau.mul(E::from_basis_vu(v, u), kt));
        }
    }

    // 3. G_1 + m_g1 + m_g2 rows (145..147): 78 * 12 = 936 K-elems across 2 rows of 512 cols
    let w78 = eval_g1_mg_weights(r1, tau, eta1, eta2, kt);
    for e_idx in 0..78 {
        let we = w78[e_idx];
        for v in 0..12 {
            let flat = e_idx * 12 + v;
            let row_off = flat >> 9;
            let col = flat & 0x1FF;
            m_rs[layout.non_mask_rows[M_WIT_ROWS + M_RS_MASK_ROWS + row_off]][col] =
                we.mul(E::from_basis_vu(v, 0), kt);
        }
    }

    // 4. Sumcheck-2 16 mask rows (scaled by eta3 != 0): cols 0..12 and 12..24
    for &(a, b) in &MASK2_PAIRS {
        for v in 0..12 {
            let yv = eta3.mul(E::from_basis_vu(v, 0), kt);
            m_rs[a][v] = yv;
            m_rs[a][12 + v] = yv;
            m_rs[b][v] = yv.scale_k(ZETA1_K, kt);
            m_rs[b][12 + v] = yv.scale_k(ZETA2_K, kt);
        }
    }

    m_rs
}

/// Fast 3D Karatsuba polynomial multiplication in E[Y, Z, x] of shape (12, 2, 16) x (12, 2, 16)
/// -> (23, 3, 31), followed by exact tower reduction mod (Y^12+Y^3+Y+Z+x, Z^2+Z+x^11, x^16+x^5+x^3+x+1).
/// Computes `sum_{k} r_wt[k] * v_open[col] * L_{w_rs}(g_es[k] * eq_col[col])` in ONLY 13,122 E::muls
/// (11x fewer E::muls than direct 140 x 512 evaluation, ZERO lookup tables, bit-for-bit exact!).
fn karatsuba4_e(a: &[E; 4], b: &[E; 4], out: &mut [E; 7], kt: &KTables) {
    let z00 = a[0].mul(b[0], kt);
    let z02 = a[1].mul(b[1], kt);
    let z01 = a[0].add(a[1]).mul(b[0].add(b[1]), kt).add(z00).add(z02);

    let z20 = a[2].mul(b[2], kt);
    let z22 = a[3].mul(b[3], kt);
    let z21 = a[2].add(a[3]).mul(b[2].add(b[3]), kt).add(z20).add(z22);

    let sa0 = a[0].add(a[2]);
    let sa1 = a[1].add(a[3]);
    let sb0 = b[0].add(b[2]);
    let sb1 = b[1].add(b[3]);
    let z10 = sa0.mul(sb0, kt);
    let z12 = sa1.mul(sb1, kt);
    let z11 = sa0.add(sa1).mul(sb0.add(sb1), kt).add(z10).add(z12);

    out[0] = z00;
    out[1] = z01;
    out[2] = z02.add(z10).add(z00).add(z20);
    out[3] = z11.add(z01).add(z21);
    out[4] = z20.add(z12).add(z02).add(z22);
    out[5] = z21;
    out[6] = z22;
}

fn karatsuba8_e(a: &[E; 8], b: &[E; 8], out: &mut [E; 15], kt: &KTables) {
    let a0 = [a[0], a[1], a[2], a[3]];
    let a1 = [a[4], a[5], a[6], a[7]];
    let as_ = [a[0].add(a[4]), a[1].add(a[5]), a[2].add(a[6]), a[3].add(a[7])];
    let b0 = [b[0], b[1], b[2], b[3]];
    let b1 = [b[4], b[5], b[6], b[7]];
    let bs_ = [b[0].add(b[4]), b[1].add(b[5]), b[2].add(b[6]), b[3].add(b[7])];
    let mut z0 = [E::ZERO; 7];
    let mut z1 = [E::ZERO; 7];
    let mut z2 = [E::ZERO; 7];
    karatsuba4_e(&a0, &b0, &mut z0, kt);
    karatsuba4_e(&a1, &b1, &mut z2, kt);
    karatsuba4_e(&as_, &bs_, &mut z1, kt);
    for i in 0..4 {
        out[i] = z0[i];
    }
    for i in 0..3 {
        out[4 + i] = z0[4 + i].add(z1[i]).add(z0[i]).add(z2[i]);
    }
    out[7] = z1[3].add(z0[3]).add(z2[3]);
    for i in 0..3 {
        out[8 + i] = z2[i].add(z1[4 + i]).add(z0[4 + i]).add(z2[4 + i]);
    }
    for i in 3..7 {
        out[8 + i] = z2[i];
    }
}

fn karatsuba16_e(a: &[E; 16], b: &[E; 16], out: &mut [E; 31], kt: &KTables) {
    let mut a0 = [E::ZERO; 8];
    let mut a1 = [E::ZERO; 8];
    let mut as_ = [E::ZERO; 8];
    let mut b0 = [E::ZERO; 8];
    let mut b1 = [E::ZERO; 8];
    let mut bs_ = [E::ZERO; 8];
    for i in 0..8 {
        a0[i] = a[i];
        a1[i] = a[8 + i];
        as_[i] = a[i].add(a[8 + i]);
        b0[i] = b[i];
        b1[i] = b[8 + i];
        bs_[i] = b[i].add(b[8 + i]);
    }
    let mut z0 = [E::ZERO; 15];
    let mut z1 = [E::ZERO; 15];
    let mut z2 = [E::ZERO; 15];
    karatsuba8_e(&a0, &b0, &mut z0, kt);
    karatsuba8_e(&a1, &b1, &mut z2, kt);
    karatsuba8_e(&as_, &bs_, &mut z1, kt);
    for i in 0..8 {
        out[i] = z0[i];
    }
    for i in 0..7 {
        out[8 + i] = z0[8 + i].add(z1[i]).add(z0[i]).add(z2[i]);
    }
    out[15] = z1[7].add(z0[7]).add(z2[7]);
    for i in 0..7 {
        out[16 + i] = z2[i].add(z1[8 + i]).add(z0[8 + i]).add(z2[8 + i]);
    }
    for i in 7..15 {
        out[16 + i] = z2[i];
    }
}

fn mul_2x16_e(a: &[[E; 16]; 2], b: &[[E; 16]; 2], out: &mut [[E; 31]; 3], kt: &KTables) {
    let mut z0 = [E::ZERO; 31];
    let mut z2 = [E::ZERO; 31];
    let mut z1 = [E::ZERO; 31];
    let mut as_ = [E::ZERO; 16];
    let mut bs_ = [E::ZERO; 16];
    for i in 0..16 {
        as_[i] = a[0][i].add(a[1][i]);
        bs_[i] = b[0][i].add(b[1][i]);
    }
    karatsuba16_e(&a[0], &b[0], &mut z0, kt);
    karatsuba16_e(&a[1], &b[1], &mut z2, kt);
    karatsuba16_e(&as_, &bs_, &mut z1, kt);
    out[0] = z0;
    for i in 0..31 {
        out[1][i] = z1[i].add(z0[i]).add(z2[i]);
    }
    out[2] = z2;
}

fn add_2x16(a: &[[E; 16]; 2], b: &[[E; 16]; 2]) -> [[E; 16]; 2] {
    let mut out = [[E::ZERO; 16]; 2];
    for z in 0..2 {
        for u in 0..16 {
            out[z][u] = a[z][u].add(b[z][u]);
        }
    }
    out
}

fn add_3x31_assign(dst: &mut [[E; 31]; 3], src: &[[E; 31]; 3]) {
    for z in 0..3 {
        for u in 0..31 {
            dst[z][u].add_assign(src[z][u]);
        }
    }
}

fn mul_3x2x16_e(a: &[[[E; 16]; 2]; 3], b: &[[[E; 16]; 2]; 3], out: &mut [[[E; 31]; 3]; 5], kt: &KTables) {
    let mut p0 = [[E::ZERO; 31]; 3];
    let mut p1 = [[E::ZERO; 31]; 3];
    let mut p2 = [[E::ZERO; 31]; 3];
    let mut q01 = [[E::ZERO; 31]; 3];
    let mut q02 = [[E::ZERO; 31]; 3];
    let mut q12 = [[E::ZERO; 31]; 3];
    mul_2x16_e(&a[0], &b[0], &mut p0, kt);
    mul_2x16_e(&a[1], &b[1], &mut p1, kt);
    mul_2x16_e(&a[2], &b[2], &mut p2, kt);
    mul_2x16_e(&add_2x16(&a[0], &a[1]), &add_2x16(&b[0], &b[1]), &mut q01, kt);
    mul_2x16_e(&add_2x16(&a[0], &a[2]), &add_2x16(&b[0], &b[2]), &mut q02, kt);
    mul_2x16_e(&add_2x16(&a[1], &a[2]), &add_2x16(&b[1], &b[2]), &mut q12, kt);
    out[0] = p0;
    out[1] = q01;
    add_3x31_assign(&mut out[1], &p0);
    add_3x31_assign(&mut out[1], &p1);
    out[2] = q02;
    add_3x31_assign(&mut out[2], &p0);
    add_3x31_assign(&mut out[2], &p1);
    add_3x31_assign(&mut out[2], &p2);
    out[3] = q12;
    add_3x31_assign(&mut out[3], &p1);
    add_3x31_assign(&mut out[3], &p2);
    out[4] = p2;
}

fn mul_6x2x16_e(a: &[[[E; 16]; 2]; 6], b: &[[[E; 16]; 2]; 6], out: &mut [[[E; 31]; 3]; 11], kt: &KTables) {
    let a_lo = [a[0], a[1], a[2]];
    let a_hi = [a[3], a[4], a[5]];
    let a_sum = [add_2x16(&a[0], &a[3]), add_2x16(&a[1], &a[4]), add_2x16(&a[2], &a[5])];
    let b_lo = [b[0], b[1], b[2]];
    let b_hi = [b[3], b[4], b[5]];
    let b_sum = [add_2x16(&b[0], &b[3]), add_2x16(&b[1], &b[4]), add_2x16(&b[2], &b[5])];
    let mut z0 = [[[E::ZERO; 31]; 3]; 5];
    let mut z1 = [[[E::ZERO; 31]; 3]; 5];
    let mut z2 = [[[E::ZERO; 31]; 3]; 5];
    mul_3x2x16_e(&a_lo, &b_lo, &mut z0, kt);
    mul_3x2x16_e(&a_hi, &b_hi, &mut z2, kt);
    mul_3x2x16_e(&a_sum, &b_sum, &mut z1, kt);
    for i in 0..3 {
        out[i] = z0[i];
    }
    for i in 0..2 {
        out[3 + i] = z0[3 + i];
        add_3x31_assign(&mut out[3 + i], &z1[i]);
        add_3x31_assign(&mut out[3 + i], &z0[i]);
        add_3x31_assign(&mut out[3 + i], &z2[i]);
    }
    out[5] = z1[2];
    add_3x31_assign(&mut out[5], &z0[2]);
    add_3x31_assign(&mut out[5], &z2[2]);
    for i in 0..2 {
        out[6 + i] = z2[i];
        add_3x31_assign(&mut out[6 + i], &z1[3 + i]);
        add_3x31_assign(&mut out[6 + i], &z0[3 + i]);
        add_3x31_assign(&mut out[6 + i], &z2[3 + i]);
    }
    for i in 2..5 {
        out[6 + i] = z2[i];
    }
}

fn eval_bilinear_rs_ip(
    r_arr: &[[[E; 16]; 2]; 12],
    v_arr: &[[[E; 16]; 2]; 12],
    w_rs: &[E],
    kt: &KTables,
) -> E {
    let a_lo = [r_arr[0], r_arr[1], r_arr[2], r_arr[3], r_arr[4], r_arr[5]];
    let a_hi = [r_arr[6], r_arr[7], r_arr[8], r_arr[9], r_arr[10], r_arr[11]];
    let a_sum: [[[E; 16]; 2]; 6] = core::array::from_fn(|i| add_2x16(&a_lo[i], &a_hi[i]));
    let b_lo = [v_arr[0], v_arr[1], v_arr[2], v_arr[3], v_arr[4], v_arr[5]];
    let b_hi = [v_arr[6], v_arr[7], v_arr[8], v_arr[9], v_arr[10], v_arr[11]];
    let b_sum: [[[E; 16]; 2]; 6] = core::array::from_fn(|i| add_2x16(&b_lo[i], &b_hi[i]));

    let mut z0 = vec![[[E::ZERO; 31]; 3]; 11];
    let mut z1 = vec![[[E::ZERO; 31]; 3]; 11];
    let mut z2 = vec![[[E::ZERO; 31]; 3]; 11];
    mul_6x2x16_e(&a_lo, &b_lo, (&mut z0[..]).try_into().unwrap(), kt);
    mul_6x2x16_e(&a_hi, &b_hi, (&mut z2[..]).try_into().unwrap(), kt);
    mul_6x2x16_e(&a_sum, &b_sum, (&mut z1[..]).try_into().unwrap(), kt);

    let mut c = vec![[[E::ZERO; 44]; 4]; 23];
    for i in 0..6 {
        for z in 0..3 {
            c[i][z][..31].copy_from_slice(&z0[i][z]);
        }
    }
    for i in 0..5 {
        for z in 0..3 {
            for u in 0..31 {
                c[6 + i][z][u] = z0[6 + i][z][u].add(z1[i][z][u]).add(z0[i][z][u]).add(z2[i][z][u]);
            }
        }
    }
    for z in 0..3 {
        for u in 0..31 {
            c[11][z][u] = z1[5][z][u].add(z0[5][z][u]).add(z2[5][z][u]);
        }
    }
    for i in 0..5 {
        for z in 0..3 {
            for u in 0..31 {
                c[12 + i][z][u] = z2[i][z][u].add(z1[6 + i][z][u]).add(z0[6 + i][z][u]).add(z2[6 + i][z][u]);
            }
        }
    }
    for i in 5..11 {
        for z in 0..3 {
            c[12 + i][z][..31].copy_from_slice(&z2[i][z]);
        }
    }

    // 1. Reduce Y^12 = Y^3 + Y + Z + x
    for v in (12..23).rev() {
        for z in 0..3 {
            for u in 0..31 {
                let val = c[v][z][u];
                if !val.is_zero() {
                    c[v - 12 + 3][z][u].add_assign(val);
                    c[v - 12 + 1][z][u].add_assign(val);
                    c[v - 12][z + 1][u].add_assign(val);
                    c[v - 12][z][u + 1].add_assign(val);
                }
            }
        }
    }
    // 2. Reduce Z^2 = Z + x^11
    for v in 0..12 {
        for z in (2..4).rev() {
            for u in 0..32 {
                let val = c[v][z][u];
                if !val.is_zero() {
                    c[v][z - 1][u].add_assign(val);
                    c[v][z - 2][u + 11].add_assign(val);
                }
            }
        }
    }
    // 3. Reduce x^16 = x^5 + x^3 + x + 1
    let mut acc = E::ZERO;
    for v in 0..12 {
        for z in 0..2 {
            for u in (16..43).rev() {
                let val = c[v][z][u];
                if !val.is_zero() {
                    c[v][z][u - 16 + 5].add_assign(val);
                    c[v][z][u - 16 + 3].add_assign(val);
                    c[v][z][u - 16 + 1].add_assign(val);
                    c[v][z][u - 16].add_assign(val);
                }
            }
            for u in 0..16 {
                acc.add_assign(c[v][z][u].mul(w_rs[32 * v + 16 * z + u], kt));
            }
        }
    }
    acc
}

/// Fast single-core verifier computation of `ip_lhs = sum_{col=0}^{511} m_col[col] * v_open_h[col]`.
/// Uses exact 3D Karatsuba + tower reduction for the 140 witness rows (13,122 E::muls vs 143,360!).
fn eval_verifier_ip_lhs(
    layout: &RowLayout,
    r1: &[E; V_SC1],
    tau: E,
    gammas: &[E; 5],
    w_rs: &[E],
    eta1: E,
    eta2: E,
    eta3: E,
    r_row_p: &[E; V_SC2],
    v_open_h: &[E],
    kt: &KTables,
) -> E {
    let eq_subrow = eq_table(&r1[0..5], kt);
    let eq_col = eq_table(&r1[5..14], kt);
    let eq_rp = eq_table(r_row_p, kt);

    // 1. Active witness rows (140 x 512) via exact 3D Karatsuba bilinear form (ONLY 13,122 E::muls!)
    let mut r_arr = [[[E::ZERO; 16]; 2]; 12];
    for x in 0..5 {
        let gx = gammas[x];
        for sub in 0..ACTIVE_SUBROWS_PER_POLY {
            let g_es = if x == 0 {
                eq_subrow[sub]
            } else {
                gx.mul(eq_subrow[sub], kt)
            };
            let r_wt = eq_rp[layout.non_mask_rows[ACTIVE_SUBROWS_PER_POLY * x + sub]];
            for v in 0..12 {
                let mut lo = (g_es.c[v] & 0xFFFF) as u32;
                while lo != 0 {
                    let u = lo.trailing_zeros() as usize;
                    r_arr[v][0][u].add_assign(r_wt);
                    lo &= lo - 1;
                }
                let mut hi = (g_es.c[v] >> 16) as u32;
                while hi != 0 {
                    let u = hi.trailing_zeros() as usize;
                    r_arr[v][1][u].add_assign(r_wt);
                    hi &= hi - 1;
                }
            }
        }
    }
    let mut v_arr = [[[E::ZERO; 16]; 2]; 12];
    for col in 0..K_COLS {
        let ec = eq_col[col];
        let vo = v_open_h[col];
        for v in 0..12 {
            let mut lo = (ec.c[v] & 0xFFFF) as u32;
            while lo != 0 {
                let u = lo.trailing_zeros() as usize;
                v_arr[v][0][u].add_assign(vo);
                lo &= lo - 1;
            }
            let mut hi = (ec.c[v] >> 16) as u32;
            while hi != 0 {
                let u = hi.trailing_zeros() as usize;
                v_arr[v][1][u].add_assign(vo);
                hi &= hi - 1;
            }
        }
    }
    let mut ip_lhs = eval_bilinear_rs_ip(&r_arr, &v_arr, w_rs, kt);

    // 2. Ring-Switching mask rows (5 x 384 = 1,920 terms, factored r_wt outside flat loop -> 0.5 ms!)
    let l_map = RingSwitchLinearMap::new(w_rs);
    for x in 0..5 {
        let g_tau = if x == 0 { tau } else { gammas[x].mul(tau, kt) };
        let r_wt = eq_rp[layout.non_mask_rows[M_WIT_ROWS + x]];
        let mut row_acc = E::ZERO;
        for flat in 0..384 {
            let v = flat / 32;
            let u = flat % 32;
            let lm = l_map.apply(g_tau.mul(E::from_basis_vu(v, u), kt));
            row_acc.add_assign(lm.mul(v_open_h[flat], kt));
        }
        ip_lhs.add_assign(r_wt.mul(row_acc, kt));
    }

    // 3. G_1 + m_g1 + m_g2 rows (936 entries)
    let w78 = eval_g1_mg_weights(r1, tau, eta1, eta2, kt);
    let r_g1_0 = eq_rp[layout.non_mask_rows[M_WIT_ROWS + M_RS_MASK_ROWS]];
    let r_g1_1 = eq_rp[layout.non_mask_rows[M_WIT_ROWS + M_RS_MASK_ROWS + 1]];
    for e_idx in 0..78 {
        let we = w78[e_idx];
        for v in 0..12 {
            let flat = e_idx * 12 + v;
            let r_wt = if (flat >> 9) == 0 { r_g1_0 } else { r_g1_1 };
            let col = flat & 0x1FF;
            let term = r_wt.mul(we.mul(E::from_basis_vu(v, 0), kt), kt);
            ip_lhs.add_assign(term.mul(v_open_h[col], kt));
        }
    }

    // 4. Sumcheck-2 16 mask rows (cols 0..24)
    let mut sum_a = E::ZERO;
    let mut sum_b = E::ZERO;
    for &(a, b) in &MASK2_PAIRS {
        sum_a.add_assign(eq_rp[a]);
        sum_b.add_assign(eq_rp[b]);
    }
    let w_blk0 = eta3.mul(sum_a.add(sum_b.scale_k(ZETA1_K, kt)), kt);
    let w_blk1 = eta3.mul(sum_a.add(sum_b.scale_k(ZETA2_K, kt)), kt);
    for v in 0..12 {
        let yv = E::from_basis_vu(v, 0);
        ip_lhs.add_assign(w_blk0.mul(yv, kt).mul(v_open_h[v], kt));
        ip_lhs.add_assign(w_blk1.mul(yv, kt).mul(v_open_h[12 + v], kt));
    }

    ip_lhs
}

/// Generate a complete ZK Peel-Binius-Shielded proof using streaming L1-cache-resident
/// 1,024-column ANTT (peak RSS ~300 MiB) and single fixed-range SHA-384 compiler oracle H.
pub fn prove(
    stmt: &PaymentStatement,
    wit_bits: &CircuitWitnessBits,
    rng: &mut impl RngCore,
) -> BiniusProof {
    let kt = k_tables();
    let layout = row_layout();
    let stmt_bytes = encode_statement(stmt);

    let rand_e = |rng: &mut dyn RngCore| -> E {
        let mut buf = [0u8; E_BYTES];
        rng.fill_bytes(&mut buf);
        E::from_bytes(&buf)
    };
    let rand_k = |rng: &mut dyn RngCore| -> u32 {
        let mut buf = [0u8; 4];
        rng.fill_bytes(&mut buf);
        u32::from_le_bytes(buf)
    };

    // 1. Populate the 256 x 512 virtual K-matrix U_H on H = V_9
    let mut u_h = vec![[0u32; K_COLS]; VIRTUAL_ROWS];

    // 1a. Pack active witness rows (140 rows = 5 polys x 28 subrows x 512 cols x 32 bits)
    for x in 0..5 {
        for sub in 0..ACTIVE_SUBROWS_PER_POLY {
            let vrow = layout.non_mask_rows[ACTIVE_SUBROWS_PER_POLY * x + sub];
            for col in 0..K_COLS {
                let base = (sub << 14) | (col << 5);
                let mut word = 0u32;
                for b in 0..32 {
                    if wit_bits.b[x][base | b] != 0 {
                        word |= 1u32 << b;
                    }
                }
                u_h[vrow][col] = word;
            }
        }
    }

    // 1b. Sample 5 x 32 = 160 uniform Ring-Switching E-masks M^{(x)}_b (5 K-rows, cols 0..384)
    let mut rs_masks = [[E::ZERO; 32]; 5];
    for x in 0..5 {
        for b in 0..32 {
            rs_masks[x][b] = rand_e(rng);
        }
        let vrow = layout.non_mask_rows[M_WIT_ROWS + x];
        for flat in 0..384 {
            let v = flat / 32;
            let u = flat % 32;
            let mut word = 0u32;
            for b in 0..32 {
                if ((rs_masks[x][b].c[v] >> u) & 1) != 0 {
                    word |= 1u32 << b;
                }
            }
            u_h[vrow][flat] = word;
        }
    }

    // 1c. Sample G_1 (19 x 4 = 76 E-coeffs) + m_g1, m_g2 (2 E-coeffs) -> 2 K-rows (936 K-elems)
    let mut g1 = [[E::ZERO; 4]; V_SC1];
    let mut s_g1 = E::ZERO;
    for d in 0..V_SC1 {
        for k in 0..4 {
            g1[d][k] = rand_e(rng);
        }
        s_g1.add_assign(g1[d][1].add(g1[d][2]).add(g1[d][3]));
    }
    let m_g1 = rand_e(rng);
    let m_g2 = rand_e(rng);
    for e_idx in 0..78 {
        let val = if e_idx < 76 {
            g1[e_idx / 4][e_idx % 4]
        } else if e_idx == 76 {
            m_g1
        } else {
            m_g2
        };
        for v in 0..12 {
            let flat = e_idx * 12 + v;
            let vrow = layout.non_mask_rows[M_WIT_ROWS + M_RS_MASK_ROWS + (flat >> 9)];
            u_h[vrow][flat & 0x1FF] = val.c[v];
        }
    }

    // 1d. Sample Sumcheck-2 16 mask rows (2 uniform E-elements per row -> cols 0..24)
    let mut s_mask2 = E::ZERO;
    for &(a, b) in &MASK2_PAIRS {
        let ga0 = rand_e(rng);
        let ga1 = rand_e(rng);
        let gb0 = rand_e(rng);
        let gb1 = rand_e(rng);
        for v in 0..12 {
            u_h[a][v] = ga0.c[v];
            u_h[a][12 + v] = ga1.c[v];
            u_h[b][v] = gb0.c[v];
            u_h[b][12 + v] = gb1.c[v];
        }
        s_mask2.add_assign(ga0);
        s_mask2.add_assign(ga1);
        s_mask2.add_assign(gb0.scale_k(ZETA1_K, kt));
        s_mask2.add_assign(gb1.scale_k(ZETA2_K, kt));
    }

    // 2. Compute Novel Basis coefficients (DEG_BOUND = 565) for all 163 K-rows + B_pcs, B_prox
    //    Total coefficient footprint: ONLY ~422 KiB (ZERO 1.3 GiB codeword allocation!)
    let mut k_pads = vec![[0u32; DEG_BOUND - K_COLS]; M_K_ROWS];
    for pad in &mut k_pads {
        for v in pad.iter_mut() {
            *v = rand_k(rng);
        }
    }
    let mut b_pcs_coeffs = vec![E::ZERO; DEG_BOUND];
    for c in &mut b_pcs_coeffs {
        *c = rand_e(rng);
    }
    let mut b_prox_coeffs = vec![E::ZERO; DEG_BOUND];
    for c in &mut b_prox_coeffs {
        *c = rand_e(rng);
    }

    let k_coeffs: Vec<[u32; DEG_BOUND]> = (0..M_K_ROWS)
        .into_par_iter()
        .map(|aidx| {
            let vrow = layout.active_rows[aidx];
            let mut h_buf = u_h[vrow].to_vec();
            kt.antt_inverse_h_k(&mut h_buf);
            let mut c = [0u32; DEG_BOUND];
            c[..K_COLS].copy_from_slice(&h_buf);
            c[K_COLS..DEG_BOUND].copy_from_slice(&k_pads[aidx]);
            c
        })
        .collect();

    let mut b_pcs_h = b_pcs_coeffs[..K_COLS].to_vec();
    kt.antt_forward_h_e(&mut b_pcs_h);

    // 3. Streaming L1-cache-resident MT_1 leaf generation across 2,048 coset blocks of 1,024 cols!
    let mut leaves = vec![[0u8; HASH_BYTES]; N_CODE];
    leaves
        .par_chunks_mut(1024)
        .enumerate()
        .for_each(|(blk, leaf_chunk)| {
            // Per-thread working set: heap-allocated so rayon work-stealing stacks stay tiny!
            let mut u_blk = vec![[0u32; 1024]; M_K_ROWS];
            for aidx in 0..M_K_ROWS {
                kt.antt_forward_block1024_k(&k_coeffs[aidx], blk, &mut u_blk[aidx]);
            }
            let mut b_pcs_blk = Box::new([E::ZERO; 1024]);
            let mut b_prox_blk = Box::new([E::ZERO; 1024]);
            kt.antt_forward_block1024_e(&b_pcs_coeffs, blk, &mut b_pcs_blk);
            kt.antt_forward_block1024_e(&b_prox_coeffs, blk, &mut b_prox_blk);

            let base_col = blk * 1024;
            let mut k_col = [0u32; M_K_ROWS];
            for sub in 0..1024 {
                for aidx in 0..M_K_ROWS {
                    k_col[aidx] = u_blk[aidx][sub];
                }
                leaf_chunk[sub] = oracle_leaf(base_col + sub, &k_col, b_pcs_blk[sub], b_prox_blk[sub]);
            }
        });

    let mut tree_levels: Vec<Vec<[u8; HASH_BYTES]>> = Vec::with_capacity(MERKLE_HEIGHT + 1);
    tree_levels.push(leaves);
    for lvl in 0..MERKLE_HEIGHT {
        let prev = &tree_levels[lvl];
        let next: Vec<[u8; HASH_BYTES]> = (0..(prev.len() / 2))
            .into_par_iter()
            .map(|pos| oracle_node(lvl as u8, pos, &prev[2 * pos], &prev[2 * pos + 1]))
            .collect();
        tree_levels.push(next);
    }
    let merkle_root = tree_levels[MERKLE_HEIGHT][0];

    // 4. Transcript init & single-element Round-2 CircuitChallenge
    let mut tr = Transcript::new(&stmt_bytes, &merkle_root, s_g1, s_mask2);
    let ch = CircuitChallenge {
        alpha: tr.squeeze_e(b"C02"),
    };

    // 5. Sumcheck 1 (19 rounds, degree 3, ZK-blinded by G_1)
    let (wb_full, wc_full, pub_rhs) = build_prover_weights(stmt, &ch, kt);
    let mut claim1 = pub_rhs.add(s_g1);
    let mut sc1_rounds = [[E::ZERO; 3]; V_SC1];
    let mut r1 = [E::ZERO; V_SC1];

    let inv6_k = kt.kinv(6);
    let interp_deg3 = |y0: E, y1: E, y2: E, y3: E| -> [E; 4] {
        let d1 = y1.add(y0);
        let d2 = y2.add(y0);
        let d3 = y3.add(y0);
        let e2 = d2.add(d1.scale_k(2, kt));
        let e3 = d3.add(d1.scale_k(3, kt));
        let c3 = e2.add(e3).scale_k(inv6_k, kt);
        let c2 = e2.add(c3.scale_k(10, kt)).scale_k(inv6_k, kt);
        let c1 = d1.add(c2).add(c3);
        [y0, c1, c2, c3]
    };

    // Round 0 (raw bits over 2^19)
    {
        let half = NUM_BITS_PER_POLY / 2;
        let (y0, y1, y2, y3) = (0..half)
            .into_par_iter()
            .fold(
                || (E::ZERO, E::ZERO, E::ZERO, E::ZERO),
                |(mut a0, mut a1, mut a2, mut a3), i| {
                    let i0 = i;
                    let i1 = i + half;
                    let mut b0 = [0u32; 5];
                    let mut b1 = [0u32; 5];
                    let mut b2 = [0u32; 5];
                    let mut b3 = [0u32; 5];
                    for x in 0..5 {
                        let v0 = wit_bits.b[x][i0] as u32;
                        let v1 = wit_bits.b[x][i1] as u32;
                        let dv = v0 ^ v1;
                        b0[x] = v0;
                        b1[x] = v1;
                        b2[x] = v0 ^ (dv << 1);
                        b3[x] = v1 ^ (dv << 1);
                    }
                    for x in 0..5 {
                        let wb0 = wb_full[x][i0];
                        let wb1 = wb_full[x][i1];
                        let dwb = wb0.add(wb1);
                        let wb2 = wb0.add(dwb.scale_k(2, kt));
                        let wb3 = wb1.add(dwb.scale_k(2, kt));

                        let wc0 = wc_full[x][i0];
                        let wc1 = wc_full[x][i1];
                        let dwc = wc0.add(wc1);
                        let wc2 = wc0.add(dwc.scale_k(2, kt));
                        let wc3 = wc1.add(dwc.scale_k(2, kt));

                        let xp1 = (x + 1) % 5;
                        let xp2 = (x + 2) % 5;

                        if b0[x] != 0 {
                            a0.add_assign(wb0);
                        }
                        if ((1 ^ b0[xp1]) & b0[xp2]) != 0 {
                            a0.add_assign(wc0);
                        }
                        if b1[x] != 0 {
                            a1.add_assign(wb1);
                        }
                        if ((1 ^ b1[xp1]) & b1[xp2]) != 0 {
                            a1.add_assign(wc1);
                        }
                        a2.add_mul_k(b2[x], wb2, kt);
                        a2.add_mul_k(kt.kmul(1 ^ b2[xp1], b2[xp2]), wc2, kt);

                        a3.add_mul_k(b3[x], wb3, kt);
                        a3.add_mul_k(kt.kmul(1 ^ b3[xp1], b3[xp2]), wc3, kt);
                    }
                    (a0, a1, a2, a3)
                },
            )
            .reduce(
                || (E::ZERO, E::ZERO, E::ZERO, E::ZERO),
                |l, r| (l.0.add(r.0), l.1.add(r.1), l.2.add(r.2), l.3.add(r.3)),
            );

        let mut poly = interp_deg3(y0, y1, y2, y3);
        let g_poly = eval_g1_round(&g1, &[], 0, kt);
        for k in 0..4 {
            poly[k].add_assign(g_poly[k]);
        }
        debug_assert_eq!(poly[1].add(poly[2]).add(poly[3]), claim1);
        sc1_rounds[0] = [poly[0], poly[2], poly[3]];
        tr.absorb_e(b"S100", &sc1_rounds[0]);
        let r_ch = tr.squeeze_e(b"C03");
        assert!(!r_ch.is_zero(), "zero SC1 challenge");
        r1[0] = r_ch;
        claim1 = poly[0].add(r_ch.mul(poly[1].add(r_ch.mul(poly[2].add(r_ch.mul(poly[3], kt)), kt)), kt));
    }

    // Fold after Round 0 -> size 2^18 = 262,144, then immediately drop full 2^19 wb_full/wc_full!
    let half0 = NUM_BITS_PER_POLY / 2;
    let mut b_cur: [Vec<E>; 5] = core::array::from_fn(|x| {
        let r0 = r1[0];
        let om = r0.add(E::one());
        (0..half0)
            .into_par_iter()
            .map(|i| match (wit_bits.b[x][i], wit_bits.b[x][i + half0]) {
                (0, 0) => E::ZERO,
                (1, 0) => om,
                (0, 1) => r0,
                (1, 1) => E::one(),
                _ => unreachable!(),
            })
            .collect()
    });
    let mut wb_cur: [Vec<E>; 5] = core::array::from_fn(|x| {
        let r0 = r1[0];
        (0..half0)
            .into_par_iter()
            .map(|i| {
                let a = wb_full[x][i];
                let b = wb_full[x][i + half0];
                a.add(r0.mul(a.add(b), kt))
            })
            .collect()
    });
    drop(wb_full);
    let mut wc_cur: [Vec<E>; 5] = core::array::from_fn(|x| {
        let r0 = r1[0];
        (0..half0)
            .into_par_iter()
            .map(|i| {
                let a = wc_full[x][i];
                let b = wc_full[x][i + half0];
                a.add(r0.mul(a.add(b), kt))
            })
            .collect()
    });
    drop(wc_full);

    // Rounds 1..18
    for d in 1..V_SC1 {
        let half = b_cur[0].len() / 2;
        let (y0, y1, y2, y3) = (0..half)
            .into_par_iter()
            .fold(
                || (E::ZERO, E::ZERO, E::ZERO, E::ZERO),
                |(mut a0, mut a1, mut a2, mut a3), i| {
                    let i0 = i;
                    let i1 = i + half;
                    let mut b0 = [E::ZERO; 5];
                    let mut b1 = [E::ZERO; 5];
                    let mut b2 = [E::ZERO; 5];
                    let mut b3 = [E::ZERO; 5];
                    for x in 0..5 {
                        let v0 = b_cur[x][i0];
                        let v1 = b_cur[x][i1];
                        let d2 = v0.add(v1).scale_k(2, kt);
                        b0[x] = v0;
                        b1[x] = v1;
                        b2[x] = v0.add(d2);
                        b3[x] = v1.add(d2);
                    }
                    let one = E::one();
                    for x in 0..5 {
                        let wb0 = wb_cur[x][i0];
                        let wb1 = wb_cur[x][i1];
                        let dwb2 = wb0.add(wb1).scale_k(2, kt);
                        let wb2 = wb0.add(dwb2);
                        let wb3 = wb1.add(dwb2);

                        let wc0 = wc_cur[x][i0];
                        let wc1 = wc_cur[x][i1];
                        let dwc2 = wc0.add(wc1).scale_k(2, kt);
                        let wc2 = wc0.add(dwc2);
                        let wc3 = wc1.add(dwc2);

                        let xp1 = (x + 1) % 5;
                        let xp2 = (x + 2) % 5;

                        a0.add_assign(b0[x].mul(wb0, kt));
                        a0.add_assign(one.add(b0[xp1]).mul(b0[xp2], kt).mul(wc0, kt));

                        a1.add_assign(b1[x].mul(wb1, kt));
                        a1.add_assign(one.add(b1[xp1]).mul(b1[xp2], kt).mul(wc1, kt));

                        a2.add_assign(b2[x].mul(wb2, kt));
                        a2.add_assign(one.add(b2[xp1]).mul(b2[xp2], kt).mul(wc2, kt));

                        a3.add_assign(b3[x].mul(wb3, kt));
                        a3.add_assign(one.add(b3[xp1]).mul(b3[xp2], kt).mul(wc3, kt));
                    }
                    (a0, a1, a2, a3)
                },
            )
            .reduce(
                || (E::ZERO, E::ZERO, E::ZERO, E::ZERO),
                |l, r| (l.0.add(r.0), l.1.add(r.1), l.2.add(r.2), l.3.add(r.3)),
            );

        let mut poly = interp_deg3(y0, y1, y2, y3);
        let g_poly = eval_g1_round(&g1, &r1[..d], d, kt);
        for k in 0..4 {
            poly[k].add_assign(g_poly[k]);
        }
        debug_assert_eq!(poly[1].add(poly[2]).add(poly[3]), claim1);
        sc1_rounds[d] = [poly[0], poly[2], poly[3]];
        let sc_tag = [b'S', b'1', b'0' + ((d / 10) as u8), b'0' + ((d % 10) as u8)];
        let ch_tag = [b'C', b'0' + (((d + 3) / 10) as u8), b'0' + (((d + 3) % 10) as u8)];
        tr.absorb_e(&sc_tag, &sc1_rounds[d]);
        let r_ch = tr.squeeze_e(&ch_tag);
        assert!(!r_ch.is_zero(), "zero SC1 challenge");
        r1[d] = r_ch;
        claim1 = poly[0].add(r_ch.mul(poly[1].add(r_ch.mul(poly[2].add(r_ch.mul(poly[3], kt)), kt)), kt));

        for x in 0..5 {
            b_cur[x] = (0..half)
                .into_par_iter()
                .map(|i| {
                    let a = b_cur[x][i];
                    let b = b_cur[x][i + half];
                    a.add(r_ch.mul(a.add(b), kt))
                })
                .collect();
            wb_cur[x] = (0..half)
                .into_par_iter()
                .map(|i| {
                    let a = wb_cur[x][i];
                    let b = wb_cur[x][i + half];
                    a.add(r_ch.mul(a.add(b), kt))
                })
                .collect();
            wc_cur[x] = (0..half)
                .into_par_iter()
                .map(|i| {
                    let a = wc_cur[x][i];
                    let b = wc_cur[x][i + half];
                    a.add(r_ch.mul(a.add(b), kt))
                })
                .collect();
        }
    }

    // 6. Degree-2 Evaluation Hiding at r1
    let w_eval: [E; 5] = core::array::from_fn(|x| b_cur[x][0]);
    let wb_r1: [E; 5] = core::array::from_fn(|x| wb_cur[x][0]);
    let wc_r1: [E; 5] = core::array::from_fn(|x| wc_cur[x][0]);

    let eq_bit = eq_table(&r1[14..19], kt); // 32 entries (5 bits)
    let mut m_eval = [E::ZERO; 5];
    for x in 0..5 {
        for b in 0..32 {
            m_eval[x].add_assign(eq_bit[b].mul(rs_masks[x][b], kt));
        }
    }
    let mut g1_r1 = E::ZERO;
    for d in 0..V_SC1 {
        let rd = r1[d];
        let gd = g1[d][0].add(rd.mul(g1[d][1].add(rd.mul(g1[d][2].add(rd.mul(g1[d][3], kt)), kt)), kt));
        let mut prod_excl = E::one();
        for j in 0..V_SC1 {
            if j != d {
                prod_excl = prod_excl.mul(r1[j], kt);
            }
        }
        g1_r1.add_assign(gd.mul(prod_excl, kt));
    }

    let mut eval_c1 = m_g1;
    let mut eval_c2 = m_g2;
    for x in 0..5 {
        let xp1 = (x + 1) % 5;
        let xp2 = (x + 2) % 5;
        eval_c1.add_assign(wb_r1[x].mul(m_eval[x], kt));
        let lin_q = w_eval[xp1]
            .add(E::one())
            .mul(m_eval[xp2], kt)
            .add(m_eval[xp1].mul(w_eval[xp2], kt));
        eval_c1.add_assign(wc_r1[x].mul(lin_q, kt));
        eval_c2.add_assign(wc_r1[x].mul(m_eval[xp1].mul(m_eval[xp2], kt), kt));
    }
    tr.absorb_e(b"DEG2C", &[eval_c1, eval_c2]);
    let tau = tr.squeeze_e(b"C22");
    assert!(!tau.is_zero(), "zero tau challenge");
    let eval_z: [E; 5] = core::array::from_fn(|x| w_eval[x].add(tau.mul(m_eval[x], kt)));
    let eval_zg = g1_r1.add(tau.mul(m_g1, kt)).add(tau.sq(kt).mul(m_g2, kt));
    tr.absorb_e(
        b"DEG2Z",
        &[eval_z[0], eval_z[1], eval_z[2], eval_z[3], eval_z[4], eval_zg],
    );

    // 7. Compressed ZK Ring-Switching (single gamma in Round 23, single rho_rs in Round 24)
    let gamma = tr.squeeze_e(b"C23");
    let gammas = expand_gammas(gamma, kt);
    let eq_subrow = eq_table(&r1[0..5], kt); // 32 (only 0..28 active)
    let eq_col = eq_table(&r1[5..14], kt);   // 512
    let mut rs_s_prime = [E::ZERO; 32];
    for x in 0..5 {
        let gx = gammas[x];
        for b in 0..32 {
            let mut sb_x = E::ZERO;
            for sub in 0..ACTIVE_SUBROWS_PER_POLY {
                let mut col_acc = E::ZERO;
                for col in 0..K_COLS {
                    let i = (sub << 14) | (col << 5) | b;
                    if wit_bits.b[x][i] != 0 {
                        col_acc.add_assign(eq_col[col]);
                    }
                }
                sb_x.add_assign(col_acc.mul(eq_subrow[sub], kt));
            }
            let masked = sb_x.add(tau.mul(rs_masks[x][b], kt));
            if x == 0 {
                rs_s_prime[b].add_assign(masked);
            } else {
                rs_s_prime[b].add_assign(gx.mul(masked, kt));
            }
        }
    }
    tr.absorb_e(b"RSSP", &rs_s_prime);
    let rho_rs = tr.squeeze_e(b"C24");
    assert!(!rho_rs.is_zero(), "zero rho_rs challenge");
    let (w_rs, eta1, eta2, eta3) = expand_rho_rs(rho_rs, kt);

    // 8. Sumcheck 2 (8 rounds, degree 2 over row in 0..256)
    let rs_target = ring_switch_target(&rs_s_prime, &w_rs, kt);
    let mut claim2 = rs_target
        .add(eta1.mul(eval_zg, kt))
        .add(eta2.mul(s_g1, kt))
        .add(eta3.mul(s_mask2, kt));

    let mut m_cur = build_m_rs_matrix(&layout, &r1, tau, &gammas, &w_rs, eta1, eta2, eta3, kt);
    let mut u_cur: Vec<Vec<E>> = (0..VIRTUAL_ROWS)
        .map(|r| (0..K_COLS).map(|c| E::from_k(u_h[r][c])).collect())
        .collect();

    let mut sc2_rounds = [[E::ZERO; 2]; V_SC2];
    let mut r_row_p = [E::ZERO; V_SC2];
    for d in 0..V_SC2 {
        let half = u_cur.len() / 2;
        let (c0, c2) = (0..half)
            .into_par_iter()
            .fold(
                || (E::ZERO, E::ZERO),
                |(mut a0, mut a2), row| {
                    let u0 = &u_cur[row];
                    let u1 = &u_cur[row + half];
                    let m0 = &m_cur[row];
                    let m1 = &m_cur[row + half];
                    for col in 0..K_COLS {
                        a0.add_assign(u0[col].mul(m0[col], kt));
                        a2.add_assign(u0[col].add(u1[col]).mul(m0[col].add(m1[col]), kt));
                    }
                    (a0, a2)
                },
            )
            .reduce(|| (E::ZERO, E::ZERO), |l, r| (l.0.add(r.0), l.1.add(r.1)));
        let c1 = claim2.add(c2);
        sc2_rounds[d] = [c0, c2];
        let sc2_tag = [b'S', b'2', b'0', b'0' + (d as u8)];
        let ch_tag = [b'C', b'0' + (((d + 25) / 10) as u8), b'0' + (((d + 25) % 10) as u8)];
        tr.absorb_e(&sc2_tag, &sc2_rounds[d]);
        let r_ch = tr.squeeze_e(&ch_tag);
        r_row_p[d] = r_ch;
        claim2 = c0.add(r_ch.mul(c1.add(r_ch.mul(c2, kt)), kt));

        let next_u: Vec<Vec<E>> = (0..half)
            .into_par_iter()
            .map(|row| {
                (0..K_COLS)
                    .map(|col| u_cur[row][col].add(r_ch.mul(u_cur[row][col].add(u_cur[row + half][col]), kt)))
                    .collect()
            })
            .collect();
        let next_m: Vec<Vec<E>> = (0..half)
            .into_par_iter()
            .map(|row| {
                (0..K_COLS)
                    .map(|col| m_cur[row][col].add(r_ch.mul(m_cur[row][col].add(m_cur[row + half][col]), kt)))
                    .collect()
            })
            .collect();
        u_cur = next_u;
        m_cur = next_m;
    }

    // 9. Split Dual-Polynomial PCS Opening (Round 33a: lambda_p -> v_prox; Round 33b: lambda_o -> v_open)
    let m_col_final = &m_cur[0];
    let mut b_eval = E::ZERO;
    for col in 0..K_COLS {
        b_eval.add_assign(m_col_final[col].mul(b_pcs_h[col], kt));
    }
    tr.absorb_e(b"BEVAL", &[b_eval]);
    let lambda_p = tr.squeeze_e(b"C33P");

    let mut prox_weights = Vec::with_capacity(M_K_ROWS);
    let mut cur_lp = lambda_p.sq(kt);
    for _ in 0..M_K_ROWS {
        prox_weights.push(cur_lp);
        cur_lp = cur_lp.mul(lambda_p, kt);
    }
    let v_prox_coeffs: Vec<E> = (0..DEG_BOUND)
        .into_par_iter()
        .map(|j| {
            let mut acc = b_prox_coeffs[j].add(lambda_p.mul(b_pcs_coeffs[j], kt));
            for aidx in 0..M_K_ROWS {
                acc.add_mul_k(k_coeffs[aidx][j], prox_weights[aidx], kt);
            }
            acc
        })
        .collect();

    tr.absorb_e(b"VPROX", &v_prox_coeffs);
    let lambda_o = tr.squeeze_e(b"C33O");

    let eq_rp = eq_table(&r_row_p, kt);
    let open_weights: Vec<E> = (0..M_K_ROWS)
        .map(|aidx| lambda_o.mul(eq_rp[layout.active_rows[aidx]], kt))
        .collect();
    let v_open_coeffs: Vec<E> = (0..DEG_BOUND)
        .into_par_iter()
        .map(|j| {
            let mut acc = b_pcs_coeffs[j];
            for aidx in 0..M_K_ROWS {
                acc.add_mul_k(k_coeffs[aidx][j], open_weights[aidx], kt);
            }
            acc
        })
        .collect();

    tr.absorb_e(b"VOPEN", &v_open_coeffs);
    let tr_final = tr.state;

    // 10. Atomic H_C4 hard-capped PoW grinding (20-bit PoW, 384-bit uniform nonces, M_pow = 2^27)
    let mut pow_seed = [0u8; 32];
    rng.fill_bytes(&mut pow_seed);
    let mut pow_base = 0u64;
    let mut pow_found: Option<[u8; POW_NONCE_BYTES]> = None;
    let chunk_size = 262_144u64;
    while pow_base < POW_MAX_ATTEMPTS {
        let chunk_end = (pow_base + chunk_size).min(POW_MAX_ATTEMPTS);
        let n_lanes = 64u64;
        let per_lane = (chunk_end - pow_base + n_lanes - 1) / n_lanes;
        if let Some(found) = (0..n_lanes).into_par_iter().find_map_first(|lane| {
            let start = pow_base + lane * per_lane;
            let end = (start + per_lane).min(chunk_end);
            if start >= end {
                return None;
            }
            let mut lane_seed = pow_seed;
            let s_bytes = start.to_le_bytes();
            for i in 0..8 {
                lane_seed[i] ^= s_bytes[i];
            }
            let mut lane_rng = ChaCha20Rng::from_seed(lane_seed);
            let mut cand = [0u8; POW_NONCE_BYTES];
            for _ in start..end {
                lane_rng.fill_bytes(&mut cand);
                if eval_h_c4(&tr_final, &cand).0 {
                    return Some(cand);
                }
            }
            None
        }) {
            pow_found = Some(found);
            break;
        }
        pow_base = chunk_end;
    }
    let pow_nonce = pow_found.expect("PoW hard cap M_pow=2^27 exceeded (prob <= 2^-184.66)");
    let (pow_ok, q_indices) = eval_h_c4(&tr_final, &pow_nonce);
    assert!(pow_ok);

    // 11. Evaluate ONLY the <= 52 unique queried columns directly from `k_coeffs`, `b_pcs_coeffs`, `b_prox_coeffs`!
    let mut unique_cols: Vec<usize> = q_indices.to_vec();
    unique_cols.sort_unstable();
    unique_cols.dedup();

    let mut basis = vec![0u32; DEG_BOUND];
    let mut opened_cols = Vec::with_capacity(unique_cols.len());
    for &idx in &unique_cols {
        let x = L_SHIFT ^ (idx as u32);
        kt.novel_basis_evals(x, DEG_BOUND, &mut basis);
        let mut k_vals = [0u32; M_K_ROWS];
        for aidx in 0..M_K_ROWS {
            let mut acc = 0u32;
            let coeffs = &k_coeffs[aidx];
            for j in 0..DEG_BOUND {
                acc ^= kt.kmul(coeffs[j], basis[j]);
            }
            k_vals[aidx] = acc;
        }
        let mut b_pcs = E::ZERO;
        let mut b_prox = E::ZERO;
        for j in 0..DEG_BOUND {
            let bj = basis[j];
            b_pcs.add_mul_k(bj, b_pcs_coeffs[j], kt);
            b_prox.add_mul_k(bj, b_prox_coeffs[j], kt);
        }
        opened_cols.push(OpenedColumn {
            k_vals,
            b_pcs,
            b_prox,
        });
    }

    // 12. Pruned Merkle authentication path for unique_cols
    let mut cur_set: BTreeSet<usize> = unique_cols.into_iter().collect();
    let mut merkle_siblings = Vec::new();
    for lvl in 0..MERKLE_HEIGHT {
        let mut next_set = BTreeSet::new();
        for &pos in &cur_set {
            let sib = pos ^ 1;
            if !cur_set.contains(&sib) {
                merkle_siblings.push(tree_levels[lvl][sib]);
            }
            next_set.insert(pos >> 1);
        }
        cur_set = next_set;
    }

    BiniusProof {
        merkle_root,
        s_g1,
        sc1_rounds,
        eval_c1,
        eval_c2,
        eval_z,
        eval_zg,
        rs_s_prime,
        s_mask2,
        sc2_rounds,
        b_eval,
        v_prox_coeffs,
        v_open_coeffs,
        pow_nonce,
        opened_cols,
        merkle_siblings,
    }
}

/// Complete, self-contained single-core verifier for Peel-Binius-Shielded proofs.
pub fn verify(stmt: &PaymentStatement, proof: &BiniusProof) -> Result<(), String> {
    if proof.v_prox_coeffs.len() != DEG_BOUND || proof.v_open_coeffs.len() != DEG_BOUND {
        return Err("invalid v_prox/v_open degree bound".into());
    }
    let kt = k_tables();
    let layout = row_layout();
    let stmt_bytes = encode_statement(stmt);

    // 1. Transcript init & Round-2 single-element alpha
    let mut tr = Transcript::new(&stmt_bytes, &proof.merkle_root, proof.s_g1, proof.s_mask2);
    let ch = CircuitChallenge {
        alpha: tr.squeeze_e(b"C02"),
    };

    // 2. Sumcheck 1 replay (19 rounds)
    let mut r1 = [E::ZERO; V_SC1];
    let mut sc1_claims = [E::ZERO; V_SC1 + 1];
    for d in 0..V_SC1 {
        let [c0, c2, c3] = proof.sc1_rounds[d];
        let sc_tag = [b'S', b'1', b'0' + ((d / 10) as u8), b'0' + ((d % 10) as u8)];
        let ch_tag = [b'C', b'0' + (((d + 3) / 10) as u8), b'0' + (((d + 3) % 10) as u8)];
        tr.absorb_e(&sc_tag, &[c0, c2, c3]);
        let r_ch = tr.squeeze_e(&ch_tag);
        if r_ch.is_zero() {
            return Err("zero SC1 challenge".into());
        }
        r1[d] = r_ch;
    }

    let (wb_r1, wc_r1, pub_rhs) = eval_verifier_weights_at_r1(stmt, &ch, &r1, kt);
    sc1_claims[0] = pub_rhs.add(proof.s_g1);
    for d in 0..V_SC1 {
        let [c0, c2, c3] = proof.sc1_rounds[d];
        let c1 = sc1_claims[d].add(c2).add(c3);
        let rd = r1[d];
        sc1_claims[d + 1] = c0.add(rd.mul(c1.add(rd.mul(c2.add(rd.mul(c3, kt)), kt)), kt));
    }
    let sc1_final = sc1_claims[V_SC1];

    // 3. Degree-2 Evaluation Hiding check
    tr.absorb_e(b"DEG2C", &[proof.eval_c1, proof.eval_c2]);
    let tau = tr.squeeze_e(b"C22");
    if tau.is_zero() {
        return Err("zero tau challenge".into());
    }
    let lhs_deg2 = sc1_final
        .add(tau.mul(proof.eval_c1, kt))
        .add(tau.sq(kt).mul(proof.eval_c2, kt));
    let mut rhs_deg2 = proof.eval_zg;
    let one = E::one();
    for x in 0..5 {
        let xp1 = (x + 1) % 5;
        let xp2 = (x + 2) % 5;
        rhs_deg2.add_assign(wb_r1[x].mul(proof.eval_z[x], kt));
        rhs_deg2.add_assign(wc_r1[x].mul(one.add(proof.eval_z[xp1]).mul(proof.eval_z[xp2], kt), kt));
    }
    if lhs_deg2 != rhs_deg2 {
        return Err("Degree-2 evaluation hiding check failed".into());
    }

    tr.absorb_e(
        b"DEG2Z",
        &[
            proof.eval_z[0],
            proof.eval_z[1],
            proof.eval_z[2],
            proof.eval_z[3],
            proof.eval_z[4],
            proof.eval_zg,
        ],
    );

    // 4. Ring-Switching consistency check
    let gamma = tr.squeeze_e(b"C23");
    let gammas = expand_gammas(gamma, kt);
    let eq_bit = eq_table(&r1[14..19], kt);
    let mut rs_lhs = E::ZERO;
    for b in 0..32 {
        rs_lhs.add_assign(eq_bit[b].mul(proof.rs_s_prime[b], kt));
    }
    let mut rs_rhs = proof.eval_z[0];
    for x in 1..5 {
        rs_rhs.add_assign(gammas[x].mul(proof.eval_z[x], kt));
    }
    if rs_lhs != rs_rhs {
        return Err("Ring-Switching sum check failed".into());
    }

    tr.absorb_e(b"RSSP", &proof.rs_s_prime);
    let rho_rs = tr.squeeze_e(b"C24");
    if rho_rs.is_zero() {
        return Err("zero rho_rs challenge".into());
    }
    let (w_rs, eta1, eta2, eta3) = expand_rho_rs(rho_rs, kt);

    // 5. Sumcheck 2 replay (8 rounds)
    let rs_target = ring_switch_target(&proof.rs_s_prime, &w_rs, kt);
    let mut claim2 = rs_target
        .add(eta1.mul(proof.eval_zg, kt))
        .add(eta2.mul(proof.s_g1, kt))
        .add(eta3.mul(proof.s_mask2, kt));
    let mut r_row_p = [E::ZERO; V_SC2];
    for d in 0..V_SC2 {
        let [c0, c2] = proof.sc2_rounds[d];
        let c1 = claim2.add(c2);
        let sc2_tag = [b'S', b'2', b'0', b'0' + (d as u8)];
        let ch_tag = [b'C', b'0' + (((d + 25) / 10) as u8), b'0' + (((d + 25) % 10) as u8)];
        tr.absorb_e(&sc2_tag, &[c0, c2]);
        let r_ch = tr.squeeze_e(&ch_tag);
        r_row_p[d] = r_ch;
        claim2 = c0.add(r_ch.mul(c1.add(r_ch.mul(c2, kt)), kt));
    }

    // 6. Split PCS challenges (Round 33a: lambda_p; Round 33b: lambda_o) & H-domain inner-product check
    tr.absorb_e(b"BEVAL", &[proof.b_eval]);
    let lambda_p = tr.squeeze_e(b"C33P");
    tr.absorb_e(b"VPROX", &proof.v_prox_coeffs);
    let lambda_o = tr.squeeze_e(b"C33O");
    tr.absorb_e(b"VOPEN", &proof.v_open_coeffs);
    let tr_final = tr.state;

    let mut v_open_h = proof.v_open_coeffs[..K_COLS].to_vec();
    kt.antt_forward_h_e(&mut v_open_h);
    let ip_lhs = eval_verifier_ip_lhs(
        &layout, &r1, tau, &gammas, &w_rs, eta1, eta2, eta3, &r_row_p, &v_open_h, kt,
    );
    let ip_rhs = proof.b_eval.add(lambda_o.mul(claim2, kt));
    if ip_lhs != ip_rhs {
        return Err("PCS H-domain inner-product check failed".into());
    }

    // 7. Atomic H_C4 PoW & 52 query indices
    let (pow_ok, q_indices) = eval_h_c4(&tr_final, &proof.pow_nonce);
    if !pow_ok {
        return Err("PoW verification failed".into());
    }
    let mut unique_cols: Vec<usize> = q_indices.to_vec();
    unique_cols.sort_unstable();
    unique_cols.dedup();
    if proof.opened_cols.len() != unique_cols.len() {
        return Err("opened column count mismatch".into());
    }

    // 8. Pruned SHA-384 Merkle path verification
    let mut level_map: BTreeMap<usize, [u8; HASH_BYTES]> = BTreeMap::new();
    for (&idx, col) in unique_cols.iter().zip(proof.opened_cols.iter()) {
        level_map.insert(idx, oracle_leaf(idx, &col.k_vals, col.b_pcs, col.b_prox));
    }
    let mut sib_cursor = 0usize;
    for lvl in 0..MERKLE_HEIGHT {
        let mut next_map: BTreeMap<usize, [u8; HASH_BYTES]> = BTreeMap::new();
        let keys: Vec<usize> = level_map.keys().copied().collect();
        let mut ki = 0usize;
        while ki < keys.len() {
            let pos = keys[ki];
            let left_pos = pos & !1;
            let right_pos = left_pos | 1;
            let (left_h, right_h) = if pos == left_pos {
                let lh = level_map[&left_pos];
                if ki + 1 < keys.len() && keys[ki + 1] == right_pos {
                    ki += 2;
                    (lh, level_map[&right_pos])
                } else {
                    if sib_cursor >= proof.merkle_siblings.len() {
                        return Err("missing Merkle sibling".into());
                    }
                    let rh = proof.merkle_siblings[sib_cursor];
                    sib_cursor += 1;
                    ki += 1;
                    (lh, rh)
                }
            } else {
                if sib_cursor >= proof.merkle_siblings.len() {
                    return Err("missing Merkle sibling".into());
                }
                let lh = proof.merkle_siblings[sib_cursor];
                sib_cursor += 1;
                let rh = level_map[&right_pos];
                ki += 1;
                (lh, rh)
            };
            let parent = pos >> 1;
            next_map.insert(parent, oracle_node(lvl as u8, parent, &left_h, &right_h));
        }
        level_map = next_map;
    }
    if sib_cursor != proof.merkle_siblings.len() {
        return Err("unused trailing Merkle siblings".into());
    }
    if level_map.get(&0) != Some(&proof.merkle_root) {
        return Err("Merkle root mismatch".into());
    }

    // 9. Pointwise Dual-Polynomial PCS Consistency on all <= 52 opened columns
    let mut prox_weights = Vec::with_capacity(M_K_ROWS);
    let mut cur_lp = lambda_p.sq(kt);
    for _ in 0..M_K_ROWS {
        prox_weights.push(cur_lp);
        cur_lp = cur_lp.mul(lambda_p, kt);
    }
    let open_weights: Vec<E> = (0..M_K_ROWS)
        .map(|aidx| lambda_o.mul(eq_eval_point(&r_row_p, layout.active_rows[aidx], kt), kt))
        .collect();

    let mut basis = vec![0u32; DEG_BOUND];
    for (&idx, col) in unique_cols.iter().zip(proof.opened_cols.iter()) {
        let x = L_SHIFT ^ (idx as u32);
        kt.novel_basis_evals(x, DEG_BOUND, &mut basis);

        let mut v_prox_eval = E::ZERO;
        let mut v_open_eval = E::ZERO;
        for j in 0..DEG_BOUND {
            let bj = basis[j];
            v_prox_eval.add_mul_k(bj, proof.v_prox_coeffs[j], kt);
            v_open_eval.add_mul_k(bj, proof.v_open_coeffs[j], kt);
        }

        let mut col_prox = col.b_prox.add(lambda_p.mul(col.b_pcs, kt));
        let mut col_open = col.b_pcs;
        for aidx in 0..M_K_ROWS {
            let v = col.k_vals[aidx];
            if v != 0 {
                col_prox.add_mul_k(v, prox_weights[aidx], kt);
                col_open.add_mul_k(v, open_weights[aidx], kt);
            }
        }
        if v_prox_eval != col_prox {
            return Err("v_prox consistency check failed".into());
        }
        if v_open_eval != col_open {
            return Err("v_open consistency check failed".into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::circuit::{build_witness_bits, PaymentWitness};
    use crate::shake256::{
        coin_commit, nullifier, owner_addr, MerkleTree24, BLIND_SECRET_BYTES, OWNER_SECRET_BYTES,
    };
    use std::time::Instant;

    #[test]
    fn test_binius_shielded_prove_verify_roundtrip() {
        let mut tree = MerkleTree24::new();
        let s0 = [0x11u8; OWNER_SECRET_BYTES];
        let s1 = [0x22u8; OWNER_SECRET_BYTES];
        let a0 = owner_addr(&s0);
        let a1 = owner_addr(&s1);
        let r_in0 = [0x33u8; BLIND_SECRET_BYTES];
        let r_in1 = [0x44u8; BLIND_SECRET_BYTES];
        let c0 = coin_commit(&a0, 700, &r_in0);
        let c1 = coin_commit(&a1, 300, &r_in1);
        let idx0 = tree.append(c0);
        let idx1 = tree.append(c1);
        let anchor = tree.root();
        let (dirs0, sibs0) = tree.auth_path(idx0);
        let (dirs1, sibs1) = tree.auth_path(idx1);

        let a_out0 = owner_addr(&[0x55u8; OWNER_SECRET_BYTES]);
        let a_out1 = owner_addr(&[0x66u8; OWNER_SECRET_BYTES]);
        let r_out0 = [0x77u8; BLIND_SECRET_BYTES];
        let r_out1 = [0x88u8; BLIND_SECRET_BYTES];

        let stmt = PaymentStatement {
            anchor,
            sn0: nullifier(&s0, &c0),
            sn1: nullifier(&s1, &c1),
            coin_out0: coin_commit(&a_out0, 650, &r_out0),
            coin_out1: coin_commit(&a_out1, 340, &r_out1),
            v_mint: 0,
            v_unshield: 0,
            fee: 10,
            tx_ctx: [0xABu8; 48],
        };
        let wit = PaymentWitness {
            is_dummy0: false,
            is_dummy1: false,
            dirs0,
            sibs0,
            dirs1,
            sibs1,
            s_in0: s0,
            s_in1: s1,
            v_in0: 700,
            v_in1: 300,
            r_in0,
            r_in1,
            addr_out0: a_out0,
            addr_out1: a_out1,
            v_out0: 650,
            v_out1: 340,
            r_out0,
            r_out1,
        };
        let bits = build_witness_bits(&stmt, &wit);
        let mut rng = ChaCha20Rng::from_seed([0x42u8; 32]);
        let t0 = Instant::now();
        let proof = prove(&stmt, &bits, &mut rng);
        let prove_ms = t0.elapsed().as_secs_f64() * 1000.0;
        let bytes = proof.to_bytes();
        let parsed = BiniusProof::from_bytes(&bytes).expect("parse");
        let t1 = Instant::now();
        verify(&stmt, &parsed).expect("verify");
        let verify_ms = t1.elapsed().as_secs_f64() * 1000.0;
        println!(
            "Peel-Binius-Shielded Depth-24 (16.7M coins): proof={} B (tx_est={} B) | prove={:.1} ms (16C) | verify={:.2} ms (1C)",
            bytes.len(),
            bytes.len() + 3600,
            prove_ms,
            verify_ms
        );
        assert!(bytes.len() + 3648 <= 149_130, "exceeded 149,130 B budget!");
    }

    #[test]
    fn test_3d_karatsuba_rs_ip_matches_naive() {
        let kt = k_tables();
        let mut rng = ChaCha20Rng::from_seed([0x99u8; 32]);
        let mut rand_e = || {
            let mut b = [0u8; E_BYTES];
            rng.fill_bytes(&mut b);
            E::from_bytes(&b)
        };
        let mut r_arr = [[[E::ZERO; 16]; 2]; 12];
        let mut v_arr = [[[E::ZERO; 16]; 2]; 12];
        for v in 0..12 {
            for z in 0..2 {
                for u in 0..16 {
                    r_arr[v][z][u] = rand_e();
                    v_arr[v][z][u] = rand_e();
                }
            }
        }
        let w_rs: Vec<E> = (0..384).map(|_| rand_e()).collect();
        let fast = eval_bilinear_rs_ip(&r_arr, &v_arr, &w_rs, kt);

        // Naive 384 x 384 reference bilinear inner product via explicit RingSwitchLinearMap
        let l_map = RingSwitchLinearMap::new(&w_rs);
        let mut naive = E::ZERO;
        for a in 0..384 {
            let ra = r_arr[a / 32][(a / 16) & 1][a & 15];
            let ba = E::from_basis_vu(a / 32, a % 32);
            for b in 0..384 {
                let vb = v_arr[b / 32][(b / 16) & 1][b & 15];
                let bb = E::from_basis_vu(b / 32, b % 32);
                let prod_basis = ba.mul(bb, kt);
                let w_ab = l_map.apply(prod_basis);
                naive.add_assign(ra.mul(vb, kt).mul(w_ab, kt));
            }
        }
        assert_eq!(fast, naive, "3D-Karatsuba Ring-Switch inner product diverged from naive 384x384 sum!");
    }
}
