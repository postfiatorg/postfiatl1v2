fn run_cli_group_01(command: &str, flags: &[String]) -> Result<(), String> {
    match command {
        "init" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let node_id = flag_value(flags, "--node-id").unwrap_or(DEFAULT_NODE_ID);
            let validator_count = flag_value(flags, "--validators")
                .unwrap_or("1")
                .parse::<u32>()
                .map_err(|_| "--validators must be a u32".to_string())?;
            let report = init(InitOptions {
                data_dir: PathBuf::from(data_dir),
                chain_id: chain_id.to_string(),
                node_id: node_id.to_string(),
                validator_count,
            })
            .map_err(|error| format!("init failed: {error}"))?;
            let json = report
                .to_json()
                .map_err(|error| format!("init report serialization failed: {error}"))?;
            print!("{json}");
            Ok(())
        }
        "init-consensus-v2" => {
            let data_dir = flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR);
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let node_id = flag_value(flags, "--node-id").unwrap_or(DEFAULT_NODE_ID);
            let validator_count = flag_value(flags, "--validators")
                .unwrap_or("1")
                .parse::<u32>()
                .map_err(|_| "--validators must be a u32".to_string())?;
            let activation_height = flag_value(flags, "--activation-height")
                .ok_or("missing --activation-height")?
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let report = init_consensus_v2(InitConsensusV2Options {
                data_dir: PathBuf::from(data_dir),
                chain_id: chain_id.to_string(),
                node_id: node_id.to_string(),
                validator_count,
                activation_height,
            })
            .map_err(|error| format!("init-consensus-v2 failed: {error}"))?;
            let json = report
                .to_json()
                .map_err(|error| format!("init report serialization failed: {error}"))?;
            print!("{json}");
            Ok(())
        }
        "topology" => {
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let hosts = flag_value(flags, "--hosts")
                .map(parse_csv_values)
                .transpose()?;
            let validators = match (flag_value(flags, "--validators"), hosts.as_ref()) {
                (Some(value), _) => value
                    .parse::<u32>()
                    .map_err(|_| "--validators must be a u32".to_string())?,
                (None, Some(hosts)) => u32::try_from(hosts.len())
                    .map_err(|_| "--hosts contains too many validators".to_string())?,
                (None, None) => 4,
            };
            let base_port = flag_value(flags, "--base-port")
                .map(|value| {
                    value
                        .parse::<u16>()
                        .map_err(|_| "--base-port must be a u16".to_string())
                })
                .transpose()?
                .unwrap_or(DEFAULT_BASE_PORT);
            let rpc_base_port = flag_value(flags, "--rpc-base-port")
                .map(|value| {
                    value
                        .parse::<u16>()
                        .map_err(|_| "--rpc-base-port must be a u16".to_string())
                })
                .transpose()?;
            let output_file = flag_value(flags, "--output").unwrap_or(DEFAULT_TOPOLOGY_FILE);
            let topology = write_local_topology(TopologyOptions {
                chain_id: chain_id.to_string(),
                validators,
                base_port,
                rpc_base_port,
                hosts,
                output_file: PathBuf::from(output_file),
            })
            .map_err(|error| format!("topology failed: {error}"))?;
            let json = serde_json::to_string_pretty(&topology)
                .map_err(|error| format!("topology serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "topology-consensus-v2" => {
            let chain_id = flag_value(flags, "--chain-id").unwrap_or(DEFAULT_CHAIN_ID);
            let hosts = flag_value(flags, "--hosts")
                .map(parse_csv_values)
                .transpose()?;
            let validators = match (flag_value(flags, "--validators"), hosts.as_ref()) {
                (Some(value), _) => value
                    .parse::<u32>()
                    .map_err(|_| "--validators must be a u32".to_string())?,
                (None, Some(hosts)) => u32::try_from(hosts.len())
                    .map_err(|_| "--hosts contains too many validators".to_string())?,
                (None, None) => 4,
            };
            let base_port = flag_value(flags, "--base-port")
                .map(|value| {
                    value
                        .parse::<u16>()
                        .map_err(|_| "--base-port must be a u16".to_string())
                })
                .transpose()?
                .unwrap_or(DEFAULT_BASE_PORT);
            let rpc_base_port = flag_value(flags, "--rpc-base-port")
                .map(|value| {
                    value
                        .parse::<u16>()
                        .map_err(|_| "--rpc-base-port must be a u16".to_string())
                })
                .transpose()?;
            let activation_height = flag_value(flags, "--activation-height")
                .ok_or("missing --activation-height")?
                .parse::<u64>()
                .map_err(|_| "--activation-height must be a u64".to_string())?;
            let output_file = flag_value(flags, "--output").unwrap_or(DEFAULT_TOPOLOGY_FILE);
            let topology = write_consensus_v2_topology(TopologyConsensusV2Options {
                chain_id: chain_id.to_string(),
                validators,
                base_port,
                rpc_base_port,
                hosts,
                output_file: PathBuf::from(output_file),
                activation_height,
            })
            .map_err(|error| format!("topology-consensus-v2 failed: {error}"))?;
            let json = serde_json::to_string_pretty(&topology)
                .map_err(|error| format!("topology serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-listen" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let max_peers = flag_value(flags, "--max-peers")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-peers must be a usize".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let bind_host = flag_value(flags, "--bind-host").map(str::to_string);
            let report = transport_listen(
                data_dir,
                PathBuf::from(topology_file),
                bind_host,
                max_peers,
                timeout_ms,
            )?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transport listen serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-dial" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let report = transport_dial(
                data_dir,
                PathBuf::from(topology_file),
                to.to_string(),
                timeout_ms,
            )?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transport dial serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-batch-listen" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let max_peers = flag_value(flags, "--max-peers")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-peers must be a usize".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let bind_host = flag_value(flags, "--bind-host").map(str::to_string);
            let report = transport_batch_listen(
                data_dir,
                PathBuf::from(topology_file),
                bind_host,
                max_peers,
                timeout_ms,
            )?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transport batch listen serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-batch-serve" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let max_batches = flag_value(flags, "--max-batches")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-batches must be a usize".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let bind_host = flag_value(flags, "--bind-host").map(str::to_string);
            let event_log = flag_value(flags, "--event-log").map(PathBuf::from);
            let report = transport_batch_serve(
                data_dir,
                PathBuf::from(topology_file),
                bind_host,
                max_batches,
                timeout_ms,
                event_log,
            )?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transport batch serve serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-batch-send" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let certificate_file = flag_value(flags, "--certificate-file").map(PathBuf::from);
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
            let report = transport_batch_send_with_retries(
                data_dir,
                PathBuf::from(topology_file),
                to.to_string(),
                batch_kind,
                PathBuf::from(batch_file),
                certificate_file,
                timeout_ms,
                send_retries,
                retry_backoff_ms,
            )?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("transport batch send serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "transport-block-vote-listen" => {
            require_unsafe_devnet_file_signer(flags, "transport block-vote listener")?;
            require_unsafe_devnet_json_storage(flags, "transport block-vote listener")?;
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let vote_dir = flag_value(flags, "--vote-dir").ok_or("missing --vote-dir")?;
            let max_requests = flag_value(flags, "--max-requests")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-requests must be a usize".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let bind_host = flag_value(flags, "--bind-host").map(str::to_string);
            let require_signed_proposal = !flag_present(flags, "--allow-unsigned-proposal");
            let report = transport_block_vote_listen(
                data_dir,
                PathBuf::from(topology_file),
                PathBuf::from(key_file),
                PathBuf::from(vote_dir),
                bind_host,
                max_requests,
                timeout_ms,
                require_signed_proposal,
            )?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport block vote listen serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-validator-serve" => {
            require_unsafe_devnet_file_signer(flags, "transport validator service")?;
            require_unsafe_devnet_json_storage(flags, "transport validator service")?;
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let key_file = flag_value(flags, "--key-file").ok_or("missing --key-file")?;
            let vote_dir = flag_value(flags, "--vote-dir").ok_or("missing --vote-dir")?;
            let max_connections = flag_value(flags, "--max-connections")
                .unwrap_or("1")
                .parse::<usize>()
                .map_err(|_| "--max-connections must be a usize".to_string())?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let bind_host = flag_value(flags, "--bind-host").map(str::to_string);
            let event_log = flag_value(flags, "--event-log").map(PathBuf::from);
            let require_signed_proposal = !flag_present(flags, "--allow-unsigned-proposal");
            let report = transport_validator_serve(
                data_dir,
                PathBuf::from(topology_file),
                PathBuf::from(key_file),
                PathBuf::from(vote_dir),
                bind_host,
                max_connections,
                timeout_ms,
                event_log,
                require_signed_proposal,
            )?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport validator serve serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-block-vote-request" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let to = flag_value(flags, "--to").ok_or("missing --to")?;
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let proposal_file =
                flag_value(flags, "--proposal-file").ok_or("missing --proposal-file")?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
            let vote_file = flag_value(flags, "--vote-file").ok_or("missing --vote-file")?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .unwrap_or("5000")
                .parse::<u64>()
                .map_err(|_| "--timeout-ms must be a u64".to_string())?;
            let report = transport_block_vote_request(TransportBlockVoteRequestOptions {
                data_dir,
                topology_file: PathBuf::from(topology_file),
                to: to.to_string(),
                batch_kind,
                batch_file: PathBuf::from(batch_file),
                proposal_file: PathBuf::from(proposal_file),
                timeout_certificate_file,
                vote_file: PathBuf::from(vote_file),
                block_height,
                timeout_ms,
                consensus_v2: None,
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport block vote request serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-certified-batch-round" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
            let validator_key_dir =
                flag_value(flags, "--validator-key-dir").ok_or("missing --validator-key-dir")?;
            let artifact_dir =
                flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
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
            let skip_block_log_verify = flags.contains(&"--skip-block-log-verify".to_string());
            let report = transport_certified_batch_round(TransportCertifiedBatchRoundOptions {
                data_dir,
                topology_file: PathBuf::from(topology_file),
                batch_kind,
                batch_file: PathBuf::from(batch_file),
                validator_key_dir: PathBuf::from(validator_key_dir),
                artifact_dir: PathBuf::from(artifact_dir),
                block_height,
                view,
                timeout_certificate_file,
                timeout_ms,
                send_retries,
                retry_backoff_ms,
                skip_block_log_verify,
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport certified batch round serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-peer-certified-batch-round" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
            let batch_kind = flag_value(flags, "--batch-kind").map(str::to_string);
            let batch_file = flag_value(flags, "--batch-file").ok_or("missing --batch-file")?;
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
            let artifact_dir =
                flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
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
            let report =
                transport_peer_certified_batch_round(TransportPeerCertifiedBatchRoundOptions {
                    data_dir,
                    topology_file: PathBuf::from(topology_file),
                    batch_kind,
                    batch_file: PathBuf::from(batch_file),
                    key_file,
                    proposal_key_file,
                    require_local_proposer,
                    require_signed_proposal,
                    allow_peer_failures,
                    quorum_early_full_propagation,
                    artifact_dir: PathBuf::from(artifact_dir),
                    block_height,
                    view,
                    timeout_certificate_file,
                    timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    local_apply_before_certified_send,
                    defer_certified_sends,
                    required_parent: None,
                })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport peer certified batch round serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "transport-peer-certified-mempool-round" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file = flag_value(flags, "--topology").ok_or("missing --topology")?;
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
            let artifact_dir =
                flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
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
            let max_transactions = flag_value(flags, "--max-transactions")
                .unwrap_or("100")
                .parse::<usize>()
                .map_err(|_| "--max-transactions must be a usize".to_string())?;
            let signed_transfer_file = flag_value(flags, "--signed-transfer-file").map(PathBuf::from);
            let signed_transfer_json =
                flag_value(flags, "--signed-transfer-json").map(str::to_string);
            let signed_asset_transaction_json = flag_value(flags, "--signed-asset-transaction-json")
                .map(str::to_string);
            let report =
                transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
                    data_dir,
                    topology_file: PathBuf::from(topology_file),
                    key_file,
                    proposal_key_file,
                    require_local_proposer,
                    require_signed_proposal,
                    allow_peer_failures,
                    quorum_early_full_propagation,
                    artifact_dir: PathBuf::from(artifact_dir),
                    block_height,
                    view,
                    timeout_certificate_file,
                    timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    local_apply_before_certified_send,
                    defer_certified_sends,
                    max_transactions,
                    signed_transfer_file,
                    signed_transfer_json,
                    signed_payment_v2_json: None,
                    signed_asset_transaction_json,
                    signed_atomic_swap_transaction_json: None,
                    signed_escrow_transaction_json: None,
                    required_parent: None,
                })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("transport peer certified mempool round serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "pftl-submit-certified-asset-ops" | "submit-certified-asset-ops" => {
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file =
                PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
            let key_file =
                PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
            let proposal_key_file = flag_value(flags, "--proposal-key-file").map(PathBuf::from);
            let artifact_dir =
                PathBuf::from(flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?);
            let ops_file = match (flag_value(flags, "--ops-file"), flag_value(flags, "--bundle")) {
                (Some(_), Some(_)) => {
                    return Err("use only one of --ops-file or --bundle".to_string());
                }
                (Some(ops_file), None) => PathBuf::from(ops_file),
                (None, Some(bundle_dir)) => {
                    let ops_file = artifact_dir.with_extension("certified-ops.request.json");
                    certified_asset_ops_from_bundle(CertifiedAssetOpsFromBundleOptions {
                        bundle_dir: PathBuf::from(bundle_dir),
                        output_file: ops_file.clone(),
                        proposer_key_file: flag_value(flags, "--proposer-key-file")
                            .map(PathBuf::from),
                        attestor_key_file: flag_value(flags, "--attestor-key-file")
                            .map(PathBuf::from),
                        finalizer_key_file: flag_value(flags, "--finalizer-key-file")
                            .map(PathBuf::from),
                        claimer_key_file: flag_value(flags, "--claimer-key-file").map(PathBuf::from),
                        owner_key_file: flag_value(flags, "--owner-key-file").map(PathBuf::from),
                        include_deposit_claim: !flag_present(flags, "--skip-deposit-claim"),
                        overwrite: flag_present(flags, "--overwrite"),
                    })?;
                    ops_file
                }
                (None, None) => return Err("missing --ops-file or --bundle".to_string()),
            };
            let max_transactions = flag_value(flags, "--max-transactions")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--max-transactions must be a usize".to_string())
                })
                .transpose()?;
            let block_height = flag_value(flags, "--height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--height must be a u64".to_string())
                })
                .transpose()?;
            let view = flag_value(flags, "--view")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--view must be a u64".to_string())
                })
                .transpose()?;
            let timeout_certificate_file =
                flag_value(flags, "--timeout-certificate-file").map(PathBuf::from);
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
            let report = certified_asset_ops_batch(CertifiedAssetOpsBatchOptions {
                data_dir,
                topology_file,
                key_file,
                proposal_key_file,
                ops_file,
                artifact_dir,
                max_transactions,
                require_local_proposer: flag_present(flags, "--require-local-proposer"),
                require_signed_proposal: !flag_present(flags, "--allow-unsigned-proposal"),
                allow_peer_failures: flag_present(flags, "--allow-peer-failures"),
                quorum_early_full_propagation: flag_present(
                    flags,
                    "--quorum-early-full-propagation",
                ),
                local_apply_before_certified_send: flag_present(
                    flags,
                    "--local-apply-before-certified-send",
                ),
                defer_certified_sends: flag_present(flags, "--defer-certified-sends"),
                block_height,
                view,
                timeout_certificate_file,
                timeout_ms,
                send_retries,
                retry_backoff_ms,
                allow_existing_mempool: flag_present(flags, "--allow-existing-mempool"),
                resume: flag_present(flags, "--resume"),
                overwrite: flag_present(flags, "--overwrite"),
                prepare_only: flag_present(flags, "--prepare-only"),
                batch_only: flag_present(flags, "--batch-only"),
            })?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("certified asset ops serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        "pftl-certified-asset-ops-from-bundle" => {
            let bundle_dir =
                PathBuf::from(flag_value(flags, "--bundle").ok_or("missing --bundle")?);
            let output_file =
                PathBuf::from(flag_value(flags, "--output").ok_or("missing --output")?);
            let report = certified_asset_ops_from_bundle(CertifiedAssetOpsFromBundleOptions {
                bundle_dir,
                output_file,
                proposer_key_file: flag_value(flags, "--proposer-key-file").map(PathBuf::from),
                attestor_key_file: flag_value(flags, "--attestor-key-file").map(PathBuf::from),
                finalizer_key_file: flag_value(flags, "--finalizer-key-file").map(PathBuf::from),
                claimer_key_file: flag_value(flags, "--claimer-key-file").map(PathBuf::from),
                owner_key_file: flag_value(flags, "--owner-key-file").map(PathBuf::from),
                include_deposit_claim: !flag_present(flags, "--skip-deposit-claim"),
                overwrite: flag_present(flags, "--overwrite"),
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("certified asset ops bundle adapter serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "nav-roundtrip-dashboard-status" => {
            let report = nav_roundtrip_dashboard_status(NavRoundtripDashboardStatusOptions {
                summary_file: PathBuf::from(
                    flag_value(flags, "--summary").ok_or("missing --summary")?,
                ),
                report_file: flag_value(flags, "--report").map(PathBuf::from),
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("NAV roundtrip dashboard status serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "nav-roundtrip-benchmark-base-args" => {
            let timeout_ms = flag_value(flags, "--timeout-ms")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--timeout-ms must be a u64".to_string())
                })
                .transpose()?;
            let send_retries = flag_value(flags, "--send-retries")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--send-retries must be a u64".to_string())
                })
                .transpose()?;
            let retry_backoff_ms = flag_value(flags, "--retry-backoff-ms")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--retry-backoff-ms must be a u64".to_string())
                })
                .transpose()?;
            let agent_timeout_secs = flag_value(flags, "--agent-timeout-secs")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--agent-timeout-secs must be a u64".to_string())
                })
                .transpose()?;
            let report = nav_roundtrip_benchmark_base_args(
                NavRoundtripBenchmarkBaseArgsOptions {
                    summary_file: PathBuf::from(
                        flag_value(flags, "--summary").ok_or("missing --summary")?,
                    ),
                    output_file: PathBuf::from(
                        flag_value(flags, "--output").ok_or("missing --output")?,
                    ),
                    data_dir: flag_value(flags, "--data-dir").map(PathBuf::from),
                    topology_file: flag_value(flags, "--topology").map(PathBuf::from),
                    key_file: PathBuf::from(
                        flag_value(flags, "--key-file").ok_or("missing --key-file")?,
                    ),
                    proposal_key_file: flag_value(flags, "--proposal-key-file").map(PathBuf::from),
                    proposer_key_file: PathBuf::from(
                        flag_value(flags, "--proposer-key-file")
                            .ok_or("missing --proposer-key-file")?,
                    ),
                    attestor_key_file: flag_value(flags, "--attestor-key-file").map(PathBuf::from),
                    finalizer_key_file: PathBuf::from(
                        flag_value(flags, "--finalizer-key-file")
                            .ok_or("missing --finalizer-key-file")?,
                    ),
                    claimer_key_file: PathBuf::from(
                        flag_value(flags, "--claimer-key-file")
                            .ok_or("missing --claimer-key-file")?,
                    ),
                    issuer_key_file: PathBuf::from(
                        flag_value(flags, "--issuer-key-file")
                            .ok_or("missing --issuer-key-file")?,
                    ),
                    owner_key_file: PathBuf::from(
                        flag_value(flags, "--owner-key-file").ok_or("missing --owner-key-file")?,
                    ),
                    settlement_key_file: flag_value(flags, "--settlement-key-file")
                        .map(PathBuf::from),
                    submitter_key_file: flag_value(flags, "--submitter-key-file")
                        .map(PathBuf::from),
                    withdrawal_signer_key_file: PathBuf::from(
                        flag_value(flags, "--withdrawal-signer-key-file")
                            .ok_or("missing --withdrawal-signer-key-file")?,
                    ),
                    nonce_base: flag_value(flags, "--nonce-base")
                        .ok_or("missing --nonce-base")?
                        .to_string(),
                    session_id_base: flag_value(flags, "--session-id-base")
                        .unwrap_or("nav-roundtrip-bench")
                        .to_string(),
                    timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    agent_timeout_secs,
                    min_gas_wei: flag_value(flags, "--min-gas-wei").map(str::to_string),
                    destination_ref: flag_value(flags, "--destination-ref").map(str::to_string),
                    overwrite: flag_present(flags, "--overwrite"),
                    report_file: flag_value(flags, "--report").map(PathBuf::from),
                },
            )?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("NAV roundtrip benchmark base args serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "nav-roundtrip-benchmark-plan" => {
            let phase = flag_value(flags, "--phase").unwrap_or("phase1");
            let run_count = flag_value(flags, "--run-count")
                .unwrap_or("10")
                .parse::<usize>()
                .map_err(|_| "--run-count must be a usize".to_string())?;
            let max_median_ms = flag_value(flags, "--max-median-ms")
                .map(|value| {
                    value
                        .parse::<f64>()
                        .map_err(|_| "--max-median-ms must be a number".to_string())
                })
                .transpose()?;
            let max_p90_ms = flag_value(flags, "--max-p90-ms")
                .map(|value| {
                    value
                        .parse::<f64>()
                        .map_err(|_| "--max-p90-ms must be a number".to_string())
                })
                .transpose()?;
            let report = nav_roundtrip_benchmark_plan(NavRoundtripBenchmarkPlanOptions {
                phase: phase.to_string(),
                base_args_file: PathBuf::from(
                    flag_value(flags, "--base-args-file").ok_or("missing --base-args-file")?,
                ),
                benchmark_dir: PathBuf::from(
                    flag_value(flags, "--benchmark-dir").ok_or("missing --benchmark-dir")?,
                ),
                replay_corpus_file: flag_value(flags, "--replay-corpus-file").map(PathBuf::from),
                replay_corpus_dir: flag_value(flags, "--replay-corpus-dir").map(PathBuf::from),
                required_candidate_classes: flag_value(flags, "--require-candidate-classes")
                    .map(parse_csv_values)
                    .transpose()?
                    .unwrap_or_default(),
                report_file: flag_value(flags, "--report").map(PathBuf::from),
                run_count,
                run_prefix: flag_value(flags, "--run-prefix")
                    .unwrap_or("run-")
                    .to_string(),
                binary: flag_value(flags, "--binary")
                    .unwrap_or("postfiat-node")
                    .to_string(),
                max_median_ms,
                max_p90_ms,
                overwrite: flag_present(flags, "--overwrite"),
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("NAV roundtrip benchmark plan serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
        "nav-roundtrip-benchmark-verify" => {
            let phase = flag_value(flags, "--phase").ok_or("missing --phase")?;
            let min_clean_runs = flag_value(flags, "--min-clean-runs")
                .map(|value| {
                    value
                        .parse::<usize>()
                        .map_err(|_| "--min-clean-runs must be a usize".to_string())
                })
                .transpose()?;
            let max_median_ms = flag_value(flags, "--max-median-ms")
                .map(|value| {
                    value
                        .parse::<f64>()
                        .map_err(|_| "--max-median-ms must be a number".to_string())
                })
                .transpose()?;
            let max_p90_ms = flag_value(flags, "--max-p90-ms")
                .map(|value| {
                    value
                        .parse::<f64>()
                        .map_err(|_| "--max-p90-ms must be a number".to_string())
                })
                .transpose()?;
            let strict_exit = flag_present(flags, "--strict");
            let required_candidate_classes = flag_value(flags, "--require-candidate-classes")
                .map(parse_csv_values)
                .transpose()?
                .unwrap_or_default();
            let report = nav_roundtrip_benchmark_verify(NavRoundtripBenchmarkVerifyOptions {
                phase: phase.to_string(),
                summary_file: flag_value(flags, "--summary").map(PathBuf::from),
                benchmark_dir: flag_value(flags, "--benchmark-dir").map(PathBuf::from),
                replay_corpus_file: flag_value(flags, "--replay-corpus-file").map(PathBuf::from),
                replay_corpus_dir: flag_value(flags, "--replay-corpus-dir").map(PathBuf::from),
                required_candidate_classes,
                report_file: flag_value(flags, "--report").map(PathBuf::from),
                min_clean_runs,
                max_median_ms,
                max_p90_ms,
                strict_exit,
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("NAV roundtrip benchmark verify serialization failed: {error}")
            })?;
            println!("{json}");
            if strict_exit && !report.passed {
                return Err(format!(
                    "NAV roundtrip benchmark verification failed: {:?}",
                    report.failure_reasons
                ));
            }
            Ok(())
        }
        "nav-roundtrip-replay-corpus-verify" => {
            let strict_exit = flag_present(flags, "--strict");
            let required_candidate_classes = flag_value(flags, "--require-candidate-classes")
                .map(parse_csv_values)
                .transpose()?
                .unwrap_or_default();
            let report = nav_roundtrip_replay_corpus_verify(
                NavRoundtripReplayCorpusVerifyOptions {
                    corpus_file: flag_value(flags, "--corpus-file").map(PathBuf::from),
                    corpus_dir: flag_value(flags, "--corpus-dir").map(PathBuf::from),
                    report_file: flag_value(flags, "--report").map(PathBuf::from),
                    require_live_compression_ready: flag_present(
                        flags,
                        "--require-live-compression-ready",
                    ),
                    required_candidate_classes,
                    strict_exit: false,
                },
            )?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("NAV roundtrip replay corpus verify serialization failed: {error}")
            })?;
            println!("{json}");
            if strict_exit && !report.passed {
                return Err(format!(
                    "NAV roundtrip replay corpus verification failed: {:?}",
                    report.failure_reasons
                ));
            }
            Ok(())
        }
        _ => unreachable!("run_cli_group_01 dispatch mismatch"),
    }
}
