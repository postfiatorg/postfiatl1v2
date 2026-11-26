//! PFSH03 wire reader and public statement shared by the prover and validators.
//! Adds per-input external ML-DSA-65 spend authorization (pk_in0, pk_in1, sig_in0, sig_in1)
//! so spend authorization reduces directly to FIPS-204 EUF-CMA + unmodified-oracle extraction.
use crate::binius_pcs::BiniusProof;
use crate::circuit::PaymentStatement;
use crate::shake256::{
    pk_hash, BLIND_SECRET_BYTES, COIN_BYTES, MERKLE_DEPTH, NODE_BYTES, SN_BYTES,
};
use sha2::{Digest, Sha384};

pub const ML_DSA_65_PK_BYTES: usize = 1952;
pub const ML_DSA_65_SIG_BYTES: usize = 3309;

#[derive(Clone, Debug)]
pub struct MintAuth {
    pub public_key: [u8; ML_DSA_65_PK_BYTES],
    pub signature: [u8; ML_DSA_65_SIG_BYTES],
}

#[derive(Clone, Debug)]
pub struct SpendAuth {
    pub public_key: [u8; ML_DSA_65_PK_BYTES],
    pub signature: [u8; ML_DSA_65_SIG_BYTES],
}

pub const TX_MAGIC: &[u8; 8] = b"PFSH03\0\0";
pub const KEM_CT_BYTES: usize = 1568; // 1,568 B
pub const NOTE_PT_BYTES: usize = 8 + BLIND_SECRET_BYTES; // 86 B (v_le64 || R_78B)
pub const NOTE_CT_BYTES: usize = NOTE_PT_BYTES + 16; // 102 B (AES-256-GCM tag)
pub const ENV_BYTES: usize = KEM_CT_BYTES + NOTE_CT_BYTES; // 1,670 B
pub const UNSHIELD_DEST_BYTES: usize = 32;
pub const ENVELOPES_END_BYTES: usize = 8
    + NODE_BYTES
    + 8
    + 8
    + UNSHIELD_DEST_BYTES
    + 8
    + 2 * SN_BYTES
    + 2 * COIN_BYTES
    + 2 * ENV_BYTES; // 3,644 B
pub const UNSIGNED_HEADER_BYTES: usize = ENVELOPES_END_BYTES + 2 * ML_DSA_65_PK_BYTES; // 7,548 B
pub const HEADER_OVERHEAD_BYTES: usize = UNSIGNED_HEADER_BYTES + 2 * ML_DSA_65_SIG_BYTES + 4; // 14,170 B

pub const W1_ARCHIVAL_BUDGET_BYTES: usize = 160_000; // ~12.87 GiB/day @ 1 tx/s
pub const MAX_POOL_COINS: usize = 1usize << MERKLE_DEPTH; // 16,777,216 coins

#[derive(Clone, Debug)]
pub struct ShieldedTx {
    pub anchor: [u8; NODE_BYTES],
    pub v_mint: u64,
    pub v_unshield: u64,
    pub unshield_dest: [u8; UNSHIELD_DEST_BYTES],
    pub fee: u64,
    pub sn0: [u8; SN_BYTES],
    pub sn1: [u8; SN_BYTES],
    pub coin_out0: [u8; COIN_BYTES],
    pub coin_out1: [u8; COIN_BYTES],
    pub env0: [u8; ENV_BYTES],
    pub env1: [u8; ENV_BYTES],
    pub spend_auth0: Box<SpendAuth>,
    pub spend_auth1: Box<SpendAuth>,
    pub proof: BiniusProof,
    pub mint_auth: Option<Box<MintAuth>>,
}

impl ShieldedTx {
    pub fn check_public_flows(&self) -> Result<(), String> {
        if self.v_mint > 0 && self.v_unshield > 0 {
            return Err("Rule R5 violation: v_mint and v_unshield cannot both be non-zero".into());
        }
        if self.fee.checked_add(self.v_unshield).is_none() {
            return Err("Rule R5 violation: fee + v_unshield overflows u64".into());
        }
        Ok(())
    }

