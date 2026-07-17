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

#[cfg(test)]
mod replicated_state_activation_cli_tests {
    include!("tests/replicated_state_activation_cli_tests.rs");
}

#[cfg(test)]
mod market_ops_replay_cli_tests {
    include!("tests/market_ops_replay_cli_planning_tests.rs");
    include!("tests/market_ops_replay_cli_bridge_tests.rs");
    include!("tests/market_ops_replay_cli_live_tests.rs");
    include!("tests/market_ops_replay_cli_wallet_tests.rs");
}
