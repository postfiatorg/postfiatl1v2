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
