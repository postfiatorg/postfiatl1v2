//! Outer R0 deposit authorization and PFSH03 per-input external ML-DSA-65 spend authorization.
//! Only public keys and signatures travel on the wire; the deposit account address is derived
//! exactly as in crates/crypto_provider.
use crate::shake256::{
    owner_addr, pk_hash, spend_dsa_seed, spend_r_pk, ADDR_BYTES, OWNER_SECRET_BYTES, PK_HASH_BYTES,
};
use fips204::{
    ml_dsa_65,
    traits::{KeyGen, SerDes, Signer, Verifier},
};
use sha2::{Digest, Sha384};
use sha3::Sha3_384;

const CONTEXT: &[u8] = b"postfiat-l1-v2/tx/v1";

pub use crate::wire::{MintAuth, SpendAuth};

impl MintAuth {
    pub fn source(&self) -> String {
        source(&self.public_key)
    }

    pub fn sign(key: &ml_dsa_65::PrivateKey, tx_ctx: &[u8; 48]) -> Result<Self, String> {
        let public_key = key.get_public_key().into_bytes();
        // Deterministic FIPS-204 signing (rnd = 0^32).
        let signature = key
            .try_sign_with_seed(&[0; 32], &digest(&public_key, tx_ctx), CONTEXT)
            .map_err(str::to_owned)?;
        Ok(Self {
            public_key,
            signature,
        })
    }

    pub fn verify(&self, tx_ctx: &[u8; 48]) -> bool {
        ml_dsa_65::PublicKey::try_from_bytes(self.public_key)
            .is_ok_and(|pk| pk.verify(&digest(&self.public_key, tx_ctx), &self.signature, CONTEXT))
    }
}

impl SpendAuth {
    pub fn pk_hash(&self) -> [u8; PK_HASH_BYTES] {
        pk_hash(&self.public_key)
    }

    pub fn sign(
        slot: u8,
        key: &ml_dsa_65::PrivateKey,
        statement_ctx: &[u8; 48],
    ) -> Result<Self, String> {
        let public_key = key.get_public_key().into_bytes();
        let pk_h = pk_hash(&public_key);
        let msg = spend_auth_digest(slot, &pk_h, statement_ctx);
        let signature = key
            .try_sign_with_seed(&[0; 32], &msg, CONTEXT)
            .map_err(str::to_owned)?;
        Ok(Self {
            public_key,
            signature,
        })
    }

    pub fn verify(&self, slot: u8, statement_ctx: &[u8; 48]) -> bool {
        let pk_h = self.pk_hash();
        let msg = spend_auth_digest(slot, &pk_h, statement_ctx);
        ml_dsa_65::PublicKey::try_from_bytes(self.public_key)
            .is_ok_and(|pk| pk.verify(&msg, &self.signature, CONTEXT))
    }
}

/// Deterministically derive the domain-separated ML-DSA-65 spend keypair, pk_hash, and r_pk
/// from a 48-byte spend secret.
pub fn spend_keypair_from_secret(
    spend_secret: &[u8; OWNER_SECRET_BYTES],
) -> (
    ml_dsa_65::PublicKey,
    ml_dsa_65::PrivateKey,
    [u8; PK_HASH_BYTES],
    [u8; OWNER_SECRET_BYTES],
) {
    let seed = spend_dsa_seed(spend_secret);
    let (pk, sk) = ml_dsa_65::KG::keygen_from_seed(&seed);
    let pk_h = pk_hash(&pk.clone().into_bytes());
    let r_pk = spend_r_pk(spend_secret);
    (pk, sk, pk_h, r_pk)
}

/// Convenience helper computing the blinded note address `A = H_A(pk_hash, r_pk)` from `spend_secret`.
pub fn owner_addr_for_secret(spend_secret: &[u8; OWNER_SECRET_BYTES]) -> [u8; ADDR_BYTES] {
    let (_, _, pk_h, r_pk) = spend_keypair_from_secret(spend_secret);
    owner_addr(&pk_h, &r_pk)
}

