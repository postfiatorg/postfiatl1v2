use super::*;
use postfiat_types::*;

fn committed_ledger(ledger: &LedgerState) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    append_ledger_state(&mut bytes, ledger, true, true, false, false, true, false, false)?;
    Ok(bytes)
}

#[test]
fn v2_installation_binds_certificates_retained_under_v1() {
    let mut old = FastPayRecoveryCommitteeV1::from_public_keys(
        "commitment-transition".into(), "11".repeat(48), 1, 1, 10, 20,
        (0..4).map(|i| (format!("validator-{i}"), "aa".repeat(32))).collect(),
    ).unwrap();
    old.schema = FASTPAY_RECOVERY_COMMITTEE_SCHEMA_V1.into();
    old.registry_root = old.computed_root().unwrap();
    let input = OwnedObjectRef { id: "33".repeat(32), version: 1 };
    let mut order = OwnedTransferOrderV3 {
        domain: old.certificate_domain(),
        recovery: FastPayOrderRecoveryV1 {
            schema: FASTPAY_ORDER_RECOVERY_SCHEMA_V1.into(), committee_epoch: 1,
            lock_id: String::new(), valid_from_height: 10, expires_at_height: 20,
            recovery_closes_at_height: 30,
        },
        inputs: vec![input.clone()],
        outputs: vec![OwnedOutputSpec { owner_pubkey_hex: "44".repeat(32), value: 9, asset: "PFT".into() }],
        fee: 1, nonce: 1, memos: Vec::new(),
    };
    order.recovery.lock_id = fastpay_transfer_lock_id_v1(&order);
    let certificate = FastPayCertificateV1::Transfer(OwnedTransferCertificateV3 {
        order, owner_pubkey_hex: "44".repeat(32), owner_signature_hex: "55".repeat(32),
        votes: vec![OwnedTransferVote { validator_id: "validator-0".into(), signature_hex: "66".repeat(32) }],
    });
    let fence = FastPayVersionFenceV1 {
        schema: FASTPAY_VERSION_FENCE_SCHEMA_V1.into(), operation: FastPayOperationKindV1::Transfer,
        origin: FastPayFenceOriginV1::Consensusless, committee_epoch: 1,
        registry_root: old.registry_root.clone(), lock_id: certificate.recovery().lock_id.clone(),
        inputs: vec![input.clone()],
        decision: FastPayRecoveryDecisionV1::Confirmed {
            order_digest: "77".repeat(48), certificate_digest: "88".repeat(48),
        },
        certificate: Some(certificate.clone()), decided_at_height: 15,
        next_versions: vec![OwnedObjectRef { id: input.id, version: 2 }],
    };
    let reveal = FastPayRecoveryRevealV1 {
        schema: FASTPAY_RECOVERY_REVEAL_SCHEMA_V1.into(), lock_id: fence.lock_id.clone(),
        order_digest: "77".repeat(48), certificate_digest: "88".repeat(48),
        revealed_at_height: 21, certificate,
    };
    let policy = FastPayRecoveryPolicyV1 {
        schema: FASTPAY_RECOVERY_POLICY_SCHEMA_V1.into(), activation_height: 10,
        max_validity_blocks: 20, max_recovery_blocks: 20,
    };
    let mut legacy = LedgerState::empty();
    legacy.fastpay_recovery_policy = Some(policy);
    legacy.fastpay_recovery_committees.push(old.clone());
    legacy.fastpay_version_fences.push(fence);
    legacy.fastpay_recovery_reveals.push(reveal);
    let v1 = committed_ledger(&legacy).unwrap();
    let mut installed = legacy.clone();
    let next = FastPayRecoveryCommitteeV1::from_public_keys(
        old.chain_id.clone(), old.genesis_hash.clone(), old.protocol_version, 2, 21, 40,
        old.validator_public_keys(),
    ).unwrap();
    installed.fastpay_recovery_committees.push(next);
    let v2 = committed_ledger(&installed).unwrap();
    assert_ne!(v1, v2);
    for mutate_reveal in [false, true] {
        let mutate = |ledger: &mut LedgerState| {
            let cert = if mutate_reveal {
                &mut ledger.fastpay_recovery_reveals[0].certificate
            } else {
                ledger.fastpay_version_fences[0].certificate.as_mut().unwrap()
            };
            let FastPayCertificateV1::Transfer(cert) = cert else { unreachable!() };
            cert.votes[0].signature_hex = "99".repeat(32);
        };
        let mut changed = legacy.clone();
        mutate(&mut changed);
        assert_eq!(committed_ledger(&changed).unwrap(), v1, "historical bytes stay exact");
        let mut changed = installed.clone();
        mutate(&mut changed);
        assert_ne!(committed_ledger(&changed).unwrap(), v2, "all retained certificates bind after installation");
    }
    let decoded: LedgerState = serde_json::from_slice(&serde_json::to_vec(&installed).unwrap()).unwrap();
    assert_eq!(committed_ledger(&decoded).unwrap(), v2);
    installed.fastpay_recovery_committees.reverse();
    assert!(committed_ledger(&installed).is_err(), "committee history cannot be reordered");
}
