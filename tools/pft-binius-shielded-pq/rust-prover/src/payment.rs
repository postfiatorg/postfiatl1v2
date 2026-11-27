//! 0-Round Non-Interactive Shielded Payment Protocol ("Peel-Binius-Shielded")
//! with Depth-24 Global Merkle Accumulator (16,777,216 coins), Native `is_dummy` 1-Input Spends,
//! and FIPS-203 ML-KEM-1024 + HMAC-SHA-384 + AES-256-GCM Stealth Receive.
//!
//! On-chain transaction footprint matches Zcash Orchard:
//!   (magic[8], anchor[48], fee[8], sn0[48], sn1[48], coin0[48], coin1[48], env0[1670], env1[1670], proof)
//! Positive mints append a fixed ML-DSA-65 public key and signature after the proof.

use crate::binius_pcs::{prove, verify};
use crate::circuit::{build_witness_bits, PaymentStatement, PaymentWitness};
use crate::mint_auth::{spend_keypair_from_secret, MintAuth, SpendAuth};
use crate::shake256::{
    coin_commit, nullifier, owner_addr, MerkleTree24, ADDR_BYTES, BLIND_SECRET_BYTES, COIN_BYTES,
    MERKLE_DEPTH, NODE_BYTES, OWNER_SECRET_BYTES, PK_HASH_BYTES, SN_BYTES,
};
use aes_gcm::aead::{Aead, KeyInit, Payload};
use aes_gcm::{Aes256Gcm, Nonce};
use fips203::ml_kem_1024;
use fips203::traits::{Decaps, Encaps, KeyGen, SerDes};
use fips204::ml_dsa_65;
use fips204::traits::SerDes as DsaSerDes;
use hmac::{Hmac, Mac};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha384;
use std::collections::HashSet;

pub use crate::wire::{
    ShieldedTx, ENV_BYTES, HEADER_OVERHEAD_BYTES, KEM_CT_BYTES, MAX_POOL_COINS, NOTE_CT_BYTES,
    NOTE_PT_BYTES, TX_MAGIC, UNSHIELD_DEST_BYTES, W1_ARCHIVAL_BUDGET_BYTES,
};
pub const KEM_EK_BYTES: usize = ml_kem_1024::EK_LEN;
pub const KEM_DK_BYTES: usize = ml_kem_1024::DK_LEN;

#[derive(Clone, Debug)]
pub struct StealthAddress {
    pub ek_bytes: [u8; KEM_EK_BYTES],
    pub addr: [u8; ADDR_BYTES],
}

#[derive(Clone)]
pub struct ShieldedWallet {
    pub spend_secret: [u8; OWNER_SECRET_BYTES],
    pub r_pk: [u8; OWNER_SECRET_BYTES],
    pub pk_hash: [u8; PK_HASH_BYTES],
    pub addr: [u8; ADDR_BYTES],
    pub kem_seed: [u8; 64],
    pub ek_bytes: [u8; KEM_EK_BYTES],
    pub dk_bytes: [u8; KEM_DK_BYTES],
}

impl ShieldedWallet {
    pub fn from_seed(rng: &mut impl RngCore) -> Self {
        let mut spend_secret = [0u8; OWNER_SECRET_BYTES];
        rng.fill_bytes(&mut spend_secret);
        let mut kem_seed = [0u8; 64];
        rng.fill_bytes(&mut kem_seed);
        Self::from_secrets(spend_secret, kem_seed)
    }

    pub fn from_secrets(spend_secret: [u8; OWNER_SECRET_BYTES], kem_seed: [u8; 64]) -> Self {
        let (_, _, pk_hash, r_pk) = spend_keypair_from_secret(&spend_secret);
        let addr = owner_addr(&pk_hash, &r_pk);
        let d: [u8; 32] = kem_seed[0..32].try_into().unwrap();
        let z: [u8; 32] = kem_seed[32..64].try_into().unwrap();
        let (ek, dk) = ml_kem_1024::KG::keygen_from_seed(d, z);
        Self {
            spend_secret,
            r_pk,
            pk_hash,
            addr,
            kem_seed,
            ek_bytes: ek.into_bytes(),
            dk_bytes: dk.into_bytes(),
        }
    }

