pub const ATOMIC_SWAP_FEE_QUOTE_SCHEMA: &str = "postfiat-atomic-swap-fee-quote-v1";
pub const ATOMIC_SWAP_FINALITY_SCHEMA: &str =
    "postfiat-rpc-mempool-submit-signed-atomic-swap-finality-v1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSwapLegFeeQuoteSummary {
    pub owner: String,
    pub sender_balance: u64,
    pub sender_sequence: u64,
    pub sequence: u64,
    pub mempool_pending_for_owner: u64,
    pub base_atomic_swap_fee: u64,
    pub state_expansion_fee: u64,
    pub minimum_fee: u64,
    pub sender_balance_after_fee: Option<u64>,
    pub sender_meets_reserve_after_fee: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AtomicSwapFeeQuoteSummary {
    pub transaction_kind: String,
    pub parent_height: u64,
    pub parent_hash: String,
    pub parent_state_root: String,
    pub quote_height: u64,
    pub account_reserve: u64,
    pub transfer_fee_byte_quantum: u64,
    pub transfer_fee_per_quantum: u64,
    pub atomic_swap_weight_bytes: u64,
    pub leg_0: AtomicSwapLegFeeQuoteSummary,
    pub leg_1: AtomicSwapLegFeeQuoteSummary,
    pub unsigned_transaction: postfiat_types::UnsignedAtomicSwapTransaction,
}

pub type AtomicSwapFinalitySummary = TxFinalitySummary;

fn validate_atomic_swap_fee_quote_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ATOMIC_SWAP_FEE_QUOTE_SCHEMA)?;
    expect_string_eq(result, "transaction_kind", ATOMIC_SWAP_TRANSACTION_KIND)?;
    let parent_height = u64_field(result, "parent_height")?;
    validate_block_tip_hash(
        parent_height,
        string_field(result, "parent_hash")?,
        "parent_hash",
    )?;
    lower_hex_field(result, "parent_state_root", 96)?;
    let quote_height = nonzero_u64_field(result, "quote_height")?;
    if parent_height.checked_add(1) != Some(quote_height) {
        return Err(invalid_result(
            "quote_height",
            "expected quote_height to immediately follow parent_height",
        ));
    }
    let account_reserve = nonzero_u64_field(result, "account_reserve")?;
    nonzero_u64_field(result, "transfer_fee_byte_quantum")?;
    nonzero_u64_field(result, "transfer_fee_per_quantum")?;
    nonzero_u64_field(result, "atomic_swap_weight_bytes")?;
    let unsigned_value = field(result, "unsigned_transaction")?;
    let unsigned = serde_json::from_value::<postfiat_types::UnsignedAtomicSwapTransaction>(
        unsigned_value.clone(),
    )
    .map_err(|error| invalid_result("unsigned_transaction", error.to_string()))?;
    unsigned
        .validate()
        .map_err(|error| invalid_result("unsigned_transaction", error))?;
    validate_atomic_swap_leg_quote(
        field(result, "leg_0")?,
        &unsigned.leg_0,
        account_reserve,
        "leg_0",
    )?;
    validate_atomic_swap_leg_quote(
        field(result, "leg_1")?,
        &unsigned.leg_1,
        account_reserve,
        "leg_1",
    )?;
    Ok(())
}

