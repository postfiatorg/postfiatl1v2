include!("tests/transport_batch_payload_tests.rs");
include!("tests/rpc_serve_request_tests.rs");
include!("tests/rpc_child_exe_tests.rs");

#[test]
fn fastpay_recovery_governance_commands_reach_their_cli_handler() {
    let bootstrap = run_cli(vec![
        "fastpay-recovery-governance-bootstrap".to_string(),
    ])
    .expect_err("missing bootstrap flags must fail");
    assert_eq!(bootstrap, "missing --validators");

    let assemble = run_cli(vec![
        "fastpay-recovery-governance-bootstrap-assemble".to_string(),
    ])
    .expect_err("missing assembly flags must fail");
    assert_eq!(assemble, "missing --payload-file");

    let prepare = run_cli(vec![
        "fastpay-committee-prepare".to_string(),
        "--valid-from".to_string(),
        "soon".to_string(),
    ])
    .expect_err("a non-numeric height must fail");
    assert_eq!(prepare, "--valid-from must be a u64");

    let control = run_cli(vec![
        "fastswap-control-prepare".to_string(),
        "--control-file".to_string(),
        "unused.json".to_string(),
        "--kind".to_string(),
        "activate-committee".to_string(),
        "--epoch".to_string(),
        "two".to_string(),
    ])
    .expect_err("a non-numeric epoch must fail");
    assert_eq!(control, "--epoch must be a u64");
    for (command, missing) in [
        ("fastswap-control-vote-sign", "missing --control-file"),
        ("fastswap-control-assemble", "missing --control-file"),
    ] {
        assert_eq!(run_cli(vec![command.to_string()]).unwrap_err(), missing);
    }
}

#[test]
fn fastswap_store_migrate_reaches_its_offline_handler() {
    let args = |extra: &[&str]| {
        std::iter::once("fastswap-store-migrate")
            .chain(extra.iter().copied())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(run_cli(args(&[])).unwrap_err(), "missing --data-dir");
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-fastswap-store-migrate-cli-{}",
        std::process::id()
    ));
    let data = data_dir.to_str().expect("utf-8 path");
    let missing = run_cli(args(&["--data-dir", data, "--dry-run"])).unwrap_err();
    assert!(missing.starts_with("fastswap-store-migrate refused:"), "{missing}");
    std::fs::create_dir_all(data_dir.join("fastswap-v1")).expect("store dir");
    run_cli(args(&["--data-dir", data, "--dry-run"])).expect("dry run of an empty store");
    run_cli(args(&["--data-dir", data])).expect("nothing to convert");
    std::fs::remove_dir_all(&data_dir).expect("cleanup");
}

#[test]
fn fastswap_committee_migrate_reaches_its_offline_handler() {
    let args = |extra: &[&str]| {
        std::iter::once("fastswap-committee-migrate")
            .chain(extra.iter().copied())
            .map(str::to_string)
            .collect::<Vec<_>>()
    };
    assert_eq!(run_cli(args(&[])).unwrap_err(), "missing --data-dir");
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-fastswap-committee-migrate-cli-{}",
        std::process::id()
    ));
    let data = data_dir.to_str().expect("utf-8 path");
    assert_eq!(
        run_cli(args(&["--data-dir", data])).unwrap_err(),
        "missing --committee-file"
    );
    let missing = run_cli(args(&[
        "--data-dir",
        data,
        "--committee-file",
        "c.json",
        "--anchor-file",
        "a.json",
        "--dry-run",
    ]))
    .unwrap_err();
    assert!(
        missing.starts_with("fastswap-committee-migrate refused: FastSwap store")
            && missing.ends_with("does not exist"),
        "{missing}"
    );
}

