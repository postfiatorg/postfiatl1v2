fn certificate_at_bound(
    unwrap: bool,
) -> (postfiat_types::FastPayCertificateV1, Vec<(String, String)>) {
    use postfiat_crypto_provider::{
        bytes_to_hex, ml_dsa_65_keygen_from_seed, ml_dsa_65_sign_with_context,
    };
    use postfiat_types::*;

    let owner = ml_dsa_65_keygen_from_seed(&[200; 32]);
    let owner_pubkey_hex = bytes_to_hex(&owner.public_key);
    let validators = (0..MAX_FASTPAY_RECOVERY_VALIDATORS)
        .map(|i| {
            (
                format!("validator-{i:03}"),
                ml_dsa_65_keygen_from_seed(&[i as u8; 32]),
            )
        })
        .collect::<Vec<_>>();
    let public_keys = validators
        .iter()
        .map(|(id, key)| (id.clone(), bytes_to_hex(&key.public_key)))
        .collect::<Vec<_>>();
    let committee = FastPayRecoveryCommitteeV1::from_public_keys(
        "fastpay-bound-test".into(),
        "11".repeat(48),
        3,
        7,
        90,
        110,
        public_keys.clone(),
    )
    .unwrap();
    let recovery = FastPayOrderRecoveryV1 {
        schema: FASTPAY_ORDER_RECOVERY_SCHEMA_V1.into(),
        committee_epoch: 7,
        lock_id: String::new(),
        valid_from_height: 100,
        expires_at_height: 110,
        recovery_closes_at_height: 120,
    };
    let inputs = vec![OwnedObjectRef {
        id: "bound-input".into(),
        version: 1,
    }];
    let sign = |key: &[u8], bytes: &[u8], context: &[u8]| {
        bytes_to_hex(&ml_dsa_65_sign_with_context(key, bytes, context).unwrap())
    };
    let certificate = if unwrap {
        let mut order = OwnedUnwrapOrderV3 {
            domain: committee.certificate_domain(),
            recovery,
            inputs,
            to_address: "recipient".into(),
            amount: 99,
            asset: "PFT".into(),
            fee: 1,
            nonce: 1,
            memos: Vec::new(),
        };
        order.recovery.lock_id = fastpay_unwrap_lock_id_v1(&order);
        let bytes = owned_unwrap_v3_signing_bytes(&order);
        FastPayCertificateV1::Unwrap(OwnedUnwrapCertificateV3 {
            order,
            owner_pubkey_hex,
            owner_signature_hex: sign(&owner.private_key, &bytes, OWNED_UNWRAP_CONTEXT_V3),
            votes: validators
                .iter()
                .map(|(id, key)| OwnedUnwrapVote {
                    validator_id: id.clone(),
                    signature_hex: sign(&key.private_key, &bytes, OWNED_UNWRAP_CONTEXT_V3),
                })
                .collect(),
        })
    } else {
        let mut order = OwnedTransferOrderV3 {
            domain: committee.certificate_domain(),
            recovery,
            inputs,
            outputs: vec![OwnedOutputSpec {
                owner_pubkey_hex: owner_pubkey_hex.clone(),
                value: 99,
                asset: "PFT".into(),
            }],
            fee: 1,
            nonce: 1,
            memos: Vec::new(),
        };
        order.recovery.lock_id = fastpay_transfer_lock_id_v1(&order);
        let bytes = owned_transfer_v3_signing_bytes(&order);
        FastPayCertificateV1::Transfer(OwnedTransferCertificateV3 {
            order,
            owner_pubkey_hex,
            owner_signature_hex: sign(&owner.private_key, &bytes, OWNED_TRANSFER_CONTEXT_V3),
            votes: validators
                .iter()
                .map(|(id, key)| OwnedTransferVote {
                    validator_id: id.clone(),
                    signature_hex: sign(&key.private_key, &bytes, OWNED_TRANSFER_CONTEXT_V3),
                })
                .collect(),
        })
    };
    (certificate, public_keys)
}