fn validate_atomic_swap_leg_quote(
    quote: &Value,
    leg: &postfiat_types::AtomicSwapLeg,
    account_reserve: u64,
    field_name: &str,
) -> Result<(), RpcResponseValidationError> {
    if clean_string_field(quote, "owner")? != leg.owner {
        return Err(invalid_result(
            format!("{field_name}.owner"),
            "expected quote owner to match unsigned leg owner",
        ));
    }
    let sender_balance = u64_field(quote, "sender_balance")?;
    let sender_sequence = u64_field(quote, "sender_sequence")?;
    let sequence = nonzero_u64_field(quote, "sequence")?;
    if sender_sequence.checked_add(1) != Some(sequence) || sequence != leg.sequence {
        return Err(invalid_result(
            format!("{field_name}.sequence"),
            "expected next owner sequence matching the unsigned leg",
        ));
    }
    if u64_field(quote, "mempool_pending_for_owner")? != 0 {
        return Err(invalid_result(
            format!("{field_name}.mempool_pending_for_owner"),
            "expected no pending transactions for either quoted owner",
        ));
    }
    let base_fee = nonzero_u64_field(quote, "base_atomic_swap_fee")?;
    let state_expansion_fee = u64_field(quote, "state_expansion_fee")?;
    let minimum_fee = nonzero_u64_field(quote, "minimum_fee")?;
    if base_fee.checked_add(state_expansion_fee) != Some(minimum_fee) || minimum_fee != leg.fee {
        return Err(invalid_result(
            format!("{field_name}.minimum_fee"),
            "expected base plus state-expansion fee matching the unsigned leg",
        ));
    }
    let expected_balance_after_fee = sender_balance.checked_sub(minimum_fee);
    let balance_after_fee = optional_u64_field_value(quote, "sender_balance_after_fee")?;
    if balance_after_fee != expected_balance_after_fee {
        return Err(invalid_result(
            format!("{field_name}.sender_balance_after_fee"),
            "expected sender balance less minimum fee",
        ));
    }
    let meets_reserve = bool_field(quote, "sender_meets_reserve_after_fee")?;
    if meets_reserve != balance_after_fee.is_some_and(|balance| balance >= account_reserve) {
        return Err(invalid_result(
            format!("{field_name}.sender_meets_reserve_after_fee"),
            "expected reserve flag to match post-fee balance",
        ));
    }
    Ok(())
}

fn validated_atomic_swap_request_params<'a>(
    request: &'a RpcRequest,
    kind: RpcRequestKind,
    method: &str,
) -> Result<&'a Map<String, Value>, RpcResponseValidationError> {
    validate_request(request, None, Some(kind))
        .map_err(|error| invalid_result("request", error.to_string()))?;
    if request.method != method {
        return Err(invalid_result(
            "request.method",
            format!("expected `{method}`, found `{}`", request.method),
        ));
    }
    request
        .params
        .as_object()
        .ok_or_else(|| invalid_result("request.params", "expected object value"))
}

fn atomic_swap_request_string<'a>(
    params: &'a Map<String, Value>,
    field_name: &str,
) -> Result<&'a str, RpcResponseValidationError> {
    params
        .get(field_name)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid_result(format!("request.params.{field_name}"), "expected string"))
}

fn atomic_swap_request_u64(
    params: &Map<String, Value>,
    field_name: &str,
) -> Result<u64, RpcResponseValidationError> {
    params
        .get(field_name)
        .and_then(Value::as_u64)
        .ok_or_else(|| {
            invalid_result(
                format!("request.params.{field_name}"),
                "expected unsigned integer",
            )
        })
}

fn validate_atomic_swap_quote_summary_for_request(
    quote: &AtomicSwapFeeQuoteSummary,
    request: &RpcRequest,
) -> Result<(), RpcResponseValidationError> {
    let params = validated_atomic_swap_request_params(
        request,
        RpcRequestKind::AtomicSwapFeeQuote,
        METHOD_ATOMIC_SWAP_FEE_QUOTE,
    )?;
    let unsigned = &quote.unsigned_transaction;
    for (field_name, expected, observed) in [
        (
            "rfq_hash",
            atomic_swap_request_string(params, "rfq_hash")?,
            unsigned.rfq_hash.as_str(),
        ),
        (
            "market_envelope_hash",
            atomic_swap_request_string(params, "market_envelope_hash")?,
            unsigned.market_envelope_hash.as_str(),
        ),
        (
            "swap_nonce",
            atomic_swap_request_string(params, "swap_nonce")?,
            unsigned.swap_nonce.as_str(),
        ),
        (
            "leg_0_owner",
            atomic_swap_request_string(params, "leg_0_owner")?,
            unsigned.leg_0.owner.as_str(),
        ),
        (
            "leg_0_recipient",
            atomic_swap_request_string(params, "leg_0_recipient")?,
            unsigned.leg_0.recipient.as_str(),
        ),
        (
            "leg_0_issuer",
            atomic_swap_request_string(params, "leg_0_issuer")?,
            unsigned.leg_0.issuer.as_str(),
        ),
        (
            "leg_0_asset_id",
            atomic_swap_request_string(params, "leg_0_asset_id")?,
            unsigned.leg_0.asset_id.as_str(),
        ),
        (
            "leg_1_owner",
            atomic_swap_request_string(params, "leg_1_owner")?,
            unsigned.leg_1.owner.as_str(),
        ),
        (
            "leg_1_recipient",
            atomic_swap_request_string(params, "leg_1_recipient")?,
            unsigned.leg_1.recipient.as_str(),
        ),
        (
            "leg_1_issuer",
            atomic_swap_request_string(params, "leg_1_issuer")?,
            unsigned.leg_1.issuer.as_str(),
        ),
        (
            "leg_1_asset_id",
            atomic_swap_request_string(params, "leg_1_asset_id")?,
            unsigned.leg_1.asset_id.as_str(),
        ),
    ] {
        if observed != expected {
            return Err(invalid_result(
                format!("unsigned_transaction.{field_name}"),
                format!("quote value `{observed}` does not match request `{expected}`"),
            ));
        }
    }
    for (field_name, expected, observed) in [
        (
            "nav_epoch",
            atomic_swap_request_u64(params, "nav_epoch")?,
            unsigned.nav_epoch,
        ),
        (
            "expires_at_height",
            atomic_swap_request_u64(params, "expires_at_height")?,
            unsigned.expires_at_height,
        ),
        (
            "leg_0_amount",
            atomic_swap_request_u64(params, "leg_0_amount")?,
            unsigned.leg_0.amount,
        ),
        (
            "leg_1_amount",
            atomic_swap_request_u64(params, "leg_1_amount")?,
            unsigned.leg_1.amount,
        ),
    ] {
        if observed != expected {
            return Err(invalid_result(
                format!("unsigned_transaction.{field_name}"),
                format!("quote value `{observed}` does not match request `{expected}`"),
            ));
        }
    }
    Ok(())
}

