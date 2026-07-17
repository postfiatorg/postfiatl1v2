//! FastPay-style consensusless owned-value settlement — M1 prototype.
//!
//! Implements the owned-value object model and the Byzantine-consistent-broadcast
//! fast path (validator lock+sign, client aggregates a 2f+1 quorum into a
//! self-authenticating transfer certificate) as a standalone primitive. This is
//! NOT yet wired into consensus (that is M2); it exists to (a) prove the
//! primitive end-to-end and (b) measure the per-order signature cost, which is
//! the M1 decision gate — does the consensusless lane stay fast under ML-DSA-65?
//!
//! Safety (FastPay Lemma A.1): each honest validator signs at most one order per
//! `(ObjectId, Version)`; two 2f+1 quorums intersect in at least one honest
//! validator, so two certificates on the same input version must certify the
//! same order. The freshness/lock enforcement is M2 at the node; this crate
//! models the cryptographic certificate itself.

#![allow(clippy::type_complexity)]

use postfiat_crypto_provider as crypto;

mod state;
pub use state::{ApplyError, ApplyOutcome, OwnedObject, OwnedObjectStore};

pub mod cancellation_model;

/// Domain-separated signing context for owned-value transfer orders and votes.
/// Owner and validators sign under the same context (a vote binds the validator
/// to the exact order bytes the owner authorized).
pub const OWNED_TRANSFER_CONTEXT: &[u8] = b"postfiat-l1-v2/owned-transfer/v1";

/// Reference to a consumed input object at a specific version.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectRef {
    pub id: [u8; 32],
    pub version: u64,
}

/// Specification of a created output object (owner + value + asset).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedObjectSpec {
    pub owner_pubkey: Vec<u8>,
    pub value: u64,
    pub asset: String,
}

/// A transfer order: consume inputs at their current versions, create outputs,
/// burn a fee. Authorized by the input owner's signature.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OwnedTransferOrder {
    pub inputs: Vec<ObjectRef>,
    pub outputs: Vec<OwnedObjectSpec>,
    pub fee: u64,
    pub nonce: u64,
}

impl OwnedTransferOrder {
    /// Canonical, length-prefixed, domain-separated bytes covered by both the
    /// owner authorization and every validator vote.
    pub fn signing_bytes(&self) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(b"postfiat.owned-transfer.v1");
        out.push(0);
        out.extend((self.inputs.len() as u64).to_le_bytes());
        for r in &self.inputs {
            out.extend_from_slice(&r.id);
            out.extend(&r.version.to_le_bytes());
        }
        out.extend((self.outputs.len() as u64).to_le_bytes());
        for o in &self.outputs {
            out.extend((o.owner_pubkey.len() as u64).to_le_bytes());
            out.extend(&o.owner_pubkey);
            out.extend(&o.value.to_le_bytes());
            out.extend((o.asset.len() as u64).to_le_bytes());
            out.extend(o.asset.as_bytes());
        }
        out.extend(&self.fee.to_le_bytes());
        out.extend(&self.nonce.to_le_bytes());
        out
    }
}

/// A validator's signed vote on an order (the "lock + sign" step).
#[derive(Clone, Debug)]
pub struct ValidatorVote {
    pub validator_id: u64,
    pub signature: Vec<u8>,
}

/// The transfer certificate: the order + owner authorization + a 2f+1 quorum of
/// validator votes. A self-authenticating finality proof — anyone can verify it
/// without trusting the aggregator.
#[derive(Clone, Debug)]
pub struct OwnedTransferCertificate {
    pub order: OwnedTransferOrder,
    pub owner_pubkey: Vec<u8>,
    pub owner_signature: Vec<u8>,
    pub votes: Vec<ValidatorVote>,
}

impl OwnedTransferCertificate {
    /// On-the-wire byte cost of the certificate (owner pubkey + owner sig + the
    /// quorum of validator sigs). This is the FastPay message size — relevant
    /// because ML-DSA sigs are ~3.3 KB each.
    pub fn certificate_bytes(&self) -> usize {
        self.owner_pubkey.len()
            + self.owner_signature.len()
            + self
                .votes
                .iter()
                .map(|v| 8 + v.signature.len())
                .sum::<usize>()
    }
}

