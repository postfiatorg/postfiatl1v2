use super::*;

/// A ledger with FastPay recovery committees for epochs 1 (heights 10-100)
/// and 2 (heights 101-200), read at height 105: epoch 2 is active.
fn two_epoch_fixture(name: &str) -> (std::path::PathBuf, postfiat_types::LedgerState) {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-fastpay-committee-read-{name}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&data_dir).expect("create data dir");
    let genesis = postfiat_types::Genesis::new_with_validator_count("fastpay-committee-read", 4);
    let store = NodeStore::new(&data_dir);
    store.write_genesis(&genesis).expect("write genesis");
    store
        .write_governance(&postfiat_types::GovernanceState::new(4))
        .expect("write governance");
    store
        .write_chain_tip(&postfiat_types::ChainTipState {
            schema: CHAIN_TIP_SCHEMA.to_string(),
            chain_id: genesis.chain_id.clone(),
            genesis_hash: genesis_hash(&genesis),
            protocol_version: genesis.protocol_version,
            height: 105,
            block_hash: "aa".repeat(48),
            state_root: "bb".repeat(48),
            ordered_batch_count: 0,
            receipt_count: 0,
            history_base_height: 0,
        })
        .expect("write tip");
    let keys = |seed: u8| {
        (0..4u8)
            .map(|index| {
                let keypair =
                    postfiat_crypto_provider::ml_dsa_65_keygen_from_seed(&[seed + index; 32]);
                (
                    format!("validator-{index}"),
                    postfiat_crypto_provider::bytes_to_hex(&keypair.public_key),
                )
            })
            .collect::<Vec<_>>()
    };
    let committee = |epoch, from, through, seed| {
        postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
            genesis.chain_id.clone(),
            genesis_hash(&genesis),
            genesis.protocol_version,
            epoch,
            from,
            through,
            keys(seed),
        )
        .expect("build recovery committee")
    };
    let mut ledger = postfiat_types::LedgerState::empty();
    ledger.fastpay_recovery_policy = Some(postfiat_types::FastPayRecoveryPolicyV1 {
        schema: postfiat_types::FASTPAY_RECOVERY_POLICY_SCHEMA_V1.to_string(),
        activation_height: 10,
        max_validity_blocks: 20,
        max_recovery_blocks: 20,
    });
    ledger.fastpay_recovery_committees = vec![committee(1, 10, 100, 1), committee(2, 101, 200, 11)];
    store.write_ledger(&ledger).expect("write ledger");
    (data_dir, ledger)
}