struct AtomicSwapFinalityRequestBinding {
    transaction: postfiat_types::SignedAtomicSwapTransaction,
    required_parent_height: u64,
    required_parent_hash: String,
}

fn atomic_swap_binding_from_finality_request(
    request: &RpcRequest,
) -> Result<AtomicSwapFinalityRequestBinding, RpcResponseValidationError> {
    let params = validated_atomic_swap_request_params(
        request,
        RpcRequestKind::MempoolSubmitSignedAtomicSwapTransactionFinality,
        METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION_FINALITY,
    )?;
    let raw = atomic_swap_request_string(params, "signed_atomic_swap_transaction_json")?;
    let transaction = serde_json::from_str(raw).map_err(|error| {
        invalid_result(
            "request.params.signed_atomic_swap_transaction_json",
            error.to_string(),
        )
    })?;
    Ok(AtomicSwapFinalityRequestBinding {
        transaction,
        required_parent_height: atomic_swap_request_u64(params, "proxy_required_current_height")?,
        required_parent_hash: atomic_swap_request_string(params, "proxy_required_parent_hash")?
            .to_string(),
    })
}

fn validate_atomic_swap_finality_for_transaction(
    result: &Value,
    binding: &AtomicSwapFinalityRequestBinding,
) -> Result<(), RpcResponseValidationError> {
    let transaction = &binding.transaction;
    let expected_tx_id = atomic_swap_transaction_tx_id(transaction);
    if string_field(result, "tx_id")? != expected_tx_id {
        return Err(invalid_result(
            "tx_id",
            "finality tx_id does not match the submitted atomic swap",
        ));
    }
    let finality = field(result, "finality")?;
    if clean_string_field(finality, "chain_id")? != transaction.unsigned.chain_id
        || string_field(finality, "genesis_hash")? != transaction.unsigned.genesis_hash
        || nonzero_u32_field(finality, "protocol_version")? != transaction.unsigned.protocol_version
    {
        return Err(invalid_result(
            "finality",
            "finality chain domain does not match the submitted atomic swap",
        ));
    }
    let block = field(finality, "block")?;
    let block_header = field(block, "header")?;
    let block_height = nonzero_u64_field(block_header, "height")?;
    let expected_child_height = binding
        .required_parent_height
        .checked_add(1)
        .ok_or_else(|| {
            invalid_result(
                "request.params.proxy_required_current_height",
                "required parent height overflows",
            )
        })?;
    if block_height != expected_child_height {
        return Err(invalid_result(
            "finality.block.header.height",
            format!(
                "finality height `{block_height}` does not immediately follow requested parent `{}`",
                binding.required_parent_height
            ),
        ));
    }
    if string_field(block_header, "parent_hash")? != binding.required_parent_hash {
        return Err(invalid_result(
            "finality.block.header.parent_hash",
            "finality parent hash does not match the requested parent pin",
        ));
    }
    if block_height > transaction.unsigned.expires_at_height {
        return Err(invalid_result(
            "finality.block.header.height",
            "atomic swap finalized after its signed expiry height",
        ));
    }
    let receipt = field(finality, "receipt")?;
    let legs = array_field(receipt, "atomic_swap_legs")?;
    if legs.len() != 2 {
        return Err(invalid_result(
            "finality.receipt.atomic_swap_legs",
            "expected exactly two atomic swap leg receipts",
        ));
    }
    for (field_name, receipt_leg, transaction_leg) in [
        ("leg_0", &legs[0], &transaction.unsigned.leg_0),
        ("leg_1", &legs[1], &transaction.unsigned.leg_1),
    ] {
        let expected_pre_sequence = transaction_leg.sequence.checked_sub(1).ok_or_else(|| {
            invalid_result(
                format!("finality.receipt.atomic_swap_legs.{field_name}.pre_sequence"),
                "submitted atomic swap has an invalid zero sequence",
            )
        })?;
        if clean_string_field(receipt_leg, "owner")? != transaction_leg.owner
            || clean_string_field(receipt_leg, "recipient")? != transaction_leg.recipient
            || string_field(receipt_leg, "asset_id")? != transaction_leg.asset_id
            || nonzero_u64_field(receipt_leg, "amount")? != transaction_leg.amount
            || nonzero_u64_field(receipt_leg, "fee_charged")? != transaction_leg.fee
            || u64_field(receipt_leg, "pre_sequence")? != expected_pre_sequence
            || nonzero_u64_field(receipt_leg, "post_sequence")? != transaction_leg.sequence
        {
            return Err(invalid_result(
                format!("finality.receipt.atomic_swap_legs.{field_name}"),
                "finalized leg does not match the submitted atomic swap",
            ));
        }
    }
    Ok(())
}

