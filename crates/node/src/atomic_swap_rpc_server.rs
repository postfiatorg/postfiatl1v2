fn run_serialized_rpc_mempool_submit(
    request_index: u64,
    request: &RpcRequest,
    raw_request: &str,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    let _mutation_guard = match context.mempool_mutation_lock.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return rpc_serve_error_response(
                &request.id,
                "rpc_mempool_mutation_busy",
                "another mempool mutation is already in progress",
            );
        }
        Err(std::sync::TryLockError::Poisoned(_)) => {
            return rpc_serve_error_response(
                &request.id,
                "rpc_mempool_mutation_lock_poisoned",
                "mempool mutation lock poisoned",
            );
        }
    };
    let method = request.method.as_str();
    if method == RPC_FINALITY_TIMEOUT_VOTE_METHOD {
        run_rpc_serve_consensus_v2_timeout_vote(request_index, request, context)
    } else if method == "shield_batch_finality" {
        run_rpc_serve_shield_batch_finality(request_index, request, context)
    } else if method == "mempool_submit_signed_payment_v2_finality" {
        run_rpc_serve_mempool_submit_signed_payment_v2_finality(request_index, request, context)
    } else if method == "mempool_submit_signed_asset_transaction_finality" {
        run_rpc_serve_mempool_submit_signed_asset_transaction_finality(
            request_index,
            request,
            context,
        )
    } else if method == "mempool_submit_signed_atomic_swap_transaction_finality" {
        run_rpc_serve_mempool_submit_signed_atomic_swap_finality(request_index, request, context)
    } else if method == "mempool_submit_fastlane_primary_finality" {
        run_rpc_serve_mempool_submit_fastlane_primary_finality(request_index, request, context)
    } else if method == "mempool_submit_signed_escrow_transaction_finality" {
        run_rpc_serve_mempool_submit_signed_escrow_transaction_finality(
            request_index,
            request,
            context,
        )
    } else if method == "mempool_submit_signed_transfer_finality" {
        run_rpc_serve_mempool_submit_signed_transfer_finality(request_index, request, context)
    } else {
        match run_rpc_request_via_child(
            &context.data_dir,
            &context.spool_dir,
            request_index,
            raw_request,
            context.child_timeout_ms,
        ) {
            Ok(response) => response,
            Err(error) => rpc_serve_child_error_response(&request.id, &error),
        }
    }
}

fn atomic_swap_rpc_required_string(
    params: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<String, (String, String)> {
    params
        .get(key)
        .and_then(serde_json::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                format!("missing or empty {key}"),
            )
        })
}

fn atomic_swap_rpc_required_u64(
    params: &serde_json::Map<String, serde_json::Value>,
    key: &str,
) -> Result<u64, (String, String)> {
    params
        .get(key)
        .and_then(serde_json::Value::as_u64)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                format!("missing or invalid {key}"),
            )
        })
}

fn atomic_swap_rpc_quote_leg(
    params: &serde_json::Map<String, serde_json::Value>,
    prefix: &str,
) -> Result<AtomicSwapQuoteLegInput, (String, String)> {
    Ok(AtomicSwapQuoteLegInput {
        owner: atomic_swap_rpc_required_string(params, &format!("{prefix}_owner"))?,
        recipient: atomic_swap_rpc_required_string(params, &format!("{prefix}_recipient"))?,
        issuer: atomic_swap_rpc_required_string(params, &format!("{prefix}_issuer"))?,
        asset_id: atomic_swap_rpc_required_string(params, &format!("{prefix}_asset_id"))?,
        amount: atomic_swap_rpc_required_u64(params, &format!("{prefix}_amount"))?,
    })
}

fn run_rpc_serve_atomic_swap_fee_quote(
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    match run_rpc_serve_atomic_swap_fee_quote_inner(request, context) {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

fn run_rpc_serve_atomic_swap_fee_quote_inner(
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> Result<RpcResponse, (String, String)> {
    let params = request.params.as_object().ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "atomic_swap_fee_quote params must be an object".to_string(),
        )
    })?;
    let report = atomic_swap_fee_quote(AtomicSwapFeeQuoteOptions {
        data_dir: context.data_dir.clone(),
        rfq_hash: atomic_swap_rpc_required_string(params, "rfq_hash")?,
        market_envelope_hash: atomic_swap_rpc_required_string(params, "market_envelope_hash")?,
        nav_epoch: atomic_swap_rpc_required_u64(params, "nav_epoch")?,
        expires_at_height: atomic_swap_rpc_required_u64(params, "expires_at_height")?,
        swap_nonce: atomic_swap_rpc_required_string(params, "swap_nonce")?,
        leg_0: atomic_swap_rpc_quote_leg(params, "leg_0")?,
        leg_1: atomic_swap_rpc_quote_leg(params, "leg_1")?,
    })
    .map_err(atomic_swap_fee_quote_rpc_error)?;
    success_response(
        &request.id,
        &report,
        vec![RpcEvent::new(
            "atomic_swap_fee_quote",
            report.unsigned_transaction.rfq_hash.clone(),
            "atomic swap fee quoted for both owners",
        )],
    )
    .map_err(|error| {
        (
            "rpc_atomic_swap_fee_quote_failed".to_string(),
            format!("atomic_swap_fee_quote response serialization failed: {error}"),
        )
    })
}

