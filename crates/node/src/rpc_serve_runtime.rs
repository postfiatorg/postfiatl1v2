fn rpc_serve(options: RpcServeOptions) -> Result<RpcServeReport, String> {
    rpc_serve_inner(
        options,
        #[cfg(test)]
        None,
    )
}

fn rpc_serve_inner(
    options: RpcServeOptions,
    #[cfg(test)] listener_for_test: Option<TcpListener>,
) -> Result<RpcServeReport, String> {
    clear_transport_ready_file(&options.ready_file, "rpc serve")?;
    let mut local_status = status(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("rpc serve status failed: {error}"))?;
    let reconciled_terminal_entries = reconcile_terminal_mempool_entries(&options.data_dir)
        .map_err(|error| format!("rpc serve terminal mempool reconciliation failed: {error}"))?;
    if reconciled_terminal_entries > 0 {
        local_status = status(NodeOptions {
            data_dir: options.data_dir.clone(),
        })
        .map_err(|error| format!("rpc serve post-reconciliation status failed: {error}"))?;
    }
    let owned_certificate_domain = owned_certificate_domain(&options.data_dir)
        .map_err(|error| format!("rpc serve FastPay domain failed: {error}"))?;
    validate_rpc_serve_bind_host(&options.bind_host)?;
    prepare_rpc_serve_spool_root(&options.spool_dir)?;
    probe_rpc_serve_spool_root(&options.spool_dir)?;
    let bind_address = socket_address(&options.bind_host, options.port);
    #[cfg(test)]
    let listener = match listener_for_test {
        Some(listener) => Ok(listener),
        None => TcpListener::bind(&bind_address),
    };
    #[cfg(not(test))]
    let listener = TcpListener::bind(&bind_address);
    let listener = listener.map_err(|error| format!("rpc serve bind `{bind_address}` failed: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("rpc serve nonblocking accept failed: {error}"))?;
    let mut event_writer = options
        .event_log
        .as_ref()
        .map(|path| open_transport_event_log(path))
        .transpose()?;
    let finality_topology_available = !options.allow_mempool_submit_finality
        || options.finality_topology_file.is_file();
    let finality_key_available = !options.allow_mempool_submit_finality
        || options.finality_key_file.is_file();
    if !finality_topology_available || !finality_key_available {
        return Err("rpc serve finality configuration files are unavailable".to_string());
    }
    let initial_mempool = mempool_state(NodeOptions {
        data_dir: options.data_dir.clone(),
    })
    .map_err(|error| format!("rpc serve initial mempool load failed: {error}"))?;
    let readiness = Arc::new(Mutex::new(RpcServeReadinessReport {
        schema: "postfiat-rpc-serve-readiness-v1".to_string(),
        ready: true,
        degraded: false,
        node_id: local_status.node_id.clone(),
        bind_address: bind_address.clone(),
        data_dir: options.data_dir.display().to_string(),
        data_dir_readable: true,
        spool_dir: options.spool_dir.display().to_string(),
        spool_probe_ok: true,
        event_log: options.event_log.as_ref().map(|path| path.display().to_string()),
        event_log_writable: true,
        local_state_loaded: true,
        listener_bound: true,
        finality_enabled: options.allow_mempool_submit_finality,
        finality_topology_available,
        finality_key_available,
        telemetry_failure_count: 0,
        last_telemetry_error: None,
    }));
    let mut requests = Vec::with_capacity(options.max_requests);
    let mempool_submit_state = Arc::new(Mutex::new(RpcServeMempoolSubmitState::default()));
    let orchard_batch_create_state = Arc::new(Mutex::new(RpcServeMempoolSubmitState::default()));
    let child_dispatch_state = Arc::new(Mutex::new(RpcServeMempoolSubmitState::default()));
    let mempool_mutation_lock = Arc::new(Mutex::new(()));
    let finality_submit_lock = Arc::new(Mutex::new(()));
    let health_cache = Arc::new(Mutex::new(RpcServeHealthCache {
        status: Some(local_status.clone()),
        mempool: Some((
            rpc_serve_health_stamp(&options.data_dir, false)?,
            initial_mempool,
        )),
        status_checked_at: Some(Instant::now()),
        mempool_checked_at: Some(Instant::now()),
    }));
    {
        let readiness = readiness
            .lock()
            .map_err(|_| "rpc serve readiness lock poisoned".to_string())?;
        write_rpc_serve_readiness(&options.ready_file, &readiness)?;
    }
    let fastswap_service = Arc::new(Mutex::new(None));
    let runtime_metrics = Arc::new(RpcServeRuntimeMetrics::default());
    let (event_sender, event_receiver) = mpsc::channel::<(Option<RpcServeEventRecord>, bool)>();
    let mut accepted_count = 0_usize;
    let mut active_connections = 0_usize;
    while rpc_serve_accept_budget_allows(accepted_count, options.max_requests) {
        while active_connections >= MAX_RPC_SERVE_ACTIVE_CONNECTIONS {
            receive_rpc_serve_event(
                &event_receiver,
                &mut active_connections,
                &mut requests,
                &mut event_writer,
                &options.ready_file,
                &readiness,
            )?;
        }
        drain_rpc_serve_events(
            &event_receiver,
            &mut active_connections,
            &mut requests,
            &mut event_writer,
            &options.ready_file,
            &readiness,
        )?;

        let (stream, peer_addr) = match listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                thread::sleep(Duration::from_millis(10));
                continue;
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(format!("rpc serve accept failed: {error}")),
        };
        set_stream_timeout(&stream, options.timeout_ms)?;
        accepted_count = accepted_count.saturating_add(1);
        active_connections = active_connections.saturating_add(1);
        runtime_metrics.connection_opened();
        let request_index = accepted_count as u64;
        let context = RpcServeConnectionContext {
            data_dir: options.data_dir.clone(),
            spool_dir: options.spool_dir.clone(),
            node_id: local_status.node_id.clone(),
            peer_addr: peer_addr.ip().to_string(),
            allow_mempool_submit: options.allow_mempool_submit,
            allow_mempool_submit_finality: options.allow_mempool_submit_finality,
            allow_orchard_batch_create: options.allow_orchard_batch_create,
            owned_lane_enabled: options.owned_lane_enabled,
            owned_certificate_domain: owned_certificate_domain.clone(),
            child_timeout_ms: options.child_timeout_ms,
            finality_topology_file: options.finality_topology_file.clone(),
            finality_key_file: options.finality_key_file.clone(),
            finality_proposal_key_file: options.finality_proposal_key_file.clone(),
            finality_artifact_root: options.finality_artifact_root.clone(),
            finality_timeout_ms: options.finality_timeout_ms,
            finality_send_retries: options.finality_send_retries,
            finality_retry_backoff_ms: options.finality_retry_backoff_ms,
            finality_quorum_early_full_propagation: options
                .finality_quorum_early_full_propagation,
            max_mempool_submit_per_peer: options.max_mempool_submit_per_peer,
            max_mempool_submit_total: options.max_mempool_submit_total,
            max_orchard_batch_create_per_peer: options.max_orchard_batch_create_per_peer,
            max_orchard_batch_create_total: options.max_orchard_batch_create_total,
            max_orchard_batch_create_concurrent: options.max_orchard_batch_create_concurrent,
            max_child_dispatch_concurrent: options.max_child_dispatch_concurrent,
            max_child_dispatch_per_peer: options.max_child_dispatch_per_peer,
            mempool_submit_state: Arc::clone(&mempool_submit_state),
            orchard_batch_create_state: Arc::clone(&orchard_batch_create_state),
            child_dispatch_state: Arc::clone(&child_dispatch_state),
            mempool_mutation_lock: Arc::clone(&mempool_mutation_lock),
            finality_submit_lock: Arc::clone(&finality_submit_lock),
            health_cache: Arc::clone(&health_cache),
            fastswap_service: Arc::clone(&fastswap_service),
            runtime_metrics: Arc::clone(&runtime_metrics),
        };
        let event_sender = event_sender.clone();
        let keep_alive = options.keep_alive;
        let timeout_ms = options.timeout_ms;
        thread::spawn(move || {
            let _active_connection_guard =
                RpcServeActiveConnectionGuard(Arc::clone(&context.runtime_metrics));
            let fallback_node_id = context.node_id.clone();
            let fallback_peer_addr = context.peer_addr.clone();
            let mut context = context;
            let mut idx = request_index;
            let mut handled = 0_u64;
            // Keep one reader for the connection lifetime so pipelined bytes
            // read ahead after a newline are not discarded between requests.
            let mut reader = BufReader::new(RpcServeDeadlineStream::new(stream, timeout_ms));
            loop {
                // Each request frame gets a fresh bounded read budget; a
                // stalled or trickling client only drops this connection.
                reader.get_mut().reset_read_deadline();
                let event = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    handle_rpc_serve_connection(&mut reader, idx, &mut context)
                }))
                .unwrap_or_else(|_| {
                    let id = format!("remote-{idx}");
                    let response =
                        rpc_serve_error_response(&id, "rpc_worker_panic", "rpc serve worker panicked");
                    rpc_serve_event(
                        &fallback_node_id,
                        idx,
                        &fallback_peer_addr,
                        id,
                        "invalid",
                        None,
                        None,
                        &response,
                    )
                });
                let read_failed = event.error_code.as_deref() == Some("rpc_read_error");
                if event_sender.send((Some(event), false)).is_err() {
                    return;
                }
                idx = idx.saturating_add(1);
                handled = handled.saturating_add(1);
                if !keep_alive || !rpc_serve_connection_request_budget_allows(handled) || read_failed {
                    break;
                }
                // Observe both read-ahead and new socket bytes without
                // consuming the next frame before the handler sees it.
                reader.get_mut().reset_read_deadline();
                match reader.fill_buf() {
                    Ok([]) | Err(_) => break,
                    Ok(_) => continue,
                }
            }
            let _ = event_sender.send((None, true));
        });
    }
    drop(event_sender);
    while active_connections > 0 {
        receive_rpc_serve_event(
            &event_receiver,
            &mut active_connections,
            &mut requests,
            &mut event_writer,
            &options.ready_file,
            &readiness,
        )?;
    }
    requests.sort_by_key(|request| request.request_index);

    let ok_count = requests.iter().filter(|request| request.ok).count() as u64;
    let request_count = requests.len() as u64;
    let mempool_submit_signed_transfer_count = requests
        .iter()
        .filter(|request| is_mempool_submit_signed_method(&request.method))
        .count() as u64;
    let mempool_submit_signed_transfer_finality_count = requests
        .iter()
        .filter(|request| is_mempool_submit_signed_transfer_finality_method(&request.method))
        .count() as u64;
    let orchard_batch_create_count = requests
        .iter()
        .filter(|request| is_orchard_batch_create_method(&request.method))
        .count() as u64;
    let invalid_signature_count = rpc_serve_error_class_count(&requests, "invalid_signature");
    let duplicate_transaction_count =
        rpc_serve_error_class_count(&requests, "duplicate_transaction");
    let request_too_large_count = rpc_serve_error_class_count(&requests, "request_too_large");
    let mempool_submit_rate_limited_count =
        rpc_serve_error_class_count(&requests, "mempool_submit_rate_limited");
    let mempool_submit_global_rate_limited_count =
        rpc_serve_error_class_count(&requests, "mempool_submit_global_rate_limited");
    let orchard_batch_create_rate_limited_count =
        rpc_serve_error_class_count(&requests, "orchard_batch_create_rate_limited");
    let orchard_batch_create_global_rate_limited_count =
        rpc_serve_error_class_count(&requests, "orchard_batch_create_global_rate_limited");
    let orchard_batch_create_concurrency_limited_count =
        rpc_serve_error_class_count(&requests, "orchard_batch_create_concurrency_limited");
    let orchard_batch_create_not_public_safe_count =
        rpc_serve_error_class_count(&requests, "orchard_batch_create_not_public_safe");
    let child_dispatch_concurrency_limited_count =
        rpc_serve_error_class_count(&requests, "rpc_child_dispatch_concurrency_limited");
    let rpc_child_timeout_count = rpc_serve_error_class_count(&requests, "rpc_child_timeout");
    let method_not_allowed_count = rpc_serve_error_class_count(&requests, "method_not_allowed");
    Ok(RpcServeReport {
        schema: "postfiat-rpc-serve-v1".to_string(),
        node_id: local_status.node_id,
        bind_address,
        event_log: options.event_log.map(|path| path.display().to_string()),
        max_requests: options.max_requests as u64,
        child_timeout_ms: options.child_timeout_ms,
        child_isolation: rpc_child_isolation_report(),
        request_count,
        ok_count,
        error_count: request_count.saturating_sub(ok_count),
        mempool_submit_signed_transfer_count,
        mempool_submit_signed_transfer_finality_count,
        orchard_batch_create_count,
        max_mempool_submit_per_peer: options.max_mempool_submit_per_peer,
        max_mempool_submit_total: options.max_mempool_submit_total,
        max_orchard_batch_create_per_peer: options.max_orchard_batch_create_per_peer,
        max_orchard_batch_create_total: options.max_orchard_batch_create_total,
        max_orchard_batch_create_concurrent: options.max_orchard_batch_create_concurrent,
        max_child_dispatch_concurrent: options.max_child_dispatch_concurrent,
        max_child_dispatch_per_peer: options.max_child_dispatch_per_peer,
        invalid_signature_count,
        duplicate_transaction_count,
        request_too_large_count,
        mempool_submit_rate_limited_count,
        mempool_submit_global_rate_limited_count,
        orchard_batch_create_rate_limited_count,
        orchard_batch_create_global_rate_limited_count,
        orchard_batch_create_concurrency_limited_count,
        orchard_batch_create_not_public_safe_count,
        child_dispatch_concurrency_limited_count,
        rpc_child_timeout_count,
        method_not_allowed_count,
        read_only: !options.allow_mempool_submit
            && !options.allow_mempool_submit_finality
            && !options.allow_orchard_batch_create
            && !options.owned_lane_enabled,
        mempool_submit_finality_enabled: options.allow_mempool_submit_finality,
        orchard_batch_create_enabled: options.allow_orchard_batch_create,
        owned_lane_enabled: options.owned_lane_enabled,
        requests,
        verified: true,
    })
}