    pub fn stealth_address(&self) -> StealthAddress {
        StealthAddress {
            ek_bytes: self.ek_bytes,
            addr: self.addr,
        }
    }

    /// 0-round offline trial decryption of an on-chain stealth output `(coin, env)`.
    /// Returns `Some((value, r_blind))` iff AES-256-GCM authenticates AND `Coin(self.addr, v, r) == coin`.
    pub fn try_scan_output(
        &self,
        coin: &[u8; COIN_BYTES],
        env: &[u8; ENV_BYTES],
    ) -> Option<(u64, [u8; BLIND_SECRET_BYTES])> {
        let ct_bytes: [u8; KEM_CT_BYTES] = env[..KEM_CT_BYTES].try_into().ok()?;
        let ct_aead = &env[KEM_CT_BYTES..];
        let dk = ml_kem_1024::DecapsKey::try_from_bytes(self.dk_bytes).ok()?;
        let ct = ml_kem_1024::CipherText::try_from_bytes(ct_bytes).ok()?;
        let ss = dk.try_decaps(&ct).ok()?.into_bytes();
        let (key, nonce) = derive_aead_key_nonce(&ss, &ct_bytes, coin);
        let cipher = Aes256Gcm::new_from_slice(&key).ok()?;
        let pt = cipher
            .decrypt(
                Nonce::from_slice(&nonce),
                Payload {
                    msg: ct_aead,
                    aad: coin.as_slice(),
                },
            )
            .ok()?;
        if pt.len() != NOTE_PT_BYTES {
            return None;
        }
        let value = u64::from_le_bytes(pt[0..8].try_into().ok()?);
        let mut r_blind = [0u8; BLIND_SECRET_BYTES];
        r_blind.copy_from_slice(&pt[8..NOTE_PT_BYTES]);
        if &coin_commit(&self.addr, value, &r_blind) != coin {
            return None;
        }
        Some((value, r_blind))
    }
}

/// Derive AES-256-GCM key (32 B) and nonce (12 B) from ML-KEM-1024 shared secret,
/// ciphertext, and coin commitment using HMAC-SHA-384 with distinct versioned labels.
pub fn derive_aead_key_nonce(
    ss: &[u8; 32],
    ct_kem: &[u8; KEM_CT_BYTES],
    coin: &[u8; COIN_BYTES],
) -> ([u8; 32], [u8; 12]) {
    let derive = |label: &[u8]| {
        let mut mac =
            <Hmac<Sha384> as Mac>::new_from_slice(ss).expect("HMAC accepts a 32-byte key");
        mac.update(label);
        mac.update(ct_kem);
        mac.update(coin);
        mac.finalize().into_bytes()
    };
    let key = derive(b"PFT-SH-KEM-KEY-V2")[..32].try_into().unwrap();
    let nonce = derive(b"PFT-SH-KEM-NONCE-V2")[..12].try_into().unwrap();
    (key, nonce)
}

/// Seal `(value, r_blind)` into a 1,670-byte anonymous ML-KEM-1024 + AES-256-GCM stealth envelope.
pub fn seal_stealth_envelope(
    recipient_ek: &[u8; KEM_EK_BYTES],
    coin: &[u8; COIN_BYTES],
    value: u64,
    r_blind: &[u8; BLIND_SECRET_BYTES],
    rng: &mut impl RngCore,
) -> Result<[u8; ENV_BYTES], String> {
    let ek = ml_kem_1024::EncapsKey::try_from_bytes(*recipient_ek)
        .map_err(|e| format!("invalid ML-KEM-1024 encaps key: {e}"))?;
    let mut m_seed = [0u8; 32];
    rng.fill_bytes(&mut m_seed);
    let (ss, ct) = ek.encaps_from_seed(&m_seed);
    let ss_bytes = ss.into_bytes();
    let ct_bytes = ct.into_bytes();
    let (key, nonce) = derive_aead_key_nonce(&ss_bytes, &ct_bytes, coin);
    let cipher = Aes256Gcm::new_from_slice(&key).map_err(|_| "AES-256-GCM init failed")?;
    let mut pt = [0u8; NOTE_PT_BYTES];
    pt[0..8].copy_from_slice(&value.to_le_bytes());
    pt[8..NOTE_PT_BYTES].copy_from_slice(r_blind);
    let ct_aead = cipher
        .encrypt(
            Nonce::from_slice(&nonce),
            Payload {
                msg: &pt,
                aad: coin.as_slice(),
            },
        )
        .map_err(|_| "AES-256-GCM encrypt failed")?;
    assert_eq!(ct_aead.len(), NOTE_CT_BYTES);
    let mut env = [0u8; ENV_BYTES];
    env[..KEM_CT_BYTES].copy_from_slice(&ct_bytes);
    env[KEM_CT_BYTES..].copy_from_slice(&ct_aead);
    Ok(env)
}

