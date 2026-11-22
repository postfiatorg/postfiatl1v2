//! 2-in/2-out global-pool shielded payment circuit over GF(2) using ONLY NIST FIPS-202 SHAKE256
//! (56 Keccak-f[1600] permutations = 8 base + 2x24 Depth-24 global Merkle paths for 16,777,216 coins,
//!  9,216 packed Merkle conditional-swap mux rows, 6,144 64-row-aligned sibling witness rows,
//!  448 native is_dummy gating rows, and 193 65-bit ripple-carry conservation rows).
//!
//! Witness layout: 5 bit-arrays B_0..B_4 of length 2^19 = 524,288 virtual bits each
//! (447,014 active rows packed into 28 active K-subrows x 512 cols x 32 bits/elem;
//!  trailing 4 zero subrows per poly omitted from MT_1 -> M_WIT_ROWS = 140 K-rows).
//!
//! Every quadratic gate in the entire circuit uses ONLY the 5 Keccak Chi monomials:
//!   M_x(i) = (1 + B_{(x+1)%5}(i)) * B_{(x+2)%5}(i),   x in 0..5.
//!
//! Single-challenge Round-2 batching:
//!   ONE single element `alpha in E = GF(2^384)` assigns a distinct univariate power
//!   alpha^c (0 <= c < 2,192,578) to every scalar constraint across Groups A, B, and C,
//!   with O(1600 + 1288 + 48*26) factored verifier MLE evaluation (< 2 ms single-core!).

use crate::field::{eq_table, eq_table_scaled_limit, KTables, E};
use crate::shake256::{
    coin_commit, keccak_f1600_trace, merkle_node, nullifier, owner_addr, shake256_pad_1perm,
    trp_input_bits, ADDR_BYTES, BLIND_SECRET_BYTES, COIN_BYTES, KECCAK_RC, MERKLE_DEPTH,
    NODE_BYTES, OWNER_SECRET_BYTES, SN_BYTES, TAG_ADDR, TAG_COIN, TAG_NODE, TAG_SN,
};

pub const NUM_PERMS: usize = 8 + 2 * MERKLE_DEPTH; // 56
pub const V_SC1: usize = 19;
pub const NUM_BITS_PER_POLY: usize = 1usize << V_SC1; // 524,288

pub const ROW_MUX_BASE: usize = NUM_PERMS * 7680; // 430,080 (9,216 rows, 64-row aligned)
pub const MUX_ROWS_PER_LEVEL: usize = 192;        // 384 bits / 2 bits per row = 3 * 64
pub const ROW_SIB_BASE: usize = ROW_MUX_BASE + 2 * MERKLE_DEPTH * MUX_ROWS_PER_LEVEL; // 439,296 (6,144 rows, 64-row aligned)
pub const SIB_ROWS_PER_LEVEL: usize = 128;        // 384 bits / 3 bits per row = 2 * 64
pub const ROW_DUMMY_VAL_BASE: usize = ROW_SIB_BASE + 2 * MERKLE_DEPTH * SIB_ROWS_PER_LEVEL; // 445,440 (64 rows)
pub const ROW_DUMMY_ROOT_BASE: usize = ROW_DUMMY_VAL_BASE + 2 * 32; // 445,504 (384 rows)
pub const ROW_ADDER_LHS1: usize = ROW_DUMMY_ROOT_BASE + 2 * 192;    // 445,888 (64 rows: v_in0 + v_in1)
pub const ROW_ADDER_LHS: usize = ROW_ADDER_LHS1 + 64;               // 445,952 (65 rows: s_in01 + v_mint)
pub const ROW_ADDER_MID: usize = ROW_ADDER_LHS + 65;                // 446,017 (64 rows: v_out0 + v_out1)
pub const ROW_ADDER_RHS: usize = ROW_ADDER_MID + 64;                // 446,081 (65 rows: s_out01 + pub_exit)
pub const ROW_PRIV_BASE: usize = ROW_ADDER_RHS + 65;                // 446,146 (868 rows)

// Bit offsets inside the 4,338-bit private input region (starting at ROW_PRIV_BASE, 5 bits/row)
pub const OFF_S_IN0: usize = 0;        // 384 bits (48 B)
pub const OFF_S_IN1: usize = 384;      // 384 bits (48 B)
pub const OFF_V_IN0: usize = 768;      // 64 bits  (8 B)
pub const OFF_V_IN1: usize = 832;      // 64 bits  (8 B)
pub const OFF_R_IN0: usize = 896;      // 624 bits (78 B)
pub const OFF_R_IN1: usize = 1520;     // 624 bits (78 B)
pub const OFF_A_OUT0: usize = 2144;    // 384 bits (48 B, PRIVATE stealth recipient address!)
pub const OFF_A_OUT1: usize = 2528;    // 384 bits (48 B, PRIVATE stealth recipient address!)
pub const OFF_V_OUT0: usize = 2912;    // 64 bits  (8 B)
pub const OFF_V_OUT1: usize = 2976;    // 64 bits  (8 B)
pub const OFF_R_OUT0: usize = 3040;    // 624 bits (78 B)
pub const OFF_R_OUT1: usize = 3664;    // 624 bits (78 B)
pub const OFF_IS_DUMMY0: usize = 4288; // 1 bit
pub const OFF_IS_DUMMY1: usize = 4289; // 1 bit
pub const OFF_DIR0: usize = 4290;      // 24 bits
pub const OFF_DIR1: usize = 4314;      // 24 bits
pub const TOTAL_PRIV_BITS: usize = 4338;
pub const PRIV_ROWS: usize = (TOTAL_PRIV_BITS + 4) / 5; // 868
pub const ACTIVE_ROWS: usize = ROW_PRIV_BASE + PRIV_ROWS; // 447,014
pub const ACTIVE_SUBROWS_PER_POLY: usize = (ACTIVE_ROWS + 16383) / 16384; // 28

#[inline(always)]
pub const fn keccak_row(perm: usize, round: usize, y: usize, z: usize) -> usize {
    perm * 7680 + round * 320 + y * 64 + z
}

#[inline(always)]
pub const fn mux_row(input_b: usize, lvl: usize, k: usize) -> usize {
    ROW_MUX_BASE + (input_b * MERKLE_DEPTH + lvl) * MUX_ROWS_PER_LEVEL + k
}

/// 64-row-aligned sibling bit location: 3 bits/row (`B_0, B_1, B_2`), 128 rows/level (2 x 64).
#[inline(always)]
pub const fn sib_loc(input_b: usize, lvl: usize, bit_idx: usize) -> (usize, usize) {
    (
        bit_idx % 3,
        ROW_SIB_BASE + (input_b * MERKLE_DEPTH + lvl) * SIB_ROWS_PER_LEVEL + bit_idx / 3,
    )
}

#[inline(always)]
pub const fn dummy_val_row(input_b: usize, k: usize) -> usize {
    ROW_DUMMY_VAL_BASE + input_b * 32 + k
}

#[inline(always)]
pub const fn dummy_root_row(input_b: usize, k: usize) -> usize {
    ROW_DUMMY_ROOT_BASE + input_b * 192 + k
}

#[inline(always)]
pub const fn priv_loc(bit_offset: usize) -> (usize, usize) {
    (bit_offset % 5, ROW_PRIV_BASE + bit_offset / 5)
}

#[derive(Clone, Debug)]
pub struct PaymentStatement {
    pub anchor: [u8; NODE_BYTES],
    pub sn0: [u8; SN_BYTES],
    pub sn1: [u8; SN_BYTES],
    pub coin_out0: [u8; COIN_BYTES],
    pub coin_out1: [u8; COIN_BYTES],
    pub v_mint: u64,
    pub v_unshield: u64,
    pub fee: u64,
    pub tx_ctx: [u8; 48],
}

impl PaymentStatement {
    /// Combined public transparent outflow `fee + v_unshield` (enforced `< 2^64` by Rule R5).
    #[inline]
    pub fn pub_exit(&self) -> u64 {
        self.fee
            .checked_add(self.v_unshield)
            .expect("Rule R5: fee + v_unshield must fit in u64")
    }
}

#[derive(Clone, Debug)]
pub struct PaymentWitness {
    pub is_dummy0: bool,
    pub is_dummy1: bool,
    pub dirs0: [u8; MERKLE_DEPTH],
    pub sibs0: [[u8; NODE_BYTES]; MERKLE_DEPTH],
    pub dirs1: [u8; MERKLE_DEPTH],
    pub sibs1: [[u8; NODE_BYTES]; MERKLE_DEPTH],
    pub s_in0: [u8; OWNER_SECRET_BYTES],
    pub s_in1: [u8; OWNER_SECRET_BYTES],
    pub v_in0: u64,
    pub v_in1: u64,
    pub r_in0: [u8; BLIND_SECRET_BYTES],
    pub r_in1: [u8; BLIND_SECRET_BYTES],
    pub addr_out0: [u8; ADDR_BYTES],
    pub addr_out1: [u8; ADDR_BYTES],
    pub v_out0: u64,
    pub v_out1: u64,
    pub r_out0: [u8; BLIND_SECRET_BYTES],
    pub r_out1: [u8; BLIND_SECRET_BYTES],
}

#[derive(Clone)]
pub struct CircuitWitnessBits {
    pub b: [Vec<u8>; 5],
}