#[test]
fn fastpay_recovery_committee_returns_the_active_epoch_and_each_epoch_by_number() {
    let (data_dir, ledger) = two_epoch_fixture("epochs");
    let active = fastpay_recovery_committee(&data_dir, None).expect("active committee");
    assert_eq!(active.schema, FASTPAY_RECOVERY_COMMITTEE_REPORT_SCHEMA);
    assert_eq!(active.height, 105);
    assert!(active.active);
    assert_eq!(active.committee, ledger.fastpay_recovery_committees[1]);
    assert_eq!(Some(active.policy.clone()), ledger.fastpay_recovery_policy);
    assert_eq!(active.chain_id, active.committee.chain_id);
    assert_eq!(active.genesis_hash, active.committee.genesis_hash);
    assert_eq!(
        active.committee.registry_root,
        active.committee.computed_root().expect("root")
    );

    let first = fastpay_recovery_committee(&data_dir, Some(1)).expect("epoch 1");
    assert!(!first.active);
    assert_eq!(first.committee, ledger.fastpay_recovery_committees[0]);
    assert_eq!(first.policy, active.policy);
    let second = fastpay_recovery_committee(&data_dir, Some(2)).expect("epoch 2");
    assert_eq!(second, active);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn fastpay_recovery_committee_and_admission_export_wait_for_an_in_progress_commit() {
    // A rotation commits epoch 3 (heights 201-300) and moves the tip to 201.
    // A read torn across it would pair the old ledger with height 201 (no
    // active committee) or the new ledger with height 105.
    let (data_dir, mut ledger) = two_epoch_fixture("commit-lock");
    let store = NodeStore::new(&data_dir);
    let commit_lock = store.lock_ordered_commit().expect("in-progress commit");
    let (done_tx, done_rx) = std::sync::mpsc::channel();
    let threads = [false, true].map(|export| {
        let (dir, done) = (data_dir.clone(), done_tx.clone());
        std::thread::spawn(move || {
            let epoch_and_height = if export {
                fastpay_admission_export(&dir)
                    .map(|admission| (admission.committee.committee_epoch, None))
            } else {
                fastpay_recovery_committee(&dir, None)
                    .map(|report| (report.committee.committee_epoch, Some(report.height)))
            };
            done.send((export, epoch_and_height.map_err(|e| e.to_string())))
                .unwrap();
        })
    });
    assert!(
        done_rx
            .recv_timeout(std::time::Duration::from_millis(200))
            .is_err(),
        "the reads must wait for the in-progress commit"
    );
    let epoch_two = &ledger.fastpay_recovery_committees[1];
    let epoch_three = postfiat_types::FastPayRecoveryCommitteeV1::from_public_keys(
        epoch_two.chain_id.clone(),
        epoch_two.genesis_hash.clone(),
        epoch_two.protocol_version,
        3,
        201,
        300,
        epoch_two
            .validators
            .iter()
            .map(|v| (v.validator_id.clone(), v.public_key_hex.clone()))
            .collect(),
    )
    .expect("epoch 3 committee");
    ledger.fastpay_recovery_committees.push(epoch_three);
    store.write_ledger(&ledger).expect("committed ledger");
    let mut tip = store.read_chain_tip().expect("tip");
    tip.height = 201;
    store.write_chain_tip(&tip).expect("committed tip");
    drop(commit_lock);
    let mut results = (0..2)
        .map(|_| {
            done_rx
                .recv_timeout(std::time::Duration::from_secs(5))
                .expect("read after commit")
        })
        .collect::<Vec<_>>();
    results.sort();
    assert_eq!(
        results,
        vec![(false, Ok((3, Some(201)))), (true, Ok((3, None)))]
    );
    for thread in threads {
        thread.join().unwrap();
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn fastpay_recovery_committee_refuses_an_unknown_epoch_with_a_typed_code() {
    let (data_dir, _) = two_epoch_fixture("unknown");
    for epoch in [0, 3, u64::MAX] {
        let error = fastpay_recovery_committee(&data_dir, Some(epoch)).expect_err("unknown epoch");
        assert_eq!(error.kind(), io::ErrorKind::NotFound);
        assert_eq!(
            fastpay_recovery_committee_error_code(&error.to_string()),
            Some(FASTPAY_RECOVERY_COMMITTEE_UNKNOWN_EPOCH)
        );
    }
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn fastpay_recovery_committee_report_holds_public_keys_only() {
    let (data_dir, _) = two_epoch_fixture("public");
    let report = fastpay_recovery_committee(&data_dir, None).expect("active committee");
    let json = serde_json::to_value(&report).expect("report json");
    let validator_fields = json["committee"]["validators"][0]
        .as_object()
        .expect("validator object")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        validator_fields,
        vec!["algorithm_id", "public_key_hex", "validator_id"]
    );
    let text = json.to_string();
    assert!(!text.contains("private") && !text.contains("secret"));
    let _ = std::fs::remove_dir_all(data_dir);
}

/// The checks the Mac wallet core applies to a pinned committee and policy
/// (`core/src/fastpay.rs` `committee()`), reproduced against live capabilities.
fn wallet_core_admits(
    admission: &FastPayAdmissionFile,
    caps: &postfiat_types::FastPayRecoveryCapabilitiesV1,
    chain_id: &str,
    genesis_hash: &str,
) -> bool {
    let committee = &admission.committee;
    let mut keys = BTreeSet::new();
    committee.validate().is_ok()
        && caps.validate().is_ok()
        && admission.policy.validate().is_ok()
        && admission.policy == caps.policy
        && committee.chain_id == chain_id
        && committee.genesis_hash == genesis_hash
        && committee.protocol_version == 1
        && caps.domain == committee.certificate_domain()
        && caps.committee_epoch == committee.committee_epoch
        && caps.validator_count == committee.validators.len()
        && caps.quorum == committee.quorum
        && caps.current_height >= caps.policy.activation_height
        && caps.current_height >= committee.valid_from_height
        && caps.current_height <= committee.new_orders_through_height
        && committee
            .validators
            .iter()
            .all(|validator| keys.insert(validator.public_key_hex.clone()))
}

#[test]
fn fastpay_admission_export_round_trips_and_satisfies_the_wallet_core_rules() {
    let (data_dir, _) = two_epoch_fixture("export");
    let admission = fastpay_admission_export(&data_dir).expect("admission export");
    let path = data_dir.join("fastpay-admission.json");
    std::fs::write(
        &path,
        serde_json::to_string_pretty(&admission).expect("admission json"),
    )
    .expect("write admission file");
    let text = std::fs::read_to_string(&path).expect("read admission file");
    let value: serde_json::Value = serde_json::from_str(&text).expect("admission value");
    let mut top = value.as_object().expect("object").keys().cloned().collect::<Vec<_>>();
    top.sort();
    assert_eq!(top, vec!["committee", "policy"]);
    let parsed: FastPayAdmissionFile = serde_json::from_str(&text).expect("parse back");
    let read = fastpay_recovery_committee(&data_dir, None).expect("active committee");
    assert_eq!(parsed.committee, read.committee);
    assert_eq!(parsed.policy, read.policy);
    // The wallet core parses each half with deny_unknown_fields.
    serde_json::from_value::<postfiat_types::FastPayRecoveryCommitteeV1>(value["committee"].clone())
        .expect("committee half");
    serde_json::from_value::<postfiat_types::FastPayRecoveryPolicyV1>(value["policy"].clone())
        .expect("policy half");

    let caps: postfiat_types::FastPayRecoveryCapabilitiesV1 = serde_json::from_str(
        &owned_recovery_capabilities_v3(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("capabilities"),
    )
    .expect("capabilities json");
    assert!(wallet_core_admits(&parsed, &caps, &read.chain_id, &read.genesis_hash));
    let mut stale = parsed.clone();
    stale.committee = fastpay_recovery_committee(&data_dir, Some(1))
        .expect("epoch 1")
        .committee;
    assert!(!wallet_core_admits(&stale, &caps, &read.chain_id, &read.genesis_hash));
    let mut duplicate = parsed;
    duplicate.committee.validators[1].public_key_hex =
        duplicate.committee.validators[0].public_key_hex.clone();
    assert!(!wallet_core_admits(&duplicate, &caps, &read.chain_id, &read.genesis_hash));
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn fastpay_admission_export_refuses_without_an_active_committee() {
    let (data_dir, mut ledger) = two_epoch_fixture("inactive");
    ledger.fastpay_recovery_committees.truncate(1);
    NodeStore::new(&data_dir)
        .write_ledger(&ledger)
        .expect("drop epoch 2");
    let error = fastpay_admission_export(&data_dir).expect_err("no active committee");
    assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    let _ = std::fs::remove_dir_all(data_dir);
}

#[test]
fn fastpay_admission_export_refuses_each_difference_from_the_capabilities() {
    let (data_dir, _) = two_epoch_fixture("differences");
    let read = fastpay_recovery_committee(&data_dir, None).expect("active committee");
    let caps: postfiat_types::FastPayRecoveryCapabilitiesV1 = serde_json::from_str(
        &owned_recovery_capabilities_v3(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .expect("capabilities"),
    )
    .expect("capabilities json");
    assert!(fastpay_admission_differences(&caps, &read.committee, &read.policy).is_empty());
    let epoch_one = fastpay_recovery_committee(&data_dir, Some(1))
        .expect("epoch 1")
        .committee;
    assert_eq!(
        fastpay_admission_differences(&caps, &epoch_one, &read.policy),
        vec!["domain", "committee_epoch"]
    );
    let mut fewer = caps.clone();
    fewer.validator_count = 3;
    fewer.quorum = 2;
    assert_eq!(
        fastpay_admission_differences(&fewer, &read.committee, &read.policy),
        vec!["validator_count", "quorum"]
    );
    let mut policy = read.policy.clone();
    policy.max_validity_blocks += 1;
    assert_eq!(
        fastpay_admission_differences(&caps, &read.committee, &policy),
        vec!["policy"]
    );
    let _ = std::fs::remove_dir_all(data_dir);
}