/// Input note specification for building a shielded transaction.
#[derive(Clone, Debug)]
pub enum SpendInput {
    Real {
        leaf_index: usize,
        spend_secret: [u8; OWNER_SECRET_BYTES],
        value: u64,
        r_blind: [u8; BLIND_SECRET_BYTES],
    },
    /// Native zero-value dummy input (requires ZERO pre-existing coins on-chain!).
    Dummy,
}

/// Build a 0-round non-interactive shielded payment (supports ZK-shielded mint `v_mint > 0`,
/// 2-real, 1-real+1-dummy, and partial transparent exit `v_unshield > 0`).
pub fn create_shielded_tx(
    tree: &MerkleTree24,
    in0: SpendInput,
    in1: SpendInput,
    out0_dest: &StealthAddress,
    out0_val: u64,
    out1_dest: &StealthAddress,
    out1_val: u64,
    v_mint: u64,
    v_unshield: u64,
    unshield_dest: [u8; UNSHIELD_DEST_BYTES],
    fee: u64,
    depositor: Option<&ml_dsa_65::PrivateKey>,
    rng: &mut impl RngCore,
) -> Result<
    (
        ShieldedTx,
        [u8; BLIND_SECRET_BYTES],
        [u8; BLIND_SECRET_BYTES],
    ),
    String,
> {
    create_profiled_tx(
        tree,
        in0,
        in1,
        out0_dest,
        out0_val,
        out1_dest,
        out1_val,
        v_mint,
        v_unshield,
        unshield_dest,
        fee,
        depositor,
        None,
        rng,
    )
}

/// A wallet may have joined at a certified frontier and hold only later leaves.
pub type Opening = ([u8; MERKLE_DEPTH], [[u8; NODE_BYTES]; MERKLE_DEPTH]);
pub trait PaymentTree {
    fn root(&self) -> [u8; NODE_BYTES];
    fn opening(&self, index: usize, coin: &[u8; COIN_BYTES]) -> Option<Opening>;
}
impl PaymentTree for MerkleTree24 {
    fn root(&self) -> [u8; NODE_BYTES] {
        self.root()
    }
    fn opening(&self, index: usize, coin: &[u8; COIN_BYTES]) -> Option<Opening> {
        (self.leaves.get(index) == Some(coin)).then(|| self.auth_path(index))
    }
}

pub fn create_profiled_tx(
    tree: &impl PaymentTree,
    in0: SpendInput,
    in1: SpendInput,
    out0_dest: &StealthAddress,
    out0_val: u64,
    out1_dest: &StealthAddress,
    out1_val: u64,
    v_mint: u64,
    v_unshield: u64,
    unshield_dest: [u8; UNSHIELD_DEST_BYTES],
    fee: u64,
    depositor: Option<&ml_dsa_65::PrivateKey>,
    l1_context: Option<&dyn Fn([u8; 48]) -> [u8; 48]>,
    rng: &mut impl RngCore,
) -> Result<
    (
        ShieldedTx,
        [u8; BLIND_SECRET_BYTES],
        [u8; BLIND_SECRET_BYTES],
    ),
    String,