fn validate_mempool_atomic_swap_entry_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    lower_hex_field(result, "tx_id", 96)?;
    let transaction = field(result, "transaction")?;
    validate_signed_atomic_swap_transaction_fields(transaction)?;
    let signed =
        serde_json::from_value::<postfiat_types::SignedAtomicSwapTransaction>(transaction.clone())
            .map_err(|error| invalid_result("transaction", error.to_string()))?;
    let expected_tx_id = atomic_swap_transaction_tx_id(&signed);
    if string_field(result, "tx_id")? != expected_tx_id {
        return Err(invalid_result(
            "tx_id",
            "expected atomic mempool tx_id to match signed transaction preimage",
        ));
    }
    Ok(())
}

pub fn decode_atomic_swap_mempool_submit_entry(
    response: &RpcResponse,
    request: &RpcRequest,
) -> Result<postfiat_types::MempoolAtomicSwapEntry, RpcResponseValidationError> {
    validate_response(response, Some(&request.id), true)?;
    let params = validated_atomic_swap_request_params(
        request,
        RpcRequestKind::MempoolSubmitSignedAtomicSwapTransaction,
        METHOD_MEMPOOL_SUBMIT_SIGNED_ATOMIC_SWAP_TRANSACTION,
    )?;
    let raw = atomic_swap_request_string(params, "signed_atomic_swap_transaction_json")?;
    let submitted = serde_json::from_str::<postfiat_types::SignedAtomicSwapTransaction>(raw)
        .map_err(|error| {
            invalid_result(
                "request.params.signed_atomic_swap_transaction_json",
                error.to_string(),
            )
        })?;
    let result = validated_summary_result(
        response,
        RpcResponseKind::MempoolSubmitSignedAtomicSwapTransaction,
    )?;
    let entry = serde_json::from_value::<postfiat_types::MempoolAtomicSwapEntry>(result.clone())
        .map_err(|error| invalid_result("result", error.to_string()))?;
    if entry.transaction != submitted {
        return Err(invalid_result(
            "transaction",
            "atomic mempool response transaction does not match the submitted signed transaction",
        ));
    }
    let expected_tx_id = atomic_swap_transaction_tx_id(&submitted);
    if entry.tx_id != expected_tx_id {
        return Err(invalid_result(
            "tx_id",
            "atomic mempool response tx_id does not match the submitted signed transaction",
        ));
    }
    Ok(entry)
}