#[test]
fn storage_activation_and_cancellation_commands_reach_their_cli_handlers() {
    let activation_template = run_cli(vec!["storage-activation-template".to_string()])
        .expect_err("missing activation height must fail in the command handler");
    assert_eq!(activation_template, "missing --activation-height");

    let activation_authorize = run_cli(vec!["storage-activation-ratify".to_string()])
        .expect_err("missing activation record must fail in the command handler");
    assert_eq!(activation_authorize, "missing --record-file");

    let activation_batch = run_cli(vec!["storage-activation-batch".to_string()])
        .expect_err("missing activation record must fail in the command handler");
    assert_eq!(activation_batch, "missing --record-file");

    let cancellation_template = run_cli(vec!["storage-cancellation-template".to_string()])
        .expect_err("missing activation ID must fail in the command handler");
    assert_eq!(cancellation_template, "missing --activation-id");

    let cancellation_authorize =
        run_cli(vec!["storage-cancellation-ratify".to_string()])
            .expect_err("missing cancellation record must fail in the command handler");
    assert_eq!(cancellation_authorize, "missing --record-file");

    let cancellation_batch = run_cli(vec!["storage-cancellation-batch".to_string()])
        .expect_err("missing cancellation record must fail in the command handler");
    assert_eq!(cancellation_batch, "missing --record-file");
}

#[test]
fn storage_backend_configuration_requires_offline_and_comparison_acknowledgements() {
    let offline = run_cli(vec!["storage-backend-configure".to_string()])
        .expect_err("backend selection without an offline confirmation must fail");
    assert_eq!(
        offline,
        "storage-backend-configure requires --offline-confirmed after every process using the data directory has been stopped"
    );

    let unsafe_ack = run_cli(vec![
        "storage-backend-configure".to_string(),
        "--offline-confirmed".to_string(),
        "--mode".to_string(),
        "legacy-jsonl".to_string(),
    ])
    .expect_err("comparison selection without unsafe acknowledgement must fail");
    assert_eq!(
        unsafe_ack,
        "legacy-jsonl and bounded-jsonl require --unsafe-comparison-mode and are permitted only on disposable offline qualification clones"
    );
}

#[test]
fn tx_latency_corpus_command_reaches_its_cli_handler() {
    let error = run_cli(vec!["tx-latency-corpus-create".to_string()])
        .expect_err("missing corpus flags must fail in its command handler");
    assert_eq!(error, "missing --data-dir");
}

#[test]
fn tx_latency_signed_transfer_corpus_is_bounded_and_exactly_indexed() {
    let data_dir = std::env::temp_dir().join(format!(
        "postfiat-tx-latency-corpus-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    init(InitOptions {
        data_dir: data_dir.clone(),
        chain_id: "postfiat-tx-latency-corpus".to_string(),
        node_id: "validator-0".to_string(),
        validator_count: 1,
    })
    .expect("initialize corpus fixture");
    let faucet = faucet_key(NodeOptions {
        data_dir: data_dir.clone(),
    })
    .expect("read corpus faucet");
    let quote = transfer_fee_quote(TransferFeeQuoteOptions {
        data_dir: data_dir.clone(),
        from: faucet.address.clone(),
        to: faucet.address.clone(),
        amount: 10,
        sequence: None,
        memo_type: None,
        memo_format: None,
        memo_data: None,
    })
    .expect("quote corpus transfer");
    let signed = wallet_sign_transfer(WalletSignTransferOptions {
        key_file: data_dir.join(postfiat_node::FAUCET_KEY_FILE),
        chain_id: quote.chain_id,
        genesis_hash: quote.genesis_hash,
        protocol_version: quote.protocol_version,
        to: quote.to,
        amount: quote.amount,
        fee: quote.minimum_fee,
        sequence: quote.sequence,
    })
    .expect("sign corpus transfer");
    let corpus_path = data_dir.join("signed-transfer-corpus.json");
    let corpus = TxLatencySignedTransferCorpusV1 {
        schema: TX_LATENCY_SIGNED_TRANSFER_CORPUS_SCHEMA.to_string(),
        transfers: vec![signed.clone()],
    };
    let bytes = serde_json::to_vec_pretty(&corpus).expect("serialize corpus");
    std::fs::write(&corpus_path, &bytes).expect("write corpus");

    let (loaded, sha256) = tx_latency_read_signed_transfer_corpus(
        &corpus_path,
        0,
        1,
        &faucet.address,
        &faucet.address,
        10,
    )
    .expect("read exact corpus");
    assert_eq!(loaded, vec![signed]);
    assert_eq!(sha256, tx_latency_sha256_hex(&bytes));
    let error = tx_latency_read_signed_transfer_corpus(
        &corpus_path,
        1,
        1,
        &faucet.address,
        &faucet.address,
        10,
    )
    .expect_err("out-of-range corpus slice must fail");
    assert!(error.contains("range 1..2 exceeds 1"), "{error}");

    std::fs::remove_dir_all(data_dir).expect("remove corpus fixture");
}

