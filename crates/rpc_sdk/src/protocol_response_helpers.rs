pub fn to_pretty_json(response: &RpcResponse) -> Result<String, serde_json::Error> {
    response.to_pretty_json()
}

pub fn response_from_json(raw: &str) -> Result<RpcResponse, serde_json::Error> {
    serde_json::from_str(raw)
}

pub fn read_response_file(path: impl AsRef<Path>) -> io::Result<RpcResponse> {
    let raw = fs::read_to_string(path)?;
    let response = response_from_json(&raw).map_err(invalid_json)?;
    response
        .validate_protocol()
        .map_err(invalid_protocol_data)?;
    Ok(response)
}

pub fn write_response_file(path: impl AsRef<Path>, response: &RpcResponse) -> io::Result<()> {
    response
        .validate_protocol()
        .map_err(invalid_protocol_input)?;
    let json = response.to_pretty_json().map_err(invalid_json)?;
    fs::write(path, json)
}

pub fn validate_response(
    response: &RpcResponse,
    expected_id: Option<&str>,
    require_ok: bool,
) -> Result<(), RpcResponseValidationError> {
    response
        .validate_protocol()
        .map_err(RpcResponseValidationError::Protocol)?;
    if let Some(expected) = expected_id {
        if response.id != expected {
            return Err(RpcResponseValidationError::UnexpectedId {
                expected: expected.to_string(),
                found: response.id.clone(),
            });
        }
    }
    if require_ok && !response.ok {
        let (code, message) = response
            .error
            .as_ref()
            .map(|error| (error.code.clone(), error.message.clone()))
            .unwrap_or_else(|| {
                (
                    "rpc_error".to_string(),
                    "rpc response was not ok".to_string(),
                )
            });
        return Err(RpcResponseValidationError::ExpectedSuccess { code, message });
    }
    Ok(())
}

pub fn validate_response_file(
    path: impl AsRef<Path>,
    expected_id: Option<&str>,
    require_ok: bool,
) -> io::Result<RpcResponse> {
    let response = read_response_file(path)?;
    validate_response(&response, expected_id, require_ok).map_err(invalid_response_validation)?;
    Ok(response)
}

pub fn validate_response_kind(
    response: &RpcResponse,
    kind: RpcResponseKind,
) -> Result<(), RpcResponseValidationError> {
    validate_response_kind_with_context(response, kind, None)
}