> {
    if v_mint > 0 && v_unshield > 0 {
        return Err("Rule R5 violation: v_mint and v_unshield are mutually exclusive".into());
    }
    if depositor.is_some() != (v_mint > 0 && l1_context.is_none()) {
        return Err("R0-Auth: depositor key required exactly for positive mint".into());
    }
    let pub_exit = fee
        .checked_add(v_unshield)
        .ok_or_else(|| "Rule R5 violation: fee + v_unshield overflows u64".to_string())?;
    let anchor = tree.root();

    let resolve_input = |inp: SpendInput,
                         rng: &mut dyn RngCore|
     -> Result<
        (
            bool,
            [u8; MERKLE_DEPTH],
            [[u8; NODE_BYTES]; MERKLE_DEPTH],
            ml_dsa_65::PrivateKey,
            [u8; ml_dsa_65::PK_LEN],
            [u8; PK_HASH_BYTES],
            [u8; OWNER_SECRET_BYTES],
            u64,
            [u8; BLIND_SECRET_BYTES],
            [u8; SN_BYTES],
        ),
        String,
    > {
        match inp {
            SpendInput::Real {
                leaf_index,
                spend_secret,
                value,
                r_blind,
            } => {
                let (pk_dsa, sk_dsa, pk_h, r_pk) = spend_keypair_from_secret(&spend_secret);
                let coin = coin_commit(&owner_addr(&pk_h, &r_pk), value, &r_blind);
                let (dirs, sibs) = tree
                    .opening(leaf_index, &coin)
                    .ok_or("input opening unavailable")?;
                if !MerkleTree24::verify_path(&coin, &dirs, &sibs, &anchor) {
                    return Err("input does not open at selected anchor".into());
                }
                let sn = nullifier(&r_pk, &coin);
                Ok((
                    false,
                    dirs,
                    sibs,
                    sk_dsa,
                    pk_dsa.into_bytes(),
                    pk_h,
                    r_pk,
                    value,
                    r_blind,
                    sn,
                ))
            }
            SpendInput::Dummy => {
                let mut s_d = [0u8; OWNER_SECRET_BYTES];
                let mut r_d = [0u8; BLIND_SECRET_BYTES];
                rng.fill_bytes(&mut s_d);
                rng.fill_bytes(&mut r_d);
                let (pk_dsa, sk_dsa, pk_h, r_pk) = spend_keypair_from_secret(&s_d);
                let a_d = owner_addr(&pk_h, &r_pk);
                let c_d = coin_commit(&a_d, 0, &r_d);
                let sn_d = nullifier(&r_pk, &c_d);
                Ok((
                    true,
                    [0u8; MERKLE_DEPTH],
                    [[0u8; NODE_BYTES]; MERKLE_DEPTH],
                    sk_dsa,
                    pk_dsa.into_bytes(),
                    pk_h,
                    r_pk,
                    0,
                    r_d,
                    sn_d,
                ))
            }
        }
    };

    let (is_dummy0, dirs0, sibs0, sk_in0, pk_in0, pk_hash0, r_pk0, v_in0, r_in0, sn0) =
        resolve_input(in0, rng)?;
    let (is_dummy1, dirs1, sibs1, sk_in1, pk_in1, pk_hash1, r_pk1, v_in1, r_in1, sn1) =
        resolve_input(in1, rng)?;

    if sn0 == sn1 {
        return Err("Rule R2 violation: duplicate nullifiers sn0 == sn1 within transaction".into());
    }

    if (v_in0 as u128) + (v_in1 as u128) + (v_mint as u128)
        != (out0_val as u128) + (out1_val as u128) + (pub_exit as u128)
    {
        return Err("value conservation mismatch".into());
    }

    let mut r_out0 = [0u8; BLIND_SECRET_BYTES];
    let mut r_out1 = [0u8; BLIND_SECRET_BYTES];
    rng.fill_bytes(&mut r_out0);
    rng.fill_bytes(&mut r_out1);

    let coin_out0 = coin_commit(&out0_dest.addr, out0_val, &r_out0);
    let coin_out1 = coin_commit(&out1_dest.addr, out1_val, &r_out1);

    let env0 = seal_stealth_envelope(&out0_dest.ek_bytes, &coin_out0, out0_val, &r_out0, rng)?;
    let env1 = seal_stealth_envelope(&out1_dest.ek_bytes, &coin_out1, out1_val, &r_out1, rng)?;

    let tx_ctx = ShieldedTx::compute_tx_ctx(
        &anchor,
        v_mint,
        v_unshield,
        &unshield_dest,
        fee,
        &sn0,
        &sn1,
        &coin_out0,
        &coin_out1,
        &env0,
        &env1,
        &pk_in0,
        &pk_in1,
    );

    let statement_ctx = l1_context.map_or(tx_ctx, |bind| bind(tx_ctx));
    let spend_auth0 = SpendAuth::sign(0, &sk_in0, &statement_ctx)?;
    let spend_auth1 = SpendAuth::sign(1, &sk_in1, &statement_ctx)?;
    let mint_auth = depositor
        .map(|key| MintAuth::sign(key, &tx_ctx))
        .transpose()?;
    let stmt = PaymentStatement {
        anchor,
        sn0,
        sn1,
        coin_out0,
        coin_out1,
        v_mint,
        v_unshield,
        fee,
        tx_ctx: statement_ctx,
        pk_hash0,
        pk_hash1,
    };
    let wit = PaymentWitness {
        is_dummy0,
        is_dummy1,
        dirs0,
        sibs0,
        dirs1,
        sibs1,
        s_in0: r_pk0,
        s_in1: r_pk1,
        v_in0,
        v_in1,
        r_in0,
        r_in1,
        addr_out0: out0_dest.addr,
        addr_out1: out1_dest.addr,
        v_out0: out0_val,
        v_out1: out1_val,
        r_out0,
        r_out1,
    };

    let wit_bits = build_witness_bits(&stmt, &wit);
    let proof = prove(&stmt, &wit_bits, rng);

    let tx = ShieldedTx {
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
        spend_auth0: Box::new(spend_auth0),
        spend_auth1: Box::new(spend_auth1),
        proof,
        mint_auth: mint_auth.map(Box::new),
    };
    Ok((tx, r_out0, r_out1))
}

