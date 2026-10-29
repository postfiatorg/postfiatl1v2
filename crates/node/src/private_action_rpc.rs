// Validator-owned private action admission over RPC (W-1, S3).
//
// `private_action_submit` and `private_action_receipt` exist only when
// rpc-serve runs with `--allow-private-action-admission` (off by default).
// The same switch starts the round service: it dispatches reserved batches
// with this validator's own finality key through the `shield_batch_finality`
// certified round (quorum-early full propagation as configured, local apply
// before certified send), and never builds or re-signs a batch.

const PRIVATE_ACTION_SERVICE_INTERVAL_MS: u64 = 500;

fn is_private_action_method(method: &str) -> bool {
    matches!(method, "private_action_submit" | "private_action_receipt")
}

fn rpc_serve_private_action_allowed(method: &str, allow_private_action_admission: bool) -> bool {
    allow_private_action_admission && is_private_action_method(method)
}

/// `code: message` errors from the admission module keep their code.
fn private_action_error_parts(error: &std::io::Error) -> (String, String) {
    let text = error.to_string();
    match text.split_once(": ") {
        Some((code, message))
            if !code.is_empty()
                && code
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_') =>
        {
            (code.to_string(), message.to_string())
        }
        _ => ("private_action_failed".to_string(), text),
    }
}

fn private_action_param<'a>(
    params: &'a serde_json::Map<String, serde_json::Value>,
    name: &str,
) -> Result<&'a str, (String, String)> {
    params
        .get(name)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            (
                "private_action_request_invalid".to_string(),
                format!("missing string param `{name}`"),
            )
        })
}

fn private_action_submit_request_from_params(
    params: &serde_json::Map<String, serde_json::Value>,
) -> Result<postfiat_node::PrivateActionSubmitRequest, (String, String)> {
    const FIELDS: [&str; 8] = [
        "chain_id",
        "genesis_hash",
        "protocol_version",
        "pool_id",
        "session_scope",
        "intent_id",
        "action_json",
        "action_hash",
    ];
    if let Some(extra) = params.keys().find(|key| !FIELDS.contains(&key.as_str())) {
        return Err((
            "private_action_request_invalid".to_string(),
            format!("unknown param `{extra}`"),
        ));
    }
    let protocol_version = params
        .get("protocol_version")
        .and_then(serde_json::Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| {
            (
                "private_action_request_invalid".to_string(),
                "protocol_version must be a u32".to_string(),
            )
        })?;
    Ok(postfiat_node::PrivateActionSubmitRequest {
        domain: postfiat_node::PrivateActionSubmitDomain {
            chain_id: private_action_param(params, "chain_id")?.to_string(),
            genesis_hash: private_action_param(params, "genesis_hash")?.to_string(),
            protocol_version,
            pool_id: private_action_param(params, "pool_id")?.to_string(),
            session_scope: private_action_param(params, "session_scope")?.to_string(),
        },
        intent_id: private_action_param(params, "intent_id")?.to_string(),
        action_json: private_action_param(params, "action_json")?.to_string(),
        action_hash: private_action_param(params, "action_hash")?.to_string(),
    })
}

fn private_action_rpc_result(
    data_dir: &Path,
    method: &str,
    params: &serde_json::Map<String, serde_json::Value>,
) -> Result<(serde_json::Value, String), (String, String)> {
    let to_value = |value: serde_json::Result<serde_json::Value>| {
        value.map_err(|error| ("rpc_internal".to_string(), error.to_string()))
    };
    if method == "private_action_submit" {
        let request = private_action_submit_request_from_params(params)?;
        let report = postfiat_node::private_action_submit(data_dir, &request)
            .map_err(|error| private_action_error_parts(&error))?;
        let label = report.record.batch_id.clone();
        Ok((to_value(serde_json::to_value(report))?, label))
    } else {
        if let Some(extra) = params.keys().find(|key| key.as_str() != "intent_id") {
            return Err((
                "private_action_request_invalid".to_string(),
                format!("unknown param `{extra}`"),
            ));
        }
        let intent_id = private_action_param(params, "intent_id")?;
        let report = postfiat_node::private_action_receipt(data_dir, intent_id)
            .map_err(|error| private_action_error_parts(&error))?;
        let label = report.state.clone();
        Ok((to_value(serde_json::to_value(report))?, label))
    }
}

fn run_rpc_serve_private_action(
    request: &RpcRequest,
    context: &RpcServeConnectionContext,
) -> RpcResponse {
    let result = request
        .params
        .as_object()
        .ok_or_else(|| {
            (
                "private_action_request_invalid".to_string(),
                "params must be an object".to_string(),
            )
        })
        .and_then(|params| private_action_rpc_result(&context.data_dir, &request.method, params));
    match result {
        Ok((value, label)) => success_response(
            &request.id,
            &value,
            vec![RpcEvent::new(request.method.clone(), label, "private action")],
        )
        .unwrap_or_else(|error| {
            rpc_serve_error_response(&request.id, "rpc_internal", &error.to_string())
        }),
        Err((code, message)) => rpc_serve_error_response(&request.id, &code, &message),
    }
}