pub fn validate_response_kind_with_context(
    response: &RpcResponse,
    kind: RpcResponseKind,
    batch_archive_context: Option<&BatchArchiveValidationContext>,
) -> Result<(), RpcResponseValidationError> {
    validate_response(response, None, true)?;
    let result = response
        .result
        .as_ref()
        .ok_or_else(|| invalid_result("result", "missing successful response result"))?;
    if contains_private_key_material_field(result) {
        return Err(invalid_result(
            "result",
            "response contains private key material fields",
        ));
    }
    match kind {
        RpcResponseKind::Status => validate_status_result(result),
        RpcResponseKind::ServerInfo => validate_server_info_result(result),
        RpcResponseKind::Metrics => validate_metrics_result(result),
        RpcResponseKind::Ledger => validate_ledger_result(result),
        RpcResponseKind::VerifyState => validate_state_verification_result(result),
        RpcResponseKind::ValidateLocalKeys { validators } => {
            validate_local_key_result(result, validators)
        }
        RpcResponseKind::Account => validate_account_result(result),
        RpcResponseKind::AccountTx => validate_account_tx_result(result),
        RpcResponseKind::Fee => validate_fee_result(result),
        RpcResponseKind::TransferFeeQuote => validate_transfer_fee_quote_result(result),
        RpcResponseKind::AtomicSwapFeeQuote => validate_atomic_swap_fee_quote_result(result),
        RpcResponseKind::AssetFeeQuote => validate_asset_fee_quote_result(result),
        RpcResponseKind::EscrowFeeQuote => validate_escrow_fee_quote_result(result),
        RpcResponseKind::NftFeeQuote => validate_nft_fee_quote_result(result),
        RpcResponseKind::OfferFeeQuote => validate_offer_fee_quote_result(result),
        RpcResponseKind::AtomicSettlementTemplate => {
            validate_atomic_settlement_template_result(result)
        }
        RpcResponseKind::OfferInfo => validate_offer_info_result(result),
        RpcResponseKind::AccountOffers => validate_account_offers_result(result),
        RpcResponseKind::BookOffers => validate_book_offers_result(result),
        RpcResponseKind::AssetInfo => validate_asset_info_result(result),
        RpcResponseKind::AccountLines => validate_account_lines_result(result),
        RpcResponseKind::AccountAssets => validate_account_assets_result(result),
        RpcResponseKind::IssuerAssets => validate_issuer_assets_result(result),
        RpcResponseKind::EscrowInfo => validate_escrow_info_result(result),
        RpcResponseKind::AccountEscrows => validate_account_escrows_result(result),
        RpcResponseKind::NftInfo => validate_nft_info_result(result),
        RpcResponseKind::AccountNfts => validate_account_nfts_result(result),
        RpcResponseKind::IssuerNfts => validate_issuer_nfts_result(result),
        RpcResponseKind::Receipts => validate_receipts_result(result),
        RpcResponseKind::Tx => validate_tx_finality_result(result),
        RpcResponseKind::Blocks => validate_blocks_result(result),
        RpcResponseKind::Validators => validate_validators_result(result),
        RpcResponseKind::Manifests => validate_manifests_result(result),
        RpcResponseKind::BatchArchive => {
            validate_batch_archive_result(result, batch_archive_context)
        }
        RpcResponseKind::ArchiveWindow => validate_archive_window_result(result),
        RpcResponseKind::MempoolSubmitTransfer => validate_mempool_entry_result(result),
        RpcResponseKind::MempoolSubmitSignedTransfer => validate_mempool_entry_result(result),
        RpcResponseKind::MempoolSubmitSignedPaymentV2 => {
            validate_mempool_payment_v2_entry_result(result)
        }
        RpcResponseKind::MempoolSubmitSignedAssetTransaction => {
            validate_mempool_asset_transaction_entry_result(result)
        }
        RpcResponseKind::MempoolSubmitSignedEscrowTransaction => {
            validate_mempool_escrow_transaction_entry_result(result)
        }
        RpcResponseKind::MempoolSubmitSignedNftTransaction => {
            validate_mempool_nft_transaction_entry_result(result)
        }
        RpcResponseKind::MempoolSubmitSignedOfferTransaction => {
            validate_mempool_offer_transaction_entry_result(result)
        }
        RpcResponseKind::MempoolSubmitSignedAtomicSwapTransaction => {
            validate_mempool_atomic_swap_entry_result(result)
        }
        RpcResponseKind::MempoolSubmitSignedAtomicSwapTransactionFinality => {
            validate_atomic_swap_finality_result(result)
        }
        RpcResponseKind::MempoolStatus => validate_mempool_status_result(result),
        RpcResponseKind::MempoolBatch => validate_mempool_batch_result(result),
        RpcResponseKind::ApplyBatch => validate_receipts_result(result),
        RpcResponseKind::ShieldBatchMint => {
            validate_shielded_action_batch_result(result, "shield_mint")
        }
        RpcResponseKind::ShieldBatchSpend => {
            validate_shielded_action_batch_result(result, "shield_spend")
        }
        RpcResponseKind::ShieldBatchMigrate => {
            validate_shielded_action_batch_result(result, "shield_migrate")
        }
        RpcResponseKind::ShieldBatchOrchard => {
            validate_shielded_action_batch_result(result, "orchard_action_v1")
        }
        RpcResponseKind::ShieldBatchOrchardDeposit => {
            validate_shielded_action_batch_result(result, "orchard_deposit_v1")
        }
        RpcResponseKind::ShieldBatchOrchardWithdraw => {
            validate_shielded_action_batch_result(result, "orchard_withdraw_v1")
        }
        RpcResponseKind::ShieldBatchSwap => {
            validate_shielded_action_batch_result(result, "shielded_swap_v1")
        }
        RpcResponseKind::ApplyShieldBatch => validate_redacted_receipts_result(result),
        RpcResponseKind::ShieldScan => validate_shield_scan_result(result),
        RpcResponseKind::ShieldDisclose => validate_shield_disclosure_result(result),
        RpcResponseKind::ShieldTurnstile => validate_shield_turnstile_result(result),
        RpcResponseKind::BridgeStatus => validate_bridge_status_result(result),
        RpcResponseKind::NavcoinBridgeRoutes => validate_navcoin_bridge_routes_result(result),
        RpcResponseKind::NavcoinBridgePacket => validate_navcoin_bridge_packet_result(result),
        RpcResponseKind::NavcoinBridgeClaims => validate_navcoin_bridge_claims_result(result),
        RpcResponseKind::NavcoinBridgeSupplyStatus => {
            validate_navcoin_bridge_supply_status_result(result)
        }
        RpcResponseKind::NavcoinBridgeReceiptReplay => {
            validate_navcoin_bridge_receipt_replay_result(result)
        }
        RpcResponseKind::NavcoinBridgePacketPreflight => {
            validate_navcoin_bridge_packet_preflight_result(result)
        }
        RpcResponseKind::BridgeBatchDomain => {
            validate_bridge_action_batch_result(result, "bridge_domain", None)
        }
        RpcResponseKind::BridgeBatchTransfer => {
            validate_bridge_action_batch_result(result, "bridge_transfer", None)
        }
        RpcResponseKind::BridgeBatchPause => {
            validate_bridge_action_batch_result(result, "bridge_pause", Some(true))
        }
        RpcResponseKind::BridgeBatchResume => {
            validate_bridge_action_batch_result(result, "bridge_pause", Some(false))
        }
        RpcResponseKind::ApplyBridgeBatch => validate_receipts_result(result),
        RpcResponseKind::FastSwapCapabilities => validate_fastswap_capabilities_result(result),
        RpcResponseKind::FastSwapPreview => {
            validate_fastswap_preview_result(result, "postfiat-fastswap-preview-v1")
        }
        RpcResponseKind::FastLaneAssetControlPreview => validate_fastswap_preview_result(
            result,
            "postfiat-fastlane-asset-control-preview-v1",
        ),
        RpcResponseKind::FastSwapVote => validate_fastswap_vote_result(result),
        RpcResponseKind::FastSwapStatus => validate_fastswap_status_result(result),
        RpcResponseKind::FastSwapEffects => validate_fastswap_effects_result(result),
        RpcResponseKind::FastSwapVoteEvidence => validate_fastswap_vote_evidence_result(result),
        RpcResponseKind::FastSwapNewRoundVote => validate_fastswap_new_round_vote_result(result),
        RpcResponseKind::FastLaneExitVote => validate_fastlane_exit_vote_result(result),
        RpcResponseKind::FastSwapCheckpointStatus => {
            validate_fastswap_checkpoint_status_result(result)
        }
        RpcResponseKind::FastSwapObjects => validate_fastswap_objects_result(result),
        RpcResponseKind::FastSwapPolicy => validate_fastswap_policy_result(result),
        RpcResponseKind::MempoolSubmitFastLanePrimary => validate_mempool_entry_result(result),
    }
}