/// Owner authorizes the order (input-spend authorization).
pub fn owner_sign(
    owner_sk: &[u8],
    order: &OwnedTransferOrder,
) -> Result<Vec<u8>, crypto::CryptoError> {
    crypto::ml_dsa_65_sign_with_context(owner_sk, &order.signing_bytes(), OWNED_TRANSFER_CONTEXT)
}

/// Verify the owner's authorization.
pub fn owner_verify(owner_pk: &[u8], order: &OwnedTransferOrder, sig: &[u8]) -> bool {
    crypto::ml_dsa_65_verify_with_context(
        owner_pk,
        &order.signing_bytes(),
        sig,
        OWNED_TRANSFER_CONTEXT,
    )
}

/// A validator locks + signs the order. (The freshness/lock check that precedes
/// this — "input version is current and unspent, and I have not already signed a
/// conflicting order for it" — is enforced in M2 at the node; here we model the
/// cryptographic sign.)
pub fn validator_sign(
    validator_sk: &[u8],
    validator_id: u64,
    order: &OwnedTransferOrder,
) -> Result<ValidatorVote, crypto::CryptoError> {
    let signature = crypto::ml_dsa_65_sign_with_context(
        validator_sk,
        &order.signing_bytes(),
        OWNED_TRANSFER_CONTEXT,
    )?;
    Ok(ValidatorVote {
        validator_id,
        signature,
    })
}

/// Verify a single validator vote.
pub fn validator_verify(
    validator_pk: &[u8],
    order: &OwnedTransferOrder,
    vote: &ValidatorVote,
) -> bool {
    crypto::ml_dsa_65_verify_with_context(
        validator_pk,
        &order.signing_bytes(),
        &vote.signature,
        OWNED_TRANSFER_CONTEXT,
    )
}

/// Client aggregates a quorum of validator votes plus owner auth into the final
/// certificate. Aggregation is pure collection — no secret material, anyone can
/// do it, exactly as in FastPay.
pub fn aggregate_certificate(
    order: OwnedTransferOrder,
    owner_pubkey: Vec<u8>,
    owner_signature: Vec<u8>,
    votes: Vec<ValidatorVote>,
) -> OwnedTransferCertificate {
    OwnedTransferCertificate {
        order,
        owner_pubkey,
        owner_signature,
        votes,
    }
}

/// Outcome of verifying a complete certificate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CertificateVerdict {
    /// Certificate is well-formed; `votes` validator signatures verified.
    Valid {
        votes: usize,
    },
    OwnerAuthFailed,
    UnknownValidator(u64),
    DuplicateValidator(u64),
    InvalidVote(u64),
}

