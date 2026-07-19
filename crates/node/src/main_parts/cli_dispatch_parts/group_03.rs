fn rpc_owned_lane_enabled(flags: &[String]) -> bool {
    !flag_present(flags, "--disable-owned-lane")
}

fn run_cli_group_03(command: &str, flags: &[String]) -> Result<(), String> {
    match command {
        "transport-certified-send-outbox-resume" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file =
                PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
            let max_jobs = flag_value(flags, "--max-jobs")
                .unwrap_or("1024")
                .parse::<usize>()
                .map_err(|_| "--max-jobs must be a usize".to_string())?;
            let report = resume_durable_certified_send_outbox(&data_dir, &topology_file, max_jobs)?;
            let all_completed = report.all_completed;
            let pending = report.pending;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("certified send outbox report serialization failed: {error}")
            })?;
            println!("{json}");
            if !all_completed {
                return Err(format!(
                    "certified send outbox resume left {pending} pending job(s)"
                ));
            }
            Ok(())
        }
        "transport-certified-batch-loop" => {
            require_unsafe_devnet_json_storage(flags, "certified batch loop")?;
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_dir = flag_value(flags, "--batch-dir").ok_or("missing --batch-dir")?;
            let validator_key_dir =
                flag_value(flags, "--validator-key-dir").ok_or("missing --validator-key-dir")?;
            let artifact_root =
                flag_value(flags, "--artifact-root").ok_or("missing --artifact-root")?;
            let processed_dir = flag_value(flags, "--processed-dir").map(PathBuf::from);
            let max_rounds = flag_value(flags, "--max-rounds")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-rounds must be a usize".to_string())?;
            let start_height = flag_value(flags, "--start-height")
                .unwrap_or("1")
                .parse::<u64>()
                .map_err(|_| "--start-height must be a u64".to_string())?;
            let poll_ms = flag_value(flags, "--poll-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--poll-ms must be a u64".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let idle_timeout_ms = flag_value(flags, "--idle-timeout-ms")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--idle-timeout-ms must be a u64".to_string())?;
            let send_retries = flag_value(flags, "--send-retries")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| "--send-retries must be a usize".to_string())?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--retry-backoff-ms must be a u64".to_string())?;
            let report = transport_certified_batch_loop(TransportCertifiedBatchLoopOptions {
                data_dir,
                topology_file: PathBuf::from(topology_file),
                batch_kind,
                batch_dir: PathBuf::from(batch_dir),
                validator_key_dir: PathBuf::from(validator_key_dir),
                artifact_root: PathBuf::from(artifact_root),
                processed_dir,
                max_rounds,
                start_height,
                poll_ms,
                timeout_ms,
                idle_timeout_ms,
                send_retries,
                retry_backoff_ms,
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport certified batch loop serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-peer-certified-batch-loop" => {
            require_unsafe_devnet_json_storage(flags, "peer-certified batch loop")?;
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_dir = flag_value(flags, "--batch-dir").ok_or("missing --batch-dir")?;
            let key_file =
                PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
            let proposal_key_file = flag_value(flags, "--proposal-key-file")
                .map(PathBuf::from)
                .or_else(|| Some(key_file.clone()));
            let require_local_proposer = flag_present(flags, "--require-local-proposer");
            let require_signed_proposal = !flag_present(flags, "--allow-unsigned-proposal");
            let allow_peer_failures = flag_present(flags, "--allow-peer-failures");
            let quorum_early_full_propagation =
                flag_present(flags, "--quorum-early-full-propagation");
            let local_apply_before_certified_send =
                flag_present(flags, "--local-apply-before-certified-send");
            let defer_certified_sends = flag_present(flags, "--defer-certified-sends");
            let artifact_root =
                flag_value(flags, "--artifact-root").ok_or("missing --artifact-root")?;
            let processed_dir = flag_value(flags, "--processed-dir").map(PathBuf::from);
            let max_rounds = flag_value(flags, "--max-rounds")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-rounds must be a usize".to_string())?;
            let start_height = flag_value(flags, "--start-height")
                .unwrap_or("1")
                .parse::<u64>()
                .map_err(|_| "--start-height must be a u64".to_string())?;
            let poll_ms = flag_value(flags, "--poll-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--poll-ms must be a u64".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let idle_timeout_ms = flag_value(flags, "--idle-timeout-ms")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--idle-timeout-ms must be a u64".to_string())?;
            let send_retries = flag_value(flags, "--send-retries")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| "--send-retries must be a usize".to_string())?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--retry-backoff-ms must be a u64".to_string())?;
            let report =
                transport_peer_certified_batch_loop(TransportPeerCertifiedBatchLoopOptions {
                    data_dir,
                    topology_file: PathBuf::from(topology_file),
                    batch_kind,
                    batch_dir: PathBuf::from(batch_dir),
                    key_file,
                    proposal_key_file,
                    require_local_proposer,
                    require_signed_proposal,
                    allow_peer_failures,
                    quorum_early_full_propagation,
                    artifact_root: PathBuf::from(artifact_root),
                    processed_dir,
                    max_rounds,
                    start_height,
                    poll_ms,
                    timeout_ms,
                    idle_timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    local_apply_before_certified_send,
                    defer_certified_sends,
                })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport peer certified batch loop serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-peer-certified-private-egress-loop" => {
            require_unsafe_devnet_json_storage(flags, "peer-certified private egress loop")?;
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let egress_dir = flag_value(flags, "--egress-dir").ok_or("missing --egress-dir")?;
            let batch_dir = flag_value(flags, "--batch-dir").ok_or("missing --batch-dir")?;
            let key_file =
                PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
            let proposal_key_file = flag_value(flags, "--proposal-key-file")
                .map(PathBuf::from)
                .or_else(|| Some(key_file.clone()));
            let require_local_proposer = flag_present(flags, "--require-local-proposer");
            let require_signed_proposal = !flag_present(flags, "--allow-unsigned-proposal");
            let allow_peer_failures = flag_present(flags, "--allow-peer-failures");
            let quorum_early_full_propagation =
                flag_present(flags, "--quorum-early-full-propagation");
            let local_apply_before_certified_send =
                flag_present(flags, "--local-apply-before-certified-send");
            let defer_certified_sends = flag_present(flags, "--defer-certified-sends");
            let artifact_root =
                flag_value(flags, "--artifact-root").ok_or("missing --artifact-root")?;
            let ready_file = flag_value(flags, "--ready-file").map(PathBuf::from);
            let processed_egress_dir =
                flag_value(flags, "--processed-egress-dir").map(PathBuf::from);
            let processed_batch_dir = flag_value(flags, "--processed-batch-dir").map(PathBuf::from);
            let max_rounds = flag_value(flags, "--max-rounds")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-rounds must be a usize".to_string())?;
            let start_height = flag_value(flags, "--start-height")
                .unwrap_or("1")
                .parse::<u64>()
                .map_err(|_| "--start-height must be a u64".to_string())?;
            let poll_ms = flag_value(flags, "--poll-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--poll-ms must be a u64".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let idle_timeout_ms = flag_value(flags, "--idle-timeout-ms")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--idle-timeout-ms must be a u64".to_string())?;
            let send_retries = flag_value(flags, "--send-retries")
                .unwrap_or("0")
                .parse::<usize>()
                .map_err(|_| "--send-retries must be a usize".to_string())?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .unwrap_or("250")
                .parse::<u64>()
                .map_err(|_| "--retry-backoff-ms must be a u64".to_string())?;
            let report = transport_peer_certified_private_egress_loop(
                TransportPeerCertifiedPrivateEgressLoopOptions {
                    data_dir,
                    topology_file: PathBuf::from(topology_file),
                    egress_dir: PathBuf::from(egress_dir),
                    batch_dir: PathBuf::from(batch_dir),
                    key_file,
                    proposal_key_file,
                    require_local_proposer,
                    require_signed_proposal,
                    allow_peer_failures,
                    quorum_early_full_propagation,
                    artifact_root: PathBuf::from(artifact_root),
                    ready_file,
                    processed_egress_dir,
                    processed_batch_dir,
                    max_rounds,
                    start_height,
                    poll_ms,
                    timeout_ms,
                    idle_timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    local_apply_before_certified_send,
                    defer_certified_sends,
                },
            )?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!(
                    "transport peer certified private egress loop serialization failed: {error}"
                )
            })?;
            println!("{json}");
            Ok(())
        }
        "rpc-serve" => {
            require_unsafe_devnet_json_storage(flags, "rpc service")?;
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let spool_dir = flag_value(flags, "--spool-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.join("runtime/rpc-spool"));
            let ready_file = flag_value(flags, "--ready-file")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.join("readiness/rpc.ready.json"));
            let bind_host = flag_value(flags, "--bind-host")
                .unwrap_or("127.0.0.1")
                .to_string();
            let port = parse_u16_flag(flags, "--port")?;
            let max_requests = flag_value(flags, "--max-requests")
                .unwrap_or("100")
                .parse::<usize>()
                .map_err(|_| "--max-requests must be a usize".to_string())?;
            if max_requests == 0 {
                return Err("--max-requests must be positive".to_string());
            }
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("30000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let child_timeout_ms = flag_value(flags, "--child-timeout-ms")
                .map(str::to_string)
                .unwrap_or_else(|| timeout_ms.to_string())
                .parse::<u64>()
                .map_err(|_| "--child-timeout-ms must be a u64".to_string())?;
            let event_log = flag_value(flags, "--event-log").map(PathBuf::from);
            let allow_mempool_submit = flag_present(flags, "--allow-mempool-submit");
            let allow_mempool_submit_finality =
                flag_present(flags, "--allow-mempool-submit-finality");
            let allow_orchard_batch_create = flag_present(flags, "--allow-orchard-batch-create");
            // FastPay is a normal signed payment capability. Operators may
            // disable it explicitly during an incident, but ordinary startup
            // must not silently remove the lane.
            let owned_lane_enabled = rpc_owned_lane_enabled(flags);
            let finality_topology_file = flag_value(flags, "--finality-topology")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.join("remote-topology.json"));
            let finality_key_file = flag_value(flags, "--finality-key-file")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.join(VALIDATOR_KEYS_FILE));
            let finality_proposal_key_file = flag_value(flags, "--finality-proposal-key-file")
                .map(PathBuf::from)
                .or_else(|| Some(finality_key_file.clone()));
            let finality_artifact_root = flag_value(flags, "--finality-artifact-root")
                .map(PathBuf::from)
                .unwrap_or_else(|| data_dir.join("rpc-finality-artifacts"));
            let finality_timeout_ms = flag_value(flags, "--finality-timeout-ms")
                .map(str::to_string)
                .unwrap_or_else(|| timeout_ms.to_string())
                .parse::<u64>()
                .map_err(|_| "--finality-timeout-ms must be a u64".to_string())?;
            let finality_send_retries = flag_value(flags, "--finality-send-retries")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--finality-send-retries must be a usize".to_string())?;
            if finality_send_retries > 16 {
                return Err("--finality-send-retries must be <= 16".to_string());
            }
            let finality_retry_backoff_ms = flag_value(flags, "--finality-retry-backoff-ms")
                .unwrap_or("25")
                .parse::<u64>()
                .map_err(|_| "--finality-retry-backoff-ms must be a u64".to_string())?;
            let finality_quorum_early_full_propagation =
                flag_present(flags, "--finality-quorum-early-full-propagation");
            let max_mempool_submit_per_peer = flag_value(flags, "--max-mempool-submit-per-peer")
                .map(str::to_string)
                .unwrap_or_else(|| DEFAULT_RPC_MEMPOOL_SUBMIT_PER_PEER.to_string())
                .parse::<u64>()
                .map_err(|_| "--max-mempool-submit-per-peer must be a u64".to_string())?;
            if max_mempool_submit_per_peer == 0 {
                return Err("--max-mempool-submit-per-peer must be positive".to_string());
            }
            let max_mempool_submit_total = flag_value(flags, "--max-mempool-submit-total")
                .map(str::to_string)
                .unwrap_or_else(|| DEFAULT_RPC_MEMPOOL_SUBMIT_TOTAL.to_string())
                .parse::<u64>()
                .map_err(|_| "--max-mempool-submit-total must be a u64".to_string())?;
            if max_mempool_submit_total == 0 {
                return Err("--max-mempool-submit-total must be positive".to_string());
            }
            let max_orchard_batch_create_per_peer =
                flag_value(flags, "--max-orchard-batch-create-per-peer")
                    .map(str::to_string)
                    .unwrap_or_else(|| DEFAULT_RPC_ORCHARD_BATCH_CREATE_PER_PEER.to_string())
                    .parse::<u64>()
                    .map_err(|_| "--max-orchard-batch-create-per-peer must be a u64".to_string())?;
            if max_orchard_batch_create_per_peer == 0 {
                return Err("--max-orchard-batch-create-per-peer must be positive".to_string());
            }
            let max_orchard_batch_create_total =
                flag_value(flags, "--max-orchard-batch-create-total")
                    .map(str::to_string)
                    .unwrap_or_else(|| DEFAULT_RPC_ORCHARD_BATCH_CREATE_TOTAL.to_string())
                    .parse::<u64>()
                    .map_err(|_| "--max-orchard-batch-create-total must be a u64".to_string())?;
            if max_orchard_batch_create_total == 0 {
                return Err("--max-orchard-batch-create-total must be positive".to_string());
            }
            let max_orchard_batch_create_concurrent =
                flag_value(flags, "--max-orchard-batch-create-concurrent")
                    .map(str::to_string)
                    .unwrap_or_else(|| DEFAULT_RPC_ORCHARD_BATCH_CREATE_CONCURRENT.to_string())
                    .parse::<u64>()
                    .map_err(|_| {
                        "--max-orchard-batch-create-concurrent must be a u64".to_string()
                    })?;
            if max_orchard_batch_create_concurrent == 0 {
                return Err("--max-orchard-batch-create-concurrent must be positive".to_string());
            }
            let keep_alive = flag_present(flags, "--keep-alive");
            let report = rpc_serve(RpcServeOptions {
                data_dir,
                spool_dir,
                ready_file,
                bind_host,
                port,
                max_requests,
                timeout_ms,
                child_timeout_ms,
                event_log,
                allow_mempool_submit,
                allow_mempool_submit_finality,
                allow_orchard_batch_create,
                owned_lane_enabled,
                finality_topology_file,
                finality_key_file,
                finality_proposal_key_file,
                finality_artifact_root,
                finality_timeout_ms,
                finality_send_retries,
                finality_retry_backoff_ms,
                finality_quorum_early_full_propagation,
                max_mempool_submit_per_peer,
                max_mempool_submit_total,
                max_orchard_batch_create_per_peer,
                max_orchard_batch_create_total,
                max_orchard_batch_create_concurrent,
                keep_alive,
            })?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("rpc serve serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "validator-keys" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators = flag_value(flags, "--validators")
                .unwrap_or("4")
                .parse::<u32>()
                .map_err(|_| "--validators must be a u32".to_string())?;
            let keys = validator_keys(ValidatorKeysOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                local_only: false,
            })
            .map_err(|error| format!("validator key generation failed: {error}"))?;
            let json = serde_json::to_string_pretty(&keys)
                .map_err(|error| format!("validator key serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "validator-key-stage" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let source_key_file =
                flag_value(flags, "--source-key-file").ok_or("missing --source-key-file")?;
            let validator_id = flag_value(flags, "--validator-id")
                .ok_or("missing --validator-id")?
                .to_string();
            let source_validator_id =
                flag_value(flags, "--source-validator-id").map(str::to_string);
            let report = stage_validator_key(ValidatorKeyStageOptions {
                data_dir: PathBuf::from(data_dir),
                source_key_file: PathBuf::from(source_key_file),
                validator_id,
                source_validator_id,
                replace: flag_present(flags, "--replace"),
            })
            .map_err(|error| format!("validator key staging failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("validator key staging serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "validate-local-keys" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators = flag_value(flags, "--validators")
                .unwrap_or("4")
                .parse::<u32>()
                .map_err(|_| "--validators must be a u32".to_string())?;
            let local_only = flag_present(flags, "--local-only");
            let report = validate_local_keys(ValidatorKeysOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                local_only,
            })
            .map_err(|error| format!("local key validation failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("local key validation serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "run" => {
            require_unsafe_devnet_json_storage(flags, "node run service")?;
            let mode = flag_value(flags, "--mode").unwrap_or("once");
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            match mode {
                "once" => {
                    let report = run_once(NodeOptions { data_dir })
                        .map_err(|error| format!("run failed: {error}"))?;
                    let json = report
                        .to_json()
                        .map_err(|error| format!("run report serialization failed: {error}"))?;
                    print!("{json}");
                }
                "peer-certified" | "peer_certified" => {
                    let options = run_peer_certified_loop_options(flags, data_dir)?;
                    let report = transport_peer_certified_batch_loop(options)
                        .map_err(|error| format!("peer-certified run failed: {error}"))?;
                    let json = serde_json::to_string_pretty(&report).map_err(|error| {
                        format!("peer-certified run serialization failed: {error}")
                    })?;
                    println!("{json}");
                }
                _ => return Err("--mode must be once or peer-certified".to_string()),
            }
            Ok(())
        }
        "status" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let expect_height = flag_value(flags, "--expect-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--expect-height must be a u64".to_string())
                })
                .transpose()?;
            let report = status(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("status failed: {error}"))?;
            let json = report
                .to_json()
                .map_err(|error| format!("status report serialization failed: {error}"))?;
            print!("{json}");
            if let Some(expected) = expect_height {
                if report.block_height != expected {
                    return Err(format!(
                        "status expected block_height {expected}, got {}",
                        report.block_height
                    ));
                }
            }
            Ok(())
        }
        "history-status" => {
            let options = parse_history_options(flags)?;
            let report = history_status(options)
                .map_err(|error| format!("history-status failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("history status serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "history-prune-plan" | "history-can-prune" => {
            let options = parse_history_options(flags)?;
            let report = history_prune_plan(options)
                .map_err(|error| format!("history-prune-plan failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("history prune-plan serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "history-prune" => {
            let options = parse_history_options(flags)?;
            let report =
                history_prune(options).map_err(|error| format!("history-prune failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("history prune serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "history-prune-recover" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = history_prune_recover(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("history-prune-recover failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("history prune recovery serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "history-checkpoint-rebuild-from-archive" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let backup_file = flag_value(flags, "--backup-file")
                .ok_or("missing --backup-file")?;
            let report = history_checkpoint_rebuild_from_archive(
                HistoryCheckpointRebuildFromArchiveOptions {
                    data_dir: PathBuf::from(data_dir),
                    backup_file: PathBuf::from(backup_file),
                },
            )
            .map_err(|error| {
                format!("history-checkpoint-rebuild-from-archive failed: {error}")
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("history checkpoint rebuild serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "history-archive-handoff-create" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let from_height = parse_u64_flag(flags, "--from-height")?;
            let to_height = parse_u64_flag(flags, "--to-height")?;
            let archive_uri = flag_value(flags, "--archive-uri").map(str::to_string);
            let output_file = flag_value(flags, "--output").ok_or("missing --output")?;
            let proof = create_history_archive_handoff(HistoryArchiveHandoffCreateOptions {
                data_dir: PathBuf::from(data_dir),
                from_height,
                to_height,
                archive_uri,
                output_file: PathBuf::from(output_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("history-archive-handoff-create failed: {error}"))?;
            let json = serde_json::to_string_pretty(&proof).map_err(|error| {
                format!("history archive handoff serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "history-archive-handoff-verify" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let proof_file = flag_value(flags, "--proof-file").ok_or("missing --proof-file")?;
            let report = verify_history_archive_handoff(HistoryArchiveHandoffVerifyOptions {
                data_dir: PathBuf::from(data_dir),
                proof_file: PathBuf::from(proof_file),
            })
            .map_err(|error| format!("history-archive-handoff-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("history archive handoff verification serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "archive-export-window" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let from_height = parse_u64_flag(flags, "--from-height")?;
            let to_height = parse_u64_flag(flags, "--to-height")?;
            let output_file = flag_value(flags, "--output").ok_or("missing --output")?;
            let bundle = export_history_archive_window(HistoryArchiveWindowExportOptions {
                data_dir: PathBuf::from(data_dir),
                from_height,
                to_height,
                archive_uri: flag_value(flags, "--archive-uri").map(str::to_string),
                output_file: PathBuf::from(output_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("archive-export-window failed: {error}"))?;
            let json = serde_json::to_string_pretty(&bundle)
                .map_err(|error| format!("archive export window serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "archive-window-verify" => {
            let bundle_file = flag_value(flags, "--bundle-file").ok_or("missing --bundle-file")?;
            let report = verify_history_archive_window_bundle(HistoryArchiveWindowVerifyOptions {
                bundle_file: PathBuf::from(bundle_file),
            })
            .map_err(|error| format!("archive-window-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("archive window verify serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "archive-window-import" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let bundle_file = flag_value(flags, "--bundle-file").ok_or("missing --bundle-file")?;
            let report = import_history_archive_window(HistoryArchiveWindowImportOptions {
                data_dir: PathBuf::from(data_dir),
                bundle_file: PathBuf::from(bundle_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("archive-window-import failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("archive window import serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "archive-window-backfill" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let source_host = flag_value(flags, "--source-host")
                .ok_or("missing --source-host")?
                .to_string();
            let source_rpc_port = parse_u16_flag(flags, "--source-rpc-port")?;
            let from_height = parse_u64_flag(flags, "--from-height")?;
            let to_height = parse_u64_flag(flags, "--to-height")?;
            let work_dir = flag_value(flags, "--work-dir")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("reports/archive-window-backfill"));
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let report = archive_window_backfill(ArchiveWindowBackfillOptions {
                data_dir,
                source_host,
                source_rpc_port,
                work_dir,
                from_height,
                to_height,
                archive_uri: flag_value(flags, "--archive-uri").map(str::to_string),
                timeout_ms,
                overwrite: flag_present(flags, "--overwrite"),
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("archive window backfill serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "block-proposer" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let block_height = flag_value(flags, "--height")
                .ok_or("missing --height")?
                .parse::<u64>()
                .map_err(|_| "--height must be a u64".to_string())?;
            let view = flag_value(flags, "--view")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--view must be a u64".to_string())?;
            let report = block_proposer(BlockProposerOptions {
                data_dir: PathBuf::from(data_dir),
                block_height,
                view,
            })
            .map_err(|error| format!("block-proposer failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("block proposer serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "metrics" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let report = metrics(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("metrics failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("metrics serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "faucet" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let key_file = faucet_key(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("faucet failed: {error}"))?;
            let json = serde_json::to_string_pretty(&key_file)
                .map_err(|error| format!("faucet serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "owned-apply" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let cert_file = flag_value(flags, "--cert-file")
                .ok_or_else(|| "--cert-file <path> is required".to_string())?;
            let cert_json = std::fs::read_to_string(&cert_file)
                .map_err(|error| format!("owned-apply: read cert file failed: {error}"))?;
            let summary = owned_apply(
                NodeOptions {
                    data_dir: PathBuf::from(data_dir),
                },
                &cert_json,
            )
            .map_err(|error| format!("owned-apply failed: {error}"))?;
            println!("{summary}");
            Ok(())
        }
        "owned-sign" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let order_file = flag_value(flags, "--order-file")
                .ok_or_else(|| "--order-file <path> is required".to_string())?;
            let validator_id = flag_value(flags, "--validator-id")
                .ok_or_else(|| "--validator-id <id> is required".to_string())?;
            let order_json = std::fs::read_to_string(&order_file)
                .map_err(|error| format!("owned-sign: read order file failed: {error}"))?;
            let vote = owned_sign(
                NodeOptions {
                    data_dir: PathBuf::from(data_dir),
                },
                &order_json,
                &validator_id,
            )
            .map_err(|error| format!("owned-sign failed: {error}"))?;
            println!("{vote}");
            Ok(())
        }
        "owned-unwrap-apply" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let cert_file = flag_value(flags, "--cert-file")
                .ok_or_else(|| "--cert-file <path> is required".to_string())?;
            let cert_json = std::fs::read_to_string(&cert_file)
                .map_err(|error| format!("owned-unwrap-apply: read cert file failed: {error}"))?;
            let summary = owned_unwrap_apply(
                NodeOptions {
                    data_dir: PathBuf::from(data_dir),
                },
                &cert_json,
            )
            .map_err(|error| format!("owned-unwrap-apply failed: {error}"))?;
            println!("{summary}");
            Ok(())
        }
        "owned-unwrap-sign" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let order_file = flag_value(flags, "--order-file")
                .ok_or_else(|| "--order-file <path> is required".to_string())?;
            let validator_id = flag_value(flags, "--validator-id")
                .ok_or_else(|| "--validator-id <id> is required".to_string())?;
            let order_json = std::fs::read_to_string(&order_file)
                .map_err(|error| format!("owned-unwrap-sign: read order file failed: {error}"))?;
            let vote = owned_unwrap_sign(
                NodeOptions {
                    data_dir: PathBuf::from(data_dir),
                },
                &order_json,
                &validator_id,
            )
            .map_err(|error| format!("owned-unwrap-sign failed: {error}"))?;
            println!("{vote}");
            Ok(())
        }
        "checkpoint-pending" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let summary = checkpoint_pending(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("checkpoint-pending failed: {error}"))?;
            println!("{summary}");
            Ok(())
        }
        "owned-safe-unlock" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let summary = owned_safe_unlock(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("owned-safe-unlock failed: {error}"))?;
            println!("{summary}");
            Ok(())
        }
        "wallet-keygen" => {
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let master_seed_hex = required_secret_flag(
                flags,
                "--master-seed-hex",
                "--master-seed-hex-file",
                "wallet master seed",
            )?;
            let account_index = flag_value(flags, "--account-index")
                .unwrap_or("0")
                .parse::<u32>()
                .map_err(|_| "--account-index must be a u32".to_string())?;
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
            let report = wallet_keygen(WalletKeygenOptions {
                chain_id: chain_id.to_string(),
                master_seed_hex: master_seed_hex.to_string(),
                account_index,
                key_file: PathBuf::from(key_file),
                backup_file: PathBuf::from(backup_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("wallet-keygen failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("wallet-keygen serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "wallet-restore" => {
            let backup_file = flag_value(flags, "--backup-file").ok_or("missing --backup-file")?;
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let report = wallet_restore(WalletRestoreOptions {
                backup_file: PathBuf::from(backup_file),
                key_file: PathBuf::from(key_file),
                overwrite: flag_present(flags, "--overwrite"),
            })
            .map_err(|error| format!("wallet-restore failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("wallet-restore serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "wallet-sign-transfer" | "wallet_sign_transfer" => {
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let sign_options = if let Some(quote_file) = flag_value(flags, "--quote-file") {
                for flag in [
                    "--chain-id",
                    "--genesis-hash",
                    "--protocol-version",
                    "--to",
                    "--amount",
                    "--fee",
                    "--sequence",
                ] {
                    if flag_value(flags, flag).is_some() {
                        return Err(format!("{flag} cannot be used with --quote-file"));
                    }
                }
                let quote = read_wallet_sign_transfer_quote_file(Path::new(quote_file))?;
                WalletSignTransferOptions {
                    key_file: PathBuf::from(key_file),
                    chain_id: quote.chain_id,
                    genesis_hash: quote.genesis_hash,
                    protocol_version: quote.protocol_version,
                    to: quote.to,
                    amount: quote.amount,
                    fee: quote.minimum_fee,
                    sequence: quote.sequence,
                }
            } else {
                let chain_id = flag_value(flags, "--chain-id").ok_or("missing --chain-id")?;
                let genesis_hash =
                    flag_value(flags, "--genesis-hash").ok_or("missing --genesis-hash")?;
                let protocol_version = flag_value(flags, "--protocol-version")
                    .ok_or("missing --protocol-version")?
                    .parse::<u32>()
                    .map_err(|_| "--protocol-version must be a u32".to_string())?;
                let to = flag_value(flags, "--to").ok_or("missing --to")?;
                let amount = flag_value(flags, "--amount")
                    .ok_or("missing --amount")?
                    .parse::<u64>()
                    .map_err(|_| "--amount must be a u64".to_string())?;
                let fee = flag_value(flags, "--fee")
                    .ok_or("missing --fee")?
                    .parse::<u64>()
                    .map_err(|_| "--fee must be a u64".to_string())?;
                let sequence = flag_value(flags, "--sequence")
                    .ok_or("missing --sequence")?
                    .parse::<u64>()
                    .map_err(|_| "--sequence must be a u64".to_string())?;
                WalletSignTransferOptions {
                    key_file: PathBuf::from(key_file),
                    chain_id: chain_id.to_string(),
                    genesis_hash: genesis_hash.to_string(),
                    protocol_version,
                    to: to.to_string(),
                    amount,
                    fee,
                    sequence,
                }
            };
            let signed = wallet_sign_transfer(sign_options)
                .map_err(|error| format!("wallet-sign-transfer failed: {error}"))?;
            let json = serde_json::to_string_pretty(&signed)
                .map_err(|error| format!("wallet-sign-transfer serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "wallet-sign-asset-transaction" | "wallet_sign_asset_transaction" => {
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let sign_options = if let Some(quote_file) = flag_value(flags, "--quote-file") {
                for flag in [
                    "--chain-id",
                    "--genesis-hash",
                    "--protocol-version",
                    "--fee",
                    "--sequence",
                    "--operation-json",
                ] {
                    if flag_value(flags, flag).is_some() {
                        return Err(format!("{flag} cannot be used with --quote-file"));
                    }
                }
                let quote = read_wallet_sign_asset_transaction_quote_file(Path::new(quote_file))?;
                WalletSignAssetTransactionOptions {
                    key_file: PathBuf::from(key_file),
                    chain_id: quote.chain_id,
                    genesis_hash: quote.genesis_hash,
                    protocol_version: quote.protocol_version,
                    fee: quote.minimum_fee,
                    sequence: quote.sequence,
                    expected_source: Some(quote.source),
                    operation: quote.operation,
                }
            } else {
                let chain_id = flag_value(flags, "--chain-id").ok_or("missing --chain-id")?;
                let genesis_hash =
                    flag_value(flags, "--genesis-hash").ok_or("missing --genesis-hash")?;
                let protocol_version = flag_value(flags, "--protocol-version")
                    .ok_or("missing --protocol-version")?
                    .parse::<u32>()
                    .map_err(|_| "--protocol-version must be a u32".to_string())?;
                let fee = flag_value(flags, "--fee")
                    .ok_or("missing --fee")?
                    .parse::<u64>()
                    .map_err(|_| "--fee must be a u64".to_string())?;
                let sequence = flag_value(flags, "--sequence")
                    .ok_or("missing --sequence")?
                    .parse::<u64>()
                    .map_err(|_| "--sequence must be a u64".to_string())?;
                let operation_json =
                    flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
                let operation = serde_json::from_str::<postfiat_types::AssetTransactionOperation>(
                    operation_json,
                )
                .map_err(|error| {
                    format!("--operation-json is not a valid asset operation: {error}")
                })?;
                WalletSignAssetTransactionOptions {
                    key_file: PathBuf::from(key_file),
                    chain_id: chain_id.to_string(),
                    genesis_hash: genesis_hash.to_string(),
                    protocol_version,
                    fee,
                    sequence,
                    expected_source: None,
                    operation,
                }
            };
            let signed = wallet_sign_asset_transaction(sign_options)
                .map_err(|error| format!("wallet-sign-asset-transaction failed: {error}"))?;
            let json = serde_json::to_string_pretty(&signed).map_err(|error| {
                format!("wallet-sign-asset-transaction serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "wallet-sign-escrow-transaction" | "wallet_sign_escrow_transaction" => {
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let sign_options = if let Some(quote_file) = flag_value(flags, "--quote-file") {
                for flag in [
                    "--chain-id",
                    "--genesis-hash",
                    "--protocol-version",
                    "--fee",
                    "--sequence",
                    "--operation-json",
                ] {
                    if flag_value(flags, flag).is_some() {
                        return Err(format!("{flag} cannot be used with --quote-file"));
                    }
                }
                let quote = read_wallet_sign_escrow_transaction_quote_file(Path::new(quote_file))?;
                WalletSignEscrowTransactionOptions {
                    key_file: PathBuf::from(key_file),
                    chain_id: quote.chain_id,
                    genesis_hash: quote.genesis_hash,
                    protocol_version: quote.protocol_version,
                    fee: quote.minimum_fee,
                    sequence: quote.sequence,
                    expected_source: Some(quote.source),
                    operation: quote.operation,
                }
            } else {
                let chain_id = flag_value(flags, "--chain-id").ok_or("missing --chain-id")?;
                let genesis_hash =
                    flag_value(flags, "--genesis-hash").ok_or("missing --genesis-hash")?;
                let protocol_version = flag_value(flags, "--protocol-version")
                    .ok_or("missing --protocol-version")?
                    .parse::<u32>()
                    .map_err(|_| "--protocol-version must be a u32".to_string())?;
                let fee = flag_value(flags, "--fee")
                    .ok_or("missing --fee")?
                    .parse::<u64>()
                    .map_err(|_| "--fee must be a u64".to_string())?;
                let sequence = flag_value(flags, "--sequence")
                    .ok_or("missing --sequence")?
                    .parse::<u64>()
                    .map_err(|_| "--sequence must be a u64".to_string())?;
                let operation_json =
                    flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
                let operation = serde_json::from_str::<postfiat_types::EscrowTransactionOperation>(
                    operation_json,
                )
                .map_err(|error| {
                    format!("--operation-json is not a valid escrow operation: {error}")
                })?;
                WalletSignEscrowTransactionOptions {
                    key_file: PathBuf::from(key_file),
                    chain_id: chain_id.to_string(),
                    genesis_hash: genesis_hash.to_string(),
                    protocol_version,
                    fee,
                    sequence,
                    expected_source: None,
                    operation,
                }
            };
            let signed = wallet_sign_escrow_transaction(sign_options)
                .map_err(|error| format!("wallet-sign-escrow-transaction failed: {error}"))?;
            let json = serde_json::to_string_pretty(&signed).map_err(|error| {
                format!("wallet-sign-escrow-transaction serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "wallet-sign-offer-transaction" | "wallet_sign_offer_transaction" => {
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let sign_options = if let Some(quote_file) = flag_value(flags, "--quote-file") {
                for flag in [
                    "--chain-id",
                    "--genesis-hash",
                    "--protocol-version",
                    "--fee",
                    "--sequence",
                    "--operation-json",
                ] {
                    if flag_value(flags, flag).is_some() {
                        return Err(format!("{flag} cannot be used with --quote-file"));
                    }
                }
                let quote = read_wallet_sign_offer_transaction_quote_file(Path::new(quote_file))?;
                WalletSignOfferTransactionOptions {
                    key_file: PathBuf::from(key_file),
                    chain_id: quote.chain_id,
                    genesis_hash: quote.genesis_hash,
                    protocol_version: quote.protocol_version,
                    fee: quote.minimum_fee,
                    sequence: quote.sequence,
                    expected_source: Some(quote.source),
                    operation: quote.operation,
                }
            } else {
                let chain_id = flag_value(flags, "--chain-id").ok_or("missing --chain-id")?;
                let genesis_hash =
                    flag_value(flags, "--genesis-hash").ok_or("missing --genesis-hash")?;
                let protocol_version = flag_value(flags, "--protocol-version")
                    .ok_or("missing --protocol-version")?
                    .parse::<u32>()
                    .map_err(|_| "--protocol-version must be a u32".to_string())?;
                let fee = flag_value(flags, "--fee")
                    .ok_or("missing --fee")?
                    .parse::<u64>()
                    .map_err(|_| "--fee must be a u64".to_string())?;
                let sequence = flag_value(flags, "--sequence")
                    .ok_or("missing --sequence")?
                    .parse::<u64>()
                    .map_err(|_| "--sequence must be a u64".to_string())?;
                let operation_json =
                    flag_value(flags, "--operation-json").ok_or("missing --operation-json")?;
                let operation = serde_json::from_str::<postfiat_types::OfferTransactionOperation>(
                    operation_json,
                )
                .map_err(|error| {
                    format!("--operation-json is not a valid offer operation: {error}")
                })?;
                WalletSignOfferTransactionOptions {
                    key_file: PathBuf::from(key_file),
                    chain_id: chain_id.to_string(),
                    genesis_hash: genesis_hash.to_string(),
                    protocol_version,
                    fee,
                    sequence,
                    expected_source: None,
                    operation,
                }
            };
            let signed = wallet_sign_offer_transaction(sign_options)
                .map_err(|error| format!("wallet-sign-offer-transaction failed: {error}"))?;
            let json = serde_json::to_string_pretty(&signed).map_err(|error| {
                format!("wallet-sign-offer-transaction serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "wallet-test-vector" => {
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let validator_count = flag_value(flags, "--validators")
                .unwrap_or("1")
                .parse::<u32>()
                .map_err(|_| "--validators must be a u32".to_string())?;
            let master_seed_hex = required_secret_flag(
                flags,
                "--master-seed-hex",
                "--master-seed-hex-file",
                "wallet test-vector master seed",
            )?;
            let account_index = flag_value(flags, "--account-index")
                .unwrap_or("0")
                .parse::<u32>()
                .map_err(|_| "--account-index must be a u32".to_string())?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let sequence = flag_value(flags, "--sequence")
                .unwrap_or("1")
                .parse::<u64>()
                .map_err(|_| "--sequence must be a u64".to_string())?;
            let signature_seed_hex = if flag_value(flags, "--signature-seed-hex").is_some()
                || flag_value(flags, "--signature-seed-hex-file").is_some()
            {
                required_secret_flag(
                    flags,
                    "--signature-seed-hex",
                    "--signature-seed-hex-file",
                    "wallet test-vector signature seed",
                )?
            } else {
                Zeroizing::new(
                    "1f1e1d1c1b1a191817161514131211100f0e0d0c0b0a09080706050403020100".to_string(),
                )
            };
            let report = wallet_test_vector(WalletTestVectorOptions {
                chain_id: chain_id.to_string(),
                validator_count,
                master_seed_hex: master_seed_hex.to_string(),
                account_index,
                to: to.to_string(),
                amount,
                sequence,
                signature_seed_hex: signature_seed_hex.to_string(),
            })
            .map_err(|error| format!("wallet-test-vector failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("wallet-test-vector serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "orchard-test-vector" => {
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let validator_count = flag_value(flags, "--validators")
                .unwrap_or("5")
                .parse::<u32>()
                .map_err(|_| "--validators must be a u32".to_string())?;
            let spending_key_seed_hex = flag_value(flags, "--spending-key-seed-hex")
                .unwrap_or("000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f");
            let value = flag_value(flags, "--value")
                .unwrap_or("7")
                .parse::<u64>()
                .map_err(|_| "--value must be a u64".to_string())?;
            let fee = flag_value(flags, "--fee")
                .unwrap_or("2")
                .parse::<u64>()
                .map_err(|_| "--fee must be a u64".to_string())?;
            let build_seed_hex = flag_value(flags, "--build-seed-hex")
                .unwrap_or("101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f");
            let proof_seed_hex = flag_value(flags, "--proof-seed-hex")
                .unwrap_or("303132333435363738393a3b3c3d3e3f404142434445464748494a4b4c4d4e4f");
            let signature_seed_hex = flag_value(flags, "--signature-seed-hex")
                .unwrap_or("4f4e4d4c4b4a494847464544434241403f3e3d3c3b3a39383736353433323130");
            let default_external_binding_hash = "000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f202122232425262728292a2b2c2d2e2f";
            let external_binding_hash = if flag_present(flags, "--no-external-binding") {
                None
            } else {
                Some(
                    flag_value(flags, "--external-binding-hash")
                        .unwrap_or(default_external_binding_hash)
                        .to_string(),
                )
            };
            let report = orchard_test_vector(OrchardTestVectorOptions {
                chain_id: chain_id.to_string(),
                validator_count,
                spending_key_seed_hex: spending_key_seed_hex.to_string(),
                value,
                fee,
                build_seed_hex: build_seed_hex.to_string(),
                proof_seed_hex: proof_seed_hex.to_string(),
                signature_seed_hex: signature_seed_hex.to_string(),
                external_binding_hash,
            })
            .map_err(|error| format!("orchard-test-vector failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("orchard-test-vector serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transfer" => {
            require_direct_state_enabled("transfer")?;
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let receipt = transfer(TransferOptions {
                data_dir: PathBuf::from(data_dir),
                key_file,
                to: to.to_string(),
                amount,
            })
            .map_err(|error| format!("transfer failed: {error}"))?;
            let json = serde_json::to_string_pretty(&receipt)
                .map_err(|error| format!("receipt serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "batch-transfer" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let batch = create_transfer_batch(BatchTransferOptions {
                data_dir: PathBuf::from(data_dir),
                key_file,
                to: to.to_string(),
                amount,
                batch_file: PathBuf::from(batch_file),
            })
            .map_err(|error| format!("batch-transfer failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "mempool-submit-transfer" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let amount = flag_value(flags, "--amount")
                .ok_or("missing --amount")?
                .parse::<u64>()
                .map_err(|_| "--amount must be a u64".to_string())?;
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let entry = submit_transfer_to_mempool(TransferOptions {
                data_dir: PathBuf::from(data_dir),
                key_file,
                to: to.to_string(),
                amount,
            })
            .map_err(|error| format!("mempool-submit-transfer failed: {error}"))?;
            let json = serde_json::to_string_pretty(&entry)
                .map_err(|error| format!("mempool entry serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "mempool-submit-signed-transfer" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let transfer_file =
                flag_value(flags, "--transfer-file").ok_or("missing --transfer-file")?;
            let entry = submit_signed_transfer_to_mempool(SignedTransferSubmitOptions {
                data_dir: PathBuf::from(data_dir),
                transfer_file: PathBuf::from(transfer_file),
            })
            .map_err(|error| format!("mempool-submit-signed-transfer failed: {error}"))?;
            let json = serde_json::to_string_pretty(&entry)
                .map_err(|error| format!("mempool entry serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "mempool-submit-signed-payment-v2" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let signed_payment_v2_json = flag_value(flags, "--signed-payment-v2-json")
                .ok_or("missing --signed-payment-v2-json")?;
            let entry =
                submit_signed_payment_v2_json_to_mempool(SignedPaymentV2JsonSubmitOptions {
                    data_dir: PathBuf::from(data_dir),
                    signed_payment_v2_json: signed_payment_v2_json.to_string(),
                })
                .map_err(|error| format!("mempool-submit-signed-payment-v2 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&entry)
                .map_err(|error| format!("mempool entry serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "mempool-submit-signed-asset-transaction" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let signed_asset_transaction_json =
                flag_value(flags, "--signed-asset-transaction-json")
                    .ok_or("missing --signed-asset-transaction-json")?;
            let entry = submit_signed_asset_transaction_json_to_mempool(
                SignedAssetTransactionJsonSubmitOptions {
                    data_dir: PathBuf::from(data_dir),
                    signed_asset_transaction_json: signed_asset_transaction_json.to_string(),
                },
            )
            .map_err(|error| format!("mempool-submit-signed-asset-transaction failed: {error}"))?;
            let json = serde_json::to_string_pretty(&entry)
                .map_err(|error| format!("mempool entry serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "mempool-submit-signed-escrow-transaction" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let signed_escrow_transaction_json =
                flag_value(flags, "--signed-escrow-transaction-json")
                    .ok_or("missing --signed-escrow-transaction-json")?;
            let entry = submit_signed_escrow_transaction_json_to_mempool(
                SignedEscrowTransactionJsonSubmitOptions {
                    data_dir: PathBuf::from(data_dir),
                    signed_escrow_transaction_json: signed_escrow_transaction_json.to_string(),
                },
            )
            .map_err(|error| format!("mempool-submit-signed-escrow-transaction failed: {error}"))?;
            let json = serde_json::to_string_pretty(&entry)
                .map_err(|error| format!("mempool entry serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "mempool-submit-signed-nft-transaction" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let signed_nft_transaction_json = flag_value(flags, "--signed-nft-transaction-json")
                .ok_or("missing --signed-nft-transaction-json")?;
            let entry = submit_signed_nft_transaction_json_to_mempool(
                SignedNftTransactionJsonSubmitOptions {
                    data_dir: PathBuf::from(data_dir),
                    signed_nft_transaction_json: signed_nft_transaction_json.to_string(),
                },
            )
            .map_err(|error| format!("mempool-submit-signed-nft-transaction failed: {error}"))?;
            let json = serde_json::to_string_pretty(&entry)
                .map_err(|error| format!("mempool entry serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "mempool-submit-signed-offer-transaction" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let signed_offer_transaction_json =
                flag_value(flags, "--signed-offer-transaction-json")
                    .ok_or("missing --signed-offer-transaction-json")?;
            let entry = submit_signed_offer_transaction_json_to_mempool(
                SignedOfferTransactionJsonSubmitOptions {
                    data_dir: PathBuf::from(data_dir),
                    signed_offer_transaction_json: signed_offer_transaction_json.to_string(),
                },
            )
            .map_err(|error| format!("mempool-submit-signed-offer-transaction failed: {error}"))?;
            let json = serde_json::to_string_pretty(&entry)
                .map_err(|error| format!("mempool entry serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "mempool-batch" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let max_transactions = flag_value(flags, "--max-transactions")
                .unwrap_or("100")
                .parse::<usize>()
                .map_err(|_| "--max-transactions must be a usize".to_string())?;
            let batch = create_mempool_batch(MempoolBatchOptions {
                data_dir: PathBuf::from(data_dir),
                batch_file: PathBuf::from(batch_file),
                max_transactions,
            })
            .map_err(|error| format!("mempool-batch failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "signed-asset-batch" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let signed_files = flag_value(flags, "--signed-asset-transaction-files")
                .ok_or("missing --signed-asset-transaction-files")?
                .split(',')
                .filter(|entry| !entry.trim().is_empty())
                .map(|entry| PathBuf::from(entry.trim()))
                .collect::<Vec<_>>();
            let batch = create_signed_asset_transaction_batch(SignedAssetTransactionBatchOptions {
                data_dir: PathBuf::from(data_dir),
                batch_file: PathBuf::from(batch_file),
                signed_asset_transaction_files: signed_files,
            })
            .map_err(|error| format!("signed-asset-batch failed: {error}"))?;
            let json = serde_json::to_string_pretty(&batch)
                .map_err(|error| format!("signed asset batch serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "mempool-status" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let mempool = mempool_state(NodeOptions {
                data_dir: PathBuf::from(data_dir),
            })
            .map_err(|error| format!("mempool-status failed: {error}"))?;
            let json = serde_json::to_string_pretty(&mempool)
                .map_err(|error| format!("mempool serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "propose-batch" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let proposal_file =
                flag_value(flags, "--proposal-file").ok_or("missing --proposal-file")?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let key_file = flag_value(flags, "--key-file").map(PathBuf::from);
            let validator_id = flag_value(flags, "--validator").map(str::to_string);
            let proposal = propose_batch(BatchProposalOptions {
                data_dir: PathBuf::from(data_dir),
                verify_block_log: !flag_present(flags, "--skip-block-log-verify"),
                batch_kind,
                batch_file: PathBuf::from(batch_file),
                proposal_file: PathBuf::from(proposal_file),
                view,
                timeout_certificate_file,
                key_file,
                validator_id,
            })
            .map_err(|error| format!("propose-batch failed: {error}"))?;
            let json = serde_json::to_string_pretty(&proposal)
                .map_err(|error| format!("block proposal serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "apply-batch" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let certificate_file = flag_value(flags, "--certificate-file").map(PathBuf::from);
            let receipts = apply_batch(ApplyBatchOptions {
                data_dir: PathBuf::from(data_dir),
                batch_file: PathBuf::from(batch_file),
                certificate_file,
            })
            .map_err(|error| format!("apply-batch failed: {error}"))?;
            let json = serde_json::to_string_pretty(&receipts)
                .map_err(|error| format!("receipt serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "ratify-validator-set" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let validator_count = flag_value(flags, "--validator-count")
                .ok_or("missing --validator-count")?
                .parse::<u32>()
                .map_err(|_| "--validator-count must be a u32".to_string())?;
            let activation_height = flag_value(flags, "--activation-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let amendment = ratify_validator_set(RatifyValidatorSetOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                support,
                validator_count,
                activation_height,
                veto_until_height,
                paused: flag_present(flags, "--paused"),
                amendment_file: PathBuf::from(amendment_file),
            })
            .map_err(|error| format!("ratify-validator-set failed: {error}"))?;
            let json = serde_json::to_string_pretty(&amendment)
                .map_err(|error| format!("amendment serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "ratify-crypto-policy" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let value = flag_value(flags, "--version")
                .ok_or("missing --version")?
                .parse::<u32>()
                .map_err(|_| "--version must be a u32".to_string())?;
            let activation_height = flag_value(flags, "--activation-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let amendment = ratify_governance(RatifyGovernanceOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                support,
                kind: GOVERNANCE_KIND_CRYPTO_POLICY.to_string(),
                value,
                activation_height,
                veto_until_height,
                paused: flag_present(flags, "--paused"),
                amendment_file: PathBuf::from(amendment_file),
            })
            .map_err(|error| format!("ratify-crypto-policy failed: {error}"))?;
            let json = serde_json::to_string_pretty(&amendment)
                .map_err(|error| format!("amendment serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "ratify-bridge-witness-epoch" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let value = flag_value(flags, "--witness-epoch")
                .ok_or("missing --witness-epoch")?
                .parse::<u32>()
                .map_err(|_| "--witness-epoch must be a u32".to_string())?;
            let activation_height = flag_value(flags, "--activation-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let amendment = ratify_governance(RatifyGovernanceOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                support,
                kind: GOVERNANCE_KIND_BRIDGE_WITNESS_EPOCH.to_string(),
                value,
                activation_height,
                veto_until_height,
                paused: flag_present(flags, "--paused"),
                amendment_file: PathBuf::from(amendment_file),
            })
            .map_err(|error| format!("ratify-bridge-witness-epoch failed: {error}"))?;
            let json = serde_json::to_string_pretty(&amendment)
                .map_err(|error| format!("amendment serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "ratify-authority-mode" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let mode = flag_value(flags, "--mode")
                .or_else(|| flag_value(flags, "--authority-mode"))
                .ok_or("missing --mode")?;
            let value = match mode {
                "0" | "foundation" | "foundation-retained" => GOVERNANCE_AUTHORITY_MODE_FOUNDATION,
                "1" | "cobalt" | "cobalt-ratified" | "cobalt-ratified-governance" => {
                    GOVERNANCE_AUTHORITY_MODE_COBALT_RATIFIED
                }
                _ => {
                    return Err(
                        "--mode must be foundation|foundation-retained|0|cobalt|cobalt-ratified|1"
                            .to_string(),
                    );
                }
            };
            let activation_height = flag_value(flags, "--activation-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let amendment = ratify_governance(RatifyGovernanceOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                support,
                kind: GOVERNANCE_KIND_AUTHORITY_MODE.to_string(),
                value,
                activation_height,
                veto_until_height,
                paused: flag_present(flags, "--paused"),
                amendment_file: PathBuf::from(amendment_file),
            })
            .map_err(|error| format!("ratify-authority-mode failed: {error}"))?;
            let json = serde_json::to_string_pretty(&amendment)
                .map_err(|error| format!("amendment serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "ratify-orchard-pool-pause" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let value = match flag_value(flags, "--state").ok_or("missing --state")? {
                "paused" => 1,
                "resumed" => 0,
                _ => return Err("--state must be paused|resumed".to_string()),
            };
            let activation_height = flag_value(flags, "--activation-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let amendment = ratify_governance(RatifyGovernanceOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                support,
                kind: GOVERNANCE_KIND_ORCHARD_POOL_PAUSE.to_string(),
                value,
                activation_height,
                veto_until_height,
                paused: flag_present(flags, "--paused"),
                amendment_file: PathBuf::from(amendment_file),
            })
            .map_err(|error| format!("ratify-orchard-pool-pause failed: {error}"))?;
            let json = serde_json::to_string_pretty(&amendment)
                .map_err(|error| format!("amendment serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "ratify-atomic-swap-pause" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let value = match flag_value(flags, "--state").ok_or("missing --state")? {
                "paused" => 1,
                "resumed" => 0,
                _ => return Err("--state must be paused|resumed".to_string()),
            };
            let activation_height = flag_value(flags, "--activation-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let amendment = ratify_governance(RatifyGovernanceOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                support,
                kind: GOVERNANCE_KIND_ATOMIC_SWAP_PAUSE.to_string(),
                value,
                activation_height,
                veto_until_height,
                paused: flag_present(flags, "--paused"),
                amendment_file: PathBuf::from(amendment_file),
            })
            .map_err(|error| format!("ratify-atomic-swap-pause failed: {error}"))?;
            let json = serde_json::to_string_pretty(&amendment)
                .map_err(|error| format!("amendment serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "ratify-bridge-verification-activation-height" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let value = flag_value(flags, "--height")
                .or_else(|| flag_value(flags, "--bridge-verification-activation-height"))
                .ok_or("missing --height")?
                .parse::<u32>()
                .map_err(|_| "--height must be a u32".to_string())?;
            let activation_height = flag_value(flags, "--activation-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let amendment = ratify_governance(RatifyGovernanceOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                support,
                kind: GOVERNANCE_KIND_BRIDGE_VERIFICATION_ACTIVATION_HEIGHT.to_string(),
                value,
                activation_height,
                veto_until_height,
                paused: flag_present(flags, "--paused"),
                amendment_file: PathBuf::from(amendment_file),
            })
            .map_err(|error| {
                format!("ratify-bridge-verification-activation-height failed: {error}")
            })?;
            let json = serde_json::to_string_pretty(&amendment)
                .map_err(|error| format!("amendment serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "ratify-atomic-swap-activation-height"
        | "ratify-replicated-state-v2-activation-height"
        | "ratify-bridge-exit-root-activation-height"
        | "ratify-vault-bridge-route-authority-activation-height" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let (kind, command_specific_height_flag) = match command {
                "ratify-atomic-swap-activation-height" => (
                    GOVERNANCE_KIND_ATOMIC_SWAP_ACTIVATION_HEIGHT,
                    "--atomic-swap-activation-height",
                ),
                "ratify-replicated-state-v2-activation-height" => (
                    GOVERNANCE_KIND_REPLICATED_STATE_V2_ACTIVATION_HEIGHT,
                    "--replicated-state-v2-activation-height",
                ),
                "ratify-bridge-exit-root-activation-height" => (
                    GOVERNANCE_KIND_BRIDGE_EXIT_ROOT_ACTIVATION_HEIGHT,
                    "--bridge-exit-root-activation-height",
                ),
                _ => (
                    GOVERNANCE_KIND_VAULT_BRIDGE_ROUTE_AUTHORITY_ACTIVATION_HEIGHT,
                    "--vault-bridge-route-authority-activation-height",
                ),
            };
            let value = flag_value(flags, "--height")
                .or_else(|| flag_value(flags, command_specific_height_flag))
                .ok_or("missing --height")?
                .parse::<u32>()
                .map_err(|_| "--height must be a u32".to_string())?;
            let activation_height = flag_value(flags, "--activation-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let veto_until_height = flag_value(flags, "--veto-until-height")
                .unwrap_or("0")
                .parse::<u64>()
                .map_err(|_| "--veto-until-height must be a u64".to_string())?;
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let amendment = ratify_governance(RatifyGovernanceOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                support,
                kind: kind.to_string(),
                value,
                activation_height,
                veto_until_height,
                paused: flag_present(flags, "--paused"),
                amendment_file: PathBuf::from(amendment_file),
            })
            .map_err(|error| format!("{command} failed: {error}"))?;
            let json = serde_json::to_string_pretty(&amendment)
                .map_err(|error| format!("amendment serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "governance-authorization-sign" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let validator = flag_value(flags, "--validator").ok_or("missing --validator")?;
            let validator_key_file =
                flag_value(flags, "--validator-key-file").ok_or("missing --validator-key-file")?;
            let proposal_slot = flag_value(flags, "--proposal-slot")
                .ok_or("missing --proposal-slot")?
                .parse::<u64>()
                .map_err(|_| "--proposal-slot must be a u64".to_string())?;
            let expires_at_height = flag_value(flags, "--expires-at-height")
                .ok_or("missing --expires-at-height")?
                .parse::<u64>()
                .map_err(|_| "--expires-at-height must be a u64".to_string())?;
            let authorization_file =
                flag_value(flags, "--authorization-file").ok_or("missing --authorization-file")?;
            let authorization =
                sign_governance_amendment_authorization(GovernanceAuthorizationSignOptions {
                    data_dir: PathBuf::from(data_dir),
                    amendment_file: PathBuf::from(amendment_file),
                    validator: validator.to_string(),
                    validator_key_file: PathBuf::from(validator_key_file),
                    proposal_slot,
                    expires_at_height,
                    authorization_file: PathBuf::from(authorization_file),
                })
                .map_err(|error| format!("governance-authorization-sign failed: {error}"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&authorization)
                    .map_err(|error| format!("authorization serialization failed: {error}"))?
            );
            Ok(())
        }
        "ethereum-checkpoint-observe" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let ethereum_rpc =
                flag_value(flags, "--ethereum-rpc").ok_or("missing --ethereum-rpc")?;
            let block_number = flag_value(flags, "--block-number")
                .map(|value| value.parse::<u64>().map_err(|_| "invalid --block-number"))
                .transpose()?;
            let checkpoint_file =
                flag_value(flags, "--checkpoint-file").ok_or("missing --checkpoint-file")?;
            let checkpoint = observe_ethereum_checkpoint(EthereumCheckpointObserveOptions {
                data_dir: PathBuf::from(data_dir),
                route_id: route_id.to_string(),
                ethereum_rpc: ethereum_rpc.to_string(),
                block_number,
                checkpoint_file: PathBuf::from(checkpoint_file),
            })
            .map_err(|error| format!("ethereum-checkpoint-observe failed: {error}"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&checkpoint).map_err(|error| {
                    format!("Ethereum checkpoint serialization failed: {error}")
                })?
            );
            Ok(())
        }
        "ethereum-receipt-proof-build" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let route_id = flag_value(flags, "--route-id").ok_or("missing --route-id")?;
            let ethereum_rpc =
                flag_value(flags, "--ethereum-rpc").ok_or("missing --ethereum-rpc")?;
            let transaction_hash =
                flag_value(flags, "--transaction-hash").ok_or("missing --transaction-hash")?;
            let proof_file = flag_value(flags, "--proof-file").ok_or("missing --proof-file")?;
            let artifact = build_ethereum_receipt_proof(EthereumReceiptProofBuildOptions {
                data_dir: PathBuf::from(data_dir),
                route_id: route_id.to_string(),
                ethereum_rpc: ethereum_rpc.to_string(),
                transaction_hash: transaction_hash.to_string(),
                proof_file: PathBuf::from(proof_file),
            })
            .map_err(|error| format!("ethereum-receipt-proof-build failed: {error}"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&artifact).map_err(|error| {
                    format!("Ethereum receipt-proof serialization failed: {error}")
                })?
            );
            Ok(())
        }
        "ethereum-checkpoint-vote-sign" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let checkpoint_file =
                flag_value(flags, "--checkpoint-file").ok_or("missing --checkpoint-file")?;
            let ethereum_rpc =
                flag_value(flags, "--ethereum-rpc").ok_or("missing --ethereum-rpc")?;
            let validator = flag_value(flags, "--validator").ok_or("missing --validator")?;
            let validator_key_file =
                flag_value(flags, "--validator-key-file").ok_or("missing --validator-key-file")?;
            let vote_file = flag_value(flags, "--vote-file").ok_or("missing --vote-file")?;
            let vote = sign_ethereum_checkpoint_vote(EthereumCheckpointVoteSignOptions {
                data_dir: PathBuf::from(data_dir),
                checkpoint_file: PathBuf::from(checkpoint_file),
                ethereum_rpc: ethereum_rpc.to_string(),
                validator: validator.to_string(),
                validator_key_file: PathBuf::from(validator_key_file),
                vote_file: PathBuf::from(vote_file),
            })
            .map_err(|error| format!("ethereum-checkpoint-vote-sign failed: {error}"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&vote)
                    .map_err(|error| format!("checkpoint vote serialization failed: {error}"))?
            );
            Ok(())
        }
        "ethereum-checkpoint-certificate-assemble" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let checkpoint_file =
                flag_value(flags, "--checkpoint-file").ok_or("missing --checkpoint-file")?;
            let vote_files =
                split_csv(flag_value(flags, "--vote-files").ok_or("missing --vote-files")?)
                    .into_iter()
                    .map(PathBuf::from)
                    .collect::<Vec<_>>();
            let certificate_file =
                flag_value(flags, "--certificate-file").ok_or("missing --certificate-file")?;
            let certificate = assemble_ethereum_checkpoint_certificate(
                EthereumCheckpointCertificateAssembleOptions {
                    data_dir: PathBuf::from(data_dir),
                    checkpoint_file: PathBuf::from(checkpoint_file),
                    vote_files,
                    certificate_file: PathBuf::from(certificate_file),
                },
            )
            .map_err(|error| format!("ethereum-checkpoint-certificate-assemble failed: {error}"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&certificate).map_err(|error| {
                    format!("checkpoint certificate serialization failed: {error}")
                })?
            );
            Ok(())
        }
        "governance-amendment-assemble" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let amendment_file =
                flag_value(flags, "--amendment-file").ok_or("missing --amendment-file")?;
            let authorization_files = split_csv(
                flag_value(flags, "--authorization-files")
                    .ok_or("missing --authorization-files")?,
            )
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
            let proposal_slot = flag_value(flags, "--proposal-slot")
                .ok_or("missing --proposal-slot")?
                .parse::<u64>()
                .map_err(|_| "--proposal-slot must be a u64".to_string())?;
            let output_file = flag_value(flags, "--output").ok_or("missing --output")?;
            let amendment =
                assemble_signed_governance_amendment(GovernanceAmendmentAssembleOptions {
                    data_dir: PathBuf::from(data_dir),
                    amendment_file: PathBuf::from(amendment_file),
                    authorization_files,
                    proposal_slot,
                    output_file: PathBuf::from(output_file),
                })
                .map_err(|error| format!("governance-amendment-assemble failed: {error}"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&amendment)
                    .map_err(|error| format!("amendment serialization failed: {error}"))?
            );
            Ok(())
        }
        "validator-registry-root" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let registry_file = flag_value(flags, "--registry-file").map(PathBuf::from);
            let report = validator_registry_root_report(ValidatorRegistryRootOptions {
                data_dir: PathBuf::from(data_dir),
                registry_file,
                validators,
            })
            .map_err(|error| format!("validator-registry-root failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("registry root serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "validator-registry-update" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let validators =
                split_csv(flag_value(flags, "--validators").ok_or("missing --validators")?);
            let support = split_csv(flag_value(flags, "--support").unwrap_or(""));
            let support = if support.is_empty() {
                validators.clone()
            } else {
                support
            };
            let activation_height = flag_value(flags, "--activation-height")
                .ok_or("missing --activation-height")?
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let previous_registry_root = flag_value(flags, "--previous-registry-root")
                .ok_or("missing --previous-registry-root")?
                .to_string();
            let new_registry_root = flag_value(flags, "--new-registry-root")
                .ok_or("missing --new-registry-root")?
                .to_string();
            let previous_validators = split_csv(
                flag_value(flags, "--previous-validators")
                    .unwrap_or_else(|| flag_value(flags, "--validators").unwrap_or("")),
            );
            let previous_validators = if previous_validators.is_empty() {
                validators.clone()
            } else {
                previous_validators
            };
            let new_validators = split_csv(
                flag_value(flags, "--new-validators")
                    .unwrap_or_else(|| flag_value(flags, "--validators").unwrap_or("")),
            );
            let new_validators = if new_validators.is_empty() {
                validators.clone()
            } else {
                new_validators
            };
            let operation = flag_value(flags, "--operation")
                .ok_or("missing --operation")?
                .to_string();
            let subject_node_id = flag_value(flags, "--subject-node-id")
                .ok_or("missing --subject-node-id")?
                .to_string();
            let previous_record_file =
                flag_value(flags, "--previous-record-file").map(PathBuf::from);
            let new_record_file = flag_value(flags, "--new-record-file").map(PathBuf::from);
            let update_file = flag_value(flags, "--update-file").ok_or("missing --update-file")?;
            let update = create_validator_registry_update(ValidatorRegistryUpdateOptions {
                data_dir: PathBuf::from(data_dir),
                validators,
                support,
                activation_height,
                previous_registry_root,
                new_registry_root,
                previous_validators,
                new_validators,
                operation,
                subject_node_id,
                previous_record_file,
                new_record_file,
                update_file: PathBuf::from(update_file),
            })
            .map_err(|error| format!("validator-registry-update failed: {error}"))?;
            let json = serde_json::to_string_pretty(&update)
                .map_err(|error| format!("registry update serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "validator-registry-authorization-sign" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let update_file = flag_value(flags, "--update-file").ok_or("missing --update-file")?;
            let validator = flag_value(flags, "--validator").ok_or("missing --validator")?;
            let validator_key_file =
                flag_value(flags, "--validator-key-file").ok_or("missing --validator-key-file")?;
            let proposal_slot = flag_value(flags, "--proposal-slot")
                .ok_or("missing --proposal-slot")?
                .parse::<u64>()
                .map_err(|_| "--proposal-slot must be a u64".to_string())?;
            let expires_at_height = flag_value(flags, "--expires-at-height")
                .ok_or("missing --expires-at-height")?
                .parse::<u64>()
                .map_err(|_| "--expires-at-height must be a u64".to_string())?;
            let authorization_file =
                flag_value(flags, "--authorization-file").ok_or("missing --authorization-file")?;
            let authorization = sign_validator_registry_update_authorization(
                ValidatorRegistryAuthorizationSignOptions {
                    data_dir: PathBuf::from(data_dir),
                    update_file: PathBuf::from(update_file),
                    validator: validator.to_string(),
                    validator_key_file: PathBuf::from(validator_key_file),
                    proposal_slot,
                    expires_at_height,
                    authorization_file: PathBuf::from(authorization_file),
                },
            )
            .map_err(|error| format!("validator-registry-authorization-sign failed: {error}"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&authorization)
                    .map_err(|error| format!("authorization serialization failed: {error}"))?
            );
            Ok(())
        }
        "validator-registry-update-assemble" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let update_file = flag_value(flags, "--update-file").ok_or("missing --update-file")?;
            let authorization_files = split_csv(
                flag_value(flags, "--authorization-files")
                    .ok_or("missing --authorization-files")?,
            )
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
            let proposal_slot = flag_value(flags, "--proposal-slot")
                .ok_or("missing --proposal-slot")?
                .parse::<u64>()
                .map_err(|_| "--proposal-slot must be a u64".to_string())?;
            let output_file = flag_value(flags, "--output").ok_or("missing --output")?;
            let update =
                assemble_signed_validator_registry_update(ValidatorRegistryUpdateAssembleOptions {
                    data_dir: PathBuf::from(data_dir),
                    update_file: PathBuf::from(update_file),
                    authorization_files,
                    proposal_slot,
                    output_file: PathBuf::from(output_file),
                })
                .map_err(|error| format!("validator-registry-update-assemble failed: {error}"))?;
            println!(
                "{}",
                serde_json::to_string_pretty(&update)
                    .map_err(|error| format!("registry update serialization failed: {error}"))?
            );
            Ok(())
        }
        "validator-registry-update-verify" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let update_file = flag_value(flags, "--update-file").ok_or("missing --update-file")?;
            let previous_registry_file =
                flag_value(flags, "--previous-registry-file").map(PathBuf::from);
            let new_registry_file = flag_value(flags, "--new-registry-file").map(PathBuf::from);
            let report =
                verify_validator_registry_update_file(ValidatorRegistryUpdateVerifyOptions {
                    data_dir: PathBuf::from(data_dir),
                    update_file: PathBuf::from(update_file),
                    previous_registry_file,
                    new_registry_file,
                })
                .map_err(|error| format!("validator-registry-update-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("registry update verification serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "validator-registry-update-apply" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let update_file = flag_value(flags, "--update-file").ok_or("missing --update-file")?;
            let current_height = flag_value(flags, "--current-height")
                .ok_or("missing --current-height")?
                .parse::<u64>()
                .map_err(|_| "--current-height must be a u64".to_string())?;
            let previous_registry_file =
                flag_value(flags, "--previous-registry-file").map(PathBuf::from);
            let output_registry_file =
                flag_value(flags, "--output-registry-file").map(PathBuf::from);
            let report = apply_validator_registry_update(ValidatorRegistryUpdateApplyOptions {
                data_dir: PathBuf::from(data_dir),
                update_file: PathBuf::from(update_file),
                current_height,
                previous_registry_file,
                output_registry_file,
            })
            .map_err(|error| format!("validator-registry-update-apply failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("registry update apply serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "validator-registry-lifecycle-replay-verify" => {
            let bundle_file = flag_value(flags, "--bundle-file").ok_or("missing --bundle-file")?;
            let report = verify_validator_registry_lifecycle_replay_bundle(
                ValidatorRegistryLifecycleReplayVerifyOptions {
                    bundle_file: PathBuf::from(bundle_file),
                },
            )
            .map_err(|error| {
                format!("validator-registry-lifecycle-replay-verify failed: {error}")
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("validator registry lifecycle replay serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "governance-replay-verify" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let package_file =
                flag_value(flags, "--package-file").ok_or("missing --package-file")?;
            let report = verify_governance_replay_package(GovernanceReplayVerifyOptions {
                data_dir: PathBuf::from(data_dir),
                package_file: PathBuf::from(package_file),
            })
            .map_err(|error| format!("governance-replay-verify failed: {error}"))?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("governance replay verification serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        _ => unreachable!("run_cli_group_03 dispatch mismatch"),
    }
}