fn typed_fastlane_result<T: DeserializeOwned>(
    result: &Value,
    label: &str,
) -> Result<T, RpcResponseValidationError> {
    serde_json::from_value(result.clone())
        .map_err(|error| invalid_result("result", format!("invalid {label}: {error}")))
}

fn validate_fastswap_capabilities_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    let value = typed_fastlane_result::<postfiat_types::FastSwapCapabilitiesV1>(
        result,
        "FastSwap capabilities",
    )?;
    if value.schema != "postfiat-fastswap-capabilities-v1"
        || value.committee.validate().is_err()
        || value.terminal_receipt_code != "fastswap_applied"
    {
        return Err(invalid_result("result", "invalid FastSwap capabilities semantics"));
    }
    Ok(())
}

fn validate_fastswap_preview_result(
    result: &Value,
    schema: &str,
) -> Result<(), RpcResponseValidationError> {
    let value = typed_fastlane_result::<postfiat_types::FastSwapPreviewResponseV1>(
        result,
        "FastSwap preview",
    )?;
    if value.schema != schema
        || value.validator_id.is_empty()
        || value.committee.validate().is_err()
        || value.effects.digest().is_err()
        || !value.effects.receipt.accepted
        || (schema == "postfiat-fastswap-preview-v1"
            && value.effects.receipt.code != "fastswap_applied")
        || (schema == "postfiat-fastlane-asset-control-preview-v1"
            && (value.effects.receipt.code != "fastlane_asset_control_applied"
                || value.effects.policy_hash != postfiat_types::FastSwapPolicyHashV1::ZERO))
    {
        return Err(invalid_result("result", "invalid FastSwap preview semantics"));
    }
    Ok(())
}