#[inline(always)]
fn get_byte_bit(bytes: &[u8], bit_idx: usize) -> u8 {
    (bytes[bit_idx / 8] >> (bit_idx % 8)) & 1
}

#[derive(Clone, Copy, Debug)]
pub enum InputBitSrc {
    Const(u8),
    Priv(usize),
    PermOut(usize, usize, usize, usize),
    MerkleLeft {
        src_perm: usize,
        x: usize,
        y: usize,
        z: usize,
        mux_x: usize,
        mux_row: usize,
    },
    MerkleRight {
        sib_x: usize,
        sib_row: usize,
        mux_x: usize,
        mux_row: usize,
    },
}

pub fn perm_input_spec(p: usize) -> Vec<InputBitSrc> {
    let mut spec = vec![InputBitSrc::Const(0); 1600];

    let set_bytes_const = |spec: &mut [InputBitSrc], byte_off: usize, data: &[u8]| {
        for (bi, &byte) in data.iter().enumerate() {
            for bit in 0..8 {
                spec[(byte_off + bi) * 8 + bit] = InputBitSrc::Const((byte >> bit) & 1);
            }
        }
    };
    let set_priv = |spec: &mut [InputBitSrc], byte_off: usize, priv_bit_off: usize, nbytes: usize| {
        for b in 0..(nbytes * 8) {
            spec[byte_off * 8 + b] = InputBitSrc::Priv(priv_bit_off + b);
        }
    };
    let set_perm_out = |spec: &mut [InputBitSrc], byte_off: usize, src_perm: usize, nbytes: usize| {
        for b in 0..(nbytes * 8) {
            let lane = b / 64;
            let x = lane % 5;
            let y = lane / 5;
            let z = b % 64;
            spec[byte_off * 8 + b] = InputBitSrc::PermOut(src_perm, x, y, z);
        }
    };
    let set_shake_pad = |spec: &mut [InputBitSrc], msg_len_bytes: usize| {
        for bit in 0..8 {
            spec[msg_len_bytes * 8 + bit] = InputBitSrc::Const((0x1F >> bit) & 1);
        }
        spec[135 * 8 + 7] = InputBitSrc::Const(1);
    };

    match p {
        0 => {
            set_bytes_const(&mut spec, 0, &[TAG_ADDR]);
            set_priv(&mut spec, 1, OFF_S_IN0, OWNER_SECRET_BYTES);
            set_shake_pad(&mut spec, 1 + OWNER_SECRET_BYTES);
        }
        1 => {
            set_bytes_const(&mut spec, 0, &[TAG_COIN]);
            set_perm_out(&mut spec, 1, 0, ADDR_BYTES);
            set_priv(&mut spec, 49, OFF_V_IN0, 8);
            set_priv(&mut spec, 57, OFF_R_IN0, BLIND_SECRET_BYTES);
            set_shake_pad(&mut spec, 135);
        }
        2 => {
            set_bytes_const(&mut spec, 0, &[TAG_SN]);
            set_priv(&mut spec, 1, OFF_S_IN0, OWNER_SECRET_BYTES);
            set_perm_out(&mut spec, 49, 1, COIN_BYTES);
            set_shake_pad(&mut spec, 97);
        }
        3 => {
            set_bytes_const(&mut spec, 0, &[TAG_ADDR]);
            set_priv(&mut spec, 1, OFF_S_IN1, OWNER_SECRET_BYTES);
            set_shake_pad(&mut spec, 1 + OWNER_SECRET_BYTES);
        }
        4 => {
            set_bytes_const(&mut spec, 0, &[TAG_COIN]);
            set_perm_out(&mut spec, 1, 3, ADDR_BYTES);
            set_priv(&mut spec, 49, OFF_V_IN1, 8);
            set_priv(&mut spec, 57, OFF_R_IN1, BLIND_SECRET_BYTES);
            set_shake_pad(&mut spec, 135);
        }
        5 => {
            set_bytes_const(&mut spec, 0, &[TAG_SN]);
            set_priv(&mut spec, 1, OFF_S_IN1, OWNER_SECRET_BYTES);
            set_perm_out(&mut spec, 49, 4, COIN_BYTES);
            set_shake_pad(&mut spec, 97);
        }
        6 => {
            set_bytes_const(&mut spec, 0, &[TAG_COIN]);
            set_priv(&mut spec, 1, OFF_A_OUT0, ADDR_BYTES);
            set_priv(&mut spec, 49, OFF_V_OUT0, 8);
            set_priv(&mut spec, 57, OFF_R_OUT0, BLIND_SECRET_BYTES);
            set_shake_pad(&mut spec, 135);
        }
        7 => {
            set_bytes_const(&mut spec, 0, &[TAG_COIN]);
            set_priv(&mut spec, 1, OFF_A_OUT1, ADDR_BYTES);
            set_priv(&mut spec, 49, OFF_V_OUT1, 8);
            set_priv(&mut spec, 57, OFF_R_OUT1, BLIND_SECRET_BYTES);
            set_shake_pad(&mut spec, 135);
        }
        8..56 => {
            let rel = p - 8;
            let input_b = rel / MERKLE_DEPTH;
            let lvl = rel % MERKLE_DEPTH;
            let src_perm = if lvl == 0 {
                if input_b == 0 { 1 } else { 4 }
            } else {
                p - 1
            };

            set_bytes_const(&mut spec, 0, &[TAG_NODE]);
            for i in 0..384 {
                let k = i / 2;
                let mux_x = if (i & 1) == 0 { 4 } else { 1 };
                let r_mux = mux_row(input_b, lvl, k);
                let lane = i / 64;
                let x = lane % 5;
                let y = lane / 5;
                let z = i % 64;
                let (sib_x, sib_row) = sib_loc(input_b, lvl, i);
                spec[8 + i] = InputBitSrc::MerkleLeft {
                    src_perm,
                    x,
                    y,
                    z,
                    mux_x,
                    mux_row: r_mux,
                };
                spec[49 * 8 + i] = InputBitSrc::MerkleRight {
                    sib_x,
                    sib_row,
                    mux_x,
                    mux_row: r_mux,
                };
            }
            set_shake_pad(&mut spec, 97);
        }
        _ => unreachable!(),
    }
    spec
}

