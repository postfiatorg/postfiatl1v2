fn run_peer_certified_loop_options(
    flags: &[String],
    data_dir: PathBuf,
) -> Result<TransportPeerCertifiedBatchLoopOptions, String> {
    let topology_file = flag_value(flags, "--topology")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("remote-topology.json"));
    let key_file = flag_value(flags, "--key-file")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join(VALIDATOR_KEYS_FILE));
    let proposal_key_file = flag_value(flags, "--proposal-key-file")
        .map(PathBuf::from)
        .or_else(|| Some(key_file.clone()));
    let batch_kind = flag_value(flags, "--batch-kind")
        .map(str::to_string)
        .or_else(|| Some("transparent".to_string()));
    let batch_dir = flag_value(flags, "--batch-dir")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("transparent-batches"));
    let artifact_root = flag_value(flags, "--artifact-root")
        .map(PathBuf::from)
        .unwrap_or_else(|| data_dir.join("peer-certified-artifacts"));
    let processed_dir = flag_value(flags, "--processed-dir")
        .map(PathBuf::from)
        .or_else(|| Some(data_dir.join("processed-batches")));
    let max_rounds = flag_value(flags, "--max-rounds")
        .map(str::to_string)
        .unwrap_or_else(|| DEFAULT_RUN_PEER_CERTIFIED_MAX_ROUNDS.to_string())
        .parse::<usize>()
        .map_err(|_| "--max-rounds must be a usize".to_string())?;
    let start_height = match flag_value(flags, "--start-height") {
        Some(value) => value
            .parse::<u64>()
            .map_err(|_| "--start-height must be a u64".to_string())?,
        None => status(NodeOptions {
            data_dir: data_dir.clone(),
        })
        .map_err(|error| format!("peer-certified run status failed: {error}"))?
        .block_height
        .checked_add(1)
        .ok_or_else(|| "peer-certified run start height overflow".to_string())?,
    };
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
    let require_local_proposer = !flag_present(flags, "--allow-nonlocal-proposer");
    let require_signed_proposal = !flag_present(flags, "--allow-unsigned-proposal");
    let allow_peer_failures = flag_present(flags, "--allow-peer-failures");
    let quorum_early_full_propagation = flag_present(flags, "--quorum-early-full-propagation");
    let local_apply_before_certified_send =
        flag_present(flags, "--local-apply-before-certified-send");
    let defer_certified_sends = flag_present(flags, "--defer-certified-sends");

    Ok(TransportPeerCertifiedBatchLoopOptions {
        data_dir,
        topology_file,
        batch_kind,
        batch_dir,
        key_file,
        proposal_key_file,
        require_local_proposer,
        require_signed_proposal,
        allow_peer_failures,
        quorum_early_full_propagation,
        local_apply_before_certified_send,
        defer_certified_sends,
        artifact_root,
        processed_dir,
        max_rounds,
        start_height,
        poll_ms,
        timeout_ms,
        idle_timeout_ms,
        send_retries,
        retry_backoff_ms,
    })
}
fn flag_value<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
    args.windows(2)
        .find(|window| window[0] == flag)
        .map(|window| window[1].as_str())
}

fn flag_present(args: &[String], flag: &str) -> bool {
    args.iter().any(|arg| arg == flag)
}

fn require_unsafe_devnet_file_signer(flags: &[String], service: &str) -> Result<(), String> {
    if flag_present(flags, "--unsafe-devnet-file-signer") {
        return Ok(());
    }
    Err(format!(
        "{service} uses an exportable plaintext validator key file; production remote-signer/HSM custody is not implemented, so controlled-devnet operation requires the explicit --unsafe-devnet-file-signer acknowledgement"
    ))
}

fn require_unsafe_devnet_json_storage(flags: &[String], service: &str) -> Result<(), String> {
    if flag_present(flags, "--unsafe-devnet-json-storage") {
        return Ok(());
    }
    Err(format!(
        "{service} uses the bounded JSON/JSONL controlled-devnet store; production transactional indexed storage is not implemented, so long-running operation requires the explicit --unsafe-devnet-json-storage acknowledgement"
    ))
}

fn required_secret_flag(
    args: &[String],
    inline_flag: &str,
    file_flag: &str,
    label: &str,
) -> Result<Zeroizing<String>, String> {
    match (flag_value(args, inline_flag), flag_value(args, file_flag)) {
        (Some(_), Some(_)) => Err(format!(
            "{label} must use either {inline_flag} or {file_flag}, not both"
        )),
        (Some(value), None) => Ok(Zeroizing::new(value.to_string())),
        (None, Some(path)) => read_secret_file(path, label),
        (None, None) => Err(format!("missing {inline_flag} or {file_flag}")),
    }
}

fn read_secret_file(path: &str, label: &str) -> Result<Zeroizing<String>, String> {
    validate_secret_file_permissions(path, label)?;
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read {label} file `{path}`: {error}"))?;
    let value = raw.trim().to_string();
    if value.is_empty() {
        return Err(format!("{label} file `{path}` is empty"));
    }
    Ok(Zeroizing::new(value))
}

#[cfg(unix)]
fn validate_secret_file_permissions(path: &str, label: &str) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("failed to inspect {label} file `{path}`: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{label} file `{path}` is not a regular file"));
    }
    let mode = metadata.permissions().mode();
    if mode & 0o077 != 0 {
        return Err(format!(
            "{label} file `{path}` must not be group/world readable or writable"
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_secret_file_permissions(path: &str, label: &str) -> Result<(), String> {
    let metadata = std::fs::metadata(path)
        .map_err(|error| format!("failed to inspect {label} file `{path}`: {error}"))?;
    if !metadata.is_file() {
        return Err(format!("{label} file `{path}` is not a regular file"));
    }
    Ok(())
}

fn parse_csv_values(value: &str) -> Result<Vec<String>, String> {
    let values = value
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect::<Vec<_>>();
    if values.is_empty() {
        return Err("CSV value must not be empty".to_string());
    }
    Ok(values)
}

fn parse_history_options(args: &[String]) -> Result<HistoryOptions, String> {
    let data_dir = PathBuf::from(flag_value(args, "--data-dir").unwrap_or(DEFAULT_DATA_DIR));
    let mut options = HistoryOptions::with_defaults(data_dir);
    if let Some(mode) = flag_value(args, "--mode") {
        options.mode = mode.to_string();
    }
    if let Some(value) = parse_optional_u64_flag(args, "--retain-recent-blocks")? {
        options.retain_recent_blocks = value;
    }
    if let Some(value) = parse_optional_u64_flag(args, "--retain-recent-receipts")? {
        options.retain_recent_receipts = value;
    }
    if let Some(value) = parse_optional_u64_flag(args, "--retain-recent-batches")? {
        options.retain_recent_batches = value;
    }
    if let Some(value) = parse_optional_u64_flag(args, "--retain-recent-governance")? {
        options.retain_recent_governance = value;
    }
    if let Some(value) = parse_optional_u64_flag(args, "--minimum-replay-window-blocks")? {
        options.minimum_replay_window_blocks = value;
    }
    if let Some(value) = parse_optional_u64_flag(args, "--up-to-height")? {
        options.prune_up_to_height = Some(value);
    }
    if let Some(value) = flag_value(args, "--archive-handoff-file") {
        options.archive_handoff_file = Some(PathBuf::from(value));
    }
    if flag_present(args, "--automatic-prune") {
        options.advisory_prune = false;
    }
    if flag_present(args, "--archive-handoff-not-required") {
        options.archive_handoff_required = false;
    }
    Ok(options)
}

fn parse_u64_flag(args: &[String], flag: &str) -> Result<u64, String> {
    flag_value(args, flag)
        .ok_or_else(|| format!("missing {flag}"))?
        .parse::<u64>()
        .map_err(|_| format!("{flag} must be a u64"))
}

fn parse_i32_flag(args: &[String], flag: &str) -> Result<i32, String> {
    flag_value(args, flag)
        .ok_or_else(|| format!("missing {flag}"))?
        .parse::<i32>()
        .map_err(|_| format!("{flag} must be an i32"))
}

fn parse_u32_flag(args: &[String], flag: &str) -> Result<u32, String> {
    flag_value(args, flag)
        .ok_or_else(|| format!("missing {flag}"))?
        .parse::<u32>()
        .map_err(|_| format!("{flag} must be a u32"))
}

fn parse_u128_flag(args: &[String], flag: &str) -> Result<u128, String> {
    flag_value(args, flag)
        .ok_or_else(|| format!("missing {flag}"))?
        .parse::<u128>()
        .map_err(|_| format!("{flag} must be a u128"))
}

fn parse_u16_flag(args: &[String], flag: &str) -> Result<u16, String> {
    flag_value(args, flag)
        .ok_or_else(|| format!("missing {flag}"))?
        .parse::<u16>()
        .map_err(|_| format!("{flag} must be a u16"))
}

fn parse_optional_u32_flag(args: &[String], flag: &str) -> Result<Option<u32>, String> {
    flag_value(args, flag)
        .map(|value| {
            value
                .parse::<u32>()
                .map_err(|_| format!("{flag} must be a u32"))
        })
        .transpose()
}

fn parse_optional_u64_flag(args: &[String], flag: &str) -> Result<Option<u64>, String> {
    flag_value(args, flag)
        .map(|value| {
            value
                .parse::<u64>()
                .map_err(|_| format!("{flag} must be a u64"))
        })
        .transpose()
}

fn read_wallet_sign_transfer_quote_file(path: &Path) -> Result<TransferFeeQuoteReport, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read --quote-file `{}`: {error}", path.display()))?;
    let value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|error| format!("failed to parse --quote-file `{}`: {error}", path.display()))?;
    let quote_value = if value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|schema| schema == "postfiat-transfer-fee-quote-v1")
    {
        value
    } else if value.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
        value
            .get("result")
            .cloned()
            .ok_or_else(|| "--quote-file RPC response is missing result".to_string())?
    } else if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        return Err("--quote-file RPC response is not ok".to_string());
    } else {
        return Err(
            "--quote-file must be a transfer-fee-quote report or successful RPC response"
                .to_string(),
        );
    };
    let quote = serde_json::from_value::<TransferFeeQuoteReport>(quote_value).map_err(|error| {
        format!("--quote-file does not contain a valid transfer quote: {error}")
    })?;
    if quote.schema != "postfiat-transfer-fee-quote-v1" {
        return Err(format!(
            "--quote-file uses unsupported quote schema `{}`",
            quote.schema
        ));
    }
    if quote.minimum_fee == 0 {
        return Err("--quote-file minimum_fee must be nonzero".to_string());
    }
    if quote.sequence == 0 {
        return Err("--quote-file sequence must be nonzero".to_string());
    }
    Ok(quote)
}