/// JSON-serializable public ledger snapshot for independent Rust & Go validator replay.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LedgerJson {
    pub coins: Vec<String>,
    pub anchors: Vec<String>,
    pub spent_sn: Vec<String>,
    #[serde(default)]
    pub leaf_count_override: Option<usize>,
}

#[derive(Clone)]
pub struct LedgerState {
    /// Active append tree. Frozen epochs retain paths for single-anchor spends.
    pub tree: MerkleTree24,
    frozen: Vec<MerkleTree24>,
    pub coin_set: HashSet<[u8; COIN_BYTES]>,
    pub anchors: HashSet<[u8; NODE_BYTES]>,
    pub anchor_list: Vec<[u8; NODE_BYTES]>,
    pub spent_sn: HashSet<[u8; SN_BYTES]>,
    pub spent_sn_list: Vec<[u8; SN_BYTES]>,
    pub supply_minted: u128,
    pub unshielded_acc: u128,
    pub fees_acc: u128,
    /// Optional simulated tree occupancy (used ONLY in capacity-overflow adversarial testing).
    pub leaf_count_override: Option<usize>,
}

impl LedgerState {
    pub fn new() -> Self {
        let tree = MerkleTree24::new();
        let root = tree.root();
        let mut anchors = HashSet::new();
        anchors.insert(root);
        Self {
            tree,
            frozen: Vec::new(),
            coin_set: HashSet::new(),
            anchors,
            anchor_list: vec![root],
            spent_sn: HashSet::new(),
            spent_sn_list: Vec::new(),
            supply_minted: 0,
            unshielded_acc: 0,
            fees_acc: 0,
            leaf_count_override: None,
        }
    }

    pub fn effective_leaf_count(&self) -> usize {
        self.leaf_count_override.unwrap_or_else(|| self.tree.len())
    }

    pub fn active_epoch(&self) -> usize {
        self.frozen.len()
    }