fn assert_certificate_bound(unwrap: bool) {
    use postfiat_types::*;
    let (certificate, public_keys) = certificate_at_bound(unwrap);
    let policy = policy();
    let context = FastPayRecoveryVerificationContext {
        validator_public_keys: &public_keys,
        expected_domain: certificate.domain(),
        committee_epoch: 7,
        policy: &policy,
        quorum: FastPayRecoveryCommitteeV1::expected_quorum(public_keys.len()).unwrap(),
    };
    let apply = |ledger: &mut LedgerState, certificate: &FastPayCertificateV1| match certificate {
        FastPayCertificateV1::Transfer(value) => {
            apply_owned_transfer_certificate_v3(ledger, value, context, 105).map(|_| ())
        }
        FastPayCertificateV1::Unwrap(value) => {
            apply_owned_unwrap_certificate_v3(ledger, value, context, 105).map(|_| ())
        }
    };
    let owner = match &certificate {
        FastPayCertificateV1::Transfer(value) => &value.owner_pubkey_hex,
        FastPayCertificateV1::Unwrap(value) => &value.owner_pubkey_hex,
    };
    let mut ledger = LedgerState::empty();
    ledger.owned_objects.push(OwnedObject {
        id: "bound-input".into(),
        version: 1,
        owner_pubkey_hex: owner.clone(),
        value: 100,
        asset: "PFT".into(),
    });
    let before = ledger.clone();
    apply(&mut ledger, &certificate).expect("128 verified voters apply");
    ledger.fastpay_version_fences[0]
        .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
        .expect("accepted fence is V2-encodable");
    let mut revealed = before.clone();
    record_fastpay_recovery_reveal_v1(&mut revealed, certificate.clone(), context, 111)
        .expect("128 verified voters reveal")
        .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
        .expect("accepted reveal is V2-encodable");

    for mutation in [
        "129 votes",
        "unknown voter",
        "empty",
        "duplicate",
        "reordered",
    ] {
        let mut changed = certificate.clone();
        // Both operations have the same vote fields but distinct wire types.
        macro_rules! mutate_votes {
            ($votes:expr) => {{
                let votes = &mut $votes;
                match mutation {
                    "129 votes" => {
                        let mut extra = votes[0].clone();
                        extra.validator_id = "unknown-extra".into();
                        votes.push(extra);
                    }
                    "unknown voter" => votes[0].validator_id = "unknown".into(),
                    "empty" => votes.clear(),
                    "duplicate" => votes[1] = votes[0].clone(),
                    _ => votes.reverse(),
                }
            }};
        }
        match &mut changed {
            FastPayCertificateV1::Transfer(value) => mutate_votes!(value.votes),
            FastPayCertificateV1::Unwrap(value) => mutate_votes!(value.votes),
        }
        if mutation == "129 votes" {
            assert!(changed.validate_commitment_shape().is_err());
            assert!(changed.canonical_bytes().is_err());
            let mut fence = ledger.fastpay_version_fences[0].clone();
            fence.certificate = Some(changed.clone());
            assert!(fence
                .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
                .is_err());
            fence
                .state_commitment_bytes()
                .expect("legacy V1 stays encodable");
        } else if mutation == "unknown voter" {
            changed
                .validate_commitment_shape()
                .expect("membership is checked at acceptance");
        }
        let mut applied = before.clone();
        let mut revealed = before.clone();
        if mutation == "reordered" {
            apply(&mut applied, &changed).expect("arrival order remains accepted");
            assert_eq!(
                ledger.fastpay_version_fences[0]
                    .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
                    .unwrap(),
                applied.fastpay_version_fences[0]
                    .state_commitment_bytes_for_version(FastPayRecoveryCommitmentVersion::V2)
                    .unwrap(),
            );
        } else {
            assert_eq!(
                apply(&mut applied, &changed),
                Err(OwnedTransferError::InvalidRecovery),
                "{mutation}"
            );
            assert_eq!(applied, before, "apply: {mutation}");
            assert_eq!(
                record_fastpay_recovery_reveal_v1(&mut revealed, changed, context, 111),
                Err(OwnedTransferError::InvalidRecovery),
                "{mutation}",
            );
            assert_eq!(revealed, before, "reveal: {mutation}");
        }
    }
}