pub fn spend_auth_digest(
    slot: u8,
    pk_h: &[u8; PK_HASH_BYTES],
    statement_ctx: &[u8; 48],
) -> [u8; 48] {
    Sha384::new()
        .chain_update(b"PFT-SH-SPEND-AUTH-V3")
        .chain_update([slot])
        .chain_update(pk_h)
        .chain_update(statement_ctx)
        .finalize()
        .into()
}

fn source(pk: &[u8; ml_dsa_65::PK_LEN]) -> String {
    let address = Sha3_384::new()
        .chain_update(b"postfiat.address.v1\0")
        .chain_update(pk)
        .finalize();
    format!("pf{}", hex::encode(&address[..20]))
}

fn digest(pk: &[u8; ml_dsa_65::PK_LEN], tx_ctx: &[u8; 48]) -> [u8; 48] {
    Sha384::new()
        .chain_update(b"PFT-SH-MINT-AUTH-V2")
        .chain_update(source(pk))
        .chain_update(tx_ctx)
        .finalize()
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::payment::{LedgerJson, LedgerState, ShieldedTx};

    #[test]
    fn mint_and_spend_require_signatures_over_exact_context() {
        let raw = include_bytes!("../../payments/tx_mint.bin");
        let tx = ShieldedTx::from_bytes(raw).unwrap();
        let json: LedgerJson =
            serde_json::from_slice(include_bytes!("../../payments/ledger_mint.json")).unwrap();
        let ledger = LedgerState::from_json(&json).unwrap();
        ledger.verify_tx(&tx).unwrap();
        let reject_mint = |mutated: ShieldedTx| {
            let err = ledger.verify_tx(&mutated).unwrap_err();
            assert!(
                err == "R0-Auth: invalid mint authorization"
                    || err == "R0-SpendAuth: invalid ML-DSA-65 spend authorization",
                "unexpected error: {err}"
            );
        };
        let mut wrong = tx.clone();
        wrong.v_mint += 1;
        reject_mint(wrong);
        let mut swapped = tx.clone();
        std::mem::swap(&mut swapped.coin_out0, &mut swapped.coin_out1);
        reject_mint(swapped);
        let mut replay = tx.clone();
        replay.env1[0] ^= 1;
        reject_mint(replay);
        let mut unsigned = tx.clone();
        unsigned.mint_auth = None;
        assert_eq!(
            ledger.verify_tx(&unsigned).unwrap_err(),
            "R0-Auth: invalid mint authorization"
        );
        let mut wrong_key = tx.clone();
        wrong_key.mint_auth.as_mut().unwrap().public_key[0] ^= 1;
        assert_eq!(
            ledger.verify_tx(&wrong_key).unwrap_err(),
            "R0-Auth: invalid mint authorization"
        );
        let mut bad_spend_sig0 = tx.clone();
        bad_spend_sig0.spend_auth0.signature[0] ^= 1;
        assert_eq!(
            ledger.verify_tx(&bad_spend_sig0).unwrap_err(),
            "R0-SpendAuth: invalid ML-DSA-65 spend authorization"
        );
        let mut bad_spend_sig1 = tx.clone();
        bad_spend_sig1.spend_auth1.signature[0] ^= 1;
        assert_eq!(
            ledger.verify_tx(&bad_spend_sig1).unwrap_err(),
            "R0-SpendAuth: invalid ML-DSA-65 spend authorization"
        );
        let mut swapped_spend = tx.clone();
        std::mem::swap(
            &mut swapped_spend.spend_auth0,
            &mut swapped_spend.spend_auth1,
        );
        reject_mint(swapped_spend);
        assert!(ShieldedTx::from_bytes(&raw[..raw.len() - ml_dsa_65::SIG_LEN]).is_err());
    }
}