    pub fn verify_spend_auth(&self, statement_ctx: &[u8; 48]) -> Result<(), String> {
        if !self.spend_auth0.verify(0, statement_ctx) || !self.spend_auth1.verify(1, statement_ctx)
        {
            return Err("R0-SpendAuth: invalid ML-DSA-65 spend authorization".into());
        }
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn compute_tx_ctx(
        anchor: &[u8; NODE_BYTES],
        v_mint: u64,
        v_unshield: u64,
        unshield_dest: &[u8; UNSHIELD_DEST_BYTES],
        fee: u64,
        sn0: &[u8; SN_BYTES],
        sn1: &[u8; SN_BYTES],
        coin_out0: &[u8; COIN_BYTES],
        coin_out1: &[u8; COIN_BYTES],
        env0: &[u8; ENV_BYTES],
        env1: &[u8; ENV_BYTES],
        pk_in0: &[u8; ML_DSA_65_PK_BYTES],
        pk_in1: &[u8; ML_DSA_65_PK_BYTES],
    ) -> [u8; 48] {
        let mut h = Sha384::new();
        h.update(b"PFT-SH-TXCTX-V3");
        h.update(TX_MAGIC);
        h.update(anchor);
        h.update(v_mint.to_le_bytes());
        h.update(v_unshield.to_le_bytes());
        h.update(unshield_dest);
        h.update(fee.to_le_bytes());
        h.update(sn0);
        h.update(sn1);
        h.update(coin_out0);
        h.update(coin_out1);
        h.update(env0);
        h.update(env1);
        h.update(pk_in0);
        h.update(pk_in1);
        let mut out = [0u8; 48];
        out.copy_from_slice(&h.finalize());
        out
    }

    pub fn statement(&self) -> PaymentStatement {
        let tx_ctx = Self::compute_tx_ctx(
            &self.anchor,
            self.v_mint,
            self.v_unshield,
            &self.unshield_dest,
            self.fee,
            &self.sn0,
            &self.sn1,
            &self.coin_out0,
            &self.coin_out1,
            &self.env0,
            &self.env1,
            &self.spend_auth0.public_key,
            &self.spend_auth1.public_key,
        );
        PaymentStatement {
            anchor: self.anchor,
            sn0: self.sn0,
            sn1: self.sn1,
            coin_out0: self.coin_out0,
            coin_out1: self.coin_out1,
            v_mint: self.v_mint,
            v_unshield: self.v_unshield,
            fee: self.fee,
            tx_ctx,
            pk_hash0: pk_hash(&self.spend_auth0.public_key),
            pk_hash1: pk_hash(&self.spend_auth1.public_key),
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let proof_bytes = self.proof.to_bytes();
        let mut out = Vec::with_capacity(HEADER_OVERHEAD_BYTES + proof_bytes.len());
        out.extend_from_slice(TX_MAGIC);
        out.extend_from_slice(&self.anchor);
        out.extend_from_slice(&self.v_mint.to_le_bytes());
        out.extend_from_slice(&self.v_unshield.to_le_bytes());
        out.extend_from_slice(&self.unshield_dest);
        out.extend_from_slice(&self.fee.to_le_bytes());
        out.extend_from_slice(&self.sn0);
        out.extend_from_slice(&self.sn1);
        out.extend_from_slice(&self.coin_out0);
        out.extend_from_slice(&self.coin_out1);
        out.extend_from_slice(&self.env0);
        out.extend_from_slice(&self.env1);
        out.extend_from_slice(&self.spend_auth0.public_key);
        out.extend_from_slice(&self.spend_auth1.public_key);
        out.extend_from_slice(&self.spend_auth0.signature);
        out.extend_from_slice(&self.spend_auth1.signature);
        out.extend_from_slice(&(proof_bytes.len() as u32).to_le_bytes());
        out.extend_from_slice(&proof_bytes);
        if let Some(auth) = &self.mint_auth {
            out.extend_from_slice(&auth.public_key);
            out.extend_from_slice(&auth.signature);
        }
        out
    }

    pub fn from_bytes(data: &[u8]) -> Result<Self, String> {
        Self::parse(data, true)
    }

    pub fn from_l1_inner(data: &[u8]) -> Result<Self, String> {
        Self::parse(data, false)
    }

    fn parse(data: &[u8], standalone: bool) -> Result<Self, String> {
        if data.len() > W1_ARCHIVAL_BUDGET_BYTES {
            return Err("shielded tx exceeds archival budget".into());
        }
        if data.len() < HEADER_OVERHEAD_BYTES {
            return Err("truncated shielded tx header".into());
        }
        let mut pos = 0usize;
        let take = |pos: &mut usize, n: usize| -> Result<&[u8], String> {
            if n > data.len() - *pos {
                return Err("truncated shielded tx".into());
            }
            let s = &data[*pos..*pos + n];
            *pos += n;
            Ok(s)
        };
        if take(&mut pos, 8)? != TX_MAGIC {
            return Err("bad shielded tx magic".into());
        }
        let anchor: [u8; NODE_BYTES] = take(&mut pos, NODE_BYTES)?.try_into().unwrap();
        let v_mint = u64::from_le_bytes(take(&mut pos, 8)?.try_into().unwrap());
        let v_unshield = u64::from_le_bytes(take(&mut pos, 8)?.try_into().unwrap());
        let unshield_dest: [u8; UNSHIELD_DEST_BYTES] =
            take(&mut pos, UNSHIELD_DEST_BYTES)?.try_into().unwrap();
        let fee = u64::from_le_bytes(take(&mut pos, 8)?.try_into().unwrap());
        let sn0: [u8; SN_BYTES] = take(&mut pos, SN_BYTES)?.try_into().unwrap();
        let sn1: [u8; SN_BYTES] = take(&mut pos, SN_BYTES)?.try_into().unwrap();
        let coin_out0: [u8; COIN_BYTES] = take(&mut pos, COIN_BYTES)?.try_into().unwrap();
        let coin_out1: [u8; COIN_BYTES] = take(&mut pos, COIN_BYTES)?.try_into().unwrap();
        let env0: [u8; ENV_BYTES] = take(&mut pos, ENV_BYTES)?.try_into().unwrap();
        let env1: [u8; ENV_BYTES] = take(&mut pos, ENV_BYTES)?.try_into().unwrap();
        let pk_in0: [u8; ML_DSA_65_PK_BYTES] =
            take(&mut pos, ML_DSA_65_PK_BYTES)?.try_into().unwrap();
        let pk_in1: [u8; ML_DSA_65_PK_BYTES] =
            take(&mut pos, ML_DSA_65_PK_BYTES)?.try_into().unwrap();
        let sig_in0: [u8; ML_DSA_65_SIG_BYTES] =
            take(&mut pos, ML_DSA_65_SIG_BYTES)?.try_into().unwrap();
        let sig_in1: [u8; ML_DSA_65_SIG_BYTES] =
            take(&mut pos, ML_DSA_65_SIG_BYTES)?.try_into().unwrap();
        let proof_len = u32::from_le_bytes(take(&mut pos, 4)?.try_into().unwrap()) as usize;
        let proof_bytes = take(&mut pos, proof_len)?;
        let mint_auth = if standalone && v_mint > 0 {
            Some(Box::new(MintAuth {
                public_key: take(&mut pos, ML_DSA_65_PK_BYTES)?.try_into().unwrap(),
                signature: take(&mut pos, ML_DSA_65_SIG_BYTES)?.try_into().unwrap(),
            }))
        } else {
            None
        };
        if pos != data.len() {
            return Err("trailing bytes in shielded tx".into());
        }
        let proof = BiniusProof::from_bytes(proof_bytes)?;
        Ok(Self {
            anchor,
            v_mint,
            v_unshield,
            unshield_dest,
            fee,
            sn0,
            sn1,
            coin_out0,
            coin_out1,
            env0,
            env1,
            spend_auth0: Box::new(SpendAuth {
                public_key: pk_in0,
                signature: sig_in0,
            }),
            spend_auth1: Box::new(SpendAuth {
                public_key: pk_in1,
                signature: sig_in1,
            }),
            proof,
            mint_auth,
        })
    }
}