fn validate_atomic_swap_finality_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    expect_string_eq(result, "schema", ATOMIC_SWAP_FINALITY_SCHEMA)?;
    lower_hex_field(result, "tx_id", 96)?;
    let tx_id = string_field(result, "tx_id")?;
    clean_string_field(result, "round_report_file")?;
    clean_string_field(result, "artifact_dir")?;
    for field_name in [
        "readiness_wait_ms",
        "mempool_submit_ms",
        "mempool_batch_ms",
        "certified_round_ms",
        "total_ms",
    ] {
        let value = field(result, field_name)?
            .as_f64()
            .ok_or_else(|| invalid_result(field_name, "expected numeric milliseconds"))?;
        if !value.is_finite() || value < 0.0 {
            return Err(invalid_result(
                field_name,
                "expected finite nonnegative milliseconds",
            ));
        }
    }
    let certified_sends_deferred = bool_field(result, "certified_sends_deferred")?;
    let round_ok = bool_field(result, "round_ok")?;
    if !round_ok && !certified_sends_deferred {
        return Err(invalid_result(
            "round_ok",
            "expected a complete round or durably deferred certified sends",
        ));
    }
    let finality = field(result, "finality")?;
    validate_tx_finality_result(finality)?;
    if string_field(finality, "tx_id")? != tx_id {
        return Err(invalid_result(
            "finality.tx_id",
            "expected nested finality tx_id to match outer tx_id",
        ));
    }
    let receipt = field(finality, "receipt")?;
    expect_bool_eq(receipt, "accepted", true)?;
    let legs = receipt
        .get("atomic_swap_legs")
        .ok_or_else(|| {
            invalid_result(
                "finality.receipt.atomic_swap_legs",
                "expected atomic leg receipts on accepted atomic finality",
            )
        })?
        .as_array()
        .ok_or_else(|| {
            invalid_result(
                "finality.receipt.atomic_swap_legs",
                "expected non-null atomic leg receipt array",
            )
        })?;
    if legs.len() != 2 {
        return Err(invalid_result(
            "finality.receipt.atomic_swap_legs",
            "expected exactly two atomic leg receipts",
        ));
    }
    Ok(())
}

fn validate_signed_atomic_swap_transaction_fields(
    transaction: &Value,
) -> Result<(), RpcResponseValidationError> {
    let signed =
        serde_json::from_value::<postfiat_types::SignedAtomicSwapTransaction>(transaction.clone())
            .map_err(|error| invalid_result("transaction", error.to_string()))?;
    signed
        .validate()
        .map_err(|error| invalid_result("transaction", error))?;
    for authorization in [
        field(transaction, "authorization_0")?,
        field(transaction, "authorization_1")?,
    ] {
        expect_string_eq(authorization, "algorithm_id", ML_DSA_65_ALGORITHM)?;
        lower_hex_string_field(authorization, "public_key_hex")?;
        lower_hex_string_field(authorization, "signature_hex")?;
    }
    Ok(())
}

fn validate_atomic_swap_leg_receipts(receipt: &Value) -> Result<(), RpcResponseValidationError> {
    let Some(legs) = receipt.get("atomic_swap_legs") else {
        return Ok(());
    };
    let legs = legs
        .as_array()
        .ok_or_else(|| invalid_result("atomic_swap_legs", "expected array value"))?;
    if legs.len() != 2 {
        return Err(invalid_result(
            "atomic_swap_legs",
            "expected exactly two atomic swap leg receipts",
        ));
    }
    for leg in legs {
        clean_string_field(leg, "owner")?;
        clean_string_field(leg, "recipient")?;
        lower_hex_field(leg, "asset_id", ISSUED_ASSET_ID_HEX_LEN)?;
        nonzero_u64_field(leg, "amount")?;
        nonzero_u64_field(leg, "fee_charged")?;
        let pre_sequence = u64_field(leg, "pre_sequence")?;
        let post_sequence = nonzero_u64_field(leg, "post_sequence")?;
        if pre_sequence.checked_add(1) != Some(post_sequence) {
            return Err(invalid_result(
                "atomic_swap_legs.post_sequence",
                "expected post_sequence to advance exactly once",
            ));
        }
    }
    if string_field(&legs[0], "owner")? != string_field(&legs[1], "recipient")?
        || string_field(&legs[1], "owner")? != string_field(&legs[0], "recipient")?
    {
        return Err(invalid_result(
            "atomic_swap_legs",
            "expected reciprocal atomic swap leg receipts",
        ));
    }
    if string_field(&legs[0], "asset_id")? == string_field(&legs[1], "asset_id")? {
        return Err(invalid_result(
            "atomic_swap_legs",
            "expected distinct atomic swap assets",
        ));
    }
    let leg_fee_total = legs.iter().try_fold(0u64, |total, leg| {
        total
            .checked_add(u64_field(leg, "fee_charged")?)
            .ok_or_else(|| {
                invalid_result(
                    "atomic_swap_legs.fee_charged",
                    "atomic swap leg fee total overflow",
                )
            })
    })?;
    let fee_charged = u64_field(receipt, "fee_charged")?;
    if fee_charged != leg_fee_total {
        return Err(invalid_result(
            "fee_charged",
            "expected outer fee_charged to equal the sum of atomic leg fees",
        ));
    }
    if u64_field(receipt, "fee_burned")? != fee_charged {
        return Err(invalid_result(
            "fee_burned",
            "expected atomic swap fee_burned to equal fee_charged",
        ));
    }
    if u64_field(receipt, "minimum_fee")? > fee_charged {
        return Err(invalid_result(
            "minimum_fee",
            "expected atomic swap minimum_fee not to exceed fee_charged",
        ));
    }
    nonzero_u64_field(receipt, "account_reserve")?;
    Ok(())
}

