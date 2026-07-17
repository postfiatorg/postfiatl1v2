pub const METHOD_ATOMIC_SWAP_FEE_QUOTE: &str = "atomic_swap_fee_quote";
pub const METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION: &str =
    "mempool_submit_signed_atomic_swap_transaction";
pub const METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION_FINALITY: &str =
    "mempool_submit_signed_atomic_swap_transaction_finality";

pub fn atomic_swap_transaction_tx_id(
    transaction: &postfiat_types::SignedAtomicSwapTransaction,
) -> String {
    hash_hex(
        postfiat_types::ATOMIC_SWAP_TRANSACTION_TX_ID_DOMAIN,
        &transaction.tx_id_preimage_bytes(),
    )
}

#[allow(clippy::too_many_arguments)]
pub fn atomic_swap_fee_quote_request(
    id: impl Into<String>,
    rfq_hash: impl Into<String>,
    market_envelope_hash: impl Into<String>,
    nav_epoch: u64,
    expires_at_height: u64,
    swap_nonce: impl Into<String>,
    leg_0_owner: impl Into<String>,
    leg_0_recipient: impl Into<String>,
    leg_0_issuer: impl Into<String>,
    leg_0_asset_id: impl Into<String>,
    leg_0_amount: u64,
    leg_1_owner: impl Into<String>,
    leg_1_recipient: impl Into<String>,
    leg_1_issuer: impl Into<String>,
    leg_1_asset_id: impl Into<String>,
    leg_1_amount: u64,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_ATOMIC_SWAP_FEE_QUOTE)
        .with_param_value("rfq_hash", string_value(rfq_hash))
        .with_param_value("market_envelope_hash", string_value(market_envelope_hash))
        .with_param_value("nav_epoch", u64_value(nav_epoch))
        .with_param_value("expires_at_height", u64_value(expires_at_height))
        .with_param_value("swap_nonce", string_value(swap_nonce))
        .with_param_value("leg_0_owner", string_value(leg_0_owner))
        .with_param_value("leg_0_recipient", string_value(leg_0_recipient))
        .with_param_value("leg_0_issuer", string_value(leg_0_issuer))
        .with_param_value("leg_0_asset_id", string_value(leg_0_asset_id))
        .with_param_value("leg_0_amount", u64_value(leg_0_amount))
        .with_param_value("leg_1_owner", string_value(leg_1_owner))
        .with_param_value("leg_1_recipient", string_value(leg_1_recipient))
        .with_param_value("leg_1_issuer", string_value(leg_1_issuer))
        .with_param_value("leg_1_asset_id", string_value(leg_1_asset_id))
        .with_param_value("leg_1_amount", u64_value(leg_1_amount))
}

pub fn mempool_submit_signed_atomic_swap_transaction_json_request(
    id: impl Into<String>,
    signed_atomic_swap_transaction_json: impl Into<String>,
) -> RpcRequest {
    RpcRequest::empty(id, METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION).with_param_value(
        "signed_atomic_swap_transaction_json",
        string_value(signed_atomic_swap_transaction_json),
    )
}

pub fn mempool_submit_signed_atomic_swap_transaction_finality_request(
    id: impl Into<String>,
    signed_atomic_swap_transaction_json: impl Into<String>,
    proxy_required_current_height: u64,
    proxy_required_state_root: &str,
    proxy_required_parent_hash: &str,
    proxy_readiness_timeout_ms: Option<u64>,
) -> RpcRequest {
    let mut request = RpcRequest::empty(
        id,
        METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION_FINALITY,
    )
    .with_param_value(
        "signed_atomic_swap_transaction_json",
        string_value(signed_atomic_swap_transaction_json),
    )
    .with_param_value(
        "proxy_required_current_height",
        u64_value(proxy_required_current_height),
    )
    .with_param_value(
        "proxy_required_state_root",
        string_value(proxy_required_state_root),
    )
    .with_param_value(
        "proxy_required_parent_hash",
        string_value(proxy_required_parent_hash),
    );
    if let Some(timeout_ms) = proxy_readiness_timeout_ms {
        request = request.with_param_value("proxy_readiness_timeout_ms", u64_value(timeout_ms));
    }
    request
}