fn validate_fastswap_vote_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    let value = typed_fastlane_result::<postfiat_types::FastSwapVoteV1>(result, "FastSwap vote")?;
    if value.validator_id.is_empty() || value.signature.is_empty() || value.signing_bytes().is_err()
    {
        return Err(invalid_result("result", "invalid FastSwap vote semantics"));
    }
    Ok(())
}

fn validate_fastswap_status_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    let value = typed_fastlane_result::<postfiat_types::FastSwapStatusResponseV1>(
        result,
        "FastSwap status",
    )?;
    if value.schema != "postfiat-fastswap-status-v1"
        || value
            .record
            .as_ref()
            .is_some_and(|record| record.swap_id != value.swap_id)
        || value
            .terminal_tombstone
            .as_ref()
            .is_some_and(|row| row.swap_id != value.swap_id)
    {
        return Err(invalid_result("result", "invalid FastSwap status semantics"));
    }
    Ok(())
}

fn validate_fastswap_effects_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    let value = typed_fastlane_result::<postfiat_types::FastSwapEffectsResponseV1>(
        result,
        "FastSwap effects",
    )?;
    if value.schema != "postfiat-fastswap-effects-v1"
        || value.effects.as_ref().is_some_and(|effects| {
            effects.swap_id != value.swap_id || effects.digest().is_err()
        })
    {
        return Err(invalid_result("result", "invalid FastSwap effects semantics"));
    }
    Ok(())
}

fn validate_fastswap_vote_evidence_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    let value = typed_fastlane_result::<postfiat_types::FastSwapVoteEvidenceResponseV1>(
        result,
        "FastSwap vote evidence",
    )?;
    let phase_vote_valid = value.vote.as_ref().is_some_and(|vote| {
        value.phase != postfiat_types::FastSwapPhaseV1::NewRound
            && vote.validator_id == value.validator_id
            && vote.swap_id == value.swap_id
            && vote.phase == value.phase
            && vote.round == value.round
            && vote.signing_bytes().is_ok()
            && !vote.signature.is_empty()
    });
    let new_round_valid = value.new_round_vote.as_ref().is_some_and(|vote| {
        value.phase == postfiat_types::FastSwapPhaseV1::NewRound
            && vote.validator_id == value.validator_id
            && vote.swap_id == value.swap_id
            && vote.target_round == value.round
            && vote.signing_bytes().is_ok()
            && !vote.signature.is_empty()
    });
    let certificate_valid = value.certificate.as_ref().is_none_or(|certificate| {
        certificate.validate_canonical_order().is_ok()
            && value.phase != postfiat_types::FastSwapPhaseV1::NewRound
            && certificate.votes.iter().all(|vote| {
                !vote.validator_id.is_empty()
                    && !vote.signature.is_empty()
                    && vote.signing_bytes().is_ok()
                    && vote.swap_id == value.swap_id
                    && vote.phase == value.phase
                    && vote.round == value.round
            })
            && certificate.votes.first().is_some_and(|vote| {
                vote.swap_id == value.swap_id
                    && vote.phase == value.phase
                    && vote.round == value.round
            })
    });
    if value.schema != "postfiat-fastswap-vote-evidence-v1"
        || value.validator_id.is_empty()
        || (value.vote.is_some() && value.new_round_vote.is_some())
        || (value.vote.is_some() && !phase_vote_valid)
        || (value.new_round_vote.is_some() && !new_round_valid)
        || !certificate_valid
    {
        return Err(invalid_result(
            "result",
            "invalid FastSwap vote evidence semantics",
        ));
    }
    Ok(())
}