pub fn build_witness_bits(stmt: &PaymentStatement, wit: &PaymentWitness) -> CircuitWitnessBits {
    let mut b = [
        vec![0u8; NUM_BITS_PER_POLY],
        vec![0u8; NUM_BITS_PER_POLY],
        vec![0u8; NUM_BITS_PER_POLY],
        vec![0u8; NUM_BITS_PER_POLY],
        vec![0u8; NUM_BITS_PER_POLY],
    ];

    let a0 = owner_addr(&wit.s_in0);
    let a1 = owner_addr(&wit.s_in1);
    let coin_in0 = coin_commit(&a0, wit.v_in0, &wit.r_in0);
    let coin_in1 = coin_commit(&a1, wit.v_in1, &wit.r_in1);
    let sn0 = nullifier(&wit.s_in0, &coin_in0);
    let sn1 = nullifier(&wit.s_in1, &coin_in1);
    let coin_out0 = coin_commit(&wit.addr_out0, wit.v_out0, &wit.r_out0);
    let coin_out1 = coin_commit(&wit.addr_out1, wit.v_out1, &wit.r_out1);

    debug_assert_eq!(sn0, stmt.sn0);
    debug_assert_eq!(sn1, stmt.sn1);
    debug_assert_eq!(coin_out0, stmt.coin_out0);
    debug_assert_eq!(coin_out1, stmt.coin_out1);

    let record_perm = |b: &mut [Vec<u8>; 5], p: usize, msg: &[u8]| {
        let mut st = shake256_pad_1perm(msg);
        let trace = keccak_f1600_trace(&mut st);
        for r in 0..24 {
            for y in 0..5 {
                for z in 0..64 {
                    let row = keccak_row(p, r, y, z);
                    for x in 0..5 {
                        b[x][row] = ((trace[r][x + 5 * y] >> z) & 1) as u8;
                    }
                }
            }
        }
    };

    let mut m0 = [0u8; 49];
    m0[0] = TAG_ADDR;
    m0[1..].copy_from_slice(&wit.s_in0);
    record_perm(&mut b, 0, &m0);

    let mut m1 = [0u8; 135];
    m1[0] = TAG_COIN;
    m1[1..49].copy_from_slice(&a0);
    m1[49..57].copy_from_slice(&wit.v_in0.to_le_bytes());
    m1[57..135].copy_from_slice(&wit.r_in0);
    record_perm(&mut b, 1, &m1);

    let mut m2 = [0u8; 97];
    m2[0] = TAG_SN;
    m2[1..49].copy_from_slice(&wit.s_in0);
    m2[49..97].copy_from_slice(&coin_in0);
    record_perm(&mut b, 2, &m2);

    let mut m3 = [0u8; 49];
    m3[0] = TAG_ADDR;
    m3[1..].copy_from_slice(&wit.s_in1);
    record_perm(&mut b, 3, &m3);

    let mut m4 = [0u8; 135];
    m4[0] = TAG_COIN;
    m4[1..49].copy_from_slice(&a1);
    m4[49..57].copy_from_slice(&wit.v_in1.to_le_bytes());
    m4[57..135].copy_from_slice(&wit.r_in1);
    record_perm(&mut b, 4, &m4);

    let mut m5 = [0u8; 97];
    m5[0] = TAG_SN;
    m5[1..49].copy_from_slice(&wit.s_in1);
    m5[49..97].copy_from_slice(&coin_in1);
    record_perm(&mut b, 5, &m5);

    let mut m6 = [0u8; 135];
    m6[0] = TAG_COIN;
    m6[1..49].copy_from_slice(&wit.addr_out0);
    m6[49..57].copy_from_slice(&wit.v_out0.to_le_bytes());
    m6[57..135].copy_from_slice(&wit.r_out0);
    record_perm(&mut b, 6, &m6);

    let mut m7 = [0u8; 135];
    m7[0] = TAG_COIN;
    m7[1..49].copy_from_slice(&wit.addr_out1);
    m7[49..57].copy_from_slice(&wit.v_out1.to_le_bytes());
    m7[57..135].copy_from_slice(&wit.r_out1);
    record_perm(&mut b, 7, &m7);

    // 48 Merkle path permutations + 9,216 mux rows + 6,144 sibling rows
    let mut roots = [[0u8; NODE_BYTES]; 2];
    for (input_b, (start_coin, dirs, sibs)) in [
        (coin_in0, &wit.dirs0, &wit.sibs0),
        (coin_in1, &wit.dirs1, &wit.sibs1),
    ]
    .into_iter()
    .enumerate()
    {
        let mut cur = start_coin;
        for lvl in 0..MERKLE_DEPTH {
            let dir = dirs[lvl] & 1;
            let sib = sibs[lvl];
            for i in 0..384 {
                let (sx, srow) = sib_loc(input_b, lvl, i);
                b[sx][srow] = get_byte_bit(&sib, i);
            }
            for k in 0..MUX_ROWS_PER_LEVEL {
                let r_mux = mux_row(input_b, lvl, k);
                let c0 = get_byte_bit(&cur, 2 * k);
                let s0 = get_byte_bit(&sib, 2 * k);
                let c1 = get_byte_bit(&cur, 2 * k + 1);
                let s1 = get_byte_bit(&sib, 2 * k + 1);
                b[0][r_mux] = 1 ^ dir;
                b[1][r_mux] = c0 ^ s0;
                b[2][r_mux] = 1 ^ dir;
                b[3][r_mux] = c1 ^ s1;
                b[4][r_mux] = 0;
            }
            let (left, right) = if dir == 0 { (cur, sib) } else { (sib, cur) };
            let mut mn = [0u8; 97];
            mn[0] = TAG_NODE;
            mn[1..49].copy_from_slice(&left);
            mn[49..97].copy_from_slice(&right);
            record_perm(&mut b, 8 + input_b * MERKLE_DEPTH + lvl, &mn);
            cur = merkle_node(&left, &right);
        }
        roots[input_b] = cur;
    }

    for (input_b, (is_d, v_in, root)) in [
        (wit.is_dummy0 as u8, wit.v_in0, &roots[0]),
        (wit.is_dummy1 as u8, wit.v_in1, &roots[1]),
    ]
    .into_iter()
    .enumerate()
    {
        for k in 0..32 {
            let r_dv = dummy_val_row(input_b, k);
            b[0][r_dv] = 1 ^ is_d;
            b[1][r_dv] = ((v_in >> (2 * k)) & 1) as u8;
            b[2][r_dv] = 1 ^ is_d;
            b[3][r_dv] = ((v_in >> (2 * k + 1)) & 1) as u8;
            b[4][r_dv] = 0;
        }
        for k in 0..192 {
            let r_dr = dummy_root_row(input_b, k);
            let d0 = get_byte_bit(root, 2 * k) ^ get_byte_bit(&stmt.anchor, 2 * k);
            let d1 = get_byte_bit(root, 2 * k + 1) ^ get_byte_bit(&stmt.anchor, 2 * k + 1);
            b[0][r_dr] = is_d;
            b[1][r_dr] = d0;
            b[2][r_dr] = is_d;
            b[3][r_dr] = d1;
            b[4][r_dr] = 0;
        }
    }

    let fill_adder = |b: &mut [Vec<u8>; 5], base: usize, av: u128, bv: u128, bits: usize| {
        let mut c_in = 0u8;
        for i in 0..bits {
            let ai = ((av >> i) & 1) as u8;
            let bi = ((bv >> i) & 1) as u8;
            let ab = ai ^ bi;
            let c_out = (ai & bi) ^ (c_in & ab);
            let row = base + i;
            b[0][row] = 1 ^ ai;
            b[1][row] = bi;
            b[2][row] = 1 ^ c_in;
            b[3][row] = ab;
            b[4][row] = c_out;
            c_in = c_out;
        }
    };
    fill_adder(&mut b, ROW_ADDER_LHS1, wit.v_in0 as u128, wit.v_in1 as u128, 64);
    let lhs1_sum = (wit.v_in0 as u128) + (wit.v_in1 as u128);
    fill_adder(&mut b, ROW_ADDER_LHS, lhs1_sum, stmt.v_mint as u128, 65);
    fill_adder(&mut b, ROW_ADDER_MID, wit.v_out0 as u128, wit.v_out1 as u128, 64);
    let mid_sum = (wit.v_out0 as u128) + (wit.v_out1 as u128);
    fill_adder(&mut b, ROW_ADDER_RHS, mid_sum, stmt.pub_exit() as u128, 65);

    let set_priv_bytes = |b: &mut [Vec<u8>; 5], off: usize, data: &[u8]| {
        for bit in 0..(data.len() * 8) {
            let (x, row) = priv_loc(off + bit);
            b[x][row] = get_byte_bit(data, bit);
        }
    };
    set_priv_bytes(&mut b, OFF_S_IN0, &wit.s_in0);
    set_priv_bytes(&mut b, OFF_S_IN1, &wit.s_in1);
    set_priv_bytes(&mut b, OFF_V_IN0, &wit.v_in0.to_le_bytes());
    set_priv_bytes(&mut b, OFF_V_IN1, &wit.v_in1.to_le_bytes());
    set_priv_bytes(&mut b, OFF_R_IN0, &wit.r_in0);
    set_priv_bytes(&mut b, OFF_R_IN1, &wit.r_in1);
    set_priv_bytes(&mut b, OFF_A_OUT0, &wit.addr_out0);
    set_priv_bytes(&mut b, OFF_A_OUT1, &wit.addr_out1);
    set_priv_bytes(&mut b, OFF_V_OUT0, &wit.v_out0.to_le_bytes());
    set_priv_bytes(&mut b, OFF_V_OUT1, &wit.v_out1.to_le_bytes());
    set_priv_bytes(&mut b, OFF_R_OUT0, &wit.r_out0);
    set_priv_bytes(&mut b, OFF_R_OUT1, &wit.r_out1);

    let (xd0, rd0) = priv_loc(OFF_IS_DUMMY0);
    b[xd0][rd0] = wit.is_dummy0 as u8;
    let (xd1, rd1) = priv_loc(OFF_IS_DUMMY1);
    b[xd1][rd1] = wit.is_dummy1 as u8;

    for lvl in 0..MERKLE_DEPTH {
        let (x0, r0) = priv_loc(OFF_DIR0 + lvl);
        b[x0][r0] = wit.dirs0[lvl] & 1;
        let (x1, r1) = priv_loc(OFF_DIR1 + lvl);
        b[x1][r1] = wit.dirs1[lvl] & 1;
    }

    CircuitWitnessBits { b }
}

#[derive(Clone, Copy, Debug)]
pub struct CircuitChallenge {
    pub alpha: E,
}

#[derive(Clone, Debug)]
pub struct ScalarConstraint {
    pub lin: Vec<(u8, u32)>,
    pub quad: Vec<(u8, u32)>,
    pub rhs: u8,
}

