//! NIST FIPS-202 SHAKE256 primitives and Depth-24 global Merkle accumulator
//! for Peel-Binius-Shielded (2^24 = 16,777,216 coins, 100% 1-permutation Keccak-f[1600]).
//!
//! All four domain-separated in-circuit calls fit strictly inside one SHAKE256 rate block (<= 135 B):
//!   - Address:    A          = SHAKE256(0x00 || S[48])                      [49 B  -> 1 perm, 48 B out]
//!   - Coin:       Coin       = SHAKE256(0x01 || A[48] || v[8] || R[78])     [135 B -> 1 perm, 48 B out]
//!   - Nullifier:  SN         = SHAKE256(0x02 || S[48] || Coin[48])          [97 B  -> 1 perm, 48 B out]
//!   - MerkleNode: Node       = SHAKE256(0x03 || Left[48] || Right[48])      [97 B  -> 1 perm, 48 B out]

use sha3::digest::{ExtendableOutput, Update, XofReader};
use sha3::Shake256;

pub const RATE_BYTES: usize = 136; // SHAKE256 r = 1088 bits = 136 bytes
pub const ADDR_BYTES: usize = 48;  // 384-bit owner address A
pub const COIN_BYTES: usize = 48;  // 384-bit coin commitment
pub const SN_BYTES: usize = 48;    // 384-bit nullifier SN
pub const NODE_BYTES: usize = 48;  // 384-bit Merkle tree digest / anchor
pub const OWNER_SECRET_BYTES: usize = 48; // 384-bit spend secret S
pub const BLIND_SECRET_BYTES: usize = 78; // 624-bit coin blinding R (1 + 48 + 8 + 78 = 135 B -> 1 perm!)

pub const MERKLE_DEPTH: usize = 24; // 2^24 = 16,777,216 global shielded coins

pub const TAG_ADDR: u8 = 0x00;
pub const TAG_COIN: u8 = 0x01;
pub const TAG_SN: u8 = 0x02;
pub const TAG_NODE: u8 = 0x03;

/// FIPS-202 Keccak-f[1600] round constants RC[0..24].
pub const KECCAK_RC: [u64; 24] = [
    0x0000000000000001,
    0x0000000000008082,
    0x800000000000808a,
    0x8000000080008000,
    0x000000000000808b,
    0x0000000080000001,
    0x8000000080008081,
    0x8000000000008009,
    0x000000000000008a,
    0x0000000000000088,
    0x0000000080008009,
    0x000000008000000a,
    0x000000008000808b,
    0x800000000000008b,
    0x8000000000008089,
    0x8000000000008003,
    0x8000000000008002,
    0x8000000000000080,
    0x000000000000800a,
    0x800000008000000a,
    0x8000000080008081,
    0x8000000000008080,
    0x0000000080000001,
    0x8000000080008008,
];

/// FIPS-202 Rho rotation offsets r[x][y].
pub const RHO_OFFSETS: [[u32; 5]; 5] = [
    [0, 36, 3, 41, 18],
    [1, 44, 10, 45, 2],
    [62, 6, 43, 15, 61],
    [28, 55, 25, 21, 56],
    [27, 20, 39, 8, 14],
];

/// Exact 1-permutation SHAKE256 padding of a message of length <= 135 bytes into 25 x 64-bit lanes.
pub fn shake256_pad_1perm(msg: &[u8]) -> [u64; 25] {
    assert!(
        msg.len() < RATE_BYTES,
        "1-perm SHAKE256 requires msg.len() <= 135 bytes, got {}",
        msg.len()
    );
    let mut block = [0u8; 200];
    block[..msg.len()].copy_from_slice(msg);
    block[msg.len()] ^= 0x1F;
    block[RATE_BYTES - 1] ^= 0x80;
    let mut state = [0u64; 25];
    for i in 0..25 {
        state[i] = u64::from_le_bytes(block[8 * i..8 * i + 8].try_into().unwrap());
    }
    state
}

/// Execute Keccak-f[1600] in-place, returning the post-(theta,rho,pi) state B[r][x+5y]
/// for all 24 rounds r in 0..24, and updating `state` to the final output.
pub fn keccak_f1600_trace(state: &mut [u64; 25]) -> [[u64; 25]; 24] {
    let mut b_trace = [[0u64; 25]; 24];
    for r in 0..24 {
        let mut c = [0u64; 5];
        for x in 0..5 {
            c[x] = state[x] ^ state[x + 5] ^ state[x + 10] ^ state[x + 15] ^ state[x + 20];
        }
        let mut d = [0u64; 5];
        for x in 0..5 {
            d[x] = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
        }
        for x in 0..5 {
            for y in 0..5 {
                state[x + 5 * y] ^= d[x];
            }
        }
        let mut b = [0u64; 25];
        for x in 0..5 {
            for y in 0..5 {
                let ny = (2 * x + 3 * y) % 5;
                let nx = y;
                b[nx + 5 * ny] = state[x + 5 * y].rotate_left(RHO_OFFSETS[x][y]);
            }
        }
        b_trace[r] = b;
        for y in 0..5 {
            for x in 0..5 {
                state[x + 5 * y] = b[x + 5 * y] ^ ((!b[((x + 1) % 5) + 5 * y]) & b[((x + 2) % 5) + 5 * y]);
            }
        }
        state[0] ^= KECCAK_RC[r];
    }
    b_trace
}