fn validate_fastswap_new_round_vote_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    let value = typed_fastlane_result::<postfiat_types::FastSwapNewRoundVoteV1>(
        result,
        "FastSwap new-round vote",
    )?;
    if value.validator_id.is_empty() || value.signature.is_empty() || value.signing_bytes().is_err()
    {
        return Err(invalid_result("result", "invalid FastSwap new-round vote semantics"));
    }
    Ok(())
}

fn validate_fastlane_exit_vote_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    let value =
        typed_fastlane_result::<postfiat_types::FastLaneExitVoteV1>(result, "FastLane exit vote")?;
    if value.validator_id.is_empty() || value.signature.is_empty() || value.signing_bytes().is_err()
    {
        return Err(invalid_result("result", "invalid FastLane exit vote semantics"));
    }
    Ok(())
}

fn validate_fastswap_checkpoint_status_result(
    result: &Value,
) -> Result<(), RpcResponseValidationError> {
    let value = typed_fastlane_result::<postfiat_types::FastLaneCheckpointStatusV1>(
        result,
        "FastLane checkpoint status",
    )?;
    if value.schema != "postfiat-fastlane-checkpoint-status-v1"
        || value.checkpoint.canonical_bytes().is_err()
        || value.vote.checkpoint != value.checkpoint
        || value.vote.validator_id.is_empty()
        || value.vote.signature.is_empty()
        || value.vote.signing_bytes().is_err()
        || value.drain_ready != value.checkpoint.drain_ready
        || (value.rotation_ready && !value.drain_ready)
    {
        return Err(invalid_result("result", "invalid FastLane checkpoint semantics"));
    }
    Ok(())
}

fn validate_fastswap_objects_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    let value = typed_fastlane_result::<postfiat_types::FastSwapObjectsResponseV1>(
        result,
        "FastSwap objects",
    )?;
    if value.schema != "postfiat-fastswap-objects-v1"
        || value.validator_id.is_empty()
        || value.committee.validate().is_err()
        || !value
            .objects
            .windows(2)
            .all(|pair| pair[0].key < pair[1].key)
        || value
            .objects
            .iter()
            .any(|object| object.canonical_bytes().is_err())
    {
        return Err(invalid_result("result", "invalid FastSwap objects semantics"));
    }
    Ok(())
}

fn validate_fastswap_policy_result(result: &Value) -> Result<(), RpcResponseValidationError> {
    let value = typed_fastlane_result::<postfiat_types::FastSwapPolicyResponseV1>(
        result,
        "FastSwap policy",
    )?;
    if value.schema != "postfiat-fastswap-policy-v1"
        || value.validator_id.is_empty()
        || value
            .policy
            .as_ref()
            .is_some_and(|policy| policy.validate().is_err())
    {
        return Err(invalid_result("result", "invalid FastSwap policy semantics"));
    }
    Ok(())
}