/// Local `postfiat-node rpc --method private_action_*`: flat flags, or
/// `--action-file` for the action text.
fn run_rpc_private_action(id: &str, method: &str, flags: &[String], data_dir: &Path) -> Result<(), String> {
    let mut params = serde_json::Map::new();
    let names: &[&str] = if method == "private_action_submit" {
        &[
            "chain_id",
            "genesis_hash",
            "pool_id",
            "session_scope",
            "intent_id",
            "action_hash",
        ]
    } else {
        &["intent_id"]
    };
    for name in names {
        let flag = format!("--{}", name.replace('_', "-"));
        let value = flag_value(flags, &flag).ok_or_else(|| format!("missing {flag}"))?;
        params.insert((*name).to_string(), serde_json::Value::String(value.to_string()));
    }
    if method == "private_action_submit" {
        let protocol_version = parse_u64_flag(flags, "--protocol-version")?;
        params.insert(
            "protocol_version".to_string(),
            serde_json::Value::from(protocol_version),
        );
        let action_json = match (flag_value(flags, "--action-json"), flag_value(flags, "--action-file")) {
            (Some(json), None) => json.to_string(),
            (None, Some(path)) => std::fs::read_to_string(path)
                .map_err(|error| format!("read --action-file `{path}` failed: {error}"))?,
            _ => return Err("use exactly one of --action-json or --action-file".to_string()),
        };
        params.insert("action_json".to_string(), serde_json::Value::String(action_json));
    }
    match private_action_rpc_result(data_dir, method, &params) {
        Ok((value, label)) => print_rpc_success(
            id,
            &value,
            vec![RpcEvent::new(method, label, "private action")],
        ),
        Err((code, message)) => Err(format!("rpc {method} failed: {code}: {message}")),
    }
}

/// One round-service pass for the RPC server's validator.
fn private_action_round_service_pass(
    context: &RpcServeConnectionContext,
) -> std::io::Result<Vec<postfiat_node::PrivateActionServiceStep>> {
    postfiat_node::private_action_service_step(
        &context.data_dir,
        // Pre-filter at view 0 so a non-proposer never writes `dispatched`;
        // the round re-checks the proposer for the prepared view.
        |_next_height| Ok(require_rpc_finality_local_proposer(context, 0).is_ok()),
        |record, _batch| {
            let request = RpcRequest::new(
                format!("private-action-{}", record.intent_id),
                "shield_batch_finality",
                serde_json::json!({ "batch_json": record.batch_json }),
            );
            match run_rpc_serve_shield_batch_finality_inner(0, &request, context) {
                Ok(_) => postfiat_node::PrivateActionDispatch::Finished,
                Err((code, _))
                    if code == "rpc_finality_wrong_proposer" || code == "rpc_finality_submit_busy" =>
                {
                    postfiat_node::PrivateActionDispatch::NotProposer
                }
                Err((code, message)) if code == "rpc_protocol_error" => {
                    postfiat_node::PrivateActionDispatch::Refused { code, message }
                }
                Err((_, message)) => postfiat_node::PrivateActionDispatch::Failed { message },
            }
        },
    )
}

fn spawn_private_action_round_service(
    context: RpcServeConnectionContext,
    stop: Arc<std::sync::atomic::AtomicBool>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while !stop.load(std::sync::atomic::Ordering::Acquire) {
            if let Err(error) = private_action_round_service_pass(&context) {
                eprintln!("private action round service pass failed: {error}");
            }
            let mut waited = 0;
            while waited < PRIVATE_ACTION_SERVICE_INTERVAL_MS
                && !stop.load(std::sync::atomic::Ordering::Acquire)
            {
                thread::sleep(Duration::from_millis(50));
                waited += 50;
            }
        }
    })
}

struct PrivateActionServiceStopGuard(Arc<std::sync::atomic::AtomicBool>);

impl Drop for PrivateActionServiceStopGuard {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::Release);
    }
}

#[cfg(test)]
mod private_action_rpc_tests {
    use super::*;

    #[test]
    fn private_action_methods_exist_only_behind_the_admission_switch() {
        for method in ["private_action_submit", "private_action_receipt"] {
            assert!(!rpc_serve_private_action_allowed(method, false));
            assert!(rpc_serve_private_action_allowed(method, true));
            // The read-only and other optional groups never include them.
            assert!(!rpc_serve_method_allowed_with_owned_lane(method, true, true, true, true));
        }
        assert!(!rpc_serve_private_action_allowed("status", true));
    }

    #[test]
    fn private_action_params_are_flat_and_closed() {
        let mut params = serde_json::Map::new();
        for (key, value) in [
            ("chain_id", "c"),
            ("genesis_hash", "g"),
            ("pool_id", "asset-orchard-v1"),
            ("session_scope", "native:abc"),
            ("intent_id", "i"),
            ("action_json", "{}"),
            ("action_hash", "h"),
        ] {
            params.insert(key.to_string(), serde_json::Value::String(value.to_string()));
        }
        params.insert("protocol_version".to_string(), serde_json::Value::from(1));
        let request = private_action_submit_request_from_params(&params).expect("flat params");
        assert_eq!(request.domain.session_scope, "native:abc");
        params.insert("note".to_string(), serde_json::Value::String("x".to_string()));
        let error = private_action_submit_request_from_params(&params).expect_err("extra param");
        assert_eq!(error.0, "private_action_request_invalid");
        let parts = private_action_error_parts(&std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "private_action_duplicate: already reserved",
        ));
        assert_eq!(parts, ("private_action_duplicate".to_string(), "already reserved".to_string()));
    }
}