/// Build the non-Merkle-mux Group C constraints (C1 pub outs, C3 dummy val, C4 dummy root, C5 adder)
/// plus the 36,864 C2 Merkle mux constraints in canonical order: [C1, C2, C3, C4, C5].
pub fn build_group_c_constraints(stmt: &PaymentStatement) -> Vec<ScalarConstraint> {
    let mut cs = Vec::with_capacity(42200);
    let perm_out_term = |perm: usize, bit_idx: usize| -> ((u8, u32), (u8, u32), u8) {
        let lane = bit_idx / 64;
        let x = lane % 5;
        let y = lane / 5;
        let z = bit_idx % 64;
        let row = keccak_row(perm, 23, y, z) as u32;
        let rc = if x == 0 && y == 0 {
            ((KECCAK_RC[23] >> z) & 1) as u8
        } else {
            0
        };
        ((x as u8, row), (x as u8, row), rc)
    };

    // C1: Public outputs (1,536 constraints)
    for (perm, target) in [
        (2usize, &stmt.sn0[..]),
        (5, &stmt.sn1[..]),
        (6, &stmt.coin_out0[..]),
        (7, &stmt.coin_out1[..]),
    ] {
        for b in 0..384 {
            let (lt, qt, rc) = perm_out_term(perm, b);
            let want = get_byte_bit(target, b);
            cs.push(ScalarConstraint {
                lin: vec![lt],
                quad: vec![qt],
                rhs: want ^ rc,
            });
        }
    }

    // C2: Merkle conditional-swap muxes (48 levels x 192 rows x 4 = 36,864 constraints)
    for input_b in 0..2 {
        let dir_base = if input_b == 0 { OFF_DIR0 } else { OFF_DIR1 };
        for lvl in 0..MERKLE_DEPTH {
            let src_perm = if lvl == 0 {
                if input_b == 0 { 1 } else { 4 }
            } else {
                8 + input_b * MERKLE_DEPTH + lvl - 1
            };
            let (dx, dr) = priv_loc(dir_base + lvl);
            let d_term = (dx as u8, dr as u32);
            for k in 0..MUX_ROWS_PER_LEVEL {
                let r_mux = mux_row(input_b, lvl, k) as u32;
                let (lt0, qt0, rc0) = perm_out_term(src_perm, 2 * k);
                let (lt1, qt1, rc1) = perm_out_term(src_perm, 2 * k + 1);
                let (sx0, sr0) = sib_loc(input_b, lvl, 2 * k);
                let (sx1, sr1) = sib_loc(input_b, lvl, 2 * k + 1);
                cs.push(ScalarConstraint { lin: vec![(0, r_mux), d_term], quad: vec![], rhs: 1 });
                cs.push(ScalarConstraint { lin: vec![(2, r_mux), d_term], quad: vec![], rhs: 1 });
                cs.push(ScalarConstraint {
                    lin: vec![(1, r_mux), lt0, (sx0 as u8, sr0 as u32)],
                    quad: vec![qt0],
                    rhs: rc0,
                });
                cs.push(ScalarConstraint {
                    lin: vec![(3, r_mux), lt1, (sx1 as u8, sr1 as u32)],
                    quad: vec![qt1],
                    rhs: rc1,
                });
            }
        }
    }

    // C3: Native is_dummy value zero-gating (384 constraints)
    for input_b in 0..2 {
        let (dx, dr) = priv_loc(if input_b == 0 { OFF_IS_DUMMY0 } else { OFF_IS_DUMMY1 });
        let d_term = (dx as u8, dr as u32);
        let v_base = if input_b == 0 { OFF_V_IN0 } else { OFF_V_IN1 };
        for k in 0..32 {
            let r_dv = dummy_val_row(input_b, k) as u32;
            let (vx0, vr0) = priv_loc(v_base + 2 * k);
            let (vx1, vr1) = priv_loc(v_base + 2 * k + 1);
            cs.push(ScalarConstraint { lin: vec![(0, r_dv), d_term], quad: vec![], rhs: 1 });
            cs.push(ScalarConstraint { lin: vec![(1, r_dv), (vx0 as u8, vr0 as u32)], quad: vec![], rhs: 0 });
            cs.push(ScalarConstraint { lin: vec![], quad: vec![(4, r_dv)], rhs: 0 });
            cs.push(ScalarConstraint { lin: vec![(2, r_dv), d_term], quad: vec![], rhs: 1 });
            cs.push(ScalarConstraint { lin: vec![(3, r_dv), (vx1 as u8, vr1 as u32)], quad: vec![], rhs: 0 });
            cs.push(ScalarConstraint { lin: vec![], quad: vec![(1, r_dv)], rhs: 0 });
        }
    }

    // C4: Native is_dummy anchor root-gating (2,304 constraints)
    for input_b in 0..2 {
        let (dx, dr) = priv_loc(if input_b == 0 { OFF_IS_DUMMY0 } else { OFF_IS_DUMMY1 });
        let d_term = (dx as u8, dr as u32);
        let root_perm = 8 + input_b * MERKLE_DEPTH + (MERKLE_DEPTH - 1);
        for k in 0..192 {
            let r_dr = dummy_root_row(input_b, k) as u32;
            let (lt0, qt0, rc0) = perm_out_term(root_perm, 2 * k);
            let (lt1, qt1, rc1) = perm_out_term(root_perm, 2 * k + 1);
            let a0 = get_byte_bit(&stmt.anchor, 2 * k);
            let a1 = get_byte_bit(&stmt.anchor, 2 * k + 1);
            cs.push(ScalarConstraint { lin: vec![(0, r_dr), d_term], quad: vec![], rhs: 0 });
            cs.push(ScalarConstraint { lin: vec![(1, r_dr), lt0], quad: vec![qt0], rhs: a0 ^ rc0 });
            cs.push(ScalarConstraint { lin: vec![], quad: vec![(4, r_dr)], rhs: 0 });
            cs.push(ScalarConstraint { lin: vec![(2, r_dr), d_term], quad: vec![], rhs: 0 });
            cs.push(ScalarConstraint { lin: vec![(3, r_dr), lt1], quad: vec![qt1], rhs: a1 ^ rc1 });
            cs.push(ScalarConstraint { lin: vec![], quad: vec![(1, r_dr)], rhs: 0 });
        }
    }

    // C5: 66-bit exact ripple-carry conservation adders (1,360 constraints):
    //   v_in0 + v_in1 + v_mint == v_out0 + v_out1 + (fee + v_unshield)
    let pub_exit = stmt.pub_exit();
    let add_adder = |cs: &mut Vec<ScalarConstraint>, base: usize, bits: usize| {
        cs.push(ScalarConstraint { lin: vec![(2, base as u32)], quad: vec![], rhs: 1 });
        for i in 0..bits {
            let row = (base + i) as u32;
            cs.push(ScalarConstraint { lin: vec![(0, row), (1, row), (3, row)], quad: vec![], rhs: 1 });
            cs.push(ScalarConstraint { lin: vec![(4, row)], quad: vec![(4, row), (1, row)], rhs: 0 });
            if i > 0 {
                cs.push(ScalarConstraint { lin: vec![(2, row), (4, row - 1)], quad: vec![], rhs: 1 });
            }
        }
    };
    add_adder(&mut cs, ROW_ADDER_LHS1, 64);
    add_adder(&mut cs, ROW_ADDER_LHS, 65);
    add_adder(&mut cs, ROW_ADDER_MID, 64);
    add_adder(&mut cs, ROW_ADDER_RHS, 65);

    for i in 0..64 {
        let (xa0, ra0) = priv_loc(OFF_V_IN0 + i);
        let (xb0, rb0) = priv_loc(OFF_V_IN1 + i);
        let rl1 = (ROW_ADDER_LHS1 + i) as u32;
        cs.push(ScalarConstraint { lin: vec![(0, rl1), (xa0 as u8, ra0 as u32)], quad: vec![], rhs: 1 });
        cs.push(ScalarConstraint { lin: vec![(1, rl1), (xb0 as u8, rb0 as u32)], quad: vec![], rhs: 0 });

        let (xa1, ra1) = priv_loc(OFF_V_OUT0 + i);
        let (xb1, rb1) = priv_loc(OFF_V_OUT1 + i);
        let rm = (ROW_ADDER_MID + i) as u32;
        cs.push(ScalarConstraint { lin: vec![(0, rm), (xa1 as u8, ra1 as u32)], quad: vec![], rhs: 1 });
        cs.push(ScalarConstraint { lin: vec![(1, rm), (xb1 as u8, rb1 as u32)], quad: vec![], rhs: 0 });
    }

    for i in 0..65 {
        let rl = (ROW_ADDER_LHS + i) as u32;
        let mint_bit = if i < 64 { ((stmt.v_mint >> i) & 1) as u8 } else { 0 };
        cs.push(ScalarConstraint { lin: vec![(1, rl)], quad: vec![], rhs: mint_bit });
        if i < 64 {
            let rl1 = (ROW_ADDER_LHS1 + i) as u32;
            cs.push(ScalarConstraint { lin: vec![(0, rl), (3, rl1), (2, rl1)], quad: vec![], rhs: 0 });
        } else {
            let rl1_63 = (ROW_ADDER_LHS1 + 63) as u32;
            cs.push(ScalarConstraint { lin: vec![(0, rl), (4, rl1_63)], quad: vec![], rhs: 1 });
        }
    }

    for i in 0..65 {
        let rr = (ROW_ADDER_RHS + i) as u32;
        let exit_bit = if i < 64 { ((pub_exit >> i) & 1) as u8 } else { 0 };
        cs.push(ScalarConstraint { lin: vec![(1, rr)], quad: vec![], rhs: exit_bit });
        if i < 64 {
            let rm = (ROW_ADDER_MID + i) as u32;
            cs.push(ScalarConstraint { lin: vec![(0, rr), (3, rm), (2, rm)], quad: vec![], rhs: 0 });
        } else {
            let rm63 = (ROW_ADDER_MID + 63) as u32;
            cs.push(ScalarConstraint { lin: vec![(0, rr), (4, rm63)], quad: vec![], rhs: 1 });
        }
    }

    for i in 0..65 {
        let rl = (ROW_ADDER_LHS + i) as u32;
        let rr = (ROW_ADDER_RHS + i) as u32;
        cs.push(ScalarConstraint { lin: vec![(3, rl), (2, rl), (3, rr), (2, rr)], quad: vec![], rhs: 0 });
    }
    cs.push(ScalarConstraint {
        lin: vec![(4, (ROW_ADDER_LHS + 64) as u32), (4, (ROW_ADDER_RHS + 64) as u32)],
        quad: vec![],
        rhs: 0,
    });

    cs
}