pub fn decode_transfer_fee_quote_summary(
    response: &RpcResponse,
) -> Result<TransferFeeQuoteSummary, RpcResponseValidationError> {
    let result = validated_summary_result(response, RpcResponseKind::TransferFeeQuote)?;
    Ok(TransferFeeQuoteSummary {
        chain_id: clean_string_field(result, "chain_id")?.to_string(),
        genesis_hash: string_field(result, "genesis_hash")?.to_string(),
        protocol_version: nonzero_u32_field(result, "protocol_version")?,
        from: clean_string_field(result, "from")?.to_string(),
        to: clean_string_field(result, "to")?.to_string(),
        amount: nonzero_u64_field(result, "amount")?,
        sequence: nonzero_u64_field(result, "sequence")?,
        sequence_source: clean_string_field(result, "sequence_source")?.to_string(),
        sender_balance: u64_field(result, "sender_balance")?,
        sender_sequence: u64_field(result, "sender_sequence")?,
        mempool_pending_for_sender: u64_field(result, "mempool_pending_for_sender")?,
        recipient_exists: bool_field(result, "recipient_exists")?,
        will_create_recipient_account: bool_field(result, "will_create_recipient_account")?,
        base_transfer_fee: nonzero_u64_field(result, "base_transfer_fee")?,
        state_expansion_fee: u64_field(result, "state_expansion_fee")?,
        minimum_fee: nonzero_u64_field(result, "minimum_fee")?,
        account_reserve: nonzero_u64_field(result, "account_reserve")?,
        transfer_account_creation_fee: nonzero_u64_field(result, "transfer_account_creation_fee")?,
        transfer_fee_byte_quantum: nonzero_u64_field(result, "transfer_fee_byte_quantum")?,
        transfer_fee_per_quantum: nonzero_u64_field(result, "transfer_fee_per_quantum")?,
        transfer_weight_bytes: nonzero_u64_field(result, "transfer_weight_bytes")?,
        sender_balance_after_amount_and_fee: optional_u64_field_value(
            result,
            "sender_balance_after_amount_and_fee",
        )?,
        sender_meets_reserve_after_transfer: bool_field(
            result,
            "sender_meets_reserve_after_transfer",
        )?,
        recipient_balance_after_amount: optional_u64_field_value(
            result,
            "recipient_balance_after_amount",
        )?,
        recipient_meets_reserve_after_transfer: bool_field(
            result,
            "recipient_meets_reserve_after_transfer",
        )?,
    })
}

pub fn decode_asset_fee_quote_summary(
    response: &RpcResponse,
) -> Result<AssetFeeQuoteSummary, RpcResponseValidationError> {
    let result = validated_summary_result(response, RpcResponseKind::AssetFeeQuote)?;
    Ok(AssetFeeQuoteSummary {
        chain_id: clean_string_field(result, "chain_id")?.to_string(),
        genesis_hash: string_field(result, "genesis_hash")?.to_string(),
        protocol_version: nonzero_u32_field(result, "protocol_version")?,
        transaction_kind: clean_string_field(result, "transaction_kind")?.to_string(),
        source: clean_string_field(result, "source")?.to_string(),
        sequence: nonzero_u64_field(result, "sequence")?,
        sequence_source: clean_string_field(result, "sequence_source")?.to_string(),
        sender_balance: u64_field(result, "sender_balance")?,
        sender_sequence: u64_field(result, "sender_sequence")?,
        mempool_pending_for_sender: u64_field(result, "mempool_pending_for_sender")?,
        base_asset_fee: nonzero_u64_field(result, "base_asset_fee")?,
        state_expansion_fee: u64_field(result, "state_expansion_fee")?,
        minimum_fee: nonzero_u64_field(result, "minimum_fee")?,
        account_reserve: nonzero_u64_field(result, "account_reserve")?,
        transfer_fee_byte_quantum: nonzero_u64_field(result, "transfer_fee_byte_quantum")?,
        transfer_fee_per_quantum: nonzero_u64_field(result, "transfer_fee_per_quantum")?,
        asset_weight_bytes: nonzero_u64_field(result, "asset_weight_bytes")?,
        sender_balance_after_fee: optional_u64_field_value(result, "sender_balance_after_fee")?,
        sender_meets_reserve_after_fee: bool_field(result, "sender_meets_reserve_after_fee")?,
    })
}

