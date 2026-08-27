fn run_cli_group_02(command: &str, flags: &[String]) -> Result<(), String> {
    match command {
        "tx-latency-benchmark" | "real-transaction-latency-benchmark" => {
            let base_dir = PathBuf::from(flag_value(flags, "--base-dir").ok_or("missing --base-dir")?);
            let topology_file =
                PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
            let wallet_key_file = PathBuf::from(
                flag_value(flags, "--wallet-key-file").ok_or("missing --wallet-key-file")?,
            );
            let wallet_address =
                flag_value(flags, "--wallet-address").ok_or("missing --wallet-address")?;
            let recipient = flag_value(flags, "--recipient").ok_or("missing --recipient")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let validators = flag_value(flags, "--validators")
                .unwrap_or("6")
                .parse::<usize>()
                .map_err(|_| "--validators must be a usize".to_string())?;
            let rounds = flag_value(flags, "--rounds")
                .unwrap_or("1000")
                .parse::<usize>()
                .map_err(|_| "--rounds must be a usize".to_string())?;
            let vote_policy = flag_value(flags, "--vote-policy")
                .unwrap_or("full")
                .to_string();
            let artifact_root = PathBuf::from(
                flag_value(flags, "--artifact-root").ok_or("missing --artifact-root")?,
            );
            let report_file =
                PathBuf::from(flag_value(flags, "--report").ok_or("missing --report")?);
            let iterations_file = flag_value(flags, "--iterations-file").map(PathBuf::from);
            let build_mode = flag_value(flags, "--build-mode")
                .unwrap_or("unknown")
                .to_string();
            let generated_utc = flag_value(flags, "--generated-utc").map(str::to_string);
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let send_retries = flag_value(flags, "--send-retries")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| "--send-retries must be a usize".to_string())?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--retry-backoff-ms must be a u64".to_string())?;
            let local_apply_before_certified_send =
                !flag_present(flags, "--local-apply-after-certified-send");
            let defer_certified_sends = flag_present(flags, "--defer-certified-sends");
            let resident_transactional_store =
                flag_present(flags, "--resident-transactional-store");
            let expected_start_height = flag_value(flags, "--expected-start-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--expected-start-height must be a u64".to_string())
                })
                .transpose()?;
            let report = tx_latency_benchmark(TxLatencyBenchmarkOptions {
                base_dir,
                topology_file,
                wallet_key_file,
                wallet_address: wallet_address.to_string(),
                recipient: recipient.to_string(),
                amount,
                validators,
                rounds,
                vote_policy,
                artifact_root,
                report_file,
                iterations_file,
                build_mode,
                generated_utc,
                timeout_ms,
                send_retries,
                retry_backoff_ms,
                local_apply_before_certified_send,
                defer_certified_sends,
                resident_transactional_store,
                expected_start_height,
            })?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("tx latency benchmark serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        _ => unreachable!("run_cli_group_02 dispatch mismatch"),
    }
}