    /// Resolve a global output index returned by settlement into a witness tree and local index.
    pub fn locate(&self, index: usize) -> Option<(&MerkleTree24, usize)> {
        let epoch = index / MAX_POOL_COINS;
        let tree = if epoch == self.active_epoch() {
            &self.tree
        } else {
            self.frozen.get(epoch)?
        };
        let local = index % MAX_POOL_COINS;
        (local < tree.len()).then_some((tree, local))
    }

    fn remember_anchor(&mut self, root: [u8; NODE_BYTES]) {
        if self.anchors.insert(root) {
            self.anchor_list.push(root);
        }
    }

    fn finish_append(&mut self) {
        self.remember_anchor(self.tree.root());
        if self.tree.len() == MAX_POOL_COINS {
            self.frozen
                .push(std::mem::replace(&mut self.tree, MerkleTree24::new()));
            self.remember_anchor(self.tree.root());
        }
    }

    /// Bootstrap a snapshot from ordered commitments, retaining each epoch's terminal anchor.
    /// Historical intermediate anchors and nullifiers are restored separately by from_json.
    pub fn from_coins(
        coins: impl IntoIterator<Item = Result<[u8; COIN_BYTES], String>>,
    ) -> Result<Self, String> {
        let mut state = Self::new();
        let iter = coins.into_iter();
        let size = iter.size_hint().0;
        state.coin_set.reserve(size);
        let mut active = Vec::with_capacity(size.min(MAX_POOL_COINS));
        for coin in iter {
            let coin = coin?;
            if coin == [0; COIN_BYTES] || !state.coin_set.insert(coin) {
                return Err("invalid or duplicate snapshot commitment".into());
            }
            active.push(coin);
            if active.len() == MAX_POOL_COINS {
                state.tree = MerkleTree24::from_owned_leaves(std::mem::take(&mut active));
                state.finish_append();
            }
        }
        state.tree = MerkleTree24::from_owned_leaves(active);
        state.remember_anchor(state.tree.root());
        Ok(state)
    }

    pub fn mint_coin(&mut self, coin: [u8; COIN_BYTES]) -> Result<usize, String> {
        if coin == [0u8; COIN_BYTES] {
            return Err("Rule R4 violation: zero coin commitment 0^48 forbidden".into());
        }
        if self.effective_leaf_count() >= MAX_POOL_COINS {
            return Err("Rule R6 violation: Depth-24 Merkle pool capacity 2^24 exceeded".into());
        }
        if self.coin_set.contains(&coin) {
            return Err("duplicate coin commitment on mint".into());
        }
        let idx = self.active_epoch() * MAX_POOL_COINS + self.tree.append(coin);
        self.coin_set.insert(coin);
        self.finish_append();
        Ok(idx)
    }

    /// Full stateless + stateful settlement verification enforcing Rules R1-R6 (without mutating state).
    pub fn verify_tx(&self, tx: &ShieldedTx) -> Result<(), String> {
        // Rule R6: strict Depth-24 tree capacity guard (|Coins| + 2 <= 2^24)
        if self.effective_leaf_count() > MAX_POOL_COINS - 2 {
            return Err("Rule R6 violation: Depth-24 Merkle pool capacity 2^24 exceeded".into());
        }
        // Rule R1: anchor membership
        if !self.anchors.contains(&tx.anchor) {
            return Err("Rule R1 violation: unknown or invalid Merkle anchor".into());
        }
        // Rule R2: intra-transaction nullifier distinctness
        if tx.sn0 == tx.sn1 {
            return Err(
                "Rule R2 violation: duplicate nullifiers sn0 == sn1 within transaction".into(),
            );
        }
        // Rule R3: global nullifier freshness (double-spend prevention)
        if self.spent_sn.contains(&tx.sn0) {
            return Err("Rule R3 violation: double spend: sn0 already spent".into());
        }
        if self.spent_sn.contains(&tx.sn1) {
            return Err("Rule R3 violation: double spend: sn1 already spent".into());
        }
        // Rule R4: strict output coin uniqueness & non-zero sentinel exclusion (coin != 0^48)
        let zero_coin = [0u8; COIN_BYTES];
        if tx.coin_out0 == zero_coin || tx.coin_out1 == zero_coin {
            return Err(
                "Rule R4 violation: output coin commitment equals empty sentinel 0^48".into(),
            );
        }
        if tx.coin_out0 == tx.coin_out1 {
            return Err("Rule R4 violation: duplicate output coin commitments".into());
        }
        if self.coin_set.contains(&tx.coin_out0) || self.coin_set.contains(&tx.coin_out1) {
            return Err("Rule R4 violation: output coin already exists in ledger".into());
        }
        // Rule R5: mutually exclusive mint/unshield, u64 pub_exit check, and ZKP verification
        tx.check_public_flows()?;
        let stmt = tx.statement();
        match (&tx.mint_auth, tx.v_mint) {
            (None, 0) => (),
            (Some(auth), amount) if amount > 0 && auth.verify(&stmt.tx_ctx) => (),
            _ => return Err("R0-Auth: invalid mint authorization".into()),
        }
        tx.verify_spend_auth(&stmt.tx_ctx)?;
        verify(&stmt, &tx.proof)?;
        Ok(())
    }

