fn run_cli_group_02(command: &str, flags: &[String]) -> Result<(), String> {
    match command {
        "nav-roundtrip-live-demo" => {
            if flag_present(flags, "--allow-peer-failures") {
                return Err(
                    "nav-roundtrip-live-demo is live-value mode and rejects --allow-peer-failures"
                        .to_string(),
                );
            }
            if flag_present(flags, "--defer-certified-sends") {
                return Err(
                    "nav-roundtrip-live-demo is live-value mode and rejects --defer-certified-sends"
                        .to_string(),
                );
            }
            let fleet_preflight_only = flag_present(flags, "--fleet-preflight-only");
            let preflight_only = flag_present(flags, "--preflight-only");
            let warm_usdc_allowance_only = flag_present(flags, "--warm-usdc-allowance-only");
            let evm_deposit_only = flag_present(flags, "--evm-deposit-only");
            let deposit_relay_only = flag_present(flags, "--deposit-relay-only");
            let primary_mint_only = flag_present(flags, "--primary-mint-only");
            let nav_exit_only = flag_present(flags, "--nav-exit-only");
            let burn_to_redeem_only = flag_present(flags, "--burn-to-redeem-only");
            let evm_withdrawal_only = flag_present(flags, "--evm-withdrawal-only");
            let pftl_settle_only = flag_present(flags, "--pftl-settle-only");
            let nav_checkpoint_only = flag_present(flags, "--nav-checkpoint-only");
            let pftl_only = flag_present(flags, "--pftl-only");
            let selected_stage_count =
                fleet_preflight_only as usize
                    + preflight_only as usize
                    + warm_usdc_allowance_only as usize
                    + evm_deposit_only as usize
                    + deposit_relay_only as usize
                    + primary_mint_only as usize
                    + nav_exit_only as usize
                    + burn_to_redeem_only as usize
                    + evm_withdrawal_only as usize
                    + pftl_settle_only as usize
                    + nav_checkpoint_only as usize;
            if selected_stage_count > 1 {
                return Err(
                    "nav-roundtrip-live-demo accepts at most one stage flag: --fleet-preflight-only, --preflight-only, --warm-usdc-allowance-only, --evm-deposit-only, --deposit-relay-only, --primary-mint-only, --nav-checkpoint-only, --nav-exit-only, --burn-to-redeem-only, --evm-withdrawal-only, or --pftl-settle-only"
                        .to_string(),
                );
            }
            if pftl_only && selected_stage_count > 0 {
                return Err(
                    "--pftl-only cannot be combined with a --*-only stage flag".to_string(),
                );
            }
            if pftl_only {
                let data_dir =
                    PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
                let topology_file =
                    PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
                let validator_key_file =
                    PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let nav_asset_id = flag_value(flags, "--nav-asset").ok_or("missing --nav-asset")?;
                let settlement_asset_id =
                    flag_value(flags, "--pfusdc").ok_or("missing --pfusdc")?;
                let subscriber = flag_value(flags, "--subscriber")
                    .or_else(|| flag_value(flags, "--owner"))
                    .ok_or("missing --subscriber or --owner for --pftl-only")?
                    .to_string();
                let owner = flag_value(flags, "--owner")
                    .map(str::to_string)
                    .unwrap_or_else(|| subscriber.clone());
                let mint_amount = flag_value(flags, "--mint-amount")
                    .ok_or("missing --mint-amount")?
                    .parse::<u64>()
                    .map_err(|_| "--mint-amount must be a u64".to_string())?;
                let settlement_amount_atoms = flag_value(flags, "--settlement-amount-atoms")
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--settlement-amount-atoms must be a u64".to_string())
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
                let report = nav_roundtrip_pftl_only(NavRoundtripPftlOnlyOptions {
                    data_dir,
                    topology_file,
                    validator_key_file,
                    proposal_key_file: flag_value(flags, "--proposal-key-file").map(PathBuf::from),
                    artifact_dir,
                    nav_asset_id: nav_asset_id.to_string(),
                    settlement_asset_id: settlement_asset_id.to_string(),
                    subscriber,
                    owner,
                    issuer_key_file: PathBuf::from(
                        flag_value(flags, "--issuer-key-file")
                            .ok_or("missing --issuer-key-file")?,
                    ),
                    owner_key_file: PathBuf::from(
                        flag_value(flags, "--owner-key-file").ok_or("missing --owner-key-file")?,
                    ),
                    submitter_key_file: flag_value(flags, "--submitter-key-file")
                        .map(PathBuf::from),
                    mint_amount,
                    settlement_amount_atoms,
                    settlement_receipt_id: flag_value(flags, "--settlement-receipt-id")
                        .map(str::to_string),
                    settlement_supply_allocation_id: flag_value(
                        flags,
                        "--settlement-supply-allocation-id",
                    )
                    .map(str::to_string),
                    same_round_nav_exit: flag_present(flags, "--same-round-nav-exit"),
                    destination_ref: flag_value(flags, "--destination-ref").map(str::to_string),
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
                    timeout_certificate_file: flag_value(flags, "--timeout-certificate-file")
                        .map(PathBuf::from),
                    timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    allow_existing_mempool: flag_present(flags, "--allow-existing-mempool"),
                    reuse_final_certified_state: flag_present(flags, "--reuse-final-certified-state"),
                    fast_demo_preflight: flag_present(flags, "--fast-demo-preflight"),
                    background_audit: flag_present(flags, "--background-audit"),
                    resume: flag_present(flags, "--resume"),
                    overwrite: flag_present(flags, "--overwrite"),
                    batch_only: flag_present(flags, "--batch-only"),
                })?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("PFTL-only NAV roundtrip serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if fleet_preflight_only {
                let data_dir =
                    PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
                let topology_file =
                    PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let timeout_ms = flag_value(flags, "--timeout-ms")
                    .unwrap_or("5000")
                    .parse::<u64>()
                    .map_err(|_| "--timeout-ms must be a u64".to_string())?;
                let report = nav_roundtrip_live_fleet_preflight(
                    &data_dir,
                    &topology_file,
                    &artifact_dir,
                    timeout_ms,
                    flag_present(flags, "--resume"),
                    flag_present(flags, "--overwrite"),
                    false,
                )?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip live demo fleet preflight serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if preflight_only {
                let data_dir =
                    PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let source_rpc_url =
                    flag_value(flags, "--source-rpc-url").ok_or("missing --source-rpc-url")?;
                let vault_address = flag_value(flags, "--vault").ok_or("missing --vault")?;
                let verifier_address =
                    flag_value(flags, "--verifier").ok_or("missing --verifier")?;
                let usdc_address = flag_value(flags, "--usdc").ok_or("missing --usdc")?;
                let stakehub_wallet =
                    flag_value(flags, "--stakehub-wallet").ok_or("missing --stakehub-wallet")?;
                let amount_atoms = flag_value(flags, "--amount-atoms")
                    .ok_or("missing --amount-atoms")?
                    .parse::<u64>()
                    .map_err(|_| "--amount-atoms must be a u64".to_string())?;
                let min_gas_wei = flag_value(flags, "--min-gas-wei")
                    .unwrap_or("1000000000000000")
                    .parse::<u128>()
                    .map_err(|_| "--min-gas-wei must be a u128".to_string())?;
                let report = nav_roundtrip_live_demo_preflight(NavRoundtripPreflightOptions {
                    data_dir,
                    artifact_dir,
                    source_rpc_url: source_rpc_url.to_string(),
                    cast_binary: flag_value(flags, "--cast-bin").unwrap_or("cast").to_string(),
                    vault_address: vault_address.to_string(),
                    verifier_address: verifier_address.to_string(),
                    usdc_address: usdc_address.to_string(),
                    stakehub_wallet: stakehub_wallet.to_string(),
                    amount_atoms,
                    min_gas_wei,
                    resume: flag_present(flags, "--resume"),
                    overwrite: flag_present(flags, "--overwrite"),
                })?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip live demo preflight serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if warm_usdc_allowance_only {
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let source_rpc_url =
                    flag_value(flags, "--source-rpc-url").ok_or("missing --source-rpc-url")?;
                let vault_address = flag_value(flags, "--vault").ok_or("missing --vault")?;
                let verifier_address =
                    flag_value(flags, "--verifier").ok_or("missing --verifier")?;
                let usdc_address = flag_value(flags, "--usdc").ok_or("missing --usdc")?;
                let stakehub_wallet =
                    flag_value(flags, "--stakehub-wallet").ok_or("missing --stakehub-wallet")?;
                let session_id = flag_value(flags, "--session-id").ok_or("missing --session-id")?;
                let required_allowance_atoms = flag_value(flags, "--required-allowance-atoms")
                    .or_else(|| flag_value(flags, "--amount-atoms"))
                    .ok_or("missing --required-allowance-atoms")?
                    .parse::<u64>()
                    .map_err(|_| "--required-allowance-atoms must be a u64".to_string())?;
                let source_chain_id = flag_value(flags, "--source-chain-id")
                    .unwrap_or("42161")
                    .parse::<u64>()
                    .map_err(|_| "--source-chain-id must be a u64".to_string())?;
                let agent_timeout_secs = flag_value(flags, "--agent-timeout-secs")
                    .unwrap_or("1200")
                    .parse::<u64>()
                    .map_err(|_| "--agent-timeout-secs must be a u64".to_string())?;
                let stakehub_home = flag_value(flags, "--stakehub-home")
                    .map(PathBuf::from)
                    .unwrap_or_else(default_stakehub_home);
                let report = nav_roundtrip_live_demo_warm_usdc_allowance(
                    NavRoundtripUsdcAllowanceSetupOptions {
                        artifact_dir,
                        source_rpc_url: source_rpc_url.to_string(),
                        cast_binary: flag_value(flags, "--cast-bin").unwrap_or("cast").to_string(),
                        stakehub_home,
                        source_chain_id,
                        vault_address: vault_address.to_string(),
                        verifier_address: verifier_address.to_string(),
                        usdc_address: usdc_address.to_string(),
                        stakehub_wallet: stakehub_wallet.to_string(),
                        required_allowance_atoms,
                        session_id: session_id.to_string(),
                        resume: flag_present(flags, "--resume"),
                        overwrite: flag_present(flags, "--overwrite"),
                        agent_timeout_secs,
                    },
                )?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip USDC allowance setup serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if evm_deposit_only {
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let source_rpc_url =
                    flag_value(flags, "--source-rpc-url").ok_or("missing --source-rpc-url")?;
                let vault_address = flag_value(flags, "--vault").ok_or("missing --vault")?;
                let usdc_address = flag_value(flags, "--usdc").ok_or("missing --usdc")?;
                let stakehub_wallet =
                    flag_value(flags, "--stakehub-wallet").ok_or("missing --stakehub-wallet")?;
                let pftl_recipient =
                    flag_value(flags, "--pftl-recipient").ok_or("missing --pftl-recipient")?;
                let nonce = flag_value(flags, "--nonce").ok_or("missing --nonce")?;
                let session_id = flag_value(flags, "--session-id").ok_or("missing --session-id")?;
                let amount_atoms = flag_value(flags, "--amount-atoms")
                    .ok_or("missing --amount-atoms")?
                    .parse::<u64>()
                    .map_err(|_| "--amount-atoms must be a u64".to_string())?;
                let source_chain_id = flag_value(flags, "--source-chain-id")
                    .unwrap_or("42161")
                    .parse::<u64>()
                    .map_err(|_| "--source-chain-id must be a u64".to_string())?;
                let agent_timeout_secs = flag_value(flags, "--agent-timeout-secs")
                    .unwrap_or("1200")
                    .parse::<u64>()
                    .map_err(|_| "--agent-timeout-secs must be a u64".to_string())?;
                let stakehub_home = flag_value(flags, "--stakehub-home")
                    .map(PathBuf::from)
                    .unwrap_or_else(default_stakehub_home);
                let report = nav_roundtrip_live_demo_evm_deposit(
                    NavRoundtripEvmDepositOptions {
                        artifact_dir,
                        source_rpc_url: source_rpc_url.to_string(),
                        cast_binary: flag_value(flags, "--cast-bin").unwrap_or("cast").to_string(),
                        stakehub_home,
                        source_chain_id,
                        vault_address: vault_address.to_string(),
                        usdc_address: usdc_address.to_string(),
                        stakehub_wallet: stakehub_wallet.to_string(),
                        pftl_recipient: pftl_recipient.to_string(),
                        amount_atoms,
                        nonce: nonce.to_string(),
                        session_id: session_id.to_string(),
                        resume: flag_present(flags, "--resume"),
                        overwrite: flag_present(flags, "--overwrite"),
                        agent_timeout_secs,
                        launch_session_managed_externally: false,
                        require_warm_allowance: flag_present(
                            flags,
                            "--require-warm-usdc-allowance",
                        ),
                    },
                )?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip EVM deposit serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if deposit_relay_only {
                let data_dir =
                    PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
                let topology_file =
                    PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
                let validator_key_file =
                    PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let evm_deposit_report_file = PathBuf::from(
                    flag_value(flags, "--evm-deposit-report")
                        .ok_or("missing --evm-deposit-report")?,
                );
                let source_rpc_url =
                    flag_value(flags, "--source-rpc-url").ok_or("missing --source-rpc-url")?;
                let vault_address = flag_value(flags, "--vault").ok_or("missing --vault")?;
                let usdc_address = flag_value(flags, "--usdc").ok_or("missing --usdc")?;
                let asset_id = flag_value(flags, "--pfusdc").ok_or("missing --pfusdc")?;
                let policy_hash =
                    flag_value(flags, "--policy-hash").ok_or("missing --policy-hash")?;
                let proposer = flag_value(flags, "--proposer").ok_or("missing --proposer")?;
                let finalizer = flag_value(flags, "--finalizer").ok_or("missing --finalizer")?;
                let claimer = flag_value(flags, "--claimer").ok_or("missing --claimer")?;
                let expires_at_height = flag_value(flags, "--expires-at-height")
                    .ok_or("missing --expires-at-height")?
                    .parse::<u64>()
                    .map_err(|_| "--expires-at-height must be a u64".to_string())?;
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
                let report = nav_roundtrip_live_demo_deposit_relay(
                    NavRoundtripDepositRelayOptions {
                        data_dir,
                        topology_file,
                        validator_key_file,
                        proposal_key_file: flag_value(flags, "--proposal-key-file")
                            .map(PathBuf::from),
                        artifact_dir,
                        evm_deposit_report_file,
                        source_rpc_url: source_rpc_url.to_string(),
                        cast_binary: flag_value(flags, "--cast-bin").unwrap_or("cast").to_string(),
                        vault_address: vault_address.to_string(),
                        token_address: usdc_address.to_string(),
                        asset_id: asset_id.to_string(),
                        policy_hash: policy_hash.to_string(),
                        proposer: proposer.to_string(),
                        attestor: flag_value(flags, "--attestor").map(str::to_string),
                        finalizer: finalizer.to_string(),
                        claimer: claimer.to_string(),
                        proposer_key_file: PathBuf::from(
                            flag_value(flags, "--proposer-key-file")
                                .ok_or("missing --proposer-key-file")?,
                        ),
                        attestor_key_file: flag_value(flags, "--attestor-key-file")
                            .map(PathBuf::from),
                        finalizer_key_file: PathBuf::from(
                            flag_value(flags, "--finalizer-key-file")
                                .ok_or("missing --finalizer-key-file")?,
                        ),
                        claimer_key_file: PathBuf::from(
                            flag_value(flags, "--claimer-key-file")
                                .ok_or("missing --claimer-key-file")?,
                        ),
                        receipt_operator_key_file: flag_value(flags, "--issuer-key-file")
                            .map(PathBuf::from),
                        claim_deposit: flag_present(flags, "--claim-deposit"),
                        expires_at_height,
                        source_proof_kind: flag_value(flags, "--source-proof-kind")
                            .map(str::to_string),
                        source_proof_hash: flag_value(flags, "--source-proof-hash")
                            .map(str::to_string),
                        source_public_values_hash: flag_value(
                            flags,
                            "--source-public-values-hash",
                        )
                        .map(str::to_string),
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
                        timeout_certificate_file: flag_value(flags, "--timeout-certificate-file")
                            .map(PathBuf::from),
                        timeout_ms,
                        send_retries,
                        retry_backoff_ms,
                        allow_existing_mempool: flag_present(flags, "--allow-existing-mempool"),
                        resume: flag_present(flags, "--resume"),
                        overwrite: flag_present(flags, "--overwrite"),
                        prepare_only: flag_present(flags, "--prepare-only"),
                        batch_only: flag_present(flags, "--batch-only"),
                    },
                )?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip deposit relay serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if primary_mint_only {
                let data_dir =
                    PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
                let topology_file =
                    PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
                let validator_key_file =
                    PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let nav_asset_id =
                    flag_value(flags, "--nav-asset").ok_or("missing --nav-asset")?;
                let settlement_asset_id =
                    flag_value(flags, "--pfusdc").ok_or("missing --pfusdc")?;
                let subscriber = flag_value(flags, "--subscriber").ok_or("missing --subscriber")?;
                let issuer_key_file = PathBuf::from(
                    flag_value(flags, "--issuer-key-file").ok_or("missing --issuer-key-file")?,
                );
                let mint_amount = flag_value(flags, "--mint-amount")
                    .ok_or("missing --mint-amount")?
                    .parse::<u64>()
                    .map_err(|_| "--mint-amount must be a u64".to_string())?;
                let settlement_amount_atoms = flag_value(flags, "--settlement-amount-atoms")
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--settlement-amount-atoms must be a u64".to_string())
                    })
                    .transpose()?;
                let nav_epoch = flag_value(flags, "--nav-epoch")
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--nav-epoch must be a u64".to_string())
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
                let report = nav_roundtrip_live_demo_primary_mint(
                    NavRoundtripPrimaryMintOptions {
                        data_dir,
                        topology_file,
                        validator_key_file,
                        proposal_key_file: flag_value(flags, "--proposal-key-file")
                            .map(PathBuf::from),
                        artifact_dir,
                        deposit_relay_report_file: flag_value(flags, "--deposit-relay-report")
                            .map(PathBuf::from),
                        nav_asset_id: nav_asset_id.to_string(),
                        settlement_asset_id: settlement_asset_id.to_string(),
                        subscriber: subscriber.to_string(),
                        issuer_key_file,
                        subscriber_key_file: flag_value(flags, "--subscriber-key-file")
                            .map(PathBuf::from),
                        settlement_receipt_id: flag_value(flags, "--settlement-receipt-id")
                            .map(str::to_string),
                        settlement_supply_allocation_id: flag_value(
                            flags,
                            "--settlement-supply-allocation-id",
                        )
                        .map(str::to_string),
                        consume_issued_settlement: flag_present(
                            flags,
                            "--consume-issued-settlement",
                        ),
                        settlement_amount_atoms,
                        mint_amount,
                        nav_epoch,
                        nav_reserve_packet_hash: flag_value(
                            flags,
                            "--nav-reserve-packet-hash",
                        )
                        .map(str::to_string),
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
                        timeout_certificate_file: flag_value(flags, "--timeout-certificate-file")
                            .map(PathBuf::from),
                        timeout_ms,
                        send_retries,
                        retry_backoff_ms,
                        allow_existing_mempool: flag_present(flags, "--allow-existing-mempool"),
                        resume: flag_present(flags, "--resume"),
                        overwrite: flag_present(flags, "--overwrite"),
                        prepare_only: flag_present(flags, "--prepare-only"),
                        batch_only: flag_present(flags, "--batch-only"),
                    },
                )?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip primary mint serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if nav_checkpoint_only {
                let data_dir =
                    PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
                let topology_file =
                    PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
                let validator_key_file =
                    PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let nav_asset_id =
                    flag_value(flags, "--nav-asset").ok_or("missing --nav-asset")?;
                let issuer_key_file = PathBuf::from(
                    flag_value(flags, "--issuer-key-file").ok_or("missing --issuer-key-file")?,
                );
                let epoch = flag_value(flags, "--nav-epoch")
                    .or_else(|| flag_value(flags, "--epoch"))
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--nav-epoch/--epoch must be a u64".to_string())
                    })
                    .transpose()?;
                let expected_vna_delta = flag_value(flags, "--expected-vna-delta")
                    .map(|value| {
                        value
                            .parse::<i128>()
                            .map_err(|_| "--expected-vna-delta must be an integer".to_string())
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
                let report = nav_roundtrip_live_demo_nav_checkpoint(
                    NavRoundtripNavCheckpointOptions {
                        data_dir,
                        topology_file,
                        validator_key_file,
                        proposal_key_file: flag_value(flags, "--proposal-key-file")
                            .map(PathBuf::from),
                        artifact_dir,
                        nav_asset_id: nav_asset_id.to_string(),
                        issuer_key_file,
                        submitter_key_file: flag_value(flags, "--submitter-key-file")
                            .map(PathBuf::from),
                        epoch,
                        expected_vna_delta,
                        reserve_packet_hash: flag_value(flags, "--reserve-packet-hash")
                            .map(str::to_string),
                        attestor_root: flag_value(flags, "--attestor-root").map(str::to_string),
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
                        timeout_certificate_file: flag_value(flags, "--timeout-certificate-file")
                            .map(PathBuf::from),
                        timeout_ms,
                        send_retries,
                        retry_backoff_ms,
                        allow_existing_mempool: flag_present(flags, "--allow-existing-mempool"),
                        resume: flag_present(flags, "--resume"),
                        overwrite: flag_present(flags, "--overwrite"),
                        prepare_only: flag_present(flags, "--prepare-only"),
                    },
                )?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip checkpoint serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if nav_exit_only {
                let data_dir =
                    PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
                let topology_file =
                    PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
                let validator_key_file =
                    PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let primary_mint_report_file = PathBuf::from(
                    flag_value(flags, "--primary-mint-report")
                        .ok_or("missing --primary-mint-report")?,
                );
                let nav_asset_id =
                    flag_value(flags, "--nav-asset").ok_or("missing --nav-asset")?;
                let settlement_asset_id =
                    flag_value(flags, "--pfusdc").ok_or("missing --pfusdc")?;
                let owner_key_file = PathBuf::from(
                    flag_value(flags, "--owner-key-file").ok_or("missing --owner-key-file")?,
                );
                let issuer_key_file = PathBuf::from(
                    flag_value(flags, "--issuer-key-file").ok_or("missing --issuer-key-file")?,
                );
                let amount = flag_value(flags, "--amount")
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--amount must be a u64".to_string())
                    })
                    .transpose()?;
                let settlement_amount_atoms = flag_value(flags, "--settlement-amount-atoms")
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--settlement-amount-atoms must be a u64".to_string())
                    })
                    .transpose()?;
                let nav_epoch = flag_value(flags, "--nav-epoch")
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--nav-epoch must be a u64".to_string())
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
                let report = nav_roundtrip_live_demo_nav_exit(NavRoundtripNavExitOptions {
                    data_dir,
                    topology_file,
                    validator_key_file,
                    proposal_key_file: flag_value(flags, "--proposal-key-file")
                        .map(PathBuf::from),
                    artifact_dir,
                    primary_mint_report_file,
                    nav_asset_id: nav_asset_id.to_string(),
                    settlement_asset_id: settlement_asset_id.to_string(),
                    owner: flag_value(flags, "--owner").map(str::to_string),
                    owner_key_file,
                    issuer_key_file,
                    amount,
                    settlement_amount_atoms,
                    settlement_receipt_hash: flag_value(flags, "--settlement-receipt-hash")
                        .map(str::to_string),
                    redemption_id: flag_value(flags, "--redemption-id").map(str::to_string),
                    same_round_settlement: flag_present(flags, "--same-round-nav-exit"),
                    nav_epoch,
                    nav_reserve_packet_hash: flag_value(flags, "--nav-reserve-packet-hash")
                        .map(str::to_string),
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
                    timeout_certificate_file: flag_value(flags, "--timeout-certificate-file")
                        .map(PathBuf::from),
                    timeout_ms,
                    send_retries,
                    retry_backoff_ms,
                    allow_existing_mempool: flag_present(flags, "--allow-existing-mempool"),
                    resume: flag_present(flags, "--resume"),
                    overwrite: flag_present(flags, "--overwrite"),
                    prepare_only: flag_present(flags, "--prepare-only"),
                    batch_only: flag_present(flags, "--batch-only"),
                })?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip NAV exit serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if burn_to_redeem_only {
                let data_dir =
                    PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
                let topology_file =
                    PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
                let validator_key_file =
                    PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let nav_exit_report_file = PathBuf::from(
                    flag_value(flags, "--nav-exit-report").ok_or("missing --nav-exit-report")?,
                );
                let settlement_asset_id =
                    flag_value(flags, "--pfusdc").ok_or("missing --pfusdc")?;
                let owner_key_file = PathBuf::from(
                    flag_value(flags, "--owner-key-file").ok_or("missing --owner-key-file")?,
                );
                let destination_ref =
                    flag_value(flags, "--destination-ref").ok_or("missing --destination-ref")?;
                let amount_atoms = flag_value(flags, "--amount-atoms")
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--amount-atoms must be a u64".to_string())
                    })
                    .transpose()?;
                let epoch = flag_value(flags, "--epoch")
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--epoch must be a u64".to_string())
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
                    nav_roundtrip_live_demo_burn_to_redeem(NavRoundtripBurnToRedeemOptions {
                        data_dir,
                        topology_file,
                        validator_key_file,
                        proposal_key_file: flag_value(flags, "--proposal-key-file")
                            .map(PathBuf::from),
                        artifact_dir,
                        nav_exit_report_file,
                        settlement_asset_id: settlement_asset_id.to_string(),
                        owner: flag_value(flags, "--owner").map(str::to_string),
                        owner_key_file,
                        amount_atoms,
                        destination_ref: destination_ref.to_string(),
                        issuer: flag_value(flags, "--issuer").map(str::to_string),
                        bucket_id: flag_value(flags, "--bucket-id").map(str::to_string),
                        epoch,
                        reserve_packet_hash: flag_value(flags, "--reserve-packet-hash")
                            .map(str::to_string),
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
                        timeout_certificate_file: flag_value(flags, "--timeout-certificate-file")
                            .map(PathBuf::from),
                        timeout_ms,
                        send_retries,
                        retry_backoff_ms,
                        allow_existing_mempool: flag_present(flags, "--allow-existing-mempool"),
                        resume: flag_present(flags, "--resume"),
                        overwrite: flag_present(flags, "--overwrite"),
                        prepare_only: flag_present(flags, "--prepare-only"),
                        batch_only: flag_present(flags, "--batch-only"),
                    })?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip burn-to-redeem serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if evm_withdrawal_only {
                let data_dir =
                    PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let burn_to_redeem_report_file = PathBuf::from(
                    flag_value(flags, "--burn-to-redeem-report")
                        .ok_or("missing --burn-to-redeem-report")?,
                );
                let source_rpc_url =
                    flag_value(flags, "--source-rpc-url").ok_or("missing --source-rpc-url")?;
                let vault_address = flag_value(flags, "--vault").ok_or("missing --vault")?;
                let verifier_address =
                    flag_value(flags, "--verifier").ok_or("missing --verifier")?;
                let usdc_address = flag_value(flags, "--usdc").ok_or("missing --usdc")?;
                let stakehub_wallet =
                    flag_value(flags, "--stakehub-wallet").ok_or("missing --stakehub-wallet")?;
                let settlement_asset_id =
                    flag_value(flags, "--pfusdc").ok_or("missing --pfusdc")?;
                if flag_present(flags, "--signatures-file")
                    && flag_present(flags, "--withdrawal-signer-key-file")
                {
                    return Err(
                        "use only one of --signatures-file or --withdrawal-signer-key-file"
                            .to_string(),
                    );
                }
                let signatures_file = flag_value(flags, "--signatures-file").map(PathBuf::from);
                let withdrawal_signer_key_file =
                    flag_value(flags, "--withdrawal-signer-key-file").map(PathBuf::from);
                if signatures_file.is_none() && withdrawal_signer_key_file.is_none() {
                    return Err(
                        "missing --signatures-file or --withdrawal-signer-key-file".to_string(),
                    );
                }
                let session_id = flag_value(flags, "--session-id").ok_or("missing --session-id")?;
                let source_chain_id = flag_value(flags, "--source-chain-id")
                    .unwrap_or("42161")
                    .parse::<u64>()
                    .map_err(|_| "--source-chain-id must be a u64".to_string())?;
                let pftl_finalized_height = flag_value(flags, "--pftl-finalized-height")
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--pftl-finalized-height must be a u64".to_string())
                    })
                    .transpose()?;
                let challenge_wait_secs = flag_value(flags, "--challenge-wait-secs")
                    .map(|value| {
                        value
                            .parse::<u64>()
                            .map_err(|_| "--challenge-wait-secs must be a u64".to_string())
                    })
                    .transpose()?;
                let agent_timeout_secs = flag_value(flags, "--agent-timeout-secs")
                    .unwrap_or("1200")
                    .parse::<u64>()
                    .map_err(|_| "--agent-timeout-secs must be a u64".to_string())?;
                let stakehub_home = flag_value(flags, "--stakehub-home")
                    .map(PathBuf::from)
                    .unwrap_or_else(default_stakehub_home);
                let report = nav_roundtrip_live_demo_evm_withdrawal(
                    NavRoundtripEvmWithdrawalOptions {
                        data_dir,
                        artifact_dir,
                        burn_to_redeem_report_file,
                        source_rpc_url: source_rpc_url.to_string(),
                        cast_binary: flag_value(flags, "--cast-bin").unwrap_or("cast").to_string(),
                        stakehub_home,
                        source_chain_id,
                        vault_address: vault_address.to_string(),
                        verifier_address: verifier_address.to_string(),
                        usdc_address: usdc_address.to_string(),
                        stakehub_wallet: stakehub_wallet.to_string(),
                        settlement_asset_id: settlement_asset_id.to_string(),
                        redemption_id: flag_value(flags, "--redemption-id").map(str::to_string),
                        pftl_finalized_height,
                        signatures_file,
                        withdrawal_signer_key_file,
                        session_id: session_id.to_string(),
                        challenge_wait_secs,
                        resume: flag_present(flags, "--resume"),
                        overwrite: flag_present(flags, "--overwrite"),
                        agent_timeout_secs,
                        launch_session_managed_externally: false,
                    },
                )?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip EVM withdrawal serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            if pftl_settle_only {
                let data_dir =
                    PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
                let topology_file =
                    PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
                let validator_key_file =
                    PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
                let artifact_dir = PathBuf::from(
                    flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?,
                );
                let evm_withdrawal_report_file = PathBuf::from(
                    flag_value(flags, "--evm-withdrawal-report")
                        .ok_or("missing --evm-withdrawal-report")?,
                );
                let settlement_asset_id =
                    flag_value(flags, "--pfusdc").ok_or("missing --pfusdc")?;
                let settlement_key_file = PathBuf::from(
                    flag_value(flags, "--settlement-key-file")
                        .ok_or("missing --settlement-key-file")?,
                );
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
                let report = nav_roundtrip_live_demo_pftl_settle(
                    NavRoundtripPftlSettleOptions {
                        data_dir,
                        topology_file,
                        validator_key_file,
                        proposal_key_file: flag_value(flags, "--proposal-key-file")
                            .map(PathBuf::from),
                        artifact_dir,
                        evm_withdrawal_report_file,
                        settlement_asset_id: settlement_asset_id.to_string(),
                        issuer_or_redemption_account: flag_value(
                            flags,
                            "--issuer-or-redemption-account",
                        )
                        .map(str::to_string),
                        settlement_key_file,
                        settlement_receipt_hash: flag_value(flags, "--settlement-receipt-hash")
                            .map(str::to_string),
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
                        timeout_certificate_file: flag_value(flags, "--timeout-certificate-file")
                            .map(PathBuf::from),
                        timeout_ms,
                        send_retries,
                        retry_backoff_ms,
                        allow_existing_mempool: flag_present(flags, "--allow-existing-mempool"),
                        resume: flag_present(flags, "--resume"),
                        overwrite: flag_present(flags, "--overwrite"),
                        prepare_only: flag_present(flags, "--prepare-only"),
                        batch_only: flag_present(flags, "--batch-only"),
                    },
                )?;
                let json = serde_json::to_string_pretty(&report).map_err(|error| {
                    format!("NAV roundtrip PFTL settle serialization failed: {error}")
                })?;
                println!("{json}");
                return Ok(());
            }
            let data_dir =
                PathBuf::from(flag_value(flags, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
            let topology_file =
                PathBuf::from(flag_value(flags, "--topology").ok_or("missing --topology")?);
            let validator_key_file =
                PathBuf::from(flag_value(flags, "--key-file").ok_or("missing --key-file")?);
            let artifact_dir =
                PathBuf::from(flag_value(flags, "--artifact-dir").ok_or("missing --artifact-dir")?);
            let source_rpc_url =
                flag_value(flags, "--source-rpc-url").ok_or("missing --source-rpc-url")?;
            let vault_address = flag_value(flags, "--vault").ok_or("missing --vault")?;
            let verifier_address = flag_value(flags, "--verifier").ok_or("missing --verifier")?;
            let usdc_address = flag_value(flags, "--usdc").ok_or("missing --usdc")?;
            let stakehub_wallet =
                flag_value(flags, "--stakehub-wallet").ok_or("missing --stakehub-wallet")?;
            let nav_asset_id = flag_value(flags, "--nav-asset").ok_or("missing --nav-asset")?;
            let settlement_asset_id = flag_value(flags, "--pfusdc").ok_or("missing --pfusdc")?;
            let policy_hash = flag_value(flags, "--policy-hash").ok_or("missing --policy-hash")?;
            let pftl_recipient =
                flag_value(flags, "--pftl-recipient").ok_or("missing --pftl-recipient")?;
            let proposer = flag_value(flags, "--proposer").ok_or("missing --proposer")?;
            let finalizer = flag_value(flags, "--finalizer").ok_or("missing --finalizer")?;
            let claimer = flag_value(flags, "--claimer").ok_or("missing --claimer")?;
            let amount_atoms = flag_value(flags, "--amount-atoms")
                .ok_or("missing --amount-atoms")?
                .parse::<u64>()
                .map_err(|_| "--amount-atoms must be a u64".to_string())?;
            let mint_amount = flag_value(flags, "--mint-amount")
                .ok_or("missing --mint-amount")?
                .parse::<u64>()
                .map_err(|_| "--mint-amount must be a u64".to_string())?;
            let nonce = flag_value(flags, "--nonce").ok_or("missing --nonce")?;
            let session_id = flag_value(flags, "--session-id").ok_or("missing --session-id")?;
            let expires_at_height = flag_value(flags, "--expires-at-height")
                .ok_or("missing --expires-at-height")?
                .parse::<u64>()
                .map_err(|_| "--expires-at-height must be a u64".to_string())?;
            let source_chain_id = flag_value(flags, "--source-chain-id")
                .unwrap_or("42161")
                .parse::<u64>()
                .map_err(|_| "--source-chain-id must be a u64".to_string())?;
            let min_gas_wei = flag_value(flags, "--min-gas-wei")
                .unwrap_or("1000000000000000")
                .parse::<u128>()
                .map_err(|_| "--min-gas-wei must be a u128".to_string())?;
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
            let challenge_wait_secs = flag_value(flags, "--challenge-wait-secs")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--challenge-wait-secs must be a u64".to_string())
                })
                .transpose()?;
            let pftl_finalized_height = flag_value(flags, "--pftl-finalized-height")
                .map(|value| {
                    value
                        .parse::<u64>()
                        .map_err(|_| "--pftl-finalized-height must be a u64".to_string())
                })
                .transpose()?;
            let agent_timeout_secs = flag_value(flags, "--agent-timeout-secs")
                .unwrap_or("1200")
                .parse::<u64>()
                .map_err(|_| "--agent-timeout-secs must be a u64".to_string())?;
            let stakehub_home = flag_value(flags, "--stakehub-home")
                .map(PathBuf::from)
                .unwrap_or_else(default_stakehub_home);
            let report = nav_roundtrip_live_demo(NavRoundtripLiveDemoOptions {
                data_dir,
                topology_file,
                validator_key_file,
                proposal_key_file: flag_value(flags, "--proposal-key-file").map(PathBuf::from),
                artifact_dir,
                source_rpc_url: source_rpc_url.to_string(),
                cast_binary: flag_value(flags, "--cast-bin").unwrap_or("cast").to_string(),
                stakehub_home,
                source_chain_id,
                vault_address: vault_address.to_string(),
                verifier_address: verifier_address.to_string(),
                usdc_address: usdc_address.to_string(),
                stakehub_wallet: stakehub_wallet.to_string(),
                nav_asset_id: nav_asset_id.to_string(),
                settlement_asset_id: settlement_asset_id.to_string(),
                policy_hash: policy_hash.to_string(),
                pftl_recipient: pftl_recipient.to_string(),
                subscriber: flag_value(flags, "--subscriber").map(str::to_string),
                owner: flag_value(flags, "--owner").map(str::to_string),
                proposer: proposer.to_string(),
                attestor: flag_value(flags, "--attestor").map(str::to_string),
                finalizer: finalizer.to_string(),
                claimer: claimer.to_string(),
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
                    flag_value(flags, "--issuer-key-file").ok_or("missing --issuer-key-file")?,
                ),
                owner_key_file: PathBuf::from(
                    flag_value(flags, "--owner-key-file").ok_or("missing --owner-key-file")?,
                ),
                settlement_key_file: flag_value(flags, "--settlement-key-file")
                    .map(PathBuf::from),
                submitter_key_file: flag_value(flags, "--submitter-key-file")
                    .map(PathBuf::from),
                amount_atoms,
                mint_amount,
                nonce: nonce.to_string(),
                session_id: session_id.to_string(),
                signatures_file: flag_value(flags, "--signatures-file").map(PathBuf::from),
                withdrawal_signer_key_file: flag_value(flags, "--withdrawal-signer-key-file")
                    .map(PathBuf::from),
                destination_ref: flag_value(flags, "--destination-ref").map(str::to_string),
                expires_at_height,
                source_proof_kind: flag_value(flags, "--source-proof-kind").map(str::to_string),
                source_proof_hash: flag_value(flags, "--source-proof-hash").map(str::to_string),
                source_public_values_hash: flag_value(flags, "--source-public-values-hash")
                    .map(str::to_string),
                min_gas_wei,
                challenge_wait_secs,
                pftl_finalized_height,
                same_round_nav_exit: flag_present(flags, "--same-round-nav-exit"),
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
                timeout_certificate_file: flag_value(flags, "--timeout-certificate-file")
                    .map(PathBuf::from),
                timeout_ms,
                send_retries,
                retry_backoff_ms,
                allow_existing_mempool: flag_present(flags, "--allow-existing-mempool"),
                reuse_final_certified_state: flag_present(flags, "--reuse-final-certified-state"),
                fast_demo_preflight: flag_present(flags, "--fast-demo-preflight"),
                background_audit: flag_present(flags, "--background-audit"),
                require_warm_usdc_allowance: flag_present(
                    flags,
                    "--require-warm-usdc-allowance",
                ),
                resume: flag_present(flags, "--resume"),
                overwrite: flag_present(flags, "--overwrite"),
                batch_only: flag_present(flags, "--batch-only"),
                agent_timeout_secs,
            })?;
            let json = serde_json::to_string_pretty(&report).map_err(|error| {
                format!("NAV roundtrip live demo serialization failed: {error}")
            })?;
            println!("{json}");
            Ok(())
        }
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
            })?;
            let json = serde_json::to_string_pretty(&report)
                .map_err(|error| format!("tx latency benchmark serialization failed: {error}"))?;
            println!("{json}");
            Ok(())
        }
        _ => unreachable!("run_cli_group_02 dispatch mismatch"),
    }
}