/// Verify a complete certificate: owner authorization + every validator vote
/// valid against the provided public-key set. The caller checks the returned
/// vote count is >= its 2f+1 threshold.
pub fn verify_certificate(
    cert: &OwnedTransferCertificate,
    validator_pks: &[(u64, Vec<u8>)],
) -> CertificateVerdict {
    if !owner_verify(&cert.owner_pubkey, &cert.order, &cert.owner_signature) {
        return CertificateVerdict::OwnerAuthFailed;
    }
    let mut valid = 0usize;
    let mut seen_validators = std::collections::BTreeSet::new();
    for vote in &cert.votes {
        if !seen_validators.insert(vote.validator_id) {
            return CertificateVerdict::DuplicateValidator(vote.validator_id);
        }
        let Some((_, pk)) = validator_pks
            .iter()
            .find(|(id, _)| *id == vote.validator_id)
        else {
            return CertificateVerdict::UnknownValidator(vote.validator_id);
        };
        if !validator_verify(pk, &cert.order, vote) {
            return CertificateVerdict::InvalidVote(vote.validator_id);
        }
        valid += 1;
    }
    CertificateVerdict::Valid { votes: valid }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keygen() -> (Vec<u8>, Vec<u8>) {
        let kp = crypto::ml_dsa_65_keygen().expect("keygen");
        (kp.public_key, kp.private_key.to_vec())
    }

    #[test]
    fn fast_path_signs_aggregates_and_verifies() {
        let (owner_pk, owner_sk) = keygen();
        let vs: Vec<(u64, Vec<u8>, Vec<u8>)> = (0..3)
            .map(|i| {
                let (pk, sk) = keygen();
                (i, pk, sk)
            })
            .collect();
        let pks: Vec<(u64, Vec<u8>)> = vs.iter().map(|(i, pk, _)| (*i, pk.clone())).collect();

        let order = OwnedTransferOrder {
            inputs: vec![ObjectRef {
                id: [1u8; 32],
                version: 1,
            }],
            outputs: vec![OwnedObjectSpec {
                owner_pubkey: owner_pk.clone(),
                value: 100,
                asset: "PFT".into(),
            }],
            fee: 1,
            nonce: 7,
        };

        let owner_sig = owner_sign(&owner_sk, &order).expect("owner sign");
        // Each validator independently signs the SAME order bytes.
        let votes: Vec<ValidatorVote> = vs
            .iter()
            .map(|(id, _, sk)| validator_sign(sk, *id, &order).expect("validator sign"))
            .collect();
        let cert = aggregate_certificate(order.clone(), owner_pk, owner_sig, votes);
        assert_eq!(
            verify_certificate(&cert, &pks),
            CertificateVerdict::Valid { votes: 3 }
        );
    }

    #[test]
    fn rejects_tampered_order_and_unauthorized_owner() {
        let (owner_pk, owner_sk) = keygen();
        let (other_pk, _other_sk) = keygen();
        let vs: Vec<(u64, Vec<u8>, Vec<u8>)> = (0..3)
            .map(|i| {
                let (pk, sk) = keygen();
                (i, pk, sk)
            })
            .collect();
        let pks: Vec<(u64, Vec<u8>)> = vs.iter().map(|(i, pk, _)| (*i, pk.clone())).collect();

        let mut order = OwnedTransferOrder {
            inputs: vec![ObjectRef {
                id: [1u8; 32],
                version: 1,
            }],
            outputs: vec![OwnedObjectSpec {
                owner_pubkey: owner_pk.clone(),
                value: 100,
                asset: "PFT".into(),
            }],
            fee: 1,
            nonce: 7,
        };
        let owner_sig = owner_sign(&owner_sk, &order).expect("owner sign");
        let votes: Vec<ValidatorVote> = vs
            .iter()
            .map(|(id, _, sk)| validator_sign(sk, *id, &order).expect("validator sign"))
            .collect();

        // Tampered order (different fee) -> owner auth must fail.
        order.fee = 999;
        let cert = aggregate_certificate(order, owner_pk.clone(), owner_sig, votes);
        assert_eq!(
            verify_certificate(&cert, &pks),
            CertificateVerdict::OwnerAuthFailed
        );

        // Wrong owner key -> owner auth fails.
        let mut order2 = cert.order.clone();
        order2.fee = 1;
        let owner_sig2 = owner_sign(&owner_sk, &order2).expect("owner sign");
        let votes2: Vec<ValidatorVote> = vs
            .iter()
            .map(|(id, _, sk)| validator_sign(sk, *id, &order2).expect("validator sign"))
            .collect();
        let cert2 = aggregate_certificate(order2, other_pk, owner_sig2, votes2);
        assert_eq!(
            verify_certificate(&cert2, &pks),
            CertificateVerdict::OwnerAuthFailed
        );
    }

    #[test]
    fn duplicate_validator_vote_never_inflates_prototype_certificate_count() {
        let (owner_public_key, owner_secret_key) = keygen();
        let (validator_public_key, validator_secret_key) = keygen();
        let order = OwnedTransferOrder {
            inputs: vec![ObjectRef {
                id: [1; 32],
                version: 1,
            }],
            outputs: vec![OwnedObjectSpec {
                owner_pubkey: owner_public_key.clone(),
                value: 99,
                asset: "PFT".to_string(),
            }],
            fee: 1,
            nonce: 8,
        };
        let owner_signature = owner_sign(&owner_secret_key, &order).expect("owner sign");
        let vote = validator_sign(&validator_secret_key, 7, &order).expect("validator sign");
        let certificate = aggregate_certificate(
            order,
            owner_public_key,
            owner_signature,
            vec![vote.clone(), vote],
        );

        assert_eq!(
            verify_certificate(&certificate, &[(7, validator_public_key)]),
            CertificateVerdict::DuplicateValidator(7)
        );
    }
}