    /// Verify and atomically settle `tx`, returning `(out0_leaf_idx, out1_leaf_idx)`.
    pub fn verify_and_apply(&mut self, tx: &ShieldedTx) -> Result<(usize, usize), String> {
        self.verify_tx(tx)?;
        self.spent_sn.insert(tx.sn0);
        self.spent_sn_list.push(tx.sn0);
        self.spent_sn.insert(tx.sn1);
        self.spent_sn_list.push(tx.sn1);
        let epoch_start = self.active_epoch() * MAX_POOL_COINS;
        let idx0 = epoch_start + self.tree.append(tx.coin_out0);
        self.coin_set.insert(tx.coin_out0);
        let idx1 = epoch_start + self.tree.append(tx.coin_out1);
        self.coin_set.insert(tx.coin_out1);
        self.supply_minted += tx.v_mint as u128;
        self.unshielded_acc += tx.v_unshield as u128;
        self.fees_acc += tx.fee as u128;
        self.finish_append();
        Ok((idx0, idx1))
    }

    pub fn to_json(&self) -> LedgerJson {
        LedgerJson {
            coins: self
                .frozen
                .iter()
                .chain(std::iter::once(&self.tree))
                .flat_map(|tree| &tree.leaves)
                .map(hex::encode)
                .collect(),
            anchors: self.anchor_list.iter().map(hex::encode).collect(),
            spent_sn: self.spent_sn_list.iter().map(hex::encode).collect(),
            leaf_count_override: self.leaf_count_override,
        }
    }

    pub fn from_json(j: &LedgerJson) -> Result<Self, String> {
        let mut st = Self::from_coins(j.coins.iter().map(|coin| {
            let bytes = hex::decode(coin).map_err(|e| format!("bad coin hex: {e}"))?;
            bytes.try_into().map_err(|_| "bad coin len".into())
        }))?;
        st.anchors.clear();
        st.anchor_list.clear();
        if j.leaf_count_override
            .is_some_and(|n| n < st.tree.len() || n > MAX_POOL_COINS)
        {
            return Err("invalid simulated active leaf count".into());
        }
        st.leaf_count_override = j.leaf_count_override;
        for a_hex in &j.anchors {
            let bytes = hex::decode(a_hex).map_err(|e| format!("bad anchor hex: {e}"))?;
            let a: [u8; NODE_BYTES] = bytes.try_into().map_err(|_| "bad anchor len")?;
            if st.anchors.insert(a) {
                st.anchor_list.push(a);
            }
        }
        // Verify latest root matches computed tree root!
        if !st
            .frozen
            .iter()
            .chain(std::iter::once(&st.tree))
            .all(|t| st.anchors.contains(&t.root()))
        {
            return Err("ledger JSON anchors do not contain current Depth-24 Merkle root".into());
        }
        for sn_hex in &j.spent_sn {
            let bytes = hex::decode(sn_hex).map_err(|e| format!("bad sn hex: {e}"))?;
            let sn: [u8; SN_BYTES] = bytes.try_into().map_err(|_| "bad sn len")?;
            if st.spent_sn.insert(sn) {
                st.spent_sn_list.push(sn);
            }
        }
        Ok(st)
    }
}