pub fn decode_escrow_fee_quote_summary(
    response: &RpcResponse,
) -> Result<EscrowFeeQuoteSummary, RpcResponseValidationError> {
    let result = validated_summary_result(response, RpcResponseKind::EscrowFeeQuote)?;
    Ok(EscrowFeeQuoteSummary {
        chain_id: clean_string_field(result, "chain_id")?.to_string(),
        genesis_hash: string_field(result, "genesis_hash")?.to_string(),
        protocol_version: nonzero_u32_field(result, "protocol_version")?,
        transaction_kind: clean_string_field(result, "transaction_kind")?.to_string(),
        source: clean_string_field(result, "source")?.to_string(),
        sequence: nonzero_u64_field(result, "sequence")?,
        sequence_source: clean_string_field(result, "sequence_source")?.to_string(),
        sender_balance: u64_field(result, "sender_balance")?,
        sender_sequence: u64_field(result, "sender_sequence")?,
        mempool_pending_for_sender: u64_field(result, "mempool_pending_for_sender")?,
        base_escrow_fee: nonzero_u64_field(result, "base_escrow_fee")?,
        state_expansion_fee: u64_field(result, "state_expansion_fee")?,
        minimum_fee: nonzero_u64_field(result, "minimum_fee")?,
        account_reserve: nonzero_u64_field(result, "account_reserve")?,
        transfer_fee_byte_quantum: nonzero_u64_field(result, "transfer_fee_byte_quantum")?,
        transfer_fee_per_quantum: nonzero_u64_field(result, "transfer_fee_per_quantum")?,
        escrow_weight_bytes: nonzero_u64_field(result, "escrow_weight_bytes")?,
        sender_balance_after_fee: optional_u64_field_value(result, "sender_balance_after_fee")?,
        sender_meets_reserve_after_fee: bool_field(result, "sender_meets_reserve_after_fee")?,
    })
}

pub fn decode_mempool_submit_transfer_summary(
    response: &RpcResponse,
) -> Result<MempoolSubmitSummary, RpcResponseValidationError> {
    decode_mempool_submit_summary(response, RpcResponseKind::MempoolSubmitTransfer)
}

pub fn decode_mempool_submit_signed_transfer_summary(
    response: &RpcResponse,
) -> Result<MempoolSubmitSummary, RpcResponseValidationError> {
    decode_mempool_submit_summary(response, RpcResponseKind::MempoolSubmitSignedTransfer)
}

pub fn decode_mempool_submit_signed_payment_v2_summary(
    response: &RpcResponse,
) -> Result<MempoolSubmitSummary, RpcResponseValidationError> {
    decode_mempool_submit_summary(response, RpcResponseKind::MempoolSubmitSignedPaymentV2)
}

pub fn tx_finality_request_from_submit(
    id: impl Into<String>,
    submit: &MempoolSubmitSummary,
) -> RpcRequest {
    tx_request(id, submit.tx_id.clone())
}

pub fn decode_tx_finality_summary(
    response: &RpcResponse,
) -> Result<TxFinalitySummary, RpcResponseValidationError> {
    decode_tx_finality_summary_for_kind(response, RpcResponseKind::Tx)
}

fn decode_tx_finality_summary_for_kind(
    response: &RpcResponse,
    kind: RpcResponseKind,
) -> Result<TxFinalitySummary, RpcResponseValidationError> {
    let result = validated_summary_result(response, kind)?;
    decode_tx_finality_summary_result(result)
}