fn build_trp_tables_pow(alpha: E, kt: &KTables) -> (Box<[[E; 320]; 5]>, Box<[[E; 320]; 5]>, E) {
    let mut u_next = Box::new([[E::ZERO; 320]; 5]);
    let mut u_curr = Box::new([[E::ZERO; 320]; 5]);
    let mut w = E::one();
    for ny in 0..5 {
        for nx in 0..5 {
            for nz in 0..64 {
                u_next[nx][64 * ny + nz] = w;
                for (ix, iy, iz) in trp_input_bits(nx, ny, nz) {
                    u_curr[ix][64 * iy + iz].add_assign(w);
                }
                w = w.mul(alpha, kt);
            }
        }
    }
    (u_next, u_curr, w)
}

pub fn build_prover_weights(
    stmt: &PaymentStatement,
    ch: &CircuitChallenge,
    kt: &KTables,
) -> ([Vec<E>; 5], [Vec<E>; 5], E) {
    let mut wb = [
        vec![E::ZERO; NUM_BITS_PER_POLY],
        vec![E::ZERO; NUM_BITS_PER_POLY],
        vec![E::ZERO; NUM_BITS_PER_POLY],
        vec![E::ZERO; NUM_BITS_PER_POLY],
        vec![E::ZERO; NUM_BITS_PER_POLY],
    ];
    let mut wc = [
        vec![E::ZERO; NUM_BITS_PER_POLY],
        vec![E::ZERO; NUM_BITS_PER_POLY],
        vec![E::ZERO; NUM_BITS_PER_POLY],
        vec![E::ZERO; NUM_BITS_PER_POLY],
        vec![E::ZERO; NUM_BITS_PER_POLY],
    ];
    let mut pub_rhs = E::ZERO;

    let (u_next, u_curr, stride) = build_trp_tables_pow(ch.alpha, kt);

    // Group A
    let mut cur_scale = E::one();
    for p in 0..NUM_PERMS {
        for r in 0..23 {
            let s = cur_scale;
            cur_scale = cur_scale.mul(stride, kt);
            let base_next = keccak_row(p, r + 1, 0, 0);
            let base_curr = keccak_row(p, r, 0, 0);
            for x in 0..5 {
                let un_x = &u_next[x];
                let uc_x = &u_curr[x];
                let wb_x = &mut wb[x];
                let wc_x = &mut wc[x];
                for yz in 0..320 {
                    let vn = s.mul(un_x[yz], kt);
                    let vc = s.mul(uc_x[yz], kt);
                    wb_x[base_next + yz].add_assign(vn);
                    wb_x[base_curr + yz].add_assign(vc);
                    wc_x[base_curr + yz].add_assign(vc);
                }
            }
            let rc = KECCAK_RC[r];
            for z in 0..64 {
                if ((rc >> z) & 1) != 0 {
                    pub_rhs.add_assign(s.mul(u_curr[0][z], kt));
                }
            }
        }
    }

    // Group B
    for p in 0..NUM_PERMS {
        let s = cur_scale;
        cur_scale = cur_scale.mul(stride, kt);
        let base_r0 = keccak_row(p, 0, 0, 0);
        for x in 0..5 {
            let un_x = &u_next[x];
            let wb_x = &mut wb[x];
            for yz in 0..320 {
                wb_x[base_r0 + yz].add_assign(s.mul(un_x[yz], kt));
            }
        }
        let spec = perm_input_spec(p);
        for (bit_idx, src) in spec.into_iter().enumerate() {
            let lane = bit_idx / 64;
            let ix = lane % 5;
            let iy = lane / 5;
            let iz = bit_idx % 64;
            match src {
                InputBitSrc::Const(0) => {}
                InputBitSrc::Const(1) => pub_rhs.add_assign(s.mul(u_curr[ix][64 * iy + iz], kt)),
                InputBitSrc::Const(_) => unreachable!(),
                InputBitSrc::Priv(off) => {
                    let w = s.mul(u_curr[ix][64 * iy + iz], kt);
                    let (px, prow) = priv_loc(off);
                    wb[px][prow].add_assign(w);
                }
                InputBitSrc::PermOut(sp, ox, oy, oz) => {
                    let w = s.mul(u_curr[ix][64 * iy + iz], kt);
                    let orow = keccak_row(sp, 23, oy, oz);
                    wb[ox][orow].add_assign(w);
                    wc[ox][orow].add_assign(w);
                    if ox == 0 && oy == 0 && ((KECCAK_RC[23] >> oz) & 1) != 0 {
                        pub_rhs.add_assign(w);
                    }
                }
                InputBitSrc::MerkleLeft { src_perm, x, y, z, mux_x, mux_row: r_mux } => {
                    let w = s.mul(u_curr[ix][64 * iy + iz], kt);
                    let orow = keccak_row(src_perm, 23, y, z);
                    wb[x][orow].add_assign(w);
                    wc[x][orow].add_assign(w);
                    wc[mux_x][r_mux].add_assign(w);
                    if x == 0 && y == 0 && ((KECCAK_RC[23] >> z) & 1) != 0 {
                        pub_rhs.add_assign(w);
                    }
                }
                InputBitSrc::MerkleRight { sib_x, sib_row, mux_x, mux_row: r_mux } => {
                    let w = s.mul(u_curr[ix][64 * iy + iz], kt);
                    wb[sib_x][sib_row].add_assign(w);
                    wc[mux_x][r_mux].add_assign(w);
                }
            }
        }
    }

    // Group C
    let cs = build_group_c_constraints(stmt);
    let mut w = cur_scale;
    for c in &cs {
        for &(x, row) in &c.lin {
            wb[x as usize][row as usize].add_assign(w);
        }
        for &(x, row) in &c.quad {
            wc[x as usize][row as usize].add_assign(w);
        }
        if c.rhs != 0 {
            pub_rhs.add_assign(w);
        }
        w = w.mul(ch.alpha, kt);
    }

    (wb, wc, pub_rhs)
}