fn atomic_swap_fee_quote_rpc_error(error: std::io::Error) -> (String, String) {
    let code = atomic_swap_fee_quote_typed_error_code(&error)
        .unwrap_or("rpc_atomic_swap_fee_quote_failed")
        .to_string();
    (code, format!("rpc atomic_swap_fee_quote failed: {error}"))
}

fn run_rpc_serve_mempool_submit_signed_atomic_swap_finality(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    match run_rpc_serve_mempool_submit_signed_atomic_swap_finality_inner(
        request_index,
        request,
        context,
    ) {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

fn run_rpc_serve_mempool_submit_signed_atomic_swap_finality_inner(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> Result<RpcResponse, (String, String)> {
    let params = request.params.as_object().ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "mempool_submit_signed_atomic_swap_transaction_finality params must be an object"
                .to_string(),
        )
    })?;
    let signed_json =
        atomic_swap_rpc_required_string(params, "signed_atomic_swap_transaction_json")?;
    let _finality_guard = match context.finality_submit_lock.try_lock() {
        Ok(guard) => guard,
        Err(std::sync::TryLockError::WouldBlock) => {
            return Err((
                "rpc_finality_submit_busy".to_string(),
                "another finality submit is already in progress".to_string(),
            ));
        }
        Err(std::sync::TryLockError::Poisoned(_)) => {
            return Err((
                "rpc_finality_submit_lock_poisoned".to_string(),
                "finality submit lock poisoned".to_string(),
            ));
        }
    };

    let request_component = transport_artifact_component(&request.id);
    let artifact_dir = context
        .finality_artifact_root
        .join(format!("rpc-finality-{request_index}-{request_component}"));
    let total_start = Instant::now();
    let readiness_wait_ms = wait_for_rpc_finality_required_parent(context, params)?;
    let finality_view = prepare_rpc_finality_view(&context.data_dir, params, &artifact_dir)?;
    require_rpc_finality_local_proposer(context, finality_view.view)?;
    let required_parent = RequiredBlockParent {
        height: atomic_swap_rpc_required_u64(params, "proxy_required_current_height")?,
        block_hash: atomic_swap_rpc_required_string(params, "proxy_required_parent_hash")?,
        state_root: atomic_swap_rpc_required_string(params, "proxy_required_state_root")?,
    };
    let round = transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
        data_dir: context.data_dir.clone(),
        topology_file: context.finality_topology_file.clone(),
        key_file: context.finality_key_file.clone(),
        proposal_key_file: context.finality_proposal_key_file.clone(),
        require_local_proposer: true,
        require_signed_proposal: true,
        allow_peer_failures: false,
        quorum_early_full_propagation: context.finality_quorum_early_full_propagation,
        artifact_dir: artifact_dir.clone(),
        block_height: None,
        view: Some(finality_view.view),
        timeout_certificate_file: finality_view.timeout_certificate_file,
        timeout_ms: context.finality_timeout_ms,
        send_retries: context.finality_send_retries,
        retry_backoff_ms: context.finality_retry_backoff_ms,
        local_apply_before_certified_send: true,
        defer_certified_sends: true,
        max_transactions: 1,
        signed_transfer_file: None,
        signed_transfer_json: None,
        signed_payment_v2_json: None,
        signed_asset_transaction_json: None,
        signed_atomic_swap_transaction_json: Some(signed_json),
        signed_escrow_transaction_json: None,
        required_parent: Some(required_parent),
    })
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("mempool_submit_signed_atomic_swap_transaction_finality failed: {error}"),
        )
    })?;
    let tx_id = round.submitted_tx_id.clone().ok_or_else(|| {
        (
            "rpc_finality_submit_failed".to_string(),
            "atomic finality submit round did not report submitted_tx_id".to_string(),
        )
    })?;
    let finality = round
        .round
        .local_hot_finality
        .iter()
        .find(|report| report.tx_id == tx_id && report.confirmed && report.receipt.accepted)
        .cloned()
        .ok_or_else(|| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("atomic finality submit round did not emit accepted hot finality for `{tx_id}`"),
            )
        })?;

    let total_ms = monotonic_elapsed_ms(total_start);
    let round_report_file = artifact_dir.join("rpc-finality-round.json");
    let round_json = serde_json::to_string_pretty(&round).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("atomic finality round report serialization failed: {error}"),
        )
    })?;
    std::fs::write(&round_report_file, round_json).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!(
                "atomic finality round report write `{}` failed: {error}",
                round_report_file.display()
            ),
        )
    })?;
    let result = RpcMempoolSubmitSignedTransferFinalityReport {
        schema: "postfiat-rpc-mempool-submit-signed-atomic-swap-finality-v1".to_string(),
        tx_id: tx_id.clone(),
        finality,
        round_report_file: round_report_file.display().to_string(),
        artifact_dir: artifact_dir.display().to_string(),
        readiness_wait_ms,
        mempool_submit_ms: round.mempool_submit_ms,
        mempool_batch_ms: round.mempool_batch_ms,
        certified_round_ms: round.round.timings.total_ms,
        total_ms,
        certified_sends_deferred: round.round.certified_sends_deferred,
        round_ok: round.round_ok,
    };
    success_response(
        &request.id,
        &result,
        vec![RpcEvent::new(
            "mempool_submit_signed_atomic_swap_transaction_finality",
            tx_id,
            "externally dual-signed atomic swap finalized",
        )],
    )
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("atomic finality RPC response serialization failed: {error}"),
        )
    })
}