fn read_wallet_sign_asset_transaction_quote_file(
    path: &Path,
) -> Result<AssetFeeQuoteReport, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read --quote-file `{}`: {error}", path.display()))?;
    let value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|error| format!("failed to parse --quote-file `{}`: {error}", path.display()))?;
    let quote_value = if value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|schema| schema == "postfiat-asset-fee-quote-v1")
    {
        value
    } else if value.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
        value
            .get("result")
            .cloned()
            .ok_or_else(|| "--quote-file RPC response is missing result".to_string())?
    } else if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        return Err("--quote-file RPC response is not ok".to_string());
    } else {
        return Err(
            "--quote-file must be an asset-fee-quote report or successful RPC response".to_string(),
        );
    };
    let quote = serde_json::from_value::<AssetFeeQuoteReport>(quote_value).map_err(|error| {
        format!("--quote-file does not contain a valid asset transaction quote: {error}")
    })?;
    if quote.schema != "postfiat-asset-fee-quote-v1" {
        return Err(format!(
            "--quote-file uses unsupported quote schema `{}`",
            quote.schema
        ));
    }
    if quote.minimum_fee == 0 {
        return Err("--quote-file minimum_fee must be nonzero".to_string());
    }
    if quote.sequence == 0 {
        return Err("--quote-file sequence must be nonzero".to_string());
    }
    quote
        .operation
        .validate()
        .map_err(|error| format!("--quote-file operation is invalid: {error}"))?;
    Ok(quote)
}

fn read_wallet_sign_escrow_transaction_quote_file(
    path: &Path,
) -> Result<EscrowFeeQuoteReport, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read --quote-file `{}`: {error}", path.display()))?;
    let value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|error| format!("failed to parse --quote-file `{}`: {error}", path.display()))?;
    let quote_value = if value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|schema| schema == "postfiat-escrow-fee-quote-v1")
    {
        value
    } else if value.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
        value
            .get("result")
            .cloned()
            .ok_or_else(|| "--quote-file RPC response is missing result".to_string())?
    } else if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        return Err("--quote-file RPC response is not ok".to_string());
    } else {
        return Err(
            "--quote-file must be an escrow-fee-quote report or successful RPC response"
                .to_string(),
        );
    };
    let quote = serde_json::from_value::<EscrowFeeQuoteReport>(quote_value).map_err(|error| {
        format!("--quote-file does not contain a valid escrow transaction quote: {error}")
    })?;
    if quote.schema != "postfiat-escrow-fee-quote-v1" {
        return Err(format!(
            "--quote-file uses unsupported quote schema `{}`",
            quote.schema
        ));
    }
    if quote.minimum_fee == 0 {
        return Err("--quote-file minimum_fee must be nonzero".to_string());
    }
    if quote.sequence == 0 {
        return Err("--quote-file sequence must be nonzero".to_string());
    }
    quote
        .operation
        .validate()
        .map_err(|error| format!("--quote-file operation is invalid: {error}"))?;
    Ok(quote)
}

fn read_wallet_sign_offer_transaction_quote_file(
    path: &Path,
) -> Result<OfferFeeQuoteReport, String> {
    let raw = std::fs::read_to_string(path)
        .map_err(|error| format!("failed to read --quote-file `{}`: {error}", path.display()))?;
    let value = serde_json::from_str::<serde_json::Value>(&raw)
        .map_err(|error| format!("failed to parse --quote-file `{}`: {error}", path.display()))?;
    let quote_value = if value
        .get("schema")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|schema| schema == "postfiat-offer-fee-quote-v1")
    {
        value
    } else if value.get("ok").and_then(serde_json::Value::as_bool) == Some(true) {
        value
            .get("result")
            .cloned()
            .ok_or_else(|| "--quote-file RPC response is missing result".to_string())?
    } else if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        return Err("--quote-file RPC response is not ok".to_string());
    } else {
        return Err(
            "--quote-file must be an offer-fee-quote report or successful RPC response".to_string(),
        );
    };
    let quote = serde_json::from_value::<OfferFeeQuoteReport>(quote_value).map_err(|error| {
        format!("--quote-file does not contain a valid offer transaction quote: {error}")
    })?;
    if quote.schema != "postfiat-offer-fee-quote-v1" {
        return Err(format!(
            "--quote-file uses unsupported quote schema `{}`",
            quote.schema
        ));
    }
    if quote.minimum_fee == 0 {
        return Err("--quote-file minimum_fee must be nonzero".to_string());
    }
    if quote.sequence == 0 {
        return Err("--quote-file sequence must be nonzero".to_string());
    }
    quote
        .operation
        .validate()
        .map_err(|error| format!("--quote-file operation is invalid: {error}"))?;
    Ok(quote)
}

fn require_direct_state_enabled(operation: &str) -> Result<(), String> {
    if direct_state_enabled() {
        return Ok(());
    }
    Err(format!(
        "{operation} mutates state outside ordered blocks; use ordered batch or mempool commands, or set {DIRECT_STATE_ENV}=1 for debug-only local mutation"
    ))
}

fn direct_state_enabled() -> bool {
    env::var(DIRECT_STATE_ENV)
        .map(|value| value == "1" || value.eq_ignore_ascii_case("true"))
        .unwrap_or(false)
}

