//! M2 state machine: the on-chain owned-value execution logic.
//!
//! An in-memory owned-object store + `apply_certificate` that enforces
//! single-consumption (version monotonicity) and single-asset value
//! conservation. This is the execution-layer logic that gets ported into
//! `LedgerState` + the execution crate as M2 proceeds; it lives here first so
//! the FastPay state machine is exercised by tests independent of consensus.
use postfiat_crypto_provider as crypto;

use crate::{verify_certificate, CertificateVerdict, OwnedTransferCertificate};

/// A live owned-value object in ledger state. Single-consumption: spending it
/// retires this version and mints fresh output versions.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedObject {
    pub id: [u8; 32],
    pub version: u64,
    pub owner_pubkey: Vec<u8>,
    pub value: u64,
    pub asset: String,
}

/// In-memory owned-object store (the ledger component; M2 wires this into
/// `LedgerState`).
#[derive(Default, Clone, Debug)]
pub struct OwnedObjectStore {
    objects: std::collections::HashMap<[u8; 32], OwnedObject>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub consumed: usize,
    pub created: Vec<OwnedObject>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApplyError {
    CertificateInvalid,
    EmptyInputs,
    EmptyOutputs,
    DuplicateInput,
    DuplicateObject,
    InvalidObject,
    UnknownInput,
    VersionMismatch,
    NotOwner,
    Overflow,
    MixedAssets,
    NotConserved,
}

impl OwnedObjectStore {
    pub fn get(&self, id: &[u8; 32]) -> Option<&OwnedObject> {
        self.objects.get(id)
    }

    /// Seed a fresh object in this research store's genesis fixture. Production
    /// account-to-owned value enters through `postfiat_execution::wrap_to_owned`
    /// or the signed consensus deposit path; this helper only models an already
    /// supply-accounted genesis object and therefore rejects malformed or
    /// duplicate fixture state.
    pub fn mint(
        &mut self,
        id: [u8; 32],
        owner_pubkey: Vec<u8>,
        value: u64,
        asset: String,
    ) -> Result<OwnedObject, ApplyError> {
        if id == [0; 32] || owner_pubkey.is_empty() || value == 0 || asset.is_empty() {
            return Err(ApplyError::InvalidObject);
        }
        if self.objects.contains_key(&id) {
            return Err(ApplyError::DuplicateObject);
        }
        let obj = OwnedObject {
            id,
            version: 1,
            owner_pubkey,
            value,
            asset,
        };
        self.objects.insert(id, obj.clone());
        Ok(obj)
    }