/// Remaining scalar constraints after factoring C1 and C2 in the verifier.
fn group_c3_to_c5(stmt: &PaymentStatement) -> Vec<ScalarConstraint> {
    let mut rest = Vec::with_capacity(3800);
    let perm_out_term = |perm: usize, bit_idx: usize| -> ((u8, u32), (u8, u32), u8) {
        let lane = bit_idx / 64;
        let x = lane % 5;
        let y = lane / 5;
        let z = bit_idx % 64;
        let row = keccak_row(perm, 23, y, z) as u32;
        let rc = if x == 0 && y == 0 { ((KECCAK_RC[23] >> z) & 1) as u8 } else { 0 };
        ((x as u8, row), (x as u8, row), rc)
    };
    for input_b in 0..2 {
        let (dx, dr) = priv_loc(if input_b == 0 { OFF_IS_DUMMY0 } else { OFF_IS_DUMMY1 });
        let d_term = (dx as u8, dr as u32);
        let v_base = if input_b == 0 { OFF_V_IN0 } else { OFF_V_IN1 };
        for k in 0..32 {
            let r_dv = dummy_val_row(input_b, k) as u32;
            let (vx0, vr0) = priv_loc(v_base + 2 * k);
            let (vx1, vr1) = priv_loc(v_base + 2 * k + 1);
            rest.push(ScalarConstraint { lin: vec![(0, r_dv), d_term], quad: vec![], rhs: 1 });
            rest.push(ScalarConstraint { lin: vec![(1, r_dv), (vx0 as u8, vr0 as u32)], quad: vec![], rhs: 0 });
            rest.push(ScalarConstraint { lin: vec![], quad: vec![(4, r_dv)], rhs: 0 });
            rest.push(ScalarConstraint { lin: vec![(2, r_dv), d_term], quad: vec![], rhs: 1 });
            rest.push(ScalarConstraint { lin: vec![(3, r_dv), (vx1 as u8, vr1 as u32)], quad: vec![], rhs: 0 });
            rest.push(ScalarConstraint { lin: vec![], quad: vec![(1, r_dv)], rhs: 0 });
        }
    }
    for input_b in 0..2 {
        let (dx, dr) = priv_loc(if input_b == 0 { OFF_IS_DUMMY0 } else { OFF_IS_DUMMY1 });
        let d_term = (dx as u8, dr as u32);
        let root_perm = 8 + input_b * MERKLE_DEPTH + (MERKLE_DEPTH - 1);
        for k in 0..192 {
            let r_dr = dummy_root_row(input_b, k) as u32;
            let (lt0, qt0, rc0) = perm_out_term(root_perm, 2 * k);
            let (lt1, qt1, rc1) = perm_out_term(root_perm, 2 * k + 1);
            let a0 = get_byte_bit(&stmt.anchor, 2 * k);
            let a1 = get_byte_bit(&stmt.anchor, 2 * k + 1);
            rest.push(ScalarConstraint { lin: vec![(0, r_dr), d_term], quad: vec![], rhs: 0 });
            rest.push(ScalarConstraint { lin: vec![(1, r_dr), lt0], quad: vec![qt0], rhs: a0 ^ rc0 });
            rest.push(ScalarConstraint { lin: vec![], quad: vec![(4, r_dr)], rhs: 0 });
            rest.push(ScalarConstraint { lin: vec![(2, r_dr), d_term], quad: vec![], rhs: 0 });
            rest.push(ScalarConstraint { lin: vec![(3, r_dr), lt1], quad: vec![qt1], rhs: a1 ^ rc1 });
            rest.push(ScalarConstraint { lin: vec![], quad: vec![(1, r_dr)], rhs: 0 });
        }
    }
    let pub_exit = stmt.pub_exit();
    let add_adder = |cs: &mut Vec<ScalarConstraint>, base: usize, bits: usize| {
        cs.push(ScalarConstraint { lin: vec![(2, base as u32)], quad: vec![], rhs: 1 });
        for i in 0..bits {
            let row = (base + i) as u32;
            cs.push(ScalarConstraint { lin: vec![(0, row), (1, row), (3, row)], quad: vec![], rhs: 1 });
            cs.push(ScalarConstraint { lin: vec![(4, row)], quad: vec![(4, row), (1, row)], rhs: 0 });
            if i > 0 {
                cs.push(ScalarConstraint { lin: vec![(2, row), (4, row - 1)], quad: vec![], rhs: 1 });
            }
        }
    };
    add_adder(&mut rest, ROW_ADDER_LHS1, 64);
    add_adder(&mut rest, ROW_ADDER_LHS, 65);
    add_adder(&mut rest, ROW_ADDER_MID, 64);
    add_adder(&mut rest, ROW_ADDER_RHS, 65);
    for i in 0..64 {
        let (xa0, ra0) = priv_loc(OFF_V_IN0 + i);
        let (xb0, rb0) = priv_loc(OFF_V_IN1 + i);
        let rl1 = (ROW_ADDER_LHS1 + i) as u32;
        rest.push(ScalarConstraint { lin: vec![(0, rl1), (xa0 as u8, ra0 as u32)], quad: vec![], rhs: 1 });
        rest.push(ScalarConstraint { lin: vec![(1, rl1), (xb0 as u8, rb0 as u32)], quad: vec![], rhs: 0 });
        let (xa1, ra1) = priv_loc(OFF_V_OUT0 + i);
        let (xb1, rb1) = priv_loc(OFF_V_OUT1 + i);
        let rm = (ROW_ADDER_MID + i) as u32;
        rest.push(ScalarConstraint { lin: vec![(0, rm), (xa1 as u8, ra1 as u32)], quad: vec![], rhs: 1 });
        rest.push(ScalarConstraint { lin: vec![(1, rm), (xb1 as u8, rb1 as u32)], quad: vec![], rhs: 0 });
    }
    for i in 0..65 {
        let rl = (ROW_ADDER_LHS + i) as u32;
        let mint_bit = if i < 64 { ((stmt.v_mint >> i) & 1) as u8 } else { 0 };
        rest.push(ScalarConstraint { lin: vec![(1, rl)], quad: vec![], rhs: mint_bit });
        if i < 64 {
            let rl1 = (ROW_ADDER_LHS1 + i) as u32;
            rest.push(ScalarConstraint { lin: vec![(0, rl), (3, rl1), (2, rl1)], quad: vec![], rhs: 0 });
        } else {
            let rl1_63 = (ROW_ADDER_LHS1 + 63) as u32;
            rest.push(ScalarConstraint { lin: vec![(0, rl), (4, rl1_63)], quad: vec![], rhs: 1 });
        }
    }
    for i in 0..65 {
        let rr = (ROW_ADDER_RHS + i) as u32;
        let exit_bit = if i < 64 { ((pub_exit >> i) & 1) as u8 } else { 0 };
        rest.push(ScalarConstraint { lin: vec![(1, rr)], quad: vec![], rhs: exit_bit });
        if i < 64 {
            let rm = (ROW_ADDER_MID + i) as u32;
            rest.push(ScalarConstraint { lin: vec![(0, rr), (3, rm), (2, rm)], quad: vec![], rhs: 0 });
        } else {
            let rm63 = (ROW_ADDER_MID + 63) as u32;
            rest.push(ScalarConstraint { lin: vec![(0, rr), (4, rm63)], quad: vec![], rhs: 1 });
        }
    }
    for i in 0..65 {
        let rl = (ROW_ADDER_LHS + i) as u32;
        let rr = (ROW_ADDER_RHS + i) as u32;
        rest.push(ScalarConstraint { lin: vec![(3, rl), (2, rl), (3, rr), (2, rr)], quad: vec![], rhs: 0 });
    }
    rest.push(ScalarConstraint {
        lin: vec![(4, (ROW_ADDER_LHS + 64) as u32), (4, (ROW_ADDER_RHS + 64) as u32)],
        quad: vec![],
        rhs: 0,
    });
    rest
}

