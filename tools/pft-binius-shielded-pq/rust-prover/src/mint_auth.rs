//! Outer R0 authorization. Only the public key and signature travel on the wire.
//! The account address is derived exactly as in crates/crypto_provider.
use fips204::{
    ml_dsa_65,
    traits::{SerDes, Signer, Verifier},
};
use sha2::{Digest, Sha384};
use sha3::Sha3_384;

const CONTEXT: &[u8] = b"postfiat-l1-v2/tx/v1";

#[derive(Clone, Debug)]
pub struct MintAuth {
    pub public_key: [u8; ml_dsa_65::PK_LEN],
    pub signature: [u8; ml_dsa_65::SIG_LEN],
}

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
    fn mint_requires_signature_over_exact_context() {
        let raw = include_bytes!("../../payments/tx_mint.bin");
        let tx = ShieldedTx::from_bytes(raw).unwrap();
        let json: LedgerJson =
            serde_json::from_slice(include_bytes!("../../payments/ledger_mint.json")).unwrap();
        let ledger = LedgerState::from_json(&json).unwrap();
        ledger.verify_tx(&tx).unwrap();
        let reject = |mutated: ShieldedTx| {
            assert_eq!(
                ledger.verify_tx(&mutated).unwrap_err(),
                "R0-Auth: invalid mint authorization"
            );
        };
        let mut wrong = tx.clone();
        wrong.v_mint += 1;
        reject(wrong);
        let mut swapped = tx.clone();
        std::mem::swap(&mut swapped.coin_out0, &mut swapped.coin_out1);
        reject(swapped);
        let mut replay = tx.clone();
        replay.env1[0] ^= 1;
        reject(replay);
        let mut unsigned = tx.clone();
        unsigned.mint_auth = None;
        reject(unsigned);
        let mut wrong_key = tx.clone();
        wrong_key.mint_auth.as_mut().unwrap().public_key[0] ^= 1;
        reject(wrong_key);
        assert!(ShieldedTx::from_bytes(&raw[..raw.len() - ml_dsa_65::SIG_LEN]).is_err());
    }
}