/// For any output bit (nx, ny, nz) of L_TRP = pi o rho o theta, return the exact 11 input bit
/// coordinates (x, y, z) whose GF(2) XOR equals B[nx, ny, nz].
pub fn trp_input_bits(nx: usize, ny: usize, nz: usize) -> [(usize, usize, usize); 11] {
    let y = nx;
    let x = (3 * (ny + 2 * nx)) % 5;
    let z = (nz + 64 - RHO_OFFSETS[x][y] as usize) % 64;
    let xm1 = (x + 4) % 5;
    let xp1 = (x + 1) % 5;
    let zm1 = (z + 63) % 64;
    [
        (x, y, z),
        (xm1, 0, z),
        (xm1, 1, z),
        (xm1, 2, z),
        (xm1, 3, z),
        (xm1, 4, z),
        (xp1, 0, zm1),
        (xp1, 1, zm1),
        (xp1, 2, zm1),
        (xp1, 3, zm1),
        (xp1, 4, zm1),
    ]
}

/// Compute 48-byte (384-bit) SHAKE256 digest via our 1-perm reference Keccak-f[1600].
pub fn shake256_48(msg: &[u8]) -> [u8; 48] {
    let mut state = shake256_pad_1perm(msg);
    keccak_f1600_trace(&mut state);
    let mut out = [0u8; 48];
    for i in 0..6 {
        out[8 * i..8 * i + 8].copy_from_slice(&state[i].to_le_bytes());
    }
    out
}

pub fn owner_addr(s: &[u8; OWNER_SECRET_BYTES]) -> [u8; ADDR_BYTES] {
    let mut msg = [0u8; 1 + OWNER_SECRET_BYTES];
    msg[0] = TAG_ADDR;
    msg[1..].copy_from_slice(s);
    shake256_48(&msg)
}

pub fn coin_commit(
    addr: &[u8; ADDR_BYTES],
    value: u64,
    r: &[u8; BLIND_SECRET_BYTES],
) -> [u8; COIN_BYTES] {
    let mut msg = [0u8; 1 + ADDR_BYTES + 8 + BLIND_SECRET_BYTES];
    msg[0] = TAG_COIN;
    msg[1..49].copy_from_slice(addr);
    msg[49..57].copy_from_slice(&value.to_le_bytes());
    msg[57..135].copy_from_slice(r);
    shake256_48(&msg)
}

pub fn nullifier(s: &[u8; OWNER_SECRET_BYTES], coin: &[u8; COIN_BYTES]) -> [u8; SN_BYTES] {
    let mut msg = [0u8; 1 + OWNER_SECRET_BYTES + COIN_BYTES];
    msg[0] = TAG_SN;
    msg[1..49].copy_from_slice(s);
    msg[49..97].copy_from_slice(coin);
    shake256_48(&msg)
}

pub fn merkle_node(left: &[u8; NODE_BYTES], right: &[u8; NODE_BYTES]) -> [u8; NODE_BYTES] {
    let mut msg = [0u8; 1 + NODE_BYTES + NODE_BYTES];
    msg[0] = TAG_NODE;
    msg[1..49].copy_from_slice(left);
    msg[49..97].copy_from_slice(right);
    shake256_48(&msg)
}

/// Precomputed empty-subtree hashes EMPTY[0..=24] for Depth-24 global accumulator.
pub fn empty_hashes() -> [[u8; NODE_BYTES]; MERKLE_DEPTH + 1] {
    let mut e = [[0u8; NODE_BYTES]; MERKLE_DEPTH + 1];
    for lvl in 0..MERKLE_DEPTH {
        e[lvl + 1] = merkle_node(&e[lvl], &e[lvl]);
    }
    e
}

/// Append-only Depth-24 SHAKE256 global coin Merkle accumulator (2^24 = 16,777,216 leaves).
#[derive(Clone, Debug)]
pub struct MerkleTree24 {
    pub leaves: Vec<[u8; COIN_BYTES]>,
    pub empty: [[u8; NODE_BYTES]; MERKLE_DEPTH + 1],
    /// Sparse level storage: nodes[lvl] contains computed prefixes at level `lvl` (0 = leaves).
    nodes: Vec<Vec<[u8; NODE_BYTES]>>,
}