#[test]
fn ordered_history_index_rebuild_requires_explicit_offline_confirmation() {
    let error = run_cli(vec!["ordered-history-index-rebuild".to_string()])
        .expect_err("rebuild without an offline confirmation must fail");
    assert_eq!(
        error,
        "ordered-history-index-rebuild requires --offline-confirmed after every process using the data directory has been stopped"
    );
}

#[test]
fn storage_integrity_migration_requires_explicit_offline_confirmation() {
    let error = run_cli(vec!["storage-integrity-migrate-legacy".to_string()])
        .expect_err("migration without an offline confirmation must fail");
    assert_eq!(
        error,
        "storage-integrity-migrate-legacy requires --offline-confirmed after every process using the data directory has been stopped"
    );
}

#[test]
fn transactional_storage_rebuild_requires_explicit_offline_confirmation() {
    let error = run_cli(vec!["storage-rebuild-transactional".to_string()])
        .expect_err("rebuild without an offline confirmation must fail");
    assert_eq!(
        error,
        "storage-rebuild-transactional requires --offline-confirmed after every process using the data directory has been stopped"
    );
}

#[cfg(test)]
mod replicated_state_activation_cli_tests {
    include!("tests/replicated_state_activation_cli_tests.rs");
}

#[cfg(test)]
mod market_ops_replay_cli_tests {
}

#[test]
fn operator_attestation_commands_reach_their_cli_handler() {
    let root = std::env::temp_dir().join(format!(
        "postfiat-operator-attestation-cli-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time")
            .as_nanos()
    ));
    std::fs::create_dir_all(&root).expect("create fixture root");
    let path = |name: &str| root.join(name).to_str().expect("utf-8 path").to_string();
    let fingerprint = "ab".repeat(32);
    let args = |items: &[&str]| {
        items
            .iter()
            .map(|item| item.to_string())
            .collect::<Vec<_>>()
    };
    run_cli(args(&[
        "operator-onboarding-keygen",
        "--validator-id",
        "validator-0",
        "--master-key-file",
        &path("master.json"),
        "--validator-key-file",
        &path("validator.json"),
    ]))
    .expect("generate onboarding keys");
    run_cli(args(&[
        "operator-attestation-create",
        "--master-key-file",
        &path("master.json"),
        "--validator-id",
        "validator-0",
        "--onboarding-challenge-id",
        &fingerprint,
        "--operator",
        "test-operator",
        "--observed-at",
        "2026-10-17T00:00:00Z",
        "--kind",
        "host",
        "--host-fingerprint",
        &fingerprint,
        "--host-admin-fingerprint",
        &fingerprint,
        "--output",
        &path("host.json"),
    ]))
    .expect("create the host-control attestation");
    run_cli(args(&[
        "operator-attestation-verify",
        "--attestation-file",
        &path("host.json"),
    ]))
    .expect("verify the host-control attestation");
    std::fs::remove_dir_all(root).expect("remove fixture root");
}