    /// Apply a certified owned-transfer: verify the certificate, consume inputs
    /// at their current versions, create outputs at content-addressed fresh ids,
    /// and enforce single-asset value conservation. Single-consumption is
    /// enforced by the version check — a consumed input's version no longer
    /// matches (it is retired), so a replay is rejected.
    pub fn apply_certificate(
        &mut self,
        cert: &OwnedTransferCertificate,
        validator_pks: &[(u64, Vec<u8>)],
    ) -> Result<ApplyOutcome, ApplyError> {
        if !matches!(
            verify_certificate(cert, validator_pks),
            CertificateVerdict::Valid { .. }
        ) {
            return Err(ApplyError::CertificateInvalid);
        }
        if cert.order.inputs.is_empty() {
            return Err(ApplyError::EmptyInputs);
        }
        if cert.order.outputs.is_empty() {
            return Err(ApplyError::EmptyOutputs);
        }

        // Consume inputs: must exist, at the claimed version, owned by the cert owner.
        let mut input_value = 0u64;
        let mut asset: Option<String> = None;
        let mut input_ids = std::collections::BTreeSet::new();
        for inp in &cert.order.inputs {
            if !input_ids.insert(inp.id) {
                return Err(ApplyError::DuplicateInput);
            }
            let obj = self.objects.get(&inp.id).ok_or(ApplyError::UnknownInput)?;
            if obj.version != inp.version {
                return Err(ApplyError::VersionMismatch);
            }
            if obj.owner_pubkey != cert.owner_pubkey {
                return Err(ApplyError::NotOwner);
            }
            input_value = input_value
                .checked_add(obj.value)
                .ok_or(ApplyError::Overflow)?;
            match &asset {
                None => asset = Some(obj.asset.clone()),
                Some(a) if a == &obj.asset => {}
                _ => return Err(ApplyError::MixedAssets),
            }
        }
        let asset = asset.unwrap_or_default();

        // Conservation: outputs must share the input asset, and outputs + fee == inputs.
        let mut output_value = 0u64;
        let mut output_ids = std::collections::BTreeSet::new();
        let mut created = Vec::with_capacity(cert.order.outputs.len());
        for (index, output) in cert.order.outputs.iter().enumerate() {
            if output.asset != asset {
                return Err(ApplyError::MixedAssets);
            }
            if output.owner_pubkey.is_empty() || output.value == 0 || output.asset.is_empty() {
                return Err(ApplyError::InvalidObject);
            }
            output_value = output_value
                .checked_add(output.value)
                .ok_or(ApplyError::Overflow)?;
            let id = output_id(&cert.order.signing_bytes(), index);
            if !output_ids.insert(id) || self.objects.contains_key(&id) {
                return Err(ApplyError::DuplicateObject);
            }
            created.push(OwnedObject {
                id,
                version: 1,
                owner_pubkey: output.owner_pubkey.clone(),
                value: output.value,
                asset: output.asset.clone(),
            });
        }
        if output_value.checked_add(cert.order.fee) != Some(input_value) {
            return Err(ApplyError::NotConserved);
        }

        // Retire consumed inputs.
        for inp in &cert.order.inputs {
            self.objects.remove(&inp.id);
        }
        // Publish prevalidated outputs only after every fallible check.
        for object in &created {
            self.objects.insert(object.id, object.clone());
        }
        Ok(ApplyOutcome {
            consumed: cert.order.inputs.len(),
            created,
        })
    }
}

/// Deterministic, content-addressed output id: SHA3-384(order bytes || index)[..32].
fn output_id(order_signing_bytes: &[u8], index: usize) -> [u8; 32] {
    let mut material = Vec::from(order_signing_bytes);
    material.extend(&(index as u64).to_le_bytes());
    let h = crypto::hash_bytes("postfiat.owned-output.v1", &material);
    let mut out = [0u8; 32];
    out.copy_from_slice(&h[..32]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        aggregate_certificate, owner_sign, validator_sign, ObjectRef, OwnedObjectSpec,
        OwnedTransferOrder,
    };

    fn keygen() -> (Vec<u8>, Vec<u8>) {
        let kp = crypto::ml_dsa_65_keygen().expect("keygen");
        (kp.public_key, kp.private_key.to_vec())
    }

    fn certify(
        owner_sk: &[u8],
        owner_pk: Vec<u8>,
        order: OwnedTransferOrder,
        vs: &[(u64, Vec<u8>, Vec<u8>)],
    ) -> OwnedTransferCertificate {
        let sig = owner_sign(owner_sk, &order).expect("owner sign");
        let votes: Vec<_> = vs
            .iter()
            .map(|(id, _, sk)| validator_sign(sk, *id, &order).expect("vsign"))
            .collect();
        aggregate_certificate(order, owner_pk, sig, votes)
    }

    fn validators() -> (Vec<(u64, Vec<u8>, Vec<u8>)>, Vec<(u64, Vec<u8>)>) {
        let vs: Vec<(u64, Vec<u8>, Vec<u8>)> = (0..3)
            .map(|i| {
                let (p, s) = keygen();
                (i, p, s)
            })
            .collect();
        let pks = vs.iter().map(|(i, p, _)| (*i, p.clone())).collect();
        (vs, pks)
    }

    #[test]
    fn applies_valid_transfer_and_enforces_conservation() {
        let (opk, osk) = keygen();
        let (rpk, _rsk) = keygen();
        let (vs, pks) = validators();
        let mut store = OwnedObjectStore::default();
        let in_id = [9u8; 32];
        store
            .mint(in_id, opk.clone(), 100, "PFT".into())
            .expect("genesis fixture");

        // 100 -> 90 to recipient + 9 change + 1 fee (conserved: 90+9+1 == 100).
        let order = OwnedTransferOrder {
            inputs: vec![ObjectRef {
                id: in_id,
                version: 1,
            }],
            outputs: vec![
                OwnedObjectSpec {
                    owner_pubkey: rpk.clone(),
                    value: 90,
                    asset: "PFT".into(),
                },
                OwnedObjectSpec {
                    owner_pubkey: opk.clone(),
                    value: 9,
                    asset: "PFT".into(),
                },
            ],
            fee: 1,
            nonce: 1,
        };
        let cert = certify(&osk, opk.clone(), order, &vs);
        let out = store.apply_certificate(&cert, &pks).expect("apply");
        assert_eq!(out.consumed, 1);
        assert_eq!(out.created.len(), 2);
        assert_eq!(out.created.iter().map(|o| o.value).sum::<u64>(), 99);
        assert!(store.get(&in_id).is_none()); // input retired
    }

    #[test]
    fn rejects_double_spend_of_consumed_input() {
        let (opk, osk) = keygen();
        let (vs, pks) = validators();
        let mut store = OwnedObjectStore::default();
        let in_id = [9u8; 32];
        store
            .mint(in_id, opk.clone(), 100, "PFT".into())
            .expect("genesis fixture");
        let order = OwnedTransferOrder {
            inputs: vec![ObjectRef {
                id: in_id,
                version: 1,
            }],
            outputs: vec![OwnedObjectSpec {
                owner_pubkey: opk.clone(),
                value: 99,
                asset: "PFT".into(),
            }],
            fee: 1,
            nonce: 1,
        };
        let cert = certify(&osk, opk.clone(), order, &vs);
        store
            .apply_certificate(&cert, &pks)
            .expect("first spend ok");
        // Replay: input is retired -> rejected (single-consumption).
        let err = store.apply_certificate(&cert, &pks).unwrap_err();
        assert_eq!(err, ApplyError::UnknownInput);
    }

    #[test]
    fn rejects_non_conserving_transfer() {
        let (opk, osk) = keygen();
        let (vs, pks) = validators();
        let mut store = OwnedObjectStore::default();
        store
            .mint([9u8; 32], opk.clone(), 100, "PFT".into())
            .expect("genesis fixture");
        // 100 -> 200 + fee 1 (not conserved).
        let order = OwnedTransferOrder {
            inputs: vec![ObjectRef {
                id: [9u8; 32],
                version: 1,
            }],
            outputs: vec![OwnedObjectSpec {
                owner_pubkey: opk.clone(),
                value: 200,
                asset: "PFT".into(),
            }],
            fee: 1,
            nonce: 1,
        };
        let cert = certify(&osk, opk.clone(), order, &vs);
        let err = store.apply_certificate(&cert, &pks).unwrap_err();
        assert_eq!(err, ApplyError::NotConserved);
    }

    #[test]
    fn rejects_version_mismatch_and_wrong_owner() {
        let (opk, osk) = keygen();
        let (other_pk, other_sk) = keygen();
        let (vs, pks) = validators();
        let mut store = OwnedObjectStore::default();
        store
            .mint([9u8; 32], opk.clone(), 100, "PFT".into())
            .expect("genesis fixture");
        // Wrong claimed version.
        let order = OwnedTransferOrder {
            inputs: vec![ObjectRef {
                id: [9u8; 32],
                version: 99,
            }],
            outputs: vec![OwnedObjectSpec {
                owner_pubkey: opk.clone(),
                value: 100,
                asset: "PFT".into(),
            }],
            fee: 0,
            nonce: 1,
        };
        let cert = certify(&osk, opk.clone(), order, &vs);
        assert_eq!(
            store.apply_certificate(&cert, &pks).unwrap_err(),
            ApplyError::VersionMismatch
        );
        // Wrong owner: input owned by `opk`, cert authorized by `other`.
        let order2 = OwnedTransferOrder {
            inputs: vec![ObjectRef {
                id: [9u8; 32],
                version: 1,
            }],
            outputs: vec![OwnedObjectSpec {
                owner_pubkey: other_pk.clone(),
                value: 100,
                asset: "PFT".into(),
            }],
            fee: 0,
            nonce: 2,
        };
        let cert2 = certify(&other_sk, other_pk, order2, &vs);
        assert_eq!(
            store.apply_certificate(&cert2, &pks).unwrap_err(),
            ApplyError::NotOwner
        );
    }

    #[test]
    fn genesis_fixture_rejects_zero_and_duplicate_objects_without_mutation() {
        let (owner, _) = keygen();
        let mut store = OwnedObjectStore::default();
        let id = [7u8; 32];
        store
            .mint(id, owner.clone(), 10, "PFT".into())
            .expect("initial genesis fixture");
        let before = store.clone();
        assert_eq!(
            store
                .mint(id, owner.clone(), 11, "PFT".into())
                .expect_err("duplicate genesis id must fail"),
            ApplyError::DuplicateObject
        );
        assert_eq!(store.objects, before.objects);
        assert_eq!(
            store
                .mint([8u8; 32], owner, 0, "PFT".into())
                .expect_err("zero-value genesis object must fail"),
            ApplyError::InvalidObject
        );
        assert_eq!(store.objects, before.objects);
    }

    #[test]
    fn duplicate_input_cannot_inflate_prototype_owned_value() {
        let (owner, owner_secret) = keygen();
        let (validators, validator_public_keys) = validators();
        let mut store = OwnedObjectStore::default();
        let id = [9u8; 32];
        store
            .mint(id, owner.clone(), 100, "PFT".into())
            .expect("genesis fixture");
        let order = OwnedTransferOrder {
            inputs: vec![ObjectRef { id, version: 1 }, ObjectRef { id, version: 1 }],
            outputs: vec![OwnedObjectSpec {
                owner_pubkey: owner.clone(),
                value: 200,
                asset: "PFT".into(),
            }],
            fee: 0,
            nonce: 1,
        };
        let certificate = certify(&owner_secret, owner, order, &validators);
        let before = store.clone();
        assert_eq!(
            store
                .apply_certificate(&certificate, &validator_public_keys)
                .expect_err("duplicate input must not be counted twice"),
            ApplyError::DuplicateInput
        );
        assert_eq!(store.objects, before.objects);
    }
}
