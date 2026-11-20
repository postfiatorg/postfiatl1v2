fn run_rpc_serve_mempool_submit_signed_offer_transaction_finality(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    match run_rpc_serve_mempool_submit_signed_offer_transaction_finality_inner(
        request_index,
        request,
        context,
    ) {
        Ok(response) => response,
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

fn run_rpc_serve_mempool_submit_signed_offer_transaction_finality_inner(
    request_index: u64,
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> Result<RpcResponse, (String, String)> {
    let params = request.params.as_object().ok_or_else(|| {
        (
            "rpc_protocol_error".to_string(),
            "mempool_submit_signed_offer_transaction_finality params must be an object".to_string(),
        )
    })?;
    let signed_offer_transaction_json = params
        .get("signed_offer_transaction_json")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "rpc_protocol_error".to_string(),
                "missing signed_offer_transaction_json".to_string(),
            )
        })?;
    if signed_offer_transaction_json.trim().is_empty() {
        return Err((
            "rpc_protocol_error".to_string(),
            "signed_offer_transaction_json must be nonempty".to_string(),
        ));
    }
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
    let finality_view = prepare_rpc_finality_round(context, params, &artifact_dir)?;
    let readiness_wait_ms = finality_view.readiness_wait_ms;
    let preview =
        postfiat_node::b23_facility_offer_preview(SignedOfferTransactionJsonSubmitOptions {
            data_dir: context.data_dir.clone(),
            signed_offer_transaction_json: signed_offer_transaction_json.to_string(),
        })
        .map_err(|error| ("rpc_finality_submit_failed".to_string(), error.to_string()))?;
    if !preview.accepted {
        // This is the literal execution preview, never a claimed block receipt.
        return success_response(&request.id, &serde_json::json!({
            "schema": "postfiat-rpc-mempool-submit-signed-offer-transaction-finality-v1",
            "tx_id": preview.tx_id,
            "finality": {"confirmed": false, "verification_mode": "mempool-admission-preview", "receipt": preview, "block": null},
            "round_report_file": null, "artifact_dir": null,
            "readiness_wait_ms": readiness_wait_ms, "mempool_submit_ms": 0.0,
            "mempool_batch_ms": 0.0, "certified_round_ms": 0.0,
            "total_ms": monotonic_elapsed_ms(total_start),
            "certified_sends_deferred": false, "round_ok": false, "idempotent_replay": false
        }), vec![]).map_err(|error| ("rpc_finality_submit_failed".to_string(), error.to_string()));
    }
    let submit_start = Instant::now();
    let entry =
        submit_signed_offer_transaction_json_to_mempool(SignedOfferTransactionJsonSubmitOptions {
            data_dir: context.data_dir.clone(),
            signed_offer_transaction_json: signed_offer_transaction_json.to_string(),
        })
        .map_err(|error| ("rpc_finality_submit_failed".to_string(), error.to_string()))?;
    let submit_ms = monotonic_elapsed_ms(submit_start);
    let mut round =
        transport_peer_certified_mempool_round(TransportPeerCertifiedMempoolRoundOptions {
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
            vote_phase_deadline_ms: context.finality_vote_phase_deadline_ms,
            local_apply_before_certified_send: true,
            defer_certified_sends: true,
            required_parent: None,
            max_transactions: 1,
            signed_transfer_file: None,
            signed_transfer_json: None,
            signed_payment_v2_json: None,
            signed_asset_transaction_json: None,
            signed_atomic_swap_transaction_json: None,
            signed_escrow_transaction_json: None,
        })
        .map_err(|error| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("mempool_submit_signed_offer_transaction_finality failed: {error}"),
            )
        })?;
    round.submitted_tx_id = Some(entry.tx_id);
    round.mempool_submit_ms = submit_ms;
    let tx_id = round.submitted_tx_id.clone().ok_or_else(|| {
        (
            "rpc_finality_submit_failed".to_string(),
            "finality submit round did not report submitted_tx_id".to_string(),
        )
    })?;
    let finality = round
        .round
        .local_hot_finality
        .iter()
        .find(|report| report.tx_id == tx_id && report.confirmed)
        .cloned()
        .ok_or_else(|| {
            (
                "rpc_finality_submit_failed".to_string(),
                format!("finality submit round did not emit hot finality for `{tx_id}`"),
            )
        })?;
    let total_ms = monotonic_elapsed_ms(total_start);
    let round_report_write_start = Instant::now();
    let round_report_file = artifact_dir.join("rpc-finality-round.json");
    let round_json = serde_json::to_string_pretty(&round).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality round report serialization failed: {error}"),
        )
    })?;
    std::fs::write(&round_report_file, round_json).map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!(
                "finality round report write `{}` failed: {error}",
                round_report_file.display()
            ),
        )
    })?;
    let round_report_write_ms = monotonic_elapsed_ms(round_report_write_start);
    let result = RpcMempoolSubmitSignedTransferFinalityReport {
        schema: "postfiat-rpc-mempool-submit-signed-offer-transaction-finality-v1".to_string(),
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
        idempotent_replay: false,
        step_timings: rpc_finality_step_timings(
            (finality_view.prepare_ms, finality_view.status_reads),
            &round,
            round_report_write_ms,
        ),
    };
    success_response(
        &request.id,
        &result,
        vec![RpcEvent::new(
            "mempool_submit_signed_offer_transaction_finality",
            tx_id,
            "externally signed offer transaction finalized",
        )],
    )
    .map_err(|error| {
        (
            "rpc_finality_submit_failed".to_string(),
            format!("finality RPC response serialization failed: {error}"),
        )
    })
}

#[cfg(test)]
#[path = "b23_facility_offer_rpc_tests.rs"]
mod b23_facility_offer_rpc_tests;