impl MerkleTree24 {
    pub fn new() -> Self {
        let empty = empty_hashes();
        let nodes = vec![Vec::new(); MERKLE_DEPTH + 1];
        Self {
            leaves: Vec::new(),
            empty,
            nodes,
        }
    }

    pub fn from_leaves(leaves: &[[u8; COIN_BYTES]]) -> Self {
        let mut tree = Self::new();
        for leaf in leaves {
            tree.append(*leaf);
        }
        tree
    }

    pub fn len(&self) -> usize {
        self.leaves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.leaves.is_empty()
    }

    pub fn root(&self) -> [u8; NODE_BYTES] {
        if self.leaves.is_empty() {
            return self.empty[MERKLE_DEPTH];
        }
        self.nodes[MERKLE_DEPTH][0]
    }

    pub fn append(&mut self, coin: [u8; COIN_BYTES]) -> usize {
        let idx = self.leaves.len();
        assert!(idx < (1usize << MERKLE_DEPTH), "Depth-24 Merkle tree full");
        self.leaves.push(coin);
        let mut cur = coin;
        let mut pos = idx;
        for lvl in 0..MERKLE_DEPTH {
            if pos < self.nodes[lvl].len() {
                self.nodes[lvl][pos] = cur;
            } else {
                assert_eq!(pos, self.nodes[lvl].len());
                self.nodes[lvl].push(cur);
            }
            let (left, right) = if (pos & 1) == 0 {
                let sib = if pos + 1 < self.nodes[lvl].len() {
                    self.nodes[lvl][pos + 1]
                } else {
                    self.empty[lvl]
                };
                (cur, sib)
            } else {
                (self.nodes[lvl][pos - 1], cur)
            };
            cur = merkle_node(&left, &right);
            pos >>= 1;
        }
        if self.nodes[MERKLE_DEPTH].is_empty() {
            self.nodes[MERKLE_DEPTH].push(cur);
        } else {
            self.nodes[MERKLE_DEPTH][0] = cur;
        }
        idx
    }

    /// Return `(directions[0..24], siblings[0..24])` for leaf at `idx`.
    /// `dir[lvl] = ((idx >> lvl) & 1) as u8` (0 => current is left child, 1 => current is right child).
    pub fn auth_path(&self, idx: usize) -> ([u8; MERKLE_DEPTH], [[u8; NODE_BYTES]; MERKLE_DEPTH]) {
        assert!(idx < self.leaves.len(), "leaf index {} out of range {}", idx, self.leaves.len());
        let mut dirs = [0u8; MERKLE_DEPTH];
        let mut sibs = [[0u8; NODE_BYTES]; MERKLE_DEPTH];
        let mut pos = idx;
        for lvl in 0..MERKLE_DEPTH {
            dirs[lvl] = (pos & 1) as u8;
            let sib_pos = pos ^ 1;
            sibs[lvl] = if sib_pos < self.nodes[lvl].len() {
                self.nodes[lvl][sib_pos]
            } else {
                self.empty[lvl]
            };
            pos >>= 1;
        }
        (dirs, sibs)
    }

    pub fn verify_path(
        leaf: &[u8; COIN_BYTES],
        dirs: &[u8; MERKLE_DEPTH],
        sibs: &[[u8; NODE_BYTES]; MERKLE_DEPTH],
        expected_root: &[u8; NODE_BYTES],
    ) -> bool {
        let mut cur = *leaf;
        for lvl in 0..MERKLE_DEPTH {
            cur = if dirs[lvl] == 0 {
                merkle_node(&cur, &sibs[lvl])
            } else {
                merkle_node(&sibs[lvl], &cur)
            };
        }
        &cur == expected_root
    }
}

/// Cross-check against RustCrypto `sha3::Shake256`.
pub fn shake256_48_rustcrypto(msg: &[u8]) -> [u8; 48] {
    let mut h = Shake256::default();
    h.update(msg);
    let mut out = [0u8; 48];
    h.finalize_xof().read(&mut out);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shake256_and_merkle24() {
        for len in [0usize, 1, 49, 97, 135] {
            let msg: Vec<u8> = (0..len).map(|i| ((i * 37 + 13) & 0xff) as u8).collect();
            assert_eq!(shake256_48(&msg), shake256_48_rustcrypto(&msg));
        }
        let mut tree = MerkleTree24::new();
        for i in 0u64..17 {
            let s = [i as u8; OWNER_SECRET_BYTES];
            let a = owner_addr(&s);
            let r = [(i.wrapping_mul(7) as u8); BLIND_SECRET_BYTES];
            let c = coin_commit(&a, 100 + i, &r);
            let idx = tree.append(c);
            assert_eq!(idx, i as usize);
        }
        let root = tree.root();
        for i in 0..17 {
            let (dirs, sibs) = tree.auth_path(i);
            assert!(MerkleTree24::verify_path(&tree.leaves[i], &dirs, &sibs, &root));
        }
    }
}