pub fn decode_atomic_swap_fee_quote_summary(
    response: &RpcResponse,
    request: &RpcRequest,
) -> Result<AtomicSwapFeeQuoteSummary, RpcResponseValidationError> {
    validate_response(response, Some(&request.id), true)?;
    let result = validated_summary_result(response, RpcResponseKind::AtomicSwapFeeQuote)?;
    let leg =
        |field_name: &str| -> Result<AtomicSwapLegFeeQuoteSummary, RpcResponseValidationError> {
            let value = field(result, field_name)?;
            Ok(AtomicSwapLegFeeQuoteSummary {
                owner: clean_string_field(value, "owner")?.to_string(),
                sender_balance: u64_field(value, "sender_balance")?,
                sender_sequence: u64_field(value, "sender_sequence")?,
                sequence: nonzero_u64_field(value, "sequence")?,
                mempool_pending_for_owner: u64_field(value, "mempool_pending_for_owner")?,
                base_atomic_swap_fee: nonzero_u64_field(value, "base_atomic_swap_fee")?,
                state_expansion_fee: u64_field(value, "state_expansion_fee")?,
                minimum_fee: nonzero_u64_field(value, "minimum_fee")?,
                sender_balance_after_fee: optional_u64_field_value(
                    value,
                    "sender_balance_after_fee",
                )?,
                sender_meets_reserve_after_fee: bool_field(
                    value,
                    "sender_meets_reserve_after_fee",
                )?,
            })
        };
    let quote = AtomicSwapFeeQuoteSummary {
        transaction_kind: clean_string_field(result, "transaction_kind")?.to_string(),
        parent_height: u64_field(result, "parent_height")?,
        parent_hash: string_field(result, "parent_hash")?.to_string(),
        parent_state_root: string_field(result, "parent_state_root")?.to_string(),
        quote_height: nonzero_u64_field(result, "quote_height")?,
        account_reserve: nonzero_u64_field(result, "account_reserve")?,
        transfer_fee_byte_quantum: nonzero_u64_field(result, "transfer_fee_byte_quantum")?,
        transfer_fee_per_quantum: nonzero_u64_field(result, "transfer_fee_per_quantum")?,
        atomic_swap_weight_bytes: nonzero_u64_field(result, "atomic_swap_weight_bytes")?,
        leg_0: leg("leg_0")?,
        leg_1: leg("leg_1")?,
        unsigned_transaction: serde_json::from_value(
            field(result, "unsigned_transaction")?.clone(),
        )
        .map_err(|error| invalid_result("unsigned_transaction", error.to_string()))?,
    };
    validate_atomic_swap_quote_summary_for_request(&quote, request)?;
    Ok(quote)
}

pub fn decode_atomic_swap_finality_summary(
    response: &RpcResponse,
    request: &RpcRequest,
) -> Result<AtomicSwapFinalitySummary, RpcResponseValidationError> {
    validate_response(response, Some(&request.id), true)?;
    let binding = atomic_swap_binding_from_finality_request(request)?;
    let result = validated_summary_result(
        response,
        RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality,
    )?;
    validate_atomic_swap_finality_for_transaction(result, &binding)?;
    decode_tx_finality_summary_result(field(result, "finality")?)
}