fn split_csv(input: &str) -> Vec<String> {
    input
        .split(',')
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn rpc_error_id(flags: &[String]) -> String {
    if let Some(id) = flag_value(flags, "--id") {
        return id.to_string();
    }
    let Some(request_file) = flag_value(flags, "--request-file") else {
        return "local-1".to_string();
    };
    let Ok(raw) = std::fs::read_to_string(request_file) else {
        return "local-1".to_string();
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&raw) else {
        return "local-1".to_string();
    };
    value
        .get("id")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("local-1")
        .to_string()
}

fn print_rpc_success<T: serde::Serialize>(
    id: &str,
    result: &T,
    events: Vec<RpcEvent>,
) -> Result<(), String> {
    let response = success_response(id, result, events)
        .map_err(|error| format!("rpc response serialization failed: {error}"))?;
    let json = to_pretty_json(&response)
        .map_err(|error| format!("rpc response serialization failed: {error}"))?;
    println!("{json}");
    Ok(())
}

fn print_rpc_error(id: &str, code: &str, message: &str) -> Result<(), String> {
    let response = error_response(
        id,
        code,
        message,
        vec![RpcEvent::new("error", code, "rpc request failed")],
    );
    let json = to_pretty_json(&response)
        .map_err(|error| format!("rpc response serialization failed: {error}"))?;
    println!("{json}");
    Ok(())
}

fn print_usage() {
    eprintln!(
        r#"usage:
  postfiat-node init [--data-dir PATH] [--chain-id ID] [--node-id ID] [--validators N]
  postfiat-node init-consensus-v2 [--data-dir PATH] [--chain-id ID] [--node-id ID] [--validators N] --activation-height N
  postfiat-node topology [--validators N] [--base-port PORT] [--hosts CSV --rpc-base-port PORT] [--output PATH]
  postfiat-node topology-consensus-v2 [--validators N] [--base-port PORT] [--hosts CSV --rpc-base-port PORT] --activation-height N [--output PATH]
  postfiat-node validator-keys [--data-dir PATH] [--validators N]
  postfiat-node validator-key-stage [--data-dir PATH] --source-key-file PATH --validator-id NODE_ID [--source-validator-id NODE_ID] [--replace]
  postfiat-node validate-local-keys [--data-dir PATH] [--validators N] [--local-only]
  postfiat-node run --unsafe-devnet-json-storage [--data-dir PATH] [--mode once|peer-certified] [--topology PATH] [--batch-kind transparent|governance|shielded|bridge] [--batch-dir PATH] [--key-file PATH] [--proposal-key-file PATH] [--artifact-root PATH] [--processed-dir PATH] [--max-rounds N] [--start-height N] [--poll-ms MS] [--timeout-ms MS] [--idle-timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--allow-nonlocal-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends]
  postfiat-node status [--data-dir PATH] [--expect-height N]
  postfiat-node history-status [--data-dir PATH] [--mode partial|archive|full] [--retain-recent-blocks N] [--retain-recent-receipts N] [--retain-recent-batches N] [--retain-recent-governance N] [--minimum-replay-window-blocks N] [--automatic-prune] [--archive-handoff-not-required]
  postfiat-node history-prune-plan [--data-dir PATH] [--up-to-height N] [--archive-handoff-file PATH] [--retain-recent-blocks N] [--minimum-replay-window-blocks N] [--archive-handoff-not-required]
  postfiat-node history-can-prune [--data-dir PATH] [--up-to-height N] [--archive-handoff-file PATH] [--retain-recent-blocks N] [--minimum-replay-window-blocks N] [--archive-handoff-not-required]
  postfiat-node history-prune [--data-dir PATH] --up-to-height N --archive-handoff-file PATH [--retain-recent-blocks N] [--minimum-replay-window-blocks N]
  postfiat-node history-prune-recover [--data-dir PATH]
  postfiat-node history-checkpoint-rebuild-from-archive [--data-dir PATH] --backup-file PATH  # offline only; rebuilds v1 from imported archive windows
  postfiat-node history-archive-handoff-create [--data-dir PATH] --from-height N --to-height N [--archive-uri URI] --output PATH [--overwrite]
  postfiat-node history-archive-handoff-verify [--data-dir PATH] --proof-file PATH
  postfiat-node archive-export-window [--data-dir PATH] --from-height N --to-height N [--archive-uri URI] --output PATH [--overwrite]
  postfiat-node archive-window-verify --bundle-file PATH
  postfiat-node archive-window-import [--data-dir PATH] --bundle-file PATH [--overwrite]
  postfiat-node archive-window-backfill [--data-dir PATH] --source-host HOST --source-rpc-port PORT --from-height N --to-height N [--archive-uri URI] [--work-dir PATH] [--timeout-ms MS] [--overwrite]
  postfiat-node block-proposer [--data-dir PATH] --height N [--view N]
  postfiat-node transport-listen [--data-dir PATH] --topology PATH [--bind-host HOST] [--max-peers N] [--timeout-ms MS]
  postfiat-node transport-dial [--data-dir PATH] --topology PATH --to NODE_ID [--timeout-ms MS]
  postfiat-node transport-batch-listen [--data-dir PATH] --topology PATH [--bind-host HOST] [--max-peers N] [--timeout-ms MS]
  postfiat-node transport-batch-serve [--data-dir PATH] --topology PATH [--bind-host HOST] [--max-batches N] [--timeout-ms MS] [--event-log PATH]
  postfiat-node transport-batch-send [--data-dir PATH] --topology PATH --to NODE_ID [--batch-kind transparent|governance|shielded|bridge] --batch-file PATH --certificate-file PATH [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS]
  postfiat-node transport-block-vote-listen [--unsafe-devnet-file-signer] [--unsafe-devnet-json-storage] [--data-dir PATH] --topology PATH --key-file PATH --vote-dir PATH [--bind-host HOST] [--max-requests N] [--timeout-ms MS] [--allow-unsigned-proposal]
  postfiat-node transport-validator-serve [--unsafe-devnet-file-signer] [--unsafe-devnet-json-storage] [--data-dir PATH] --topology PATH --key-file PATH --vote-dir PATH [--bind-host HOST] [--max-connections N] [--timeout-ms MS] [--event-log PATH] [--allow-unsigned-proposal]
  postfiat-node transport-block-vote-request [--data-dir PATH] --topology PATH --to NODE_ID [--batch-kind transparent|governance|shielded|bridge] --batch-file PATH --proposal-file PATH [--timeout-certificate-file PATH] --vote-file PATH --height N [--timeout-ms MS]
  postfiat-node transport-certified-batch-round [--data-dir PATH] --topology PATH [--batch-kind transparent|governance|shielded|bridge] --batch-file PATH --validator-key-dir PATH --artifact-dir PATH [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS]
  postfiat-node transport-peer-certified-batch-round [--data-dir PATH] --topology PATH [--batch-kind transparent|governance|shielded|bridge] --batch-file PATH --key-file PATH [--proposal-key-file PATH] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] --artifact-dir PATH [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS]
  postfiat-node transport-peer-certified-mempool-round [--data-dir PATH] --topology PATH --key-file PATH [--proposal-key-file PATH] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] --artifact-dir PATH [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--max-transactions N] [--signed-transfer-file PATH|--signed-transfer-json JSON|--signed-asset-transaction-json JSON]
  postfiat-node pftl-submit-certified-asset-ops [--data-dir PATH] --topology PATH --key-file PATH [--proposal-key-file PATH] (--ops-file PATH | --bundle DIR [--proposer-key-file PATH] [--attestor-key-file PATH] [--finalizer-key-file PATH] [--claimer-key-file PATH] [--owner-key-file PATH]) --artifact-dir PATH [--max-transactions N] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--allow-existing-mempool] [--resume|--overwrite] [--prepare-only|--batch-only]
  postfiat-node pftl-certified-asset-ops-from-bundle --bundle DIR --output PATH [--proposer-key-file PATH] [--attestor-key-file PATH] [--finalizer-key-file PATH] [--claimer-key-file PATH] [--owner-key-file PATH] [--overwrite]
  postfiat-node nav-roundtrip-live-demo [--data-dir PATH] --topology PATH --key-file PATH [--proposal-key-file PATH] --artifact-dir PATH --source-rpc-url URL [--cast-bin PATH] [--stakehub-home PATH] [--source-chain-id N] --vault ADDRESS --verifier ADDRESS --usdc ADDRESS --stakehub-wallet ADDRESS --nav-asset ASSET_ID --pfusdc ASSET_ID --policy-hash HASH --pftl-recipient ACCOUNT [--subscriber ACCOUNT] [--owner ACCOUNT] --proposer ACCOUNT [--attestor ACCOUNT] --finalizer ACCOUNT --claimer ACCOUNT --proposer-key-file PATH [--attestor-key-file PATH] --finalizer-key-file PATH --claimer-key-file PATH --issuer-key-file PATH --owner-key-file PATH [--settlement-key-file PATH] [--submitter-key-file PATH] --amount-atoms N --mint-amount N --nonce BYTES32 --session-id ID [--signatures-file PATH|--withdrawal-signer-key-file PATH] [--destination-ref REF] --expires-at-height N [--source-proof-kind KIND --source-proof-hash HASH [--source-public-values-hash HASH]] [--min-gas-wei N] [--pftl-finalized-height N] [--challenge-wait-secs N] [--agent-timeout-secs N] [--require-warm-usdc-allowance] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--allow-existing-mempool] [--fast-demo-preflight] [--background-audit] [--reuse-final-certified-state] [--same-round-nav-exit] [--batch-only] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --pftl-only [--data-dir PATH] --topology PATH --key-file PATH [--proposal-key-file PATH] --artifact-dir PATH --nav-asset ASSET_ID --pfusdc ASSET_ID [--subscriber ACCOUNT] [--owner ACCOUNT] --issuer-key-file PATH --owner-key-file PATH [--submitter-key-file PATH] --mint-amount N [--settlement-amount-atoms N] [--settlement-receipt-id ID] [--settlement-supply-allocation-id ID] [--destination-ref REF] [--same-round-nav-exit] [--require-local-proposer] [--allow-unsigned-proposal] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--allow-existing-mempool] [--fast-demo-preflight] [--background-audit] [--reuse-final-certified-state] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --fleet-preflight-only [--data-dir PATH] --topology PATH --artifact-dir PATH [--timeout-ms MS] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --preflight-only [--data-dir PATH] --artifact-dir PATH --source-rpc-url URL [--cast-bin PATH] --vault ADDRESS --verifier ADDRESS --usdc ADDRESS --stakehub-wallet ADDRESS --amount-atoms N [--min-gas-wei N] [--overwrite]
  postfiat-node nav-roundtrip-live-demo --warm-usdc-allowance-only --artifact-dir PATH --source-rpc-url URL [--cast-bin PATH] [--stakehub-home PATH] [--source-chain-id N] --vault ADDRESS --verifier ADDRESS --usdc ADDRESS --stakehub-wallet ADDRESS --required-allowance-atoms N --session-id ID [--agent-timeout-secs N] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --evm-deposit-only --artifact-dir PATH --source-rpc-url URL [--cast-bin PATH] [--stakehub-home PATH] [--source-chain-id N] --vault ADDRESS --usdc ADDRESS --stakehub-wallet ADDRESS --pftl-recipient ACCOUNT --amount-atoms N --nonce BYTES32 --session-id ID [--agent-timeout-secs N] [--require-warm-usdc-allowance] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --deposit-relay-only [--data-dir PATH] --topology PATH --key-file PATH [--proposal-key-file PATH] --artifact-dir PATH --evm-deposit-report PATH --source-rpc-url URL [--cast-bin PATH] --vault ADDRESS --usdc ADDRESS --pfusdc ASSET_ID --policy-hash HASH --proposer ACCOUNT [--attestor ACCOUNT] --finalizer ACCOUNT --claimer ACCOUNT --proposer-key-file PATH [--attestor-key-file PATH] --finalizer-key-file PATH --claimer-key-file PATH [--issuer-key-file PATH] --expires-at-height N [--source-proof-kind KIND --source-proof-hash HASH [--source-public-values-hash HASH]] [--claim-deposit] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--allow-existing-mempool] [--prepare-only|--batch-only] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --primary-mint-only [--data-dir PATH] --topology PATH --key-file PATH [--proposal-key-file PATH] --artifact-dir PATH [--deposit-relay-report PATH] --nav-asset ASSET_ID --pfusdc ASSET_ID --subscriber ACCOUNT --issuer-key-file PATH [--subscriber-key-file PATH --consume-issued-settlement [--settlement-supply-allocation-id ID]] --mint-amount N [--settlement-receipt-id ID] [--settlement-amount-atoms N] [--nav-epoch N] [--nav-reserve-packet-hash HASH] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--allow-existing-mempool] [--prepare-only|--batch-only] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --nav-checkpoint-only [--data-dir PATH] --topology PATH --key-file PATH [--proposal-key-file PATH] --artifact-dir PATH --nav-asset ASSET_ID --issuer-key-file PATH [--submitter-key-file PATH] [--nav-epoch N] [--expected-vna-delta N] [--reserve-packet-hash HASH] [--attestor-root HASH] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--allow-existing-mempool] [--prepare-only] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --nav-exit-only [--data-dir PATH] --topology PATH --key-file PATH [--proposal-key-file PATH] --artifact-dir PATH --primary-mint-report PATH --nav-asset ASSET_ID --pfusdc ASSET_ID [--owner ACCOUNT] --owner-key-file PATH --issuer-key-file PATH [--amount N] [--settlement-amount-atoms N] [--settlement-receipt-hash HASH] [--redemption-id ID] [--same-round-nav-exit] [--nav-epoch N] [--nav-reserve-packet-hash HASH] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--allow-existing-mempool] [--prepare-only|--batch-only] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --burn-to-redeem-only [--data-dir PATH] --topology PATH --key-file PATH [--proposal-key-file PATH] --artifact-dir PATH --nav-exit-report PATH --pfusdc ASSET_ID [--owner ACCOUNT] --owner-key-file PATH --destination-ref evm-erc20:CHAIN_ID:ADDRESS [--amount-atoms N] [--issuer ACCOUNT] [--bucket-id BUCKET_ID] [--epoch N] [--reserve-packet-hash HASH] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--allow-existing-mempool] [--prepare-only|--batch-only] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --evm-withdrawal-only [--data-dir PATH] --artifact-dir PATH --burn-to-redeem-report PATH --source-rpc-url URL [--cast-bin PATH] [--stakehub-home PATH] [--source-chain-id N] --vault ADDRESS --verifier ADDRESS --usdc ADDRESS --stakehub-wallet ADDRESS --pfusdc ASSET_ID (--signatures-file PATH|--withdrawal-signer-key-file PATH) --session-id ID [--redemption-id ID] [--pftl-finalized-height N] [--challenge-wait-secs N] [--agent-timeout-secs N] [--resume|--overwrite]
  postfiat-node nav-roundtrip-live-demo --pftl-settle-only [--data-dir PATH] --topology PATH --key-file PATH [--proposal-key-file PATH] --artifact-dir PATH --evm-withdrawal-report PATH --pfusdc ASSET_ID --settlement-key-file PATH [--issuer-or-redemption-account ACCOUNT] [--settlement-receipt-hash HASH] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] [--height N] [--view N] [--timeout-certificate-file PATH] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--allow-existing-mempool] [--prepare-only|--batch-only] [--resume|--overwrite]
  postfiat-node nav-roundtrip-dashboard-status --summary PATH [--report PATH]
  postfiat-node nav-roundtrip-benchmark-base-args --summary PATH --output PATH --key-file PATH [--proposal-key-file PATH] --proposer-key-file PATH [--attestor-key-file PATH] --finalizer-key-file PATH --claimer-key-file PATH --issuer-key-file PATH --owner-key-file PATH [--settlement-key-file PATH] [--submitter-key-file PATH] --withdrawal-signer-key-file PATH --nonce-base BYTES32 [--session-id-base ID] [--data-dir PATH] [--topology PATH] [--destination-ref REF] [--timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS] [--agent-timeout-secs N] [--min-gas-wei N] [--report PATH] [--overwrite]
  postfiat-node nav-roundtrip-benchmark-plan --base-args-file PATH --benchmark-dir DIR [--phase phase1|phase2|phase3] [--replay-corpus-file PATH|--replay-corpus-dir DIR] [--require-candidate-classes CSV] [--run-count N] [--run-prefix PREFIX] [--binary CMD] [--max-median-ms MS] [--max-p90-ms MS] [--report PATH] [--overwrite]
  postfiat-node nav-roundtrip-benchmark-verify --phase phase1|phase2|phase3 (--summary PATH|--benchmark-dir DIR) [--replay-corpus-file PATH|--replay-corpus-dir DIR] [--require-candidate-classes CSV] [--report PATH] [--min-clean-runs N] [--max-median-ms MS] [--max-p90-ms MS] [--strict]
  postfiat-node nav-roundtrip-replay-corpus-verify (--corpus-file PATH|--corpus-dir DIR) [--report PATH] [--require-live-compression-ready] [--require-candidate-classes CSV] [--strict]
  postfiat-node transport-certified-batch-loop --unsafe-devnet-json-storage [--data-dir PATH] --topology PATH [--batch-kind transparent|governance|shielded|bridge] --batch-dir PATH --validator-key-dir PATH --artifact-root PATH [--processed-dir PATH] [--max-rounds N] [--start-height N] [--poll-ms MS] [--timeout-ms MS] [--idle-timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS]
  postfiat-node transport-peer-certified-batch-loop --unsafe-devnet-json-storage [--data-dir PATH] --topology PATH [--batch-kind transparent|governance|shielded|bridge] --batch-dir PATH --key-file PATH [--proposal-key-file PATH] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] --artifact-root PATH [--processed-dir PATH] [--max-rounds N] [--start-height N] [--poll-ms MS] [--timeout-ms MS] [--idle-timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS]
  postfiat-node transport-certified-send-outbox-resume [--data-dir PATH] --topology PATH [--max-jobs N]
  postfiat-node transport-peer-certified-private-egress-loop --unsafe-devnet-json-storage [--data-dir PATH] --topology PATH --egress-dir PATH --batch-dir PATH --key-file PATH [--proposal-key-file PATH] [--require-local-proposer] [--allow-unsigned-proposal] [--allow-peer-failures] [--quorum-early-full-propagation] [--local-apply-before-certified-send] [--defer-certified-sends] --artifact-root PATH [--ready-file PATH] [--processed-egress-dir PATH] [--processed-batch-dir PATH] [--max-rounds N] [--start-height N] [--poll-ms MS] [--timeout-ms MS] [--idle-timeout-ms MS] [--send-retries N] [--retry-backoff-ms MS]
  Plaintext transport listeners must bind only to loopback or a private overlay address; public and wildcard binds are rejected.
  Resident transport readiness markers: set POSTFIAT_TRANSPORT_VALIDATOR_READY_FILE or POSTFIAT_TRANSPORT_BLOCK_VOTE_READY_FILE to write a JSON ready file after prewarm and bind, before accepting client traffic.
  postfiat-node rpc-serve --unsafe-devnet-json-storage [--data-dir PATH] [--spool-dir PATH] [--ready-file PATH] --port PORT [--bind-host HOST] [--max-requests N] [--timeout-ms MS] [--child-timeout-ms MS] [--event-log PATH] [--allow-mempool-submit] [--allow-mempool-submit-finality] [--finality-topology PATH] [--finality-key-file PATH] [--finality-proposal-key-file PATH] [--finality-artifact-root PATH] [--finality-timeout-ms MS] [--finality-send-retries N] [--finality-retry-backoff-ms MS] [--finality-quorum-early-full-propagation] [--max-mempool-submit-per-peer N] [--max-mempool-submit-total N] [--allow-orchard-batch-create] [--max-orchard-batch-create-per-peer N] [--max-orchard-batch-create-total N] [--max-orchard-batch-create-concurrent N] [--disable-owned-lane] [--keep-alive]
  postfiat-node rpc-catch-up [--data-dir PATH] --source-host HOST --source-rpc-port PORT [--work-dir PATH] [--max-blocks N] [--timeout-ms MS]
  postfiat-node rpc-catch-up-certified-delta [--data-dir PATH] --source-host HOST --source-rpc-port PORT --expected-height N --expected-block-hash HASH --expected-state-root HASH [--work-dir PATH] [--max-blocks N] [--timeout-ms MS]
  postfiat-node metrics [--data-dir PATH]
  postfiat-node export-envelope-bundle [--data-dir PATH] --asset-id ASSET_ID --epoch N --bundle DIR [--overwrite]
  postfiat-node replay-envelope --bundle DIR
  postfiat-node market-ops-status [--data-dir PATH] --asset-id ASSET_ID [--epoch N]
  postfiat-node market-ops-operation-bundle [--data-dir PATH] --asset-id ASSET_ID [--issuer ACCOUNT] [--epoch N] --policy-file PATH --policy-inputs-file PATH --bundle DIR --evm-chain-id N --adapter-address ADDRESS --vault-address ADDRESS --mint-controller-address ADDRESS --funded-alignment-reserve-usd-e8 N --data-window-start UNIX --data-window-end UNIX --valid-after UNIX --expires-at UNIX [--encoding-version N] [--discount-trigger-bps N] [--premium-trigger-bps N] [--cooldown-seconds N] [--nonce BYTES32] [--previous-market-state-hash BYTES32] [--overwrite]
  postfiat-node navcoin-bridge-routes [--data-dir PATH]
  postfiat-node navcoin-bridge-packet [--data-dir PATH] --route-id ID --packet-hash HASH
  postfiat-node navcoin-bridge-claims [--data-dir PATH] --route-id ID [--limit N] [--include-terminal]
  postfiat-node navcoin-bridge-supply-status [--data-dir PATH] --route-id ID
  postfiat-node navcoin-bridge-receipt-replay [--data-dir PATH] --route-id ID
  postfiat-node navcoin-bridge-route-init [--data-dir PATH] --config-file PATH --ethereum-chain-id N --latest-finalized-nav-epoch N --return-finality-blocks N [--replace]
  postfiat-node navcoin-bridge-primary-subscribe [--data-dir PATH] --request-file PATH
  postfiat-node navcoin-bridge-export-debit [--data-dir PATH] --request-file PATH
  postfiat-node navcoin-bridge-destination-consume [--data-dir PATH] --route-id ID --packet-hash HASH
  postfiat-node navcoin-bridge-refund-source [--data-dir PATH] --route-id ID --request-file PATH
  postfiat-node navcoin-bridge-return-burn-request [--data-dir PATH] --route-id ID --ethereum-sender ADDRESS --pftl-recipient ACCOUNT --amount-atoms N --return-nonce BYTES32 --burn-height N --output-file PATH [--overwrite]
  postfiat-node navcoin-bridge-record-return-burn [--data-dir PATH] --route-id ID --request-file PATH
  postfiat-node navcoin-bridge-import-return [--data-dir PATH] --route-id ID --burn-event-hash HASH --pftl-recipient ACCOUNT
  postfiat-node vault-bridge-status [--data-dir PATH] --asset-id ASSET_ID
  postfiat-node vault-bridge-conservation-audit [--data-dir PATH] --asset-id ASSET_ID --source-rpc-url URL [--cast-bin PATH]
  postfiat-node vault-bridge-receipts [--data-dir PATH] --asset-id ASSET_ID [--bucket-id BUCKET_ID]
  postfiat-node vault-bridge-asset-id --pftl-chain-id CHAIN_ID --issuer ACCOUNT --asset-code CODE [--asset-version N] [--env-file PATH --overwrite]
  postfiat-node vault-bridge-bootstrap-bundle --pftl-chain-id CHAIN_ID --source-chain-id CHAIN_ID --vault-address ADDRESS --token-address ADDRESS --issuer ACCOUNT --asset-code CODE --asset-precision N --valuation-unit UNIT --valuation-policy-hash HASH --bundle DIR [--reserve-operator ACCOUNT --redemption-account ACCOUNT --asset-version N --asset-display-name NAME --max-supply N --verifier-kind KIND --max-snapshot-age-blocks N --challenge-window-blocks N --max-epoch-gap-blocks N --settle-deadline-blocks N --min-challenge-bond N --min-attestations N --tolerance-bp N --bridge-observer-min-confirmations N --vault-bridge-route-policy-hash HASH --sp1-program-vkey VKEY --sp1-proof-encoding ENCODING --max-proof-bytes N --max-public-values-bytes N --trust-accounts ACCOUNT[,ACCOUNT...] --trust-limit N --trust-reserve-paid N --overwrite]
  postfiat-node vault-bridge-deposit-intent --source-chain-id CHAIN_ID --vault-address ADDRESS --token-address ADDRESS --depositor ADDRESS --amount-atoms AMOUNT --pftl-recipient ACCOUNT --nonce BYTES32 --asset-id ASSET_ID --policy-hash ROUTE_PROFILE_HASH --route-epoch EPOCH [--proposer ACCOUNT --expires-at-height HEIGHT --bundle DIR]
  postfiat-node vault-bridge-deposit-plan (--log-file EVM_LOG_JSON | --receipt-file EVM_RECEIPT_JSON [--vault-address ADDRESS] [--token-address ADDRESS]) --asset-id ASSET_ID --policy-hash HASH --proposer ADDRESS [--finalizer ADDRESS] [--claimer ADDRESS] [--attestor ADDRESS] --expires-at-height HEIGHT [--source-proof-kind KIND [--source-proof-hash HASH] [--source-public-values-hash HASH] [--source-proof-file PATH --source-public-values-file PATH]]
  postfiat-node vault-bridge-deposit-relay-bundle (--log-file EVM_LOG_JSON | --receipt-file EVM_RECEIPT_JSON [--vault-address ADDRESS] [--token-address ADDRESS]) --asset-id ASSET_ID --policy-hash HASH --proposer ADDRESS [--finalizer ADDRESS] [--claimer ADDRESS] [--attestor ADDRESS] --expires-at-height HEIGHT --bundle DIR [--overwrite] [--source-proof-kind KIND [--source-proof-hash HASH] [--source-public-values-hash HASH] [--source-proof-file PATH --source-public-values-file PATH]]
  postfiat-node vault-bridge-deposit-relay-rpc-bundle --source-rpc-url URL --tx-hash HASH [--cast-bin PATH] [--vault-address ADDRESS] [--token-address ADDRESS] --asset-id ASSET_ID --policy-hash HASH --proposer ADDRESS [--finalizer ADDRESS] [--claimer ADDRESS] [--attestor ADDRESS] --expires-at-height HEIGHT --bundle DIR [--overwrite] [--source-proof-kind KIND [--source-proof-hash HASH] [--source-public-values-hash HASH] [--source-proof-file PATH --source-public-values-file PATH]]
  postfiat-node vault-bridge-burn-to-redeem-bundle [--data-dir PATH] --owner ACCOUNT --asset-id ASSET_ID --amount-atoms AMOUNT --destination-ref evm-erc20:CHAIN_ID:ADDRESS --bundle DIR [--issuer ACCOUNT] [--bucket-id BUCKET_ID] [--epoch N] [--reserve-packet-hash HASH] [--overwrite]
  postfiat-node vault-bridge-withdrawal-plan [--data-dir PATH] --asset-id ASSET_ID --redemption-id ID [--pftl-finalized-height HEIGHT] [--evm-chain-id CHAIN_ID --verifier-address ADDRESS] [--signatures-file SIGNATURES_JSON]
  postfiat-node vault-bridge-withdrawal-signature-bundle [--data-dir PATH] --asset-id ASSET_ID --redemption-id ID --evm-chain-id CHAIN_ID --verifier-address ADDRESS --bundle DIR [--relay-bundle DIR] [--overwrite] [--pftl-finalized-height HEIGHT]
  postfiat-node vault-bridge-withdrawal-relay-bundle [--data-dir PATH] --asset-id ASSET_ID --redemption-id ID --bundle DIR [--overwrite] [--pftl-finalized-height HEIGHT] [--evm-chain-id CHAIN_ID --verifier-address ADDRESS] [--signatures-file SIGNATURES_JSON]
  postfiat-node vault-bridge-export-reserve-packet [--data-dir PATH] --asset-id ASSET_ID --epoch N --bundle DIR [--overwrite]
  postfiat-node vault-bridge-replay-reserve-packet --bundle DIR
  postfiat-node faucet [--data-dir PATH]
  postfiat-node wallet-keygen [--chain-id ID] (--master-seed-hex HEX | --master-seed-hex-file PATH) [--account-index N] --key-file PATH --backup-file PATH [--overwrite]
  postfiat-node wallet-restore --backup-file PATH --key-file PATH [--overwrite]
  postfiat-node wallet-sign-transfer --key-file PATH --quote-file QUOTE_JSON
  postfiat-node wallet-sign-transfer --key-file PATH --chain-id ID --genesis-hash HASH --protocol-version N --to ADDRESS --amount AMOUNT --fee FEE --sequence N
  postfiat-node wallet-sign-asset-transaction --key-file PATH --quote-file ASSET_QUOTE_JSON
  postfiat-node wallet-sign-asset-transaction --key-file PATH --chain-id ID --genesis-hash HASH --protocol-version N --fee FEE --sequence N --operation-json JSON
  postfiat-node wallet-sign-escrow-transaction --key-file PATH --quote-file ESCROW_QUOTE_JSON
  postfiat-node wallet-sign-escrow-transaction --key-file PATH --chain-id ID --genesis-hash HASH --protocol-version N --fee FEE --sequence N --operation-json JSON
  postfiat-node wallet-sign-offer-transaction --key-file PATH --quote-file OFFER_QUOTE_JSON
  postfiat-node wallet-sign-offer-transaction --key-file PATH --chain-id ID --genesis-hash HASH --protocol-version N --fee FEE --sequence N --operation-json JSON
  postfiat-node wallet-test-vector [--chain-id ID] [--validators N] (--master-seed-hex HEX | --master-seed-hex-file PATH) [--account-index N] --to ADDRESS --amount AMOUNT [--sequence N] [--signature-seed-hex HEX | --signature-seed-hex-file PATH]
  postfiat-node orchard-test-vector [--chain-id ID] [--validators N] [--spending-key-seed-hex HEX] [--value N] [--fee N] [--build-seed-hex HEX] [--proof-seed-hex HEX] [--signature-seed-hex HEX] [--external-binding-hash HEX | --no-external-binding]
  postfiat-node transfer [--data-dir PATH] [--key-file PATH] --to ADDRESS --amount AMOUNT
  postfiat-node batch-transfer [--data-dir PATH] [--key-file PATH] --to ADDRESS --amount AMOUNT --batch-file PATH
  postfiat-node mempool-submit-transfer [--data-dir PATH] [--key-file PATH] --to ADDRESS --amount AMOUNT
  postfiat-node mempool-submit-signed-transfer [--data-dir PATH] --transfer-file PATH
  postfiat-node mempool-submit-signed-payment-v2 [--data-dir PATH] --signed-payment-v2-json JSON
  postfiat-node mempool-submit-signed-asset-transaction [--data-dir PATH] --signed-asset-transaction-json JSON
  postfiat-node mempool-submit-signed-escrow-transaction [--data-dir PATH] --signed-escrow-transaction-json JSON
  postfiat-node mempool-submit-signed-nft-transaction [--data-dir PATH] --signed-nft-transaction-json JSON
  postfiat-node mempool-submit-signed-offer-transaction [--data-dir PATH] --signed-offer-transaction-json JSON
  postfiat-node mempool-batch [--data-dir PATH] --batch-file PATH [--max-transactions N]
  postfiat-node signed-asset-batch [--data-dir PATH] --batch-file PATH --signed-asset-transaction-files CSV
  postfiat-node mempool-status [--data-dir PATH]
  postfiat-node asset-fee-quote [--data-dir PATH] --source ADDRESS --operation-json JSON [--sequence N]
  postfiat-node asset-info [--data-dir PATH] --asset-id ID
  postfiat-node account-lines [--data-dir PATH] --account ADDRESS [--issuer ADDRESS] [--asset-id ID] [--limit N]
  postfiat-node account-assets [--data-dir PATH] --account ADDRESS [--asset-id ID] [--limit N]
  postfiat-node issuer-assets [--data-dir PATH] --issuer ADDRESS [--limit N]
  postfiat-node escrow-fee-quote [--data-dir PATH] --source ADDRESS --operation-json JSON [--sequence N]
  postfiat-node nft-fee-quote [--data-dir PATH] --source ADDRESS --operation-json JSON [--sequence N]
  postfiat-node offer-fee-quote [--data-dir PATH] --source ADDRESS --operation-json JSON [--sequence N]
  postfiat-node atomic-settlement-template [--data-dir PATH] --left-owner ADDRESS --left-recipient ADDRESS --left-asset-id PFT_OR_ASSET_ID --left-amount AMOUNT --right-owner ADDRESS --right-recipient ADDRESS --right-asset-id PFT_OR_ASSET_ID --right-amount AMOUNT --condition TEXT --cancel-after HEIGHT [--finish-after HEIGHT] [--left-sequence N] [--right-sequence N]
  postfiat-node escrow-info [--data-dir PATH] --escrow-id ID
  postfiat-node account-escrows [--data-dir PATH] --account ADDRESS [--role owner|recipient] [--state open|finished|canceled] [--limit N]
  postfiat-node propose-batch [--data-dir PATH] [--batch-kind transparent|governance|shielded|bridge] --batch-file PATH --proposal-file PATH [--view N] [--timeout-certificate-file PATH] [--key-file PATH] [--validator ID]
  postfiat-node apply-batch [--data-dir PATH] --batch-file PATH [--certificate-file PATH]
  postfiat-node ratify-validator-set [--data-dir PATH] --validators CSV [--support CSV] --validator-count N [--activation-height H] [--veto-until-height H] [--paused] --amendment-file PATH
  postfiat-node ratify-crypto-policy [--data-dir PATH] --validators CSV [--support CSV] --version N [--activation-height H] [--veto-until-height H] [--paused] --amendment-file PATH
  postfiat-node ratify-bridge-witness-epoch [--data-dir PATH] --validators CSV [--support CSV] --witness-epoch N [--activation-height H] [--veto-until-height H] [--paused] --amendment-file PATH
  postfiat-node ratify-authority-mode [--data-dir PATH] --validators CSV [--support CSV] --mode foundation|cobalt-ratified [--activation-height H] [--veto-until-height H] [--paused] --amendment-file PATH
  postfiat-node ratify-orchard-pool-pause [--data-dir PATH] --validators CSV [--support CSV] --state paused|resumed [--activation-height H] [--veto-until-height H] [--paused] --amendment-file PATH
  postfiat-node ratify-atomic-swap-pause [--data-dir PATH] --validators CSV [--support CSV] --state paused|resumed [--activation-height H] [--veto-until-height H] [--paused] --amendment-file PATH
  postfiat-node ratify-atomic-swap-activation-height [--data-dir PATH] --validators CSV [--support CSV] --height H [--activation-height H] [--veto-until-height H] [--paused] --amendment-file PATH
  postfiat-node ratify-replicated-state-v2-activation-height [--data-dir PATH] --validators CSV [--support CSV] --height H [--activation-height H] [--veto-until-height H] [--paused] --amendment-file PATH
  postfiat-node governance-authorization-sign [--data-dir PATH] --amendment-file PATH --validator NODE_ID --validator-key-file PATH --proposal-slot H --expires-at-height H --authorization-file PATH
  postfiat-node governance-amendment-assemble [--data-dir PATH] --amendment-file PATH --authorization-files CSV --proposal-slot H --output PATH
  postfiat-node ethereum-checkpoint-observe [--data-dir PATH] --route-id ID --ethereum-rpc http://HOST:PORT[/PATH] [--block-number N] --checkpoint-file PATH
  postfiat-node ethereum-receipt-proof-build [--data-dir PATH] --route-id ID --ethereum-rpc http://HOST:PORT[/PATH] --transaction-hash 0xHASH --proof-file PATH
  postfiat-node pfusdc-egress-witness [--data-dir PATH] --withdrawal-id 96_HEX [--prior-checkpoint 96_HEX]
  postfiat-node pfusdc-checkpoint-witness [--data-dir PATH] --prior-checkpoint 96_HEX --target-block 96_HEX
  postfiat-node ethereum-checkpoint-vote-sign [--data-dir PATH] --checkpoint-file PATH --ethereum-rpc http://HOST:PORT[/PATH] --validator NODE_ID --validator-key-file PATH --vote-file PATH
  postfiat-node ethereum-checkpoint-certificate-assemble [--data-dir PATH] --checkpoint-file PATH --vote-files CSV --certificate-file PATH
  postfiat-node validator-registry-root [--data-dir PATH] [--registry-file PATH] --validators CSV
  postfiat-node validator-registry-update [--data-dir PATH] --validators CSV [--support CSV] --activation-height N --previous-registry-root HASH --new-registry-root HASH [--previous-validators CSV] [--new-validators CSV] --operation admit|remove|suspend|reactivate|rotate_key --subject-node-id NODE_ID [--previous-record-file PATH] [--new-record-file PATH] --update-file PATH
  postfiat-node validator-registry-authorization-sign [--data-dir PATH] --update-file PATH --validator NODE_ID --validator-key-file PATH --proposal-slot H --expires-at-height H --authorization-file PATH
  postfiat-node validator-registry-update-assemble [--data-dir PATH] --update-file PATH --authorization-files CSV --proposal-slot H --output PATH
  postfiat-node validator-registry-update-verify [--data-dir PATH] --update-file PATH [--previous-registry-file PATH] [--new-registry-file PATH]
  postfiat-node validator-registry-update-apply [--data-dir PATH] --update-file PATH --current-height N [--previous-registry-file PATH] [--output-registry-file PATH]
  postfiat-node validator-registry-lifecycle-replay-verify --bundle-file PATH
  postfiat-node governance-replay-build [--data-dir PATH] [--genesis-bundle-file PATH] --previous-registry-file PATH --update-file PATH --new-registry-file PATH [--amendment-replay-bundle-file PATH] [--governance-batch-file PATH] [--post-change-block-file PATH --post-change-batch-file PATH --post-change-certificate-file PATH] --output PATH [--overwrite]
  postfiat-node governance-replay-verify [--data-dir PATH] --package-file PATH
  postfiat-node governance-amendment-replay-verify --bundle-file PATH
  postfiat-node operator-manifest-create --master-key-file PATH --chain-id ID --network NAME --validator-id NODE_ID --hot-public-key-hex HEX --operator NAME --contact CONTACT --provider-group NAME --region-group NAME --jurisdiction-group NAME --legal-domain-group NAME --funding-domain-group NAME [--rotation-state active|staged|retiring|suspended|removed] [--effective-height N] [--trust-graph-root HASH --trust-graph-version N --trust-view-id HASH --trust-view-version N] --output PATH [--overwrite]
  postfiat-node operator-manifest-verify --manifest-file PATH
  postfiat-node governance-genesis-bundle [--data-dir PATH] --manifest-dir PATH --validators CSV --quorum N --network NAME --output PATH
  postfiat-node governance-genesis-verify [--data-dir PATH] --bundle-file PATH
  postfiat-node governance-agent-gate1-5 [--agent-dir PATH] [--output PATH] [--overwrite]
  postfiat-node governance-agent-model-request [--agent-dir PATH] [--cobalt-certificate-hash HASH --round-id ID --round-domain DOMAIN] [--output PATH] [--overwrite]
  postfiat-node governance-agent-gate3-5 --outputs-dir PATH [--model-request PATH] [--expected-count N] [--cobalt-certificate-hash HASH --round-id ID --round-domain DOMAIN] [--output PATH] [--overwrite]
  postfiat-node governance-agent-gate3-6 [--agent-dir PATH] [--cobalt-certificate-hash HASH --round-id ID --round-domain DOMAIN] [--output PATH] [--overwrite]
  postfiat-node governance-agent-gate7-5 [--ruleset PATH] [--evidence PATH] [--output PATH] [--overwrite]
  postfiat-node governance-agent-gate7-6 [--ruleset PATH] [--comparison-dir PATH] [--output PATH] [--overwrite]
  postfiat-node governance-agent-gate8-5 [--agent-dir PATH] [--ruleset PATH] [--evidence PATH] [--output PATH] [--replay-bundle-output PATH] [--overwrite]
  postfiat-node governance-agent-gate9-5 [--agent-dir PATH] [--ruleset PATH] [--evidence PATH] [--output PATH] [--overwrite]
  postfiat-node governance-agent-gate10-1 [--model-request PATH] [--ruleset PATH] [--gate-9_5-report PATH] [--output PATH] [--overwrite]
  postfiat-node governance-agent-gate10-5 [--model-request PATH] [--ruleset PATH] [--gate-9_5-report PATH] [--output PATH] [--overwrite]
  postfiat-node governance-agent-gate14 [--model-request PATH] [--ruleset PATH] [--gate-9_5-report PATH] [--receipt-report PATH] [--output PATH] [--overwrite]
  postfiat-node governance-agent-gate15 [--model-request PATH] [--ruleset PATH] [--gate-9_5-report PATH] [--receipt-report PATH] [--gate-14-report PATH] [--output PATH] [--overwrite]
  postfiat-node governance-agent-evidence-lineage-audit [--model-request PATH] [--gate-3_5-report PATH] [--gate-3_6-report PATH] [--gate-10_1-report PATH] [--gate-10_5-report PATH|--receipt-report PATH] [--gate-14-report PATH] [--gate-15-report PATH]
  postfiat-node governance-agent-implementation-execution [--work-item PATH] [--output PATH] [--overwrite]
  postfiat-node apply-amendment [--data-dir PATH] --amendment-file PATH
  postfiat-node governance-batch [--data-dir PATH] [--amendment-file PATH] [--registry-update-file PATH] --batch-file PATH
  postfiat-node fastswap-governance-bootstrap [--data-dir PATH] --validators CSV [--support CSV] --payload-file PATH --amendment-file PATH --batch-file PATH [--governance-activation-height H] [--veto-until-height H] [--paused]
  postfiat-node fastswap-governance-bootstrap-assemble [--data-dir PATH] --payload-file PATH --signed-amendment-file PATH --proposal-slot H --batch-file PATH
  postfiat-node fastpay-recovery-governance-bootstrap [--data-dir PATH] --validators CSV [--support CSV] [--veto-until-height H] --payload-file PATH --amendment-file PATH --batch-file PATH
  postfiat-node fastpay-recovery-governance-bootstrap-assemble [--data-dir PATH] --payload-file PATH --signed-amendment-file PATH --proposal-slot H --batch-file PATH
  postfiat-node apply-governance-batch [--data-dir PATH] --batch-file PATH [--certificate-file PATH]
  postfiat-node account [--data-dir PATH] --address ADDRESS
  postfiat-node account-tx [--data-dir PATH] --address ADDRESS [--from-height HEIGHT] [--to-height HEIGHT] [--limit N]
  postfiat-node account-tx-index-build [--data-dir PATH]
  postfiat-node account-tx-index-status [--data-dir PATH]
  postfiat-node owned-objects [--data-dir PATH] --owner-public-key-hex HEX [--asset ASSET] [--limit N]
  postfiat-node receipts [--data-dir PATH] [--tx-id ID] [--limit N]
  postfiat-node rpc --method server_info [--data-dir PATH]
  postfiat-node rpc --method ledger [--data-dir PATH] [--limit N]
  postfiat-node rpc --method tx [--data-dir PATH] --tx-id ID [--audit-block-log]
  postfiat-node rpc --method validators [--data-dir PATH]
  postfiat-node rpc --method manifests [--data-dir PATH]
  postfiat-node rpc --method fee [--data-dir PATH]
  postfiat-node rpc --method nft_fee_quote [--data-dir PATH] --source ADDRESS --operation-json JSON [--sequence N]
  postfiat-node rpc --method offer_fee_quote [--data-dir PATH] --source ADDRESS --operation-json JSON [--sequence N]
  postfiat-node rpc --method offer_info [--data-dir PATH] --offer-id ID
  postfiat-node rpc --method account_offers [--data-dir PATH] --account ADDRESS [--state open|filled|canceled|unfunded] [--limit N]
  postfiat-node rpc --method book_offers [--data-dir PATH] --taker-gets-asset-id ASSET --taker-pays-asset-id ASSET [--limit N]
  postfiat-node rpc --method asset_info [--data-dir PATH] --asset-id ID
  postfiat-node rpc --method account_lines [--data-dir PATH] --account ADDRESS [--issuer ADDRESS] [--asset-id ID] [--limit N]
  postfiat-node rpc --method account_assets [--data-dir PATH] --account ADDRESS [--asset-id ID] [--limit N]
  postfiat-node rpc --method owned_objects [--data-dir PATH] --owner-public-key-hex HEX [--asset ASSET] [--limit N]
  postfiat-node rpc --method issuer_assets [--data-dir PATH] --issuer ADDRESS [--limit N]
  postfiat-node rpc --method escrow_info [--data-dir PATH] --escrow-id ID
  postfiat-node rpc --method account_escrows [--data-dir PATH] --account ADDRESS [--role owner|recipient] [--state open|finished|canceled] [--limit N]
  postfiat-node rpc --method nft_info [--data-dir PATH] --nft-id ID
  postfiat-node rpc --method account_nfts [--data-dir PATH] --account ADDRESS [--include-burned] [--limit N]
  postfiat-node rpc --method issuer_nfts [--data-dir PATH] --issuer ADDRESS [--collection-id ID] [--include-burned] [--limit N]
  postfiat-node blocks [--data-dir PATH] [--from-height HEIGHT] [--limit N]
  postfiat-node block-vote [--data-dir PATH] [--key-file PATH] [--validator ID] [--height N] [--proposal-file PATH --batch-file PATH] [--timeout-certificate-file PATH] --vote-file PATH [--skip-block-log-verify]
  postfiat-node block-vote-equivocation [--data-dir PATH] --first-proposal-file PATH --second-proposal-file PATH --first-vote-file PATH --second-vote-file PATH --evidence-file PATH
  postfiat-node block-vote-equivocation-verify [--data-dir PATH] --first-proposal-file PATH --second-proposal-file PATH --first-vote-file PATH --second-vote-file PATH --evidence-file PATH
  postfiat-node block-proposal-equivocation [--data-dir PATH] --first-proposal-file PATH --second-proposal-file PATH --evidence-file PATH
  postfiat-node block-proposal-equivocation-verify [--data-dir PATH] --first-proposal-file PATH --second-proposal-file PATH --evidence-file PATH
  postfiat-node block-certificate [--data-dir PATH] [--height N] [--proposal-file PATH --batch-file PATH] [--timeout-certificate-file PATH] --vote-files CSV --certificate-file PATH [--skip-block-log-verify]
  postfiat-node block-certificate-from-archive [--data-dir PATH] --block-file PATH --batch-file PATH --certificate-file PATH
  postfiat-node block-timeout-vote [--data-dir PATH] [--key-file PATH] [--validator ID] --height N --view N --high-qc ID --vote-file PATH
  postfiat-node block-timeout-certificate [--data-dir PATH] --height N --view N --vote-files CSV --certificate-file PATH
  postfiat-node block-timeout-verify [--data-dir PATH] --certificate-file PATH
  postfiat-node certify-batch [--data-dir PATH] [--batch-kind transparent|governance|shielded|bridge] --batch-file PATH --validator-key-dir PATH --proposal-file PATH --vote-dir PATH --certificate-file PATH [--height N] [--view N] [--timeout-certificate-file PATH] [--skip-block-log-verify]
  postfiat-node batch-archive [--data-dir PATH] [--batch-kind KIND] [--batch-id ID] [--limit N]
  postfiat-node verify-state [--data-dir PATH]
  postfiat-node verify-blocks [--data-dir PATH]
  postfiat-node verify-governance [--data-dir PATH] [--cobalt-mode canonical|non-uniform] [--trust-graph-root HASH]
  postfiat-node verify-bridge [--data-dir PATH]
  postfiat-node verify-mempool [--data-dir PATH]
  postfiat-node verify-shielded [--data-dir PATH]
  postfiat-node orchard-action [--data-dir PATH] --action-file PATH [--apply]
  postfiat-node orchard-operator-policy [--data-dir PATH] [--privacy-enabled] [--max-concurrent-verifiers N] [--verifier-timeout-ms MS] [--root-retention-roots N] [--indexing-role disabled|local|public]
  postfiat-node orchard-fee-resource-policy [--data-dir PATH]
  postfiat-node orchard-frontier-cache-warm [--data-dir PATH]
  postfiat-node orchard-pool-report [--data-dir PATH]
  postfiat-node orchard-output-create [--data-dir PATH] (--recipient-view-key-file PATH | --recipient-key-file PATH | --recipient-address-raw-hex HEX) --action-file PATH [--value N] [--memo-hex HEX] [--fee N] [--overwrite]
  postfiat-node orchard-deposit-create [--data-dir PATH] [--key-file PATH] (--recipient-view-key-file PATH | --recipient-key-file PATH | --recipient-address-raw-hex HEX) --deposit-file PATH --amount N [--fee N] [--memo-hex HEX] [--policy-id ID] [--disclosure-hash HEX] [--overwrite]
  postfiat-node asset-orchard-ingress-create [--data-dir PATH] --key-file PATH --asset-id ASSET --amount N --note-seed-hex HEX --ingress-file PATH --note-file PATH [--fee N] [--overwrite]
  postfiat-node asset-orchard-egress-create [--data-dir PATH] --note-file PATH --to ACCOUNT --egress-file PATH [--amount N] [--overwrite]
  postfiat-node asset-orchard-private-egress-create [--data-dir PATH] --note-file PATH --to ACCOUNT --policy-id ID --disclosure-hash HASH --egress-file PATH [--asset-id ASSET] [--amount N] [--fee 0] [--overwrite]
  postfiat-node asset-orchard-note-status [--data-dir PATH] --note-file PATH
  postfiat-node asset-orchard-scan [--data-dir PATH] --note-seed-hex HEX --note-file PATH [--overwrite]
  postfiat-node asset-orchard-swap-create [--data-dir PATH] --input-note-file-a PATH --input-note-file-b PATH --output-note-seed-hex-a HEX --output-note-seed-hex-b HEX --action-file PATH --output-note-file-a PATH --output-note-file-b PATH [--overwrite]
  postfiat-node orchard-spend-create [--data-dir PATH] (--key-file PATH | --spending-key-hex HEX) --input-output-index N (--recipient-view-key-file PATH | --recipient-key-file PATH | --recipient-address-raw-hex HEX) --action-file PATH [--amount N] [--change-recipient-view-key-file PATH | --change-recipient-key-file PATH | --change-recipient-address-raw-hex HEX] [--memo-hex HEX] [--fee N] [--overwrite]
  postfiat-node orchard-withdraw-create [--data-dir PATH] (--key-file PATH | --spending-key-hex HEX) --input-output-index N --to ADDRESS --amount N --action-file PATH [--change-recipient-view-key-file PATH | --change-recipient-key-file PATH | --change-recipient-address-raw-hex HEX] [--memo-hex HEX] [--fee N] [--policy-id ID] [--disclosure-hash HEX] [--overwrite]
  postfiat-node orchard-keygen (--master-seed-hex HEX | --master-seed-hex-file PATH) [--account-index N] --key-file PATH [--overwrite]
  postfiat-node orchard-view-key-export --key-file PATH --view-key-file PATH [--overwrite]
  postfiat-node orchard-scan [--data-dir PATH] (--view-key-file PATH | --key-file PATH | --spending-key-hex HEX)
  postfiat-node orchard-disclose [--data-dir PATH] (--view-key-file PATH | --key-file PATH | --spending-key-hex HEX) --output-index N --packet-file PATH [--overwrite]
  postfiat-node orchard-disclosure-verify [--data-dir PATH] --packet-file PATH
  postfiat-node shield-mint [--data-dir PATH] --owner OWNER --amount AMOUNT [--asset-id ASSET] [--memo MEMO]
  postfiat-node shield-spend [--data-dir PATH] --note-id NOTE --to OWNER --amount AMOUNT [--memo MEMO]
  postfiat-node shield-batch-mint [--data-dir PATH] --owner OWNER --amount AMOUNT --batch-file PATH [--asset-id ASSET] [--memo MEMO]
  postfiat-node shield-batch-spend [--data-dir PATH] --note-id NOTE --to OWNER --amount AMOUNT --batch-file PATH [--memo MEMO]
  postfiat-node shield-batch-migrate [--data-dir PATH] --note-id NOTE --target-pool POOL --batch-file PATH [--memo MEMO]
  postfiat-node shield-batch-orchard [--data-dir PATH] --action-file PATH --batch-file PATH
  postfiat-node shield-batch-orchard-deposit [--data-dir PATH] --deposit-file PATH --batch-file PATH
  postfiat-node shield-batch-asset-orchard-ingress [--data-dir PATH] --ingress-file PATH --batch-file PATH
  postfiat-node shield-batch-asset-orchard-egress [--data-dir PATH] --egress-file PATH --batch-file PATH
  postfiat-node shield-batch-asset-orchard-private-egress [--data-dir PATH] --egress-file PATH --batch-file PATH
  postfiat-node shield-batch-orchard-withdraw [--data-dir PATH] --action-file PATH --to ADDRESS --amount N --fee N --batch-file PATH [--policy-id ID] [--disclosure-hash HEX]
  postfiat-node shield-batch-swap [--data-dir PATH] --swap-file PATH --batch-file PATH
  postfiat-node apply-shield-batch [--data-dir PATH] --batch-file PATH [--certificate-file PATH]
  postfiat-node shield-scan [--data-dir PATH] --owner OWNER
  postfiat-node shield-disclose [--data-dir PATH] --note-id NOTE
  postfiat-node shield-turnstile [--data-dir PATH]
  postfiat-node shield-root [--data-dir PATH]
  postfiat-node bridge-domain [--data-dir PATH] [--domain-id ID] [--name NAME] [--source-chain ID] [--target-chain ID] [--bridge-id ID] [--door-account ID] --inbound-cap AMOUNT --outbound-cap AMOUNT
  postfiat-node bridge-transfer [--data-dir PATH] [--domain-id ID] [--direction inbound|outbound] --from SOURCE --to DEST --amount AMOUNT --witness-id ID [--witness-epoch N] [--asset-id ASSET]
  postfiat-node bridge-pause [--data-dir PATH] [--domain-id ID]
  postfiat-node bridge-resume [--data-dir PATH] [--domain-id ID]
  postfiat-node bridge-status [--data-dir PATH]
  postfiat-node bridge-batch-domain [--data-dir PATH] [--domain-id ID] [--name NAME] [--source-chain ID] [--target-chain ID] [--bridge-id ID] [--door-account ID] --inbound-cap AMOUNT --outbound-cap AMOUNT --batch-file PATH
  postfiat-node bridge-batch-transfer [--data-dir PATH] [--domain-id ID] [--direction inbound|outbound] --from SOURCE --to DEST --amount AMOUNT --witness-id ID [--witness-epoch N] --batch-file PATH [--asset-id ASSET]
  postfiat-node bridge-batch-pause [--data-dir PATH] [--domain-id ID] --batch-file PATH
  postfiat-node bridge-batch-resume [--data-dir PATH] [--domain-id ID] --batch-file PATH
  postfiat-node apply-bridge-batch [--data-dir PATH] --batch-file PATH [--certificate-file PATH]
  postfiat-node snapshot-export [--data-dir PATH] --snapshot-dir PATH
  postfiat-node snapshot-import [--data-dir PATH] --snapshot-dir PATH [--node-id ID]
  postfiat-node snapshot-publisher-key-export --publisher-key-file PATH --public-key-file PATH
  postfiat-node snapshot-export-signed [--data-dir PATH] --snapshot-dir PATH --publisher-key-file PATH
  postfiat-node snapshot-import-signed [--data-dir PATH] --snapshot-dir PATH --trusted-publisher-key-file PATH [--node-id ID]
  postfiat-node deployment-publisher-key-create --publisher-key-file PATH
  postfiat-node deployment-publisher-key-export --publisher-key-file PATH --public-key-file PATH
  postfiat-node deployment-validator-units-stage --release-id ID --topology-file PATH --binary-file PATH --swap-circuit-metadata-file PATH --private-egress-circuit-metadata-file PATH --output-dir PATH
  postfiat-node deployment-manifest-create --deployment-id ID --valid-from-unix N --valid-until-unix N --chain-id ID --genesis-hash HASH --git-revision REV --binary-file PATH --build-profile PROFILE --build-features CSV --protocol-version N --rpc-schema SCHEMA --service-unit-file PATH --environment-file PATH --validator-bindings-file PATH --topology-file PATH --swap-circuit-metadata-file PATH --private-egress-circuit-metadata-file PATH --publisher-key-file PATH --manifest-file PATH
  postfiat-node deployment-manifest-verify --manifest-file PATH --trusted-publisher-key-file PATH [--now-unix N] [--validator-id ID --validator-bindings-file PATH] [--runtime-binary-file PATH --runtime-topology-file PATH --runtime-swap-circuit-metadata-file PATH --runtime-private-egress-circuit-metadata-file PATH]
  postfiat-node rpc --method METHOD [--id ID] [--data-dir PATH] [method flags]
  postfiat-node rpc --request-file PATH [--data-dir PATH]

Direct state mutation commands require POSTFIAT_ALLOW_DIRECT_STATE=1 and are debug-only; ordered batch and mempool commands are the normal path."#
    );
}