pub fn mempool_submit_signed_atomic_swap_transaction_finality_from_quote_request(
    id: impl Into<String>,
    signed_atomic_swap_transaction_json: impl Into<String>,
    quote: &AtomicSwapFeeQuoteSummary,
    proxy_readiness_timeout_ms: Option<u64>,
) -> RpcRequest {
    mempool_submit_signed_atomic_swap_transaction_finality_request(
        id,
        signed_atomic_swap_transaction_json,
        quote.parent_height,
        &quote.parent_state_root,
        &quote.parent_hash,
        proxy_readiness_timeout_ms,
    )
}

fn validate_atomic_swap_fee_quote_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(
        params,
        &[
            "rfq_hash",
            "market_envelope_hash",
            "nav_epoch",
            "expires_at_height",
            "swap_nonce",
            "leg_0_owner",
            "leg_0_recipient",
            "leg_0_issuer",
            "leg_0_asset_id",
            "leg_0_amount",
            "leg_1_owner",
            "leg_1_recipient",
            "leg_1_issuer",
            "leg_1_asset_id",
            "leg_1_amount",
        ],
    )?;
    lower_hex_param(params, "rfq_hash", 96)?;
    lower_hex_param(params, "market_envelope_hash", 96)?;
    u64_param(params, "nav_epoch")?;
    nonzero_u64_param(params, "expires_at_height")?;
    lower_hex_param(params, "swap_nonce", 96)?;
    for field in [
        "leg_0_owner",
        "leg_0_recipient",
        "leg_0_issuer",
        "leg_1_owner",
        "leg_1_recipient",
        "leg_1_issuer",
    ] {
        string_param(params, field)?;
    }
    lower_hex_param(params, "leg_0_asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    lower_hex_param(params, "leg_1_asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
    nonzero_u64_param(params, "leg_0_amount")?;
    nonzero_u64_param(params, "leg_1_amount")?;
    if string_param(params, "leg_0_owner")? == string_param(params, "leg_1_owner")? {
        return Err(invalid_request_params(
            "leg_1_owner",
            "atomic swap owners must differ",
        ));
    }
    if string_param(params, "leg_0_owner")? != string_param(params, "leg_1_recipient")?
        || string_param(params, "leg_1_owner")? != string_param(params, "leg_0_recipient")?
    {
        return Err(invalid_request_params(
            "leg_1_recipient",
            "atomic swap legs must be reciprocal",
        ));
    }
    if string_param(params, "leg_0_asset_id")? == string_param(params, "leg_1_asset_id")? {
        return Err(invalid_request_params(
            "leg_1_asset_id",
            "atomic swap assets must differ",
        ));
    }
    Ok(())
}

fn validate_signed_atomic_swap_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(params, &["signed_atomic_swap_transaction_json"])?;
    validate_signed_atomic_swap_request_json(params)
}

fn validate_signed_atomic_swap_finality_request_params(
    params: &Value,
) -> Result<(), RpcRequestValidationError> {
    let params = request_params(params)?;
    require_only_params(
        params,
        &[
            "signed_atomic_swap_transaction_json",
            "proxy_required_current_height",
            "proxy_required_state_root",
            "proxy_required_parent_hash",
            "proxy_readiness_timeout_ms",
        ],
    )?;
    validate_signed_atomic_swap_request_json(params)?;
    u64_param(params, "proxy_required_current_height")?;
    lower_hex_param(params, "proxy_required_state_root", 96)?;
    lower_hex_param(params, "proxy_required_parent_hash", 96)?;
    optional_nonzero_u64_param(params, "proxy_readiness_timeout_ms")?;
    Ok(())
}

fn validate_signed_atomic_swap_request_json(
    params: &Map<String, Value>,
) -> Result<(), RpcRequestValidationError> {
    let raw = string_param(params, "signed_atomic_swap_transaction_json")?;
    if raw.len() > MAX_RPC_SIGNED_TRANSFER_JSON_BYTES {
        return Err(invalid_request_params(
            "signed_atomic_swap_transaction_json",
            format!("expected at most {MAX_RPC_SIGNED_TRANSFER_JSON_BYTES} bytes"),
        ));
    }
    let transaction = serde_json::from_str::<postfiat_types::SignedAtomicSwapTransaction>(raw)
        .map_err(|error| {
            invalid_request_params("signed_atomic_swap_transaction_json", error.to_string())
        })?;
    transaction
        .validate()
        .map_err(|error| invalid_request_params("signed_atomic_swap_transaction_json", error))
}