fn decode_tx_finality_summary_result(
    result: &Value,
) -> Result<TxFinalitySummary, RpcResponseValidationError> {
    let receipt = field(result, "receipt")?;
    let block = field(result, "block")?;
    let certificate = field(block, "header.certificate")?;
    Ok(TxFinalitySummary {
        proof_id: string_field(result, "proof_id")?.to_string(),
        chain_id: clean_string_field(result, "chain_id")?.to_string(),
        genesis_hash: string_field(result, "genesis_hash")?.to_string(),
        protocol_version: nonzero_u32_field(result, "protocol_version")?,
        tx_id: string_field(result, "tx_id")?.to_string(),
        accepted: bool_field(receipt, "accepted")?,
        receipt_code: clean_string_field(receipt, "code")?.to_string(),
        receipt_message: clean_string_field(receipt, "message")?.to_string(),
        receipt_index: u64_field(result, "receipt_index")?,
        receipt_count: nonzero_u64_field(result, "receipt_count")?,
        block_height: nonzero_u64_field(block, "header.height")?,
        block_hash: string_field(block, "header.block_hash")?.to_string(),
        state_root: string_field(block, "header.state_root")?.to_string(),
        certificate_id: string_field(block, "header.certificate_id")?.to_string(),
        registry_root: string_field(certificate, "registry_root")?.to_string(),
        quorum: nonzero_u64_field(certificate, "quorum")?,
        validator_count: array_field(certificate, "validators")?.len() as u64,
        vote_count: array_field(certificate, "votes")?.len() as u64,
        block_count: nonzero_u64_field(result, "block_count")?,
        tip_hash: string_field(result, "tip_hash")?.to_string(),
        tip_state_root: string_field(result, "tip_state_root")?.to_string(),
    })
}

pub fn decode_account_summary(
    response: &RpcResponse,
) -> Result<AccountSummary, RpcResponseValidationError> {
    let result = validated_summary_result(response, RpcResponseKind::Account)?;
    Ok(AccountSummary {
        address: clean_string_field(result, "address")?.to_string(),
        balance: u64_field(result, "balance")?,
        sequence: u64_field(result, "sequence")?,
        public_key_hex: optional_lower_hex_string_field_value(result, "public_key_hex")?,
    })
}

pub fn decode_receipt_summaries(
    response: &RpcResponse,
) -> Result<Vec<ReceiptSummary>, RpcResponseValidationError> {
    let result = validated_summary_result(response, RpcResponseKind::Receipts)?;
    receipt_summaries_from_result(result)
}

fn decode_mempool_submit_summary(
    response: &RpcResponse,
    kind: RpcResponseKind,
) -> Result<MempoolSubmitSummary, RpcResponseValidationError> {
    let result = validated_summary_result(response, kind)?;
    let envelope_field = if matches!(kind, RpcResponseKind::MempoolSubmitSignedPaymentV2) {
        "payment"
    } else {
        "transfer"
    };
    let unsigned = field(result, &format!("{envelope_field}.unsigned"))?;
    Ok(MempoolSubmitSummary {
        tx_id: string_field(result, "tx_id")?.to_string(),
        chain_id: clean_string_field(unsigned, "chain_id")?.to_string(),
        genesis_hash: string_field(unsigned, "genesis_hash")?.to_string(),
        protocol_version: nonzero_u32_field(unsigned, "protocol_version")?,
        from: clean_string_field(unsigned, "from")?.to_string(),
        to: clean_string_field(unsigned, "to")?.to_string(),
        amount: nonzero_u64_field(unsigned, "amount")?,
        fee: nonzero_u64_field(unsigned, "fee")?,
        sequence: u64_field(unsigned, "sequence")?,
        algorithm_id: clean_string_field(field(result, envelope_field)?, "algorithm_id")?
            .to_string(),
    })
}

fn receipt_summaries_from_result(
    result: &Value,
) -> Result<Vec<ReceiptSummary>, RpcResponseValidationError> {
    result_array(result, "result")?
        .iter()
        .map(|receipt| {
            Ok(ReceiptSummary {
                tx_id: clean_string_field(receipt, "tx_id")?.to_string(),
                accepted: bool_field(receipt, "accepted")?,
                code: clean_string_field(receipt, "code")?.to_string(),
                message: clean_string_field(receipt, "message")?.to_string(),
            })
        })
        .collect()
}

fn validated_summary_result(
    response: &RpcResponse,
    kind: RpcResponseKind,
) -> Result<&Value, RpcResponseValidationError> {
    validate_response_kind(response, kind)?;
    response
        .result
        .as_ref()
        .ok_or_else(|| invalid_result("result", "missing successful response result"))
}