/// Ultra-fast factored verifier MLE evaluation of `(W_B[0..5](r1), W_C[0..5](r1), pub_rhs)` at `r1 in E^19`.
/// Factors ALL 56 perms in Group A, ALL 48 Merkle perms in Group B, and ALL 36,864 Merkle mux
/// constraints in Group C2 into O(1) dot-product templates reused across all 48 levels!
pub fn eval_verifier_weights_at_r1(
    stmt: &PaymentStatement,
    ch: &CircuitChallenge,
    r1: &[E; V_SC1],
    kt: &KTables,
) -> ([E; 5], [E; 5], E) {
    let num_hi = (ACTIVE_ROWS + 63) / 64;
    let eq_hi = eq_table_scaled_limit(&r1[..13], E::one(), num_hi, kt);
    let eq_lo = eq_table(&r1[13..], kt);

    let mut wb_eval = [E::ZERO; 5];
    let mut wc_eval = [E::ZERO; 5];
    let mut pub_rhs = E::ZERO;

    let (u_next, u_curr, stride) = build_trp_tables_pow(ch.alpha, kt);

    let mut un_dot = [[E::ZERO; 5]; 5];
    let mut uc_dot = [[E::ZERO; 5]; 5];
    for x in 0..5 {
        for y in 0..5 {
            let mut an = E::ZERO;
            let mut ac = E::ZERO;
            for z in 0..64 {
                an.add_assign(u_next[x][64 * y + z].mul(eq_lo[z], kt));
                ac.add_assign(u_curr[x][64 * y + z].mul(eq_lo[z], kt));
            }
            un_dot[x][y] = an;
            uc_dot[x][y] = ac;
        }
    }

    // Group A (56 perms x 23 transitions)
    let mut cur_scale = E::one();
    let mut sum_next_y = [E::ZERO; 5];
    let mut sum_curr_y = [E::ZERO; 5];
    let mut rc_acc = [E::ZERO; 64];
    // Interior Keccak rows belong to two consecutive transitions. Their
    // current-round contribution is their next-round contribution times stride.
    let mut interior = [E::ZERO; 5];
    for p in 0..NUM_PERMS {
        let first = keccak_row(p, 0, 0, 0) >> 6;
        for y in 0..5 {
            sum_curr_y[y].add_assign(cur_scale.mul(eq_hi[first + y], kt));
        }
        for r in 0..23 {
            let s = cur_scale;
            cur_scale = cur_scale.mul(stride, kt);
            let hi_next = keccak_row(p, r + 1, 0, 0) >> 6;
            let target = if r == 22 { &mut sum_next_y } else { &mut interior };
            for y in 0..5 {
                target[y].add_assign(s.mul(eq_hi[hi_next + y], kt));
            }
            let rc = KECCAK_RC[r];
            for z in 0..64 {
                if ((rc >> z) & 1) != 0 {
                    rc_acc[z].add_assign(s);
                }
            }
        }
    }
    for y in 0..5 {
        sum_next_y[y].add_assign(interior[y]);
        sum_curr_y[y].add_assign(interior[y].mul(stride, kt));
    }
    for x in 0..5 {
        for y in 0..5 {
            wb_eval[x].add_assign(sum_next_y[y].mul(un_dot[x][y], kt));
            let tc = sum_curr_y[y].mul(uc_dot[x][y], kt);
            wb_eval[x].add_assign(tc);
            wc_eval[x].add_assign(tc);
        }
    }
    for z in 0..64 {
        if !rc_acc[z].is_zero() {
            pub_rhs.add_assign(rc_acc[z].mul(u_curr[0][z], kt));
        }
    }

    // Bucketed accumulators for non-factored terms (Base 8 perms + C1/C3/C4/C5)
    let mut b_lo = [[E::ZERO; 64]; 5];
    let mut c_lo = [[E::ZERO; 64]; 5];
    let mut sum_r0_y = [E::ZERO; 5];

    // Group B1: Base 8 perms (0..8)
    for p in 0..8 {
        let s = cur_scale;
        cur_scale = cur_scale.mul(stride, kt);
        let hi_r0 = (p * 7680) >> 6;
        for y in 0..5 {
            sum_r0_y[y].add_assign(s.mul(eq_hi[hi_r0 + y], kt));
        }
        let spec = perm_input_spec(p);
        let mut const_acc = E::ZERO;
        for (bit_idx, src) in spec.into_iter().enumerate() {
            let lane = bit_idx / 64;
            let ix = lane % 5;
            let iy = lane / 5;
            let iz = bit_idx % 64;
            match src {
                InputBitSrc::Const(0) => {}
                InputBitSrc::Const(1) => const_acc.add_assign(u_curr[ix][64 * iy + iz]),
                InputBitSrc::Priv(off) => {
                    let w = s.mul(u_curr[ix][64 * iy + iz], kt);
                    let (px, prow) = priv_loc(off);
                    b_lo[px][prow & 63].add_assign(w.mul(eq_hi[prow >> 6], kt));
                }
                InputBitSrc::PermOut(sp, ox, oy, oz) => {
                    let w = s.mul(u_curr[ix][64 * iy + iz], kt);
                    let orow = keccak_row(sp, 23, oy, oz);
                    let we = w.mul(eq_hi[orow >> 6], kt);
                    b_lo[ox][orow & 63].add_assign(we);
                    c_lo[ox][orow & 63].add_assign(we);
                    if ox == 0 && oy == 0 && ((KECCAK_RC[23] >> oz) & 1) != 0 {
                        pub_rhs.add_assign(w);
                    }
                }
                _ => unreachable!(),
            }
        }
        pub_rhs.add_assign(s.mul(const_acc, kt));
    }

    // Group B2: Factored 48 Merkle perms (p = 8..56)!
    // Precompute the level-independent dot-product templates ONCE (~800 E::muls total)!
    let mut b_cur_dot = [E::ZERO; 6];      // lane 0..5 -> (x = lane%5, y = lane/5)
    let mut b_mux_dot = [[E::ZERO; 2]; 3]; // blk 0..2, parity 0 (x=4) / 1 (x=1)
    let mut b_sib_dot = [[E::ZERO; 3]; 2]; // blk 0..1, x in 0..2
    let mut b_merkle_rhs = E::ZERO;

    // Constant bits of Merkle perm: byte 0 = TAG_NODE (0x03), byte 97 = 0x1F, bit 135*8+7 = 1
    let mut add_const_bit = |bit_idx: usize| {
        let lane = bit_idx / 64;
        b_merkle_rhs.add_assign(u_curr[lane % 5][64 * (lane / 5) + (bit_idx % 64)]);
    };
    for bit in 0..8 {
        if ((TAG_NODE >> bit) & 1) != 0 {
            add_const_bit(bit);
        }
        if ((0x1Fu8 >> bit) & 1) != 0 {
            add_const_bit(97 * 8 + bit);
        }
    }
    add_const_bit(135 * 8 + 7);

    for i in 0..384 {
        let lbit = 8 + i;
        let ll = lbit / 64;
        let ul = u_curr[ll % 5][64 * (ll / 5) + (lbit % 64)];

        let rbit = 49 * 8 + i;
        let rl = rbit / 64;
        let ur = u_curr[rl % 5][64 * (rl / 5) + (rbit % 64)];

        // src_perm output bit i: lane = i/64, z = i%64 -> eq_lo[z]
        let lane = i / 64;
        let z = i % 64;
        b_cur_dot[lane].add_assign(ul.mul(eq_lo[z], kt));
        if lane == 0 && ((KECCAK_RC[23] >> z) & 1) != 0 {
            b_merkle_rhs.add_assign(ul);
        }

        // mux_row(b, lvl, k = i/2): blk = k/64, lo = k%64, parity = i&1
        let k = i / 2;
        let sum_lr = ul.add(ur);
        b_mux_dot[k >> 6][i & 1].add_assign(sum_lr.mul(eq_lo[k & 63], kt));

        // sib_loc(b, lvl, i): m = i/3 (0..127), blk = m/64, lo = m%64, x = i%3
        let m = i / 3;
        b_sib_dot[m >> 6][i % 3].add_assign(ur.mul(eq_lo[m & 63], kt));
    }

    // Accumulate scales across all 48 Merkle perms (only ~12 E::muls per perm!)
    let mut b_cur_scale = [E::ZERO; 6];
    let mut b_mux_scale = [E::ZERO; 3];
    let mut b_sib_scale = [E::ZERO; 2];
    let mut b_rhs_scale = E::ZERO;
    for p in 8..NUM_PERMS {
        let s = cur_scale;
        cur_scale = cur_scale.mul(stride, kt);
        let rel = p - 8;
        let input_b = rel / MERKLE_DEPTH;
        let lvl = rel % MERKLE_DEPTH;
        let src_perm = if lvl == 0 {
            if input_b == 0 { 1 } else { 4 }
        } else {
            p - 1
        };

        let hi_r0 = (p * 7680) >> 6;
        for y in 0..5 {
            sum_r0_y[y].add_assign(s.mul(eq_hi[hi_r0 + y], kt));
        }
        b_rhs_scale.add_assign(s);

        let src_hi_y0 = keccak_row(src_perm, 23, 0, 0) >> 6;
        let src_hi_y1 = keccak_row(src_perm, 23, 1, 0) >> 6;
        let s_y0 = s.mul(eq_hi[src_hi_y0], kt);
        let s_y1 = s.mul(eq_hi[src_hi_y1], kt);
        for lane in 0..5 {
            b_cur_scale[lane].add_assign(s_y0);
        }
        b_cur_scale[5].add_assign(s_y1);

        let mux_hi0 = mux_row(input_b, lvl, 0) >> 6;
        for blk in 0..3 {
            b_mux_scale[blk].add_assign(s.mul(eq_hi[mux_hi0 + blk], kt));
        }
        let sib_hi0 = (ROW_SIB_BASE + rel * SIB_ROWS_PER_LEVEL) >> 6;
        for blk in 0..2 {
            b_sib_scale[blk].add_assign(s.mul(eq_hi[sib_hi0 + blk], kt));
        }
    }

    // Apply Group B2 factored totals
    for x in 0..5 {
        for y in 0..5 {
            wb_eval[x].add_assign(sum_r0_y[y].mul(un_dot[x][y], kt));
        }
    }
    pub_rhs.add_assign(b_rhs_scale.mul(b_merkle_rhs, kt));
    for lane in 0..6 {
        let x = lane % 5;
        let t = b_cur_scale[lane].mul(b_cur_dot[lane], kt);
        wb_eval[x].add_assign(t);
        wc_eval[x].add_assign(t);
    }
    for blk in 0..3 {
        wc_eval[4].add_assign(b_mux_scale[blk].mul(b_mux_dot[blk][0], kt));
        wc_eval[1].add_assign(b_mux_scale[blk].mul(b_mux_dot[blk][1], kt));
    }
    for blk in 0..2 {
        for x in 0..3 {
            wb_eval[x].add_assign(b_sib_scale[blk].mul(b_sib_dot[blk][x], kt));
        }
    }

    // Group C: [C1 (1536), C2 (36864 factored!), C3 (384), C4 (2304), C5 (1090)]
    let rest_cs = group_c3_to_c5(stmt);
    let mut w = cur_scale;

    // Every public output lane uses the same 64 low-row equality weights.
    let mut powers = [E::ZERO; 64];
    let mut dot = E::ZERO;
    let mut lane_stride = E::one();
    for z in 0..64 {
        powers[z] = lane_stride;
        dot.add_assign(lane_stride.mul(eq_lo[z], kt));
        lane_stride = lane_stride.mul(ch.alpha, kt);
    }
    for (perm, output) in [(2, &stmt.sn0), (5, &stmt.sn1), (6, &stmt.coin_out0), (7, &stmt.coin_out1)] {
        for (lane, bytes) in output.chunks_exact(8).enumerate() {
            let x = lane % 5;
            let hi = keccak_row(perm, 23, lane / 5, 0) >> 6;
            let term = dot.mul(w, kt).mul(eq_hi[hi], kt);
            wb_eval[x].add_assign(term);
            wc_eval[x].add_assign(term);
            let mut bits = u64::from_le_bytes(bytes.try_into().unwrap());
            if lane == 0 { bits ^= KECCAK_RC[23]; }
            let mut rhs = E::ZERO;
            while bits != 0 {
                rhs.add_assign(powers[bits.trailing_zeros() as usize]);
                bits &= bits - 1;
            }
            pub_rhs.add_assign(w.mul(rhs, kt));
            w = w.mul(lane_stride, kt);
        }
    }

    // C2 (1536 .. 1536 + 36864): Factored across all 48 Merkle levels!
    // Precompute 1-level template for alpha^0 .. alpha^767 ONCE (~768 E::muls total!)
    let mut c2_mux_dot = [[E::ZERO; 4]; 3]; // blk 0..2, x in 0..3 (B_0, B_1, B_2, B_3)
    let mut c2_cur_dot = [E::ZERO; 6];      // lane 0..5
    let mut c2_sib_dot = [[E::ZERO; 3]; 2]; // blk 0..1, x in 0..2
    let mut c2_dir_sum = E::ZERO;           // sum of weights on dir bit (also equals rhs=1 sum for B_0, B_2!)
    let mut c2_rhs_total = E::ZERO;
    let mut pw = E::one();
    for k in 0..MUX_ROWS_PER_LEVEL {
        let w0 = pw;
        let w1 = w0.mul(ch.alpha, kt);
        let w2 = w1.mul(ch.alpha, kt);
        let w3 = w2.mul(ch.alpha, kt);
        pw = w3.mul(ch.alpha, kt);

        let eq_k = eq_lo[k & 63];
        let blk = k >> 6;
        c2_mux_dot[blk][0].add_assign(w0.mul(eq_k, kt));
        c2_mux_dot[blk][2].add_assign(w1.mul(eq_k, kt));
        c2_mux_dot[blk][1].add_assign(w2.mul(eq_k, kt));
        c2_mux_dot[blk][3].add_assign(w3.mul(eq_k, kt));

        let w01 = w0.add(w1);
        c2_dir_sum.add_assign(w01);
        c2_rhs_total.add_assign(w01);

        // Bit 2k (weight w2) and bit 2k+1 (weight w3)
        for (bit_off, wb_i) in [(2 * k, w2), (2 * k + 1, w3)] {
            let lane = bit_off / 64;
            let z = bit_off % 64;
            c2_cur_dot[lane].add_assign(wb_i.mul(eq_lo[z], kt));
            if lane == 0 && ((KECCAK_RC[23] >> z) & 1) != 0 {
                c2_rhs_total.add_assign(wb_i);
            }
            let m = bit_off / 3;
            c2_sib_dot[m >> 6][bit_off % 3].add_assign(wb_i.mul(eq_lo[m & 63], kt));
        }
    }
    let c2_stride = pw; // alpha^768

    let mut c2_mux_scale = [E::ZERO; 3];
    let mut c2_cur_scale = [E::ZERO; 6];
    let mut c2_sib_scale = [E::ZERO; 2];
    let mut c2_rhs_scale = E::ZERO;
    for input_b in 0..2 {
        let dir_base = if input_b == 0 { OFF_DIR0 } else { OFF_DIR1 };
        for lvl in 0..MERKLE_DEPTH {
            let lvl_w = w;
            w = w.mul(c2_stride, kt);
            let rel = input_b * MERKLE_DEPTH + lvl;
            let src_perm = if lvl == 0 {
                if input_b == 0 { 1 } else { 4 }
            } else {
                8 + rel - 1
            };

            c2_rhs_scale.add_assign(lvl_w);
            let (dx, dr) = priv_loc(dir_base + lvl);
            b_lo[dx][dr & 63].add_assign(lvl_w.mul(c2_dir_sum, kt).mul(eq_hi[dr >> 6], kt));

            let mux_hi0 = mux_row(input_b, lvl, 0) >> 6;
            for blk in 0..3 {
                c2_mux_scale[blk].add_assign(lvl_w.mul(eq_hi[mux_hi0 + blk], kt));
            }
            let src_hi_y0 = keccak_row(src_perm, 23, 0, 0) >> 6;
            let src_hi_y1 = keccak_row(src_perm, 23, 1, 0) >> 6;
            let s_y0 = lvl_w.mul(eq_hi[src_hi_y0], kt);
            let s_y1 = lvl_w.mul(eq_hi[src_hi_y1], kt);
            for lane in 0..5 {
                c2_cur_scale[lane].add_assign(s_y0);
            }
            c2_cur_scale[5].add_assign(s_y1);

            let sib_hi0 = (ROW_SIB_BASE + rel * SIB_ROWS_PER_LEVEL) >> 6;
            for blk in 0..2 {
                c2_sib_scale[blk].add_assign(lvl_w.mul(eq_hi[sib_hi0 + blk], kt));
            }
        }
    }
    pub_rhs.add_assign(c2_rhs_scale.mul(c2_rhs_total, kt));
    for blk in 0..3 {
        for x in 0..4 {
            wb_eval[x].add_assign(c2_mux_scale[blk].mul(c2_mux_dot[blk][x], kt));
        }
    }
    for lane in 0..6 {
        let x = lane % 5;
        let t = c2_cur_scale[lane].mul(c2_cur_dot[lane], kt);
        wb_eval[x].add_assign(t);
        wc_eval[x].add_assign(t);
    }
    for blk in 0..2 {
        for x in 0..3 {
            wb_eval[x].add_assign(c2_sib_scale[blk].mul(c2_sib_dot[blk][x], kt));
        }
    }

    // C3, C4, C5 (remaining 3,778 constraints)
    for c in &rest_cs {
        for &(x, row) in &c.lin {
            let r = row as usize;
            b_lo[x as usize][r & 63].add_assign(w.mul(eq_hi[r >> 6], kt));
        }
        for &(x, row) in &c.quad {
            let r = row as usize;
            c_lo[x as usize][r & 63].add_assign(w.mul(eq_hi[r >> 6], kt));
        }
        if c.rhs != 0 {
            pub_rhs.add_assign(w);
        }
        w = w.mul(ch.alpha, kt);
    }

    for x in 0..5 {
        for z in 0..64 {
            if !b_lo[x][z].is_zero() {
                wb_eval[x].add_assign(b_lo[x][z].mul(eq_lo[z], kt));
            }
            if !c_lo[x][z].is_zero() {
                wc_eval[x].add_assign(c_lo[x][z].mul(eq_lo[z], kt));
            }
        }
    }

    (wb_eval, wc_eval, pub_rhs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::field::k_tables;
    use crate::shake256::MerkleTree24;

    #[test]
    fn test_circuit_weights_and_zero_sum_real_and_dummy() {
        let kt = k_tables();
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

        for mode in 0..4 {
            let (is_dummy0, is_dummy1, v_in0, v_in1, v_mint, v_unshield, v_out0, v_out1, fee) = match mode {
                0 => (false, false, 700u64, 300u64, 0u64, 0u64, 650u64, 340u64, 10u64),   // 2-in/2-out
                1 => (false, true,  700u64, 0u64,   0u64, 0u64, 400u64, 290u64, 10u64),   // 1-in+1-dummy
                2 => (true,  true,  0u64,   0u64, 1000u64, 0u64, 990u64, 0u64,  10u64),   // ZK-shielded mint
                _ => (false, true,  700u64, 0u64,   0u64, 150u64, 540u64, 0u64, 10u64),   // partial unshield
            };
            let c0_eff = coin_commit(&a0, v_in0, &r_in0);
            let c1_eff = coin_commit(&a1, v_in1, &r_in1);
            let stmt = PaymentStatement {
                anchor,
                sn0: nullifier(&s0, &c0_eff),
                sn1: nullifier(&s1, &c1_eff),
                coin_out0: coin_commit(&a_out0, v_out0, &r_out0),
                coin_out1: coin_commit(&a_out1, v_out1, &r_out1),
                v_mint,
                v_unshield,
                fee,
                tx_ctx: [0x99u8; 48],
            };
            let wit = PaymentWitness {
                is_dummy0,
                is_dummy1,
                dirs0: if is_dummy0 { [0u8; MERKLE_DEPTH] } else { dirs0 },
                sibs0: if is_dummy0 { [[0u8; NODE_BYTES]; MERKLE_DEPTH] } else { sibs0 },
                dirs1: if is_dummy1 { [0u8; MERKLE_DEPTH] } else { dirs1 },
                sibs1: if is_dummy1 { [[0u8; NODE_BYTES]; MERKLE_DEPTH] } else { sibs1 },
                s_in0: s0,
                s_in1: s1,
                v_in0,
                v_in1,
                r_in0,
                r_in1,
                addr_out0: a_out0,
                addr_out1: a_out1,
                v_out0,
                v_out1,
                r_out0,
                r_out1,
            };
            let bits = build_witness_bits(&stmt, &wit);
            for x in 0..5 {
                assert!(bits.b[x][ACTIVE_SUBROWS_PER_POLY * 16384..].iter().all(|&v| v == 0));
            }
            let ch = CircuitChallenge {
                alpha: E { c: core::array::from_fn(|i| 0x1357_2468u32.wrapping_mul((i + 1) as u32)) },
            };
            let (wb, wc, pub_rhs) = build_prover_weights(&stmt, &ch, kt);

            let mut sum = E::ZERO;
            for i in 0..NUM_BITS_PER_POLY {
                let b_row = [bits.b[0][i], bits.b[1][i], bits.b[2][i], bits.b[3][i], bits.b[4][i]];
                for x in 0..5 {
                    if b_row[x] != 0 { sum.add_assign(wb[x][i]); }
                    if ((1 ^ b_row[(x + 1) % 5]) & b_row[(x + 2) % 5]) != 0 { sum.add_assign(wc[x][i]); }
                }
            }
            assert_eq!(sum, pub_rhs, "circuit zero-sum failed (mode={})", mode);

            let r1: [E; V_SC1] = core::array::from_fn(|k| E {
                c: core::array::from_fn(|i| ((k * 13 + i * 7 + 3) as u32).wrapping_mul(0x9E37_79B9)),
            });
            let (vwb, vwc, vrhs) = eval_verifier_weights_at_r1(&stmt, &ch, &r1, kt);
            assert_eq!(vrhs, pub_rhs, "pub_rhs mismatch!");
            let eq_r1 = eq_table(&r1, kt);
            for x in 0..5 {
                let mut db = E::ZERO;
                let mut dc = E::ZERO;
                for i in 0..NUM_BITS_PER_POLY {
                    if !wb[x][i].is_zero() { db.add_assign(wb[x][i].mul(eq_r1[i], kt)); }
                    if !wc[x][i].is_zero() { dc.add_assign(wc[x][i].mul(eq_r1[i], kt)); }
                }
                assert_eq!(vwb[x], db, "wb mismatch x={}", x);
                assert_eq!(vwc[x], dc, "wc mismatch x={}", x);
            }
        }
    }
}
