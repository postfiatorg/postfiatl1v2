use postfiat_bridge::{
    decode_packet_cancelled_event, decode_packet_consumed_event, decode_return_burned_event,
    ethereum_keccak256, verify_ethereum_checkpoint_certificate, verify_ethereum_receipt_log,
};
use postfiat_crypto_provider::hex_to_bytes;
use postfiat_types::{
    EthereumExternalEventProofV1, EthereumRouteVerificationPolicyV1, FastSwapCommitteeV1,
    FastSwapOpaqueHashV1, Genesis, LedgerState, PftlUniswapConsensusExportPacket,
    PftlUniswapConsensusRouteState, PftlUniswapDestinationConsumeOperation,
    PftlUniswapRefundSourceOperation, PftlUniswapReturnImportOperation,
    PftlUniswapRouteInitOperation,
};

type ExecutionError = (&'static str, String);

pub(crate) fn verify_live_route_initialization(
    genesis: &Genesis,
    ledger: &LedgerState,
    operation: &PftlUniswapRouteInitOperation,
) -> Result<(), ExecutionError> {
    if operation.route_trust_class != postfiat_bridge::ROUTE_TRUST_CLASS_BFT_CHECKPOINT {
        return Err((
            "pftl_uniswap_ethereum_trust_class_mismatch",
            "live checkpoint-verified routes must declare the BFT_CHECKPOINT trust class"
                .to_string(),
        ));
    }
    let policy = operation
        .ethereum_verification_policy
        .as_ref()
        .ok_or_else(missing_external_verification)?;
    if u64::from(policy.minimum_confirmations) != operation.return_finality_blocks {
        return Err((
            "pftl_uniswap_ethereum_policy_finality_mismatch",
            "route finality blocks must exactly match the governed Ethereum verification policy"
                .to_string(),
        ));
    }
    let committee = committee_for_policy(genesis, ledger, policy)?;
    committee.validate().map_err(|error| {
        (
            "bad_pftl_uniswap_ethereum_committee",
            format!("Ethereum checkpoint committee is invalid: {error:?}"),
        )
    })
}

pub(crate) fn verify_live_route_reference(
    genesis: &Genesis,
    ledger: &LedgerState,
    route: &PftlUniswapConsensusRouteState,
) -> Result<(), ExecutionError> {
    if route.route_trust_class != postfiat_bridge::ROUTE_TRUST_CLASS_BFT_CHECKPOINT {
        return Err((
            "pftl_uniswap_ethereum_trust_class_mismatch",
            "live checkpoint-verified route state does not declare BFT_CHECKPOINT".to_string(),
        ));
    }
    let policy = route
        .ethereum_verification_policy
        .as_ref()
        .ok_or_else(missing_external_verification)?;
    if u64::from(policy.minimum_confirmations) != route.return_finality_blocks {
        return Err((
            "pftl_uniswap_ethereum_policy_finality_mismatch",
            "route finality blocks do not match the governed Ethereum verification policy"
                .to_string(),
        ));
    }
    committee_for_policy(genesis, ledger, policy).map(|_| ())
}

pub(crate) fn verify_live_export(
    genesis: &Genesis,
    ledger: &LedgerState,
    route: &PftlUniswapConsensusRouteState,
    operation: &postfiat_types::PftlUniswapExportDebitOperation,
) -> Result<(), ExecutionError> {
    verify_live_route_reference(genesis, ledger, route)?;
    operation
        .ethereum_packet_digest
        .as_ref()
        .ok_or_else(missing_external_verification)?;
    if operation.ethereum_packet_schema_version
        != Some(postfiat_types::PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V1)
    {
        return Err((
            "pftl_uniswap_ethereum_packet_schema_mismatch",
            "live export requires the exact supported Ethereum packet schema version".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn verify_destination_consume(
    genesis: &Genesis,
    ledger: &LedgerState,
    route: &PftlUniswapConsensusRouteState,
    packet: &PftlUniswapConsensusExportPacket,
    operation: &PftlUniswapDestinationConsumeOperation,
) -> Result<(), ExecutionError> {
    verify_live_route_reference(genesis, ledger, route)?;
    let (checkpoint, log) = verified_external_log(
        genesis,
        ledger,
        route,
        operation.external_event_proof.as_ref(),
    )?;
    require_operation_heights(
        checkpoint.block_number,
        checkpoint.observed_head_number,
        operation.consumed_height,
        operation.finalized_height,
    )?;
    let controller = evm_address("handoff controller", &route.handoff_controller)?;
    let event = decode_packet_consumed_event(&log, controller).map_err(external_proof_error)?;
    let source_packet_commitment = ethereum_keccak256(&hex_exact::<48>(
        "source packet hash",
        &operation.packet_hash,
    )?);
    let recipient = evm_address("packet recipient", &packet.ethereum_recipient)?;
    let route_config_commitment = ethereum_keccak256(&hex_exact::<48>(
        "route config digest",
        &route.route_config_digest,
    )?);
    let route_trust_class = ethereum_keccak256(route.route_trust_class.as_bytes());
    let packet_digest = packet
        .ethereum_packet_digest
        .as_ref()
        .ok_or_else(missing_external_verification)
        .and_then(|digest| hex_exact::<32>("Ethereum packet digest", digest))?;
    require_packet_schema(packet)?;
    if event.packet_digest != packet_digest
        || event.source_packet_commitment != source_packet_commitment
        || event.recipient != recipient
        || event.route_config_commitment != route_config_commitment
        || event.route_trust_class != route_trust_class
        || event.mint_amount_atoms != packet.amount_atoms
        || event.settlement_amount_atoms == 0
    {
        return Err((
            "pftl_uniswap_ethereum_event_binding_mismatch",
            "PacketConsumed does not exactly bind the source packet, governed route, recipient, trust class, and amount"
                .to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn verify_source_refund(
    genesis: &Genesis,
    ledger: &LedgerState,
    route: &PftlUniswapConsensusRouteState,
    packet: &PftlUniswapConsensusExportPacket,
    operation: &PftlUniswapRefundSourceOperation,
) -> Result<(), ExecutionError> {
    verify_live_route_reference(genesis, ledger, route)?;
    let (_checkpoint, log) = verified_external_log(
        genesis,
        ledger,
        route,
        operation.external_event_proof.as_ref(),
    )?;
    let controller = evm_address("handoff controller", &route.handoff_controller)?;
    let event = decode_packet_cancelled_event(&log, controller).map_err(external_proof_error)?;
    let source_packet_commitment = ethereum_keccak256(&hex_exact::<48>(
        "source packet hash",
        &operation.packet_hash,
    )?);
    let packet_digest = packet
        .ethereum_packet_digest
        .as_ref()
        .ok_or_else(missing_external_verification)
        .and_then(|digest| hex_exact::<32>("Ethereum packet digest", digest))?;
    require_packet_schema(packet)?;
    if event.packet_digest != packet_digest
        || event.source_packet_commitment != source_packet_commitment
        || event.deadline != packet.destination_deadline_seconds
        || event.cancelled_at <= event.deadline
    {
        return Err((
            "pftl_uniswap_ethereum_cancellation_binding_mismatch",
            "PacketCancelled does not exactly bind the source packet and expired destination deadline"
                .to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn verify_return_import(
    genesis: &Genesis,
    ledger: &LedgerState,
    route: &PftlUniswapConsensusRouteState,
    operation: &PftlUniswapReturnImportOperation,
) -> Result<(), ExecutionError> {
    verify_live_route_reference(genesis, ledger, route)?;
    let (checkpoint, log) = verified_external_log(
        genesis,
        ledger,
        route,
        operation.external_event_proof.as_ref(),
    )?;
    require_operation_heights(
        checkpoint.block_number,
        checkpoint.observed_head_number,
        operation.burn_height,
        operation.finalized_height,
    )?;
    let controller = evm_address("handoff controller", &route.handoff_controller)?;
    let event = decode_return_burned_event(&log, controller).map_err(external_proof_error)?;
    let expected_return_id = hex_exact::<32>("return burn id", &operation.burn_event_hash)?;
    let expected_sender = evm_address("Ethereum sender", &operation.ethereum_sender)?;
    let expected_nonce = hex_exact::<32>("return nonce", &operation.return_nonce)?;
    let expected_asset = hex_exact::<48>("native NAV asset", &operation.native_nav_asset_id)?;
    let expected_wrapped = evm_address("wrapped NAVCoin", &operation.wrapped_navcoin_token)?;
    if event.return_burn_id != expected_return_id
        || event.ethereum_sender != expected_sender
        || event.return_nonce != expected_nonce
        || event.pftl_recipient != operation.pftl_recipient
        || event.native_nav_asset_id != expected_asset
        || event.amount_atoms != operation.amount_atoms
        || event.ethereum_chain_id != operation.ethereum_chain_id
        || event.bridge_controller != controller
        || event.wrapped_navcoin != expected_wrapped
        || event.burn_height != operation.burn_height
    {
        return Err((
            "pftl_uniswap_ethereum_return_binding_mismatch",
            "ReturnBurned does not exactly bind every governed return-import field".to_string(),
        ));
    }
    Ok(())
}

fn verified_external_log(
    genesis: &Genesis,
    ledger: &LedgerState,
    route: &PftlUniswapConsensusRouteState,
    proof: Option<&EthereumExternalEventProofV1>,
) -> Result<
    (
        postfiat_types::EthereumFinalizedCheckpointV1,
        postfiat_bridge::EthereumLogV1,
    ),
    ExecutionError,
> {
    let policy = route
        .ethereum_verification_policy
        .as_ref()
        .ok_or_else(missing_external_verification)?;
    let proof = proof.ok_or_else(missing_external_verification)?;
    let committee = committee_for_policy(genesis, ledger, policy)?;
    let (checkpoint, _) =
        verify_ethereum_checkpoint_certificate(committee, &proof.checkpoint_certificate).map_err(
            |error| {
                (
                    "invalid_pftl_uniswap_ethereum_checkpoint",
                    format!("Ethereum checkpoint certificate is invalid: {error:?}"),
                )
            },
        )?;
    bind_checkpoint_to_route(genesis, route, policy, checkpoint)?;
    let log_index = usize::try_from(proof.log_index).map_err(|_| {
        (
            "invalid_pftl_uniswap_ethereum_log_index",
            "Ethereum log index exceeds this platform's addressable range".to_string(),
        )
    })?;
    let log =
        verify_ethereum_receipt_log(checkpoint.receipts_root, &proof.receipt_proof, log_index)
            .map_err(external_proof_error)?;
    Ok((checkpoint.clone(), log))
}

fn committee_for_policy<'a>(
    genesis: &Genesis,
    ledger: &'a LedgerState,
    policy: &EthereumRouteVerificationPolicyV1,
) -> Result<&'a FastSwapCommitteeV1, ExecutionError> {
    policy.validate().map_err(|error| {
        (
            "bad_pftl_uniswap_ethereum_policy",
            format!("Ethereum route verification policy is invalid: {error:?}"),
        )
    })?;
    let expected_genesis = hex_exact::<48>("genesis hash", &crate::genesis_hash(genesis))?;
    ledger
        .fastswap_committees
        .iter()
        .find(|committee| {
            committee.domain.committee_epoch == policy.authority_epoch
                && committee.domain.committee_root == policy.committee_root
                && committee.domain.chain.chain_id == genesis.chain_id
                && committee.domain.chain.genesis_hash == FastSwapOpaqueHashV1(expected_genesis)
                && committee.domain.chain.protocol_version == genesis.protocol_version
        })
        .ok_or_else(|| {
            (
                "missing_pftl_uniswap_ethereum_committee",
                "no governed committee exactly matches the route's Ethereum verification authority"
                    .to_string(),
            )
        })
}

fn bind_checkpoint_to_route(
    genesis: &Genesis,
    route: &PftlUniswapConsensusRouteState,
    policy: &EthereumRouteVerificationPolicyV1,
    checkpoint: &postfiat_types::EthereumFinalizedCheckpointV1,
) -> Result<(), ExecutionError> {
    let route_digest = FastSwapOpaqueHashV1(hex_exact::<48>(
        "route config digest",
        &route.route_config_digest,
    )?);
    let controller = evm_address("handoff controller", &route.handoff_controller)?;
    let wrapped = evm_address("wrapped NAVCoin", &route.wrapped_navcoin_token)?;
    if checkpoint.pftl_domain.chain_id != genesis.chain_id
        || checkpoint.pftl_domain.genesis_hash
            != FastSwapOpaqueHashV1(hex_exact::<48>(
                "genesis hash",
                &crate::genesis_hash(genesis),
            )?)
        || checkpoint.pftl_domain.protocol_version != genesis.protocol_version
        || checkpoint.route_id != route.route_id
        || checkpoint.route_config_digest != route_digest
        || checkpoint.ethereum_chain_id != route.ethereum_chain_id
        || checkpoint.minimum_confirmations != policy.minimum_confirmations
        || checkpoint.authority_epoch != policy.authority_epoch
        || checkpoint.committee_root != policy.committee_root
        || checkpoint.handoff_controller != controller
        || checkpoint.wrapped_navcoin_token != wrapped
        || checkpoint.handoff_controller_code_hash != policy.handoff_controller_code_hash
        || checkpoint.wrapped_navcoin_code_hash != policy.wrapped_navcoin_code_hash
    {
        return Err((
            "pftl_uniswap_ethereum_checkpoint_route_mismatch",
            "Ethereum checkpoint does not exactly match the chain, route, authority, contracts, and governed code hashes"
                .to_string(),
        ));
    }
    Ok(())
}

fn require_operation_heights(
    checkpoint_block: u64,
    checkpoint_head: u64,
    operation_block: u64,
    operation_head: u64,
) -> Result<(), ExecutionError> {
    if checkpoint_block != operation_block || checkpoint_head != operation_head {
        return Err((
            "pftl_uniswap_ethereum_height_mismatch",
            "operation block/finality heights do not match the certified Ethereum checkpoint"
                .to_string(),
        ));
    }
    Ok(())
}

fn evm_address(field: &'static str, value: &str) -> Result<[u8; 20], ExecutionError> {
    let hex = value.strip_prefix("0x").ok_or_else(|| {
        (
            "bad_pftl_uniswap_ethereum_address",
            format!("{field} is not 0x-prefixed"),
        )
    })?;
    hex_exact(field, hex)
}

fn hex_exact<const N: usize>(field: &'static str, value: &str) -> Result<[u8; N], ExecutionError> {
    let bytes = hex_to_bytes(value).map_err(|_| {
        (
            "bad_pftl_uniswap_ethereum_hex",
            format!("{field} is not canonical hexadecimal"),
        )
    })?;
    bytes.try_into().map_err(|bytes: Vec<u8>| {
        (
            "bad_pftl_uniswap_ethereum_hex_length",
            format!("{field} has {} bytes; expected {N}", bytes.len()),
        )
    })
}

fn missing_external_verification() -> ExecutionError {
    (
        "pftl_uniswap_external_verification_required",
        "live PFTL-Uniswap transitions require a governed Ethereum checkpoint and receipt proof"
            .to_string(),
    )
}

fn require_packet_schema(packet: &PftlUniswapConsensusExportPacket) -> Result<(), ExecutionError> {
    if packet.ethereum_packet_schema_version
        != Some(postfiat_types::PFTL_UNISWAP_EXTERNAL_PACKET_SCHEMA_V1)
    {
        return Err((
            "pftl_uniswap_ethereum_packet_schema_mismatch",
            "external evidence references an unsupported or legacy packet schema".to_string(),
        ));
    }
    Ok(())
}

fn external_proof_error(error: postfiat_bridge::EthereumProofError) -> ExecutionError {
    (
        "invalid_pftl_uniswap_ethereum_receipt_proof",
        format!("{}: {error}", error.code()),
    )
}