#[test]
fn transfer_certificate_bounds_and_unknown_voters_reject_before_mutation() {
    assert_certificate_bound(false);
}

#[test]
fn unwrap_certificate_bounds_and_unknown_voters_reject_before_mutation() {
    assert_certificate_bound(true);
}

#[test]
fn v2_installation_rejects_historical_overbound_certificates_atomically() {
    use postfiat_types::*;
    let validators = recovery_validator_keys()
        .iter()
        .map(|(id, key)| {
            (
                id.clone(),
                postfiat_crypto_provider::bytes_to_hex(&key.public_key),
            )
        })
        .collect::<Vec<_>>();
    let mut old = FastPayRecoveryCommitteeV1::from_public_keys(
        domain().chain_id,
        domain().genesis_hash,
        3,
        7,
        90,
        110,
        validators.clone(),
    )
    .unwrap();
    old.schema = FASTPAY_RECOVERY_COMMITTEE_SCHEMA_V1.into();
    old.registry_root = old.computed_root().unwrap();
    let (mut certificate, _) =
        signed_certificate_for_domain("historical-overbound", old.certificate_domain());
    let next = FastPayRecoveryCommitteeV1::from_public_keys(
        old.chain_id.clone(),
        old.genesis_hash.clone(),
        3,
        8,
        111,
        140,
        validators,
    )
    .unwrap();
    for i in certificate.votes.len()..=MAX_FASTPAY_RECOVERY_VALIDATORS {
        let mut vote = certificate.votes[0].clone();
        vote.validator_id = format!("historical-unknown-{i:03}");
        certificate.votes.push(vote);
    }
    // Reproduce previously accepted retained state without passing new admission.
    let fence =
        transfer_confirmed_fence(&certificate, 100, FastPayFenceOriginV1::Consensusless).unwrap();
    let reveal = FastPayRecoveryRevealV1 {
        schema: FASTPAY_RECOVERY_REVEAL_SCHEMA_V1.into(),
        lock_id: certificate.order.recovery.lock_id.clone(),
        order_digest: fastpay_transfer_order_digest_v3(&certificate.order),
        certificate_digest: fastpay_transfer_certificate_digest_v3(&certificate).unwrap(),
        revealed_at_height: 111,
        certificate: FastPayCertificateV1::Transfer(certificate),
    };
    for retained_reveal in [false, true] {
        let mut ledger = LedgerState::empty();
        ledger.fastpay_recovery_policy = Some(policy());
        ledger.fastpay_recovery_committees.push(old.clone());
        if retained_reveal {
            ledger.fastpay_recovery_reveals.push(reveal.clone());
        } else {
            ledger.fastpay_version_fences.push(fence.clone());
        }
        let v1 = if retained_reveal {
            reveal.state_commitment_bytes()
        } else {
            fence.state_commitment_bytes()
        }
        .unwrap();
        let before = ledger.clone();
        let error = execute_fastpay_recovery_governance_update_v1(
            &mut ledger,
            &recovery_governance_update(policy(), next.clone()),
            100,
        )
        .expect_err("V2 cannot install over a 129-vote historical record");
        assert!(error.contains("validator bound"), "{error}");
        assert_eq!(ledger, before);
        let after = if retained_reveal {
            ledger.fastpay_recovery_reveals[0].state_commitment_bytes()
        } else {
            ledger.fastpay_version_fences[0].state_commitment_bytes()
        }
        .unwrap();
        assert_eq!(v1, after, "historical V1 bytes are never rewritten");
    }
}
